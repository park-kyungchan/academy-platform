//! Explicit owner roots and bounded, descriptor-relative immutable-file reads.
use crate::{Error, Result, digest};
use rustix::{
    fd::OwnedFd,
    fs::{self, FileType, Mode, OFlags},
};
use std::path::{Component, Path, PathBuf};

fn refusal<T>(value: rustix::io::Result<T>) -> Result<T> {
    value.map_err(|_| Error::new("release_artifact_unavailable"))
}
pub fn canonical_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && path.to_str().is_some_and(|s| {
            let joined = path
                .components()
                .filter_map(|c| match c {
                    Component::Normal(v) => v.to_str(),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            s == format!("/{joined}")
        })
}
fn directory(stat: &fs::Stat, uid: u32, private: bool) -> Result<()> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || (stat.st_uid != uid && stat.st_uid != 0)
        || stat.st_mode & 0o022 != 0
        || (private && (stat.st_uid != uid || stat.st_mode & 0o077 != 0))
    {
        return Err(Error::new("release_owner_directory_unqualified"));
    }
    Ok(())
}
fn flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK
}

pub struct OwnerDirectory {
    path: PathBuf,
    fd: OwnedFd,
    uid: u32,
    device: u64,
    inode: u64,
}
impl OwnerDirectory {
    pub fn open(path: &Path) -> Result<Self> {
        if !canonical_absolute(path) {
            return Err(Error::new("explicit_canonical_root_required"));
        }
        let uid = rustix::process::geteuid().as_raw();
        let mut fd = refusal(fs::open("/", flags(), Mode::empty()))?;
        directory(&refusal(fs::fstat(&fd))?, uid, false)?;
        let names = path
            .components()
            .filter_map(|c| match c {
                Component::Normal(v) => Some(v),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (i, name) in names.iter().enumerate() {
            fd = refusal(fs::openat(&fd, *name, flags(), Mode::empty()))?;
            directory(&refusal(fs::fstat(&fd))?, uid, i + 1 == names.len())?;
        }
        let stat = refusal(fs::fstat(&fd))?;
        Ok(Self {
            path: path.into(),
            fd,
            uid,
            device: stat.st_dev,
            inode: stat.st_ino,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn revalidate(&self) -> Result<()> {
        let current = Self::open(&self.path)?;
        if current.device != self.device || current.inode != self.inode || current.uid != self.uid {
            return Err(Error::new("release_root_identity_changed"));
        }
        Ok(())
    }
    /// Ordinary product receipts, exclusively created below the admitted FD.
    /// No caller payload/path, replacement workflow journal or symlink traversal.
    pub fn write_receipt(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if name.is_empty()
            || name.len() > 100
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
            || !name.ends_with(".json")
        {
            return Err(Error::new("receipt_name_unqualified"));
        }
        self.revalidate()?;
        match fs::mkdirat(&self.fd, "receipts", Mode::from_raw_mode(0o700)) {
            Ok(()) => (),
            Err(rustix::io::Errno::EXIST) => (),
            Err(_) => return Err(Error::new("receipt_write_unavailable_reconcile")),
        }
        let dir = fs::openat(&self.fd, "receipts", flags(), Mode::empty())
            .map_err(|_| Error::new("receipt_write_unavailable_reconcile"))?;
        directory(&refusal(fs::fstat(&dir))?, self.uid, true)?;
        let fd = fs::openat(
            &dir,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| Error::new("receipt_write_unavailable_reconcile"))?;
        let mut offset = 0;
        while offset < bytes.len() {
            let n = rustix::io::write(&fd, &bytes[offset..])
                .map_err(|_| Error::new("receipt_write_unavailable_reconcile"))?;
            if n == 0 {
                return Err(Error::new("receipt_write_unavailable_reconcile"));
            }
            offset += n;
        }
        fs::fsync(&fd)
            .and_then(|()| fs::fsync(&dir))
            .and_then(|()| fs::fsync(&self.fd))
            .map_err(|_| Error::new("receipt_write_unavailable_reconcile"))?;
        let stat = refusal(fs::fstat(&fd))?;
        if stat.st_uid != self.uid
            || stat.st_nlink != 1
            || stat.st_mode & 0o077 != 0
            || stat.st_size < 0
            || stat.st_size as usize != bytes.len()
        {
            return Err(Error::new("receipt_identity_unqualified"));
        }
        let current = refusal(fs::openat(&self.fd, "receipts", flags(), Mode::empty()))?;
        let before = refusal(fs::fstat(&dir))?;
        let after = refusal(fs::fstat(&current))?;
        if before.st_dev != after.st_dev || before.st_ino != after.st_ino {
            return Err(Error::new("receipt_identity_unqualified"));
        }
        self.revalidate()
    }
    pub fn read(&self, absolute: &Path, cap: usize) -> Result<CapturedFile> {
        if cap == 0 || !canonical_absolute(absolute) {
            return Err(Error::new("release_artifact_path_unqualified"));
        }
        self.revalidate()?;
        let relative = absolute
            .strip_prefix(&self.path)
            .map_err(|_| Error::new("release_artifact_outside_explicit_root"))?;
        let names = relative
            .components()
            .map(|c| match c {
                Component::Normal(v) => Ok(v),
                _ => Err(Error::new("release_artifact_path_unqualified")),
            })
            .collect::<Result<Vec<_>>>()?;
        let (leaf, parents) = names
            .split_last()
            .ok_or_else(|| Error::new("release_artifact_path_unqualified"))?;
        let mut ancestors = Vec::new();
        for name in parents {
            let fd = refusal(fs::openat(
                ancestors.last().unwrap_or(&self.fd),
                *name,
                flags(),
                Mode::empty(),
            ))?;
            directory(&refusal(fs::fstat(&fd))?, self.uid, false)?;
            ancestors.push(fd);
        }
        let fd = refusal(fs::openat(
            ancestors.last().unwrap_or(&self.fd),
            *leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ))?;
        let before = refusal(fs::fstat(&fd))?;
        if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
            || before.st_uid != self.uid
            || before.st_mode & 0o022 != 0
            || before.st_nlink != 1
        {
            return Err(Error::new("release_artifact_unqualified"));
        }
        if before.st_size < 0 || before.st_size as u64 > cap as u64 {
            return Err(Error::new("release_artifact_size_rejected"));
        }
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 65536];
        loop {
            let remaining = cap.saturating_add(1).saturating_sub(bytes.len());
            if remaining == 0 {
                break;
            }
            let size = remaining.min(buffer.len());
            let n = refusal(rustix::io::read(&fd, &mut buffer[..size]))?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
        }
        let after = refusal(fs::fstat(&fd))?;
        if bytes.len() > cap
            || before.st_size as u64 != bytes.len() as u64
            || !same_file(&before, &after)
        {
            return Err(Error::new("release_artifact_changed"));
        }
        // Re-read the same descriptor; a successful metadata check alone is not byte identity.
        let mut offset = 0;
        loop {
            let n = refusal(rustix::io::pread(&fd, &mut buffer, offset as u64))?;
            if n == 0 {
                break;
            }
            if bytes.get(offset..offset + n) != Some(&buffer[..n]) {
                return Err(Error::new("release_artifact_changed"));
            }
            offset += n;
        }
        if offset != bytes.len() || !same_file(&before, &refusal(fs::fstat(&fd))?) {
            return Err(Error::new("release_artifact_changed"));
        }
        self.revalidate()?;
        Ok(CapturedFile {
            sha256: digest(&bytes),
            bytes,
        })
    }
}
fn same_file(a: &fs::Stat, b: &fs::Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_uid == b.st_uid
        && a.st_mode == b.st_mode
        && a.st_nlink == b.st_nlink
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}
pub struct CapturedFile {
    pub bytes: Vec<u8>,
    pub sha256: String,
}
