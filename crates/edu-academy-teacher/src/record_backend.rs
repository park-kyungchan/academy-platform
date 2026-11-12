//! Persistent invented records and scripted effects; no live transport or approval authority.
use crate::{
    record_data::*,
    record_store::{AccountGuard, DraftStore, MAX_ROOT_BYTES},
};
use edu_academy_core::js::{Fault, Result};
use edu_academy_sources::{
    data::{self, obj},
    native::{self, SourcePort},
};
use rustix::fs::{self as rfs, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path},
    sync::{Arc, Mutex},
};

pub const MAX_SOURCE_SNAPSHOT_BYTES: usize = 1_048_576;
pub const MAX_SOURCE_RECORDS: usize = 256;
pub const MAX_SOURCE_OPERATIONS: usize = 128;
pub const MAX_SOURCE_JSON_DEPTH: usize = 32;
pub const SOURCE_SNAPSHOT: &str = "record-source.json";
pub const RECORD_SOURCE_FORMAT: &str = "academy.record-source/3";
pub const MAX_SOURCE_SCRIPTS: usize = 256;
pub const SOURCE_READ_DEADLINE_MS: u64 = 2_000;
pub const MAX_SOURCE_READ_RETRIES: usize = 1;
pub const MAX_RECORD_STUDENTS: usize = 256;
pub const MAX_RECORD_BINDINGS: usize = 32;

