use edu_academy_core::{
    js::{Fault, Js, Result},
    workflow::{Runtime, codec, review::Policy},
};
use edu_academy_teacher::{
    facts::SeededFacts,
    record_data::*,
    record_fixture::*,
    record_parent::render_parent,
    record_parse::{ParsedLines, parse_lines},
    record_policy::*,
    record_words::{AskSet, RecordQuestion},
};
use serde_json::{Value, json};
fn js(value: Value) -> Js {
    codec::parse(value.to_string().as_bytes()).unwrap()
}
fn before() -> AnswerRecord {
    let mut fixture =
        RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let mut before = fixture.records.remove(0);
    let RecordObservation::Observed { fields, .. } = &mut before.snapshot.observation else {
        panic!("observed fixture required");
    };
    *fields = RecordFields::from_fn(|field| {
        RecordValue::Known(
            match field {
                RecordField::Attendance => " ",
                RecordField::DailyTestAssessment | RecordField::PreviousHomeworkAssessment => "-1",
                _ => "",
            }
            .into(),
        )
    });
    before
}
fn fields(before: &mut AnswerRecord) -> &mut RecordFields<RecordValue> {
    let RecordObservation::Observed { fields, .. } = &mut before.snapshot.observation else {
        panic!("observed fixture required");
    };
    fields
}
fn catalog() -> Js {
    js(json!({"books":[{
        "studentRef":"77001", "bookRef":"invented-book", "label":"가상교재", "edition":null, "bindingState":"confirmed",
        "units":[{"unitRef":"invented-unit", "code":"2-1", "title":"가상주제", "kind":"minor", "activities":["필수예제"], "sections":[]}]
    }]}))
}
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        "2040-01-04T12:00:00.000Z".into()
    }
    fn epoch_ms(&self) -> u64 {
        0
    }
    fn uuid(&self) -> Result<String> {
        Err(Fault::error("policy_review_must_not_publish"))
    }
}
// Reuse core-owned, wholly invented observation/context data. This is a pure
// review fixture, not a source/backend adapter or future Service placeholder.
fn review_input(value: &str, fact: &str, deadline: Option<&str>) -> (Js, Js) {
    let cases: Value = serde_json::from_str(include_str!(
        "../../edu-academy-core/tests/fixtures/workflow-store-review-fixes-v2/CASES.json"
    ))
    .unwrap();
    let mut input = cases[0]["input"].clone();
    input["observationHandle"] = json!("synthetic:held-observation");
    input["context"]["facts"][0]["text"] = json!(fact);
    input["proposals"][0]["value"] = json!(value);
    input["proposals"][0]["learningEntries"] = json!([{
        "bookRef":"invented-book", "unitRef":"invented-unit", "scope":"unit", "sectionRefs":[], "activity":"필수예제", "deadline":deadline
    }]);
    let mut observation = js(cases[0]["read"]["observation"].clone());
    let mut fields = observation.get("fields").clone();
    for field in RecordField::ALL {
        let target = observation.get(if field == RecordField::PreviousHomeworkAssessment {
            "previousTarget"
        } else {
            "target"
        });
        if target.absent() {
            continue;
        }
        let mut value = fields.get(field.wire_name()).clone();
        value.set(
            "effectTarget",
            edu_academy_teacher::record_effect::canonical_effect_target(target)
                .unwrap()
                .into(),
        );
        value.set("effectRecordKey", target.get("recordId").clone());
        fields.set(field.wire_name(), value);
    }
    observation.set("fields", fields);
    (js(input), observation)
}

#[test]
fn missing_labels_memo_and_patch_dispositions() {
    use FieldDisposition::*;
    let mut before = before();
    let mut input = TeacherInputs {
        progress: Some(vec!["가상 진도".into()]),
        homework: Some(vec!["가상 숙제".into()]),
        attendance: Some("출석".into()),
        prev_homework: Some("A".into()),
        ..Default::default()
    };
    let result = settle_fields(&before, &input).unwrap();
    for field in [
        RecordField::Progress,
        RecordField::Homework,
        RecordField::Attendance,
        RecordField::PreviousHomeworkAssessment,
    ] {
        assert_eq!(result[field], ProposalConfirmed);
    }
    assert_eq!(result.lesson_memo, Unresolved);
    assert_eq!(
        result.daily_test_assessment, Unresolved,
        "narrative never supplies an assessment"
    );
    assert_eq!(wire_value(RecordField::Attendance, "출석").unwrap(), "Y");
    assert_eq!(
        wire_value(RecordField::PreviousHomeworkAssessment, "A").unwrap(),
        "4"
    );
    fields(&mut before).lesson_memo = RecordValue::Known("기존 가상 메모".into());
    assert_eq!(
        settle_fields(&before, &input).unwrap().lesson_memo,
        ConfirmedExisting
    );
    input.memo = Some("없음".into());
    assert_eq!(
        settle_fields(&before, &input).unwrap().lesson_memo,
        NoAddition
    );
    input.memo = Some("".into());
    assert_eq!(
        settle_fields(&before, &input).unwrap().lesson_memo,
        ProposalConfirmed
    );
    input.memo = Some("새 가상 메모".into());
    assert_eq!(
        settle_fields(&before, &input).unwrap().lesson_memo,
        Unresolved
    );
    input.replace = Some(false);
    assert_eq!(
        settle_fields(&before, &input).unwrap().lesson_memo,
        ConfirmedExisting
    );
    input.replace = Some(true);
    assert_eq!(
        settle_fields(&before, &input).unwrap().lesson_memo,
        ProposalConfirmed
    );
    assert_eq!(
        fields(&mut before).lesson_memo,
        RecordValue::Known("기존 가상 메모".into())
    );
    fields(&mut before).lesson_memo = RecordValue::Unknown;
    input.memo = Some("없음".into());
    assert_eq!(
        settle_fields(&before, &input).unwrap().lesson_memo,
        NoAddition
    );
    assert_eq!(fields(&mut before).lesson_memo, RecordValue::Unknown);

    // D-words carry: correct answer shape is not a valid assessment value.
    let choice = QuestionChoice {
        label: "잘못된 평가".into(),
        answer: AnswerValue::Text("999".into()),
    };
    AskSet::new(vec![RecordQuestion {
        field: AnswerField::DailyTest,
        question: "평가를 알려주세요.".into(),
        choices: vec![choice.clone()],
    }])
    .unwrap();
    let unchanged = input.clone();
    let refusal = apply_choice(&mut input, AnswerField::DailyTest, &choice.answer).unwrap_err();
    assert_eq!(refusal.message, "invalid_explicit_field_value");
    assert_eq!(
        input, unchanged,
        "refused choice must not enter teacher inputs"
    );
    apply_choice(
        &mut input,
        AnswerField::DailyTest,
        &AnswerValue::Text("미확인".into()),
    )
    .unwrap();
    assert_eq!(
        settle_fields(&before, &input)
            .unwrap()
            .daily_test_assessment,
        Deferred
    );
}

