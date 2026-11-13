//! Shared synthetic acceptance cases for file and owner-supplied attempt ledgers.
use super::{
    Runtime, codec,
    ledger::{Attempt, AttemptLedger, AttemptTarget},
    state::{self, FieldState},
    store::{Kind, Store, mkdir},
};
use crate::js::{Fault, Js, Result, obj, sha};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Default)]
pub struct MemoryLedger(Mutex<BTreeMap<String, (AttemptTarget, Attempt)>>);
fn matches(target: &AttemptTarget, prior: &AttemptTarget, attempt: &Attempt) -> bool {
    attempt.state.needs_reconciliation()
        && target.group == prior.group
        && target.field == prior.field
        && (target.record_key == prior.record_key || target.occurrence == prior.occurrence)
}
impl AttemptLedger for MemoryLedger {
    fn begin(
        &self,
        key: &str,
        target: &AttemptTarget,
        digest: &str,
        _: &dyn Runtime,
    ) -> Result<bool> {
        let mut entries = self
            .0
            .lock()
            .map_err(|_| Fault::error("workflow_ledger_unavailable"))?;
        if entries.contains_key(key) {
            return Ok(false);
        }
        if entries.values().any(|(t, a)| matches(target, t, a)) {
            return Err(Fault::error("workflow_attempt_fenced"));
        }
        entries.insert(
            key.into(),
            (
                target.clone(),
                Attempt {
                    key: key.into(),
                    state: FieldState::DispatchUnknown,
                    execution: digest.into(),
                    outcome: None,
                    reconciliation: None,
                },
            ),
        );
        Ok(true)
    }
    fn record_outcome(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        _: &dyn Runtime,
    ) -> Result<bool> {
        let mut entries = self
            .0
            .lock()
            .map_err(|_| Fault::error("workflow_ledger_unavailable"))?;
        let (_, a) = entries
            .get_mut(key)
            .ok_or_else(|| Fault::error("workflow_ledger_unavailable"))?;
        if !a.state.needs_reconciliation() || a.outcome.is_some() {
            return Ok(false);
        }
        a.outcome = Some(digest.into());
        a.state = state;
        Ok(true)
    }
    fn record_reconciliation(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        _: &dyn Runtime,
    ) -> Result<bool> {
        let mut entries = self
            .0
            .lock()
            .map_err(|_| Fault::error("workflow_ledger_unavailable"))?;
        let (_, a) = entries
            .get_mut(key)
            .ok_or_else(|| Fault::error("workflow_ledger_unavailable"))?;
        if !a.state.needs_reconciliation() || a.reconciliation.is_some() {
            return Ok(false);
        }
        if a.outcome.is_none()
            || !matches!(
                state,
                FieldState::ReconciledPersisted
                    | FieldState::ReconciledNotPersisted
                    | FieldState::ReconciledConflict
            )
        {
            return Err(Fault::error("workflow_ledger_unavailable"));
        }
        a.reconciliation = Some(digest.into());
        a.state = state;
        Ok(true)
    }
    fn attempt(&self, key: &str) -> Result<Option<Attempt>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Fault::error("workflow_ledger_unavailable"))?
            .get(key)
            .map(|(_, a)| a.clone()))
    }
    fn attempts(&self, group: &str, limit: usize) -> Result<Vec<Attempt>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Fault::error("workflow_ledger_unavailable"))?
            .values()
            .filter(|(t, _)| t.group == group)
            .take(limit)
            .map(|(_, a)| a.clone())
            .collect())
    }
    fn open_on(&self, target: &AttemptTarget) -> Result<Vec<Attempt>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Fault::error("workflow_ledger_unavailable"))?
            .values()
            .filter(|(t, a)| matches(target, t, a))
            .map(|(_, a)| a.clone())
            .collect())
    }
}
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        "2040-01-04T12:00:00.000Z".into()
    }
    fn epoch_ms(&self) -> u64 {
        2209291200000
    }
    fn uuid(&self) -> Result<String> {
        Ok(format!(
            "ledger-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ))
    }
}
pub fn root(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var("S10_TEST_ROOT").expect("explicit private evidence root required"),
    )
    .join(format!(
        "ledger-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    mkdir(&root).unwrap();
    root
}
pub fn js(v: serde_json::Value) -> Js {
    codec::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
pub fn scope(student: &str) -> Js {
    obj(&[
        ("sourceNamespace", "invented".into()),
        ("teacherId", "11".into()),
        ("studentId", student.into()),
        ("lessonDate", "2040-01-04".into()),
    ])
}
pub fn target(scope: &Js) -> Js {
    super::merge(scope.clone(), &obj(&[("courseId", "13".into())]))
}
// Same obj/js/Clock/root construction as tests/workflow.rs and settlement_rule_b.rs.
pub fn execution(
    store: &Store,
    scope: &Js,
    field: &str,
    record: Js,
    occurrence: &Js,
    review_id: &str,
) -> Js {
    store
        .publish(
            Kind::Review,
            review_id,
            &obj(&[(
                "patch",
                Js::Arr(vec![obj(&[
                    ("field", field.into()),
                    ("target", occurrence.clone()),
                    ("before", "- Before".into()),
                    ("after", "- Sent".into()),
                ])]),
            )]),
            &Clock,
        )
        .unwrap();
    obj(&[
        ("scope", scope.clone()),
        ("field", field.into()),
        ("effectRecordKey", record),
        ("requestRef", "first".into()),
        ("reviewDigest", review_id.into()),
        ("effectTarget", "invented-target".into()),
        ("actionDigest", sha("invented action").into()),
        ("settlementRule", state::SETTLEMENT_RULE.into()),
    ])
}
pub fn outcome(persisted: bool) -> Js {
    js(
        serde_json::json!({"settlementRule":state::SETTLEMENT_RULE,"actionDigest":sha("invented action"),"recordedAt":"2040-01-04T12:00:00.000Z","transport":"returned","readbackStatus":if persisted {"persisted"} else {"unresolved"},"receipt":{"httpStatus":204,"bodySha256":sha("invented response"),"actionDigest":sha("invented action")}}),
    )
}
pub fn reconciliation(value: Js) -> Js {
    obj(&[
        ("settlementRule", state::SETTLEMENT_RULE.into()),
        ("actionDigest", sha("invented action").into()),
        ("readAt", "2040-01-04T12:10:00.000Z".into()),
        ("current", value),
    ])
}
fn publish_execution(store: &Store, key: &str, record: &Js) -> Result<(bool, Js)> {
    store.add_ledger(&state::lock_key(record.get("scope"))?, key, &Clock)?;
    store.publish(Kind::Execution, key, record, &Clock)
}
/// Cases shared by both stores: create once, state transitions, immutable
/// outcome/reconciliation, key ordering and unresolved target selection.
pub fn run(factory: &dyn Fn(&Path, &Path) -> Result<Store>) {
    let root = root("shared");
    let store = factory(&root.join("store"), &root).unwrap();
    let scope = scope("12");
    let target = target(&scope);
    let r = execution(&store, &scope, "progress", "15".into(), &target, "review");
    assert!(publish_execution(&store, "z", &r).unwrap().0);
    let changed = super::merge(r.clone(), &obj(&[("requestRef", "other".into())]));
    assert_eq!(
        publish_execution(&store, "z", &changed).unwrap(),
        (false, r.clone())
    );
    assert_eq!(
        state::field_state(&store, "z").unwrap(),
        FieldState::DispatchUnknown
    );
    assert_eq!(
        state::unresolved_on_field(&store, &scope, "progress", "15", &target)
            .unwrap()
            .len(),
        1
    );
    let out = outcome(true);
    assert!(store.publish(Kind::Outcome, "z", &out, &Clock).unwrap().0);
    assert_eq!(
        store
            .publish(Kind::Outcome, "z", &outcome(false), &Clock)
            .unwrap(),
        (false, out)
    );
    assert_eq!(
        state::field_state(&store, "z").unwrap(),
        FieldState::SettledPersisted
    );
    assert!(
        state::unresolved_on_field(&store, &scope, "progress", "15", &target)
            .unwrap()
            .is_empty()
    );
    assert!(publish_execution(&store, "\u{e000}", &r).unwrap().0);
    assert!(
        store
            .publish(Kind::Outcome, "\u{e000}", &outcome(false), &Clock)
            .unwrap()
            .0
    );
    assert_eq!(
        state::field_state(&store, "\u{e000}").unwrap(),
        FieldState::ReconciliationRequired
    );
    assert_eq!(
        store
            .publish(Kind::Outcome, "\u{e000}", &outcome(true), &Clock)
            .unwrap(),
        (false, outcome(false))
    );

    let rec = reconciliation("- Before".into());
    assert!(
        store
            .publish(Kind::Reconciliation, "\u{e000}", &rec, &Clock)
            .unwrap()
            .0
    );
    assert_eq!(
        store
            .publish(
                Kind::Reconciliation,
                "\u{e000}",
                &reconciliation("- Other".into()),
                &Clock
            )
            .unwrap(),
        (false, rec)
    );
    assert_eq!(
        state::field_state(&store, "\u{e000}").unwrap(),
        FieldState::ReconciledNotPersisted
    );
    assert!(publish_execution(&store, "\u{10000}", &r).unwrap().0);
    let executions = state::student_executions(&store, &scope).unwrap();
    assert_eq!(
        executions
            .iter()
            .map(|e| e.key.as_str())
            .collect::<Vec<_>>(),
        ["z", "\u{10000}", "\u{e000}"]
    );
    assert_eq!(
        executions.iter().map(|e| e.state).collect::<Vec<_>>(),
        [
            FieldState::SettledPersisted,
            FieldState::DispatchUnknown,
            FieldState::ReconciledNotPersisted
        ]
    );
    let pending = state::unresolved_on_field(&store, &scope, "progress", "15", &target).unwrap();
    assert_eq!(
        pending.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(),
        ["\u{10000}"]
    );
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
/// Store-level atomic fencing exists only with an AttemptLedger. File stores
/// enforce the fence in Service, so they intentionally do not run these cases.
pub fn run_ledgered(factory: &dyn Fn(&Path, &Path) -> Result<Store>) {
    let root = root("fences");
    let store = factory(&root.join("store"), &root).unwrap();
    let scope = scope("12");
    let target = target(&scope);
    let r = execution(&store, &scope, "progress", "15".into(), &target, "review");
    assert!(publish_execution(&store, "first", &r).unwrap().0);
    let mut different = target.clone();
    different.set("courseId", "another".into());
    let same_key = execution(
        &store,
        &scope,
        "progress",
        "15".into(),
        &different,
        "different-occurrence",
    );
    let same_occurrence = execution(
        &store,
        &scope,
        "progress",
        "other-record".into(),
        &target,
        "same-occurrence",
    );
    for r in [&same_key, &same_occurrence] {
        assert_eq!(
            publish_execution(&store, "blocked", r).unwrap_err().message,
            "workflow_attempt_fenced"
        );
    }
    let other_field = execution(
        &store,
        &scope,
        "homework",
        "15".into(),
        &target,
        "other-field",
    );
    assert!(
        publish_execution(&store, "other-field", &other_field)
            .unwrap()
            .0
    );
    let other_scope = self::scope("99");
    let other_student = execution(
        &store,
        &other_scope,
        "progress",
        "15".into(),
        &target,
        "other-student",
    );
    assert!(
        publish_execution(&store, "other-student", &other_student)
            .unwrap()
            .0
    );
    assert_eq!(
        state::unresolved_on_field(&store, &scope, "progress", "15", &target)
            .unwrap()
            .iter()
            .map(|e| e.key.as_str())
            .collect::<Vec<_>>(),
        ["first"]
    );
    store
        .publish(Kind::Outcome, "first", &outcome(false), &Clock)
        .unwrap();
    assert_eq!(
        publish_execution(&store, "blocked", &same_occurrence)
            .unwrap_err()
            .message,
        "workflow_attempt_fenced"
    );
    store
        .publish(
            Kind::Reconciliation,
            "first",
            &reconciliation("- Sent".into()),
            &Clock,
        )
        .unwrap();
    assert!(
        publish_execution(&store, "freed", &same_occurrence)
            .unwrap()
            .0
    );
    store
        .publish(Kind::Outcome, "freed", &outcome(true), &Clock)
        .unwrap();
    assert!(publish_execution(&store, "freed-again", &r).unwrap().0);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