/// Metadata is invented scenario data; every academic read uses the shared state.
pub struct SyntheticRecordDayPort {
    state: Arc<Mutex<SyntheticRecordState>>,
    students: Vec<crate::synthetic::ScenarioStudent>,
    read_at: String,
}
impl SyntheticRecordDayPort {
    pub fn new(
        state: Arc<Mutex<SyntheticRecordState>>,
        students: Vec<crate::synthetic::ScenarioStudent>,
        read_at: &str,
    ) -> Result<Self> {
        if students.len() > MAX_RECORD_STUDENTS {
            return Err(limit());
        }
        chrono::DateTime::parse_from_rfc3339(read_at).map_err(|_| invalid())?;
        let mut seen = BTreeMap::new();
        for student in &students {
            native::literal_name(&student.name.clone().into()).map_err(data::fault_from_error)?;
            native::literal_key(&student.key.clone().into()).map_err(data::fault_from_error)?;
            if student.bindings.is_empty() {
                return Err(invalid());
            }
            if student.bindings.len() > MAX_RECORD_BINDINGS {
                return Err(limit());
            }
            for binding in &student.bindings {
                native::literal_key(&binding.course_key.clone().into())
                    .map_err(data::fault_from_error)?;
            }
            let signature = (&student.name, &student.bindings[0].course_key);
            if seen
                .insert(&student.key, signature)
                .is_some_and(|old| old != signature)
            {
                return Err(invalid());
            }
        }
        Ok(Self {
            state,
            students,
            read_at: read_at.into(),
        })
    }
    pub fn set_read_at(&mut self, read_at: &str) -> Result<()> {
        chrono::DateTime::parse_from_rfc3339(read_at).map_err(|_| invalid())?;
        self.read_at = read_at.into();
        Ok(())
    }
    // Never retain the mutex while waiting for account: adapter callers already
    // holding account can acquire the same mutex without an inverted lock order.
    fn locked<T>(
        &self,
        f: impl FnOnce(&mut SyntheticRecordState, &AccountGuard<'_>) -> Result<T>,
    ) -> data::Result<T> {
        let store = self
            .state
            .lock()
            .map_err(|_| data::Error::error("record_source_unavailable"))?
            .store
            .clone();
        let account = store.lock_account().map_err(data::error_from_fault)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| data::Error::error("record_source_unavailable"))?;
        f(&mut state, &account).map_err(data::error_from_fault)
    }
    fn snapshot(
        &self,
        state: &mut SyntheticRecordState,
        account: &AccountGuard<'_>,
        student: &crate::synthetic::ScenarioStudent,
        date: &str,
    ) -> Result<RecordSnapshot> {
        let id = RecordIdentity {
            owner: state.owner.clone(),
            student_key: student.key.clone(),
            lesson_date: date.into(),
            kind: RecordKind::DailyRecord,
        };
        state.read_with_account(account, &id, &self.read_at)
    }
    fn row(
        &self,
        student: &crate::synthetic::ScenarioStudent,
        record: &RecordSnapshot,
    ) -> Result<edu_academy_core::js::Js> {
        let RecordObservation::Observed { occurrence, fields } = &record.observation else {
            return Err(invalid());
        };
        let mut values = obj(&[]);
        for field in RecordField::ALL {
            if let RecordValue::Known(value) = &fields[field] {
                values.set(field.wire_name(), value.clone().into());
            }
        }
        Ok(obj(&[
            ("studentName", student.name.clone().into()),
            ("date", record.requested_date.clone().into()),
            ("groupId", "1".into()),
            ("teacherKey", crate::synthetic::TEACHER_ID.into()),
            ("studentKey", student.key.clone().into()),
            ("courseKey", student.bindings[0].course_key.clone().into()),
            ("recordKey", occurrence.clone().into()),
            ("membershipKey", "601".into()),
            ("fields", values),
        ]))
    }
}
impl SourcePort for SyntheticRecordDayPort {
    fn authenticate(&mut self) -> data::Result<edu_academy_core::js::Js> {
        self.locked(|state, _| {
            state.operation()?;
            Ok(obj(&[("status", "authenticated".into())]))
        })
    }
    fn discover(
        &mut self,
        input: &edu_academy_core::js::Js,
    ) -> data::Result<edu_academy_core::js::Js> {
        if !input
            .get("teacherLabel")
            .is(crate::synthetic::TEACHER_LABEL)
        {
            return Err(data::Error::error("record_source_request_invalid"));
        }
        self.locked(|state, _| {
            state.operation()?;
            Ok(obj(&[(
                "groups",
                edu_academy_core::js::Js::Arr(vec![obj(&[("groupId", "1".into())])]),
            )]))
        })
    }
    fn execute(
        &mut self,
        input: &edu_academy_core::js::Js,
    ) -> data::Result<edu_academy_core::js::Js> {
        let names = input.get("studentNames").array();
        let groups = input.get("groupIds").array();
        if !input.get("family").is("day_record")
            || !input
                .get("teacherLabel")
                .is(crate::synthetic::TEACHER_LABEL)
            || names.len() != 1
            || !names[0].string()
            || groups.len() != 1
            || !groups[0].is("1")
            || !input.get("date").string()
        {
            return Err(data::Error::error("record_source_request_invalid"));
        }
        self.locked(|state, account| {
            let matching: Vec<_> = self
                .students
                .iter()
                .filter(|s| names[0].is(&s.name))
                .collect();
            let rows = if let Some(student) = matching.first() {
                if matching.iter().any(|s| s.key != student.key) {
                    return Err(Fault::error("record_source_name_ambiguous"));
                }
                let date = String::from_utf16(input.get("date").units()).map_err(|_| invalid())?;
                let record = self.snapshot(state, account, student, &date)?;
                if matches!(record.observation, RecordObservation::Observed { .. }) {
                    vec![self.row(student, &record)?]
                } else {
                    vec![]
                }
            } else {
                state.operation()?;
                vec![]
            };
            Ok(obj(&[("dayRecords", edu_academy_core::js::Js::Arr(rows))]))
        })
    }
    fn journal_state(
        &mut self,
        target: &edu_academy_core::js::Js,
    ) -> data::Result<edu_academy_core::js::Js> {
        self.locked(|state, account| {
            let student = self
                .students
                .iter()
                .find(|s| target.get("studentKey").is(&s.key))
                .ok_or_else(invalid)?;
            let date = String::from_utf16(target.get("date").units()).map_err(|_| invalid())?;
            let current = self.snapshot(state, account, student, &date)?;
            let current_row = self.row(student, &current)?;
            if native::target(&current_row) != native::target(target) {
                return Err(Fault::error("native_dailyrecord_state_conflict"));
            }
            state.operation()?;
            state.reads = state.reads.checked_add(1).ok_or_else(limit)?;
            let source = state.load()?;
            let prior = source
                .records
                .iter()
                .filter(|r| r.identity.student_key == student.key && r.identity.lesson_date < date)
                .max_by_key(|r| &r.identity.lesson_date)
                .ok_or_else(|| Fault::error("native_dailyrecord_previous_unverified"))?;
            let previous = self.snapshot(state, account, student, &prior.identity.lesson_date)?;
            let previous_row = self.row(student, &previous)?;
            Ok(obj(&[
                ("target", native::target(&current_row)),
                ("previousDate", previous_row.get("date").clone()),
                ("previousRecordKey", previous_row.get("recordKey").clone()),
                ("fields", current_row.get("fields").clone()),
                ("wireDigest", content_hash(&current.observation)?.into()),
            ]))
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WritableTarget {
    pub identity: RecordIdentity,
    pub field: RecordField,
    pub synthetic_target: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReadOutcome {
    Observed { elapsed_ms: u64 },
    Timeout,
    Unavailable,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadScript {
    pub identity: RecordIdentity,
    pub outcome: ReadOutcome,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectOutcome {
    Success,
    Reject,
    AppliedLostAck,
    UnappliedLostAck,
    ReturnedTimeout,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectScript {
    pub identity: RecordIdentity,
    pub field: RecordField,
    pub outcome: EffectOutcome,
    pub readback: Option<ReadOutcome>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportOutcome {
    Returned,
    Lost,
    ReturnedTimeout,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectResult {
    pub field: RecordField,
    pub outcome: EffectOutcome,
    pub transport: TransportOutcome,
}

/// Only concrete owned synthetic data can enter this constructor boundary.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordSeed {
    pub format: String,
    pub owner: OwnerScope,
    pub records: Vec<RecordSnapshot>,
    pub writable_targets: Vec<WritableTarget>,
    pub read_scripts: Vec<ReadScript>,
    pub effect_scripts: Vec<EffectScript>,
}
/// Counters belong to this process, while every read reloads the durable source.
pub struct SyntheticRecordState {
    store: Arc<DraftStore>,
    root: File,
    owner: OwnerScope,
    reads: u64,
    operations: usize,
    dispatches: u64,
    entry_limit: usize,
}
fn unavailable() -> Fault {
    Fault::error("record_source_unavailable")
}
fn invalid() -> Fault {
    Fault::error("record_source_invalid")
}
fn limit() -> Fault {
    Fault::error("record_source_limit")
}
fn io<T, E>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|_| unavailable())
}
fn flags() -> OFlags {
    OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK
}
fn root_file(path: &Path) -> Result<File> {
    let mut root: File = io(rfs::open(
        "/",
        flags() | OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
    ))?
    .into();
    for part in path.components() {
        match part {
            Component::Normal(name) => {
                root = io(rfs::openat(
                    &root,
                    name,
                    flags() | OFlags::RDONLY | OFlags::DIRECTORY,
                    Mode::empty(),
                ))?
                .into()
            }
            Component::RootDir => {}
            _ => return Err(invalid()),
        }
    }
    Ok(root)
}
fn private(file: &File) -> Result<()> {
    let m = io(file.metadata())?;
    if !m.is_file()
        || m.nlink() != 1
        || m.mode() & 0o777 != 0o600
        || m.uid() != rustix::process::geteuid().as_raw()
    {
        return Err(invalid());
    }
    Ok(())
}
fn check_root(root: &File, account: &AccountGuard<'_>) -> Result<()> {
    let m = io(root.metadata())?;
    if (m.dev(), m.ino()) != account.root_identity()? {
        return Err(Fault::error("record_source_root_mismatch"));
    }
    Ok(())
}
fn remove_temp(root: &File) -> Result<()> {
    match rfs::unlinkat(
        root,
        format!(".record-source-{}.tmp", std::process::id()),
        rfs::AtFlags::empty(),
    ) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(_) => Err(unavailable()),
    }
}
fn read_bytes(root: &File) -> Result<Option<Vec<u8>>> {
    let file: File = match rfs::openat(
        root,
        SOURCE_SNAPSHOT,
        flags() | OFlags::RDONLY,
        Mode::empty(),
    ) {
        Ok(fd) => fd.into(),
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(unavailable()),
    };
    private(&file)?;
    if io(file.metadata())?.len() > MAX_SOURCE_SNAPSHOT_BYTES as u64 {
        return Err(limit());
    }
    let mut bytes = Vec::new();
    io(file
        .take((MAX_SOURCE_SNAPSHOT_BYTES + 1) as u64)
        .read_to_end(&mut bytes))?;
    if bytes.len() > MAX_SOURCE_SNAPSHOT_BYTES {
        return Err(limit());
    }
    Ok(Some(bytes))
}
fn depth(bytes: &[u8]) -> Result<()> {
    let (mut level, mut quoted, mut escaped) = (0usize, false, false);
    for &b in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    level += 1;
                    if level > MAX_SOURCE_JSON_DEPTH {
                        return Err(limit());
                    }
                }
                b'}' | b']' => level = level.checked_sub(1).ok_or_else(invalid)?,
                _ => {}
            }
        }
    }
    Ok(()) // The typed JSON decoder checks balanced syntax and string endings.
}
fn valid_identity(id: &RecordIdentity, owner: &OwnerScope) -> Result<()> {
    let date =
        chrono::NaiveDate::parse_from_str(&id.lesson_date, "%Y-%m-%d").map_err(|_| invalid())?;
    if &id.owner != owner
        || id.student_key.is_empty()
        || date.format("%Y-%m-%d").to_string() != id.lesson_date
    {
        return Err(invalid());
    }
    Ok(())
}
fn validate(seed: &RecordSeed, owner: &OwnerScope) -> Result<()> {
    if seed.format != RECORD_SOURCE_FORMAT {
        return Err(Fault::error("record_source_format_unsupported"));
    }
    if &seed.owner != owner || owner.owner.is_empty() || owner.account.is_empty() {
        return Err(invalid());
    }
    if seed.records.len() > MAX_SOURCE_RECORDS {
        return Err(limit());
    }
    if seed.writable_targets.len() > MAX_SOURCE_RECORDS
        || seed.read_scripts.len() > MAX_SOURCE_SCRIPTS
        || seed.effect_scripts.len() > MAX_SOURCE_SCRIPTS
    {
        return Err(limit());
    }
    let mut targets = BTreeSet::new();
    for t in &seed.writable_targets {
        valid_identity(&t.identity, owner)?;
        if t.synthetic_target.is_empty()
            || !targets.insert((t.identity.key()?, t.field.wire_name()))
        {
            return Err(invalid());
        }
    }
    for id in seed
        .read_scripts
        .iter()
        .map(|s| &s.identity)
        .chain(seed.effect_scripts.iter().map(|s| &s.identity))
    {
        valid_identity(id, owner)?;
    }
    let mut identities = BTreeSet::new();
    for record in &seed.records {
        valid_identity(&record.identity, owner)?;
        if record.requested_date != record.identity.lesson_date
            || !identities.insert(record.identity.key()?)
        {
            return Err(invalid());
        }
        chrono::DateTime::parse_from_rfc3339(&record.provenance.read_at).map_err(|_| invalid())?;
        // Absence is represented by a missing row; transient failures are not records.
        if !matches!(&record.observation, RecordObservation::Observed { occurrence, .. } if !occurrence.is_empty())
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn decode(bytes: &[u8], owner: &OwnerScope) -> Result<RecordSeed> {
    if bytes.len() > MAX_SOURCE_SNAPSHOT_BYTES {
        return Err(limit());
    }
    depth(bytes)?;
    let version: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if version.get("format").and_then(|v| v.as_str()) != Some(RECORD_SOURCE_FORMAT) {
        return Err(Fault::error("record_source_format_unsupported"));
    }
    let seed: RecordSeed = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    validate(&seed, owner)?;
    Ok(seed)
}
fn initialize(root: &File, bytes: &[u8]) -> Result<()> {
    let temp = format!(".record-source-{}.tmp", std::process::id());
    let mut file: File = io(rfs::openat(
        root,
        temp.as_str(),
        flags() | OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
        Mode::from_raw_mode(0o600),
    ))?
    .into();
    let mut published = false;
    let result = (|| {
        private(&file)?;
        io(file.write_all(bytes).and_then(|()| file.sync_all()))?;
        io(rfs::renameat(root, temp.as_str(), root, SOURCE_SNAPSHOT))?;
        published = true;
        io(root.sync_all())
    })();
    if result.is_err() {
        // No retry: bytes may already be visible after an uncertain directory sync.
        let _ = rfs::unlinkat(root, temp.as_str(), rfs::AtFlags::empty());
    }
    result.map_err(|mut error| {
        if !published {
            error.properties.set("writeOutcome", "not_published".into());
            return error;
        }
        let status = match read_bytes(root) {
            Ok(Some(value)) if value == bytes => "matches",
            Ok(Some(_)) => "different",
            Ok(None) => "missing",
            Err(_) => "unreadable",
        };
        error.properties.set("writeOutcome", "uncertain".into());
        error.properties.set("targetReadback", status.into());
        error
    })
}
impl SyntheticRecordState {
    /// Open the marked owner root first. Seed only an absent snapshot; an existing
    /// snapshot is validated and reopened, never merged with or replaced by seed.
    /// entry_limit bounds root accounting during first publication (including temp).
    pub fn open(root: &Path, seed: RecordSeed, entry_limit: usize) -> Result<Self> {
        let store = DraftStore::open(root, seed.owner.clone())?;
        let account = store.lock_account()?;
        let directory = root_file(root)?;
        check_root(&directory, &account)?;
        if let Some(bytes) = read_bytes(&directory)? {
            decode(&bytes, &seed.owner)?;
        } else {
            remove_temp(&directory)?;
            validate(&seed, &seed.owner)?;
            let bytes = serde_json::to_vec(&seed).map_err(|_| invalid())?;
            decode(&bytes, &seed.owner)?;
            let usage = account.usage(entry_limit)?;
            if usage
                .bytes
                .checked_add(bytes.len() as u64)
                .is_none_or(|n| n > MAX_ROOT_BYTES)
                || usage.entries >= entry_limit
            {
                return Err(Fault::error("record_store_capacity"));
            }
            initialize(&directory, &bytes)?;
        }
        drop(account);
        Ok(Self {
            store: Arc::new(store),
            root: directory,
            owner: seed.owner,
            reads: 0,
            operations: 0,
            dispatches: 0,
            entry_limit,
        })
    }
    /// The server starts one logical teacher call; this is not an approval gate.
    pub fn begin_call(&mut self) {
        self.operations = 0;
    }
    pub fn read_count(&self) -> u64 {
        self.reads
    }
    pub fn source_operations(&self) -> usize {
        self.operations
    }
    pub fn dispatch_count(&self) -> u64 {
        self.dispatches
    }
    /// This backend has no parent transmission operation.
    pub fn notification_count(&self) -> u64 {
        0
    }
    fn operation(&mut self) -> Result<()> {
        if self.operations == MAX_SOURCE_OPERATIONS {
            return Err(limit());
        }
        self.operations += 1;
        Ok(())
    }
    fn load(&self) -> Result<RecordSeed> {
        decode(
            &read_bytes(&self.root)?.ok_or_else(unavailable)?,
            &self.owner,
        )
    }
    fn persist(&self, account: &AccountGuard<'_>, source: &RecordSeed) -> Result<()> {
        check_root(&self.root, account)?;
        let bytes = serde_json::to_vec(source).map_err(|_| invalid())?;
        decode(&bytes, &self.owner)?;
        remove_temp(&self.root)?;
        let usage = account.usage(self.entry_limit)?;
        if usage
            .bytes
            .checked_add(bytes.len() as u64)
            .is_none_or(|n| n > MAX_ROOT_BYTES)
            || usage.entries >= self.entry_limit
        {
            return Err(Fault::error("record_store_capacity"));
        }
        initialize(&self.root, &bytes)
    }
    pub(crate) fn source_store(&self, id: &RecordIdentity) -> Result<Arc<DraftStore>> {
        valid_identity(id, &self.owner)?;
        Ok(Arc::clone(&self.store))
    }
    /// The recorded-answer adapter makes one read, with no inferred deadline or retry.
    pub(crate) fn read_once_with_account(
        &mut self,
        account: &AccountGuard<'_>,
        id: &RecordIdentity,
        read_at: &str,
    ) -> Result<RecordSnapshot> {
        check_root(&self.root, account)?;
        valid_identity(id, &self.owner)?;
        chrono::DateTime::parse_from_rfc3339(read_at).map_err(|_| invalid())?;
        self.operation()?;
        self.reads = self.reads.checked_add(1).ok_or_else(limit)?;
        let mut source = self.load()?;
        if let Some(n) = source.read_scripts.iter().position(|s| s.identity == *id) {
            let outcome = source.read_scripts.remove(n).outcome;
            self.persist(account, &source)?;
            match outcome {
                ReadOutcome::Observed { .. } => {}
                ReadOutcome::Unavailable => return Err(unavailable()),
                ReadOutcome::Timeout => return Err(Fault::error("record_source_timeout")),
            }
        }
        Ok(observe(source, id, read_at))
    }
    /// Exact resolved identity only. Never searches another date or projects a
    /// known blank/unknown/attribution reason to an invented academic value.
    pub fn read(&mut self, id: &RecordIdentity, read_at: &str) -> Result<RecordSnapshot> {
        valid_identity(id, &self.owner)?;
        chrono::DateTime::parse_from_rfc3339(read_at).map_err(|_| invalid())?;
        let store = Arc::clone(&self.store);
        let account = store.lock_account()?;
        self.read_with_account(&account, id, read_at)
    }
    /// Evidence of the same held root lock; never acquires an account lock again.
    pub fn read_with_account(
        &mut self,
        account: &AccountGuard<'_>,
        id: &RecordIdentity,
        read_at: &str,
    ) -> Result<RecordSnapshot> {
        check_root(&self.root, account)?;
        valid_identity(id, &self.owner)?;
        chrono::DateTime::parse_from_rfc3339(read_at).map_err(|_| invalid())?;
        let mut source;
        for retry in 0..=MAX_SOURCE_READ_RETRIES {
            self.operation()?;
            self.reads = self.reads.checked_add(1).ok_or_else(limit)?;
            source = self.load()?;
            let outcome =
                if let Some(n) = source.read_scripts.iter().position(|s| s.identity == *id) {
                    let outcome = source.read_scripts.remove(n).outcome;
                    self.persist(account, &source)?;
                    outcome
                } else {
                    ReadOutcome::Observed { elapsed_ms: 0 }
                };
            match outcome {
                ReadOutcome::Unavailable => return Err(unavailable()),
                ReadOutcome::Observed { elapsed_ms } if elapsed_ms < SOURCE_READ_DEADLINE_MS => {
                    return Ok(observe(source, id, read_at));
                }
                _ if retry < MAX_SOURCE_READ_RETRIES => continue,
                _ => return Err(Fault::error("record_source_timeout")),
            }
        }
        unreachable!("bounded read always returns")
    }
    /// One invented batch, under the caller's lock. Core approval/fences remain
    /// the adapter's responsibility; this API never retries an effect.
    pub fn dispatch_with_account(
        &mut self,
        account: &AccountGuard<'_>,
        id: &RecordIdentity,
        patch: &RecordPatch,
        at: &str,
    ) -> Result<Vec<EffectResult>> {
        check_root(&self.root, account)?;
        valid_identity(id, &self.owner)?;
        chrono::DateTime::parse_from_rfc3339(at).map_err(|_| invalid())?;
        let mut source = self.load()?;
        if !source.writable_targets.iter().any(|t| t.identity == *id) {
            return Err(Fault::error("record_source_not_writable"));
        }
        if RecordField::ALL.iter().all(|&f| patch[f].is_none())
            || RecordField::ALL.iter().any(|&f| {
                patch[f].as_ref().is_some_and(|p| {
                    !source.writable_targets.iter().any(|t| {
                        t.identity == *id && t.field == f && t.synthetic_target == p.effect_target
                    })
                })
            })
        {
            return Err(invalid());
        }
        let mut record = observe(source.clone(), id, at);
        let mut applied = false;
        let mut results = Vec::new();
        for field in RecordField::ALL {
            let Some(p) = &patch[field] else { continue };
            let script = source
                .effect_scripts
                .iter()
                .position(|s| s.identity == *id && s.field == field)
                .map(|n| source.effect_scripts.remove(n));
            let outcome = script
                .as_ref()
                .map_or(EffectOutcome::Success, |s| s.outcome);
            if let Some(readback) = script.and_then(|s| s.readback) {
                source.read_scripts.push(ReadScript {
                    identity: id.clone(),
                    outcome: readback,
                });
            }
            if matches!(
                outcome,
                EffectOutcome::Success | EffectOutcome::AppliedLostAck
            ) {
                if matches!(record.observation, RecordObservation::NotObserved) {
                    record.observation = RecordObservation::Observed {
                        occurrence: p.effect_target.clone(),
                        fields: RecordFields::from_fn(|_| RecordValue::Unknown),
                    };
                }
                if let RecordObservation::Observed { fields, .. } = &mut record.observation {
                    fields[field] = RecordValue::Known(p.value.clone());
                }
                record.provenance.field_reasons[field] = None;
                applied = true;
            }
            results.push(EffectResult {
                field,
                outcome,
                transport: match outcome {
                    EffectOutcome::AppliedLostAck | EffectOutcome::UnappliedLostAck => {
                        TransportOutcome::Lost
                    }
                    EffectOutcome::ReturnedTimeout => TransportOutcome::ReturnedTimeout,
                    _ => TransportOutcome::Returned,
                },
            });
        }
        if applied {
            record.provenance.source_revision = Some(content_hash(&record.observation)?);
            record.provenance.wire_digest = record.provenance.source_revision.clone();
            if let Some(row) = source.records.iter_mut().find(|r| r.identity == *id) {
                *row = record;
            } else {
                source.records.push(record);
            }
        }
        validate(&source, &self.owner)?;
        self.operation()?;
        let next_count = self.dispatches.checked_add(1).ok_or_else(limit)?;
        let published = self.persist(account, &source);
        if published.is_ok()
            || published
                .as_ref()
                .is_err_and(|e| e.properties.get("writeOutcome").is("uncertain"))
        {
            self.dispatches = next_count;
        }
        published?;
        Ok(results)
    }
}
fn observe(source: RecordSeed, id: &RecordIdentity, read_at: &str) -> RecordSnapshot {
    let mut record = source
        .records
        .into_iter()
        .find(|r| r.identity == *id)
        .unwrap_or_else(|| RecordSnapshot {
            identity: id.clone(),
            requested_date: id.lesson_date.clone(),
            provenance: RecordProvenance {
                read_at: read_at.into(),
                evidence_refs: vec![],
                source_revision: None,
                wire_digest: None,
                field_reasons: RecordFields::from_fn(|_| None),
            },
            observation: RecordObservation::NotObserved,
        });
    record.provenance.read_at = read_at.into();
    record
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_depth_boundary_ignores_escaped_string_contents() {
        for n in [MAX_SOURCE_JSON_DEPTH, MAX_SOURCE_JSON_DEPTH + 1] {
            let bytes = format!("{}0{}", "[".repeat(n), "]".repeat(n));
            assert_eq!(depth(bytes.as_bytes()).is_ok(), n == MAX_SOURCE_JSON_DEPTH);
        }
        depth(br#"{"text":"[\"{\\]"}"#).unwrap();
    }
}
