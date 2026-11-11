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

fn prepare_v2(sc: Scenario, req: &PrepareRequest) -> edu_academy_teacher::envelope::Envelope {
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc, NOW);
    edu_academy_teacher::prepare(&mut backend, &seeded, req, today())
}
fn scenario_v2() -> Scenario {
    load_scenario(include_str!("fixtures/scenario-v2.json")).unwrap()
}

#[test]
fn seven_contract_targets_byte_for_byte() {
    for (req, fixture) in [
        (req_student("가상학생A"), DISPLAY_Q01),
        (
            PrepareRequest {
                class_name: Some("가상반A".into()),
                ..Default::default()
            },
            DISPLAY_Q02,
        ),
        (req_student("가상학생F"), DISPLAY_NOT_FOUND),
        (
            PrepareRequest {
                student: Some("가상학생A".into()),
                class_name: Some("가상반B".into()),
                ..Default::default()
            },
            include_str!("fixtures/display-disagreement.txt"),
        ),
        (
            PrepareRequest {
                student: Some("가상학생A".into()),
                date: Some("2026-10-20".into()),
                ..Default::default()
            },
            include_str!("fixtures/display-outside-admission.txt"),
        ),
        (
            PrepareRequest {
                class_name: Some("가상반C".into()),
                ..Default::default()
            },
            include_str!("fixtures/display-empty-roster.txt"),
        ),
        (
            PrepareRequest {
                student: Some("가상학생A".into()),
                date: Some("지난 수업일".into()),
                ..Default::default()
            },
            include_str!("fixtures/display-previous-lesson-date.txt"),
        ),
    ] {
        let mut sc = scenario_v2();
        if req.class_name.as_deref() == Some("가상반C") {
            sc.classes[2].roster.clear();
        }
        let result = prepare_v2(sc, &req);
        assert_eq!(result.display, fixture);
        display_lint(&result.display).unwrap();
        assert!(!result.display.ends_with('\n'));
    }
}

#[test]
fn q01_q02_data_and_line_limits() {
    let one = prepare_v2(scenario_v2(), &req_student("가상학생A"));
    assert!(one.display.lines().count() <= 30);
    assert_eq!(
        data(&one).students[0]
            .previous_lesson
            .as_ref()
            .unwrap()
            .date,
        "2026-11-09"
    );
    assert_eq!(data(&one).students[0].checks[0].verdict, Verdict::Waiting);
    assert_eq!(data(&one).read_at, "2026-11-11T09:10:00Z");
    let group = prepare_v2(
        scenario_v2(),
        &PrepareRequest {
            class_name: Some("가상반A".into()),
            ..Default::default()
        },
    );
    assert!(group.display.lines().count() <= 45);
    assert_eq!(
        data(&group)
            .students
            .iter()
            .map(|s| s.student.as_str())
            .collect::<Vec<_>>(),
        ["가상학생A", "가상학생B", "가상학생C"]
    );
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
        envelope.display.contains("지난 수업 기록이 없어"),
        "{}",
        envelope.display
    );
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
        envelope.display.lines().count() <= 4,
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
        envelope
            .coverage
            .sources
            .iter()
            .all(|s| s.name != "daily_record"),
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
    let envelope = prepare_v2(scenario_v2(), &req_student("  가상학생A  "));
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
    assert!(envelope.display.contains("지난 수업 기록이 없어"));
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
            .contains("가상학생F의 지난 수업 기록을 읽지 못했습니다."),
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

