//! Invented Service scenarios; no native endpoint or private payload.
use edu_academy_core::{
    js::{Js, Result, obj, sha},
    workflow::{
        Runtime,
        approval::Approval,
        canonical, codec, http, native, review, service, state,
        store::{Kind, Store, mkdir},
    },
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
fn js(v: serde_json::Value) -> Js {
    codec::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
const NOW: &str = "2040-01-04T12:00:00.000Z";
const DUE: f64 = 2209291800000.0;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Clock {
    now: Rc<RefCell<String>>,
    seq: Cell<u64>,
}
impl Runtime for Clock {
    fn now(&self) -> String {
        self.now.borrow().clone()
    }
    fn epoch_ms(&self) -> u64 {
        0
    }
    fn uuid(&self) -> Result<String> {
        self.seq.set(self.seq.get() + 1);
        Ok(format!("00000000-0000-4000-8000-{:012}", self.seq.get()))
    }
}
struct Policy;
impl review::Policy for Policy {
    fn parent_pins(&self) -> Js {
        js(
            json!({"sourceRef":"invented", "policySha256":"3".repeat(64), "safeguardSourceSha256":"4".repeat(64)}),
        )
    }
    fn learning_pins(&self) -> Js {
        js(
            json!({"bundleSha256":"5".repeat(64),"configSha256":"6".repeat(64),"catalogSha256":"7".repeat(64)}),
        )
    }
    fn check_lines(&self, _: &Js) -> bool {
        true
    }
    fn excluded_parent_language(&self, _: &Js) -> bool {
        false
    }
    fn scope_qualified(&self, _: &Js) -> bool {
        true
    }
    fn learning_review(&self, _: &Js, _: &Js, _: &Js) -> Vec<Js> {
        vec![]
    }
}
struct Source(Rc<RefCell<Js>>, Rc<Cell<usize>>);
impl service::Source for Source {
    fn read(&mut self, _: &Js, _: &Js, _: &Js) -> Result<Js> {
        self.1.set(self.1.get() + 1);
        Ok(obj(&[
            ("status", "observed".into()),
            ("observation", self.0.borrow().clone()),
            (
                "subject",
                js(json!({"displayName":"Invented Rule Student"})),
            ),
        ]))
    }
}
struct Backend {
    clock: Rc<RefCell<String>>,
    return_at: Option<&'static str>,
    calls: usize,
    status: &'static str,
    transport: &'static str,
    receipt: Option<u16>,
    application: Option<String>,
    observation: Rc<RefCell<Js>>,
}
impl service::Backend for Backend {
    fn configured(&self) -> bool {
        true
    }
    fn save(
        &mut self,
        scope: &Js,
        patches: &Js,
        approval: &mut dyn Approval,
        _: &Js,
    ) -> Result<Js> {
        let p = &patches.array()[0];
        let action = obj(&[
            (
                "target",
                obj(&[
                    ("studentKey", scope.get("studentId").clone()),
                    ("teacherKey", scope.get("teacherId").clone()),
                    ("date", scope.get("lessonDate").clone()),
                ]),
            ),
            ("attemptId", "invented-attempt".into()),
            ("field", p.get("field").clone()),
            ("effectTarget", p.get("effectTarget").clone()),
            ("effectRecordKey", p.get("effectRecordKey").clone()),
            ("before", p.get("before").clone()),
            ("after", p.get("value").clone()),
        ]);
        assert!(approval.consume(&action)?.get("status").is("verified"));
        self.calls += 1;
        assert_ne!(
            self.status, "crash",
            "invented process ended after consumption"
        );
        if let Some(at) = self.return_at {
            *self.clock.borrow_mut() = at.into();
        }
        if let Some(value) = &self.application {
            let mut observation = self.observation.borrow_mut();
            let mut fields = observation.get("fields").clone();
            let mut field = fields.get("progress").clone();
            field.set("value", value.as_str().into());
            fields.set("progress", field);
            observation.set("fields", fields);
        }
        let receipt = self.receipt.map_or(Js::Null, |n| js(json!({"httpStatus":n,
            "bodySha256":sha("invented response"), "actionDigest":sha(&canonical(&action).unwrap())})));
        Ok(Js::Arr(vec![obj(&[
            ("field", p.get("field").clone()),
            ("status", self.status.into()),
            ("transport", self.transport.into()),
            ("receipt", receipt),
            ("dispatched", true.into()),
        ])]))
    }
}
struct Case {
    root: PathBuf,
    source: Source,
    backend: Backend,
    clock: Clock,
    input: Js,
}
impl Case {
    fn new(status: &'static str, transport: &'static str, receipt: Option<u16>) -> Self {
        let root = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap()).join(format!(
            "rule-b-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        mkdir(&root).unwrap();
        let scope = js(
            json!({"sourceNamespace":"academy.lms.dayrecord","teacherId":"11",
            "studentId":"12","lessonDate":"2040-01-04"}),
        );
        let mut target = scope.clone();
        for (k, v) in [
            ("courseId", "13"),
            ("membershipId", "14"),
            ("recordId", "15"),
        ] {
            target.set(k, v.into());
        }
        let mut previous = target.clone();
        previous.set("lessonDate", "2040-01-03".into());
        previous.set("recordId", "16".into());
        let mut fields = obj(&[]);
        for field in review::FIELDS {
            let value = match field {
                "attendance" => "Y",
                "dailyTestAssessment" | "previousHomeworkAssessment" => "0",
                _ => "- Before",
            };
            fields.set(field,js(json!({"state":"known","value":value,"evidenceRefs":["invented-read"],
                "effectRecordKey":if field=="previousHomeworkAssessment" {"16"} else {"15"},
                "effectTarget":if field=="previousHomeworkAssessment" {"previous-target"} else {"target"}})));
        }
        let observation = obj(&[
            ("scope", scope),
            ("target", target),
            ("previousTarget", previous),
            ("observedAt", NOW.into()),
            ("sourceRevision", "1".repeat(64).into()),
            ("wireDigest", "2".repeat(64).into()),
            ("fields", fields),
            ("toneSamples", Js::Arr(vec![])),
            ("transmissionState", "not_sent".into()),
        ]);
        let observation = Rc::new(RefCell::new(observation));
        let clock = Rc::new(RefCell::new(NOW.into()));
        let mut case = Self {
            root,
            source: Source(observation.clone(), Rc::new(Cell::new(0))),
            backend: Backend {
                clock: clock.clone(),
                return_at: None,
                calls: 0,
                status,
                transport,
                receipt,
                application: None,
                observation,
            },
            clock: Clock {
                now: clock,
                seq: Cell::new(0),
            },
            input: Js::Null,
        };
        case.prepare("first", "- Sent");
        case
    }
    fn open(&self) -> Store {
        Store::open(&self.root.join("store"), &self.root).unwrap()
    }
    fn prepare(&mut self, request: &str, value: &str) {
        self.prepare_field(request, value, "progress");
    }
    fn prepare_field(&mut self, request: &str, value: &str, field: &str) {
        let store = self.open();
        let scope = self.source.0.borrow().get("scope").clone();
        self.source
            .0
            .borrow_mut()
            .set("observedAt", self.clock.now().into());
        let context = js(
            json!({"schemaVersion":"academy.dailyrecord-user-context/1", "purpose":"Invented rule test",
            "currentRequestRef":request, "scope":serde_json::from_str::<serde_json::Value>(&edu_academy_core::js::stringify(&scope).unwrap()).unwrap(),
            "selectedFields":[field], "memoAnswer":{"kind":"no_addition","requestRef":request},
            "facts":[{"ref":"fact","field":field,"text":value,"explicitValue":value,"requiredFragments":[],"questionCounts":[],"deadlines":[],"stageClaims":[]}]}),
        );
        let mut svc = service::Service {
            store: &store,
            source: &mut self.source,
            backend: &mut self.backend,
            policy: &Policy,
            runtime: &self.clock,
        };
        let intake = svc.intake(&obj(&[("context", context.clone())])).unwrap();
        let mut input = obj(&[
            ("context", context),
            ("observationHandle", intake.get("observationHandle").clone()),
            (
                "proposals",
                js(
                    json!([{"field":field,"value":value,"factRefs":["fact"],"decision":{"kind":"replace","requestRef":request}}]),
                ),
            ),
        ]);
        let review = svc.review(&input, true).unwrap();
        assert!(review.get("status").is("prepared"), "{review:?}");
        input.set("reviewDigest", review.get("reviewDigest").clone());
        input.set(
            "request",
            js(json!({"requestRef":request,"instruction":{"text":"Invented save instruction"}})),
        );
        self.input = input;
    }
    fn save(&mut self) -> Js {
        let store = self.open();
        service::Service {
            store: &store,
            source: &mut self.source,
            backend: &mut self.backend,
            policy: &Policy,
            runtime: &self.clock,
        }
        .save(&self.input)
        .unwrap()
    }
    fn at(&self, now: &str) {
        *self.clock.now.borrow_mut() = now.into();
    }
    fn value(&self, value: Option<&str>) {
        let mut o = self.source.0.borrow_mut();
        let mut fields = o.get("fields").clone();
        let mut f = fields.get("progress").clone();
        f.set(
            "state",
            if value.is_some() { "known" } else { "unknown" }.into(),
        );
        f.set("value", value.map_or(Js::Undefined, Into::into));
        if value.is_none() {
            f = obj(&[
                ("state", "unknown".into()),
                ("reasonCode", "invented_unknown".into()),
                ("effectTarget", f.get("effectTarget").clone()),
                ("effectRecordKey", f.get("effectRecordKey").clone()),
            ]);
        }
        fields.set("progress", f);
        o.set("fields", fields);
    }
    fn reconcile(&mut self, resolution: bool) -> Js {
        let store = self.open();
        let mut input = obj(&[("context", self.input.get("context").clone())]);
        if resolution {
            input.set("resolution",js(json!({"request":{"requestRef":"first","instruction":{"text":"It failed; release it."}},"executions":[{"requestRef":"first","field":"progress"}]})));
        }
        service::Service {
            store: &store,
            source: &mut self.source,
            backend: &mut self.backend,
            policy: &Policy,
            runtime: &self.clock,
        }
        .reconcile(&input)
        .unwrap()
    }
    fn execution(&self) -> state::Execution {
        state::student_executions(&self.open(), self.input.get("context").get("scope"))
            .unwrap()
            .remove(0)
    }
    fn no_settlement(&self) {
        assert!(
            self.open()
                .get(Kind::Reconciliation, &self.execution().key)
                .unwrap()
                .absent()
        );
    }
    fn pending(result: &Js) {
        assert_eq!(
            result.get("status"),
            &Js::from("held"),
            "confirmation must remain pending"
        );
        assert_eq!(
            result.get("reasonCode"),
            &Js::from("save_confirmation_pending")
        );
        assert_eq!(result.get("earliestSettleAtEpochMs"), &Js::Num(DUE));
    }
}
#[test]
fn t1_immediate_saved_admits_second_save() {
    let mut c = Case::new("persisted", "returned", Some(204));
    c.backend.application = Some("- Sent".into());
    assert!(c.save().get("status").is("saved"));
    let outcome = c.open().get(Kind::Outcome, &c.execution().key).unwrap();
    assert_eq!(outcome.get("receipt").get("httpStatus"), &Js::Num(204.0));
    assert!(
        outcome
            .get("receipt")
            .get("bodySha256")
            .is(&sha("invented response"))
    );
    assert_eq!(c.execution().state, state::FieldState::SettledPersisted);
    c.prepare("second", "- Next");
    c.backend.application = Some("- Next".into());
    assert!(c.save().get("status").is("saved"));
    assert_eq!(c.backend.calls, 2);
}
fn late(value: &str, state: state::FieldState, retry: bool) {
    let mut c = Case::new("persisted", "lost", None);
    c.backend.application = Some(value.into());
    assert!(c.save().get("status").is("held"));
    c.at("2040-01-04T12:09:59.999Z");
    let reads = c.source.1.get();
    Case::pending(&c.reconcile(false));
    assert_eq!(c.source.1.get(), reads);
    c.no_settlement();
    c.at("2040-01-04T12:10:00.000Z");
    let mut observation = c.source.0.borrow_mut();
    let mut fields = observation.get("fields").clone();
    let mut homework = fields.get("homework").clone();
    homework.set("value", "- Changed elsewhere".into());
    fields.set("homework", homework);
    observation.set("fields", fields);
    drop(observation);
    assert!(c.reconcile(false).get("status").is("settled"));
    assert_eq!(c.execution().state, state);
    assert_eq!(c.backend.calls, 1);
    if retry {
        c.prepare("second", "- Next");
        c.save();
        assert_eq!(c.backend.calls, 2);
    }
}
#[test]
fn t2_lost_answer_sent_value_waits_until_q() {
    late("- Sent", state::FieldState::ReconciledPersisted, true);
}
#[test]
fn t3_lost_answer_before_value_allows_new_dispatch() {
    late("- Before", state::FieldState::ReconciledNotPersisted, true);
}
#[test]
fn t4_lost_answer_other_value_conflicts() {
    late("- Other", state::FieldState::ReconciledConflict, false);
    let mut c = Case::new("persisted", "lost", None);
    c.prepare("unicode", "- \u{fffd}");
    c.backend.application = Some("- \u{fffd}".into());
    c.save();
    let mut observation = c.source.0.borrow_mut();
    let mut fields = observation.get("fields").clone();
    let mut progress = fields.get("progress").clone();
    progress.set("value", Js::Str(vec![45, 32, 0xd800]));
    fields.set("progress", progress);
    observation.set("fields", fields);
    drop(observation);
    c.at("2040-01-04T12:10:00.000Z");
    assert!(c.reconcile(false).get("status").is("settled"));
    assert_eq!(c.execution().state, state::FieldState::ReconciledConflict);
    assert_eq!(c.backend.calls, 1);
}
#[test]
fn t5_receipted_third_value_fences_second_dispatch_until_q() {
    let mut c = Case::new("conflict", "returned", Some(200));
    c.backend.application = Some("- Other".into());
    let first = c.save();
    c.prepare("second", "- Next");
    let second = c.save();
    assert_eq!(c.backend.calls, 1, "second dispatch admitted inside Q");
    Case::pending(&first);
    Case::pending(&second);
    c.at("2040-01-04T12:01:40.000Z");
    c.value(Some("- Sent")); // The first write may still apply inside Q.
    Case::pending(&c.save());
    assert_eq!(c.backend.calls, 1);
    c.value(Some("- Other"));
    assert!(c.execution().state.needs_reconciliation());
    c.at("2040-01-04T12:10:00.000Z");
    assert!(c.reconcile(false).get("status").is("settled"));
    assert_eq!(c.execution().state, state::FieldState::ReconciledConflict);
}
#[test]
fn t6_2xx_before_value_remains_held_until_q() {
    let mut c = Case::new("not_persisted", "returned", Some(200));
    c.save();
    assert!(c.execution().state.needs_reconciliation());
    Case::pending(&c.reconcile(false));
    c.no_settlement();
    c.at("2040-01-04T12:10:00.000Z");
    c.reconcile(false);
    assert_eq!(
        c.execution().state,
        state::FieldState::ReconciledNotPersisted
    );
    assert_eq!(c.backend.calls, 1);
}
#[test]
fn t7_second_save_in_window_names_code_and_time() {
    let mut c = Case::new("persisted", "lost", None);
    c.save();
    c.prepare("second", "- Next");
    Case::pending(&c.save());
    assert_eq!(c.backend.calls, 1);
    c.no_settlement();
}
#[test]
fn t8_reopening_store_preserves_all_three_q_results() {
    for (value, state) in [
        ("- Sent", state::FieldState::ReconciledPersisted),
        ("- Before", state::FieldState::ReconciledNotPersisted),
        ("- Other", state::FieldState::ReconciledConflict),
    ] {
        late(value, state, false);
    }
}
#[test]
fn t9_pre_rule_record_keeps_legacy_hold() {
    let mut c = Case::new("persisted", "lost", None);
    c.save();
    let old = c.open();
    let e = c.execution();
    let mut record = e.record.clone();
    record.set("settlementRule", Js::Null);
    record.set("dispatchedAt", Js::Null);
    c.root = c.root.join("legacy");
    mkdir(&c.root).unwrap();
    let store = c.open();
    store
        .add_ledger(
            &state::lock_key(record.get("scope")).unwrap(),
            &e.key,
            &c.clock,
        )
        .unwrap();
    store
        .publish(Kind::Execution, &e.key, &record, &c.clock)
        .unwrap();
    let digest = record.get("reviewDigest").code_units();
    let review = old.get(Kind::Review, &digest).unwrap();
    let handle = review.get("observationHandle").code_units();
    for (kind, key, value) in [
        (Kind::Review, digest, review),
        (
            Kind::Observation,
            handle.clone(),
            old.get(Kind::Observation, &handle).unwrap(),
        ),
        (
            Kind::Outcome,
            e.key.clone(),
            old.get(Kind::Outcome, &e.key).unwrap(),
        ),
    ] {
        store.publish(kind, &key, &value, &c.clock).unwrap();
    }
    c.at("2040-01-04T12:10:00.000Z");
    let r = c.reconcile(false);
    assert!(
        r.get("reasonCode")
            .is("qualified_proof_or_target_repair_required")
    );
    c.no_settlement();
    assert!(c.execution().state.needs_reconciliation());
    assert_eq!(c.backend.calls, 1);
}
#[test]
fn t10_resolution_never_substitutes_for_a_known_read() {
    let mut c = Case::new("persisted", "lost", None);
    c.save();
    Case::pending(&c.reconcile(true));
    c.no_settlement();
    c.value(None);
    c.at("2040-01-04T12:10:00.000Z");
    let unknown = c.reconcile(true);
    assert!(unknown.get("status").is("held"));
    assert!(
        unknown
            .get("reasonCode")
            .is("readback_field_unknown_reconcile_later")
    );
    c.no_settlement();
    assert!(c.execution().state.needs_reconciliation());
    assert_eq!(c.backend.calls, 1);
}
#[test]
fn t11_redirect_and_non_2xx_equal_read_wait_until_q() {
    for status in [302, 503] {
        let mut c = Case::new("persisted", "returned", Some(status));
        c.backend.application = Some("- Sent".into());
        assert!(c.save().get("status").is("held"));
        Case::pending(&c.reconcile(false));
        c.no_settlement();
        c.at("2040-01-04T12:10:00.000Z");
        c.reconcile(false);
        assert_eq!(c.execution().state, state::FieldState::ReconciledPersisted);
        assert_eq!(c.backend.calls, 1);
    }
}
#[test]
fn t12_missing_receipt_equal_read_is_unresolved() {
    let mut c = Case::new("persisted", "returned", None);
    c.backend.application = Some("- Sent".into());
    assert!(c.save().get("status").is("held"));
    assert!(c.execution().state.needs_reconciliation());
    c.no_settlement();
    assert_eq!(c.backend.calls, 1);
}
#[test]
fn t13_clock_before_dispatch_is_before_q() {
    let mut c = Case::new("persisted", "lost", None);
    c.save();
    c.at("2040-01-04T11:59:59.999Z");
    Case::pending(&c.reconcile(false));
    c.no_settlement();
    assert_eq!(c.backend.calls, 1);
}

// Synthetic owner port: NativeBackend calls the real http::apply, never a network.
impl http::HttpPort for Backend {
    fn read(&mut self, _: &Js) -> Result<Js> {
        let mut fields = obj(&[]);
        let mut targets = obj(&[]);
        for f in review::FIELDS {
            fields.set(
                f,
                self.observation
                    .borrow()
                    .get("fields")
                    .get(f)
                    .get("value")
                    .clone(),
            );
            targets.set(
                f,
                self.observation
                    .borrow()
                    .get("fields")
                    .get(f)
                    .get("effectTarget")
                    .clone(),
            );
        }
        if self.calls > 0 {
            fields.set("progress", self.application.as_deref().unwrap().into());
            if self.status == "sibling" || self.status == "before-sibling" {
                fields.set("homework", "- Other actor".into());
            }
        }
        Ok(obj(&[
            (
                "target",
                js(
                    json!({"studentName":"Invented Rule Student","date":"2040-01-04","groupId":"17","teacherKey":"11","studentKey":"12","courseKey":"13","recordKey":"15","membershipKey":"14"}),
                ),
            ),
            ("fields", fields),
            ("effectTargets", targets),
            ("previousRecordKey", "16".into()),
            ("previousDate", "2040-01-03".into()),
            ("wireDigest", "2".repeat(64).into()),
        ]))
    }
    fn write(&mut self, _: &Js) -> Result<Js> {
        self.calls += 1;
        Ok(js(
            json!({"httpStatus":200,"bodySha256":sha("invented response")}),
        ))
    }
}
impl native::SourcePort for Backend {
    fn authenticate(&mut self) -> Result<()> {
        Ok(())
    }
    fn discover_groups(&mut self, _: &Js) -> Result<Js> {
        Ok(js(json!({"groups":[{"groupId":"17"}]})))
    }
    fn execute(&mut self, _: &Js) -> Result<Js> {
        Ok(obj(&[(
            "dayRecords",
            Js::Arr(vec![
                http::HttpPort::read(self, &Js::Null)?.get("target").clone(),
            ]),
        )]))
    }
    fn journal_state(&mut self, target: &Js) -> Result<Js> {
        http::HttpPort::read(self, target)
    }
    fn apply(&mut self, action: &Js, approval: &mut dyn Approval) -> Result<Js> {
        http::apply(action, self, approval)
    }
}
#[test]
fn t14_native_equal_read_requires_unchanged_siblings() {
    for mode in ["persisted", "sibling", "third", "before-sibling"] {
        let mut c = Case::new(mode, "returned", Some(200));
        c.backend.application = Some(
            match mode {
                "third" => "- Other",
                "before-sibling" => "- Before",
                _ => "- Sent",
            }
            .into(),
        );
        let store = c.open();
        let admission = js(
            json!({"teacherId":"11","date":"2040-01-04","students":[{"studentId":"12","displayName":"Invented Rule Student"}]}),
        );
        let mut native =
            native::NativeBackend::new(&mut c.backend, admission, Js::Null, vec![], &c.clock);
        let r = service::Service {
            store: &store,
            source: &mut c.source,
            backend: &mut native,
            policy: &Policy,
            runtime: &c.clock,
        }
        .save(&c.input)
        .unwrap();
        if mode != "persisted" {
            Case::pending(&r);
            assert!(c.execution().state.needs_reconciliation());
            c.no_settlement();
        } else {
            assert!(r.get("status").is("saved"), "{r:?}");
        }
        assert_eq!(c.backend.calls, 1);
    }
}
#[test]
fn t15_window_starts_after_backend_return() {
    let mut c = Case::new("not_persisted", "returned", Some(200));
    c.backend.return_at = Some("2040-01-04T12:03:20.000Z");
    let saved = c.save();
    assert_eq!(
        saved.get("earliestSettleAtEpochMs"),
        &Js::Num(DUE + 200_000.0)
    );
    c.at("2040-01-04T12:10:00.000Z");
    assert!(
        c.reconcile(false).get("status").is("held"),
        "settled before return + Q"
    );
    c.no_settlement();
    c.at("2040-01-04T12:13:20.000Z");
    assert!(c.reconcile(false).get("status").is("settled"));
    assert_eq!(
        c.execution().state,
        state::FieldState::ReconciledNotPersisted
    );
}
#[test]
fn t16_recovered_execution_starts_its_own_window() {
    let mut c = Case::new("crash", "lost", None);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.save())).is_err());
    let e = c.execution();
    assert!(c.open().get(Kind::Outcome, &e.key).unwrap().absent());
    c.at("2040-01-04T12:03:20.000Z");
    let recovered = c.reconcile(false);
    assert_eq!(
        recovered.get("earliestSettleAtEpochMs"),
        &Js::Num(DUE + 200_000.0)
    );
    assert!(
        c.open()
            .get(Kind::Outcome, &e.key)
            .unwrap()
            .get("recordedAt")
            .is("2040-01-04T12:03:20.000Z")
    );
    c.at("2040-01-04T12:10:00.000Z");
    assert!(
        c.reconcile(false).get("status").is("held"),
        "recovery settled before recovery + Q"
    );
    c.no_settlement();
    c.at("2040-01-04T12:13:20.000Z");
    assert!(c.reconcile(false).get("status").is("settled"));
    assert_eq!(
        c.execution().state,
        state::FieldState::ReconciledNotPersisted
    );
}

