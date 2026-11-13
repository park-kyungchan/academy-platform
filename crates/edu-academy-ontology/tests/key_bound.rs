//! The engine ledger applies its explicit 8192-byte execution-key policy.
use edu_academy_core::{
    decision::FIELDS,
    js::{Js, obj, sha},
    workflow::{canonical, ledger_cases, schema::MAX_EFFECT_TARGET_UNITS, service, state},
};

#[test]
fn qualified_large_key_is_refused_without_creating_a_store() {
    let scope = ledger_cases::scope("12");
    let mut target = ledger_cases::target(&scope);
    target.set("membershipId", "membership".into());
    target.set("recordId", "record".into());
    let effect_target = Js::Str(vec![0; MAX_EFFECT_TARGET_UNITS]);
    let mut fields = obj(&[]);
    for field in FIELDS {
        fields.set(
            field,
            obj(&[
                ("state", "unknown".into()),
                ("reasonCode", "invented".into()),
            ]),
        );
    }
    fields.set(
        "progress",
        obj(&[
            ("state", "known".into()),
            ("value", "- Before".into()),
            ("evidenceRefs", Js::Arr(vec!["invented-evidence".into()])),
            ("effectTarget", effect_target.clone()),
            ("effectRecordKey", "record".into()),
        ]),
    );
    let observation = obj(&[
        ("scope", scope.clone()),
        ("target", target),
        ("previousTarget", Js::Null),
        ("observedAt", "2040-01-04T12:00:00.000Z".into()),
        ("sourceRevision", sha("invented source").into()),
        ("wireDigest", sha("invented wire").into()),
        ("fields", fields),
        ("toneSamples", Js::Arr(vec![])),
        ("transmissionState", "not_sent".into()),
    ]);
    let qualified = service::qualified_observation(&observation, &scope).unwrap();
    assert_eq!(
        qualified.get("fields").get("progress").get("effectTarget"),
        &effect_target
    );
    let key = state::execution_key(
        "request",
        &scope,
        "progress",
        "record",
        &"- Before".into(),
        &"- After".into(),
        &effect_target,
    )
    .unwrap();
    let effect_json = canonical(&effect_target).unwrap();
    assert_eq!(effect_json.len(), 4096 * 6 + 2);
    assert_eq!(key.len(), 24_916);
    let owner = ledger_cases::root("key-bound");
    let missing = owner.join("not-created.sqlite");
    let ledger = edu_academy_ontology::EngineLedger::new(missing.clone(), &ledger_cases::Clock);
    let target = edu_academy_core::workflow::ledger::AttemptTarget {
        group: "invented".into(),
        source_namespace: "invented".into(),
        teacher_id: "11".into(),
        scope_student_id: "12".into(),
        request_lesson_date: "2040-01-04".into(),
        field: "progress".into(),
        record_key: "record".into(),
        request_ref: "request".into(),
        occurrence: [
            "invented".into(),
            "12".into(),
            "2040-01-04".into(),
            "course".into(),
        ],
    };
    for oversized in [&key, &"x".repeat(8193)] {
        let fault = ledger
            .begin(oversized, &target, &sha("execution"), &ledger_cases::Clock)
            .unwrap_err();
        assert_eq!(fault.message, "workflow_ledger_unavailable");
        assert!(
            fault
                .properties
                .get("engineCode")
                .is("execution_key_too_large")
        );
        assert!(!missing.exists(), "no engine call for an oversized key");
    }
    let root = owner.join("store");
    edu_academy_ontology::open_store(&root, &owner, &ledger_cases::Clock).unwrap();
    let ledger = edu_academy_ontology::EngineLedger::new(
        owner.join("store.ledger/academy.sqlite"),
        &ledger_cases::Clock,
    );
    let exact = "x".repeat(8192);
    assert!(
        ledger
            .begin(&exact, &target, &sha("execution"), &ledger_cases::Clock)
            .unwrap()
    );
    assert_eq!(ledger.attempt(&exact).unwrap().unwrap().key, exact);
    std::fs::remove_dir_all(owner).unwrap();
}

#[test]
fn oversized_execution_publish_creates_no_body() {
    use edu_academy_core::workflow::store::Kind;
    fn files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = vec![];
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                found.extend(files(&path));
            } else {
                found.push(path);
            }
        }
        found.sort();
        found
    }
    let owner = ledger_cases::root("publish-key-bound");
    let root = owner.join("store");
    let store = edu_academy_ontology::open_store(&root, &owner, &ledger_cases::Clock).unwrap();
    let scope = ledger_cases::scope("12");
    let record = ledger_cases::execution(
        &store,
        &scope,
        "progress",
        "15".into(),
        &ledger_cases::target(&scope),
        "review",
    );
    let before = files(&root);
    let fault = store
        .publish(
            Kind::Execution,
            &"x".repeat(8193),
            &record,
            &ledger_cases::Clock,
        )
        .unwrap_err();
    let after = files(&root);
    assert_eq!(after, before, "refused publication must create no file");
    assert_eq!(fault.message, "workflow_execution_key_too_large");
    assert_eq!(
        store
            .attempt_count(&state::lock_key(&scope).unwrap(), 10)
            .unwrap(),
        0
    );
    assert!(files(&root.join("execution")).is_empty());
    drop(store);
    std::fs::remove_dir_all(owner).unwrap();
}

