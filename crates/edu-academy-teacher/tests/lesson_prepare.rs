use chrono::{NaiveDate, Weekday};
use edu_academy_teacher::{
    backend::SourceBackend,
    envelope::{AskField, Status, StopCode},
    facts::{ClassFact, SeededFacts, TeacherFacts},
    lesson::{LessonData, PrepareRequest, previous_lesson, scan_line},
    load_scenario,
    render::{Verdict, display_lint},
    synthetic::{NOW, Scenario, SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort},
};

const SCENARIO: &str = include_str!("fixtures/scenario-v1.json");
const DISPLAY_Q01: &str = include_str!("fixtures/display-q01.txt");
const DISPLAY_Q02: &str = include_str!("fixtures/display-q02.txt");
const DISPLAY_NOT_FOUND: &str = include_str!("fixtures/display-previous-not-found.txt");

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()
}
fn scenario() -> Scenario {
    load_scenario(SCENARIO).unwrap()
}
type SyntheticBackend =
    SourceBackend<SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort>;
fn make_backend() -> SyntheticBackend {
    SourceBackend::synthetic(scenario(), NOW)
}
fn facts() -> SeededFacts {
    SeededFacts::from_scenario(&scenario())
}
fn req_student(name: &str) -> PrepareRequest {
    PrepareRequest {
        student: Some(name.into()),
        ..Default::default()
    }
}
fn data(envelope: &edu_academy_teacher::envelope::Envelope) -> LessonData {
    serde_json::from_value(envelope.data.clone()).unwrap()
}

// Q01: S-017, S-019, S-026, S-024/S-025 carry-over; sections of 2-1 not observed.
#[test]
fn q01_single_student_previous_lesson_and_checks() {
    let mut backend = make_backend();
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ok);
    assert_eq!(envelope.display, DISPLAY_Q01);
    display_lint(&envelope.display).unwrap();
    assert!(
        envelope
            .display
            .contains("못 가져온 숙제: 가우스 5-1-2 1-2 유형다지기 8문항 (11월 5일 숙제, 판정 전)")
    );
    assert!(!envelope.display.contains("2026-11-04"));
    let parsed = data(&envelope);
    let student = &parsed.students[0];
    let prev = student.previous_lesson.as_ref().unwrap();
    assert_eq!(prev.date, "2026-11-09");
    assert_ne!(prev.date, "2026-11-04");
    assert_eq!(prev.how_found, "schedule");
    assert!(prev.progress.contains("1-2 유형다지기"));
    assert!(prev.homework.contains("2-1 필수예제"));
    assert_eq!(student.checks.len(), 3);
    assert_eq!(student.checks[0].verdict, Verdict::Waiting);
    assert_eq!(student.checks[1].verdict, Verdict::NotObserved);
    assert_eq!(student.checks[2].verdict, Verdict::NotObserved);
    assert_eq!(student.carry_over.len(), 1);
    assert_eq!(
        student.carry_over[0].homework_line,
        "가우스 5-1-2 1-2 유형다지기 8문항"
    );
    assert_eq!(student.carry_over[0].assigned_on, "2026-11-05");
    assert!(envelope.display.contains("교사 확인 전"));
    // Logical reads: 1 record probe + 1 prestudy + 1 bindings + 1 index.
    assert_eq!(backend.source_reads(), 4);
    // Coverage lists only the reads made, with the clock value as read time.
    let names: Vec<&str> = envelope
        .coverage
        .sources
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(names, ["daily_record", "prestudy", "textbook_sections"]);
    assert!(envelope.coverage.sources.iter().all(|s| s.read_at == NOW));
    assert!(
        envelope.coverage.unknowns.is_empty(),
        "{:?}",
        envelope.coverage.unknowns
    );
    // One index read for both section kinds: bindings 1 + index 3 port calls.
    assert_eq!(backend.textbook_port_calls(), 4);
    assert_eq!(backend.prestudy_port_calls(), 5);
}

