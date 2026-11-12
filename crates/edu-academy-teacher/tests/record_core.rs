//! Invented borrowed-port integration; core remains the only settlement owner.
use edu_academy_core::{
    js::{Js, Result, obj},
    workflow::{
        Runtime, codec,
        service::Source,
        state,
        store::{Kind, Store},
    },
};
use edu_academy_teacher::{
    facts::SeededFacts,
    record_backend::{EffectOutcome, EffectScript, SyntheticRecordState},
    record_core::{ClockedHeldSource, CoreAdapter},
    record_data::RecordField,
    record_effect::{
        HeldBackend, SyntheticCoreBackend, SyntheticCoreSource, canonical_effect_target,
    },
    record_fixture::RecordFixture,
    record_policy::RecordPolicy,
    record_store::DraftStore,
};
use std::{
    cell::Cell,
    path::PathBuf,
    sync::{Arc, Mutex},
};

struct Clock {
    at: Cell<&'static str>,
    sequence: Cell<u64>,
}
impl Runtime for Clock {
    fn now(&self) -> String {
        self.at.get().into()
    }
    fn epoch_ms(&self) -> u64 {
        0
    }
    fn uuid(&self) -> Result<String> {
        self.sequence.set(self.sequence.get() + 1);
        Ok(format!("invented-wrapper-{}", self.sequence.get()))
    }
}
fn js(value: serde_json::Value) -> Js {
    codec::parse(&serde_json::to_vec(&value).unwrap()).unwrap()
}
struct Case {
    root: PathBuf,
    fixture: RecordFixture,
    source: SyntheticCoreSource,
    backend: SyntheticCoreBackend,
    store: Store,
    drafts: DraftStore,
    policy: RecordPolicy,
    clock: Clock,
}
impl Case {
    fn new(name: &str, field: RecordField, outcome: EffectOutcome) -> Self {
        let mut fixture =
            RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
        let record = &fixture.records[0];
        fixture.seed.effect_scripts = vec![EffectScript {
            identity: record.snapshot.identity.clone(),
            field,
            outcome,
            readback: None,
        }];
        let root = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
            .join(format!("wrapper-{name}-{}", std::process::id()));
        let state = Arc::new(Mutex::new(
            SyntheticRecordState::open(&root, fixture.seed.clone(), 10000).unwrap(),
        ));
        let drafts = DraftStore::open(&root, fixture.seed.owner.clone()).unwrap();
        let store = Store::open(&root, &root).unwrap();
        let policy = RecordPolicy::new(
            SeededFacts::default(),
            js(serde_json::json!({
                "books":[{"studentRef":record.snapshot.identity.student_key}]
            })),
        );
        Self {
            source: SyntheticCoreSource {
                state: Arc::clone(&state),
                records: vec![record.clone()],
            },
            backend: SyntheticCoreBackend { state },
            store,
            drafts,
            root,
            fixture,
            policy,
            clock: Clock {
                at: Cell::new("2026-11-13T09:00:00.000Z"),
                sequence: Cell::new(0),
            },
        }
    }
    fn scope(&self) -> Js {
        self.fixture.records[0].answer.get("scope").clone()
    }
    fn run(&mut self, call: impl FnOnce(&mut CoreAdapter<'_>) -> Result<Js>) -> Result<Js> {
        let account = self.drafts.lock_account()?;
        let _draft = account.lock_draft(&self.fixture.records[0].snapshot.identity)?;
        let mut source = ClockedHeldSource {
            inner: &mut self.source,
            account: &account,
            runtime: &self.clock,
        };
        let mut backend = HeldBackend {
            inner: &mut self.backend,
            account: &account,
        };
        call(&mut CoreAdapter::new(
            &self.store,
            &mut source,
            &mut backend,
            &self.policy,
            &self.clock,
        ))
    }
    fn read(&mut self) -> Js {
        let scope = self.scope();
        let account = self.drafts.lock_account().unwrap();
        let _draft = account
            .lock_draft(&self.fixture.records[0].snapshot.identity)
            .unwrap();
        ClockedHeldSource {
            inner: &mut self.source,
            account: &account,
            runtime: &self.clock,
        }
        .read(&scope, &Js::Null, &Js::Null)
        .unwrap()
        .get("observation")
        .clone()
    }
    fn prepare(&mut self, field: RecordField) -> Js {
        let value = if field == RecordField::Attendance {
            "N"
        } else {
            "2"
        };
        assert!(
            !self.fixture.records[0].field_evidence[field]
                .get("before")
                .get("value")
                .is(value)
        );
        let context = js(serde_json::json!({
            "schemaVersion":"academy.dailyrecord-user-context/1", "purpose":"Invented wrapper test",
            "currentRequestRef":"invented-wrapper-request", "scope":serde_json::from_slice::<serde_json::Value>(&codec::bytes(&self.scope()).unwrap()).unwrap(),
            "selectedFields":[field.wire_name()], "memoAnswer":{"kind":"no_addition","requestRef":"invented-wrapper-request"},
            "facts":[{"ref":"invented-option", "field":field.wire_name(), "text":"Invented explicit option", "origin":"user_supplied",
                "explicitValue":value,"requiredFragments":[],"questionCounts":[],"deadlines":[],"stageClaims":[]}]
        }));
        let intake = self
            .run(|c| c.intake(&obj(&[("context", context.clone())])))
            .unwrap();
        assert!(intake.get("observationHandle").string());
        let mut input = obj(&[
            ("context", context),
            ("observationHandle", intake.get("observationHandle").clone()),
            (
                "proposals",
                js(
                    serde_json::json!([{"field":field.wire_name(),"value":value,"factRefs":["invented-option"],
                "decision":{"kind":"replace","requestRef":"invented-wrapper-request"}}]),
                ),
            ),
        ]);
        let review = self.run(|c| c.review(&input, true)).unwrap();
        assert!(
            review.get("status").is("prepared"),
            "review must prepare the invented option"
        );
        let target = self.fixture.records[0].field_evidence[field].get("target");
        let binding = canonical_effect_target(target).unwrap().into();
        assert_eq!(review.get("patch").array()[0].get("effectTarget"), &binding);
        input.set("reviewDigest", review.get("reviewDigest").clone());
        input.set("request", js(serde_json::json!({"requestRef":"invented-wrapper-request", "instruction":{"text":"Invented explicit save instruction"}})));
        input
    }
    fn reconcile(&mut self) -> Js {
        let mut context = js(serde_json::json!({
            "schemaVersion":"academy.dailyrecord-user-context/1", "purpose":"Invented recovery",
            "currentRequestRef":"invented-recovery", "selectedFields":["attendance"],
            "memoAnswer":{"kind":"no_addition","requestRef":"invented-recovery"}, "facts":[]
        }));
        context.set("scope", self.scope());
        self.run(|c| c.reconcile(&obj(&[("context", context)])))
            .unwrap()
    }
    fn execution(&self) -> state::Execution {
        let mut executions = state::student_executions(&self.store, &self.scope()).unwrap();
        assert_eq!(executions.len(), 1);
        executions.remove(0)
    }
}

#[test]
fn core_service_reuses_durable_execution() {
    let mut c = Case::new(
        "reopen",
        RecordField::PreviousHomeworkAssessment,
        EffectOutcome::AppliedLostAck,
    );
    let input = c.prepare(RecordField::PreviousHomeworkAssessment);
    let saved = c.run(|core| core.save(&input)).unwrap();
    assert!(saved.get("status").is("held"));
    let execution = c.execution();
    let outcome = c.store.get(Kind::Outcome, &execution.key).unwrap();
    assert!(execution.state.needs_reconciliation());
    // Reopen every durable port after releasing the caller-owned locks.
    c.store = Store::open(&c.root, &c.root).unwrap();
    c.drafts = DraftStore::open(&c.root, c.fixture.seed.owner.clone()).unwrap();
    let reopened = Arc::new(Mutex::new(
        SyntheticRecordState::open(&c.root, c.fixture.seed.clone(), 10000).unwrap(),
    ));
    c.source.state = Arc::clone(&reopened);
    c.backend.state = reopened;
    let saved = c.run(|core| core.save(&input)).unwrap();
    assert!(saved.get("status").is("held"));
    let scope = c.scope();
    let status = c
        .run(|core| core.status(&obj(&[("scope", scope)])))
        .unwrap();
    assert!(
        status.get("fields").array()[0].get("executions").array()[0]
            .get("reconciliation")
            .is("manual_reconciliation")
    );
    let reads = c.source.state.lock().unwrap().read_count();
    let pending = c.reconcile();
    assert!(pending.get("reasonCode").is("save_confirmation_pending"));
    assert_eq!(c.source.state.lock().unwrap().read_count(), reads);
    c.clock.at.set("2026-11-13T09:10:00.000Z");
    assert!(c.reconcile().get("status").is("settled"));
    assert_eq!(c.execution().key, execution.key);
    assert_eq!(c.execution().record, execution.record);
    assert_eq!(c.execution().state, state::FieldState::ReconciledPersisted);
    assert_eq!(c.store.get(Kind::Outcome, &execution.key).unwrap(), outcome);
    assert_eq!(
        c.source.state.lock().unwrap().dispatch_count(),
        0,
        "reopened state must never redispatch"
    );
}

#[test]
fn core_mapping_preserves_rule_b() {
    for (name, outcome, expected) in [
        (
            "success",
            EffectOutcome::Success,
            state::FieldState::ReconciledPersisted,
        ),
        (
            "unapplied",
            EffectOutcome::UnappliedLostAck,
            state::FieldState::ReconciledNotPersisted,
        ),
    ] {
        let mut c = Case::new(name, RecordField::Attendance, outcome);
        let before = c.read();
        let input = c.prepare(RecordField::Attendance);
        let saved = c.run(|core| core.save(&input)).unwrap();
        assert!(
            saved.get("status").is("held"),
            "receipt-less synthetic result cannot settle early"
        );
        assert!(c.execution().state.needs_reconciliation());
        c.clock.at.set("2026-11-13T09:09:59.000Z");
        let reads = c.source.state.lock().unwrap().read_count();
        let pending = c.reconcile();
        assert!(pending.get("reasonCode").is("save_confirmation_pending"));
        assert_eq!(c.source.state.lock().unwrap().read_count(), reads);
        c.clock.at.set("2026-11-13T09:10:00.000Z");
        assert!(c.reconcile().get("status").is("settled"));
        assert_eq!(c.execution().state, expected);
        assert_eq!(c.source.state.lock().unwrap().dispatch_count(), 1);
        let after = c.read();
        for field in RecordField::ALL {
            if field != RecordField::Attendance {
                assert_eq!(
                    before.get("fields").get(field.wire_name()).get("value"),
                    after.get("fields").get(field.wire_name()).get("value")
                );
            }
        }
    }
}

#[test]
fn held_read_uses_runtime_time_and_fresh_evidence() {
    let mut c = Case::new("read-time", RecordField::Attendance, EffectOutcome::Success);
    let capture = c.fixture.records[0].clone();
    let first = c.read();
    assert_ne!(capture.snapshot.provenance.read_at, c.clock.now());
    assert!(first.get("observedAt").is(&c.clock.now()));
    let input = c.prepare(RecordField::Attendance);
    c.run(|core| core.save(&input)).unwrap();
    c.clock.at.set("2026-11-13T09:00:01.000Z");
    let current = c.read();
    assert!(current.get("observedAt").is(&c.clock.now()));
    assert!(current.get("fields").get("attendance").get("value").is("N"));
    for field in RecordField::ALL {
        let value = current.get("fields").get(field.wire_name());
        let old = first.get("fields").get(field.wire_name());
        let capture_refs = capture.field_evidence[field]
            .get("before")
            .get("evidenceRefs");
        assert_eq!(value.get("effectTarget"), old.get("effectTarget"));
        assert_ne!(value.get("evidenceRefs"), capture_refs);
        assert!(
            value
                .get("evidenceRefs")
                .array()
                .iter()
                .all(|r| r.code_units().contains(&c.clock.now())),
            "fresh_evidence_must_use_read_time"
        );
    }
    assert!(
        c.execution().state.needs_reconciliation(),
        "fresh early equality is not settlement"
    );
}

#[test]
fn account_busy_before_fence() {
    let mut c = Case::new("busy", RecordField::Attendance, EffectOutcome::Success);
    let competitor = DraftStore::open(&c.root, c.fixture.seed.owner.clone()).unwrap();
    let busy = competitor.lock_account().unwrap();
    let entered = Cell::new(false);
    let result = c.run(|core| {
        entered.set(true);
        core.save(&obj(&[]))
    });
    assert_eq!(result.unwrap_err().message, "shared_writer_lock_busy");
    assert!(!entered.get());
    assert!(
        state::student_executions(&c.store, &c.scope())
            .unwrap()
            .is_empty()
    );
    assert_eq!(c.source.state.lock().unwrap().dispatch_count(), 0);
    assert_eq!(c.source.state.lock().unwrap().read_count(), 0);
    drop(busy);
}