#[test]
fn class_roster_disagreement_stops_whole_request() {
    let mut sc = scenario();
    // Keep the authoritative requested roster unchanged; move only the
    // student's record. Exercise the first, middle and last roster entry.
    for (index, name) in [(0, "가상학생A"), (1, "가상학생B"), (2, "가상학생C")] {
        let mut changed = sc.clone();
        changed.students[index].class_name = "가상반B".into();
        let seeded = SeededFacts::from_scenario(&changed);
        let mut backend = SourceBackend::synthetic(changed, NOW);
        let result = edu_academy_teacher::prepare(
            &mut backend,
            &seeded,
            &PrepareRequest {
                class_name: Some("가상반A".into()),
                ..Default::default()
            },
            today(),
        );
        assert_eq!(result.status, Status::Unavailable);
        assert!(result.ask.is_none());
        let stop = result.stop.as_ref().unwrap();
        assert_eq!(stop.code, StopCode::RosterClassMismatch);
        assert_eq!(
            result.display,
            format!(
                "가상반A의 반 명부와 학생 기록이 {name} 학생의 반에 대해 일치하지 않아 반 전체 수업 준비를 중단했습니다. 반 명부를 수정하거나 학생 이름을 지정해 주세요."
            )
        );
        assert_eq!(stop.message_ko, result.display);
        assert_eq!(data(&result).class_name.as_deref(), Some("가상반A"));
        assert!(data(&result).students.is_empty());
        assert!(!result.display.contains("지난 수업:"));
    }
    // Explicitly naming the student retains the student-mode class ask.
    sc.students[1].class_name = "가상반B".into();
    assert_ask_labels_resolve(
        sc,
        PrepareRequest {
            student: Some("가상학생B".into()),
            class_name: Some("가상반A".into()),
            ..Default::default()
        },
        AskField::ClassName,
    );
}

#[test]
fn student_without_class_stops_instead_of_empty_ask() {
    for missing in ["", "  "] {
        let mut sc = scenario();
        sc.students[0].class_name = missing.into();
        let seeded = SeededFacts::from_scenario(&sc);
        let mut backend = SourceBackend::synthetic(sc, NOW);
        let result = edu_academy_teacher::prepare(
            &mut backend,
            &seeded,
            &PrepareRequest {
                student: Some("가상학생A".into()),
                class_name: Some("가상반A".into()),
                ..Default::default()
            },
            today(),
        );
        assert_eq!(result.status, Status::Unavailable);
        assert!(result.ask.is_none());
        assert_eq!(result.stop.unwrap().code, StopCode::StudentNotFound);
        assert_eq!(
            result.display,
            edu_academy_teacher::display_labels::STUDENT_MISSING
        );
    }
}