// Q02: S-002 roster order, S-019..S-021, S-027 not observed (never not done).
#[test]
fn q02_class_of_three_roster_order() {
    let mut backend = make_backend();
    let req = PrepareRequest {
        class_name: Some("가상반A".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ok);
    assert_eq!(envelope.display, DISPLAY_Q02);
    display_lint(&envelope.display).unwrap();
    let parsed = data(&envelope);
    let names: Vec<&str> = parsed.students.iter().map(|s| s.student.as_str()).collect();
    assert_eq!(names, ["가상학생A", "가상학생B", "가상학생C"]);
    for student in &parsed.students {
        assert_eq!(student.previous_lesson.as_ref().unwrap().date, "2026-11-09");
    }
    let b = &parsed.students[1];
    assert_eq!(b.checks[0].verdict, Verdict::NotObserved);
    assert!(!envelope.display.contains("안 함"));
    assert!(!envelope.display.contains("하지 않"));
    assert_eq!(backend.source_reads(), 12);
    assert_eq!(envelope.coverage.sources.len(), 9);
}

#[test]
fn previous_lesson_uses_moved_to_date() {
    let mut backend = make_backend();
    let student = backend.resolve_student("가상학생A", None).unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    match previous_lesson(&mut backend, &facts(), &student, date) {
        edu_academy_teacher::lesson::PreviousOutcome::Found(found) => {
            assert_eq!(found.date, NaiveDate::from_ymd_opt(2026, 11, 5).unwrap());
            assert_eq!(found.how_found, "moved_lesson");
        }
        _ => panic!("expected found previous lesson"),
    }
}

struct NoScheduleFacts;
impl TeacherFacts for NoScheduleFacts {
    fn class(&self, _name: &str) -> Option<ClassFact> {
        Some(ClassFact {
            name: "무일정반".into(),
            weekdays: vec![],
            roster: vec![],
        })
    }
    fn moved_lessons(&self, _class_name: &str) -> &[edu_academy_teacher::facts::MovedLesson] {
        &[]
    }
}

#[test]
fn previous_lesson_day_scan_without_schedule() {
    struct EmptyFacts;
    impl TeacherFacts for EmptyFacts {
        fn class(&self, _name: &str) -> Option<ClassFact> {
            Some(ClassFact {
                name: "가상반A".into(),
                weekdays: Vec::new(),
                roster: vec![],
            })
        }
        fn moved_lessons(&self, _c: &str) -> &[edu_academy_teacher::facts::MovedLesson] {
            &[]
        }
    }
    let mut backend = make_backend();
    let student = backend.resolve_student("가상학생A", None).unwrap();
    match previous_lesson(&mut backend, &EmptyFacts, &student, today()) {
        edu_academy_teacher::lesson::PreviousOutcome::Found(found) => {
            assert_eq!(found.date, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
            assert_eq!(found.how_found, "day_scan");
        }
        _ => panic!("expected day-scan find"),
    }
    let _ = NoScheduleFacts;
    let _ = Weekday::Mon;
}

#[test]
fn previous_lesson_not_found_states_range() {
    let mut backend = make_backend();
    let req = PrepareRequest {
        student: Some("가상학생A".into()),
        date: Some("2026-10-01".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ok);
    let parsed = data(&envelope);
    assert!(parsed.students[0].previous_lesson.is_none());
    assert_eq!(
        envelope.coverage.unknowns,
        ["previous_lesson_not_found:가상학생A"]
    );
    assert!(
        envelope.display.contains("찾아봤지만 기록이 없습니다."),
        "{}",
        envelope.display
    );
}

// R5: the not-found display fixture, byte for byte (day scan of 11-11).
#[test]
fn previous_not_found_display_fixture() {
    let mut backend = make_backend();
    let envelope = edu_academy_teacher::prepare(
        &mut backend,
        &NoScheduleFacts,
        &req_student("가상학생F"),
        today(),
    );
    assert_eq!(envelope.status, Status::Ok);
    assert_eq!(envelope.display, DISPLAY_NOT_FOUND);
    display_lint(&envelope.display).unwrap();
    // Only the daily-record read was made.
    let names: Vec<&str> = envelope
        .coverage
        .sources
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(names, ["daily_record"]);
}

#[test]
fn ambiguous_student_asks_on_student_field() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    let mut duplicate = value["students"][0].clone();
    duplicate["key"] = serde_json::json!("1009");
    duplicate["class_name"] = serde_json::json!("가상반B");
    value["students"].as_array_mut().unwrap().push(duplicate);
    let scenario = load_scenario(&value.to_string()).unwrap();
    let mut backend = SourceBackend::synthetic(scenario.clone(), NOW);
    let seeded = SeededFacts::from_scenario(&scenario);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ask);
    let ask = envelope.ask.as_ref().unwrap();
    assert_eq!(ask.field, AskField::Student);
    assert!(!ask.options.is_empty());
}

// R7: duplicate names are told apart by class, and the answer resolves.
#[test]
fn duplicate_names_resolve_by_class_label() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    value["students"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "가상학생A", "key": "1010", "class_name": "가상반B",
            "bindings": [{"label": "가우스 5-1-2", "course_key": "101", "book_no": "201"}]
        }));
    let scenario = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&scenario);
    let mut backend = SourceBackend::synthetic(scenario.clone(), NOW);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ask);
    let options: Vec<&str> = envelope
        .ask
        .as_ref()
        .unwrap()
        .options
        .iter()
        .map(|o| o.value.as_str())
        .collect();
    assert_eq!(options, ["가상학생A (가상반A)", "가상학생A (가상반B)"]);
    // Answering with the labelled form resolves instead of looping.
    let mut backend2 = SourceBackend::synthetic(scenario.clone(), NOW);
    let envelope = edu_academy_teacher::prepare(
        &mut backend2,
        &seeded,
        &req_student("가상학생A (가상반B)"),
        today(),
    );
    assert_eq!(envelope.status, Status::Ok);
    assert_eq!(data(&envelope).students[0].student, "가상학생A");
    // Giving class_name together with student resolves it too.
    let mut backend3 = SourceBackend::synthetic(scenario, NOW);
    let req = PrepareRequest {
        student: Some("가상학생A".into()),
        class_name: Some("가상반B".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend3, &seeded, &req, today());
    assert_eq!(envelope.status, Status::Ok);
}

// R7: neither student nor class asks on class_name with the class names.
#[test]
fn missing_student_and_class_asks_on_class_name() {
    let mut backend = make_backend();
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &PrepareRequest::default(), today());
    assert_eq!(envelope.status, Status::Ask);
    let ask = envelope.ask.as_ref().unwrap();
    assert_eq!(ask.field, AskField::ClassName);
    let options: Vec<&str> = ask.options.iter().map(|o| o.value.as_str()).collect();
    assert_eq!(options, ["가상반A", "가상반B", "가상반C"]);
    assert!(ask.options.iter().all(|o| o.meaning_ko == "반 전체 준비"));
    assert!(
        envelope.display.contains("학생 이름"),
        "{}",
        envelope.display
    );
}

#[test]
fn unknown_class_asks_on_class_name() {
    let mut backend = make_backend();
    let req = PrepareRequest {
        class_name: Some("가상반Z".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ask);
    let ask = envelope.ask.as_ref().unwrap();
    assert_eq!(ask.field, AskField::ClassName);
    assert_eq!(ask.options.len(), 3);
}

#[test]
fn unknown_student_stops_student_not_found() {
    let mut backend = make_backend();
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &req_student("가상학생Z"), today());
    assert_eq!(envelope.status, Status::Unavailable);
    assert_eq!(
        envelope.stop.as_ref().unwrap().code,
        StopCode::StudentNotFound
    );
    assert_eq!(
        envelope.stop.as_ref().unwrap().code.as_str(),
        "student_not_found"
    );
}

#[test]
fn section_titles_of_progress_minor_are_not_checked() {
    let mut backend = make_backend();
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &req_student("가상학생A"), today());
    for check in &data(&envelope).students[0].checks {
        assert_ne!(check.verdict, Verdict::Found);
        assert!(!check.scope.starts_with("1-2"), "{}", check.scope);
    }
    let student =
        edu_academy_sources::textbook_index::TextbookStudent::new("1001", "가상학생A").unwrap();
    let scope = edu_academy_sources::textbook_index::BookScope::new("101", "201").unwrap();
    let checks = backend.section_checks(&student, &scope, "1-2", &["필수예제"]);
    assert_eq!(checks[0].verdict, Verdict::Found);
    assert_eq!((checks[0].correct, checks[0].total), (Some(8), Some(10)));
}

#[test]
fn scenario_has_at_most_one_record_per_student_date() {
    let duplicated = SCENARIO.replace(
        "{\"student\": \"가상학생A\", \"date\": \"2026-11-05\"",
        "{\"student\": \"가상학생A\", \"date\": \"2026-11-09\"",
    );
    assert!(load_scenario(&duplicated).is_err());
    assert!(load_scenario(SCENARIO).is_ok());
}

