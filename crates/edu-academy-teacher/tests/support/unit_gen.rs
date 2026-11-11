//! Deterministic, fixture-independent classroom inputs and alternate raw ports.
//! Seeds describe generated inputs, not implementation-selected scenario rows.
use edu_academy_core::js::Js;
use edu_academy_sources::{
    data::{Error, Result, string},
    history::Port,
    textbook::{FetchPolicy, TextbookResponse},
    textbook_index::{BookScope, TextbookSectionPort, TextbookStudent},
};
use edu_academy_teacher::{
    backend::{BackendIdentity, SourceBackend, StudentEntry},
    envelope::{Coverage, Envelope, SourceRead},
    facts::SeededFacts,
    synthetic::{
        NativeSectionAttempt, NativeSectionItem, Scenario, SyntheticDayPort, SyntheticPrestudyPort,
        SyntheticTextbookPort, TEACHER_ID, TEACHER_KEY, TEACHER_LABEL, load_scenario,
    },
    unit::status,
    unit_data::*,
    unit_fixture::{ReadClock, SharedFrameClock, SyntheticDailyTestPort},
    unit_sources::DailyTestReader,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

pub const NOW: &str = "2026-11-11T09:00:00+09:00";
pub const CASES: u64 = 200;
pub const NAMES: [&str; 6] = [
    "가상학생A",
    "가상학생B",
    "가상학생C",
    "가상학생D",
    "가상학생E",
    "가상학생F",
];
const CLASSES: [&str; 3] = ["가상반A", "가상반B", "가상반C"];

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }
    pub fn pick(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }
}

/// Failure injection is at raw ports, below the real parsers. The audit records
/// completed raw reads rather than assuming a requested source was observed.
#[derive(Clone, Debug, Default)]
pub struct Faults {
    pub detail_fail: Vec<String>,
    pub notes_fail: bool,
    pub paper_fail: bool,
    pub timeout_once: bool,
    pub daily_fail: Option<String>,
    pub relative_rows: bool,
    pub page_at: Option<String>,
    pub header_at: Option<String>,
    pub header_fail: bool,
    pub prestudy_fail: bool,
}
#[derive(Clone, Debug)]
pub struct ReadEvent {
    pub source: String,
    pub scope: String,
    pub succeeded: bool,
}
type Audit = Rc<RefCell<Vec<ReadEvent>>>;
fn event(a: &Audit, source: &str, scope: &str, succeeded: bool) {
    a.borrow_mut().push(ReadEvent {
        source: source.into(),
        scope: scope.into(),
        succeeded,
    });
}

