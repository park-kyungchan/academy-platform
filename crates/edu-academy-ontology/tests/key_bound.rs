//! Reproduces the validator/key-size contradiction in the LB continuation.
use edu_academy_core::{
    decision::FIELDS,
    js::{Js, obj, sha},
    workflow::{canonical, ledger_cases, schema::MAX_EFFECT_TARGET_UNITS, service, state},
};

#[test]
fn qualified_effect_target_exceeds_the_requested_key_bound() {
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
    assert!(
        key.len() > 16_384,
        "current qualified route inputs exceed the Lead's allowed maximum"
    );
    println!(
        "BOUND_REPRO qualified_observation=true effect_utf16_units={} effect_json_bytes={} execution_key_bytes={} requested_max_bytes=16384",
        effect_target.units().len(),
        effect_json.len(),
        key.len()
    );
}