#[test]
fn synthetic_day_port_serves_backend_read() {
    let mut backend = make_backend();
    let record = backend
        .read_record("1001", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap())
        .unwrap();
    assert!(record.is_some());
    let missing = backend
        .read_record("1001", NaiveDate::from_ymd_opt(2026, 11, 10).unwrap())
        .unwrap();
    assert!(missing.is_none());
}

#[test]
fn scan_line_reads_book_minor_and_activity() {
    let line = scan_line("- 가우스 5-1-2 2-1 필수예제 (소단원 2-1 시작)").unwrap();
    assert_eq!(line.book_label, "가우스 5-1-2");
    assert_eq!(line.minor, "2-1");
    assert_eq!(line.activity, "필수예제");
    assert!(scan_line("메모만 있는 줄").is_none());
}

#[test]
fn tolerant_date_forms() {
    for (input, expected) in [
        ("오늘", "2026-11-11"),
        ("11월 9일", "2026-11-09"),
        ("2026-11-09", "2026-11-09"),
    ] {
        let mut backend = make_backend();
        let req = PrepareRequest {
            student: Some("가상학생A".into()),
            date: Some(input.into()),
            ..Default::default()
        };
        let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
        assert_eq!(data(&envelope).lesson_date, expected, "{input}");
    }
    let mut backend = make_backend();
    let req = PrepareRequest {
        student: Some("가상학생A".into()),
        date: Some("다음 주".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ask);
    assert_eq!(envelope.ask.as_ref().unwrap().field, AskField::Date);
}

// R3: a judged carry-over item is not shown.
#[test]
fn judged_carry_over_is_not_shown() {
    struct JudgedFacts;
    impl TeacherFacts for JudgedFacts {
        fn class(&self, name: &str) -> Option<ClassFact> {
            SeededFacts::from_scenario(&scenario()).class(name)
        }
        fn moved_lessons(&self, class_name: &str) -> &[edu_academy_teacher::facts::MovedLesson] {
            let _ = class_name;
            &[]
        }
        fn not_brought(&self, _student: &str) -> Vec<edu_academy_teacher::facts::NotBrought> {
            vec![edu_academy_teacher::facts::NotBrought {
                homework_line: "가우스 5-1-2 1-2 유형다지기 8문항".into(),
                assigned_on: NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(),
                judged: true,
            }]
        }
    }
    // Use the seeded schedule through a wrapper is complex; the seeded facts
    // with a judged twin are covered by filtering in prepare, exercised here
    // through a scenario whose only carry-over row is judged.
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    value["carry_over"][0]["judged"] = serde_json::json!(true);
    let scenario = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&scenario);
    let mut backend = SourceBackend::synthetic(scenario, NOW);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ok);
    assert!(
        !envelope.display.contains("못 가져온 숙제"),
        "{}",
        envelope.display
    );
    assert!(data(&envelope).students[0].carry_over.is_empty());
    let _ = JudgedFacts;
}

// R4/N1: F's earliest record has no predecessor. Alone, every lesson-data
// read of the call failed, so the call stops; the per-student line is kept
// when other students succeed (`one_failed_student_stays_ok_when_two_succeed`).
#[test]
fn first_record_without_predecessor_all_failed_stops() {
    let later = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
    // Single form.
    let mut backend = make_backend();
    let req = PrepareRequest {
        student: Some("가상학생F".into()),
        date: Some("2026-11-16".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Unavailable, "{}", envelope.display);
    assert_eq!(
        envelope.stop.as_ref().unwrap().code,
        StopCode::SourceUnavailable
    );
    assert!(
        envelope
            .coverage
            .unknowns
            .contains(&"daily_record_read_failed:가상학생F".to_string()),
        "{:?}",
        envelope.coverage.unknowns
    );
    // F has no successful lesson-data read, so it is not listed as a source.
    assert!(
        envelope.coverage.sources.is_empty(),
        "{:?}",
        envelope.coverage.sources
    );
    let _ = later;
    // Class form containing only F.
    let mut backend2 = make_backend();
    let req = PrepareRequest {
        class_name: Some("가상반C".into()),
        date: Some("2026-11-16".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend2, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Unavailable, "{}", envelope.display);
    assert_eq!(
        envelope.stop.as_ref().unwrap().code,
        StopCode::SourceUnavailable
    );
}

// R6: a waiting upload of the same minor in the other book is not 대기.
#[test]
fn prestudy_verdict_requires_the_homework_book() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    value["prestudy"]["waiting"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "student": "가상학생B", "unit_label": "가우스 5-1-2 4-3 가상주제M",
            "date": "2026-11-10", "key": "7003"
        }));
    let scenario = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&scenario);
    let mut backend = SourceBackend::synthetic(scenario, NOW);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생B"), today());
    assert_eq!(envelope.status, Status::Ok);
    let parsed = data(&envelope);
    assert_eq!(
        parsed.students[0].checks[0].verdict,
        Verdict::NotInReadRange
    );
    assert_ne!(parsed.students[0].checks[0].verdict, Verdict::Waiting);
}

// R9: a homework book with no binding asks on book with the bound books.
#[test]
fn unbound_homework_book_asks_on_book() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    for record in value["records"].as_array_mut().unwrap() {
        if record["student"] == "가상학생A" && record["date"] == "2026-11-09" {
            record["homework"] = serde_json::json!("- 가우스 5-1-9 2-1 필수예제");
        }
    }
    let scenario = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&scenario);
    let mut backend = SourceBackend::synthetic(scenario, NOW);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ask, "{}", envelope.display);
    let ask = envelope.ask.as_ref().unwrap();
    assert_eq!(ask.field, AskField::Book);
    let options: Vec<&str> = ask.options.iter().map(|o| o.value.as_str()).collect();
    assert_eq!(options, ["가우스 5-1-2"]);
}

