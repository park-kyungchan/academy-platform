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
            if linked != edge.identity || fd != edge.identity {
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
            if &a != identity || &b != identity {
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
