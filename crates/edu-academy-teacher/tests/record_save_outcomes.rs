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

#[test]
fn c_core_target_refusal_rows() {
    use edu_academy_core::{
        js::{obj, sha},
        workflow::{
            Runtime,
            approval::{Approval, RequestApproval},
            state,
            store::{Kind, Store},
        },
    };
    struct Clock;
    impl Runtime for Clock {
        fn now(&self) -> String {
            "2040-01-04T12:00:00.000Z".into()
        }
        fn epoch_ms(&self) -> u64 {
            0
        }
        fn uuid(&self) -> edu_academy_core::js::Result<String> {
            Ok("invented-refusal".into())
        }
    }
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let record = &fixture.records[0];
    let preview = &record.snapshot.identity;
    let root = std::path::PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("core-checker-{}", std::process::id()));
    edu_academy_core::workflow::store::mkdir(&root).unwrap();
    let scope = obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "teacher".into()),
        ("studentId", "student".into()),
        ("lessonDate", "2040-01-04".into()),
    ]);
    for (name, code, row) in [
        (
            "C-core-target-missing",
            "workflow_effect_target_required",
            "R05",
        ),
        ("C-core-target-drift", "workflow_effect_target_drift", "R07"),
        (
            "C-core-legacy-unbound",
            "workflow_legacy_execution_unbound",
            "R37/R09",
        ),
    ] {
        let store = Store::open(&root.join(name), &root).unwrap();
        let reviewed = obj(&[
            ("field", "progress".into()),
            ("effectRecordKey", "physical".into()),
            ("effectTarget", "reviewed-target".into()),
            ("before", "Before".into()),
            ("after", "After".into()),
        ]);
        let mut action = reviewed.clone();
        action.set(
            "target",
            obj(&[
                ("teacherKey", "teacher".into()),
                ("studentKey", "student".into()),
                ("date", "2040-01-04".into()),
            ]),
        );
        if row == "R05" {
            action = obj(&[
                ("field", "progress".into()),
                ("effectRecordKey", "physical".into()),
            ]);
        }
        if row == "R07" {
            action.set("effectTarget", "different-target".into());
        }
        let legacy = obj(&[
            ("scope", scope.clone()),
            ("requestRef", "older".into()),
            ("field", "progress".into()),
            ("effectRecordKey", "physical".into()),
            ("beforeSha256", sha("Before").into()),
            ("afterSha256", sha("After").into()),
        ]);
        if row == "R37/R09" {
            store
                .add_ledger(&state::lock_key(&scope).unwrap(), "legacy", &Clock)
                .unwrap();
            store
                .publish(Kind::Execution, "legacy", &legacy, &Clock)
                .unwrap();
        }
        let mut approval = RequestApproval {
            store: &store,
            runtime: &Clock,
            input: obj(&[
                ("scope", scope.clone()),
                ("requestRef", "new".into()),
                ("reviewed", Js::Arr(vec![reviewed])),
            ]),
        };
        // This is the actual synthetic effect boundary: only verified approval
        // could append a dispatch. The error is the returned core result.
        let result = approval.consume(&action);
        let mut dispatches = vec![];
        if result
            .as_ref()
            .is_ok_and(|r| r.get("status").is("verified"))
        {
            dispatches.push(Dispatch {
                identity: preview.clone(),
                field: RecordField::Progress,
                target: record.field_evidence[RecordField::Progress]
                    .get("target")
                    .clone(),
                attempt: name.into(),
                approving_call: 1,
                fenced: true,
                earlier_unresolved: false,
            });
        }
        assert_eq!(result.unwrap_err().message, code, "{name}/{row}");
        assert!(dispatches.is_empty());
        check(preview, &[], &dispatches, &[]);
        let executions = state::student_executions(&store, &scope).unwrap();
        assert_eq!(executions.len(), usize::from(row == "R37/R09"));
        if row == "R37/R09" {
            assert_eq!(executions[0].record, legacy);
            assert!(executions[0].state.needs_reconciliation());
            assert!(store.get(Kind::Reconciliation, "legacy").unwrap().absent());
            let status_input = obj(&[("scope", scope.clone())]);
            let before =
                edu_academy_core::workflow::service::status(&store, &status_input).unwrap();
            // C-core-facts-unqualified: absent legacy facts grant no proof.
            assert!(
                !before.get("fields").array()[0].get("executions").array()[0]
                    .has("completionFacts")
            );
            let facts = obj(&[("claim", "unqualified".into())]);
            store
                .publish(
                    Kind::Outcome,
                    "legacy",
                    &obj(&[
                        ("status", "unresolved".into()),
                        ("completionFacts", facts.clone()),
                    ]),
                    &Clock,
                )
                .unwrap();
            // C-core-facts-roundtrip: actual status reads the same Outcome.
            let after = edu_academy_core::workflow::service::status(&store, &status_input).unwrap();
            assert_eq!(
                after.get("fields").array()[0].get("executions").array()[0].get("completionFacts"),
                &facts
            );
            assert!(
                state::field_state(&store, "legacy")
                    .unwrap()
                    .needs_reconciliation()
            );
            check(preview, &[], &dispatches, &[]);
        }
    }
}

#[test]
fn e_read_failed() {
    use edu_academy_core::{js::obj, workflow::service::Source};
    use edu_academy_teacher::{
        record_backend::SyntheticRecordState, record_effect::SyntheticCoreSource,
    };
    use std::sync::{Arc, Mutex};
    let mut fixture =
        RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let failed = fixture.records[4].clone();
    let mut scope = fixture.records[0].answer.get("scope").clone();
    scope.set(
        "studentId",
        failed.snapshot.identity.student_key.clone().into(),
    );
    scope.set(
        "lessonDate",
        failed.snapshot.identity.lesson_date.clone().into(),
    );
    assert!(
        !fixture
            .seed
            .records
            .iter()
            .any(|r| r.identity == failed.snapshot.identity)
    );
    fixture.records = vec![failed.clone()];
    let root = std::path::PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("source-row-{}", std::process::id()));
    let state = Arc::new(Mutex::new(
        SyntheticRecordState::open(&root, fixture.seed, 10000).unwrap(),
    ));
    let mut source = SyntheticCoreSource {
        state: Arc::clone(&state),
        records: fixture.records,
    };
    let result = source
        .read(&scope, &"invented-read".into(), &obj(&[]))
        .unwrap();
    assert!(result.get("status").is("unavailable"));
    assert!(result.get("reasonCode").is("student_name_not_found"));
    assert_eq!(result, failed.answer);
    assert!(!result.has("observation"));
    assert_eq!(state.lock().unwrap().read_count(), 0);
    assert_eq!(state.lock().unwrap().dispatch_count(), 0);
    // E-read-failed, R05: actual recorded failure, no inferred absence, no
    // pre-effect record, dispatch or saved claim. No save actor is constructed.
    check(&failed.snapshot.identity, &[], &[], &[]);
}
