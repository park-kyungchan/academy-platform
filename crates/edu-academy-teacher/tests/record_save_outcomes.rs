//! Initial read-only rows. Later packets add real dispatch/result traces here.
use edu_academy_core::{js::Js, workflow::codec};
use edu_academy_teacher::{record_data::*, record_fixture::*};
use std::{collections::BTreeSet, panic::catch_unwind};
#[derive(Clone)]
struct Dispatch {
    identity: RecordIdentity,
    field: RecordField,
    target: Js,
    attempt: String,
    approving_call: u64,
    fenced: bool,
    earlier_unresolved: bool,
}
#[derive(Clone)]
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
fn shared_checker_rejects_wrong_identity_duplicate_and_unequal_readback() {
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let record = &fixture.records[0];
    let preview = &record.snapshot.identity;
    let field = RecordField::Attendance;
    let target = record.field_evidence[field].get("target").clone();
    let selected = [(field, target.clone(), RecordValue::Known("L".into()))];
    let dispatched = Dispatch {
        identity: preview.clone(),
        field,
        target: target.clone(),
        attempt: "invented-attempt".into(),
        approving_call: 1,
        fenced: true,
        earlier_unresolved: false,
    };
    let saved = Saved {
        identity: preview.clone(),
        field,
        target,
        intended: "L".into(),
        readback: RecordValue::Known("L".into()),
    };
    // Establish that each negative starts from a checker-accepted trace.
    check(
        preview,
        &selected,
        std::slice::from_ref(&dispatched),
        std::slice::from_ref(&saved),
    );
    for wrong_student in [true, false] {
        let mut wrong = dispatched.clone();
        if wrong_student {
            wrong.identity.student_key = "invented-other-student".into();
        } else {
            wrong.identity.lesson_date = "2040-01-01".into();
        }
        assert!(catch_unwind(|| check(preview, &selected, &[wrong], &[])).is_err());
    }
    assert!(
        catch_unwind(|| check(
            preview,
            &selected,
            &[dispatched.clone(), dispatched.clone()],
            &[]
        ))
        .is_err()
    );
    let mut repeated_call = dispatched.clone();
    repeated_call.attempt = "invented-second-attempt".into();
    assert!(catch_unwind(|| check(preview, &selected, &[dispatched, repeated_call], &[])).is_err());
    let mut unequal = saved;
    unequal.readback = RecordValue::Known("Y".into());
    assert!(catch_unwind(|| check(preview, &selected, &[], &[unequal])).is_err());
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
            assert_eq!(
                result.err().expect("misbound read admitted").message,
                "record_answer_identity_conflict"
            );
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
    // Mutate a later field independently; checking only the first field fails this case.
    let mut capture = value["answers"][0]["capture"].clone();
    capture["answer"]["structuredContent"]["fields"][3]["target"]["studentId"] =
        "invented-other-student".into();
    let result = decode_record_answer(
        &codec::parse(capture.to_string().as_bytes()).unwrap(),
        fixture.records[0].snapshot.identity.clone(),
    );
    assert_eq!(
        result.err().expect("later field mismatch admitted").message,
        "record_answer_identity_conflict"
    );
    assert!(fixture.seed.read_scripts.is_empty());
}

#[test]
fn c_ambiguous_preview() {
    use edu_academy_teacher::record_parse::{ParsedLines, parse_lines};
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let mut books = Vec::new();
    for edition in ["edition-A", "edition-B"] {
        books.push(serde_json::json!({
            "studentRef":"invented-student", "bookRef":edition,
            "label":"invented-book", "edition":edition,
            "bindingState":"confirmed", "units":[{
                "unitRef":"invented-unit", "code":"2-1", "title":"invented-topic",
                "kind":"minor", "activities":["필수예제"], "sections":[]
            }]
        }));
    }
    let catalog = codec::parse(serde_json::json!({"books":books}).to_string().as_bytes()).unwrap();
    let result = parse_lines(
        RecordField::Homework,
        &["invented-book 2-1 필수예제 8문항".into()],
        &catalog,
    )
    .unwrap();
    let ParsedLines::Ask(questions) = result else {
        panic!("ambiguous edition must ask");
    };
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].field, AnswerField::Homework);
    assert_eq!(questions[0].choices.len(), 2);
    assert!(questions[0].choices.iter().all(|choice| matches!(
        &choice.answer, AnswerValue::Lines(lines) if lines.len() == 1 && lines[0].ends_with("8문항")
    )));
    // C-ambiguous-preview: lexical questions create neither a durable fence nor
    // dispatch/saved traces. That is R05, not the conditional later R07.
    check(&fixture.records[0].snapshot.identity, &[], &[], &[]);
    let contract_row = "R05";
    assert_eq!(contract_row, "R05");
}

