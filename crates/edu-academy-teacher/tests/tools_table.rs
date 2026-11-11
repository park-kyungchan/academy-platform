use edu_academy_teacher::{
    envelope::{Ask, AskField, AskOption, Envelope, Status, Stop, StopCode},
    labels::{ASSESSMENT_LABELS, label_for_code, wire_code},
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
    assert_eq!(tools_list_minified().len(), 7960);
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

// M10/M13: every advertised enum value maps to exactly one wire code and back.
#[test]
fn assessment_labels_map_every_advertised_value() {
    let write = TOOLS
        .iter()
        .find(|t| t.name == "academy_record_write")
        .unwrap();
    let schema: serde_json::Value = serde_json::from_str(write.input_schema).unwrap();
    for (field, expected_codes) in [
        ("daily_test", (-1..=10).collect::<Vec<i32>>()),
        ("prev_homework", (-1..=5).collect::<Vec<i32>>()),
    ] {
        let advertised: Vec<&str> = schema["properties"][field]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let mut codes = Vec::new();
        for label in &advertised {
            let code = wire_code(field, label).unwrap_or_else(|| panic!("{field} {label}"));
            assert!(!codes.contains(&code), "{field} {label} duplicates {code}");
            codes.push(code);
            assert_eq!(label_for_code(field, code), Some(*label));
        }
        assert_eq!(codes, expected_codes, "{field}");
    }
    // The one accepted alias: prev_homework 0 is also accepted as 미실시,
    // while daily_test 0's advertised label is 미실시 itself.
    assert_eq!(wire_code("prev_homework", "미실시"), Some(0));
    assert_eq!(label_for_code("prev_homework", 0), Some("미숙제"));
    assert_eq!(wire_code("daily_test", "미실시"), Some(0));
    assert_eq!(ASSESSMENT_LABELS.len(), 19);
    // Attendance labels stay as they are.
    let attendance: Vec<&str> = schema["properties"]["attendance"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(attendance, ["출석", "결석", "지각", "미입력"]);
}

#[test]
fn verdict_labels_are_the_single_table() {
    assert_eq!(Verdict::ALL.len(), 18);
    for verdict in Verdict::ALL {
        assert!(!verdict.label_ko().is_empty());
        assert_eq!(
            verdict.label_ko(),
            edu_academy_teacher::display_labels::verdict(verdict.code())
        );
    }
    assert_eq!(Verdict::Found.label_ko(), "서버 확인");
    assert_eq!(Verdict::Waiting.label_ko(), "미확인(업로드됨·교사 확인 전)");
    assert_eq!(
        Verdict::NotObservedInEitherSource.label_ko(),
        "미확인(두 원천 모두에 없음)"
    );
}

#[test]
fn structural_lint_and_server_vocabulary_are_separate() {
    display_lint(DISPLAY_Q01).unwrap();
    display_lint(DISPLAY_Q02).unwrap();
    assert!(edu_academy_teacher::render::vocabulary_lint("섹션: not_observed").is_err());
    assert!(edu_academy_teacher::render::vocabulary_lint("hello world test").is_err());
    display_lint("DailyTest 확인").unwrap();
    display_lint("진도: 가우스 5-1-2 1-2 유형다지기").unwrap();
}

#[test]
fn ask_option_without_meaning_has_no_colon() {
    let ask = Ask {
        field: AskField::Student,
        question_ko: "어느 학생인가요?".into(),
        options: vec![AskOption {
            value: "가상학생A".into(),
            label_ko: "가상학생A".into(),
            meaning_ko: String::new(),
        }],
    };
    let result = Envelope::ask(
        ask.question_ko.clone(),
        empty_data(),
        ask,
        Default::default(),
    )
    .unwrap();
    assert_eq!(result.display, "어느 학생인가요?\n- 가상학생A");
}

#[test]
fn unknown_tool_lists_only_implemented_slice() {
    assert_eq!(
        Envelope::unknown_tool().display,
        "그런 이름의 도구는 없습니다. 쓸 수 있는 도구: academy_lesson_prepare"
    );
}

#[test]
fn display_lint_checks_closing_heading_order_and_six_fields() {
    for text in [DISPLAY_Q01, DISPLAY_Q02] {
        display_lint(text).unwrap();
        assert!(
            display_lint(
                text.lines()
                    .take(text.lines().count() - 1)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .as_str()
            )
            .is_err()
        );
        assert!(display_lint(&text.replace("알림\n", "다음 확인\n")).is_err());
        let reversed = text
            .replacen("알림\n", "TEMP\n", 1)
            .replacen("학생별 할 일\n", "알림\n", 1)
            .replacen("TEMP\n", "학생별 할 일\n", 1);
        assert!(display_lint(&reversed).is_err());
        assert!(display_lint(&text.replacen("가상학생D | 오늘", "가상학생D 오늘", 1)).is_err());
        assert!(display_lint(&text.replacen(" | ", " / ", 1)).is_err());
    }
}

#[test]
fn server_owned_vocabulary_and_templates_do_not_leak_codes() {
    use edu_academy_teacher::{display_labels as l, render::vocabulary_lint};
    let source = include_str!("../src/display_labels.rs");
    let constants =
        regex::Regex::new(r#"(?s)pub const [A-Z_]+: &str =\s*"((?:\\.|[^"\\])*)""#).unwrap();
    let templates = regex::Regex::new(r#"(?s)format!\(\s*"((?:\\.|[^"\\])*)""#).unwrap();
    let slots = regex::Regex::new(r"\{[^}]*\}").unwrap();
    let mut checked = 0;
    for capture in constants
        .captures_iter(source)
        .chain(templates.captures_iter(source))
    {
        let text = slots.replace_all(&capture[1], "");
        vocabulary_lint(&text).unwrap();
        checked += 1;
    }
    assert!(checked > 90);
    for label in l::PARTS.into_iter().chain(l::UNITS) {
        vocabulary_lint(label).unwrap();
    }
    for verdict in Verdict::ALL {
        vocabulary_lint(verdict.label_ko()).unwrap();
    }
}
