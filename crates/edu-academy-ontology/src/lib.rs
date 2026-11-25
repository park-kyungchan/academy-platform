//! Academy ledger schema and the key-addressed operations of its engine adapter.
use edu_academy_core::{
    js::{Fault, Result, obj},
    workflow::{
        Runtime,
        ledger::{Attempt, AttemptLedger, AttemptTarget},
        state::FieldState,
        store::{Store, ledger_sibling},
    },
};
use ont::{
    Answer, Context, Failure,
    action::{ApplyActionRequest, apply_action},
    get::{GetRequest, get},
    search::{SearchRequest, search},
};
use ont_core::{Id, Namespace};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};

const MAX_EXECUTION_KEY_BYTES: usize = 8192;

pub const SCHEMA: &str = include_str!("../../../ontology/academy.schema.json");

/// The schema `academy_read`: the captured records as a read-only store with
/// its views: the textbook records (`registered_book`, `section_attempt`) and
/// the lesson record (`lesson_record`, `lesson_day`). Its spool root is
/// `ACADEMY_READ_SPOOL_ROOT`.
pub const ACADEMY_READ_SCHEMA: &str = include_str!("../../../ontology/academy-read.schema.json");

/// The schema `academy_memory`: what the teacher and the agent know about each
/// student and as of when (`docs/academy-memory.md`). It is installed in the
/// host's store beside the domain `work`, whose namespace it declares; the
/// Academy's server stays the authority of the records it reads.
pub const ACADEMY_MEMORY_SCHEMA: &str =
    include_str!("../../../ontology/academy-memory.schema.json");

/// The spool root that `ACADEMY_READ_SCHEMA` names in its source globs.
pub const ACADEMY_READ_SPOOL_ROOT: &str = "/var/lib/academy-read/spool";

