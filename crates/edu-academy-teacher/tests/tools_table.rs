use edu_academy_teacher::{
    envelope::{Envelope, LessonData, Status},
    render::{Verdict, display_lint},
    tools::{TOOLS, dispatch, docs_table, tools_json, tools_list_minified},
};

const FIXTURE: &[u8] = include_bytes!("fixtures/tools-list.json");
const DISPLAY_Q01: &str = include_str!("fixtures/display-q01.txt");
const DISPLAY_Q02: &str = include_str!("fixtures/display-q02.txt");

#[test]
fn tools_list_equals_fixture() {
    // Exact bytes in fixture key order, and the same value as parsed JSON.
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
    // The 13 tools outside slice 1 answer unavailable/tool_not_in_slice_1.
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
        assert_eq!(envelope.stop.as_ref().unwrap().code, "tool_not_in_slice_1");
        assert!(!envelope.display.is_empty());
    }
}

#[test]
fn envelope_invariant_holds_for_every_constructor() {
    // ok: no carrier.
    let ok = Envelope::ok("확인".into(), LessonData::default(), Default::default());
    assert!(ok.ask.is_none() && ok.next.is_none() && ok.stop.is_none());
    // ask / stop constructors carry exactly one.
    let unavailable = Envelope::unavailable_tool();
    assert_eq!(unavailable.status, Status::Unavailable);
    assert!(unavailable.stop.is_some());
    // Direct constructor rejects violations.
    assert!(
        Envelope::new(
            Status::Ok,
            "x".into(),
            LessonData::default(),
            None,
            None,
            Some(edu_academy_teacher::envelope::Stop::new("c", "문장")),
            Default::default()
        )
        .is_err()
    );
    assert!(
        Envelope::new(
            Status::Unavailable,
            "x".into(),
            LessonData::default(),
            None,
            None,
            None,
            Default::default()
        )
        .is_err()
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
    // Book labels and unit codes are allowed.
    display_lint("진도: 가우스 5-1-2 1-2 유형다지기").unwrap();
}
