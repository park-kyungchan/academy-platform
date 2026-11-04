use super::{
    canonical,
    store::{Kind, Store},
};
use crate::js::{Fault, Js, Result, obj, sha};
pub fn request_key(scope: &Js, request: impl Into<Js>) -> Result<String> {
    canonical(&obj(&[
        ("scope", scope.clone()),
        ("requestRef", request.into()),
    ]))
}
pub fn lock_key(scope: &Js) -> Result<String> {
    canonical(&obj(&[
        ("sourceNamespace", scope.get("sourceNamespace").clone()),
        ("teacherId", scope.get("teacherId").clone()),
        ("studentId", scope.get("studentId").clone()),
    ]))
}
pub fn execution_key(
    request: impl Into<Js>,
    scope: &Js,
    field: &str,
    record: impl Into<Js>,
    before: &Js,
    after: &Js,
) -> Result<String> {
    canonical(&obj(&[
        ("requestRef", request.into()),
        ("student", lock_key(scope)?.into()),
        ("field", field.into()),
        ("effectRecordKey", record.into()),
        (
            "beforeSha256",
            sha(&String::from_utf16_lossy(before.units())).into(),
        ),
        (
            "afterSha256",
            sha(&String::from_utf16_lossy(after.units())).into(),
        ),
    ]))
}
/// Preserve the new exact key while refusing legacy aliases for the same
/// recorded transition, including settled outcomes. Recovery must be explicit.
pub fn checked_execution_key(
    store: &Store,
    request: &Js,
    scope: &Js,
    transition: &Js,
) -> Result<String> {
    let field = transition.get("field").code_units();
    let record = transition.get("effectRecordKey");
    let before = transition.get("before");
    let after = transition.get("after");
    let key = execution_key(
        request.clone(),
        scope,
        &field,
        record.clone(),
        before,
        after,
    )?;
    let before_sha = sha(&String::from_utf16_lossy(before.units()));
    let after_sha = sha(&String::from_utf16_lossy(after.units()));
    for prior in student_executions(store, scope)? {
        let r = &prior.record;
        if r.get("requestRef") == request
            && r.get("field").is(&field)
            && r.get("effectRecordKey") == record
            && r.get("beforeSha256").is(&before_sha)
            && r.get("afterSha256").is(&after_sha)
            && prior.key != key
        {
            return Err(Fault::error("workflow_execution_key_encoding_conflict"));
        }
    }
    Ok(key)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldState {
    Available,
    DispatchUnknown,
    ReconciliationRequired,
    SettledPersisted,
    SettledNoop,
    SettledNotPersisted,
    ReconciledPersisted,
    ReconciledNotPersisted,
    ReconciledConflict,
}
impl FieldState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::DispatchUnknown => "dispatch_outcome_unknown",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::SettledPersisted => "settled_persisted",
            Self::SettledNoop => "settled_noop",
            Self::SettledNotPersisted => "settled_not_persisted",
            Self::ReconciledPersisted => "reconciled_persisted",
            Self::ReconciledNotPersisted => "reconciled_not_persisted",
            Self::ReconciledConflict => "reconciled_conflict",
        }
    }
    pub fn needs_reconciliation(self) -> bool {
        matches!(self, Self::DispatchUnknown | Self::ReconciliationRequired)
    }
    pub fn settled_effect(self) -> bool {
        matches!(
            self,
            Self::SettledPersisted | Self::SettledNoop | Self::ReconciledPersisted
        )
    }
    pub fn consumed_without_effect(self) -> bool {
        matches!(
            self,
            Self::SettledNotPersisted | Self::ReconciledNotPersisted | Self::ReconciledConflict
        )
    }
}
pub fn field_state(store: &Store, key: &str) -> Result<FieldState> {
    let r = store.get(Kind::Reconciliation, key)?;
    if !r.absent() {
        return Ok(match r.get("classification").code_units().as_str() {
            "persisted" => FieldState::ReconciledPersisted,
            "not_persisted" => FieldState::ReconciledNotPersisted,
            _ => FieldState::ReconciledConflict,
        });
    }
    let o = store.get(Kind::Outcome, key)?;
    if !o.absent() {
        return Ok(match o.get("status").code_units().as_str() {
            "persisted" => FieldState::SettledPersisted,
            "noop" => FieldState::SettledNoop,
            "not_persisted" => FieldState::SettledNotPersisted,
            _ => FieldState::ReconciliationRequired,
        });
    }
    Ok(if store.get(Kind::Execution, key)?.absent() {
        FieldState::Available
    } else {
        FieldState::DispatchUnknown
    })
}
pub struct Execution {
    pub key: String,
    pub record: Js,
    pub state: FieldState,
}
impl Execution {
    pub fn value(&self) -> Js {
        obj(&[
            ("key", self.key.clone().into()),
            ("record", self.record.clone()),
            ("state", self.state.name().into()),
        ])
    }
}
pub fn student_executions(store: &Store, scope: &Js) -> Result<Vec<Execution>> {
    let mut out = vec![];
    for key in store.ledger(&lock_key(scope)?)? {
        let record = store.get(Kind::Execution, &key)?;
        // Match the TS reference: ledger publication precedes the execution
        // fence. A crash between them never authorized dispatch, so skip the
        // index-only entry and permit a fresh save. Keep exact Unicode keys.
        if record.absent() {
            continue;
        }
        let state = field_state(store, &key)?;
        out.push(Execution { key, record, state });
    }
    Ok(out)
}
pub fn unresolved_on_field(
    store: &Store,
    scope: &Js,
    field: &str,
    record: impl Into<Js>,
) -> Result<Vec<Execution>> {
    let record = record.into();
    Ok(student_executions(store, scope)?
        .into_iter()
        .filter(|e| {
            e.record.get("field").is(field)
                && e.record.get("effectRecordKey") == &record
                && e.state.needs_reconciliation()
        })
        .collect())
}
pub fn attribution_summary(a: &Js) -> Js {
    obj(&[
        ("observedBy", a.get("observedBy").clone()),
        ("client", super::nullable(a.get("client").get("name"))),
        ("pid", a.get("process").get("pid").clone()),
        (
            "parentCommand",
            a.get("process").get("parentCommand").clone(),
        ),
        ("herdrPaneId", a.get("herdrPaneId").clone()),
        (
            "declaredRuntime",
            super::nullable(a.get("declared").get("runtime")),
        ),
        (
            "declaredSession",
            super::nullable(a.get("declared").get("sessionRef")),
        ),
    ])
}
