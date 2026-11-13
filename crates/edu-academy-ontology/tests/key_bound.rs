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