fn action_rows(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.contains(" | ") && !line.starts_with("학생 | "))
        .collect()
}
fn changed_rows(before: &str, after: &str) -> (usize, usize) {
    let a = action_rows(before);
    let b = action_rows(after);
    (
        a.iter().filter(|r| !b.contains(r)).count(),
        b.iter().filter(|r| !a.contains(r)).count(),
    )
}
#[test]
fn one_fact_changes_one_video_row() {
    let original = prepare_v2(scenario_v2(), &req_student("가상학생A"));
    let mut sc = scenario_v2();
    let video = sc.prestudy.waiting.remove(0);
    sc.prestudy.completed.push(video);
    let changed = prepare_v2(sc, &req_student("가상학생A"));
    assert_eq!(changed_rows(&original.display, &changed.display), (1, 1));
    assert!(
        changed
            .display
            .lines()
            .any(|r| r.contains("예습영상 | 영상 확인 | 서버 확인"))
    );
    assert_eq!(
        data(&changed).students[0].checks[0].verdict,
        Verdict::Completed
    );
}
#[test]
fn closing_one_reminder_removes_only_its_row() {
    let original = prepare_v2(scenario_v2(), &req_student("가상학생A"));
    let mut sc = scenario_v2();
    sc.reminders[1].status = "closed".into();
    let changed = prepare_v2(sc, &req_student("가상학생A"));
    assert_eq!(changed_rows(&original.display, &changed.display), (1, 0));
    assert!(!changed.display.contains("가상학생E |"));
    assert_eq!(data(&changed).reminders.len(), 2);
}
#[test]
fn dated_notebook_fact_changes_only_its_row() {
    let original = prepare_v2(scenario_v2(), &req_student("가상학생A"));
    let mut sc = scenario_v2();
    sc.teacher_fact_reports.push(serde_json::from_value(serde_json::json!({
        "kind":"solution_notebook_check","student":"가상학생A","book":"가우스 5-1-2","unit":"2-1","completed":true,"reported_on":"2026-11-11"
    })).unwrap());
    let changed = prepare_v2(sc, &req_student("가상학생A"));
    assert_eq!(changed_rows(&original.display, &changed.display), (1, 2));
    assert!(
        changed
            .display
            .lines()
            .any(|r| r.contains("풀이노트 검사 | 교사 확인(11월 11일)"))
    );
}
#[test]
fn not_brought_judgment_removes_its_row() {
    let original = prepare_v2(scenario_v2(), &req_student("가상학생A"));
    let mut sc = scenario_v2();
    sc.carry_over[0].judged = true;
    let changed = prepare_v2(sc, &req_student("가상학생A"));
    assert_eq!(changed_rows(&original.display, &changed.display), (1, 0));
    assert!(!changed.display.contains("미지참 판정"));
}
#[test]
fn anonymous_counts_are_not_claimed_as_distinct_labels() {
    let mut sc = scenario_v2();
    sc.teacher_fact_reports[1].item_labels = vec!["1".into(), "1".into()];
    let result = prepare_v2(sc, &req_student("가상학생A"));
    let carry = result
        .display
        .lines()
        .find(|line| line.contains("미지참 판정"))
        .unwrap();
    assert!(carry.contains("문항 8개"));
    assert!(!carry.contains("문항 번호"));
}
#[test]
fn unknown_part_does_not_repeat_rows_and_next_is_bounded() {
    let result = prepare_v2(
        scenario_v2(),
        &PrepareRequest {
            class_name: Some("가상반A".into()),
            ..Default::default()
        },
    );
    let unknown = result
        .display
        .split("미확인 사항\n")
        .nth(1)
        .unwrap()
        .split("\n다음 확인")
        .next()
        .unwrap();
    assert!(!unknown.contains(" | "));
    assert!(!unknown.contains("미확인("));
    let next = result
        .display
        .split("다음 확인\n")
        .last()
        .unwrap()
        .split("\n읽은 원천")
        .next()
        .unwrap();
    assert!(next.lines().count() <= 3);
    for line in result.display.lines().filter(|line| line.contains('|')) {
        assert_eq!(line.split(" | ").count(), 6);
    }
}
#[test]
fn reminder_urgency_uses_today_even_for_historical_request() {
    use edu_academy_teacher::render::urgency;
    assert_eq!(
        urgency("2026-11-10T18:10:00+09:00", "2026-11-11"),
        "기한 지남"
    );
    assert_eq!(urgency("2026-11-11T18:10:00+09:00", "2026-11-11"), "오늘");
    assert_eq!(urgency("2026-11-12T18:10:00+09:00", "2026-11-11"), "임박");
    let result = prepare_v2(
        scenario_v2(),
        &PrepareRequest {
            student: Some("가상학생A".into()),
            date: Some("지난 수업일".into()),
            ..Default::default()
        },
    );
    assert!(result.display.contains("오늘 11월 11일 18:10"));
    assert_eq!(data(&result).lesson_date, "2026-11-09");
    assert_eq!(
        data(&result).students[0]
            .previous_lesson
            .as_ref()
            .unwrap()
            .date,
        "2026-11-05"
    );
    assert!(
        result
            .coverage
            .sources
            .iter()
            .any(|s| s.name == "date_resolution")
    );
}
#[test]
fn admission_refusal_reads_no_source() {
    let sc = scenario_v2();
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc, NOW);
    let result = edu_academy_teacher::prepare(
        &mut backend,
        &seeded,
        &PrepareRequest {
            student: Some("가상학생A".into()),
            date: Some("2026-10-20".into()),
            ..Default::default()
        },
        today(),
    );
    assert_eq!(result.stop.unwrap().code, StopCode::DateNotAdmitted);
    assert_eq!(backend.source_reads(), 0);
    assert_eq!(backend.attempted_reads(), 0);
    assert_eq!(backend.prestudy_port_calls(), 0);
    assert_eq!(backend.textbook_port_calls(), 0);
    assert!(result.coverage.sources.is_empty());
}
#[test]
fn missing_class_roster_stop_names_student() {
    let mut sc = scenario_v2();
    sc.students[1].class_name.clear();
    let result = prepare_v2(
        sc,
        &PrepareRequest {
            class_name: Some("가상반A".into()),
            ..Default::default()
        },
    );
    assert_eq!(result.stop.unwrap().code, StopCode::StudentNotFound);
    assert!(result.display.contains("가상학생B"));
    assert!(result.ask.is_none());
}