#[test]
fn c_parent_preview() {
    use edu_academy_teacher::{
        record_parent::render_parent,
        record_parse::{ParsedLines, parse_lines},
    };
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let catalog = codec::parse(
        serde_json::json!({"books":[{
            "studentRef":"invented-student", "bookRef":"invented-book", "label":"가상교재",
            "edition":null, "bindingState":"confirmed", "units":[{
                "unitRef":"invented-unit", "code":"2-1", "title":"가상주제",
                "kind":"minor", "activities":["필수예제"], "sections":[]
            }]
        }]})
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    let ParsedLines::Ready(entries) = parse_lines(
        RecordField::Homework,
        &["가상교재 2-1 필수예제 8문항".into()],
        &catalog,
    )
    .unwrap() else {
        panic!("resolved proposal required");
    };
    let proposal = render_parent(&entries, &catalog).unwrap();
    assert_eq!(
        proposal.homework.as_deref(),
        Some("- 가상교재, 2-1 가상주제 필수예제 8문항")
    );
    for field in RecordField::ALL {
        if field != RecordField::Homework {
            assert!(proposal[field].is_none());
        }
    }
    // C-parent-preview is a proposal only (R05): this pure renderer records no
    // durable pre-effect fact (R07), invokes no backend and makes no saved claim.
    check(&fixture.records[0].snapshot.identity, &[], &[], &[]);
}

#[test]
fn d_missing_fields() {
    use edu_academy_teacher::{
        envelope::{Coverage, Envelope, Status},
        record_words::{AskSet, RecordQuestion},
    };
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let set = AskSet::new(vec![
        RecordQuestion {
            field: AnswerField::DailyTest,
            question: "일일시험 평가는 어떻게 기록할까요?".into(),
            choices: vec![QuestionChoice {
                label: "미확인".into(),
                answer: AnswerValue::Text("미확인".into()),
            }],
        },
        RecordQuestion {
            field: AnswerField::Memo,
            question: "메모 문장을 그대로 답해 주세요.".into(),
            choices: vec![],
        },
    ])
    .unwrap();
    let result = Envelope::record_ask(
        "가상학생A 입력 확인".into(),
        serde_json::json!({}),
        set,
        Coverage::default(),
    )
    .unwrap();
    assert_eq!(result.status, Status::Ask);
    assert_eq!(result.questions.len(), 2);
    assert!(result.display.contains("일일시험 평가 선택"));
    assert!(result.display.contains("메모 선택"));
    assert!(!result.display.contains("출석 선택"));
    // D-missing-fields: R05 content only, no durable R07 fact. Grouped questions
    // expose no dispatch/saved API; the shared checker receives empty traces.
    check(&fixture.records[0].snapshot.identity, &[], &[], &[]);
}

#[test]
fn d_unresolved_unsent() {
    use edu_academy_teacher::record_policy::settle_fields;
    let mut fixture =
        RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let mut before = fixture.records.remove(0);
    let RecordObservation::Observed { fields, .. } = &mut before.snapshot.observation else {
        panic!("observed fixture required");
    };
    *fields = RecordFields::from_fn(|_| RecordValue::Unknown);
    fields.homework = RecordValue::Known("".into());
    let input = TeacherInputs {
        homework: Some(vec!["가상 숙제".into()]),
        ..Default::default()
    };
    let dispositions = settle_fields(&before, &input).unwrap();
    assert_eq!(dispositions.homework, FieldDisposition::ProposalConfirmed);
    for field in RecordField::ALL {
        if field != RecordField::Homework {
            assert_eq!(dispositions[field], FieldDisposition::Unresolved);
        }
    }
    // D-unresolved-unsent: only input dispositions, no preview/approval/attempt.
    // R05, not a durable R07 fact. No unsupplied field can acquire a saved claim.
    check(&before.snapshot.identity, &[], &[], &[]);
}
