use super::{
    Runtime, canonical, codec, io_fault,
    ledger::{AttemptLedger, AttemptTarget},
    state::{self, FieldState},
};
use crate::js::{Fault, Js, Result, obj, sha};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
const MAX_ATTEMPT_BODY_BYTES: usize = 65_536;

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
/// Name the engine sibling independently of separators and `.` components.
pub fn ledger_sibling(root: &Path) -> Option<PathBuf> {
    let root = root.components().collect::<PathBuf>();
    let mut name = root.file_name()?.to_os_string();
    name.push(".ledger");
    Some(root.with_file_name(name))
}
fn has_engine_execution(root: &Path) -> Result<bool> {
    let entries = match fs::read_dir(root.join("execution")) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(io_fault(e)),
    };
    let mut first = None;
    for entry in entries {
        let entry = entry.map_err(io_fault)?;
        if entry.file_name().as_encoded_bytes().starts_with(b".") {
            continue;
        }
        first = Some(entry);
        break;
    }
    let Some(entry) = first else {
        return Ok(false);
    };
    let name = entry.file_name();
    let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
        return Ok(false);
    };
    let Some((key, digest)) = stem.split_once('.') else {
        return Ok(false);
    };
    Ok([key, digest]
        .iter()
        .all(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())))
}
fn normal_under(root: &Path, owner_root: &Path) -> Result<()> {
    if !root.is_absolute()
        || !owner_root.is_absolute()
        || !root.starts_with(owner_root)
        || !root
            .strip_prefix(owner_root)
            .map_err(|_| Fault::error("dailyrecord_workflow_store_owner_root_required"))?
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return Err(Fault::error(
            "dailyrecord_workflow_store_owner_root_required",
        ));
    }
    Ok(())
}
pub struct Store {
    root: PathBuf,
    attempts: Option<Box<dyn AttemptLedger>>,
}
impl Store {
    pub const SCOPE: &'static str = "single_host_owner_filesystem";
    /// Empty directories created when a core store is first opened.
    pub const DIRECTORIES: &'static [&'static str] = &[
        "observation",
        "review",
        "request",
        "execution",
        "outcome",
        "reconciliation",
        "ledger",
        "audit",
        "locks",
    ];
    pub fn open(root: &Path, owner_root: &Path) -> Result<Self> {
        normal_under(root, owner_root)?;
        // A normal last component under the owner root always has a ledger sibling.
        if ledger_sibling(root).is_some_and(|path| path.exists()) || has_engine_execution(root)? {
            return Err(Fault::error("workflow_ledger_legacy_root"));
        }
        Self::initialize(root, owner_root)
    }
    fn initialize(root: &Path, owner_root: &Path) -> Result<Self> {
        normal_under(root, owner_root)?;
        if fs::canonicalize(owner_root).map_err(io_fault)? != owner_root {
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
        mkdir(root)?;
        for k in Self::DIRECTORIES {
            mkdir(&root.join(k))?;
        }
        Ok(Self {
            root: root.into(),
            attempts: None,
        })
    }
    /// The review named by an execution must be published before the execution.
    /// The service route guarantees this order; importers must also preserve it.
    pub fn open_with_ledger(
        root: &Path,
        owner_root: &Path,
        ledger: Box<dyn AttemptLedger>,
    ) -> Result<Self> {
        let mut store = Self::initialize(root, owner_root)?;
        if fs::read_dir(root.join("ledger"))
            .map_err(io_fault)?
            .next()
            .transpose()
            .map_err(io_fault)?
            .is_some()
        {
            return Err(Fault::error("workflow_ledger_legacy_root"));
        }
        store.attempts = Some(ledger);
        Ok(store)
    }
    pub fn ledgered(&self) -> bool {
        self.attempts.is_some()
    }
    /// Count bounded attempt metadata without reading any record body.
    pub fn attempt_count(&self, group: &str, limit: usize) -> Result<usize> {
        self.attempts
            .as_deref()
            .ok_or_else(|| Fault::error("workflow_ledger_unavailable"))?
            .attempts(group, limit)
            .map(|attempts| attempts.len())
    }
    pub(crate) fn attempt_ledger(&self) -> Option<&dyn AttemptLedger> {
        self.attempts.as_deref()
    }
    pub fn attempt_target(&self, execution: &Js) -> Result<AttemptTarget> {
        let missing = || Fault::error("workflow_execution_review_target_unavailable");
        let field = required_text(execution, "field")?;
        let digest = required_text(execution, "reviewDigest")?;
        let review = self.get(Kind::Review, &digest)?;
        let row = review
            .get("patch")
            .array()
            .iter()
            .find(|p| p.get("field").is(&field))
            .ok_or_else(missing)?;
        if !execution.has("effectRecordKey") || !execution.has("requestRef") {
            return Err(missing());
        }
        target_from_parts(
            execution.get("scope"),
            &field,
            execution.get("effectRecordKey"),
            execution.get("requestRef"),
            row.get("target"),
        )
    }
    fn body_path(&self, kind: Kind, id: &str, digest: &str) -> PathBuf {
        self.path(kind, id).with_file_name(format!(
            "{}.{}.json",
            sha(&(kind.name().to_owned() + "\n" + id)),
            digest
        ))
    }
    fn checked_body(&self, kind: Kind, id: &str, digest: &str) -> Result<Js> {
        let fail = || Fault::error("workflow_attempt_body_unavailable");
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(fail());
        }
        // Linux O_NOFOLLOW | O_NONBLOCK: refuse links and avoid blocking on a FIFO.
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(0o400000 | 0o4000)
            .open(self.body_path(kind, id, digest))
            .map_err(|_| fail())?;
        let meta = file.metadata().map_err(|_| fail())?;
        let status = fs::read_to_string("/proc/self/status").map_err(|_| fail())?;
        let uid = status
            .lines()
            .find(|s| s.starts_with("Uid:"))
            .and_then(|s| s.split_whitespace().nth(2))
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(fail)?;
        if !meta.is_file()
            || meta.uid() != uid
            || meta.mode() & 0o7777 != 0o600
            || meta.nlink() != 1
            || meta.len() > MAX_ATTEMPT_BODY_BYTES as u64
        {
            return Err(fail());
        }
        let mut bytes = vec![];
        file.take((MAX_ATTEMPT_BODY_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > MAX_ATTEMPT_BODY_BYTES || format!("{:x}", Sha256::digest(&bytes)) != digest
        {
            return Err(fail());
        }
        let wrapper = codec::parse(&bytes).map_err(|_| fail())?;
        if !wrapper.get("kind").is(kind.name()) || !wrapper.get("id").is(id) {
            return Err(fail());
        }
        Ok(wrapper.get("record").clone())
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
        if let Some(ledger) = self.attempts.as_deref().filter(|_| attempt_kind(kind)) {
            let Some(attempt) = ledger.attempt(id)? else {
                return Ok(Js::Null);
            };
            let digest = match kind {
                Kind::Execution => Some(attempt.execution),
                Kind::Outcome => attempt.outcome,
                Kind::Reconciliation => attempt.reconciliation,
                _ => unreachable!(),
            };
            return digest.map_or(Ok(Js::Null), |digest| self.checked_body(kind, id, &digest));
        }
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
        let created = if let Some(ledger) = self.attempts.as_deref().filter(|_| attempt_kind(kind))
        {
            let bytes = codec::bytes(&v)?;
            if bytes.len() > MAX_ATTEMPT_BODY_BYTES {
                return Err(Fault::error("workflow_attempt_body_too_large"));
            }
            let target = if matches!(kind, Kind::Execution) {
                if ledger
                    .execution_key_limit()
                    .is_some_and(|limit| id.len() > limit)
                {
                    return Err(Fault::error("workflow_execution_key_too_large"));
                }
                Some(self.attempt_target(record)?)
            } else {
                None
            };
            let state = match kind {
                Kind::Outcome => state::classify(
                    &self.get(Kind::Execution, id)?,
                    record,
                    &self.get(Kind::Reconciliation, id)?,
                    &|d| self.get(Kind::Review, d),
                )?,
                Kind::Reconciliation => {
                    let state = state::classify(
                        &self.get(Kind::Execution, id)?,
                        &self.get(Kind::Outcome, id)?,
                        record,
                        &|d| self.get(Kind::Review, d),
                    )?;
                    if !matches!(
                        state,
                        FieldState::ReconciledPersisted
                            | FieldState::ReconciledNotPersisted
                            | FieldState::ReconciledConflict
                    ) {
                        return Err(Fault::error("workflow_reconciliation_not_closing"));
                    }
                    state
                }
                _ => FieldState::DispatchUnknown,
            };
            let digest = format!("{:x}", Sha256::digest(&bytes));
            publish_body(&self.body_path(kind, id, &digest), &bytes, clock)?;
            match kind {
                Kind::Execution => ledger.begin(
                    id,
                    target.as_ref().expect("execution target"),
                    &digest,
                    clock,
                )?,
                Kind::Outcome => ledger.record_outcome(id, &digest, state, clock)?,
                Kind::Reconciliation => ledger.record_reconciliation(id, &digest, state, clock)?,
                _ => unreachable!(),
            }
        } else {
            publish(&self.path(kind, id), &codec::bytes(&v)?, clock)?
        };
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
        if self.ledgered() {
            return Ok(());
        }
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
        if self.ledgered() {
            return Err(Fault::error("workflow_ledger_unavailable"));
        }
        let mut keys = vec![];
        for p in names(&self.root.join("ledger").join(sha(group)))? {
            let v = codec::parse(&fs::read(p).map_err(io_fault)?)?;
            if v.get("group").is(group) {
                let value = v.get("key");
                if !value.string() {
                    return Err(Fault::error("workflow_store_ledger_key_invalid"));
                }
                let key = String::from_utf16(value.units())
                    .map_err(|_| Fault::error("workflow_store_ledger_key_invalid"))?;
                keys.push(key);
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
fn attempt_kind(kind: Kind) -> bool {
    matches!(kind, Kind::Execution | Kind::Outcome | Kind::Reconciliation)
}
fn required_text(record: &Js, key: &str) -> Result<String> {
    if !record.get(key).string() {
        return Err(Fault::error("workflow_execution_review_target_unavailable"));
    }
    String::from_utf16(record.get(key).units())
        .map_err(|_| Fault::error("workflow_execution_review_target_unavailable"))
}
pub(crate) fn target_from_parts(
    scope: &Js,
    field: &str,
    record: &Js,
    request: &Js,
    target: &Js,
) -> Result<AttemptTarget> {
    Ok(AttemptTarget {
        group: state::lock_key(scope)?,
        source_namespace: required_text(scope, "sourceNamespace")?,
        teacher_id: required_text(scope, "teacherId")?,
        scope_student_id: required_text(scope, "studentId")?,
        request_lesson_date: required_text(scope, "lessonDate")?,
        field: field.into(),
        record_key: canonical(record)
            .map_err(|_| Fault::error("workflow_execution_review_target_unavailable"))?,
        request_ref: canonical(request)
            .map_err(|_| Fault::error("workflow_execution_review_target_unavailable"))?,
        occurrence: [
            required_text(target, "sourceNamespace")?,
            required_text(target, "studentId")?,
            required_text(target, "lessonDate")?,
            required_text(target, "courseId")?,
        ],
    })
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
/// Content-addressed body: write, file sync, rename, directory sync.
/// Replacing the same digest name replaces equal bytes with a single-link file.
fn publish_body(target: &Path, bytes: &[u8], clock: &dyn Runtime) -> Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| Fault::error("workflow_store_path_invalid"))?;
    let temp = dir.join(".tmp-".to_owned() + &clock.uuid()?);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(io_fault)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(io_fault)?;
    drop(file);
    fs::rename(&temp, target).map_err(io_fault)?;
    sync_dir(dir)
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
