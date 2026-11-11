//! Guarded synthetic filesystem boundary, without draft lifecycle or save authority.
use crate::record_data::{OwnerScope, RecordIdentity};
use edu_academy_core::{
    js::{Fault, Result},
    workflow::store::{Guard, LockOptions, lock_path},
};
use rustix::fs::{self as rfs, Dir, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs::{File, Permissions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, PermissionsExt},
        },
    },
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

pub const MARKER: &str = ".academy-record-synthetic-v1";
pub const MAX_FILE_BYTES: usize = 65_536;
pub const MAX_ROOT_BYTES: u64 = 64 * 1024 * 1024;
pub const LOCK_WAIT_MS: u64 = 2_000;
pub const LOCK_POLL_MS: u64 = 20;
pub const MAX_LOCK_WAITS: usize = (LOCK_WAIT_MS / LOCK_POLL_MS) as usize;
const DIRS: &[&str] = &[
    "locks",
    "record-drafts",
    "observation",
    "review",
    "request",
    "execution",
    "outcome",
    "reconciliation",
    "ledger",
    "audit",
];
fn unavailable() -> Fault {
    Fault::error("record_store_unavailable")
}
fn refused() -> Fault {
    Fault::error("record_store_not_synthetic")
}
fn capacity() -> Fault {
    Fault::error("record_store_capacity")
}
fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| unavailable())
}
fn os_error(e: rustix::io::Errno) -> Fault {
    match e {
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => refused(),
        _ => unavailable(),
    }
}
fn syscall<T>(r: rustix::io::Result<T>) -> Result<T> {
    r.map_err(os_error)
}
fn flags() -> OFlags {
    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK
}
fn private(f: &File, directory: bool) -> Result<()> {
    let m = io(f.metadata())?;
    let kind_ok = if directory {
        m.is_dir()
    } else {
        m.is_file() && m.nlink() == 1
    };
    let mode = if directory { 0o700 } else { 0o600 };
    if m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o777 != mode || !kind_ok {
        return Err(refused());
    }
    Ok(())
}
fn directory(parent: &File, name: &OsStr, check: bool) -> Result<File> {
    let opts = flags() | OFlags::DIRECTORY;
    let f: File = syscall(rfs::openat(parent, name, opts, Mode::empty()))?.into();
    if check {
        private(&f, true)?;
    }
    Ok(f)
}
fn absolute(path: &Path) -> Result<File> {
    let normal = path
        .components()
        .all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    if !path.is_absolute() || !normal {
        return Err(refused());
    }
    let mut f: File = syscall(rfs::open("/", flags() | OFlags::DIRECTORY, Mode::empty()))?.into();
    for c in path.components() {
        if let Component::Normal(name) = c {
            f = directory(&f, name, false)?;
        }
    }
    Ok(f)
}
fn regular(parent: &File, name: &OsStr, create: bool) -> Result<Option<File>> {
    let extra = if create {
        OFlags::RDWR | OFlags::CREATE
    } else {
        OFlags::RDONLY
    };
    let fd = match rfs::openat(parent, name, flags() | extra, Mode::from_raw_mode(0o600)) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) if !create => return Ok(None),
        Err(e) => return Err(os_error(e)),
    };
    let f: File = fd.into();
    private(&f, false)?;
    Ok(Some(f))
}
fn read_at(parent: &File, name: &OsStr) -> Result<Option<Vec<u8>>> {
    let Some(f) = regular(parent, name, false)? else {
        return Ok(None);
    };
    if io(f.metadata())?.len() > MAX_FILE_BYTES as u64 {
        return Err(unavailable());
    }
    let mut bytes = Vec::new();
    io(f.take((MAX_FILE_BYTES + 1) as u64).read_to_end(&mut bytes))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(unavailable());
    }
    Ok(Some(bytes))
}
fn atomic(parent: &File, name: &OsStr, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(unavailable());
    }
    read_at(parent, name)?; // Refuse unsafe existing targets before creating a temp.
    let tmp = format!(".record-write-{}.tmp", std::process::id());
    if regular(parent, OsStr::new(&tmp), false)?.is_some() {
        return Err(unavailable());
    }
    let mut f: File = syscall(rfs::openat(
        parent,
        tmp.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    ))?
    .into();
    io(f.set_permissions(Permissions::from_mode(0o600)))?;
    let written = io(f.write_all(bytes).and_then(|()| f.sync_all()));
    let result = written.and_then(|()| syscall(rfs::renameat(parent, tmp.as_str(), parent, name)));
    if result.is_err() {
        let _ = rfs::unlinkat(parent, tmp.as_str(), rfs::AtFlags::empty());
    }
    result?;
    io(parent.sync_all())
}
fn entries(f: &File) -> Result<Dir> {
    syscall(Dir::read_from(f))
}
fn name(entry: &rustix::fs::DirEntry) -> Option<&OsStr> {
    let n = entry.file_name().to_bytes();
    (!matches!(n, b"." | b"..")).then(|| OsStr::from_bytes(n))
}
fn poll_allowed(elapsed_ms: u128, waits: usize) -> bool {
    elapsed_ms < u128::from(LOCK_WAIT_MS) && waits < MAX_LOCK_WAITS
}
fn bootstrap_lock(f: &File) -> Result<()> {
    let start = Instant::now();
    let mut waits = 0;
    loop {
        match f.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) => (),
            Err(_) => return Err(unavailable()),
        }
        if !poll_allowed(start.elapsed().as_millis(), waits) {
            return Err(Fault::error("shared_writer_lock_busy"));
        }
        std::thread::sleep(Duration::from_millis(LOCK_POLL_MS));
        waits += 1;
    }
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Marker {
    format: String,
    owner: OwnerScope,
}
pub struct DraftStore {
    path: PathBuf,
    root: File,
    owner: OwnerScope,
}
pub struct AccountGuard<'a> {
    store: &'a DraftStore,
    _lock: Guard,
}
pub struct DraftGuard<'a> {
    account: &'a AccountGuard<'a>,
    _lock: Guard,
}
#[derive(Debug, PartialEq, Eq)]
pub struct RootUsage {
    pub bytes: u64,
    pub entries: usize,
}
impl DraftStore {
    pub fn open(root: &Path, owner: OwnerScope) -> Result<Self> {
        if owner.owner.is_empty() || owner.account.is_empty() {
            return Err(refused());
        }
        let parent = absolute(root.parent().ok_or_else(refused)?)?;
        let leaf = root.file_name().ok_or_else(refused)?;
        match rfs::mkdirat(&parent, leaf, Mode::from_raw_mode(0o700)) {
            Ok(()) => {
                io(parent.sync_all())?;
            }
            Err(rustix::io::Errno::EXIST) => (),
            Err(e) => return Err(os_error(e)),
        }
        let fd = directory(&parent, leaf, false)?;
        let metadata = io(fd.metadata())?;
        if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o022 != 0 {
            return Err(refused());
        }
        bootstrap_lock(&fd)?;
        let expected = Marker {
            format: MARKER.into(),
            owner: owner.clone(),
        };
        match read_at(&fd, OsStr::new(MARKER)).map_err(|_| refused())? {
            Some(bytes) => {
                let actual: Marker = serde_json::from_slice(&bytes).map_err(|_| refused())?;
                if actual != expected {
                    return Err(refused());
                }
                private(&fd, true)?;
            }
            None => {
                for entry in entries(&fd)? {
                    if name(&syscall(entry)?).is_some() {
                        return Err(refused());
                    }
                }
                io(fd.set_permissions(Permissions::from_mode(0o700)))?;
                atomic(
                    &fd,
                    OsStr::new(MARKER),
                    &serde_json::to_vec(&expected).map_err(|_| refused())?,
                )?;
            }
        }
        // Validate all existing directories before initializing any new ones.
        for d in DIRS {
            match rfs::openat(&fd, *d, flags() | OFlags::DIRECTORY, Mode::empty()) {
                Ok(child) => private(&File::from(child), true)?,
                Err(rustix::io::Errno::NOENT) => (),
                Err(e) => return Err(os_error(e)),
            }
        }
        for d in DIRS {
            match rfs::mkdirat(&fd, *d, Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => (),
                Err(e) => return Err(os_error(e)),
            }
            directory(&fd, OsStr::new(d), true)?;
        }
        io(fd.sync_all())?;
        io(fd.unlock())?;
        Ok(Self {
            path: root.into(),
            root: fd,
            owner,
        })
    }
    pub fn root(&self) -> &Path {
        &self.path
    }
    fn verify(&self) -> Result<()> {
        let current = absolute(&self.path)?;
        private(&current, true)?;
        let a = io(current.metadata())?;
        let b = io(self.root.metadata())?;
        if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
            return Err(refused());
        }
        let bytes = read_at(&current, OsStr::new(MARKER))
            .map_err(|_| refused())?
            .ok_or_else(refused)?;
        let marker: Marker = serde_json::from_slice(&bytes).map_err(|_| refused())?;
        if marker.format != MARKER || marker.owner != self.owner {
            return Err(refused());
        }
        Ok(())
    }
    fn parent(&self, relative: &Path) -> Result<(File, OsString)> {
        let parts = relative
            .components()
            .map(|c| {
                if let Component::Normal(n) = c {
                    Ok(n)
                } else {
                    Err(refused())
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let (leaf, parents) = parts.split_last().ok_or_else(refused)?;
        let mut dir = io(self.root.try_clone())?;
        for part in parents {
            dir = directory(&dir, part, true)?;
        }
        Ok((dir, leaf.to_os_string()))
    }
    fn lock_file(&self, relative: &Path) -> Result<Guard> {
        let (parent, leaf) = self.parent(relative)?;
        let f = regular(&parent, &leaf, true)?.ok_or_else(unavailable)?;
        // Pin the validated inode for the existing path-only lock API.
        let pinned = PathBuf::from(format!("/proc/self/fd/{}", f.as_raw_fd()));
        lock_path(
            &pinned,
            LockOptions {
                wait: Duration::from_millis(LOCK_WAIT_MS),
                poll: Duration::from_millis(LOCK_POLL_MS),
            },
            &|| false,
        )
    }
    pub fn lock_account(&self) -> Result<AccountGuard<'_>> {
        self.verify()?;
        let lock = self.lock_file(Path::new("locks/account.flock"))?;
        self.verify()?;
        Ok(AccountGuard {
            store: self,
            _lock: lock,
        })
    }
}
impl AccountGuard<'_> {
    pub fn lock_draft(&self, identity: &RecordIdentity) -> Result<DraftGuard<'_>> {
        self.store.verify()?;
        if identity.owner != self.store.owner {
            return Err(refused());
        }
        let lock = self
            .store
            .lock_file(&PathBuf::from(format!("locks/{}.flock", identity.key()?)))?;
        Ok(DraftGuard {
            account: self,
            _lock: lock,
        })
    }
    pub fn read_bytes(&self, relative: &Path) -> Result<Option<Vec<u8>>> {
        self.store.verify()?;
        let (parent, leaf) = self.store.parent(relative)?;
        read_at(&parent, &leaf)
    }
    /// Counts regular-file bytes, including marker, locks and orphan temp payloads.
    /// The caller supplies its operation's entry budget; stop at budget + 1.
    pub fn usage(&self, entry_limit: usize) -> Result<RootUsage> {
        self.store.verify()?;
        let mut total = RootUsage {
            bytes: 0,
            entries: 0,
        };
        let mut pending = vec![io(self.store.root.try_clone())?];
        while let Some(dir) = pending.pop() {
            for entry in entries(&dir)? {
                let entry = syscall(entry)?;
                let Some(n) = name(&entry) else {
                    continue;
                };
                total.entries = total.entries.checked_add(1).ok_or_else(capacity)?;
                if total.entries > entry_limit {
                    return Err(capacity());
                }
                let f: File = syscall(rfs::openat(&dir, n, flags(), Mode::empty()))?.into();
                let m = io(f.metadata())?;
                if m.is_dir() {
                    private(&f, true)?;
                    pending.push(f);
                } else {
                    private(&f, false)?;
                    total.bytes = total.bytes.checked_add(m.len()).ok_or_else(capacity)?;
                    if total.bytes > MAX_ROOT_BYTES {
                        return Err(capacity());
                    }
                }
            }
        }
        Ok(total)
    }
}
impl DraftGuard<'_> {
    /// Raw atomic file replacement only, not a draft create/replace operation.
    /// After an I/O failure, read the target before retrying (rename may have run).
    pub fn write_atomic(&self, relative: &Path, bytes: &[u8]) -> Result<()> {
        let first = relative.components().next().ok_or_else(refused)?;
        if matches!(first, Component::Normal(n) if n == MARKER || n == "locks") {
            return Err(refused());
        }
        self.account.store.verify()?;
        let (parent, leaf) = self.account.store.parent(relative)?;
        if leaf.to_string_lossy().starts_with(".record-write-") {
            return Err(refused());
        }
        atomic(&parent, &leaf, bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_budget_boundaries() {
        let end = u128::from(LOCK_WAIT_MS);
        assert!(poll_allowed(end - 1, MAX_LOCK_WAITS - 1));
        for over in [0, 1] {
            assert!(!poll_allowed(end + over, 0));
            assert!(!poll_allowed(0, MAX_LOCK_WAITS + over as usize));
        }
        assert_eq!(MAX_LOCK_WAITS as u64 * LOCK_POLL_MS, LOCK_WAIT_MS);
    }
}
