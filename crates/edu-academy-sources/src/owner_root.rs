//! Separate strict admission. Descriptor-relative no-follow reads; never legacy QueryRefusal.
//! Owner publishes immutable bytes. Metadata checks cannot defeat malicious same-UID mutation
//! or establish a cross-file atomic snapshot.
use crate::data::*;
use rustix::{
    fd::OwnedFd,
    fs::{self, AtFlags, FileType, Mode, OFlags, Stat},
    process::geteuid,
};
use std::path::{Component, Path};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub device: u64,
    pub inode: u64,
    pub uid: u32,
    pub mode: u32,
    pub links: u64,
    pub size: i64,
    pub modified: (i64, i64),
    pub changed: (i64, i64),
}
impl Identity {
    #[expect(
        clippy::unnecessary_cast,
        reason = "Retain explicit identity widths at the platform Stat boundary"
    )]
    fn of(s: &Stat) -> Self {
        Self {
            device: s.st_dev as u64,
            inode: s.st_ino as u64,
            uid: s.st_uid,
            mode: s.st_mode,
            links: s.st_nlink as u64,
            size: s.st_size as i64,
            modified: (s.st_mtime as i64, s.st_mtime_nsec as i64),
            changed: (s.st_ctime as i64, s.st_ctime_nsec as i64),
        }
    }
    // Directory entry publication changes size, link count and timestamps without
    // replacing the directory. Pin its object and admission attributes; leaf
    // bytes still require full before/after identity plus digest verification.
    fn same_directory(&self, other: &Self) -> bool {
        self.device == other.device
            && self.inode == other.inode
            && self.uid == other.uid
            && self.mode == other.mode
    }
    pub fn value(&self) -> Js {
        obj(&[
            ("device", self.device.to_string().into()),
            ("inode", self.inode.to_string().into()),
            ("uid", Js::Num(f64::from(self.uid))),
            ("mode", Js::Num(f64::from(self.mode))),
            ("links", self.links.to_string().into()),
            ("size", self.size.to_string().into()),
            (
                "mtime",
                format!("{}:{}", self.modified.0, self.modified.1).into(),
            ),
            (
                "ctime",
                format!("{}:{}", self.changed.0, self.changed.1).into(),
            ),
        ])
    }
}
fn admission(code: &str) -> Error {
    Error::new("OwnerRootAdmission", code)
}
fn os<T>(v: rustix::io::Result<T>, code: &str) -> Result<T> {
    v.map_err(|_| admission(code))
}
fn directory(s: &Stat, uid: u32, private: bool) -> Result<()> {
    if FileType::from_raw_mode(s.st_mode) != FileType::Directory
        || if private {
            s.st_uid != uid || (s.st_mode & 0o077) != 0
        } else {
            (s.st_uid != 0 && s.st_uid != uid) || (s.st_mode & 0o022) != 0
        }
    {
        return Err(admission(if private {
            "owner_root_directory_private_required"
        } else {
            "owner_root_ancestor_untrusted"
        }));
    }
    Ok(())
}
struct Edge {
    parent: usize,
    name: String,
    child: usize,
    identity: Identity,
}
pub struct OwnerRoot {
    fds: Vec<OwnedFd>,
    edges: Vec<Edge>,
    root: usize,
    root_identity: Identity,
    uid: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checkpoint {
    LeafPinned,
    BytesCaptured,
}
pub struct Capture {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub identity: Identity,
    pub relative_path: String,
}
impl Capture {
    pub fn json(&self) -> Result<Js> {
        let s =
            std::str::from_utf8(&self.bytes).map_err(|_| admission("owner_root_utf8_invalid"))?;
        parse(s.strip_prefix('\u{feff}').unwrap_or(s))
            .map_err(|_| admission("owner_root_json_invalid"))
    }
}
impl OwnerRoot {
    pub fn open(root: &Path) -> Result<Self> {
        if !root.is_absolute() || root == Path::new("/") || root.to_str().is_none() {
            return Err(admission("owner_root_explicit_canonical_absolute_required"));
        }
        let canonical = format!(
            "/{}",
            root.components()
                .filter_map(|c| if let Component::Normal(s) = c {
                    s.to_str()
                } else {
                    None
                })
                .collect::<Vec<_>>()
                .join("/")
        );
        if root.to_str() != Some(&canonical) {
            return Err(admission("owner_root_explicit_canonical_absolute_required"));
        }
        let uid = geteuid().as_raw();
        let flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK;
        let slash = os(
            fs::open("/", flags, Mode::empty()),
            "owner_root_open_failed",
        )?;
        directory(
            &os(fs::fstat(&slash), "owner_root_stat_failed")?,
            uid,
            false,
        )?;
        let mut fds = vec![slash];
        let mut edges = vec![];
        let parts = root
            .components()
            .filter_map(|c| {
                if let Component::Normal(s) = c {
                    s.to_str()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for (i, name) in parts.iter().enumerate() {
            let parent = fds.len() - 1;
            let fd = os(
                fs::openat(&fds[parent], *name, flags, Mode::empty()),
                "owner_root_walk_refused",
            )?;
            let s = os(fs::fstat(&fd), "owner_root_stat_failed")?;
            directory(&s, uid, i + 1 == parts.len())?;
            let identity = Identity::of(&s);
            let child = fds.len();
            fds.push(fd);
            edges.push(Edge {
                parent,
                name: (*name).into(),
                child,
                identity,
            });
        }
        let index = fds.len() - 1;
        let identity = Identity::of(&os(fs::fstat(&fds[index]), "owner_root_stat_failed")?);
        let out = Self {
            fds,
            edges,
            root: index,
            root_identity: identity,
            uid,
        };
        out.revalidate()?;
        Ok(out)
    }
    fn revalidate(&self) -> Result<()> {
        for edge in &self.edges {
            let linked = Identity::of(&os(
                fs::statat(
                    &self.fds[edge.parent],
                    edge.name.as_str(),
                    AtFlags::SYMLINK_NOFOLLOW,
                ),
                "owner_root_ancestor_replaced",
            )?);
            let fd = Identity::of(&os(
                fs::fstat(&self.fds[edge.child]),
                "owner_root_stat_failed",
            )?);
            if !linked.same_directory(&edge.identity) || !fd.same_directory(&edge.identity) {
                return Err(admission("owner_root_ancestor_replaced"));
            }
        }
        Ok(())
    }
    pub fn read(&self, relative: &str, expected: &str, cap: usize) -> Result<Capture> {
        self.read_observed(relative, expected, cap, |_| {})
    }
    /// Checkpoint is an observation seam for deterministic replacement/race tests, never a
    /// privilege provider. The UID always comes from the current OS effective owner.
    pub fn read_observed(
        &self,
        relative: &str,
        expected: &str,
        cap: usize,
        mut checkpoint: impl FnMut(Checkpoint),
    ) -> Result<Capture> {
        if !sha_valid(&expected.into()) || cap == 0 || cap > 32 * 1024 * 1024 {
            return Err(admission("owner_root_arguments_invalid"));
        }
        let path = Path::new(relative);
        if path.is_absolute()
            || relative.is_empty()
            || path.to_str() != Some(relative)
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || path
                .components()
                .filter_map(|c| {
                    if let Component::Normal(s) = c {
                        s.to_str()
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("/")
                != relative
        {
            return Err(admission("owner_root_relative_path_invalid"));
        }
        self.revalidate()?;
        let mut dirs: Vec<OwnedFd> = vec![];
        let mut links: Vec<(String, Identity)> = vec![];
        let parts = relative.split('/').collect::<Vec<_>>();
        for name in &parts[..parts.len() - 1] {
            let parent = dirs.last().unwrap_or(&self.fds[self.root]);
            let fd = os(
                fs::openat(
                    parent,
                    *name,
                    OFlags::RDONLY
                        | OFlags::DIRECTORY
                        | OFlags::NOFOLLOW
                        | OFlags::CLOEXEC
                        | OFlags::NONBLOCK,
                    Mode::empty(),
                ),
                "owner_root_descendant_walk_refused",
            )?;
            let s = os(fs::fstat(&fd), "owner_root_stat_failed")?;
            directory(&s, self.uid, true)?;
            if s.st_dev as u64 != self.root_identity.device {
                return Err(admission("owner_root_device_crossing"));
            }
            links.push(((*name).into(), Identity::of(&s)));
            dirs.push(fd);
        }
        let parent = dirs.last().unwrap_or(&self.fds[self.root]);
        let name = parts[parts.len() - 1];
        let fd = os(
            fs::openat(
                parent,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ),
            "owner_root_leaf_open_refused",
        )?;
        let s = os(fs::fstat(&fd), "owner_root_stat_failed")?;
        if FileType::from_raw_mode(s.st_mode) != FileType::RegularFile
            || s.st_uid != self.uid
            || (s.st_mode & 0o077) != 0
            || s.st_nlink != 1
        {
            return Err(admission("owner_root_regular_private_single_link_required"));
        }
        if s.st_dev as u64 != self.root_identity.device {
            return Err(admission("owner_root_device_crossing"));
        }
        if s.st_size < 0 || s.st_size as u64 > cap as u64 {
            return Err(admission("owner_root_size_limit"));
        }
        let before = Identity::of(&s);
        checkpoint(Checkpoint::LeafPinned);
        let mut bytes = vec![];
        let mut buf = [0u8; 65536];
        loop {
            let remaining = (cap + 1).saturating_sub(bytes.len());
            if remaining == 0 {
                break;
            }
            let n = os(
                rustix::io::read(&fd, &mut buf[..remaining.min(65536)]),
                "owner_root_read_failed",
            )?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buf[..n]);
        }
        checkpoint(Checkpoint::BytesCaptured);
        let mut verify = vec![0u8; bytes.len() + 1];
        let mut offset = 0;
        loop {
            let n = os(
                rustix::io::pread(&fd, &mut verify[offset..], offset as u64),
                "owner_root_read_failed",
            )?;
            offset += n;
            if n == 0 || offset == verify.len() {
                break;
            }
        }
        verify.truncate(offset);
        if verify != bytes {
            return Err(admission("owner_root_leaf_changed_during_read"));
        }
        let after = Identity::of(&os(fs::fstat(&fd), "owner_root_stat_failed")?);
        let linked = Identity::of(&os(
            fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW),
            "owner_root_leaf_replaced",
        )?);
        if before != after || before != linked || bytes.len() as i64 != before.size {
            return Err(admission("owner_root_leaf_changed_during_read"));
        }
        if bytes.len() > cap {
            return Err(admission("owner_root_size_limit"));
        }
        for (i, (name, identity)) in links.iter().enumerate() {
            let p = if i == 0 {
                &self.fds[self.root]
            } else {
                &dirs[i - 1]
            };
            let a = Identity::of(&os(
                fs::statat(p, name.as_str(), AtFlags::SYMLINK_NOFOLLOW),
                "owner_root_descendant_replaced",
            )?);
            let b = Identity::of(&os(fs::fstat(&dirs[i]), "owner_root_stat_failed")?);
            if !a.same_directory(identity) || !b.same_directory(identity) {
                return Err(admission("owner_root_descendant_replaced"));
            }
        }
        self.revalidate()?;
        let sha256 = hash(&bytes);
        if sha256 != expected {
            return Err(admission("owner_root_hash_mismatch"));
        }
        Ok(Capture {
            bytes,
            sha256,
            identity: before,
            relative_path: relative.into(),
        })
    }
    /// Manifest and both leaves are captured and parsed from those same bytes. Their
    /// independent immutability is required; no cross-file atomicity is asserted.
    pub fn saved(&self, manifest_hash: &str) -> Result<(crate::saved::SavedContext, Vec<Capture>)> {
        let manifest = self.read(crate::saved::PIN_PATH, manifest_hash, 4 * 1024 * 1024)?;
        let pins = manifest.json()?;
        if !array(pins.get("files")) {
            return Err(admission("owner_root_manifest_invalid"));
        }
        let mut captures = vec![manifest];
        for path in [crate::saved::AUDIT_PATH, crate::saved::LESSON_PATH] {
            let rows = pins
                .get("files")
                .array()
                .iter()
                .filter(|p| p.get("path").is(path))
                .collect::<Vec<_>>();
            if rows.len() != 1 || !sha_valid(rows[0].get("sha256")) {
                return Err(admission("owner_root_manifest_invalid"));
            }
            captures.push(self.read(path, &string(rows[0].get("sha256")), 32 * 1024 * 1024)?);
        }
        let context =
            crate::saved::SavedContext::from_bytes(&captures[1].bytes, &captures[2].bytes)?;
        Ok((context, captures))
    }
}
/// Exact permissive legacy pinned-file boundary, separate from owner admission.
pub fn legacy_pinned(path: &Path, expected: &str) -> Result<Capture> {
    if !path.is_absolute() || !sha_valid(&expected.into()) {
        return Err(Error::error("label_snapshot_arguments_invalid"));
    }
    if std::fs::canonicalize(path).ok().as_deref() != Some(path) {
        return Err(Error::error("label_snapshot_symlink_or_noncanonical_path"));
    }
    let fd = fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| Error::error("label_snapshot_file_invalid"))?;
    let before = Identity::of(&os(fs::fstat(&fd), "label_snapshot_file_invalid")?);
    if FileType::from_raw_mode(before.mode) != FileType::RegularFile
        || before.size < 0
        || before.size > 4 * 1024 * 1024
    {
        return Err(Error::error("label_snapshot_file_invalid"));
    }
    let mut b = vec![0u8; before.size as usize + 1];
    let n = rustix::io::read(&fd, &mut b[..])
        .map_err(|_| Error::error("label_snapshot_file_invalid"))?;
    let after = Identity::of(&os(fs::fstat(&fd), "label_snapshot_file_invalid")?);
    if n as i64 != before.size
        || before.size != after.size
        || before.modified != after.modified
        || before.changed != after.changed
    {
        return Err(Error::error("label_snapshot_changed_during_read"));
    }
    b.truncate(n);
    let sha256 = hash(&b);
    if sha256 != expected {
        return Err(Error::error("label_snapshot_hash_mismatch"));
    }
    Ok(Capture {
        bytes: b,
        sha256,
        identity: before,
        relative_path: path.to_string_lossy().into(),
    })
}

/// A persistent uncertainty marker contains only hashes, never source/credential bytes.
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PendingEffect {
    pub scope: String,
    pub operation: String,
    pub readback: String,
    pub expected: String,
}
/// Kernel-owned exclusion for the full authenticated account lifetime. The inode
/// is never unlinked on release: unlinking a locked file permits a second lock.
/// All sessions of an installed owner MUST use its same explicit canonical root.
/// Same-UID malicious file replacement is outside the owner trust boundary.
pub struct AccountLease {
    root: std::rc::Rc<OwnerRoot>,
    fd: OwnedFd,
    name: String,
    identity: Identity,
    shared: SharedAccountLock,
}
impl Drop for AccountLease {
    fn drop(&mut self) {
        // CLOEXEC does not close an unrelated child's inherited descriptor until
        // exec. Explicitly release our shared open-file-description lock rather
        // than making owner Drop wait for that child's scheduling. Never unlink.
        while let Err(rustix::io::Errno::INTR) = fs::flock(&self.fd, fs::FlockOperation::Unlock) {}
    }
}
/// Read-only descriptor for the launcher-owned inode. Rust never publishes or
/// modifies this file. Same-UID malicious path replacement is outside admission.
struct SharedAccountLock {
    fd: OwnedFd,
    path: std::path::PathBuf,
    identity: Identity,
}
impl Drop for SharedAccountLock {
    fn drop(&mut self) {
        while let Err(rustix::io::Errno::INTR) = fs::flock(&self.fd, fs::FlockOperation::Unlock) {}
    }
}
impl SharedAccountLock {
    fn validate(&self) -> Result<()> {
        let linked = os(
            fs::statat(fs::CWD, &self.path, AtFlags::SYMLINK_NOFOLLOW),
            "owner_shared_account_replaced",
        )?;
        if !Identity::of(&linked).same_directory(&self.identity)
            || FileType::from_raw_mode(linked.st_mode) != FileType::RegularFile
            || std::fs::canonicalize(&self.path).ok().as_ref() != Some(&self.path)
        {
            return Err(admission("owner_shared_account_replaced"));
        }
        Ok(())
    }
}
impl OwnerRoot {
    fn shared_account_lock(&self) -> Result<SharedAccountLock> {
        // Use the same private-file capture/revalidation as pinned owner inputs.
        // This document is explicit owner configuration, not credential discovery.
        let fd = os(
            fs::openat(
                &self.fds[self.root],
                "account-lock.json",
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ),
            "owner_shared_account_configuration_required",
        )?;
        let stat = os(
            fs::fstat(&fd),
            "owner_shared_account_configuration_required",
        )?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(admission("owner_shared_account_configuration_required"));
        }
        let mut bytes = [0u8; 16385];
        let n = os(
            rustix::io::pread(&fd, &mut bytes, 0),
            "owner_shared_account_configuration_required",
        )?;
        let capture = self.read("account-lock.json", &hash(&bytes[..n]), 16384)?;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Configuration {
            schema_version: String,
            path: std::path::PathBuf,
        }
        let config: Configuration = serde_json::from_slice(&capture.bytes)
            .map_err(|_| admission("owner_shared_account_configuration_required"))?;
        if config.schema_version != "academy.account-lock/1"
            || !config.path.is_absolute()
            || std::fs::canonicalize(&config.path).ok().as_ref() != Some(&config.path)
        {
            return Err(admission("owner_shared_account_path_refused"));
        }
        let fd = os(
            fs::open(
                &config.path,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ),
            "owner_shared_account_open_refused",
        )?;
        let stat = os(fs::fstat(&fd), "owner_shared_account_stat_failed")?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(admission("owner_shared_account_regular_file_required"));
        }
        fs::flock(&fd, fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| admission("owner_account_busy"))?;
        let shared = SharedAccountLock {
            fd,
            path: config.path,
            identity: Identity::of(&stat),
        };
        shared.validate()?;
        Ok(shared)
    }
    /// Lock order everywhere: launcher-owned shared file FIRST, owner pending
    /// marker SECOND, both nonblocking. Busy shared admission creates no marker.
    pub fn account(self: &std::rc::Rc<Self>, account_sha256: &str) -> Result<AccountLease> {
        if !sha_valid(&account_sha256.into()) {
            return Err(admission("owner_account_identity_required"));
        }
        self.revalidate()?;
        let shared = self.shared_account_lock()?;
        let name = format!("account-{account_sha256}.lock");
        let fd = os(
            fs::openat(
                &self.fds[self.root],
                name.as_str(),
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ),
            "owner_account_open_refused",
        )?;
        let stat = os(fs::fstat(&fd), "owner_account_stat_failed")?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_uid != self.uid
            || stat.st_mode & 0o777 != 0o600
            || stat.st_nlink != 1
            || stat.st_dev as u64 != self.root_identity.device
            || stat.st_size > 4096
        {
            return Err(admission("owner_account_private_file_required"));
        }
        fs::flock(&fd, fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| admission("owner_account_busy"))?;
        let lease = AccountLease {
            root: std::rc::Rc::clone(self),
            fd,
            name,
            identity: Identity::of(&stat),
            shared,
        };
        lease.validate()?;
        os(fs::fsync(&self.fds[self.root]), "owner_account_sync_failed")?;
        // Invalid/truncated markers fail closed, even before authentication.
        lease.pending()?;
        Ok(lease)
    }
}
impl AccountLease {
    pub(crate) fn validate(&self) -> Result<()> {
        self.shared.validate()?;
        self.root.revalidate()?;
        let linked = Identity::of(&os(
            fs::statat(
                &self.root.fds[self.root.root],
                self.name.as_str(),
                AtFlags::SYMLINK_NOFOLLOW,
            ),
            "owner_account_replaced",
        )?);
        let current = Identity::of(&os(fs::fstat(&self.fd), "owner_account_stat_failed")?);
        if !linked.same_directory(&self.identity)
            || !current.same_directory(&self.identity)
            || linked.links != 1
            || current.links != 1
            || current.size > 4096
        {
            return Err(admission("owner_account_replaced"));
        }
        Ok(())
    }
    pub(crate) fn pending(&self) -> Result<Option<PendingEffect>> {
        self.validate()?;
        let mut bytes = [0u8; 4097];
        let n = os(
            rustix::io::pread(&self.fd, &mut bytes, 0),
            "owner_account_read_failed",
        )?;
        if n == 0 || &bytes[..n] == b"0\n" {
            return Ok(None);
        }
        let pending: PendingEffect = serde_json::from_slice(&bytes[..n])
            .map_err(|_| admission("owner_account_reconciliation_required"))?;
        if [
            &pending.scope,
            &pending.operation,
            &pending.readback,
            &pending.expected,
        ]
        .iter()
        .any(|v| !sha_valid(&v.as_str().into()))
        {
            return Err(admission("owner_account_reconciliation_required"));
        }
        Ok(Some(pending))
    }
    fn store(&self, bytes: &[u8]) -> Result<()> {
        self.validate()?;
        let mut at = 0;
        while at < bytes.len() {
            let n = os(
                rustix::io::pwrite(&self.fd, &bytes[at..], at as u64),
                "owner_account_write_failed",
            )?;
            if n == 0 {
                return Err(admission("owner_account_write_failed"));
            }
            at += n;
        }
        os(
            fs::ftruncate(&self.fd, bytes.len() as u64),
            "owner_account_write_failed",
        )?;
        os(fs::fsync(&self.fd), "owner_account_sync_failed")?;
        self.validate()
    }
    pub fn reconciliation_required(&self) -> Result<bool> {
        Ok(self.pending()?.is_some())
    }
    pub(crate) fn begin(&self, pending: &PendingEffect) -> Result<()> {
        if self.pending()?.is_some() {
            return Err(admission("owner_account_reconciliation_required"));
        }
        let bytes =
            serde_json::to_vec(pending).map_err(|_| admission("owner_account_write_failed"))?;
        self.store(&bytes)
    }
    /// Only an actual, matching native readback may call this boundary.
    pub(crate) fn reconciled(&self) -> Result<()> {
        self.store(b"0\n")
    }
}

#[cfg(test)]
#[test]
fn account_drop_releases_flock_with_retained_file_description() {
    use std::os::unix::fs::DirBuilderExt;
    let path = std::path::PathBuf::from(std::env::var("S09_NATIVE_FIXTURE_ROOT").unwrap())
        .join(format!("lease-alias-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .unwrap();
    let shared = path.join("shared.lock");
    std::fs::write(&shared, b"").unwrap();
    let config = path.join("account-lock.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"academy.account-lock/1", "path":shared
        }))
        .unwrap(),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(config, std::fs::Permissions::from_mode(0o600)).unwrap();
    let root = std::rc::Rc::new(OwnerRoot::open(&path).unwrap());
    let key = hash(b"synthetic-retained-description");
    let lease = root.account(&key).unwrap();
    // dup retains the same open file description, as an unrelated fork does
    // before CLOEXEC takes effect. No unsafe code, child or credential is needed.
    let inherited = rustix::io::dup(&lease.fd).unwrap();
    let shared_inherited = rustix::io::dup(&lease.shared.fd).unwrap();
    drop(lease);
    let reacquired = root.account(&key);
    let released = reacquired.is_ok();
    drop(inherited);
    drop(shared_inherited);
    if released {
        assert!(root.account(&key).is_err());
    }
    drop(reacquired);
    drop(root);
    std::fs::remove_dir_all(path).unwrap();
    assert!(
        released,
        "owner Drop must release its flock before an inherited descriptor closes"
    );
}