/// Engine authority for attempt identity, state, digests and fencing.
#[derive(Clone)]
pub struct EngineLedger {
    store: PathBuf,
    read_time: u64,
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn id(object_type: &str, key: &str) -> Id {
    Id {
        object_type: object_type.into(),
        namespace: Namespace {
            instance: "academy://owner".into(),
            account: "teacher".into(),
        },
        native_key: hash(key),
    }
}
fn request_id(action: char, key: &str, digest: &str) -> String {
    let input = if action == 's' {
        key.to_owned()
    } else {
        format!("{key}\n{digest}")
    };
    format!("{action}-{}", &hash(&input)[..62])
}
fn unavailable() -> Fault {
    Fault::error("workflow_ledger_unavailable")
}
fn engine_failure(failure: Failure) -> Fault {
    let code = match failure {
        Failure::Refused(r) => r.code,
        Failure::Internal(i) => i.code,
    };
    Fault::from_parts(
        "Error",
        "workflow_ledger_unavailable",
        obj(&[("engineCode", code.into())]),
    )
}
fn answer(answer: Answer) -> Result<Value> {
    match answer {
        Answer::Enveloped { result, .. } => Ok(result),
        _ => Err(unavailable()),
    }
}
#[derive(Clone, Copy)]
enum Mutation {
    Begin,
    Outcome,
    Reconciliation,
}
fn mutation_answer(result: std::result::Result<Answer, Failure>, kind: Mutation) -> Result<bool> {
    match result {
        Ok(a) => match answer(a)?["outcome"].as_str() {
            Some("applied") => Ok(true),
            Some("already_applied") => Ok(false),
            _ => Err(unavailable()),
        },
        Err(Failure::Refused(r))
            if matches!(kind, Mutation::Begin) && r.code == "request_id_reused" =>
        {
            Ok(false)
        }
        Err(Failure::Refused(r)) if r.code == "submission_criteria" => {
            let failed = r
                .extra
                .get("failed")
                .and_then(Value::as_array)
                .ok_or_else(unavailable)?;
            let codes = failed
                .iter()
                .map(|v| v.as_str().ok_or_else(unavailable))
                .collect::<Result<Vec<_>>>()?;
            if codes.is_empty() {
                return Err(unavailable());
            }
            match kind {
                Mutation::Begin if codes.contains(&"attempt_exists") => Ok(false),
                Mutation::Begin if codes.contains(&"field_fenced") => {
                    Err(Fault::error("workflow_attempt_fenced"))
                }
                Mutation::Outcome
                    if codes
                        .iter()
                        .all(|c| matches!(*c, "outcome_exists" | "attempt_not_open")) =>
                {
                    Ok(false)
                }
                Mutation::Reconciliation if codes.contains(&"attempt_not_open") => Ok(false),
                _ => Err(engine_failure(Failure::Refused(r))),
            }
        }
        Err(f) => Err(engine_failure(f)),
    }
}
impl EngineLedger {
    /// The owner must have opened and installed the schema in this store first.
    /// Reads use the core Runtime's opening time; mutations use their call's Runtime.
    pub fn new(store: PathBuf, runtime: &dyn Runtime) -> Self {
        Self {
            store,
            read_time: runtime.epoch_ms() / 1000,
        }
    }
    fn context(&self, now: u64) -> Context {
        let mut ctx = Context::new(self.store.clone(), now);
        ctx.client = "edu-academy".into();
        ctx.agent = Some("ledger".into());
        ctx
    }
    fn apply(
        &self,
        action: &str,
        parameters: Value,
        rid: String,
        clock: &dyn Runtime,
    ) -> std::result::Result<Answer, Failure> {
        apply_action(
            &self.context(clock.epoch_ms() / 1000),
            &ApplyActionRequest {
                op: None,
                action_type: action.into(),
                parameters,
                agent: None,
                session: None,
                expected: None,
                request_id: Some(rid),
            },
        )
    }
    pub fn begin(
        &self,
        key: &str,
        target: &AttemptTarget,
        digest: &str,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        if key.len() > MAX_EXECUTION_KEY_BYTES {
            return Err(Fault::from_parts(
                "Error",
                "workflow_ledger_unavailable",
                obj(&[("engineCode", "execution_key_too_large".into())]),
            ));
        }
        let registered=answer(self.apply("register_student",json!({"scope":id("student",&target.group),"source_namespace":target.source_namespace,"teacher_id":target.teacher_id,"student_id":target.scope_student_id}),request_id('s',&target.group,""),clock).map_err(engine_failure)?)?;
        if !matches!(
            registered["outcome"].as_str(),
            Some("applied" | "already_applied")
        ) {
            return Err(unavailable());
        }
        mutation_answer(self.apply("begin_attempt",json!({
            "attempt":id("save_attempt",key),"student":id("student",&target.group),
            "field":target.field,"record_key":target.record_key,
            "source_namespace":target.occurrence[0],"student_id":target.occurrence[1],
            "lesson_date":target.occurrence[2],"course_id":target.occurrence[3],
            "request_lesson_date":target.request_lesson_date,"request_ref":target.request_ref,
            "execution_sha256":digest,"execution_key":key,
        }),request_id('b',key,digest),clock),Mutation::Begin)
    }
    pub fn record_outcome(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        let (action, prefix) = match state {
            FieldState::ReconciliationRequired => ("record_outcome", 'o'),
            FieldState::SettledPersisted => ("settle_outcome", 't'),
            _ => return self.closed_answer(key),
        };
        mutation_answer(
            self.apply(
                action,
                json!({"attempt":id("save_attempt",key),"outcome_sha256":digest}),
                request_id(prefix, key, digest),
                clock,
            ),
            Mutation::Outcome,
        )
    }
    pub fn record_reconciliation(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        if !matches!(
            state,
            FieldState::ReconciledPersisted
                | FieldState::ReconciledNotPersisted
                | FieldState::ReconciledConflict
        ) {
            return self.closed_answer(key);
        }
        mutation_answer(self.apply("record_reconciliation",json!({"attempt":id("save_attempt",key),"reconciliation_sha256":digest,"state":state.name()}),request_id('r',key,digest),clock),Mutation::Reconciliation)
    }
    fn closed_answer(&self, key: &str) -> Result<bool> {
        match self.attempt(key)? {
            Some(a) if !a.state.needs_reconciliation() => Ok(false),
            _ => Err(unavailable()),
        }
    }
    pub fn attempt(&self, key: &str) -> Result<Option<Attempt>> {
        let result = match get(
            &self.context(self.read_time),
            &GetRequest {
                op: None,
                id: id("save_attempt", key),
            },
        ) {
            Ok(a) => answer(a)?,
            Err(Failure::Refused(r)) if r.code == "unavailable" => return Ok(None),
            Err(f) => return Err(engine_failure(f)),
        };
        let objects = result["objects"].as_array().ok_or_else(unavailable)?;
        if objects.len() != 1 {
            return Err(unavailable());
        }
        decode_attempt(&objects[0]).map(Some)
    }
    fn search_attempts(&self, filter: Value, limit: usize) -> Result<Vec<Attempt>> {
        let mut attempts = Vec::new();
        let mut continuation = None;
        while attempts.len() < limit {
            let request = SearchRequest {
                op: None,
                object_type: "save_attempt".into(),
                filter: Some(serde_json::from_value(filter.clone()).map_err(|_| unavailable())?),
                limit: Some((limit - attempts.len()).min(1000) as u64),
                continuation: continuation.clone(),
            };
            let result =
                answer(search(&self.context(self.read_time), &request).map_err(engine_failure)?)?;
            if result["basis"]["qualified"] != true {
                return Err(unavailable());
            }
            let objects = result["objects"].as_array().ok_or_else(unavailable)?;
            for object in objects {
                attempts.push(decode_attempt(object)?);
            }
            let next = match &result["continuation"] {
                Value::Null => None,
                Value::String(token) => Some(token.clone()),
                _ => return Err(unavailable()),
            };
            if next.is_none() {
                break;
            }
            if objects.is_empty() || next == continuation {
                return Err(unavailable());
            }
            continuation = next;
        }
        Ok(attempts)
    }
}
impl AttemptLedger for EngineLedger {
    fn execution_key_limit(&self) -> Option<usize> {
        Some(MAX_EXECUTION_KEY_BYTES)
    }

