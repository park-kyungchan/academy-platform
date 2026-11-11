use chrono::{NaiveDate, Weekday};
use edu_academy_teacher::{
    backend::SourceBackend,
    envelope::Status,
    facts::{ClassFact, SeededFacts, TeacherFacts},
    lesson::{PrepareRequest, previous_lesson, scan_line},
    load_scenario,
    render::{Verdict, display_lint},
    synthetic::Scenario,
};

const SCENARIO: &str = include_str!("fixtures/scenario-v1.json");
const DISPLAY_Q01: &str = include_str!("fixtures/display-q01.txt");
const DISPLAY_Q02: &str = include_str!("fixtures/display-q02.txt");

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()
}
fn scenario() -> Scenario {
    load_scenario(SCENARIO).unwrap()
}
fn backend() -> SourceBackend {
    SourceBackend::new(scenario())
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

// Q01: S-017, S-019, S-026; sections of 2-1 not observed; no carry-over.
#[test]
fn q01_single_student_previous_lesson_and_checks() {
    let mut backend = backend();
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ok);
    assert_eq!(envelope.display, DISPLAY_Q01);
    display_lint(&envelope.display).unwrap();
    assert!(!envelope.display.contains("못 가져온 숙제"));
    assert!(!envelope.display.contains("2026-11-04"));
    let student = &envelope.data.students[0];
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
    // Waiting must never claim teacher review in display wording beyond the
    // fixed label, which says teacher review is pending.
    assert!(envelope.display.contains("교사 확인 전"));
    // Source reads: 1 record probe + 1 prestudy + 2 section checks.
    assert_eq!(backend.source_reads(), 4);
}

// Q02: S-002 roster order, S-019..S-021, S-027 not observed (never not done).
#[test]
fn q02_class_of_three_roster_order() {
    let mut backend = backend();
    let req = PrepareRequest {
        class_name: Some("가상반A".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ok);
    assert_eq!(envelope.display, DISPLAY_Q02);
    display_lint(&envelope.display).unwrap();
    let names: Vec<&str> = envelope
        .data
        .students
        .iter()
        .map(|s| s.student.as_str())
        .collect();
    assert_eq!(names, ["가상학생A", "가상학생B", "가상학생C"]);
    for student in &envelope.data.students {
        assert_eq!(student.previous_lesson.as_ref().unwrap().date, "2026-11-09");
    }
    let b = &envelope.data.students[1];
    assert_eq!(b.checks[0].verdict, Verdict::NotObserved);
    assert!(!envelope.display.contains("안 함"));
    assert!(!envelope.display.contains("하지 않"));
    assert_eq!(backend.source_reads(), 12);
}

#[test]
fn previous_lesson_uses_moved_to_date() {
    let mut backend = backend();
    let student = backend.resolve_student("가상학생A").unwrap();
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
    // A student whose (stub) class has no seeded weekdays: the class name of
    // the resolved student is replaced by the stub through a custom facts
    // view that reports no schedule for it.
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
    let mut backend = backend();
    let student = backend.resolve_student("가상학생A").unwrap();
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
    let mut backend = backend();
    let req = PrepareRequest {
        student: Some("가상학생A".into()),
        date: Some("2026-10-01".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ok);
    assert!(envelope.data.students[0].previous_lesson.is_none());
    let range = envelope.data.students[0].searched_range.as_deref().unwrap();
    assert!(range.contains("부터"), "{range}");
    assert!(envelope.display.contains(range), "{}", envelope.display);
}

#[test]
fn ambiguous_student_asks_on_student_field() {
    // Two admitted students share one name (a second admission entry).
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO).unwrap();
    let mut duplicate = value["students"][0].clone();
    duplicate["key"] = serde_json::json!("1009");
    value["students"].as_array_mut().unwrap().push(duplicate);
    let scenario = load_scenario(&value.to_string()).unwrap();
    let mut backend = SourceBackend::new(scenario.clone());
    let seeded = SeededFacts::from_scenario(&scenario);
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &seeded, &req_student("가상학생A"), today());
    assert_eq!(envelope.status, Status::Ask);
    assert_eq!(envelope.ask.as_ref().unwrap().field, "student");
}

#[test]
fn unknown_student_stops_student_not_found() {
    let mut backend = backend();
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &req_student("가상학생Z"), today());
    assert_eq!(envelope.status, Status::Unavailable);
    assert_eq!(envelope.stop.as_ref().unwrap().code, "student_not_found");
}

#[test]
fn section_titles_of_progress_minor_are_not_checked() {
    // S-030/S-031 titles belong to progress minor 1-2; only homework minor
    // 2-1 is checked in slice 1, so no check graded from those titles.
    let mut backend = backend();
    let envelope =
        edu_academy_teacher::prepare(&mut backend, &facts(), &req_student("가상학생A"), today());
    for check in &envelope.data.students[0].checks {
        assert_ne!(check.verdict, Verdict::Found);
        assert!(!check.scope.starts_with("1-2"), "{}", check.scope);
    }
    // The graded path itself works on the source layer for that title.
    let student =
        edu_academy_sources::textbook_index::TextbookStudent::new("1001", "가상학생A").unwrap();
    let scope = edu_academy_sources::textbook_index::BookScope::new("101", "201").unwrap();
    let graded = backend.section_verdict(&student, &scope, "1-2", "필수예제");
    assert_eq!(graded.verdict, Verdict::Found);
    assert_eq!((graded.correct, graded.total), (Some(8), Some(10)));
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
    // The earliest record of a student (2026-11-02 for A) reads through the
    // real Backend, which requires a predecessor row.
    let mut backend = backend();
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
        let mut backend = backend();
        let req = PrepareRequest {
            student: Some("가상학생A".into()),
            date: Some(input.into()),
            ..Default::default()
        };
        let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
        assert_eq!(envelope.data.lesson_date, expected, "{input}");
    }
    let mut backend = backend();
    let req = PrepareRequest {
        student: Some("가상학생A".into()),
        date: Some("다음 주".into()),
        ..Default::default()
    };
    let envelope = edu_academy_teacher::prepare(&mut backend, &facts(), &req, today());
    assert_eq!(envelope.status, Status::Ask);
    assert_eq!(envelope.ask.as_ref().unwrap().field, "date");
}