// R9: a non-Gauss homework book asks on book as well.
#[test]
fn non_gauss_homework_book_asks_on_book() {
    let mut backend = make_backend();
    let req = PrepareRequest {
        student: Some("가상학생A".into()),
        book: Some("다빈치 5-1-2".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ask, "{}", envelope.display);
    assert_eq!(envelope.ask.as_ref().unwrap().field, AskField::Book);
}

// R9: names and class names are trimmed before use.
#[test]
fn inputs_are_trimmed() {
    let mut backend = make_backend();
    let envelope = edu_academy_teacher::prepare(
        &mut backend,
        &facts(),
        &req_student("  가상학생A  "),
        today(),
    );
    assert_eq!(envelope.status, Status::Ok);
    assert_eq!(envelope.display, DISPLAY_Q01);
}

// ---- R1: a second, minimal set of ports serves prepare unchanged. ----
mod minimal {
    use edu_academy_core::js::Js;
    use edu_academy_sources::{
        data::{Error, Result, obj},
        http::Response,
        native::SourcePort,
        prestudy::{ListKind, PrestudyPort, PrestudySearchForm},
        textbook::{FetchPolicy, TextbookResponse},
        textbook_index::{BookScope, TextbookSectionPort, TextbookStudent},
    };

    pub struct EmptyDayPort;
    impl SourcePort for EmptyDayPort {
        fn authenticate(&mut self) -> Result<Js> {
            Ok(obj(&[("status", "authenticated".into())]))
        }
        fn discover(&mut self, _input: &Js) -> Result<Js> {
            Ok(obj(&[(
                "groups",
                Js::Arr(vec![obj(&[("groupId", "1".into())])]),
            )]))
        }
        fn execute(&mut self, _input: &Js) -> Result<Js> {
            Ok(obj(&[("dayRecords", Js::Arr(vec![]))]))
        }
        fn journal_state(&mut self, _target: &Js) -> Result<Js> {
            Err(Error::error("minimal_no_record"))
        }
    }

    pub struct FailingPrestudyPort;
    impl PrestudyPort for FailingPrestudyPort {
        fn shell(&mut self, _kind: ListKind, _bound: usize) -> Result<Response> {
            Err(Error::refusal("minimal_prestudy_unavailable"))
        }
        fn search(
            &mut self,
            _kind: ListKind,
            _form: &PrestudySearchForm,
            _student_name: &str,
            _bound: usize,
        ) -> Result<Response> {
            Err(Error::refusal("minimal_prestudy_unavailable"))
        }
        fn summary(&mut self, _bound: usize) -> Result<Response> {
            Err(Error::refusal("minimal_prestudy_unavailable"))
        }
        fn detail(&mut self, _key: &str, _bound: usize) -> Result<Response> {
            Err(Error::refusal("minimal_prestudy_unavailable"))
        }
        fn report(&mut self, _key: &str, _bound: usize) -> Result<Response> {
            Err(Error::refusal("minimal_prestudy_unavailable"))
        }
    }

    pub struct FailingTextbookPort;
    impl TextbookSectionPort for FailingTextbookPort {
        fn progress(
            &mut self,
            _student: &TextbookStudent,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            Err(Error::refusal("minimal_textbook_unavailable"))
        }
        fn wrong_notes(
            &mut self,
            _student: &TextbookStudent,
            _scope: &BookScope,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            Err(Error::refusal("minimal_textbook_unavailable"))
        }
        fn detail(
            &mut self,
            _student: &TextbookStudent,
            _attempt_key: &str,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            Err(Error::refusal("minimal_textbook_unavailable"))
        }
        fn paper_search(
            &mut self,
            _student: &TextbookStudent,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            Err(Error::refusal("minimal_textbook_unavailable"))
        }
    }

    /// Name resolution uses the admission list; lesson record reads fail
    /// only while the shared flag is set.
    pub struct ToggleDayPort {
        pub fail: std::sync::Arc<std::sync::atomic::AtomicBool>,
        expecting_previous: Option<Js>,
    }
    impl ToggleDayPort {
        pub fn new(fail: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
            Self {
                fail,
                expecting_previous: None,
            }
        }
        fn failing(&self) -> bool {
            self.fail.load(std::sync::atomic::Ordering::SeqCst)
        }
        fn text(value: &Js) -> String {
            String::from_utf16_lossy(value.units())
        }
        fn key_for(name: &str) -> &'static str {
            match name {
                "가상학생B" => "1002",
                "가상학생C" => "1003",
                _ => "1001",
            }
        }
        fn row(name: &str, date: &str, record_key: &str) -> Js {
            obj(&[
                ("studentName", name.into()),
                ("date", date.into()),
                ("groupId", "1".into()),
                ("teacherKey", "9001".into()),
                ("studentKey", Self::key_for(name).into()),
                ("courseKey", "101".into()),
                ("recordKey", record_key.into()),
                ("membershipKey", "601".into()),
            ])
        }
    }
    impl SourcePort for ToggleDayPort {
        fn authenticate(&mut self) -> Result<Js> {
            Ok(obj(&[("status", "authenticated".into())]))
        }
        fn discover(&mut self, _input: &Js) -> Result<Js> {
            if self.failing() {
                return Err(Error::error("toggle_source_down"));
            }
            Ok(obj(&[(
                "groups",
                Js::Arr(vec![obj(&[("groupId", "1".into())])]),
            )]))
        }
        fn execute(&mut self, input: &Js) -> Result<Js> {
            if self.failing() {
                return Err(Error::error("toggle_source_down"));
            }
            if let Some(previous) = self.expecting_previous.take() {
                return Ok(obj(&[("dayRecords", Js::Arr(vec![previous]))]));
            }
            let date = Self::text(input.get("date"));
            let names = input.get("studentNames").array();
            let name = names.first().map_or(String::new(), Self::text);
            if date == "2026-11-09" {
                Ok(obj(&[(
                    "dayRecords",
                    Js::Arr(vec![Self::row(&name, &date, "8100")]),
                )]))
            } else {
                Ok(obj(&[("dayRecords", Js::Arr(vec![]))]))
            }
        }
        fn journal_state(&mut self, target: &Js) -> Result<Js> {
            if self.failing() {
                return Err(Error::error("toggle_source_down"));
            }
            let name = Self::text(target.get("studentName"));
            self.expecting_previous = Some(Self::row(&name, "2026-11-05", "7999"));
            Ok(obj(&[
                ("target", target.clone()),
                ("previousDate", "2026-11-05".into()),
                ("previousRecordKey", "7999".into()),
                (
                    "fields",
                    obj(&[
                        ("progress", "- 가우스 5-1-2 2-1 필수예제 풀이".into()),
                        ("homework", "- 가우스 5-1-2 2-1 유형다지기".into()),
                    ]),
                ),
                (
                    "wireDigest",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                ),
            ]))
        }
    }

    /// Bindings-only textbook port: two same-name students, one book each.
    pub struct BindingsTextbookPort;
    impl TextbookSectionPort for BindingsTextbookPort {
        fn progress(
            &mut self,
            student: &TextbookStudent,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            let (label, course, book) = match student.student_key() {
                "1030" => ("가우스 5-1-1", "102", "202"),
                _ => ("가우스 6-1-1", "103", "203"),
            };
            let html = format!(
                "<table><tr><td>가상</td><td>{label}</td><td><button onclick=\"createBookIncorrectPapers({course},{book},{})\">생성</button></td></tr></table>",
                student.student_key()
            );
            Ok(TextbookResponse {
                status: 200,
                headers: vec![],
                bytes: html.into_bytes(),
                final_url: None,
                redirected: false,
                credentials_sent: true,
                redirect_followed: false,
            })
        }
        fn wrong_notes(
            &mut self,
            _student: &TextbookStudent,
            _scope: &BookScope,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            Err(Error::refusal("minimal_textbook_unavailable"))
        }
        fn detail(
            &mut self,
            _student: &TextbookStudent,
            _attempt_key: &str,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            Err(Error::refusal("minimal_textbook_unavailable"))
        }
        fn paper_search(
            &mut self,
            _student: &TextbookStudent,
            _max: usize,
            _policy: FetchPolicy,
        ) -> Result<TextbookResponse> {
            Err(Error::refusal("minimal_textbook_unavailable"))
        }
    }

    /// Every read fails, including the name search.
    pub struct FailingDayPort;
    impl SourcePort for FailingDayPort {
        fn authenticate(&mut self) -> Result<Js> {
            Ok(obj(&[("status", "authenticated".into())]))
        }
        fn discover(&mut self, _input: &Js) -> Result<Js> {
            Err(Error::error("minimal_source_down"))
        }
        fn execute(&mut self, _input: &Js) -> Result<Js> {
            Err(Error::error("minimal_source_down"))
        }
        fn journal_state(&mut self, _target: &Js) -> Result<Js> {
            Err(Error::error("minimal_source_down"))
        }
        fn has_name_search(&self) -> bool {
            true
        }
        fn search_students_by_name(&mut self, _input: &Js) -> Option<Result<Js>> {
            Some(Err(Error::error("minimal_source_down")))
        }
    }
}