pub struct Case {
    pub seed: u64,
    pub scenario: Scenario,
    pub request: UnitStatusRequest,
    pub faults: Faults,
}
impl Case {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let students = 1 + rng.pick(4);
        let books = 1 + rng.pick(3);
        let labels: Vec<_> = (0..books)
            .map(|b| format!("가우스 5-1-{}", b + 1))
            .collect();
        let mut scenario = load_scenario(
            &json!({
                "today":"2026-11-11", "unit_read_at":NOW, "read_at":NOW,
                "students": (0..students).map(|i| json!({
                    "name":NAMES[i], "key":format!("{}",1001+i),
                    "class_name":CLASSES[i%3],
                    "bindings":labels.iter().enumerate().map(|(b,label)|json!({
                        "label":label,"course_key":format!("{}",101+b),
                        "book_no":format!("{}",201+b)
                    })).collect::<Vec<_>>()
                })).collect::<Vec<_>>(),
                "classes":[],"records":[],"prestudy":{},"sections":{}
            })
            .to_string(),
        )
        .expect("valid generator skeleton");
        for (b, edition) in labels.iter().enumerate() {
            let count = 1 + rng.pick(70);
            scenario.edition_trees.push(EditionTree {
                edition: edition.clone(),
                family: "gauss".into(),
                school_level: "elementary".into(),
                read_at: NOW.into(),
                chapters: vec![Chapter {
                    code: "1".into(),
                    title: format!("가상대단원{}", b + 1),
                    middles: vec![],
                    minors: (1..=count)
                        .map(|i| Minor {
                            code: format!("1-{i}"),
                            title: format!("가상주제{i}"),
                            activities: vec![
                                "필수예제".into(),
                                "유형다지기".into(),
                                "실력쌓기".into(),
                                "대단원총괄".into(),
                            ],
                            unit_wide: i == count,
                        })
                        .collect(),
                }],
            });
        }
        for i in 0..students {
            for (b, edition) in labels.iter().enumerate() {
                let mut a = section(i, b, "1-1", "필수예제", 0);
                a.source_score = 10.25 + rng.pick(80) as f64;
                a.source_date = (rng.pick(2) == 0).then(|| "2026-11-09".into());
                scenario.sections.attempts.push(a);
                let attempt = DailyAttempt {
                    paper_key: format!("{}", 91000 + i * 100 + b),
                    attempt_key: format!("{}", 92000 + i * 100 + b),
                    edition_mapping: Some(edition.clone()),
                    unit_mapping: Some("1-1".into()),
                    source_title: format!("가상시험{b}"),
                    level: None,
                    occurred_at: None,
                    score: Some(31.25 + i as f64),
                };
                scenario.dailytest_frames.push(DailyFrame {
                    student_key: format!("{}", 1001 + i),
                    read_at: NOW.into(),
                    scope: DailyScope {
                        from: "2026-10-28".into(),
                        to: "2026-11-11".into(),
                        selector_label: "현재 필터".into(),
                        page_label: "첫 관측 페이지".into(),
                        complete: false,
                    },
                    expected_today: vec![],
                    attempts: vec![attempt.clone()],
                    headers: vec![attempt],
                });
            }
        }
        Self {
            seed,
            request: UnitStatusRequest {
                student: NAMES[0].into(),
                book: Some(labels[0].clone()),
                unit: Some("1-1".into()),
                topic: Some("섹션".into()),
                ..Default::default()
            },
            scenario,
            faults: Faults::default(),
        }
    }
    /// Project to a small, explicit scope while retaining generated neighbours.
    pub fn scope(&mut self, count: usize, topic: &str) {
        let tree = &mut self.scenario.edition_trees[0];
        let template = tree.chapters[0].minors[0].clone();
        tree.chapters[0].minors = (1..=count)
            .map(|i| Minor {
                code: format!("1-{i}"),
                title: format!("가상주제{i}"),
                ..template.clone()
            })
            .collect();
        self.request.unit = None;
        self.request.unit_from = None;
        self.request.unit_to = None;
        self.request.topic = Some(topic.into());
    }
    pub fn own_sections(&mut self, rows: Vec<NativeSectionAttempt>) {
        self.scenario
            .sections
            .attempts
            .retain(|a| a.student_key != "1001" || a.book_no != "201");
        self.scenario.sections.attempts.extend(rows);
    }
    pub fn own_daily(&mut self, attempts: Vec<DailyAttempt>) {
        let f = &mut self.scenario.dailytest_frames[0];
        f.attempts = attempts.clone();
        f.headers = attempts;
    }
    pub fn judgment(
        &mut self,
        topic: &str,
        activity: Option<&str>,
        level: Option<&str>,
        snapshot: Option<&str>,
    ) {
        self.scenario.teacher_judgments.push(TeacherJudgment {
            course_key: None,
            book_no: None,
            student_key: "1001".into(),
            edition: self.scenario.edition_trees[0].edition.clone(),
            unit: "1-1".into(),
            topic: topic.into(),
            activity: activity.map(str::to_string),
            level: level.map(str::to_string),
            stage: None,
            snapshot: snapshot.map(str::to_string),
            verdict: UnitVerdict::Confirmed,
            actor: None,
            recorded_at: None,
        });
    }
    pub fn stage(&mut self, unit: &str, kind: &str, stage: &str, completed: bool) {
        self.scenario.teacher_fact_reports.push(serde_json::from_value(json!({
            "student_key":"1001","student":NAMES[0],"book":self.scenario.edition_trees[0].edition,
            "unit":unit,"kind":kind,"stage":stage,"completed":completed,"reported_on":"2026-11-11"
        })).unwrap());
    }
    pub fn describe(&self) -> String {
        format!(
            "seed={} students={} books={} leaves={} topic={:?} unit={:?} faults={:?}",
            self.seed,
            self.scenario.students.len(),
            self.scenario.edition_trees.len(),
            self.scenario
                .edition_trees
                .first()
                .map_or(0, |t| t.leaves().len()),
            self.request.topic,
            self.request.unit,
            self.faults
        )
    }
    pub fn run(&self) -> Envelope {
        self.session().call(&self.request)
    }
    /// The status reader currently aborts before a relative read if the primary
    /// page fails. The public renderer still accepts this valid partial input;
    /// construct it from the generator's oracle, not from status output.
    pub fn render_relative_only(&self) -> String {
        let tree = &self.scenario.edition_trees[0];
        let frame = &self.scenario.dailytest_frames[0];
        let source = SourceRead {
            name: "dailytest_relative_page".into(),
            read_at: frame.read_at.clone(),
            scope: "selector 11 / page 1 / relative period bounds unavailable / not complete"
                .into(),
        };
        let mut coverage = Coverage {
            sources: vec![source.clone()],
            unknowns: vec!["daily_partial_page".into()],
        };
        let mut data = UnitData {
            course_key: "101".into(),
            book_no: "201".into(),
            student_key: "1001".into(),
            student: NAMES[0].into(),
            edition: tree.edition.clone(),
            family: tree.family.clone(),
            topic: "DailyTest".into(),
            default_read_at: NOW.into(),
            read_at: frame.read_at.clone(),
            tree_read_at: tree.read_at.clone(),
            minors: tree.leaves(),
            activity: None,
            level: None,
            daily: Some(DailyRead {
                unmapped_attempts: vec![],
                sources: vec![source],
                read_at: frame.read_at.clone(),
                scope: frame.scope.clone(),
                expected_today: vec![],
                attempts: frame.attempts.clone(),
                unknowns: vec!["daily_partial_page".into()],
                observed: true,
            }),
            sections: None,
            judgments: vec![],
            excluded: vec![],
            stages: vec![],
            prestudy: vec![],
            clinic: vec![],
            first_omitted: None,
        };
        edu_academy_teacher::unit_render::render(&mut data, &mut coverage)
    }
    pub fn session(&self) -> Session {
        let s = &self.scenario;
        let audit = Rc::new(RefCell::new(Vec::new()));
        let backend = SourceBackend::new(
            SyntheticDayPort::new(s.clone()),
            PrestudyPortWithFault {
                inner: SyntheticPrestudyPort::new(s.clone()),
                fail: self.faults.prestudy_fail,
            },
            TextPort {
                inner: SyntheticTextbookPort::new(s.clone()),
                faults: self.faults.clone(),
                audit: audit.clone(),
            },
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
        );
        let clock = SharedFrameClock::new(s.dailytest_frames.clone());
        let daily = DailyTestReader::new(
            DailyPort {
                inner: SyntheticDailyTestPort::new(s.clone(), clock.clone()),
                faults: self.faults.clone(),
                audit: audit.clone(),
            },
            clock,
            TEACHER_ID,
        );
        Session {
            backend,
            daily,
            facts: SeededFacts::from_scenario(s),
            audit,
        }
    }
}

