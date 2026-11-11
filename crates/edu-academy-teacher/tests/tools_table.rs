use edu_academy_teacher::{
    envelope::{Ask, AskField, AskOption, Envelope, Status, Stop, StopCode},
    lesson::LessonData,
    render::{Verdict, display_lint},
    tools::{TOOLS, dispatch, docs_table, tools_json, tools_list_minified},
};

const FIXTURE: &[u8] = include_bytes!("fixtures/tools-list.json");
const DISPLAY_Q01: &str = include_str!("fixtures/display-q01.txt");
const DISPLAY_Q02: &str = include_str!("fixtures/display-q02.txt");

fn empty_data() -> serde_json::Value {
    serde_json::to_value(LessonData::default()).unwrap()
}

#[test]
fn tools_list_equals_fixture() {
    assert_eq!(tools_list_minified().as_bytes(), FIXTURE);
    let expected: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    assert_eq!(tools_json(), expected);
}

#[test]
fn tools_list_at_most_8000_bytes() {
    assert_eq!(tools_list_minified().len(), 7983);
    assert!(tools_list_minified().len() <= 8000);
}

#[test]
fn each_description_at_most_400_bytes_and_starts_korean() {
    for tool in TOOLS {
        assert!(tool.description.len() <= 400, "{}", tool.name);
        let first = tool.description.chars().next().unwrap();
        assert!(
            ('\u{AC00}'..='\u{D7A3}').contains(&first),
            "{} does not start with Korean",
            tool.name
        );
    }
}

#[test]
fn names_unique_and_14() {
    assert_eq!(TOOLS.len(), 14);
    let mut names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 14);
    assert!(!docs_table().is_empty());
}

#[test]
fn every_advertised_name_has_a_handler_and_unknown_name_refuses() {
    for tool in TOOLS {
        assert!(dispatch(tool.name).is_some(), "{}", tool.name);
    }
    assert!(dispatch("academy_no_such_tool").is_none());
    let unavailable: Vec<_> = TOOLS
        .iter()
        .filter(|t| t.name != "academy_lesson_prepare")
        .collect();
    assert_eq!(unavailable.len(), 13);
    for tool in unavailable {
        let envelope = dispatch(tool.name)
            .and_then(|h| h.unavailable_envelope())
            .unwrap();
        assert_eq!(envelope.status, Status::Unavailable);
        assert_eq!(
            envelope.stop.as_ref().unwrap().code,
            StopCode::ToolNotInSlice1
        );
        assert_eq!(
            envelope.stop.as_ref().unwrap().code.as_str(),
            "tool_not_in_slice_1"
        );
        assert!(!envelope.display.is_empty());
    }
}

#[test]
fn envelope_invariant_holds_for_every_constructor() {
    let ok = Envelope::ok("확인".into(), empty_data(), Default::default());
    assert!(ok.ask.is_none() && ok.next.is_none() && ok.stop.is_none());
    let unavailable = Envelope::unavailable_tool();
    assert_eq!(unavailable.status, Status::Unavailable);
    assert!(unavailable.stop.is_some());
    assert!(
        Envelope::new(
            Status::Ok,
            "x".into(),
            empty_data(),
            None,
            None,
            Some(Stop::new(StopCode::SourceUnavailable, "문장")),
            Default::default()
        )
        .is_err()
    );
    assert!(
        Envelope::new(
            Status::Unavailable,
            "x".into(),
            empty_data(),
            None,
            None,
            None,
            Default::default()
        )
        .is_err()
    );
}

// R7: an ask without options is a constructor error, never a panic.
#[test]
fn ask_without_options_is_a_constructor_error() {
    let ask = Ask {
        field: AskField::Student,
        question_ko: "어느 학생인가요?".into(),
        options: vec![],
    };
    assert!(
        Envelope::ask(
            "어느 학생인가요?".into(),
            empty_data(),
            ask,
            Default::default()
        )
        .is_err()
    );
    let ask = Ask {
        field: AskField::Student,
        question_ko: "어느 학생인가요?".into(),
        options: vec![AskOption {
            value: "가상학생A".into(),
            label_ko: "가상학생A".into(),
            meaning_ko: "그 학생".into(),
        }],
    };
    assert!(
        Envelope::ask(
            "어느 학생인가요?".into(),
            empty_data(),
            ask,
            Default::default()
        )
        .is_ok()
    );
}

#[test]
fn verdict_labels_are_the_single_table() {
    assert_eq!(Verdict::ALL.len(), 18);
    assert_eq!(Verdict::Found.label_ko(), "확인됨");
    assert_eq!(Verdict::Waiting.label_ko(), "대기: 업로드됨, 교사 확인 전");
    assert_eq!(
        Verdict::NotObservedInEitherSource.label_ko(),
        "미확인 (두 원천 모두 없음)"
    );
}

#[test]
fn display_lint_accepts_fixtures_and_rejects_codes() {
    display_lint(DISPLAY_Q01).unwrap();
    display_lint(DISPLAY_Q02).unwrap();
    assert!(display_lint("섹션: not_observed").is_err());
    assert!(display_lint("hello world test").is_err());
    display_lint("진도: 가우스 5-1-2 1-2 유형다지기").unwrap();
}