    fn begin(
        &self,
        key: &str,
        target: &AttemptTarget,
        digest: &str,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        EngineLedger::begin(self, key, target, digest, clock)
    }
    fn record_outcome(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        EngineLedger::record_outcome(self, key, digest, state, clock)
    }
    fn record_reconciliation(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        EngineLedger::record_reconciliation(self, key, digest, state, clock)
    }
    fn attempt(&self, key: &str) -> Result<Option<Attempt>> {
        EngineLedger::attempt(self, key)
    }
    fn attempts(&self, group: &str, limit: usize) -> Result<Vec<Attempt>> {
        self.search_attempts(json!({"eq":["student",id("student",group)]}), limit)
    }
    fn open_on(&self, target: &AttemptTarget) -> Result<Vec<Attempt>> {
        self.search_attempts(
            json!({"all":[
                {"eq":["student",id("student",&target.group)]},
                {"in":["state",["dispatch_outcome_unknown","reconciliation_required"]]},
                {"eq":["field",target.field]},
                {"any":[{"eq":["record_key",target.record_key]}, {"all":[
                    {"eq":["source_namespace",target.occurrence[0]]},
                    {"eq":["student_id",target.occurrence[1]]},
                    {"eq":["lesson_date",target.occurrence[2]]},
                    {"eq":["course_id",target.occurrence[3]]}
                ]}]}
            ]}),
            usize::MAX,
        )
    }
}
fn decode_attempt(object: &Value) -> Result<Attempt> {
    let observation = if object["head"].is_object() {
        // search returns its single current observation directly under head.
        &object["head"]
    } else {
        // get includes history and identifies its current observation by revision.
        let head = object["head"].as_str().ok_or_else(unavailable)?;
        object["observations"]
            .as_array()
            .ok_or_else(unavailable)?
            .iter()
            .find(|o| o["revision"].as_str() == Some(head))
            .ok_or_else(unavailable)?
    };
    let fields = &observation["fields"];
    let key = property(fields, "execution_key")?;
    if object["id"]["object_type"] != "save_attempt" || object["id"]["native_key"] != hash(key) {
        return Err(unavailable());
    }
    let state = match property(fields, "state")? {
        "available" => FieldState::Available,
        "settled_noop" => FieldState::SettledNoop,
        "settled_not_persisted" => FieldState::SettledNotPersisted,
        "dispatch_outcome_unknown" => FieldState::DispatchUnknown,
        "reconciliation_required" => FieldState::ReconciliationRequired,
        "settled_persisted" => FieldState::SettledPersisted,
        "reconciled_persisted" => FieldState::ReconciledPersisted,
        "reconciled_not_persisted" => FieldState::ReconciledNotPersisted,
        "reconciled_conflict" => FieldState::ReconciledConflict,
        _ => return Err(unavailable()),
    };
    Ok(Attempt {
        key: key.into(),
        state,
        execution: digest(fields, "execution_sha256")?,
        outcome: optional_digest(fields, "outcome_sha256")?,
        reconciliation: optional_digest(fields, "reconciliation_sha256")?,
    })
}
/// Open the private engine ledger beside the body root, upgrading it before reads.
pub fn open_store(root: &Path, owner_root: &Path, runtime: &dyn Runtime) -> Result<Store> {
    let sibling =
        ledger_sibling(root).ok_or_else(|| Fault::error("workflow_ledger_root_required"))?;
    let root = root.components().collect::<PathBuf>();
    let root = root.as_path();
    if root == owner_root
        || !root.is_absolute()
        || !owner_root.is_absolute()
        || !root.starts_with(owner_root)
        || !sibling.starts_with(owner_root)
        || fs::canonicalize(owner_root).map_err(|_| unavailable())? != owner_root
    {
        return Err(Fault::error("workflow_ledger_root_required"));
    }
    // Validate both routes before creating either directory; never follow a symlink.
    for path in [root, sibling.as_path()] {
        let mut cursor = owner_root.to_path_buf();
        for component in path
            .strip_prefix(owner_root)
            .map_err(|_| unavailable())?
            .components()
        {
            if !matches!(component, Component::Normal(_)) {
                return Err(Fault::error("workflow_ledger_root_required"));
            }
            cursor.push(component);
            match fs::symlink_metadata(&cursor) {
                Ok(meta) if meta.file_type().is_symlink() => return Err(unavailable()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(unavailable()),
                _ => {}
            }
        }
    }
    if has_entries(&root.join("ledger"))? || has_legacy_execution(&root.join("execution"))? {
        return Err(Fault::error("workflow_ledger_legacy_root"));
    }
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&sibling)
        .map_err(|_| unavailable())?;
    let path = sibling.join("academy.sqlite");
    initialize_engine(&path, runtime, || Ok(()))?;
    let mut engine = ont_store::Store::open_schema_existing_writable(&path)
        .map_err(|e| engine_failure(Failure::Refused(ont::Refusal::new(e.code))))?;
    let pins = engine.schema_pins().map_err(|_| unavailable())?;
    if pins
        .get("academy")
        .is_none_or(|pin| pin.sha256 != hash(SCHEMA))
    {
        engine
            .install_schema(
                SCHEMA.as_bytes(),
                ont_store::schema_mode::InstallArgs {
                    installed_at: runtime.epoch_ms() / 1000,
                    ..Default::default()
                },
            )
            .map_err(|e| engine_failure(Failure::Refused(ont::Refusal::new(e.code))))?;
    }
    drop(engine);
    Store::open_with_ledger(root, owner_root, Box::new(EngineLedger::new(path, runtime)))
}
// The callback lets unit controls stop or interleave exactly before publication.
fn initialize_engine(
    path: &Path,
    runtime: &dyn Runtime,
    before_publish: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    let sibling = path.parent().ok_or_else(unavailable)?;
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| unavailable())?
        .as_nanos();
    let temp = sibling.join(format!(
        ".academy.sqlite.new-{}-{suffix}",
        std::process::id()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|_| unavailable())?;
    // A killed initializer's temporary file is never read and is left in place.
    // A kill between link and removal leaves a second hard link of the live store;
    // nothing opens that name, and cleanup may only unlink it.
    let mut engine = ont_store::Store::open_schema(&temp)
        .map_err(|e| engine_failure(Failure::Refused(ont::Refusal::new(e.code))))?;
    engine
        .install_schema(
            SCHEMA.as_bytes(),
            ont_store::schema_mode::InstallArgs {
                installed_at: runtime.epoch_ms() / 1000,
                ..Default::default()
            },
        )
        .map_err(|e| engine_failure(Failure::Refused(ont::Refusal::new(e.code))))?;
    drop(engine);
    file.sync_all().map_err(|_| unavailable())?;
    drop(file);
    before_publish()?;
    match fs::hard_link(&temp, path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(unavailable()),
    }
    fs::remove_file(temp).map_err(|_| unavailable())?;
    edu_academy_core::workflow::store::sync_dir(sibling)
}
fn has_entries(path: &Path) -> Result<bool> {
    match fs::read_dir(path) {
        Ok(mut entries) => entries
            .next()
            .transpose()
            .map(|v| v.is_some())
            .map_err(|_| unavailable()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(unavailable()),
    }
}
fn has_legacy_execution(path: &Path) -> Result<bool> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(unavailable()),
    };
    for entry in entries {
        let entry = entry.map_err(|_| unavailable())?;
        let name = entry.file_name();
        if let Some(name) = name.to_str().and_then(|n| n.strip_suffix(".json"))
            && name.len() == 64
            && name.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn property<'a>(fields: &'a Value, name: &str) -> Result<&'a str> {
    if fields[name]["kind"] != "value" {
        return Err(unavailable());
    }
    fields[name]["value"].as_str().ok_or_else(unavailable)
}
fn digest(fields: &Value, name: &str) -> Result<String> {
    let value = property(fields, name)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(unavailable());
    }
    Ok(value.into())
}
fn optional_digest(fields: &Value, name: &str) -> Result<Option<String>> {
    if fields.get(name).is_none() || fields[name]["kind"] == "missing" {
        Ok(None)
    } else {
        digest(fields, name).map(Some)
    }
}

