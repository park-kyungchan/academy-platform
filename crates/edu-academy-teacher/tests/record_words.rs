use edu_academy_teacher::{
    display_labels as l, envelope::*, labels, record_data::*, record_words::*,
};
use serde_json::json;

fn question(field: AnswerField, question: &str, choices: Vec<QuestionChoice>) -> RecordQuestion {
    RecordQuestion {
        field,
        question: question.into(),
        choices,
    }
}
fn choice(label: &str, answer: AnswerValue) -> QuestionChoice {
    QuestionChoice {
        label: label.into(),
        answer,
    }
}

#[test]
fn merged_questions_round_trip() {
    let assessment = labels::ASSESSMENT_LABELS
        .iter()
        .filter(|e| e.field == "daily_test")
        .map(|e| choice(e.label, AnswerValue::Text(e.label.into())))
        .collect();
    let questions = vec![
        question(
            AnswerField::DailyTest,
            "일일시험 평가를 어떻게 기록할까요?",
            assessment,
        ),
        question(
            AnswerField::Memo,
            "메모가 있으면 문장을 그대로 답해 주세요.",
            vec![choice("없음", AnswerValue::Text("없음".into()))],
        ),
        question(
            AnswerField::Progress,
            "진도의 “좀”은 어떤 뜻인가요?",
            vec![],
        ),
        question(
            AnswerField::Homework,
            "숙제의 교재를 확인해 주세요.",
            vec![choice(
                "판본A 문장",
                AnswerValue::Lines(vec![
                    "가상교재 판본A 2-1 필수예제 8문항".into(),
                    "변경하지 않은 다른 숙제 줄".into(),
                ]),
            )],
        ),
        question(
            AnswerField::Progress,
            "진도의 “나머지”는 어떤 범위인가요?",
            vec![],
        ),
    ];
    let set = AskSet::new(questions.clone()).unwrap();
    let display = set.display();
    for field in ["일일시험 평가", "메모", "진도", "숙제"] {
        assert_eq!(display.matches(&format!("{field} 선택")).count(), 1);
    }
    assert!(
        !display.contains("출석 선택"),
        "settled fields must not reappear"
    );
    assert!(display.contains("“좀”"));
    assert!(display.contains("“나머지”"));
    assert!(display.contains(
        "일일시험 평가 선택\n일일시험 평가를 어떻게 기록할까요?\n- 미확인\n- 미실시\n- 10점"
    ));
    assert!(display.contains("메모 선택\n메모가 있으면 문장을 그대로 답해 주세요.\n- 없음"));
    let serialized = serde_json::to_value(&set).unwrap();
    let decoded: Vec<RecordQuestion> = serde_json::from_value(serialized).unwrap();
    assert_eq!(decoded, questions);
    assert_eq!(AskSet::new(decoded).unwrap(), set);
    assert!(AskSet::new(vec![]).is_err());
    assert!(
        AskSet::new(vec![question(
            AnswerField::Progress,
            "진도 문장",
            vec![choice("잘못된 형식", AnswerValue::Text("8문항".into()))]
        )])
        .is_err()
    );
}

#[test]
fn known_blank_is_not_unknown() {
    use edu_academy_core::workflow::review::valid_value;
    let blanks = RecordFields {
        progress: "",
        homework: "",
        lesson_memo: "",
        attendance: " ",
        daily_test_assessment: "-1",
        previous_homework_assessment: "-1",
    };
    for field in RecordField::ALL {
        let known = RecordValue::Known(blanks[field].into());
        assert!(valid_value(field.wire_name(), &blanks[field].into()));
        assert_ne!(
            l::record_value(field, &known),
            l::record_value(field, &RecordValue::Unknown)
        );
        assert_eq!(
            l::record_value(field, &RecordValue::Unknown),
            "미확인(원천 값 미확인)"
        );
        assert_eq!(known, RecordValue::Known(blanks[field].into()));
    }
    for (wire, expected) in [("Y", "출석"), ("N", "결석"), ("L", "지각"), (" ", "미입력")]
    {
        assert_eq!(
            l::record_value(RecordField::Attendance, &RecordValue::Known(wire.into())),
            expected
        );
    }
    assert_eq!(
        l::record_value(
            RecordField::DailyTestAssessment,
            &RecordValue::Known("-1".into())
        ),
        "미확인"
    );
    assert_eq!(
        l::record_value(
            RecordField::PreviousHomeworkAssessment,
            &RecordValue::Known("4".into())
        ),
        "A"
    );
    assert_eq!(labels::wire_code("prev_homework", "미실시"), Some(0));
    assert_eq!(
        l::record_value(
            RecordField::DailyTestAssessment,
            &RecordValue::Known("999".into())
        ),
        "999"
    );
    assert!(!valid_value("dailyTestAssessment", &"999".into()));
    assert_eq!(l::record_disposition(FieldDisposition::Deferred), "보류");
    assert_eq!(
        l::record_disposition(FieldDisposition::Unresolved),
        "미확인(입력 필요)"
    );
}