pub fn section(
    student: usize,
    book: usize,
    unit: &str,
    activity: &str,
    retest: usize,
) -> NativeSectionAttempt {
    let identity = student * 1000 + book * 100 + retest;
    NativeSectionAttempt {
        student_key: format!("{}", 1001 + student),
        course_key: format!("{}", 101 + book),
        book_no: format!("{}", 201 + book),
        paper_key: format!("{}", 81000 + identity),
        attempt_key: format!("{}", 82000 + identity),
        title: format!(
            "[{activity}] [가우스] 5-1-{} {unit} 가상주제 - {}",
            book + 1,
            NAMES[student]
        ),
        source_score: 41.25 + retest as f64,
        source_date: None,
        items: vec![NativeSectionItem {
            ordinal: "1".into(),
            label: Some(format!("{}", 61000 + identity)),
            scoring: "X".into(),
        }],
        wrong_note: true,
        paper_search: true,
    }
}

pub struct Session {
    backend: SourceBackend<SyntheticDayPort, PrestudyPortWithFault, TextPort>,
    daily: DailyTestReader<DailyPort>,
    facts: SeededFacts,
    pub audit: Audit,
}
impl Session {
    pub fn call(&mut self, request: &UnitStatusRequest) -> Envelope {
        status(
            &mut self.backend,
            &mut self.daily,
            &self.facts,
            request,
            &ReadClock {
                read_at: NOW.into(),
            },
        )
    }
}