#[cfg(test)]
mod constructor_controls {
    use super::*;
    use edu_academy_core::workflow::ledger_cases::{Clock, root};

    #[test]
    fn engine_store_appears_complete() {
        let owner = root("interrupted-engine-initializer");
        let sibling = owner.join("store.ledger");
        fs::create_dir(&sibling).unwrap();
        let path = sibling.join("academy.sqlite");
        assert!(
            initialize_engine(&path, &Clock, || {
                assert!(
                    !path.exists(),
                    "the final name must not appear before publication"
                );
                Err(Fault::error("invented_interruption"))
            })
            .is_err()
        );
        assert!(!path.exists());
        let leftovers = fs::read_dir(&sibling)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(leftovers.len(), 1);
        assert!(
            leftovers[0]
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(".academy.sqlite.new-")
        );
        open_store(&owner.join("store"), &owner, &Clock).unwrap();
        assert!(
            leftovers[0].exists(),
            "interrupted temporaries are left untouched"
        );
        let mut engine = ont_store::Store::open_schema_existing_writable(&path).unwrap();
        assert_eq!(
            engine.schema_pins().unwrap()["academy"].sha256,
            hash(SCHEMA)
        );
        drop(engine);
        fs::remove_dir_all(owner).unwrap();
    }

    #[test]
    fn publication_keeps_winning_attempt() {
        let owner = root("engine-publication-winner");
        let sibling = owner.join("store.ledger");
        fs::create_dir(&sibling).unwrap();
        let path = sibling.join("academy.sqlite");
        initialize_engine(&path, &Clock, || {
            open_store(&owner.join("store"), &owner, &Clock)?;
            let ledger = EngineLedger::new(path.clone(), &Clock);
            let target = AttemptTarget {
                group: "S1".into(),
                source_namespace: "invented".into(),
                teacher_id: "11".into(),
                scope_student_id: "S1".into(),
                request_lesson_date: "2040-01-04".into(),
                field: "homework".into(),
                record_key: "R1".into(),
                request_ref: "request".into(),
                occurrence: [
                    "invented".into(),
                    "S1".into(),
                    "2040-01-03".into(),
                    "C1".into(),
                ],
            };
            assert!(ledger.begin("winner", &target, &hash("execution"), &Clock)?);
            Ok(())
        })
        .unwrap();
        open_store(&owner.join("store"), &owner, &Clock).unwrap();
        assert!(
            EngineLedger::new(path, &Clock)
                .attempt("winner")
                .unwrap()
                .is_some(),
            "a later publisher must preserve the store and its winning attempt"
        );
        fs::remove_dir_all(owner).unwrap();
    }
}
