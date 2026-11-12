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
    effect_target: &Js,
) -> Result<String> {
    super::schema::effect_target(effect_target)?;
    canonical(&obj(&[
        ("requestRef", request.into()),
        ("student", lock_key(scope)?.into()),
        ("field", field.into()),
        ("effectRecordKey", record.into()),
        ("effectTarget", effect_target.clone()),
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
        transition.get("effectTarget"),
    )?;
    let before_sha = sha(&String::from_utf16_lossy(before.units()));
    let after_sha = sha(&String::from_utf16_lossy(after.units()));
    for prior in student_executions(store, scope)? {
        let r = &prior.record;
        if r.get("field").is(&field)
            && r.get("effectTarget").absent()
            && (prior.state.needs_reconciliation() || r.get("requestRef") == request)
        {
            return Err(Fault::error("workflow_legacy_execution_unbound"));
        }
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
pub const SETTLEMENT_RULE: &str = "academy.rule-b/1";
// Four measured live saves showed no change later than 10 minutes;
// change this window only on an observed late save.
pub const SETTLEMENT_WINDOW_SECONDS: i64 = 600;
pub fn settlement_at(execution: &Js, outcome: &Js) -> Option<i64> {
    if !execution.get("settlementRule").is(SETTLEMENT_RULE)
        || !outcome.get("settlementRule").is(SETTLEMENT_RULE)
        || outcome.get("actionDigest") != execution.get("actionDigest")
    {
        return None;
    }
    crate::instant::epoch_ms(
        outcome.get("recordedAt"),
        crate::instant::Grammar::DailyRecord,
    )
    .map(|at| at + SETTLEMENT_WINDOW_SECONDS * 1000)
}
pub fn returned_receipt(record: &Js) -> bool {
    record.get("transport").is("returned")
        && matches!(record.get("receipt").get("httpStatus"), Js::Num(n) if n.fract() == 0.0 && (200.0..300.0).contains(n))
        && crate::js::test("^[a-f0-9]{64}$", record.get("receipt").get("bodySha256"))
}
pub fn field_state(store: &Store, key: &str) -> Result<FieldState> {
    let execution = store.get(Kind::Execution, key)?;
    let outcome = store.get(Kind::Outcome, key)?;
    if let Some(due) = settlement_at(&execution, &outcome) {
        let reconciliation = store.get(Kind::Reconciliation, key)?;
        let marked = |r: &Js| {
            r.get("settlementRule").is(SETTLEMENT_RULE)
                && r.get("actionDigest") == execution.get("actionDigest")
                && crate::js::test("^[a-f0-9]{64}$", execution.get("actionDigest"))
        };
        if marked(&reconciliation)
            && crate::instant::epoch_ms(
                reconciliation.get("readAt"),
                crate::instant::Grammar::DailyRecord,
            )
            .is_some_and(|at| at >= due)
            && reconciliation.get("current").string()
        {
            let review = store.get(Kind::Review, &execution.get("reviewDigest").code_units())?;
            let patch = review
                .get("patch")
                .array()
                .iter()
                .find(|p| {
                    p.get("field") == execution.get("field")
                        && p.get("before").string()
                        && p.get("after").string()
                })
                .ok_or_else(|| Fault::error("workflow_execution_review_target_unavailable"))?;
            return Ok(if reconciliation.get("current") == patch.get("after") {
                FieldState::ReconciledPersisted
            } else if reconciliation.get("current") == patch.get("before") {
                FieldState::ReconciledNotPersisted
            } else {
                FieldState::ReconciledConflict
            });
        }
        if marked(&outcome)
            && returned_receipt(&outcome)
            && outcome.get("receipt").get("actionDigest") == execution.get("actionDigest")
            && outcome.get("readbackStatus").is("persisted")
        {
            return Ok(FieldState::SettledPersisted);
        }
    }
    Ok(if store.get(Kind::Execution, key)?.absent() {
        if store.get(Kind::Outcome, key)?.absent() && store.get(Kind::Reconciliation, key)?.absent()
        {
            FieldState::Available
        } else {
            FieldState::ReconciliationRequired
        }
    } else if store.get(Kind::Outcome, key)?.absent() {
        FieldState::DispatchUnknown
    } else {
        FieldState::ReconciliationRequired
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
            if !store.get(Kind::Outcome, &key)?.absent()
                || !store.get(Kind::Reconciliation, &key)?.absent()
            {
                return Err(Fault::error("workflow_execution_missing_with_evidence"));
            }
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
    target: &Js,
) -> Result<Vec<Execution>> {
    let record = record.into();
    let mut pending = vec![];
    for execution in student_executions(store, scope)? {
        if !execution.record.get("field").is(field) || !execution.state.needs_reconciliation() {
            continue;
        }
        let missing = || Fault::error("workflow_execution_review_target_unavailable");
        let digest = execution.record.get("reviewDigest");
        if !digest.string() {
            return Err(missing());
        }
        let review = store.get(Kind::Review, &digest.code_units())?;
        let prior = review
            .get("patch")
            .array()
            .iter()
            .find(|patch| patch.get("field").is(field))
            .ok_or_else(missing)?;
        let occurrence = ["sourceNamespace", "studentId", "lessonDate", "courseId"];
        let previous = prior.get("target");
        if occurrence.iter().any(|key| previous.get(key).absent()) {
            return Err(missing());
        }
        if execution.record.get("effectRecordKey") == &record
            || occurrence
                .iter()
                .all(|key| previous.get(key) == target.get(key))
        {
            pending.push(execution);
        }
    }
    Ok(pending)
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