#[test]
fn prepare_runs_against_minimal_ports() {
    use edu_academy_teacher::backend::StudentEntry;
    let mut backend = SourceBackend::new(
        minimal::EmptyDayPort,
        minimal::FailingPrestudyPort,
        minimal::FailingTextbookPort,
        vec![StudentEntry {
            name: "가상학생A".into(),
            key: "1001".into(),
            class_name: "가상반A".into(),
        }],
        edu_academy_teacher::backend::BackendIdentity {
            teacher_id: "9001".into(),
            teacher_label: "가상교사".into(),
            teacher_key: "99001".into(),
            read_at: NOW.into(),
        },
    );
    let envelope = edu_academy_teacher::prepare(
        &mut backend,
        &NoScheduleFacts,
        &req_student("가상학생A"),
        today(),
    );
    assert_eq!(envelope.status, Status::Ok, "{}", envelope.display);
    assert!(envelope.display.contains("찾아봤지만 기록이 없습니다."));
}

// R4: only when every read of the call failed is the call unavailable.
#[test]
fn all_reads_failed_stops_source_unavailable() {
    use edu_academy_teacher::backend::StudentEntry;
    let mut backend = SourceBackend::new(
        minimal::FailingDayPort,
        minimal::FailingPrestudyPort,
        minimal::FailingTextbookPort,
        vec![StudentEntry {
            name: "가상학생A".into(),
            key: "1001".into(),
            class_name: "가상반A".into(),
        }],
        edu_academy_teacher::backend::BackendIdentity {
            teacher_id: "9001".into(),
            teacher_label: "가상교사".into(),
            teacher_key: "99001".into(),
            read_at: NOW.into(),
        },
    );
    let envelope = edu_academy_teacher::prepare(
        &mut backend,
        &NoScheduleFacts,
        &req_student("가상학생A"),
        today(),
    );
    assert_eq!(envelope.status, Status::Unavailable);
    assert_eq!(
        envelope.stop.as_ref().unwrap().code,
        StopCode::SourceUnavailable
    );
}

// R2: a failed prestudy or section read comes back Unknown, never absent.
#[test]
fn failed_prestudy_and_section_reads_are_unknown() {
    use edu_academy_teacher::backend::StudentEntry;
    let mut backend = SourceBackend::new(
        minimal::EmptyDayPort,
        minimal::FailingPrestudyPort,
        minimal::FailingTextbookPort,
        vec![StudentEntry {
            name: "가상학생A".into(),
            key: "1001".into(),
            class_name: "가상반A".into(),
        }],
        edu_academy_teacher::backend::BackendIdentity {
            teacher_id: "9001".into(),
            teacher_label: "가상교사".into(),
            teacher_key: "99001".into(),
            read_at: NOW.into(),
        },
    );
    assert_eq!(
        backend.prestudy_verdict("가상학생A", "가우스 5-1-2", "2-1"),
        Verdict::Unknown
    );
    let student =
        edu_academy_sources::textbook_index::TextbookStudent::new("1001", "가상학생A").unwrap();
    let scope = edu_academy_sources::textbook_index::BookScope::new("101", "201").unwrap();
    let checks = backend.section_checks(&student, &scope, "2-1", &["필수예제", "유형다지기"]);
    assert_eq!(checks.len(), 2);
    assert!(checks.iter().all(|c| c.verdict == Verdict::Unknown));
}

fn toggle_backend(
    fail: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    entries: Vec<edu_academy_teacher::backend::StudentEntry>,
) -> SourceBackend<minimal::ToggleDayPort, minimal::FailingPrestudyPort, minimal::FailingTextbookPort>
{
    SourceBackend::new(
        minimal::ToggleDayPort::new(fail.clone()),
        minimal::FailingPrestudyPort,
        minimal::FailingTextbookPort,
        entries,
        edu_academy_teacher::backend::BackendIdentity {
            teacher_id: "9001".into(),
            teacher_label: "가상교사".into(),
            teacher_key: "99001".into(),
            read_at: NOW.into(),
        },
    )
}