// Two distinct fields of one student's dated record, with independent windows.
fn mixed_window_case(kind: &str) {
    let mut c = Case::new("persisted", "lost", None);
    assert!(c.save().get("status").is("held"));
    c.at("2040-01-04T12:05:00.000Z");
    c.prepare_field("future", "- Future homework", "homework");
    assert!(c.save().get("status").is("held"));
    if kind == "legacy" {
        let old = c.open();
        let executions =
            state::student_executions(&old, c.input.get("context").get("scope")).unwrap();
        c.root = c.root.join("legacy");
        mkdir(&c.root).unwrap();
        let store = c.open();
        for e in executions {
            let mut record = e.record;
            if record.get("field").is("progress") {
                record.set("settlementRule", Js::Null);
                record.set("dispatchedAt", Js::Null);
            }
            store
                .add_ledger(
                    &state::lock_key(record.get("scope")).unwrap(),
                    &e.key,
                    &c.clock,
                )
                .unwrap();
            store
                .publish(Kind::Execution, &e.key, &record, &c.clock)
                .unwrap();
            let digest = record.get("reviewDigest").code_units();
            let review = old.get(Kind::Review, &digest).unwrap();
            let handle = review.get("observationHandle").code_units();
            for (kind, key, value) in [
                (Kind::Review, digest, review),
                (
                    Kind::Observation,
                    handle.clone(),
                    old.get(Kind::Observation, &handle).unwrap(),
                ),
                (
                    Kind::Outcome,
                    e.key.clone(),
                    old.get(Kind::Outcome, &e.key).unwrap(),
                ),
            ] {
                store.publish(kind, &key, &value, &c.clock).unwrap();
            }
        }
    }
    c.value(Some("- Sent"));
    if kind == "unknown" {
        c.value(None);
    }
    if kind == "unavailable" {
        let mut observation = c.source.0.borrow_mut();
        let mut fields = observation.get("fields").clone();
        let mut binding = fields.get("attendance").clone();
        binding.set("value", "invalid-attendance".into());
        fields.set("attendance", binding);
        observation.set("fields", fields);
    }
    c.at(if kind == "future_only" {
        "2040-01-04T12:09:00.000Z"
    } else {
        "2040-01-04T12:10:00.000Z"
    });
    c.source
        .0
        .borrow_mut()
        .set("observedAt", c.clock.now().into());
    let reads = c.source.1.get();
    let result = c.reconcile(false);
    let expected = match kind {
        "unavailable" => "readback_unavailable_reconcile_later",
        "unknown" => "readback_field_unknown_reconcile_later",
        "legacy" => "qualified_proof_or_target_repair_required",
        _ => "save_confirmation_pending",
    };
    assert_eq!(
        result.get("reasonCode"),
        &Js::from(expected),
        "reconcile distinguishes open due executions from future waits: {kind}"
    );
    assert!(result.get("status").is("held"));
    assert_eq!(
        c.source.1.get() - reads,
        usize::from(kind != "future_only"),
        "exactly the due execution is read"
    );
    assert_eq!(c.backend.calls, 2, "reconcile never dispatches a save");
    let store = c.open();
    let executions =
        state::student_executions(&store, c.input.get("context").get("scope")).unwrap();
    assert_eq!(executions.len(), 2);
    let future = executions
        .iter()
        .find(|e| e.record.get("requestRef").is("future"))
        .unwrap();
    let due = executions
        .iter()
        .find(|e| e.record.get("requestRef").is("first"))
        .unwrap();
    assert!(future.state.needs_reconciliation());
    assert!(
        store
            .get(Kind::Reconciliation, &future.key)
            .unwrap()
            .absent()
    );
    assert_eq!(due.state.needs_reconciliation(), kind != "equal");
    let future_field = result
        .get("fields")
        .array()
        .iter()
        .find(|f| f.get("requestRef").is("future"))
        .unwrap();
    assert_eq!(
        future_field.get("earliestSettleAtEpochMs"),
        &Js::Num(DUE + 300_000.)
    );
    if kind == "equal" || kind == "future_only" {
        assert_eq!(
            result.get("earliestSettleAtEpochMs"),
            &Js::Num(if kind == "equal" { DUE + 300_000. } else { DUE }),
            "pending retains the next actual due time"
        );
    }
    let key = state::request_key(c.input.get("context").get("scope"), "first").unwrap();
    let audit = store.audit(&key, 256).unwrap();
    for event in [
        "reconcile_readback_unavailable",
        "reconcile_unqualified_evidence",
    ] {
        let expected = (kind == "unavailable" && event == "reconcile_readback_unavailable")
            || (kind == "legacy" && event == "reconcile_unqualified_evidence");
        assert_eq!(
            audit
                .array()
                .iter()
                .filter(|row| row.get("event").is(event))
                .count(),
            usize::from(expected),
            "existing audit events remain unchanged"
        );
    }
}
#[test]
fn mixed_due_unavailable_keeps_failure() {
    mixed_window_case("unavailable");
}
#[test]
fn mixed_due_unknown_keeps_failure() {
    mixed_window_case("unknown");
}
#[test]
fn mixed_due_unqualified_evidence_keeps_failure() {
    mixed_window_case("legacy");
}
#[test]
fn mixed_due_closed_with_future_is_pending() {
    mixed_window_case("equal");
}
#[test]
fn mixed_only_future_is_pending_without_reads() {
    mixed_window_case("future_only");
}