struct TextPort {
    inner: SyntheticTextbookPort,
    faults: Faults,
    audit: Audit,
}
impl TextbookSectionPort for TextPort {
    fn progress(
        &mut self,
        s: &TextbookStudent,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        let result = self.inner.progress(s, n, p);
        event(&self.audit, "bindings", s.student_key(), result.is_ok());
        result
    }
    fn wrong_notes(
        &mut self,
        s: &TextbookStudent,
        b: &BookScope,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        if self.faults.notes_fail || self.faults.timeout_once {
            let timeout = self.faults.timeout_once;
            self.faults.timeout_once = false;
            event(&self.audit, "notes", b.book_no(), false);
            return Err(Error::error(if timeout {
                "timeout"
            } else {
                "source_unavailable"
            }));
        }
        let result = self.inner.wrong_notes(s, b, n, p);
        event(&self.audit, "notes", b.book_no(), result.is_ok());
        result
    }
    fn detail(
        &mut self,
        s: &TextbookStudent,
        a: &str,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        let result = if self.faults.detail_fail.iter().any(|key| key == a) {
            Err(Error::error("source_unavailable"))
        } else {
            self.inner.detail(s, a, n, p)
        };
        event(&self.audit, "detail", a, result.is_ok());
        result
    }
    fn paper_search(
        &mut self,
        s: &TextbookStudent,
        n: usize,
        p: FetchPolicy,
    ) -> Result<TextbookResponse> {
        let result = if self.faults.paper_fail {
            Err(Error::error("source_unavailable"))
        } else {
            self.inner.paper_search(s, n, p)
        };
        event(&self.audit, "papers", s.student_key(), result.is_ok());
        result
    }
}
struct DailyPort {
    inner: SyntheticDailyTestPort,
    faults: Faults,
    audit: Audit,
}
impl Port for DailyPort {
    fn dated_students(&mut self, input: &Js) -> Result<Js> {
        self.inner.dated_students(input)
    }
    fn daily_page(&mut self, s: &Js, selector: &Js) -> Result<Js> {
        let scope = string(selector);
        if self
            .faults
            .daily_fail
            .as_deref()
            .is_some_and(|v| v == scope || v == "all")
        {
            event(&self.audit, "daily_page", &scope, false);
            return Err(Error::error("source_unavailable"));
        }
        let primary: Js = "1".into();
        let mut result = self.inner.daily_page(
            s,
            if self.faults.relative_rows && scope == "11" {
                &primary
            } else {
                selector
            },
        )?;
        result.set("selector", selector.clone());
        if let Some(at) = &self.faults.page_at {
            result.set("observedAt", at.clone().into());
        }
        event(&self.audit, "daily_page", &scope, true);
        Ok(result)
    }
    fn paper_header(&mut self, s: &Js, code: &Js) -> Result<Js> {
        if self.faults.header_fail {
            return Err(Error::error("source_unavailable"));
        }
        let mut result = self.inner.paper_header(s, code)?;
        if let Some(at) = &self.faults.header_at {
            result.set("observedAt", at.clone().into());
        }
        event(&self.audit, "daily_header", "selected", true);
        Ok(result)
    }
}

struct PrestudyPortWithFault {
    inner: SyntheticPrestudyPort,
    fail: bool,
}
impl edu_academy_sources::prestudy::PrestudyPort for PrestudyPortWithFault {
    fn shell(
        &mut self,
        k: edu_academy_sources::prestudy::ListKind,
        n: usize,
    ) -> Result<edu_academy_sources::http::Response> {
        if self.fail {
            return Err(Error::error("source_unavailable"));
        }
        self.inner.shell(k, n)
    }
    fn search(
        &mut self,
        k: edu_academy_sources::prestudy::ListKind,
        f: &edu_academy_sources::prestudy::PrestudySearchForm,
        s: &str,
        n: usize,
    ) -> Result<edu_academy_sources::http::Response> {
        self.inner.search(k, f, s, n)
    }
    fn summary(&mut self, n: usize) -> Result<edu_academy_sources::http::Response> {
        self.inner.summary(n)
    }
    fn detail(&mut self, k: &str, n: usize) -> Result<edu_academy_sources::http::Response> {
        self.inner.detail(k, n)
    }
    fn report(&mut self, k: &str, n: usize) -> Result<edu_academy_sources::http::Response> {
        self.inner.report(k, n)
    }
}