#[test]
fn scenario_v2_preserves_v1_root_and_wires_added_facts() {
    let old: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    let new: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/scenario-v2.json")).unwrap();
    for (key, value) in old.as_object().unwrap() {
        assert_eq!(&new[key], value, "{key}");
    }
    assert_eq!(
        new.as_object().unwrap().len(),
        old.as_object().unwrap().len() + 4
    );
    let seeded = SeededFacts::from_scenario(&scenario_v2());
    assert_eq!(seeded.read_at(), Some("2026-11-11T09:10:00Z"));
    assert_eq!(seeded.reports().len(), 8);
    assert_eq!(seeded.reminders().len(), 3);
    assert_eq!(seeded.admitted_dates().unwrap().from, "2026-10-21");
    for bad in [
        serde_json::json!({"from":"2026-11-11","to":"2026-10-21"}),
        serde_json::json!({"from":"bad","to":"2026-11-11"}),
    ] {
        let mut invalid = new.clone();
        invalid["admitted_dates"] = bad;
        assert!(load_scenario(&invalid.to_string()).is_err());
    }
}
#[test]
fn once_per_student_port_reads_and_separate_date_resolution() {
    for (req, expected) in [
        (req_student("가상학생A"), 4),
        (
            PrepareRequest {
                class_name: Some("가상반A".into()),
                ..Default::default()
            },
            12,
        ),
        (
            PrepareRequest {
                student: Some("가상학생A".into()),
                date: Some("지난 수업일".into()),
                ..Default::default()
            },
            3,
        ),
    ] {
        let sc = scenario_v2();
        let seeded = SeededFacts::from_scenario(&sc);
        let mut backend = SourceBackend::synthetic(sc, NOW);
        let result = edu_academy_teacher::prepare(&mut backend, &seeded, &req, today());
        assert_eq!(result.status, Status::Ok);
        assert_eq!(backend.source_reads(), expected);
    }
}

#[test]
fn cached_scope_keeps_its_original_read_time_in_footer() {
    let result = prepare_v2(scenario_v2(), &req_student("가상학생A"));
    let mut coverage = result.coverage.clone();
    coverage
        .sources
        .iter_mut()
        .find(|s| s.name == "prestudy")
        .unwrap()
        .read_at = "2026-11-10T09:10:00Z".into();
    let text = edu_academy_teacher::render::lesson_prepare(&data(&result), &coverage);
    assert!(text.contains("읽은 원천 (11월 11일 18:10)"));
    assert!(text.contains("(11월 10일 18:10)"));
    display_lint(&text).unwrap();
}
#[test]
fn reminder_next_uses_overdue_before_today_for_named_students() {
    let mut sc = scenario_v2();
    sc.reminders[1].at = "2026-11-10T18:10:00+09:00".into();
    let result = prepare_v2(sc, &req_student("가상학생A"));
    let next = result.display.split("다음 확인\n").last().unwrap();
    assert!(next.starts_with("- 가상학생E의 기한 지남"));
}

