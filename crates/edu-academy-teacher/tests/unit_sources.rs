//! Alternate raw ports exercise the real source parsers and failure boundaries.
use edu_academy_core::js::Js;
use edu_academy_sources::{
    data::{Error, Result},
    history::Port,
    native::SourcePort,
    prestudy::PrestudyPort,
    textbook::{FetchPolicy, TextbookResponse},
    textbook_index::{BookScope, TextbookSectionPort, TextbookStudent},
};
use edu_academy_teacher::{
    backend::{BackendIdentity, ResolvedStudent, SourceBackend, StudentEntry},
    facts::SeededFacts,
    synthetic::*,
    unit::status,
    unit_fixture::*,
    unit_sources::*,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
fn scenario() -> Scenario {
    load_scenario(include_str!("fixtures/scenario-v3.json")).unwrap()
}
#[derive(Default)]
struct Calls {
    events: Vec<String>,
    timeout: usize,
}
struct TextPort {
    inner: SyntheticTextbookPort,
    calls: Rc<RefCell<Calls>>,
    paper_fail: bool,
    note_fail: bool,
    detail_fail: Option<String>,
    progress_fail: bool,
}
impl TextbookSectionPort for TextPort {
    fn progress(
        &mut self,
        s: &TextbookStudent,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.calls
            .borrow_mut()
            .events
            .push(format!("progress:{}", s.student_key()));
        if self.progress_fail {
            return Err(Error::error("source_unavailable"));
        }
        self.inner.progress(s, n, p)
    }
    fn wrong_notes(
        &mut self,
        s: &TextbookStudent,
        b: &BookScope,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.calls
            .borrow_mut()
            .events
            .push(format!("notes:{}:{}", s.student_key(), b.book_no()));
        if self.note_fail {
            return Err(Error::error("source_unavailable"));
        }
        if self.calls.borrow().timeout > 0 {
            self.calls.borrow_mut().timeout -= 1;
            return Err(Error::error("timeout"));
        }
        self.inner.wrong_notes(s, b, n, p)
    }
    fn detail(
        &mut self,
        s: &TextbookStudent,
        a: &str,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.calls
            .borrow_mut()
            .events
            .push(format!("detail:{}:{a}", s.student_key()));
        if self.detail_fail.as_deref() == Some(a) {
            return Err(Error::error("source_unavailable"));
        }
        self.inner.detail(s, a, n, p)
    }
    fn paper_search(
        &mut self,
        s: &TextbookStudent,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.calls
            .borrow_mut()
            .events
            .push(format!("paper:{}", s.student_key()));
        if self.paper_fail {
            return Err(Error::error("source_unavailable"));
        }
        self.inner.paper_search(s, n, p)
    }
}
fn backend<T: TextbookSectionPort>(
    s: &Scenario,
    text: T,
) -> SourceBackend<SyntheticDayPort, SyntheticPrestudyPort, T> {
    SourceBackend::new(
        SyntheticDayPort::new(s.clone()),
        SyntheticPrestudyPort::new(s.clone()),
        text,
        s.students
            .iter()
            .map(|s| StudentEntry {
                name: s.name.clone(),
                key: s.key.clone(),
                class_name: s.class_name.clone(),
            })
            .collect(),
        BackendIdentity {
            teacher_id: TEACHER_ID.into(),
            teacher_key: TEACHER_KEY.into(),
            teacher_label: TEACHER_LABEL.into(),
            read_at: NOW.into(),
        },
    )
}
fn text_port(s: &Scenario) -> TextPort {
    TextPort {
        inner: SyntheticTextbookPort::new(s.clone()),
        calls: Rc::new(RefCell::new(Calls::default())),
        paper_fail: false,
        note_fail: false,
        detail_fail: None,
        progress_fail: false,
    }
}
fn call<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort, H: Port>(
    b: &mut SourceBackend<D, P, T>,
    d: &mut DailyTestReader<H>,
    s: &Scenario,
    value: serde_json::Value,
) -> edu_academy_teacher::envelope::Envelope {
    status(
        b,
        d,
        &SeededFacts::from_scenario(s),
        &serde_json::from_value(value).unwrap(),
        &ReadClock {
            read_at: NOW.into(),
        },
    )
}
fn reader(s: &Scenario) -> DailyTestReader<SyntheticDailyTestPort> {
    let c = SharedFrameClock::new(s.dailytest_frames.clone());
    DailyTestReader::new(
        SyntheticDailyTestPort::new(s.clone(), c.clone()),
        c,
        TEACHER_ID,
    )
}
struct DailyPort {
    inner: SyntheticDailyTestPort,
    events: Rc<RefCell<Vec<String>>>,
    wrong_owner: bool,
    wrong_header: bool,
    fail_header: Option<String>,
    fail_page: bool,
    fail_selector: Option<String>,
    observed_at: Option<String>,
    timeout: usize,
}
impl Port for DailyPort {
    fn dated_students(&mut self, input: &Js) -> Result<Js> {
        self.events.borrow_mut().push("dated".into());
        let mut rows = self.inner.dated_students(input)?;
        if self.wrong_owner {
            let mut row = rows.array()[0].clone();
            row.set("teacherKey", "9999".into());
            rows = Js::Arr(vec![row]);
        }
        Ok(rows)
    }
    fn daily_page(&mut self, s: &Js, selector: &Js) -> Result<Js> {
        self.events.borrow_mut().push(format!("page:{selector:?}"));
        if self.timeout > 0 {
            self.timeout -= 1;
            return Err(Error::error("timeout"));
        }
        if self.fail_page
            || self
                .fail_selector
                .as_ref()
                .is_some_and(|x| edu_academy_sources::data::string(selector) == *x)
        {
            return Err(Error::error("source_unavailable"));
        }
        let mut raw = self.inner.daily_page(s, selector)?;
        if let Some(at) = &self.observed_at {
            raw.set("observedAt", at.clone().into());
        }
        Ok(raw)
    }
    fn paper_header(&mut self, s: &Js, code: &Js) -> Result<Js> {
        self.events.borrow_mut().push("header".into());
        let key =
            edu_academy_sources::data::string(&edu_academy_sources::history::decode_code(code)?);
        if self.fail_header.as_deref() == Some(&key) {
            return Err(Error::error("source_unavailable"));
        }
        let mut header = self.inner.paper_header(s, code)?;
        if self.wrong_header {
            header.set("paperKey", "9999".into());
        }
        Ok(header)
    }
}
fn daily_port(s: &Scenario) -> (DailyPort, SharedFrameClock) {
    let c = SharedFrameClock::new(s.dailytest_frames.clone());
    (
        DailyPort {
            inner: SyntheticDailyTestPort::new(s.clone(), c.clone()),
            events: Rc::new(RefCell::new(Vec::new())),
            wrong_owner: false,
            wrong_header: false,
            fail_header: None,
            fail_page: false,
            fail_selector: None,
            observed_at: None,
            timeout: 0,
        },
        c,
    )
}
#[test]
fn source_failures_retry_only_timeouts_and_count_reads() {
    let s = scenario();
    let p = text_port(&s);
    let events = p.calls.clone();
    events.borrow_mut().timeout = 1;
    let mut b = backend(&s, p);
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"섹션"}),
    );
    assert!(out.stop.is_none());
    let es = &events.borrow().events;
    assert_eq!(es.iter().filter(|e| e.starts_with("notes:")).count(), 3);
    assert_eq!(es.iter().filter(|e| e.starts_with("detail:")).count(), 1);
    assert_eq!(b.read_counts(), (3, 3)); // binding, index, one grade; index/grade revalidate progress.
    let mut p = text_port(&s);
    p.note_fail = true;
    let events = p.calls.clone();
    let mut b = backend(&s, p);
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"2-3","topic":"섹션"}),
    );
    assert!(out.stop.is_some());
    assert_eq!(
        events
            .borrow()
            .events
            .iter()
            .filter(|e| e.starts_with("notes:"))
            .count(),
        1
    );
    assert_eq!(b.read_counts(), (2, 1));
    let mut calls = 0;
    let result: Result<usize> = retry_timeout(|| {
        calls += 1;
        Err(Error::error("timeout"))
    });
    assert!(result.is_err());
    assert_eq!(calls, 2);
    calls = 0;
    assert_eq!(
        retry_timeout(|| {
            calls += 1;
            Ok(None::<usize>)
        })
        .unwrap(),
        None
    );
    assert_eq!(calls, 1);
}
#[test]
fn q16_failed_paper_search_is_unknown_not_two_source_absence() {
    let s = scenario();
    let mut p = text_port(&s);
    p.paper_fail = true;
    let mut b = backend(&s, p);
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"2-3","topic":"섹션"}),
    );
    assert!(out.stop.is_none());
    assert!(!out.display.contains("두 원천 모두에 없음"));
    assert!(
        !out.display
            .contains("오답노트와 시험지 검색 각 첫 관측 페이지")
    );
    assert!(!out.coverage.unknowns.is_empty());
    assert_eq!(out.data["sections"]["absent_units"], json!([]));
}
#[test]
fn failed_detail_beside_success_keeps_the_success_and_qualified_sources() {
    let s = scenario();
    let mut p = text_port(&s);
    p.detail_fail = Some("8200".into());
    let mut b = backend(&s, p);
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"1-2","topic":"섹션"}),
    );
    assert!(out.stop.is_none());
    assert_eq!(out.data["sections"]["records"].as_array().unwrap().len(), 1);
    assert!(out.display.contains("원천 점수: 60"));
    assert!(!out.display.contains("원천 점수: 80"));
    assert!(
        out.coverage
            .unknowns
            .iter()
            .any(|u| u.starts_with("section_grade_unavailable:"))
    );
}
#[test]
fn partial_topics_are_ok_total_requested_failure_stops() {
    let s = scenario();
    let (p, c) = daily_port(&s);
    let mut p = p;
    p.fail_page = true;
    let mut d = DailyTestReader::new(p, c, TEACHER_ID);
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생A","unit":"1-2"}),
    );
    assert!(out.stop.is_none());
    assert!(out.coverage.sources.iter().all(|s| s.name != "dailytest"));
    assert!(out.coverage.unknowns.contains(&"daily_unavailable".into()));
    let (p, c) = daily_port(&s);
    let mut p = p;
    p.fail_page = true;
    let mut d = DailyTestReader::new(p, c, TEACHER_ID);
    let out = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert_eq!(
        out.stop.unwrap().code,
        edu_academy_teacher::envelope::StopCode::SourceUnavailable
    );
}
#[test]
fn owner_and_exact_header_identity_are_enforced_by_history() {
    let s = scenario();
    for owner in [true, false] {
        let (mut p, c) = daily_port(&s);
        p.wrong_owner = owner;
        p.wrong_header = !owner;
        let events = p.events.clone();
        let mut d = DailyTestReader::new(p, c, TEACHER_ID);
        let mut b = backend(&s, text_port(&s));
        let out = call(
            &mut b,
            &mut d,
            &s,
            json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
        );
        if owner {
            assert!(out.stop.is_some());
            assert_eq!(events.borrow().len(), 1);
        } else {
            assert!(out.stop.is_none());
            assert_eq!(out.data["daily"]["attempts"], json!([]));
            assert!(!out.display.contains("미확인(읽은 기간에 없음)"));
            assert!(!out.coverage.unknowns.is_empty());
        }
    }
}
#[test]
fn daily_exact_key_join_retests_and_unknown_dates() {
    let mut s = scenario();
    let a = s.dailytest_frames[0].attempts[0].clone();
    for i in 1..=2 {
        let mut other = a.clone();
        other.attempt_key = format!("{}", 9200 + i);
        other.score = Some(20.0 + i as f64);
        s.dailytest_frames[0].attempts.push(other);
    }
    let mut b = backend(&s, text_port(&s));
    let mut d = reader(&s);
    let out = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    let attempts = out.data["daily"]["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 3);
    assert!(
        attempts
            .iter()
            .all(|a| a["level"].is_null() && a["occurred_at"].is_null())
    );
    assert_eq!(
        out.display
            .lines()
            .filter(|l| l.contains("원천: 서버 확인: 원천 점수"))
            .count(),
        3
    );
    assert_eq!(
        out.display
            .matches("시험 날짜: 미확인(날짜 미확인)")
            .count(),
        3
    );
    assert_eq!(
        out.display
            .matches("시험 수준: 미확인(수준 표기 없음)")
            .count(),
        3
    );
    assert!(out.display.contains("같은 시험 시도인지: 미확인"));
    assert_eq!(d.source_requests(), 6);
    let mut s = scenario();
    s.dailytest_frames[0].headers[0].edition_mapping = None;
    s.dailytest_frames[0].headers[0].unit_mapping = None;
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert!(!out.display.contains("미확인(읽은 기간에 없음)"));
    assert_eq!(
        out.data["daily"]["attempts"][0]["edition_mapping"],
        json!(null)
    );
}
#[test]
fn failed_header_does_not_latch_an_unrelated_success() {
    let mut s = scenario();
    let mut a = s.dailytest_frames[0].attempts[0].clone();
    a.paper_key = "9991".into();
    a.attempt_key = "9992".into();
    a.score = Some(61.0);
    s.dailytest_frames[0].attempts.push(a.clone());
    s.dailytest_frames[0].headers.push(a);
    let (mut p, c) = daily_port(&s);
    p.fail_header = Some("9001".into());
    let mut d = DailyTestReader::new(p, c, TEACHER_ID);
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert!(out.stop.is_none());
    assert_eq!(out.data["daily"]["attempts"].as_array().unwrap().len(), 1);
    assert!(out.display.contains("원천 점수 61"));
    assert!(!out.coverage.unknowns.is_empty());
}
#[test]
fn frame_latches_once_and_timeout_retries_do_not_advance_it() {
    let s = scenario();
    let (mut p, c) = daily_port(&s);
    p.timeout = 1;
    let mut d = DailyTestReader::new(p, c.clone(), TEACHER_ID);
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생B","unit":"4-4","topic":"DailyTest"}),
    );
    assert!(out.stop.is_none());
    assert_eq!(out.data["read_at"], "2026-11-11T09:00:00+09:00");
    assert_eq!(d.source_requests(), 4);
    assert_eq!(c.reads("1002"), 1);
    let out = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생B","unit":"4-4","topic":"DailyTest"}),
    );
    assert_eq!(out.data["read_at"], "2026-11-11T10:00:00+09:00");
    assert_eq!(c.reads("1002"), 2);
    let _ = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생B","topic":"클리닉"}),
    );
    assert_eq!(c.reads("1002"), 2);
}
#[test]
fn student_and_edition_scope_survive_duplicate_titles() {
    let mut s = scenario();
    let mut other = s.sections.attempts[0].clone();
    other.student_key = "1003".into();
    other.course_key = "102".into();
    other.book_no = "202".into();
    other.attempt_key = "7777".into();
    other.paper_key = "7776".into();
    other.source_score = 13.0;
    s.sections.attempts.push(other);
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"섹션"}),
    );
    assert!(out.display.contains("원천 점수: 80"));
    assert!(!out.display.contains("원천 점수: 13"));
    for (book, score, not_score) in [("가우스 5-1-1", 90, 70), ("가우스 5-1-2", 70, 90)] {
        let out = call(
            &mut b,
            &mut reader(&s),
            &s,
            json!({"student":"가상학생B","book":book,"topic":"섹션"}),
        );
        assert_eq!(out.data["sections"]["records"].as_array().unwrap().len(), 1);
        assert_eq!(
            out.data["sections"]["records"][0]["score"].as_f64(),
            Some(score as f64)
        );
        assert_ne!(
            out.data["sections"]["records"][0]["score"].as_f64(),
            Some(not_score as f64)
        );
    }
}
#[test]
fn no_record_is_success_and_no_grade_is_fabricated() {
    let mut s = scenario();
    s.sections.attempts.clear();
    s.sections.index_titles.clear();
    s.sections.grades.clear();
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"1-2","topic":"섹션"}),
    );
    assert!(out.stop.is_none());
    assert!(
        out.data["sections"]["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(!out.display.contains("두 원천 모두에 없음"));
    assert!(!out.display.contains("원천 점수:"));
    assert_eq!(b.read_counts(), (2, 2));
}
#[test]
fn direct_history_read_preserves_the_resolved_key() {
    let s = scenario();
    let mut d = reader(&s);
    let read = d
        .read(&ResolvedStudent {
            name: "가상학생C".into(),
            key: "1003".into(),
            class_name: None,
        })
        .unwrap();
    assert_eq!(read.attempts[0].score, Some(70.0));
    assert!(!read.scope.complete);
}

#[test]
fn partial_daily_page_keeps_success_and_actual_source_timestamps() {
    let s = scenario();
    let (mut p, c) = daily_port(&s);
    p.fail_selector = Some("11".into());
    p.observed_at = Some("2026-11-11T08:45:00+09:00".into());
    let mut d = DailyTestReader::new(p, c, TEACHER_ID);
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut d,
        &s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert!(out.stop.is_none());
    assert!(out.display.contains("원천 점수 90"));
    assert!(out.display.contains("08:45"));
    assert!(
        out.coverage
            .sources
            .iter()
            .any(|s| s.name == "dailytest" && s.read_at == "2026-11-11T08:45:00+09:00")
    );
    assert!(
        !out.coverage
            .sources
            .iter()
            .any(|s| s.name == "dailytest_relative_page")
    );
    assert!(out.coverage.unknowns.contains(&"daily_partial_page".into()));
}
#[test]
fn dated_retests_keep_each_attempt_and_date() {
    let mut s = scenario();
    s.dailytest_frames[0].attempts[0].occurred_at = Some("2026-11-09".into());
    s.dailytest_frames[0].headers[0].occurred_at = Some("2026-11-09".into());
    let mut a = s.dailytest_frames[0].attempts[0].clone();
    a.attempt_key = "9909".into();
    a.score = Some(22.0);
    a.occurred_at = Some("2026-11-11".into());
    s.dailytest_frames[0].attempts.push(a.clone());
    s.dailytest_frames[0].headers.push(a);
    let mut b = backend(&s, text_port(&s));
    let out = call(
        &mut b,
        &mut reader(&s),
        &s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert!(out.display.contains("11월 9일"));
    assert!(out.display.contains("11월 11일"));
    assert!(
        out.data["daily"]["attempts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["score"].as_f64() == Some(22.0) && a["occurred_at"] == "2026-11-11")
    );
}