#[test]
fn old_lesson_words_and_empty_stage() {
    use chrono::NaiveDate;
    use edu_academy_teacher::{
        backend::SourceBackend,
        facts::SeededFacts,
        lesson::PrepareRequest,
        synthetic::{NOW, load_scenario},
    };
    assert_eq!(l::lesson_stage_name(""), "");
    assert_eq!(l::lesson_stage_name("solving"), "풀이");
    assert_eq!(l::lesson_stage_name("grading"), "채점");
    assert_eq!(l::lesson_stage_name("correction"), "오답수정");
    assert_eq!(l::lesson_stage_name("unlisted"), l::UNSUPPORTED_STAGE);
    let scenario = load_scenario(include_str!("fixtures/scenario-v2.json")).unwrap();
    let facts = SeededFacts::from_scenario(&scenario);
    let mut backend = SourceBackend::synthetic(scenario, NOW);
    let output = edu_academy_teacher::prepare(
        &mut backend,
        &facts,
        &PrepareRequest {
            student: Some("가상학생A".into()),
            ..Default::default()
        },
        NaiveDate::from_ymd_opt(2026, 11, 11).unwrap(),
    );
    assert_eq!(output.display, include_str!("fixtures/display-q01.txt"));
    assert!(
        !serde_json::to_value(output)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("questions")
    );
}

#[test]
fn record_envelope_compatibility() {
    let old = Ask {
        field: AskField::Date,
        question_ko: "날짜를 알려주세요.".into(),
        options: vec![AskOption {
            value: "2026-11-11".into(),
            label_ko: "오늘".into(),
            meaning_ko: "오늘 날짜".into(),
        }],
    };
    let envelope = Envelope::ask(
        "날짜를 알려주세요.".into(),
        json!({}),
        old.clone(),
        Coverage::default(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&envelope).unwrap(),
        json!({
            "status":"ask", "display":"날짜를 알려주세요.\n- 오늘: 오늘 날짜", "data":{},
            "ask":{"field":"date", "question_ko":"날짜를 알려주세요.", "options":[{"value":"2026-11-11","label_ko":"오늘","meaning_ko":"오늘 날짜"}]},
            "next":null, "stop":null, "coverage":{"sources":[],"unknowns":[]}
        })
    );
    let mut empty = old;
    empty.options.clear();
    assert!(Envelope::ask("".into(), json!({}), empty, Coverage::default()).is_err());
    let set = AskSet::new(vec![
        question(
            AnswerField::Homework,
            "숙제 문장을 그대로 답해 주세요.",
            vec![choice(
                "전체 숙제",
                AnswerValue::Lines(vec!["가상 숙제 줄".into()]),
            )],
        ),
        question(AnswerField::Memo, "메모 문장을 그대로 답해 주세요.", vec![]),
        question(
            AnswerField::Replace,
            "교체할까요?",
            vec![
                choice("교체", AnswerValue::Boolean(true)),
                choice("유지", AnswerValue::Boolean(false)),
            ],
        ),
    ])
    .unwrap();
    let envelope = Envelope::record_ask(
        "가상학생A 입력 확인".into(),
        json!({"kept":"data"}),
        set.clone(),
        Coverage::default(),
    )
    .unwrap();
    assert_eq!(envelope.status, Status::Ask);
    assert!(envelope.next.is_none() && envelope.stop.is_none());
    assert_eq!(envelope.questions, set.questions());
    assert_eq!(envelope.ask.as_ref().unwrap().field, AskField::Homework);
    assert_eq!(
        envelope.ask.as_ref().unwrap().options[0].value,
        "[\"가상 숙제 줄\"]"
    );
    assert_eq!(
        envelope.display,
        format!("가상학생A 입력 확인\n{}", set.display())
    );
    let encoded = serde_json::to_value(envelope).unwrap();
    let decoded: Vec<RecordQuestion> =
        serde_json::from_value(encoded["questions"].clone()).unwrap();
    assert_eq!(decoded, set.questions());
    assert_eq!(encoded["data"], json!({"kept":"data"}));
    for (field, choices) in [
        (AnswerField::Memo, vec![]),
        (
            AnswerField::Restart,
            vec![choice("새로 시작", AnswerValue::Boolean(true))],
        ),
    ] {
        let set = AskSet::new(vec![question(field, "입력을 확인해 주세요.", choices)]).unwrap();
        let result = Envelope::record_ask("".into(), json!({}), set, Coverage::default()).unwrap();
        assert_eq!(result.ask.unwrap().field, AskField::from(field));
    }
}

#[test]
fn approval_words_is_lexical_only() {
    for words in [
        "저장",
        "저장해",
        "저장해줘",
        "저장해주세요",
        "저장하세요",
        "저장하자",
        "응",
        "네",
        "예",
        "그래",
        "좋아",
        "맞아",
        "ㅇㅇ",
        "오케이",
        "  저장해줘  ",
    ] {
        assert!(approval_words(words));
    }
    for words in [
        "",
        "저장하지 마",
        "안 저장해",
        "저장 말아",
        "숙제만 바꿔 저장해",
        "저장해도 돼?",
        "맞으면 저장해",
        "네 수정해",
        "추가하고 저장",
        "저장해줘?",
        "응 근데 빼줘",
    ] {
        assert!(!approval_words(words));
    }
}
