//! Guarded synthetic filesystem boundary, without draft lifecycle or save authority.
use crate::record_data::{
    AnswerField, AnswerValue, ContentBinding, Draft, DraftState, OwnerScope, RECORD_FIELD_COUNT,
    RecordField, RecordFields, RecordIdentity, RecordObservation, RecordValue, content_hash,
};
use edu_academy_core::{
    js::{Fault, Js, Result, sha},
    workflow::{
        codec, state,
        store::{Guard, LockOptions, lock_path},
    },
};
use rustix::fs::{self as rfs, Dir, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    collections::BTreeSet,
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
pub const MAX_EXECUTION_RECORDS: usize = 4_096;
pub const MAX_DRAFT_CANDIDATES: usize = 256;
pub const MAX_OPEN_DATES: usize = 32;
pub const MAX_RETAINED_GENERATIONS: usize = 1_024;
pub const DRAFT_RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
#[derive(Debug, Default)]
pub struct DraftCleanup {
    pub examined: usize,
    pub removed: usize,
    pub protected: usize,
    pub next_eligible_ms: Option<i64>,
}
/// Relative payload path, not an index: owner/student bucket, identity/generation file.
pub fn draft_path(identity: &RecordIdentity, generation: &str) -> Result<PathBuf> {
    Ok(PathBuf::from("record-drafts")
        .join(content_hash(&(&identity.owner, &identity.student_key))?)
        .join(format!("{}-{}.json", identity.key()?, sha(generation))))
}
#[derive(Clone, Copy, Debug)]
pub enum ExecutionAdmission {
    ReadOnly,
    Save { selected_fields: usize },
}
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
fn corrupt() -> Fault {
    Fault::error("record_store_corrupt")
}
fn execution_limit() -> Fault {
    Fault::error("record_execution_limit")
}
const HASH_HEX_BYTES: usize = 64;
fn digest(s: &str) -> bool {
    s.len() == HASH_HEX_BYTES
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn timestamp(s: &str) -> Result<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|t| t.timestamp_millis())
        .map_err(|_| corrupt())
}
/// Validate durable intent, not current source freshness, policy or approval.
/// Historical attempt bindings retain their own hashes; core owns their evidence.
pub fn validate_draft(bytes: &[u8], identity: &RecordIdentity) -> Result<Draft> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(corrupt());
    }
    let d: Draft = serde_json::from_slice(bytes).map_err(|_| corrupt())?;
    let date = chrono::NaiveDate::parse_from_str(&d.identity.lesson_date, "%Y-%m-%d")
        .map_err(|_| corrupt())?;
    if d.identity != *identity
        || d.before.identity != *identity
        || d.before.requested_date != identity.lesson_date
        || date.format("%Y-%m-%d").to_string() != identity.lesson_date
        || identity.owner.owner.is_empty()
        || identity.owner.account.is_empty()
        || identity.student_key.is_empty()
        || d.generation.is_empty()
        || d.preview_hash != content_hash(&d.rendered_preview)?
    {
        return Err(corrupt());
    }
    let created = timestamp(&d.created)?;
    let used = timestamp(&d.last_used)?;
    timestamp(&d.before.provenance.read_at)?;
    if used < created || (d.state == DraftState::Open) != d.closed_at.is_none() {
        return Err(corrupt());
    }
    if let Some(at) = &d.closed_at
        && timestamp(at)? < created
    {
        return Err(corrupt());
    }
    let binding_valid = |b: &ContentBinding| {
        b.identity == *identity
            && b.generation == d.generation
            && digest(&b.patch_hash)
            && digest(&b.source_hash)
    };
    let fields = match &d.before.observation {
        RecordObservation::Observed { occurrence, fields } if !occurrence.is_empty() => {
            Some(fields.clone())
        }
        RecordObservation::NotObserved => Some(RecordFields::from_fn(|_| RecordValue::Unknown)),
        RecordObservation::Failed { reason_code } if !reason_code.is_empty() => None,
        _ => return Err(corrupt()),
    };
    if let Some(b) = &d.binding {
        let fields = fields.as_ref().ok_or_else(corrupt)?;
        let expected =
            ContentBinding::new(identity.clone(), d.generation.clone(), &d.patch, fields)?;
        if *b != expected {
            return Err(corrupt());
        }
    }
    for field in RecordField::ALL {
        if let Some(patch) = &d.patch[field]
            && (!d.editable[field] || patch.effect_target.is_empty())
        {
            return Err(corrupt());
        }
    }
    for q in &d.pending_questions {
        if q.revision != d.revision || q.reason_code.is_empty() || q.text.is_empty() {
            return Err(corrupt());
        }
        for choice in &q.choices {
            let correct = match q.field {
                AnswerField::Progress | AnswerField::Homework => {
                    matches!(choice.answer, AnswerValue::Lines(_))
                }
                AnswerField::Replace | AnswerField::Restart => {
                    matches!(choice.answer, AnswerValue::Boolean(_))
                }
                _ => matches!(choice.answer, AnswerValue::Text(_)),
            };
            if choice.label.is_empty() || !correct {
                return Err(corrupt());
            }
        }
    }
    if let Some(a) = &d.pending_attempt {
        let expected = "p8s3:".to_owned()
            + &content_hash(&(&a.binding, a.ordinal.get(), &a.instruction_hash))?;
        if !binding_valid(&a.binding)
            || !digest(&a.instruction_hash)
            || a.ordinal > d.next_attempt_ordinal
            || a.request_ref != expected
        {
            return Err(corrupt());
        }
    }
    if d.failed_attempts
        .as_ref()
        .is_some_and(|f| !binding_valid(&f.binding))
    {
        return Err(corrupt());
    }
    Ok(d)
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
fn core_record(parent: &File, kind: &str, key: &str) -> Result<Option<Js>> {
    let leaf = sha(&(kind.to_owned() + "\n" + key)) + ".json";
    let Some(bytes) = read_at(parent, OsStr::new(&leaf))? else {
        return Ok(None);
    };
    let value = codec::parse(&bytes).map_err(|_| corrupt())?;
    if !value.get("kind").is(kind)
        || !value.get("id").is(key)
        || !matches!(value.get("record"), Js::Obj(_))
    {
        return Err(corrupt());
    }
    Ok(Some(value.get("record").clone()))
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
fn bounded_lock(path: &Path, elapsed_ms: impl Fn() -> u128, waits: usize) -> Result<Guard> {
    let waits = Cell::new(waits);
    lock_path(
        path,
        LockOptions {
            wait: Duration::from_millis(LOCK_WAIT_MS),
            poll: Duration::from_millis(LOCK_POLL_MS),
        },
        &|| {
            let n = waits.get();
            if !poll_allowed(elapsed_ms(), n) {
                return true;
            }
            // Each permitted core iteration can sleep once: reserve that wait.
            waits.set(n + 1);
            false
        },
    )
    .map_err(|e| {
        if e.message == "workflow_lock_cancelled" {
            Fault::error("shared_writer_lock_busy")
        } else {
            e
        }
    })
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
        let start = Instant::now();
        bounded_lock(&pinned, || start.elapsed().as_millis(), 0)
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
    /// Read-only, at most 256 payloads for this owner/student; no expiry or deletion.
    pub fn lookup_drafts(&self, student: &str) -> Result<Vec<Draft>> {
        self.store.verify()?;
        if student.is_empty() {
            return Err(corrupt());
        }
        let bucket = content_hash(&(&self.store.owner, student))?;
        let root = directory(&self.store.root, OsStr::new("record-drafts"), true)?;
        let dir: File = match rfs::openat(
            &root,
            bucket.as_str(),
            flags() | OFlags::DIRECTORY,
            Mode::empty(),
        ) {
            Ok(fd) => fd.into(),
            Err(rustix::io::Errno::NOENT) => return Ok(vec![]),
            Err(e) => return Err(os_error(e)),
        };
        private(&dir, true)?;
        let mut leaves = vec![];
        for entry in entries(&dir)? {
            let entry = syscall(entry)?;
            if let Some(n) = name(&entry) {
                if leaves.len() == MAX_DRAFT_CANDIDATES {
                    return Err(capacity());
                }
                leaves.push(n.to_os_string());
            }
        }
        let mut drafts = vec![];
        let mut dates = BTreeSet::new();
        let mut open = BTreeSet::new();
        for leaf in leaves {
            let bytes = read_at(&dir, &leaf)?.ok_or_else(corrupt)?;
            let candidate: Draft = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            let identity = RecordIdentity {
                owner: self.store.owner.clone(),
                student_key: student.into(),
                ..candidate.identity.clone()
            };
            let d = validate_draft(&bytes, &identity)?;
            if draft_path(&identity, &d.generation)?.file_name() != Some(leaf.as_os_str()) {
                return Err(corrupt());
            }
            if d.state == DraftState::Open {
                if !open.insert(identity.key()?) {
                    return Err(corrupt());
                }
                dates.insert(identity.lesson_date);
                if dates.len() > MAX_OPEN_DATES {
                    return Err(capacity());
                }
            }
            drafts.push(d);
        }
        Ok(drafts)
    }
    /// Metadata-only global count; payload lookup and cleanup have their own 256 cap.
    pub fn retained_generations(&self) -> Result<usize> {
        self.store.verify()?;
        let root = directory(&self.store.root, OsStr::new("record-drafts"), true)?;
        let mut count = 0;
        let mut buckets = 0;
        for entry in entries(&root)? {
            let entry = syscall(entry)?;
            let Some(n) = name(&entry) else { continue };
            buckets += 1;
            if buckets > MAX_RETAINED_GENERATIONS {
                return Err(capacity());
            }
            if !n.to_str().is_some_and(digest) {
                return Err(corrupt());
            }
            let dir = directory(&root, n, true)?;
            for entry in entries(&dir)? {
                let entry = syscall(entry)?;
                let Some(n) = name(&entry) else { continue };
                count += 1;
                if count > MAX_RETAINED_GENERATIONS {
                    return Err(capacity());
                }
                regular(&dir, n, false)?.ok_or_else(corrupt)?;
            }
        }
        Ok(count)
    }
    /// Only on an explicit write/open attempt. Unknown unresolved references retain
    /// all candidates conservatively; the core owns execution classification.
    /// A later error may follow deletions; inspect the inventory before retrying.
    pub fn cleanup_drafts(&self, scope: &Js, now: &str) -> Result<DraftCleanup> {
        self.preflight_executions(scope, ExecutionAdmission::ReadOnly)?;
        let at = timestamp(now)?;
        let student = String::from_utf16(scope.get("studentId").units()).map_err(|_| corrupt())?;
        let drafts = self.lookup_drafts(&student)?;
        let core =
            edu_academy_core::workflow::store::Store::open(self.store.root(), self.store.root())?;
        let unresolved = state::student_executions(&core, scope)?;
        let refs: BTreeSet<_> = unresolved
            .iter()
            .filter(|e| e.state.needs_reconciliation())
            .map(|e| e.record.get("requestRef").code_units())
            .collect();
        let known: BTreeSet<_> = drafts
            .iter()
            .filter_map(|d| d.pending_attempt.as_ref().map(|a| a.request_ref.clone()))
            .collect();
        let unknown = refs.iter().any(|r| !known.contains(r));
        let mut result = DraftCleanup {
            examined: drafts.len(),
            ..Default::default()
        };
        for d in drafts {
            if unknown
                || d.pending_attempt
                    .as_ref()
                    .is_some_and(|a| refs.contains(&a.request_ref))
            {
                result.protected += 1;
                continue;
            }
            let Some(closed) = &d.closed_at else { continue };
            let eligible = timestamp(closed)?
                .max(timestamp(&d.last_used)?)
                .checked_add(DRAFT_RETENTION_MS)
                .ok_or_else(corrupt)?;
            if at < eligible {
                result.next_eligible_ms = Some(
                    result
                        .next_eligible_ms
                        .map_or(eligible, |old| old.min(eligible)),
                );
                continue;
            }
            let _draft = self.lock_draft(&d.identity)?;
            let path = draft_path(&d.identity, &d.generation)?;
            let (dir, leaf) = self.store.parent(&path)?;
            let bytes = read_at(&dir, &leaf)?.ok_or_else(corrupt)?;
            if validate_draft(&bytes, &d.identity)? != d {
                return Err(corrupt());
            }
            syscall(rfs::unlinkat(&dir, &leaf, rfs::AtFlags::empty()))?;
            io(dir.sync_all())?;
            result.removed += 1;
        }
        if result.removed > 0 && result.removed == result.examined {
            let root = directory(&self.store.root, OsStr::new("record-drafts"), true)?;
            let bucket = content_hash(&(&self.store.owner, student.as_str()))?;
            syscall(rfs::unlinkat(
                &root,
                bucket.as_str(),
                rfs::AtFlags::REMOVEDIR,
            ))?;
            io(root.sync_all())?;
        }
        Ok(result)
    }
    /// Call before locking the write's target, retaining this account guard through
    /// the subsequent draft lock/write. Include all new temp/archive bytes, without
    /// subtracting old payloads: atomic replacement temporarily needs both copies.
    pub fn admit_draft_write(
        &self,
        scope: &Js,
        now: &str,
        new_generation: Option<&RecordIdentity>,
        bytes: u64,
        entry_limit: usize,
    ) -> Result<RootUsage> {
        if let Some(id) = new_generation {
            if id.owner != self.store.owner || !scope.get("studentId").is(&id.student_key) {
                return Err(corrupt());
            }
            let date = chrono::NaiveDate::parse_from_str(&id.lesson_date, "%Y-%m-%d")
                .map_err(|_| corrupt())?;
            if date.format("%Y-%m-%d").to_string() != id.lesson_date {
                return Err(corrupt());
            }
        }
        self.cleanup_drafts(scope, now)?;
        let count = self.retained_generations()?;
        if count + usize::from(new_generation.is_some()) > MAX_RETAINED_GENERATIONS {
            return Err(capacity());
        }
        if let Some(id) = new_generation {
            let mut dates: BTreeSet<_> = self
                .lookup_drafts(&id.student_key)?
                .into_iter()
                .filter(|d| d.state == DraftState::Open)
                .map(|d| d.identity.lesson_date)
                .collect();
            dates.insert(id.lesson_date.clone());
            if dates.len() > MAX_OPEN_DATES {
                return Err(capacity());
            }
        }
        let usage = self.usage(entry_limit)?;
        if usage
            .bytes
            .checked_add(bytes)
            .is_none_or(|n| n > MAX_ROOT_BYTES)
        {
            return Err(capacity());
        }
        Ok(usage)
    }
    /// Read-only preflight of the files core student_executions will consume.
    /// Supply the server-resolved core scope; owner association is the caller's job.
    /// Keep this guard through core entry; Save reserves the entire field batch.
    /// This neither grants approval nor writes a second counter or ledger.
    pub fn preflight_executions(&self, scope: &Js, mode: ExecutionAdmission) -> Result<usize> {
        self.store.verify()?;
        for field in ["sourceNamespace", "teacherId", "studentId"] {
            if !scope.get(field).string() || scope.get(field).units().is_empty() {
                return Err(Fault::error("record_execution_scope_invalid"));
            }
        }
        let selected = match mode {
            ExecutionAdmission::ReadOnly => 0,
            ExecutionAdmission::Save { selected_fields }
                if (1..=RECORD_FIELD_COUNT).contains(&selected_fields) =>
            {
                selected_fields
            }
            _ => return Err(Fault::error("record_execution_fields_invalid")),
        };
        let group = state::lock_key(scope)?;
        let ledger = directory(&self.store.root, OsStr::new("ledger"), true)?;
        let group_dir = match rfs::openat(
            &ledger,
            sha(&group).as_str(),
            flags() | OFlags::DIRECTORY,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(0),
            Err(e) => return Err(os_error(e)),
        };
        private(&group_dir, true)?;
        let mut members = Vec::new();
        for entry in entries(&group_dir)? {
            let entry = syscall(entry)?;
            let Some(n) = name(&entry) else { continue };
            if members.len() == MAX_EXECUTION_RECORDS {
                return Err(execution_limit()); // Stop at entry 4,097 before reading it.
            }
            members.push(n.to_os_string());
        }
        let count = members.len();
        if selected > 0
            && (count == MAX_EXECUTION_RECORDS || count + selected > MAX_EXECUTION_RECORDS)
        {
            return Err(execution_limit());
        }
        let execution = directory(&self.store.root, OsStr::new("execution"), true)?;
        let outcome = directory(&self.store.root, OsStr::new("outcome"), true)?;
        let reconciliation = directory(&self.store.root, OsStr::new("reconciliation"), true)?;
        for leaf in members {
            let bytes = read_at(&group_dir, &leaf)?.ok_or_else(corrupt)?;
            let member = codec::parse(&bytes).map_err(|_| corrupt())?;
            if !member.get("group").is(&group) || !member.get("key").string() {
                return Err(corrupt());
            }
            let key = String::from_utf16(member.get("key").units()).map_err(|_| corrupt())?;
            if leaf != OsStr::new(&(sha(&key) + ".json")) {
                return Err(corrupt());
            }
            if let Some(record) = core_record(&execution, "execution", &key)? {
                if state::lock_key(record.get("scope"))? != group {
                    return Err(corrupt());
                }
                core_record(&outcome, "outcome", &key)?;
                core_record(&reconciliation, "reconciliation", &key)?;
            }
            // Index-only members count too, even though core skips their absent fences.
        }
        Ok(count)
    }
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
    use std::os::unix::fs::DirBuilderExt;
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
    #[test]
    fn delegated_lock_budget_boundaries() {
        let path = std::env::temp_dir().join(format!("record-lock-budget-{}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        for suffix in ["account.flock", "draft.flock"] {
            let path = path.join(suffix);
            let end = u128::from(LOCK_WAIT_MS);
            let lock = bounded_lock(&path, || end - 1, MAX_LOCK_WAITS - 1).unwrap();
            let calls = Cell::new(0);
            let result = bounded_lock(
                &path,
                || {
                    calls.set(calls.get() + 1);
                    0
                },
                MAX_LOCK_WAITS - 1,
            );
            assert_eq!(result.err().unwrap().message, "shared_writer_lock_busy");
            assert_eq!(calls.get(), 2); // One permitted wait, then the 100-wait limit.
            drop(lock);
            for over in [0, 1] {
                for (ms, waits) in [(end + over, 0), (0, MAX_LOCK_WAITS + over as usize)] {
                    // Even a free lock must stop through the actual core callback.
                    let error = bounded_lock(&path, || ms, waits).err().unwrap();
                    assert_eq!(error.message, "shared_writer_lock_busy");
                }
            }
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(path).unwrap();
    }
}
