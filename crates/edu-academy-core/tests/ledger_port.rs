//! Synthetic ledger controls, with no live transport or retained private data.
use edu_academy_core::{
    js::{Fault, Js, Result, obj, sha},
    workflow::{
        Runtime, codec,
        ledger::{Attempt, AttemptLedger, AttemptTarget},
        ledger_cases::{
            self, Clock, MemoryLedger, execution, outcome, reconciliation, root, scope, target,
        },
        state::{self, FieldState},
        store::{Kind, Store},
    },
};
use std::{
    cell::Cell,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    rc::Rc,
};
fn memory(root: &Path, owner: &Path) -> Result<Store> {
    Store::open_with_ledger(root, owner, Box::<MemoryLedger>::default())
}
#[test]
fn ledger_cases_on_files() {
    ledger_cases::run(&Store::open);
}
#[test]
fn ledger_cases_on_memory() {
    ledger_cases::run(&memory);
    ledger_cases::run_ledgered(&memory);
}

#[derive(Default)]
struct Probe {
    ledger: Rc<MemoryLedger>,
    override_state: bool,
    named_digest: Option<String>,
    fail_after_begin: Cell<bool>,
    require_body: Option<PathBuf>,
    attempt_reads: Rc<Cell<usize>>,
    group_reads: Rc<Cell<usize>>,
}
impl AttemptLedger for Probe {
    fn begin(
        &self,
        key: &str,
        target: &AttemptTarget,
        digest: &str,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        let result = self.ledger.begin(key, target, digest, clock)?;
        if self.fail_after_begin.replace(false) {
            // Simulate an action committed but its answer lost. The next request
            // must be able to read the committed attempt's already durable body.
            let path = self
                .require_body
                .as_ref()
                .unwrap()
                .join("execution")
                .join(format!(
                    "{}.{}.json",
                    sha(&("execution\n".to_owned() + key)),
                    digest
                ));
            assert!(
                path.is_file(),
                "committed attempt must already name a durable body"
            );
            return Err(Fault::error("invented_answer_lost"));
        }
        Ok(result)
    }
    fn record_outcome(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        self.ledger.record_outcome(key, digest, state, clock)
    }
    fn record_reconciliation(
        &self,
        key: &str,
        digest: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool> {
        self.ledger.record_reconciliation(key, digest, state, clock)
    }
    fn attempt(&self, key: &str) -> Result<Option<Attempt>> {
        self.attempt_reads.set(self.attempt_reads.get() + 1);
        if let Some(digest) = &self.named_digest {
            return Ok(Some(Attempt {
                key: key.into(),
                state: FieldState::DispatchUnknown,
                execution: digest.clone(),
                outcome: None,
                reconciliation: None,
            }));
        }
        Ok(self.ledger.attempt(key)?.map(|mut a| {
            if self.override_state {
                a.state = FieldState::ReconciledConflict;
            }
            a
        }))
    }
    fn attempts(&self, group: &str, limit: usize) -> Result<Vec<Attempt>> {
        self.group_reads.set(self.group_reads.get() + 1);
        let mut attempts = self.ledger.attempts(group, limit)?;
        if self.override_state {
            for a in &mut attempts {
                a.state = FieldState::ReconciledConflict;
            }
        }
        Ok(attempts)
    }
    fn open_on(&self, target: &AttemptTarget) -> Result<Vec<Attempt>> {
        self.ledger.open_on(target)
    }
}
#[test]
fn classify_equals_field_state() {
    let root = root("classify");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let scope = scope("12");
    let e = execution(
        &store,
        &scope,
        "progress",
        "15".into(),
        &target(&scope),
        "review",
    );
    let null = Js::Null;
    let unresolved = outcome(false);
    let settled = outcome(true);
    let persisted = reconciliation("- Sent".into());
    let before = reconciliation("- Before".into());
    let conflict = reconciliation("- Other".into());
    let early = edu_academy_core::workflow::merge(
        persisted.clone(),
        &obj(&[("readAt", "2040-01-04T12:09:59.999Z".into())]),
    );
    let rows = [
        (&null, &null, &null, FieldState::Available),
        (&e, &null, &null, FieldState::DispatchUnknown),
        (&e, &unresolved, &null, FieldState::ReconciliationRequired),
        (
            &null,
            &unresolved,
            &null,
            FieldState::ReconciliationRequired,
        ),
        (&null, &null, &persisted, FieldState::ReconciliationRequired),
        (&e, &settled, &null, FieldState::SettledPersisted),
        (&e, &unresolved, &persisted, FieldState::ReconciledPersisted),
        (&e, &unresolved, &before, FieldState::ReconciledNotPersisted),
        (&e, &unresolved, &conflict, FieldState::ReconciledConflict),
        (&e, &unresolved, &early, FieldState::ReconciliationRequired),
    ];
    for (index, (e, o, r, expected)) in rows.into_iter().enumerate() {
        let key = format!("case-{index}");
        for (kind, record) in [
            (Kind::Execution, e),
            (Kind::Outcome, o),
            (Kind::Reconciliation, r),
        ] {
            if !record.absent() {
                store.publish(kind, &key, record, &Clock).unwrap();
            }
        }
        let classified = state::classify(e, o, r, &|d| store.get(Kind::Review, d)).unwrap();
        assert_eq!(classified, expected, "table case {index}");
        assert_eq!(state::field_state(&store, &key).unwrap(), classified);
    }
    let no_review =
        |_: &str| -> Result<Js> { panic!("review loaded on a path that does not need it") };
    assert_eq!(
        state::classify(&e, &settled, &null, &no_review).unwrap(),
        FieldState::SettledPersisted
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn state_is_the_ledgers() {
    let root = root("authority");
    let probe = Probe {
        override_state: true,
        ..Probe::default()
    };
    let attempt_reads = probe.attempt_reads.clone();
    let group_reads = probe.group_reads.clone();
    let store = Store::open_with_ledger(&root.join("store"), &root, Box::new(probe)).unwrap();
    let scope = scope("12");
    let e = execution(
        &store,
        &scope,
        "progress",
        "15".into(),
        &target(&scope),
        "review",
    );
    store.publish(Kind::Execution, "key", &e, &Clock).unwrap();
    store
        .publish(Kind::Outcome, "key", &outcome(false), &Clock)
        .unwrap();
    assert_eq!(
        state::classify(&e, &outcome(false), &Js::Null, &|d| store
            .get(Kind::Review, d))
        .unwrap(),
        FieldState::ReconciliationRequired
    );
    attempt_reads.set(0);
    assert_eq!(
        state::field_state(&store, "key").unwrap(),
        FieldState::ReconciledConflict,
        "field_state must use ledger state"
    );
    assert_eq!(
        attempt_reads.get(),
        1,
        "state lookup reads only the attempt"
    );
    group_reads.set(0);

    assert_eq!(
        state::student_executions(&store, &scope).unwrap()[0].state,
        FieldState::ReconciledConflict,
        "student_executions must use ledger state"
    );
    assert_eq!(
        group_reads.get(),
        1,
        "student query reads the attempts once"
    );

    drop(store);
    fs::remove_dir_all(root).unwrap();
}
fn body(store: &Store) -> PathBuf {
    fs::read_dir(store.root().join("execution"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}
#[test]
fn body_file_checked() {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let uid: u32 = status
        .lines()
        .find(|s| s.starts_with("Uid:"))
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    for damage in [
        "mode", "link", "large", "bytes", "symlink", "missing", "owner",
    ] {
        if damage == "owner" && uid != 0 {
            eprintln!("owner case skipped: not root");
            continue;
        }
        let root = root(damage);
        let store = memory(&root.join("store"), &root).unwrap();
        let scope = scope("12");
        let e = execution(
            &store,
            &scope,
            "progress",
            "15".into(),
            &target(&scope),
            "review",
        );
        store.publish(Kind::Execution, "key", &e, &Clock).unwrap();
        let path = body(&store);
        match damage {
            "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            "link" => fs::hard_link(&path, root.join("second-link")).unwrap(),
            "large" => fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(65_537)
                .unwrap(),
            "bytes" => {
                let mut bytes = fs::read(&path).unwrap();
                bytes[0] = b'[';
                fs::write(&path, bytes).unwrap();
            }
            "symlink" => {
                let retained = root.join("retained");
                fs::rename(&path, &retained).unwrap();
                symlink(retained, &path).unwrap();
            }
            "missing" => fs::remove_file(&path).unwrap(),
            "owner" => {
                std::os::unix::fs::chown(&path, Some(4242), None).unwrap();
                eprintln!("owner case executed: effective uid {uid}, body owner 4242");
            }
            _ => unreachable!(),
        }
        assert_eq!(
            store.get(Kind::Execution, "key").unwrap_err().message,
            "workflow_attempt_body_unavailable",
            "body check: {damage}"
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    for (label, kind, id) in [("kind", "outcome", "key"), ("id", "execution", "another")] {
        let root = root(label);
        let bytes = codec::bytes(&obj(&[
            ("kind", kind.into()),
            ("id", id.into()),
            ("record", obj(&[("marker", "invented body".into())])),
        ]))
        .unwrap();
        let digest = sha(std::str::from_utf8(&bytes).unwrap());
        let store = Store::open_with_ledger(
            &root.join("store"),
            &root,
            Box::new(Probe {
                named_digest: Some(digest.clone()),
                ..Probe::default()
            }),
        )
        .unwrap();
        let path = store.root().join("execution").join(format!(
            "{}.{}.json",
            sha("execution\nkey"),
            digest
        ));
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            store.get(Kind::Execution, "key").unwrap_err().message,
            "workflow_attempt_body_unavailable",
            "body wrapper check: {label}"
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn body_survives_a_second_link() {
    let root = root("publish-link");
    let store = memory(&root.join("store"), &root).unwrap();
    let scope = scope("12");
    let e = execution(
        &store,
        &scope,
        "progress",
        "15".into(),
        &target(&scope),
        "review",
    );
    let bytes = codec::bytes(&obj(&[
        ("kind", "execution".into()),
        ("id", "key".into()),
        ("record", e.clone()),
    ]))
    .unwrap();
    let digest = sha(std::str::from_utf8(&bytes).unwrap());
    let retained = root.join("retained");
    fs::write(&retained, bytes).unwrap();
    fs::set_permissions(&retained, fs::Permissions::from_mode(0o600)).unwrap();
    let final_path =
        store
            .root()
            .join("execution")
            .join(format!("{}.{}.json", sha("execution\nkey"), digest));
    fs::hard_link(&retained, &final_path).unwrap();
    assert!(store.publish(Kind::Execution, "key", &e, &Clock).unwrap().0);
    assert_eq!(
        store.get(Kind::Execution, "key").unwrap(),
        e,
        "rename must replace the multiply linked body"
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn body_only_through_digest() {
    let root = root("orphan");
    let store = memory(&root.join("store"), &root).unwrap();
    let wrapper = obj(&[
        ("kind", "execution".into()),
        ("id", "orphan".into()),
        ("record", obj(&[("marker", "orphan body".into())])),
    ]);
    let bytes = codec::bytes(&wrapper).unwrap();
    let key_hash = sha("execution\norphan");
    // Both a legacy key-only file and an unreferenced content-addressed body.
    fs::write(
        store
            .root()
            .join("execution")
            .join(format!("{key_hash}.json")),
        &bytes,
    )
    .unwrap();
    fs::write(
        store.root().join("execution").join(format!(
            "{key_hash}.{}.json",
            sha(std::str::from_utf8(&bytes).unwrap())
        )),
        bytes,
    )
    .unwrap();
    assert_eq!(
        store.get(Kind::Execution, "orphan").unwrap(),
        Js::Null,
        "orphan body must never be returned without a ledger digest"
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn reconciliation_must_close() {
    let root = root("closing");
    let ledger = Rc::new(MemoryLedger::default());
    let store = Store::open_with_ledger(
        &root.join("store"),
        &root,
        Box::new(Probe {
            ledger: ledger.clone(),
            ..Probe::default()
        }),
    )
    .unwrap();
    let scope = scope("12");
    let e = execution(
        &store,
        &scope,
        "progress",
        "15".into(),
        &target(&scope),
        "review",
    );
    store.publish(Kind::Execution, "key", &e, &Clock).unwrap();
    store
        .publish(Kind::Outcome, "key", &outcome(false), &Clock)
        .unwrap();
    assert_eq!(
        store
            .publish(
                Kind::Reconciliation,
                "key",
                &reconciliation(Js::Num(42.)),
                &Clock
            )
            .unwrap_err()
            .message,
        "workflow_reconciliation_not_closing"
    );
    assert_eq!(
        fs::read_dir(store.root().join("reconciliation"))
            .unwrap()
            .count(),
        0
    );
    assert!(
        ledger
            .attempt("key")
            .unwrap()
            .unwrap()
            .reconciliation
            .is_none()
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn body_precedes_ledger_even_when_answer_is_lost() {
    let root = root("lost");
    let path = root.join("store");
    let store = Store::open_with_ledger(
        &path,
        &root,
        Box::new(Probe {
            fail_after_begin: Cell::new(true),
            require_body: Some(path.clone()),
            ..Probe::default()
        }),
    )
    .unwrap();
    let scope = scope("12");
    let e = execution(
        &store,
        &scope,
        "progress",
        "15".into(),
        &target(&scope),
        "review",
    );
    assert_eq!(
        store
            .publish(Kind::Execution, "key", &e, &Clock)
            .unwrap_err()
            .message,
        "invented_answer_lost"
    );
    assert_eq!(
        store.get(Kind::Execution, "key").unwrap(),
        e,
        "retry must find the committed body after the answer was lost"
    );
    assert_eq!(
        store.publish(Kind::Execution, "key", &e, &Clock).unwrap(),
        (false, e)
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn target_validation_and_ledger_contract() {
    let root = root("target");
    let store = memory(&root.join("store"), &root).unwrap();
    let scope = scope("12");
    let mut occurrence = target(&scope);
    occurrence.set("lessonDate", "2040-01-03".into());
    let mut e = execution(
        &store,
        &scope,
        "previousHomeworkAssessment",
        obj(&[("id", "15".into())]),
        &occurrence,
        "review",
    );
    e.set("requestRef", Js::Num(7.));
    let t = store.attempt_target(&e).unwrap();
    assert_eq!(t.record_key, "{\"id\":\"15\"}");
    assert_eq!(t.request_ref, "7");
    assert_eq!(t.request_lesson_date, "2040-01-04");
    assert_eq!(t.occurrence[2], "2040-01-03");
    e.set("field", Js::Null);
    assert_eq!(
        store.attempt_target(&e).unwrap_err().message,
        "workflow_execution_review_target_unavailable"
    );
    let ledger = MemoryLedger::default();
    assert_eq!(
        ledger
            .record_outcome(
                "absent",
                "digest",
                FieldState::ReconciliationRequired,
                &Clock
            )
            .unwrap_err()
            .message,
        "workflow_ledger_unavailable"
    );
    ledger.begin("key", &t, "digest", &Clock).unwrap();
    assert_eq!(
        ledger
            .record_reconciliation("key", "digest", FieldState::ReconciledConflict, &Clock)
            .unwrap_err()
            .message,
        "workflow_ledger_unavailable"
    );
    assert!(
        ledger
            .attempt("key")
            .unwrap()
            .unwrap()
            .reconciliation
            .is_none()
    );
    ledger
        .record_outcome(
            "key",
            "outcome-digest",
            FieldState::SettledPersisted,
            &Clock,
        )
        .unwrap();
    assert!(
        !ledger
            .record_reconciliation(
                "key",
                "reconciliation-digest",
                FieldState::ReconciledConflict,
                &Clock
            )
            .unwrap(),
        "reconciliation on an outcome-closed attempt must return false"
    );
    let closed = ledger.attempt("key").unwrap().unwrap();
    assert_eq!(closed.state, FieldState::SettledPersisted);
    assert_eq!(closed.outcome.as_deref(), Some("outcome-digest"));
    assert!(closed.reconciliation.is_none());
    assert!(ledger.attempts(&t.group, 0).unwrap().is_empty());
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

mod route {
    use edu_academy_core::{
        js::{Js, Result, obj, sha},
        workflow::{
            Runtime,
            approval::Approval,
            canonical, codec, review, service, state,
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
        ledger: Option<Rc<super::MemoryLedger>>,
        root: PathBuf,
        source: Source,
        backend: Backend,
        clock: Clock,
        input: Js,
    }
    impl Case {
        fn new(
            status: &'static str,
            transport: &'static str,
            receipt: Option<u16>,
            ledgered: bool,
        ) -> Self {
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
                ledger: ledgered.then(|| Rc::new(super::MemoryLedger::default())),
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
            match &self.ledger {
                Some(ledger) => Store::open_with_ledger(
                    &self.root.join("store"),
                    &self.root,
                    Box::new(super::Probe {
                        ledger: ledger.clone(),
                        ..super::Probe::default()
                    }),
                )
                .unwrap(),
                None => Store::open(&self.root.join("store"), &self.root).unwrap(),
            }
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
    }
    #[test]
    fn existing_route_unchanged() {
        // Re-run settlement_rule_b::t1_immediate_saved_admits_second_save against
        // each store factory, preserving its two save calls and assertions.
        let mut answers = vec![];
        for ledgered in [false, true] {
            let mut c = Case::new("persisted", "returned", Some(204), ledgered);
            c.backend.application = Some("- Sent".into());
            let first = c.save();
            assert!(first.get("status").is("saved"));
            let e = state::student_executions(&c.open(), c.input.get("context").get("scope"))
                .unwrap()
                .remove(0);
            let out = c.open().get(Kind::Outcome, &e.key).unwrap();
            assert_eq!(out.get("receipt").get("httpStatus"), &Js::Num(204.));
            assert!(
                out.get("receipt")
                    .get("bodySha256")
                    .is(&sha("invented response"))
            );
            assert_eq!(e.state, state::FieldState::SettledPersisted);
            // The file ledger consumes an extra UUID for its index file. Give the
            // next user request the same independent entropy sequence on both stores.
            c.clock.seq.set(1000);
            c.prepare("second", "- Next");
            c.backend.application = Some("- Next".into());
            let second = c.save();
            assert!(second.get("status").is("saved"));
            assert_eq!(c.backend.calls, 2);
            answers.push((first, second));
            std::fs::remove_dir_all(&c.root).unwrap();
        }
        assert_eq!(
            answers[0], answers[1],
            "both stores return identical save answers"
        );
    }
}

#[test]
fn legacy_root_refused() {
    let owner = root("engine-sibling");
    let store_root = owner.join("store");
    fs::create_dir(owner.join("store.ledger")).unwrap();
    assert_eq!(
        Store::open(&store_root, &owner).err().unwrap().message,
        "workflow_ledger_legacy_root"
    );
    assert!(
        !store_root.exists(),
        "refuse before creating file-store directories"
    );
    fs::remove_dir_all(owner).unwrap();
}

#[test]
fn open_with_ledger_refuses_file_attempts() {
    let owner = root("file-form");
    let store_root = owner.join("store");
    let file_store = Store::open(&store_root, &owner).unwrap();
    let s = scope("12");
    let e = execution(
        &file_store,
        &s,
        "progress",
        "15".into(),
        &target(&s),
        "review",
    );
    file_store
        .publish(Kind::Execution, "existing", &e, &Clock)
        .unwrap();
    file_store
        .add_ledger(&state::lock_key(&s).unwrap(), "existing", &Clock)
        .unwrap();
    drop(file_store);
    assert_eq!(
        Store::open_with_ledger(&store_root, &owner, Box::<MemoryLedger>::default())
            .err()
            .unwrap()
            .message,
        "workflow_ledger_legacy_root",
        "file attempts must not lose their fences"
    );
    fs::remove_dir_all(owner).unwrap();
}
