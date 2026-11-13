//! Academy ledger schema and the key-addressed operations of its engine adapter.
use edu_academy_core::{
    js::{Fault, Result, obj},
    workflow::{
        Runtime,
        ledger::{Attempt, AttemptTarget},
        state::FieldState,
    },
};
use ont::{
    Answer, Context, Failure,
    action::{ApplyActionRequest, apply_action},
    get::{GetRequest, get},
};
use ont_core::{Id, Namespace};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub const SCHEMA: &str = include_str!("../../../ontology/academy.schema.json");

/// Key-addressed operations only. This is not yet an `AttemptLedger`: the packet
/// does not preserve the original execution key needed by its enumeration methods.
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
            "execution_sha256":digest,
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
        let object = &objects[0];
        let head = object["head"].as_str().ok_or_else(unavailable)?;
        let observation = object["observations"]
            .as_array()
            .ok_or_else(unavailable)?
            .iter()
            .find(|o| o["revision"].as_str() == Some(head))
            .ok_or_else(unavailable)?;
        let fields = &observation["fields"];
        let state = match property(fields, "state")? {
            "available" => FieldState::Available,
            "dispatch_outcome_unknown" => FieldState::DispatchUnknown,
            "reconciliation_required" => FieldState::ReconciliationRequired,
            "settled_persisted" => FieldState::SettledPersisted,
            "settled_noop" => FieldState::SettledNoop,
            "settled_not_persisted" => FieldState::SettledNotPersisted,
            "reconciled_persisted" => FieldState::ReconciledPersisted,
            "reconciled_not_persisted" => FieldState::ReconciledNotPersisted,
            "reconciled_conflict" => FieldState::ReconciledConflict,
            _ => return Err(unavailable()),
        };
        Ok(Some(Attempt {
            key: key.into(),
            state,
            execution: digest(fields, "execution_sha256")?,
            outcome: optional_digest(fields, "outcome_sha256")?,
            reconciliation: optional_digest(fields, "reconciliation_sha256")?,
        }))
    }
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