#[test]
fn moved_target_does_not_block_other_pending_fields_in_either_key_order() {
    use edu_academy_core::workflow::ledger_cases;
    use state::FieldState;
    let mut failures = vec![];
    for ledgered in [false, true] {
        let mut previous = None;
        for moved_first in [true, false] {
            let mut c = Case::new("persisted", "lost", None);
            let root = c.root.join("pending");
            let store = if ledgered {
                Store::open_with_ledger(
                    &root,
                    &c.root,
                    Box::new(ledger_cases::MemoryLedger::default()),
                )
            } else {
                Store::open(&root, &c.root)
            }
            .unwrap();
            let scope = c.input.get("context").get("scope").clone();
            let target = c.source.0.borrow().get("target").clone();
            let moved_key = if moved_first { "a-moved" } else { "z-moved" };
            for (key, field) in [
                (moved_key, "progress"),
                ("b-homework", "homework"),
                ("c-memo", "lessonMemo"),
            ] {
                let mut execution =
                    ledger_cases::execution(&store, &scope, field, "15".into(), &target, field);
                execution.set("effectTarget", "target".into());
                store
                    .add_ledger(&state::lock_key(&scope).unwrap(), key, &c.clock)
                    .unwrap();
                store
                    .publish(Kind::Execution, key, &execution, &c.clock)
                    .unwrap();
                store
                    .publish(Kind::Outcome, key, &ledger_cases::outcome(false), &c.clock)
                    .unwrap();
            }
            c.at("2040-01-04T12:10:00.000Z");
            {
                let mut observation = c.source.0.borrow_mut();
                observation.set("observedAt", c.clock.now().into());
                let mut fields = observation.get("fields").clone();
                let mut moved = fields.get("progress").clone();
                moved.set("effectTarget", "moved-target".into());
                fields.set("progress", moved);
                let mut homework = fields.get("homework").clone();
                homework.set("value", "- Sent".into());
                fields.set("homework", homework);
                observation.set("fields", fields);
            }
            let reads_before = c.source.1.get();
            let result = service::Service {
                store: &store,
                source: &mut c.source,
                backend: &mut c.backend,
                policy: &Policy,
                runtime: &c.clock,
            }
            .reconcile(&obj(&[("context", c.input.get("context").clone())]));
            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    failures.push(format!(
                        "ledgered={ledgered} moved_first={moved_first}: {}",
                        error.message
                    ));
                    drop(store);
                    std::fs::remove_dir_all(c.root).unwrap();
                    continue;
                }
            };
            assert!(result.get("status").is("held"));
            assert!(
                result
                    .get("reasonCode")
                    .is("qualified_proof_or_target_repair_required")
            );
            let mut outcomes = result.get("fields").array().to_vec();
            outcomes.sort_by_key(|f| f.get("field").code_units());
            for (field, expected) in [
                ("progress", FieldState::ReconciliationRequired),
                ("homework", FieldState::ReconciledPersisted),
                ("lessonMemo", FieldState::ReconciledNotPersisted),
            ] {
                let row = outcomes.iter().find(|f| f.get("field").is(field)).unwrap();
                assert!(row.get("executionState").is(expected.name()), "{row:?}");
            }
            let moved = outcomes
                .iter()
                .find(|f| f.get("field").is("progress"))
                .unwrap();
            assert!(moved.get("status").is("uncertain"));
            assert!(moved.get("reasonCode").is("workflow_effect_target_drift"));
            assert!(store.get(Kind::Reconciliation, moved_key).unwrap().absent());
            assert_eq!(
                state::unresolved_on_field(&store, &scope, "progress", "15", &target)
                    .unwrap()
                    .len(),
                1
            );
            for key in ["b-homework", "c-memo"] {
                assert!(!store.get(Kind::Reconciliation, key).unwrap().absent());
            }
            assert_eq!(c.backend.calls, 0, "reconciliation must not dispatch");
            assert_eq!(c.source.1.get() - reads_before, 3, "three due reads");
            if let Some(previous) = previous {
                assert_eq!(
                    outcomes, previous,
                    "per-field results must not depend on key order"
                );
            }
            previous = Some(outcomes);
            drop(store);
            std::fs::remove_dir_all(c.root).unwrap();
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