fn entry(name: &str, key: &str, class_name: &str) -> edu_academy_teacher::backend::StudentEntry {
    edu_academy_teacher::backend::StudentEntry {
        name: name.into(),
        key: key.into(),
        class_name: class_name.into(),
    }
}

// N1(a): name resolution works but every lesson-data read fails, class form.
#[test]
fn all_lesson_reads_failed_stops_when_name_resolution_works() {
    let fail = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut backend = toggle_backend(
        &fail,
        vec![
            entry("가상학생A", "1001", "가상반A"),
            entry("가상학생B", "1002", "가상반A"),
            entry("가상학생C", "1003", "가상반A"),
        ],
    );
    let req = PrepareRequest {
        class_name: Some("가상반A".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Unavailable, "{}", envelope.display);
    assert_eq!(
        envelope.stop.as_ref().unwrap().code,
        StopCode::SourceUnavailable
    );
}

// N1(b): a reused backend gives the same answer on a later all-failed call.
#[test]
fn reused_backend_second_all_failed_call_stops() {
    let fail = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut backend = toggle_backend(&fail, vec![entry("가상학생A", "1001", "가상반A")]);
    let first = edu_academy_teacher::prepare(
        &mut backend,
        &NoScheduleFacts,
        &req_student("가상학생A"),
        today(),
    );
    assert_eq!(first.status, Status::Ok, "{}", first.display);
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let second = edu_academy_teacher::prepare(
        &mut backend,
        &NoScheduleFacts,
        &req_student("가상학생A"),
        today(),
    );
    assert_eq!(second.status, Status::Unavailable, "{}", second.display);
    assert_eq!(
        second.stop.as_ref().unwrap().code,
        StopCode::SourceUnavailable
    );
}

// N1(c): one student fails and two succeed: ok with the failed student's line.
#[test]
fn one_failed_student_stays_ok_when_two_succeed() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    for student in value["students"].as_array_mut().unwrap() {
        if student["name"] == "가상학생F" {
            student["class_name"] = serde_json::json!("가상반A");
        }
    }
    value["classes"][0]["roster"] = serde_json::json!(["가상학생F", "가상학생A", "가상학생B"]);
    let sc = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc, NOW);
    let req = PrepareRequest {
        class_name: Some("가상반A".into()),
        date: Some("2026-11-16".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &seeded, &req, today());
    assert_eq!(envelope.status, Status::Ok, "{}", envelope.display);
    assert!(
        envelope
            .display
            .contains("- 가상학생F 지난 수업 기록을 읽지 못했습니다."),
        "{}",
        envelope.display
    );
}

// N2: a failed prestudy read is not listed as a source.
#[test]
fn failed_prestudy_read_is_not_a_source() {
    let sc = scenario();
    let mut backend = SourceBackend::new(
        SyntheticDayPort::new(sc.clone()),
        minimal::FailingPrestudyPort,
        SyntheticTextbookPort::new(sc),
        vec![entry("가상학생A", "1001", "가상반A")],
        edu_academy_teacher::backend::BackendIdentity {
            teacher_id: "9001".into(),
            teacher_label: "가상교사".into(),
            teacher_key: "99001".into(),
            read_at: NOW.into(),
        },
    );
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ok, "{}", envelope.display);
    assert_eq!(
        data(&envelope).students[0].checks[0].verdict,
        Verdict::Unknown
    );
    assert!(
        envelope
            .coverage
            .sources
            .iter()
            .all(|s| s.name != "prestudy"),
        "{:?}",
        envelope.coverage.sources
    );
    assert!(
        envelope
            .coverage
            .unknowns
            .iter()
            .any(|u| u.starts_with("prestudy_unknown:")),
        "{:?}",
        envelope.coverage.unknowns
    );
}

// N3: a name that itself ends with a parenthesis resolves as the full name.
#[test]
fn name_ending_in_parenthesis_resolves_as_full_name() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    value["students"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "가상학생F (2)", "key": "1011", "class_name": "가상반C",
            "bindings": [{"label": "가우스 5-1-2", "course_key": "101", "book_no": "201"}]
        }));
    let sc = load_scenario(&value.to_string()).unwrap();
    let mut backend = SourceBackend::synthetic(sc, NOW);
    let resolved = backend.resolve_student("가상학생F (2)", None).unwrap();
    assert_eq!(resolved.key, "1011");
    assert_eq!(resolved.name, "가상학생F (2)");
}

// N3: same-name students in one class are told apart by the bound book.
#[test]
fn same_class_duplicate_names_resolve_by_book_label() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    for (key, label, course, book) in [
        ("1020", "가우스 5-1-1", "102", "202"),
        ("1021", "가우스 6-1-1", "103", "203"),
    ] {
        value["students"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "name": "가상학생D", "key": key, "class_name": "가상반A",
                "bindings": [{"label": label, "course_key": course, "book_no": book}]
            }));
    }
    let sc = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc.clone(), NOW);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생D"), today());
    assert_eq!(envelope.status, Status::Ask, "{}", envelope.display);
    let options: Vec<&str> = envelope
        .ask
        .as_ref()
        .unwrap()
        .options
        .iter()
        .map(|o| o.value.as_str())
        .collect();
    assert!(
        options.contains(&"가상학생D (가상반A, 가우스 5-1-1)"),
        "{options:?}"
    );
    assert!(
        options.contains(&"가상학생D (가상반A, 가우스 6-1-1)"),
        "{options:?}"
    );
    assert!(options.contains(&"가상학생D (가상반B)"), "{options:?}");
    let mut backend2 = SourceBackend::synthetic(sc, NOW);
    let resolved = backend2
        .resolve_student("가상학생D (가상반A, 가우스 6-1-1)", None)
        .unwrap();
    assert_eq!(resolved.key, "1021");
}

// N3: students identical by name, class and book give a stop, not an ask.
#[test]
fn indistinguishable_duplicate_names_stop() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    for key in ["1020", "1021"] {
        value["students"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "name": "가상학생D", "key": key, "class_name": "가상반A",
                "bindings": [{"label": "가우스 5-1-1", "course_key": "102", "book_no": "202"}]
            }));
    }
    let sc = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc, NOW);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생D"), today());
    assert_eq!(envelope.status, Status::Unavailable, "{}", envelope.display);
    assert_eq!(
        envelope.stop.as_ref().unwrap().code.as_str(),
        "student_ambiguous_unresolvable"
    );
    assert!(
        envelope.display.contains("구분할 수 없습니다"),
        "{}",
        envelope.display
    );
    assert!(
        envelope.display.contains("반 명부를 수정"),
        "{}",
        envelope.display
    );
}