#[test]
fn deferred_assessment_and_previewed_subset() {
    use FieldDisposition::*;
    let mut before = before();
    fields(&mut before).progress = RecordValue::Known("기존 가상 진도 (legacy)".into());
    let input = TeacherInputs {
        homework: Some(vec!["가상 숙제".into()]),
        daily_test: Some("-1".into()),
        ..Default::default()
    };
    let result = settle_fields(&before, &input).unwrap();
    assert_eq!(result.progress, ConfirmedExisting);
    assert_eq!(result.homework, ProposalConfirmed);
    assert_eq!(result.daily_test_assessment, Deferred);
    assert_eq!(result.attendance, Unresolved);
    assert_eq!(result.lesson_memo, Unresolved);
    assert_eq!(result.previous_homework_assessment, Unresolved);
    assert_eq!(
        fields(&mut before).daily_test_assessment,
        RecordValue::Known("-1".into())
    );
    assert_eq!(
        fields(&mut before).progress,
        RecordValue::Known("기존 가상 진도 (legacy)".into())
    );
    // C-parent carry: local rendering accepts these literal supplied qualifiers.
    // Core independently rejects the count/date unsupported by the input facts.
    let catalog = catalog();
    let ParsedLines::Ready(entries) = parse_lines(
        RecordField::Progress,
        &["가상교재 2-1 필수예제 8문항 2040-01-05까지".into()],
        &catalog,
    )
    .unwrap() else {
        panic!("local mapping required");
    };
    let value = render_parent(&entries, &catalog).unwrap().progress.unwrap();
    let policy = RecordPolicy::new(SeededFacts::default(), catalog);
    assert!(policy.check_lines(&value.clone().into()));
    let (input, observation) =
        review_input(&value, "가상교재 2-1 필수예제 4문항", Some("2040-01-05"));
    let rejected = policy
        .review(&input, &observation, &observation, &Clock)
        .unwrap();
    let row = &rejected.get("rows").array()[0];
    assert!(
        row.get("blockers")
            .array()
            .iter()
            .any(|v| v.is("unsupported_question_count"))
    );
    assert!(
        row.get("blockers")
            .array()
            .iter()
            .any(|v| v.is("unsupported_deadline"))
    );
    assert!(rejected.get("patch").array().is_empty());
    let (input, observation) = review_input(
        &value,
        "가상교재 2-1 필수예제 8문항 2040-01-05까지",
        Some("2040-01-05"),
    );
    let accepted = policy
        .review(&input, &observation, &observation, &Clock)
        .unwrap();
    assert!(accepted.get("status").is("prepared"));
    assert_eq!(accepted.get("patch").array().len(), 1);
    assert!(accepted.get("execution").is("not_supported"));
    assert!(policy.excluded_parent_language(&"- 교사 확인".into()));
}

#[test]
fn previous_homework_targets_earlier_occurrence() {
    let mut before = before();
    let input = TeacherInputs {
        prev_homework: Some("A".into()),
        ..Default::default()
    };
    let evidence = before.field_evidence.previous_homework_assessment.clone();
    assert_eq!(
        settle_fields(&before, &input)
            .unwrap()
            .previous_homework_assessment,
        FieldDisposition::ProposalConfirmed
    );
    assert_eq!(before.field_evidence.previous_homework_assessment, evidence);
    let date = before.snapshot.identity.lesson_date.clone();
    let mut target = evidence.get("target").clone();
    target.set("lessonDate", date.into());
    before
        .field_evidence
        .previous_homework_assessment
        .set("target", target);
    assert_eq!(
        settle_fields(&before, &input).unwrap_err().message,
        "record_previous_target_unverified"
    );
    before.field_evidence.previous_homework_assessment = evidence.clone();
    let mut target = evidence.get("target").clone();
    target.set("studentId", "another-invented-student".into());
    before
        .field_evidence
        .previous_homework_assessment
        .set("target", target);
    assert!(settle_fields(&before, &input).is_err());
    before.field_evidence.previous_homework_assessment = evidence;
    before.snapshot.observation = RecordObservation::Failed {
        reason_code: "invented-source-failure".into(),
    };
    assert_eq!(
        settle_fields(&before, &input).unwrap_err().message,
        "record_source_unavailable"
    );
}
