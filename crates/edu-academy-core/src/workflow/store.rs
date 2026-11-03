use super::{Runtime, codec, io_fault};
use crate::js::{Fault, Js, Result, obj, sha};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Observation,
    Review,
    Request,
    Execution,
    Outcome,
    Reconciliation,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Review => "review",
            Self::Request => "request",
            Self::Execution => "execution",
            Self::Outcome => "outcome",
            Self::Reconciliation => "reconciliation",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "observation" => Ok(Self::Observation),
            "review" => Ok(Self::Review),
            "request" => Ok(Self::Request),
            "execution" => Ok(Self::Execution),
            "outcome" => Ok(Self::Outcome),
            "reconciliation" => Ok(Self::Reconciliation),
            _ => Err(Fault::error("workflow_record_kind_invalid")),
        }
    }
}
pub fn mkdir(path: &Path) -> Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(io_fault)
}
pub fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(io_fault)
}
pub struct Store {
    root: PathBuf,
}
impl Store {
    pub const SCOPE: &'static str = "single_host_owner_filesystem";
    pub fn open(root: &Path, owner_root: &Path) -> Result<Self> {
        if !root.is_absolute()
            || !owner_root.is_absolute()
            || !root.starts_with(owner_root)
            || fs::canonicalize(owner_root).map_err(io_fault)? != owner_root
        {
            return Err(Fault::error(
                "dailyrecord_workflow_store_owner_root_required",
            ));
        }
        let mut p = owner_root.to_path_buf();
        for part in root
            .strip_prefix(owner_root)
            .map_err(|_| Fault::error("dailyrecord_workflow_store_owner_root_required"))?
            .components()
        {
            if !matches!(part, std::path::Component::Normal(_)) {
                return Err(Fault::error(
                    "dailyrecord_workflow_store_owner_root_required",
                ));
            }
            p.push(part);
            match fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(Fault::error("dailyrecord_workflow_store_symlink_refused"));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(io_fault(e)),
                _ => {}
            }
        }
        for k in [
            "",
            "observation",
            "review",
            "request",
            "execution",
            "outcome",
            "reconciliation",
            "ledger",
            "audit",
            "locks",
        ] {
            mkdir(&root.join(k))?;
        }
        Ok(Self { root: root.into() })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn path(&self, kind: Kind, id: &str) -> PathBuf {
        self.root
            .join(kind.name())
            .join(sha(&(kind.name().to_owned() + "\n" + id)) + ".json")
    }
    pub fn get(&self, kind: Kind, id: &str) -> Result<Js> {
        let bytes = match fs::read(self.path(kind, id)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Js::Null),
            Err(e) => return Err(io_fault(e)),
        };
        let parsed = codec::parse(&bytes)?;
        if !parsed.get("kind").is(kind.name()) || !parsed.get("id").is(id) {
            return Err(Fault::error("workflow_store_record_identity_conflict"));
        }
        Ok(parsed.get("record").clone())
    }
    pub fn publish(
        &self,
        kind: Kind,
        id: &str,
        record: &Js,
        clock: &dyn Runtime,
    ) -> Result<(bool, Js)> {
        let v = obj(&[
            ("kind", kind.name().into()),
            ("id", id.into()),
            ("record", record.clone()),
        ]);
        let created = publish(&self.path(kind, id), &codec::bytes(&v)?, clock)?;
        Ok((
            created,
            if created {
                record.clone()
            } else {
                self.get(kind, id)?
            },
        ))
    }
    pub fn add_ledger(&self, group: &str, key: &str, clock: &dyn Runtime) -> Result<()> {
        let dir = self.root.join("ledger").join(sha(group));
        mkdir(&dir)?;
        publish(
            &dir.join(sha(key) + ".json"),
            &codec::bytes(&obj(&[("group", group.into()), ("key", key.into())]))?,
            clock,
        )?;
        Ok(())
    }
    pub fn ledger(&self, group: &str) -> Result<Vec<String>> {
        let mut keys = vec![];
        for p in names(&self.root.join("ledger").join(sha(group)))? {
            let v = codec::parse(&fs::read(p).map_err(io_fault)?)?;
            if v.get("group").is(group) {
                keys.push(v.get("key").code_units());
            }
        }
        keys.sort_by_key(|s| s.encode_utf16().collect::<Vec<_>>());
        Ok(keys)
    }
    pub fn append_audit(&self, key: &str, event: &Js, clock: &dyn Runtime) -> Result<()> {
        let dir = self.root.join("audit").join(sha(key));
        mkdir(&dir)?;
        let v = super::merge(
            obj(&[
                ("requestKey", key.into()),
                ("at", clock.wall_now().into()),
                ("pid", Js::Num(f64::from(clock.pid()))),
            ]),
            event,
        );
        publish(
            &dir.join(format!("{:015}-{}.json", clock.epoch_ms(), clock.uuid()?)),
            &codec::bytes(&v)?,
            clock,
        )?;
        Ok(())
    }
    pub fn audit(&self, key: &str, limit: usize) -> Result<Js> {
        let files = names(&self.root.join("audit").join(sha(key)))?;
        let mut out = vec![];
        for p in files.iter().skip(files.len().saturating_sub(limit)) {
            let v = codec::parse(&fs::read(p).map_err(io_fault)?)?;
            if v.get("requestKey").is(key) {
                out.push(v);
            }
        }
        Ok(Js::Arr(out))
    }
    pub fn lock(
        &self,
        key: &str,
        options: LockOptions,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Guard> {
        lock_path(
            &self.root.join("locks").join(sha(key) + ".flock"),
            options,
            cancel,
        )
    }
}
pub fn names(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(io_fault(e)),
    };
    let mut v = vec![];
    for e in entries {
        let e = e.map_err(io_fault)?;
        let n = e.file_name();
        let n = n.to_string_lossy();
        if !n.starts_with('.') && n.ends_with(".json") {
            v.push(e.path());
        }
    }
    v.sort();
    Ok(v)
}
/// Temp0600 -> write -> fsync -> hard link -> unlink -> directory fsync.
fn publish(target: &Path, bytes: &[u8], clock: &dyn Runtime) -> Result<bool> {
    let dir = target
        .parent()
        .ok_or_else(|| Fault::error("workflow_store_path_invalid"))?;
    let temp = dir.join(".tmp-".to_owned() + &clock.uuid()?);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(io_fault)?;
    f.write_all(bytes)
        .and_then(|()| f.sync_all())
        .map_err(io_fault)?;
    drop(f);
    let linked = fs::hard_link(&temp, target);
    fs::remove_file(&temp).map_err(io_fault)?;
    match linked {
        Ok(()) => {
            sync_dir(dir)?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(io_fault(e)),
    }
}
#[derive(Clone, Copy)]
pub struct LockOptions {
    pub wait: Duration,
    pub poll: Duration,
}
impl Default for LockOptions {
    fn default() -> Self {
        Self {
            wait: Duration::from_secs(30),
            poll: Duration::from_millis(20),
        }
    }
}
/// The open file description owns the kernel flock; drop never removes its pathname.
pub struct Guard {
    file: File,
}
impl Guard {
    pub fn file(&self) -> &File {
        &self.file
    }
}
pub fn lock_path(path: &Path, options: LockOptions, cancel: &dyn Fn() -> bool) -> Result<Guard> {
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
        .map_err(io_fault)?;
    let start = Instant::now();
    loop {
        if cancel() {
            return Err(Fault::error("workflow_lock_cancelled"));
        }
        match file.try_lock() {
            Ok(()) => return Ok(Guard { file }),
            Err(std::fs::TryLockError::WouldBlock) => {}
            Err(std::fs::TryLockError::Error(e)) => return Err(io_fault(e)),
        }
        if start.elapsed() > options.wait {
            return Err(Fault::error("shared_writer_lock_busy"));
        }
        std::thread::sleep(options.poll);
    }
}