// N4: a not-brought item follows the resolved student, not the spoken name.
#[test]
fn not_brought_is_keyed_by_resolved_student() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    value["students"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "가상학생A", "key": "1010", "class_name": "가상반B",
            "bindings": [{"label": "가우스 5-1-2", "course_key": "101", "book_no": "201"}]
        }));
    value["carry_over"][0]["student_key"] = serde_json::json!("1010");
    let sc = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc.clone(), NOW);
    let envelope = edu_academy_teacher::prepare(
        &mut backend,
        &seeded,
        &req_student("가상학생A (가상반B)"),
        today(),
    );
    assert_eq!(envelope.status, Status::Ok, "{}", envelope.display);
    assert_eq!(data(&envelope).students[0].carry_over.len(), 1);
    let mut backend2 = SourceBackend::synthetic(sc, NOW);
    let envelope = edu_academy_teacher::prepare(
        &mut backend2,
        &seeded,
        &req_student("가상학생A (가상반A)"),
        today(),
    );
    assert_eq!(envelope.status, Status::Ok, "{}", envelope.display);
    assert!(
        data(&envelope).students[0].carry_over.is_empty(),
        "{}",
        envelope.display
    );
    assert!(
        !envelope.display.contains("못 가져온 숙제"),
        "{}",
        envelope.display
    );
}

// P8-S1b 5a: the answer is compared with the generated option strings, so
// a class name containing ", " (and a parenthesis) still resolves.
#[test]
fn duplicate_names_in_comma_class_resolve_by_generated_option() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    for (key, label, course, book) in [
        ("1030", "가우스 5-1-1", "102", "202"),
        ("1031", "가우스 6-1-1", "103", "203"),
    ] {
        value["students"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "name": "가상학생G", "key": key, "class_name": "가상반A, 1조 (월수)",
                "bindings": [{"label": label, "course_key": course, "book_no": book}]
            }));
    }
    let sc = load_scenario(&value.to_string()).unwrap();
    let mut backend = SourceBackend::synthetic(sc.clone(), NOW);
    let err = backend.resolve_student("가상학생G", None).unwrap_err();
    let edu_academy_teacher::backend::BackendError::Ask(ask) = err else {
        panic!("expected ask, got {err:?}");
    };
    let options: Vec<&str> = ask.options.iter().map(|o| o.value.as_str()).collect();
    assert_eq!(
        options,
        [
            "가상학생G (가상반A, 1조 (월수), 가우스 5-1-1)",
            "가상학생G (가상반A, 1조 (월수), 가우스 6-1-1)"
        ]
    );
    // One bindings read per same-class candidate.
    assert_eq!(backend.source_reads(), 2);
    let mut backend2 = SourceBackend::synthetic(sc, NOW);
    let resolved = backend2
        .resolve_student("가상학생G (가상반A, 1조 (월수), 가우스 6-1-1)", None)
        .unwrap();
    assert_eq!(resolved.key, "1031");
    assert_eq!(resolved.name, "가상학생G");
}

// P8-S1b 5b: two candidates getting the same option string stop; a student
// bound to two books whose classmate is bound to one of them collides on
// the shared first book.
#[test]
fn two_books_same_class_duplicate_option_stops() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    value["students"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "가상학생H", "key": "1032", "class_name": "가상반A",
            "bindings": [
                {"label": "가우스 5-1-1", "course_key": "102", "book_no": "202"},
                {"label": "가우스 5-1-2", "course_key": "101", "book_no": "201"}
            ]
        }));
    value["students"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "가상학생H", "key": "1033", "class_name": "가상반A",
            "bindings": [{"label": "가우스 5-1-1", "course_key": "102", "book_no": "202"}]
        }));
    let sc = load_scenario(&value.to_string()).unwrap();
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc, NOW);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생H"), today());
    assert_eq!(envelope.status, Status::Unavailable, "{}", envelope.display);
    assert_eq!(
        envelope.stop.as_ref().unwrap().code.as_str(),
        "student_ambiguous_unresolvable"
    );
}

// P8-S1b 5c: a backend built from three minimal ports (no synthetic book
// table) tells same-class, same-name students apart by the bindings port.
#[test]
fn minimal_ports_tell_same_class_students_apart() {
    let mut backend = SourceBackend::new(
        minimal::EmptyDayPort,
        minimal::FailingPrestudyPort,
        minimal::BindingsTextbookPort,
        vec![
            entry("가상학생G", "1030", "가상반A"),
            entry("가상학생G", "1031", "가상반A"),
        ],
        edu_academy_teacher::backend::BackendIdentity {
            teacher_id: "9001".into(),
            teacher_label: "가상교사".into(),
            teacher_key: "99001".into(),
            read_at: NOW.into(),
        },
    );
    let err = backend.resolve_student("가상학생G", None).unwrap_err();
    let edu_academy_teacher::backend::BackendError::Ask(ask) = err else {
        panic!("expected ask, got {err:?}");
    };
    let options: Vec<&str> = ask.options.iter().map(|o| o.value.as_str()).collect();
    assert_eq!(
        options,
        [
            "가상학생G (가상반A, 가우스 5-1-1)",
            "가상학생G (가상반A, 가우스 6-1-1)"
        ]
    );
    assert_eq!(backend.source_reads(), 2);
    let resolved = backend
        .resolve_student("가상학생G (가상반A, 가우스 6-1-1)", None)
        .unwrap();
    assert_eq!(resolved.key, "1031");
}

#[test]
fn misspelt_name_reads_no_bindings() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    let mut duplicate = value["students"][0].clone();
    duplicate["key"] = serde_json::json!("1099");
    value["students"].as_array_mut().unwrap().push(duplicate);
    let mut backend = SourceBackend::synthetic(load_scenario(&value.to_string()).unwrap(), NOW);
    let err = backend.resolve_student("가상학생Typo", None).unwrap_err();
    let edu_academy_teacher::backend::BackendError::Stop(stop) = err else {
        panic!("{err:?}")
    };
    assert_eq!(stop.code, StopCode::StudentNotFound);
    assert_eq!(backend.source_reads(), 0);
}