#[test]
fn oversized_body_publish_leaves_no_file_or_attempt() {
    use edu_academy_core::workflow::{codec, store::Kind};
    fn files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut paths = vec![];
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            paths.push(path.clone());
            if path.is_dir() {
                paths.extend(files(&path));
            }
        }
        paths.sort();
        paths
    }
    let owner = ledger_cases::root("body-bound");
    let result = std::panic::catch_unwind(|| {
        let root = owner.join("store");
        let store = edu_academy_ontology::open_store(&root, &owner, &ledger_cases::Clock).unwrap();
        let scope = ledger_cases::scope("12");
        let mut record = ledger_cases::execution(
            &store,
            &scope,
            "progress",
            "15".into(),
            &ledger_cases::target(&scope),
            "review",
        );
        record.set("padding", "".into());
        let wrapper = |record: &Js| {
            obj(&[
                ("kind", "execution".into()),
                ("id", "body-bound".into()),
                ("record", record.clone()),
            ])
        };
        let base = codec::bytes(&wrapper(&record)).unwrap().len();
        record.set("padding", "x".repeat(65_537 - base).into());
        assert_eq!(codec::bytes(&wrapper(&record)).unwrap().len(), 65_537);
        let before = files(&root);
        let published = store.publish(Kind::Execution, "body-bound", &record, &ledger_cases::Clock);
        let after = files(&root);
        let count = store
            .attempt_count(&state::lock_key(&scope).unwrap(), 10)
            .unwrap();
        let read = store.get(Kind::Execution, "body-bound");
        assert_eq!(
            (after, count),
            (before, 0),
            "oversized body must leave no file or open attempt; publish={published:?}, get={read:?}"
        );
        assert_eq!(
            published.unwrap_err().message,
            "workflow_attempt_body_too_large"
        );
        assert_eq!(read.unwrap(), Js::Null);
        record.set("padding", "x".repeat(65_536 - base).into());
        assert_eq!(codec::bytes(&wrapper(&record)).unwrap().len(), 65_536);
        assert!(
            store
                .publish(Kind::Execution, "body-bound", &record, &ledger_cases::Clock)
                .unwrap()
                .0
        );
        assert_eq!(store.get(Kind::Execution, "body-bound").unwrap(), record);
    });
    std::fs::remove_dir_all(owner).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn outcome_and_reconciliation_body_bounds_preserve_ledger_and_readback() {
    use edu_academy_core::workflow::{codec, store::Kind};
    let owner = ledger_cases::root("settlement-body-bound");
    let result = std::panic::catch_unwind(|| {
        let root = owner.join("store");
        let store = edu_academy_ontology::open_store(&root, &owner, &ledger_cases::Clock).unwrap();
        let scope = ledger_cases::scope("12");
        let execution = ledger_cases::execution(
            &store,
            &scope,
            "progress",
            "15".into(),
            &ledger_cases::target(&scope),
            "review",
        );
        store
            .publish(Kind::Execution, "key", &execution, &ledger_cases::Clock)
            .unwrap();
        let ledger = edu_academy_ontology::EngineLedger::new(
            owner.join("store.ledger/academy.sqlite"),
            &ledger_cases::Clock,
        );
        for (kind, mut record) in [
            (Kind::Outcome, ledger_cases::outcome(false)),
            (
                Kind::Reconciliation,
                ledger_cases::reconciliation("- Before".into()),
            ),
        ] {
            record.set("padding", "".into());
            let wrapper = |record: &Js| {
                obj(&[
                    ("kind", kind.name().into()),
                    ("id", "key".into()),
                    ("record", record.clone()),
                ])
            };
            let base = codec::bytes(&wrapper(&record)).unwrap().len();
            record.set("padding", "x".repeat(65_537 - base).into());
            let before = ledger.attempt("key").unwrap().unwrap();
            let err = store
                .publish(kind, "key", &record, &ledger_cases::Clock)
                .unwrap_err();
            assert_eq!(err.message, "workflow_attempt_body_too_large");
            assert!(
                std::fs::read_dir(root.join(kind.name()))
                    .unwrap()
                    .next()
                    .is_none()
            );
            let after = ledger.attempt("key").unwrap().unwrap();
            assert_eq!(
                (
                    after.execution,
                    after.outcome,
                    after.reconciliation,
                    after.state
                ),
                (
                    before.execution,
                    before.outcome,
                    before.reconciliation,
                    before.state
                )
            );
            record.set("padding", "x".repeat(65_536 - base).into());
            assert_eq!(codec::bytes(&wrapper(&record)).unwrap().len(), 65_536);
            assert!(
                store
                    .publish(kind, "key", &record, &ledger_cases::Clock)
                    .unwrap()
                    .0
            );
            assert_eq!(store.get(kind, "key").unwrap(), record);
        }
    });
    std::fs::remove_dir_all(owner).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