fn add_report(sc: &mut Scenario, value: serde_json::Value) {
    sc.teacher_fact_reports
        .push(serde_json::from_value(value).unwrap());
}
fn notebook_rows(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|r| r.contains(" | 풀이노트 검사 | "))
        .collect()
}
#[test]
fn repair_notebook_check_covers_only_named_assignment() {
    let mut sc = scenario_v2();
    add_report(
        &mut sc,
        serde_json::json!({"kind":"solution_notebook_check","student":"가상학생A","book":"가우스 5-1-2","unit":"2-1","completed":true,"reported_on":"2026-11-11"}),
    );
    let e = prepare_v2(sc.clone(), &req_student("가상학생A"));
    let rows = notebook_rows(&e.display);
    assert_eq!(rows.len(), 2, "{}", e.display);
    assert!(
        rows.iter()
            .any(|r| r.contains("1-2 풀이노트") && r.contains("미확인(실물 확인 전)"))
    );
    sc.teacher_fact_reports.last_mut().unwrap().reported_on = "2026-11-08".into();
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(
        notebook_rows(&e.display)
            .iter()
            .all(|r| !r.contains("교사 확인("))
    );
}
#[test]
fn repair_fact_identity_and_assignment_do_not_collide() {
    let mut sc = scenario_v2();
    sc.teacher_fact_reports[4].book = "가우스 5-1-1".into();
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(
        !e.display
            .lines()
            .any(|r| r.contains("배정 풀이 확인 | 교사 확인("))
    );
    let mut sc = scenario_v2();
    sc.students[4].name = "가상학생A".into();
    let mut req = req_student("가상학생A");
    req.class_name = Some("가상반A".into());
    let e = prepare_v2(sc, &req);
    assert!(
        !e.display
            .lines()
            .any(|r| r.contains("배정 풀이 확인 | 교사 확인(")),
        "{}",
        e.display
    );
}
#[test]
fn repair_independent_rows_survive_missing_homework_and_record() {
    let mut sc = scenario_v2();
    sc.records
        .iter_mut()
        .find(|r| r.student == "가상학생A" && r.date == "2026-11-09")
        .unwrap()
        .homework
        .clear();
    let e = prepare_v2(sc.clone(), &req_student("가상학생A"));
    assert!(e.display.contains("미지참 판정"));
    assert!(e.display.contains("설명 보완(미처리)"));
    assert!(e.display.contains("배정 숙제를 읽을 수 없습니다"));
    sc.records.retain(|r| r.student != "가상학생A");
    sc.no_predecessor.push("가상학생A".into());
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(e.display.contains("미지참 판정"));
    assert!(e.display.contains("풀이노트 검사"));
    assert!(e.display.contains("설명 보완(미처리)"));
}
#[test]
fn repair_explicit_no_homework_differs_from_unreadable() {
    let mut sc = scenario_v2();
    sc.records
        .iter_mut()
        .find(|r| r.student == "가상학생A" && r.date == "2026-11-09")
        .unwrap()
        .homework = "없음".into();
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(e.display.contains("숙제가 배정되지 않았습니다"));
}
#[test]
fn repair_every_homework_item_is_used() {
    let mut sc = scenario_v2();
    let binding = sc.students[1].bindings[1].clone();
    sc.students[0].bindings.push(binding);
    sc.records
        .iter_mut()
        .find(|r| r.student == "가상학생A" && r.date == "2026-11-09")
        .unwrap()
        .homework
        .push_str("\n- 가우스 5-1-1 4-3 필수예제");
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(
        e.display
            .lines()
            .any(|r| r.contains("가우스 5-1-1 4-3 예습영상 | 영상 확인")),
        "{}",
        e.display
    );
}
#[test]
fn repair_footer_only_claims_actual_reads_and_each_scope_time() {
    let e = prepare_v2(scenario_v2(), &req_student("가상학생A"));
    let d = data(&e);
    let mut c = e.coverage;
    c.sources.retain(|s| {
        s.name != "prestudy" && s.name != "textbook_sections" && s.name != "registration"
    });
    let text = edu_academy_teacher::render::lesson_prepare(&d, &c);
    let footer = text.split("읽은 원천").nth(1).unwrap();
    assert!(!footer.contains("예습영상("));
    assert!(!footer.contains("학생·교재 등록("));
    assert!(!footer.contains("섹션 제목("));
}
#[test]
fn repair_partial_grades_name_only_unread_activity() {
    let mut sc = scenario_v2();
    let mut title = sc.sections.index_titles[0].clone();
    title.title = title.title.replace("1-2", "2-1");
    let mut grade = sc.sections.grades[0].clone();
    grade.title = title.title.clone();
    sc.sections.index_titles.push(title);
    sc.sections.grades.push(grade);
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(e.display.contains("채점 결과 8/10문항"));
    let unknown = e
        .display
        .split("미확인 사항\n")
        .nth(1)
        .unwrap()
        .split("다음 확인\n")
        .next()
        .unwrap();
    assert!(unknown.contains("2-1 유형다지기"), "{unknown}");
    assert!(!unknown.contains("필수예제"));
}
#[test]
fn repair_dynamic_cells_are_reversibly_encoded_and_linted() {
    let mut sc = scenario_v2();
    sc.reminders[0].action = "교재 | 확인\n다음\\값".into();
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert_eq!(e.status, Status::Ok, "{}", e.display);
    assert!(display_lint(&e.display).is_ok());
    assert!(e.display.contains("교재 \\u007C 확인\\n다음\\\\값"));
    assert!(!e.display.lines().any(|r| r.starts_with("다음\\값")));
    let mut sc = scenario_v2();
    sc.reminders[0].action = "untranslated".into();
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(e.stop.is_some(), "{}", e.display);
}
#[test]
fn repair_resolved_followups_and_corrections_are_omitted() {
    let mut sc = scenario_v2();
    sc.teacher_fact_reports[6].status = "closed".into();
    sc.teacher_fact_reports[7].status = "completed".into();
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(!e.display.contains("설명 보완(미처리)"));
    assert!(!e.display.contains("오답수정 일정 확인"));
}
#[test]
fn repair_class_previous_date_is_shared_and_admitted() {
    let mut sc = scenario_v2();
    sc.records
        .retain(|r| !(r.date == "2026-11-09" && r.student != "가상학생A"));
    let req = PrepareRequest {
        class_name: Some("가상반A".into()),
        date: Some("지난 수업일".into()),
        ..Default::default()
    };
    let e = prepare_v2(sc.clone(), &req);
    let d = data(&e);
    assert_eq!(d.lesson_date, "2026-11-09");
    assert!(
        d.students[1].previous_lesson.is_none(),
        "{:?}",
        d.students[1]
    );
    sc.admitted_dates.as_mut().unwrap().from = "2026-11-10".into();
    let e = prepare_v2(sc, &req);
    assert_eq!(e.stop.unwrap().code, StopCode::DateNotAdmitted);
}
#[test]
fn repair_seoul_reminder_time_and_runtime_limits() {
    let mut sc = scenario_v2();
    sc.reminders[0].at = "2026-11-10T23:30:00Z".into();
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(e.display.contains("오늘 11월 11일 08:30"));
    let mut sc = scenario_v2();
    for _ in 0..50 {
        sc.reminders.push(sc.reminders[0].clone());
    }
    let e = prepare_v2(sc, &req_student("가상학생A"));
    assert!(e.display.len() <= 6000);
    assert!(e.display.lines().count() <= 60);
    assert!(e.display.contains("생략"));
    assert!(display_lint(&e.display).is_ok());
}

