//! Initial read-only rows. Later packets add real dispatch/result traces here.
use edu_academy_core::{js::Js, workflow::codec};
use edu_academy_teacher::{record_data::*, record_fixture::*};
use std::collections::BTreeSet;
struct Dispatch {
    identity: RecordIdentity,
    field: RecordField,
    target: Js,
    attempt: String,
    approving_call: u64,
    fenced: bool,
    earlier_unresolved: bool,
}
struct Saved {
    identity: RecordIdentity,
    field: RecordField,
    target: Js,
    intended: String,
    readback: RecordValue,
}
fn check(
    preview: &RecordIdentity,
    selected: &[(RecordField, Js, RecordValue)],
    dispatches: &[Dispatch],
    saved: &[Saved],
) {
    let mut attempts = BTreeSet::new();
    let mut calls = BTreeSet::new();
    for effect in dispatches {
        assert_eq!(&effect.identity, preview, "G1: target differs from preview");
        assert!(
            selected
                .iter()
                .any(|(field, target, _)| *field == effect.field && *target == effect.target),
            "G1: physical field target not previewed"
        );
        assert!(
            effect.fenced && !effect.earlier_unresolved,
            "G2: missing fence or unresolved predecessor"
        );
        assert!(
            attempts.insert((&effect.attempt, effect.field.wire_name())),
            "G2: attempt repeated"
        );
        assert!(
            calls.insert((effect.approving_call, effect.field.wire_name())),
            "G2: approving call repeated field"
        );
    }
    for claim in saved {
        assert_eq!(&claim.identity, preview, "G3: wrong readback target");
        assert!(
            selected
                .iter()
                .any(|(field, target, desired)| *field == claim.field
                    && *target == claim.target
                    && *desired == RecordValue::Known(claim.intended.clone())),
            "G3: unsent or misbound physical field claimed saved"
        );
        assert_eq!(
            claim.readback,
            RecordValue::Known(claim.intended.clone()),
            "G3: unequal or unknown readback"
        );
    }
}
#[test]
fn b2_save_outcome_rows() {
    let input = include_str!("fixtures/record-answers.json");
    let fixture = RecordFixture::from_json(input).unwrap();
    let value: serde_json::Value = serde_json::from_str(input).unwrap();
    for (name, index, wrong_identity, contract_row) in [
        ("B-read-failed", 6, false, "R05"),
        ("B-target-identity", 0, true, "R05"),
    ] {
        let item = &value["answers"][index];
        let mut identity: RecordIdentity =
            serde_json::from_value(item["identity"].clone()).unwrap();
        let preview = identity.clone();
        if wrong_identity {
            identity.student_key = "1003".into();
        }
        let capture = codec::parse(item["capture"].to_string().as_bytes()).unwrap();
        let result = decode_record_answer(&capture, identity);
        if wrong_identity {
            assert!(result.is_err(), "misbound read admitted");
        } else {
            assert!(matches!(
                result.unwrap().snapshot.observation,
                RecordObservation::Failed { .. }
            ));
        }
        // This decoder has no mutation/backend or saved-status API. Both actual
        // read-only results therefore produce empty effect/confirmation traces.
        check(&preview, &[], &[], &[]);
        assert_eq!(
            contract_row, "R05",
            "absent fence cannot classify an applied attempt"
        );
        assert!(name.starts_with("B-"));
    }
    assert!(fixture.seed.read_scripts.is_empty());
}