#[test]
fn binding_order_does_not_change_option() {
    fn options(reverse: bool) -> Vec<String> {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
        let first = serde_json::json!({"label":"가우스 5-1-1","course_key":"102","book_no":"202"});
        let second = value["students"][0]["bindings"][0].clone();
        value["students"][0]["bindings"] = if reverse {
            serde_json::json!([second, first])
        } else {
            serde_json::json!([first, second])
        };
        let mut duplicate = value["students"][0].clone();
        duplicate["key"] = serde_json::json!("1099");
        duplicate["bindings"] =
            serde_json::json!([{"label":"가우스 6-1-1","course_key":"103","book_no":"203"}]);
        value["students"].as_array_mut().unwrap().push(duplicate);
        let mut backend = SourceBackend::synthetic(load_scenario(&value.to_string()).unwrap(), NOW);
        let err = backend.resolve_student("가상학생A", None).unwrap_err();
        let edu_academy_teacher::backend::BackendError::Ask(ask) = err else {
            panic!("{err:?}")
        };
        ask.options.into_iter().map(|o| o.value).collect()
    }
    assert_eq!(options(false), options(true));
}

#[test]
fn option_matches_across_name_prefixes_stop() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    for (key, name, class_name) in [
        ("1098", "가상학생X", "가상반 (추가"),
        ("1099", "가상학생X (가상반", "추가"),
    ] {
        value["students"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "name":name, "key":key, "class_name":class_name, "bindings":[]
            }));
    }
    let mut backend = SourceBackend::synthetic(load_scenario(&value.to_string()).unwrap(), NOW);
    let err = backend
        .resolve_student("가상학생X (가상반 (추가)", None)
        .unwrap_err();
    let edu_academy_teacher::backend::BackendError::Stop(stop) = err else {
        panic!("{err:?}")
    };
    assert_eq!(stop.code, StopCode::StudentAmbiguousUnresolvable);
}

fn assert_ask_labels_resolve(sc: Scenario, req: PrepareRequest, field: AskField) {
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc, NOW);
    let envelope = edu_academy_teacher::prepare(&mut backend, &seeded, &req, today());
    assert_eq!(envelope.status, Status::Ask, "{}", envelope.display);
    let ask = envelope.ask.as_ref().unwrap();
    assert_eq!(ask.field, field);
    let mut last = 0;
    for option in &ask.options {
        let line = if option.meaning_ko.is_empty() {
            format!("- {}", option.label_ko)
        } else {
            format!("- {}: {}", option.label_ko, option.meaning_ko)
        };
        let offset = envelope.display[last..]
            .find(&line)
            .expect("option in display in order");
        last += offset + line.len();
        let mut answer = req.clone();
        let target = match field {
            AskField::Student => &mut answer.student,
            AskField::ClassName => &mut answer.class_name,
            AskField::Book => &mut answer.book,
            AskField::Date => &mut answer.date,
            _ => panic!("unexpected slice-1 field"),
        };
        *target = Some(option.label_ko.clone());
        let resolved = edu_academy_teacher::prepare(&mut backend, &seeded, &answer, today());
        assert_eq!(
            resolved.status,
            Status::Ok,
            "{}: {}",
            option.label_ko,
            resolved.display
        );
    }
}

#[test]
fn all_class_date_and_book_ask_labels_resolve() {
    assert_ask_labels_resolve(scenario(), PrepareRequest::default(), AskField::ClassName);
    assert_ask_labels_resolve(
        scenario(),
        PrepareRequest {
            class_name: Some("가상반Z".into()),
            ..Default::default()
        },
        AskField::ClassName,
    );
    assert_ask_labels_resolve(
        scenario(),
        PrepareRequest {
            student: Some("가상학생A".into()),
            date: Some("invalid".into()),
            ..Default::default()
        },
        AskField::Date,
    );
    assert_ask_labels_resolve(
        scenario(),
        PrepareRequest {
            student: Some("가상학생B".into()),
            book: Some("가우스 9-9-9".into()),
            ..Default::default()
        },
        AskField::Book,
    );
}

#[test]
fn student_class_disagreement_ask_labels_resolve() {
    assert_ask_labels_resolve(
        scenario(),
        PrepareRequest {
            student: Some("가상학생A".into()),
            class_name: Some("가상반Z".into()),
            ..Default::default()
        },
        AskField::ClassName,
    );
}

#[test]
fn class_disagreement_labels_resolve_for_generated_and_duplicate_names() {
    assert_ask_labels_resolve(
        scenario(),
        PrepareRequest {
            student: Some("가상학생A (가상반A)".into()),
            class_name: Some("가상반Z".into()),
            ..Default::default()
        },
        AskField::ClassName,
    );
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    let mut duplicate = value["students"][0].clone();
    duplicate["key"] = serde_json::json!("1050");
    duplicate["class_name"] = serde_json::json!("가상반B");
    value["students"].as_array_mut().unwrap().push(duplicate);
    assert_ask_labels_resolve(
        load_scenario(&value.to_string()).unwrap(),
        PrepareRequest {
            student: Some("가상학생A".into()),
            class_name: Some("가상반Z".into()),
            ..Default::default()
        },
        AskField::ClassName,
    );
}

#[test]
fn ambiguous_student_ask_labels_resolve_with_class_and_book() {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    for (key, class_name, book) in [
        ("1020", "가상반A", "가우스 5-1-1"),
        ("1021", "가상반A", "가우스 6-1-1"),
    ] {
        value["students"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "name":"가상학생D", "key":key, "class_name":class_name,
                "bindings":[{"label":book,"course_key":"102","book_no":"202"}]
            }));
    }
    let sc = load_scenario(&value.to_string()).unwrap();
    assert_ask_labels_resolve(sc.clone(), req_student("가상학생D"), AskField::Student);
    assert_ask_labels_resolve(
        sc,
        PrepareRequest {
            student: Some("가상학생D".into()),
            class_name: Some("가상반A".into()),
            ..Default::default()
        },
        AskField::Student,
    );
}

#[test]
fn q03_unknown_class_display_has_three_choices() {
    let result = edu_academy_teacher::prepare(
        &mut make_backend(),
        &facts(),
        &PrepareRequest {
            class_name: Some("가상반Z".into()),
            ..Default::default()
        },
        today(),
    );
    assert_eq!(result.status, Status::Ask);
    assert_eq!(
        result.display,
        "어느 반인가요?\n- 가상반A: 반 전체 준비\n- 가상반B: 반 전체 준비\n- 가상반C: 반 전체 준비"
    );
}