#[test]
fn repair_keyed_facts_and_ambiguous_abbreviations_stay_scoped() {
    let mut sc = scenario_v2();
    sc.students[4].name = "가상학생A".into();
    let value = serde_json::to_value(&sc.teacher_fact_reports[0]).unwrap();
    // Supply an explicit resolved identity for the two relevant facts.
    sc.teacher_fact_reports[0] = serde_json::from_value({
        let mut v = value;
        v["student_key"] = "1001".into();
        v
    })
    .unwrap();
    let mut v = serde_json::to_value(&sc.teacher_fact_reports[4]).unwrap();
    v["student_key"] = "1001".into();
    sc.teacher_fact_reports[4] = serde_json::from_value(v).unwrap();
    let req = PrepareRequest {
        student: Some("가상학생A".into()),
        class_name: Some("가상반A".into()),
        ..Default::default()
    };
    let e = prepare_v2(sc.clone(), &req);
    assert!(e.display.contains("배정 풀이 확인 | 교사 확인(11월 11일)"));
    add_report(
        &mut sc,
        serde_json::json!({"kind":"assigned_stages","student":"가상학생A","student_key":"1001","assigned_on":"2026-11-09","book":"가우스 5-1-1","unit":"4-3","activity":"필수예제","stages":["solving"],"reported_on":"2026-11-09"}),
    );
    let e = prepare_v2(sc, &req);
    assert!(!e.display.contains("배정 풀이 확인 | 교사 확인("));
}
#[test]
fn repair_footer_mixed_scope_times_do_not_relabel_other_students() {
    let e = prepare_v2(
        scenario_v2(),
        &PrepareRequest {
            class_name: Some("가상반A".into()),
            ..Default::default()
        },
    );
    let mut c = e.coverage.clone();
    c.sources
        .iter_mut()
        .find(|s| s.name == "prestudy" && s.scope.starts_with("가상학생A"))
        .unwrap()
        .read_at = "2026-11-10T09:10:00Z".into();
    let text = edu_academy_teacher::render::lesson_prepare(&data(&e), &c);
    assert!(text.contains("예습영상(가상학생A 가우스 5-1-2 2-1) (11월 10일 18:10)"));
    assert!(text.contains("예습영상(가상학생B 가우스 5-1-1 4-3)"));
    assert!(!text.contains("예습영상(가상학생B 가우스 5-1-1 4-3) (11월 10일"));
}
#[test]
fn repair_missing_reads_are_named_outside_read_footer() {
    let sc = scenario_v2();
    let seeded = SeededFacts::from_scenario(&sc);
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
    let e = edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생A"), today());
    assert_eq!(e.status, Status::Ok);
    let parts = e.display.split("읽은 원천").collect::<Vec<_>>();
    let footer = parts.last().unwrap();
    assert!(!footer.contains("예습영상("));
    assert!(e.display.contains("예습영상: 원천을 읽지 못해"));
}
#[test]
fn repair_runtime_bound_keeps_whole_student_blocks_and_names_first_omitted() {
    let mut sc = scenario_v2();
    sc.teacher_fact_reports[6].action = "가".repeat(1500);
    let e = prepare_v2(
        sc,
        &PrepareRequest {
            class_name: Some("가상반A".into()),
            ..Default::default()
        },
    );
    assert!(e.display.len() <= 6000);
    assert!(e.display.lines().count() <= 60);
    assert!(e.display.contains("가상학생A부터 생략"));
    assert!(!e.display.contains("가상학생A |"));
    assert!(!e.display.contains("가상학생B |"));
    assert!(
        e.coverage
            .unknowns
            .iter()
            .any(|s| s.contains("display_truncated:가상학생A"))
    );
    assert!(!e.display.contains("명단이 비어 있어"));
    assert!(
        e.display
            .ends_with("이번 결과는 읽기만 했고 저장한 것은 없습니다.")
    );
}

#[test]
fn repair_reused_ports_keep_their_supplied_scope_clock() {
    let sc = scenario_v2();
    let seeded = SeededFacts::from_scenario(&sc);
    let mut backend = SourceBackend::synthetic(sc.clone(), NOW);
    let first =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생A"), today());
    assert_eq!(first.status, Status::Ok);
    let calls = (backend.prestudy_port_calls(), backend.textbook_port_calls());
    let mut fresh = sc;
    fresh.read_at = Some("2026-11-11T10:10:00Z".into());
    let second = edu_academy_teacher::prepare(
        &mut backend,
        &SeededFacts::from_scenario(&fresh),
        &req_student("가상학생A"),
        today(),
    );
    assert_eq!(second.status, Status::Ok);
    assert!(backend.prestudy_port_calls() > calls.0 && backend.textbook_port_calls() > calls.1);
    assert!(second.display.contains("읽은 원천 (11월 11일 19:10)"));
    assert!(
        second
            .display
            .contains("예습영상(가상학생A 2-1·대기/완료목록) (11월 11일 18:10)")
    );
}
