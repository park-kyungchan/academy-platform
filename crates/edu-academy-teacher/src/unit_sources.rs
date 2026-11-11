//! Real source-parser composition, with one timeout retry at the raw port boundary.
use crate::{
    backend::{BackendError, ResolvedStudent, SourceBackend},
    unit_data::*,
    unit_fixture::SharedFrameClock,
};
use edu_academy_core::js::Js;
use edu_academy_sources::{
    data::{Error, Result, obj, string},
    history::{History, Port, encode_code},
    native::SourcePort,
    prestudy::PrestudyPort,
    textbook_index::{
        BookScope, GradeOutcome, MAX_SOURCE_TITLES, PaperSearchStatus, TextbookSectionPort,
        TextbookStudent,
    },
};

pub fn retry_timeout<T>(mut read: impl FnMut() -> Result<T>) -> Result<T> {
    match read() {
        Err(e)
            if matches!(
                e.message.as_str(),
                "timeout"
                    | "source_timeout"
                    | "request_timeout"
                    | "native_transport_timeout"
                    | "source_response_timeout"
            ) =>
        {
            read()
        }
        result => result,
    }
}
#[derive(Default)]
struct Captured {
    dated: Option<Js>,
    pages: Vec<(Js, Js)>,
}
struct RetryingPort<'a, H> {
    port: &'a mut H,
    requests: &'a mut usize,
    captured: &'a mut Captured,
}
impl<H: Port> Port for RetryingPort<'_, H> {
    fn dated_students(&mut self, input: &Js) -> Result<Js> {
        let rows = retry_timeout(|| {
            *self.requests += 1;
            self.port.dated_students(input)
        })?;
        self.captured.dated = Some(rows.clone());
        Ok(rows)
    }
    fn daily_page(&mut self, student: &Js, selector: &Js) -> Result<Js> {
        let raw = retry_timeout(|| {
            *self.requests += 1;
            self.port.daily_page(student, selector)
        })?;
        self.captured.pages.push((selector.clone(), raw.clone()));
        Ok(raw)
    }
    fn paper_header(&mut self, student: &Js, code: &Js) -> Result<Js> {
        retry_timeout(|| {
            *self.requests += 1;
            self.port.paper_header(student, code)
        })
    }
    fn paper_admission(&self) -> Option<&edu_academy_sources::owner_admission::TransportAdmission> {
        self.port.paper_admission()
    }
    fn qualified_paper_header(
        &mut self,
        student: &Js,
        code: &Js,
    ) -> Option<Result<edu_academy_sources::history::QualifiedHeader>> {
        *self.requests += 1;
        let first = self.port.qualified_paper_header(student, code)?;
        match first {
            Err(e)
                if matches!(
                    e.message.as_str(),
                    "timeout"
                        | "source_timeout"
                        | "request_timeout"
                        | "native_transport_timeout"
                        | "source_response_timeout"
                ) =>
            {
                *self.requests += 1;
                self.port.qualified_paper_header(student, code)
            }
            other => Some(other),
        }
    }
}
fn history<'a, H: Port>(
    port: &'a mut H,
    teacher: &str,
    student: &ResolvedStudent,
    frame: &DailyFrame,
    requests: &'a mut usize,
    captured: &'a mut Captured,
) -> Result<History<RetryingPort<'a, H>>> {
    History::new(
        RetryingPort {
            port,
            requests,
            captured,
        },
        obj(&[("teacherId", teacher.into())]),
        obj(&[
            ("schemaVersion", "academy.metadata-admission/1".into()),
            ("sourceNamespace", "academy.lms.metadata".into()),
            ("teacherId", teacher.into()),
            ("allowedFrom", frame.scope.from.clone().into()),
            ("allowedThrough", frame.scope.to.clone().into()),
            ("studentScope", "configured".into()),
            ("relativeSelectors", Js::Arr(vec!["1".into(), "11".into()])),
            ("operations", Js::Arr(vec!["daily_history".into()])),
        ]),
        vec![obj(&[
            ("studentId", student.key.clone().into()),
            ("displayName", student.name.clone().into()),
        ])],
        vec![student.name.clone().into()],
        vec![student.key.clone().into()],
    )
}
pub struct DailyTestReader<H: Port> {
    port: H,
    pub clock: SharedFrameClock,
    teacher_id: String,
    requests: usize,
}
impl<H: Port> DailyTestReader<H> {
    pub fn new(port: H, clock: SharedFrameClock, teacher_id: &str) -> Self {
        Self {
            port,
            clock,
            teacher_id: teacher_id.into(),
            requests: 0,
        }
    }
    pub fn source_requests(&self) -> usize {
        self.requests
    }
    pub fn read(&mut self, student: &ResolvedStudent) -> Result<DailyRead> {
        let frame = self
            .clock
            .begin_read(&student.key)
            .ok_or_else(|| Error::error("daily_frame_unavailable"))?;
        let mut out = DailyRead {
            sources: Vec::new(),
            read_at: frame.read_at.clone(),
            scope: frame.scope.clone(),
            expected_today: frame.expected_today.clone(),
            attempts: Vec::new(),
            unknowns: Vec::new(),
            observed: false,
        };
        let mut captured = Captured::default();
        let mut reader = history(
            &mut self.port,
            &self.teacher_id,
            student,
            &frame,
            &mut self.requests,
            &mut captured,
        )?;
        let metadata = reader.metadata(&obj(&[
            ("displayName", student.name.clone().into()),
            ("lessonDate", frame.scope.to.clone().into()),
            (
                "queries",
                Js::Arr(vec![obj(&[
                    ("kind", "daily_history".into()),
                    ("periods", Js::Arr(vec!["11".into()])),
                ])]),
            ),
        ]));
        drop(reader);
        // On partial failure, keep only pages accepted by the real project_page parser,
        // after the dated owner/student occurrence has passed the same exact contract.
        let dated = captured
            .dated
            .as_ref()
            .ok_or_else(|| Error::error("daily_occurrence_unavailable"))?;
        if dated.array().len() != 1
            || string(dated.array()[0].get("teacherKey")) != self.teacher_id
            || string(dated.array()[0].get("studentKey")) != student.key
            || string(dated.array()[0].get("studentName")) != student.name
            || string(dated.array()[0].get("date")) != frame.scope.to
        {
            return Err(Error::error("daily_occurrence_scope_conflict"));
        }
        if metadata.is_err() {
            out.unknowns.push("daily_partial_page".into());
        }
        let identity = obj(&[
            ("studentKey", student.key.clone().into()),
            ("studentName", student.name.clone().into()),
        ]);
        let mut rows = Vec::new();
        for (selector, raw) in &captured.pages {
            match edu_academy_sources::history::project_page(raw, &identity, selector) {
                Ok(page) => {
                    out.observed = true;
                    let at = string(page.get("observedAt"));
                    if selector.is("1") {
                        out.read_at = at.clone();
                    }
                    out.sources.push(crate::envelope::SourceRead {
                        name: if selector.is("1") {
                            "dailytest"
                        } else {
                            "dailytest_relative_page"
                        }
                        .into(),
                        read_at: at,
                        scope: if selector.is("1") {format!("{}..{} / selector 1 / page 1 / {} {} / not complete",frame.scope.from,frame.scope.to,frame.scope.selector_label,frame.scope.page_label)}else{format!("selector {} / page 1 / relative period bounds unavailable / not complete",string(selector))},
                    });
                    rows.extend(page.get("rows").array().iter().cloned());
                }
                Err(_) => out.unknowns.push("daily_partial_page".into()),
            }
        }
        if !out.observed {
            return Err(Error::error("daily_page_unavailable"));
        }
        let mut seen = std::collections::HashSet::new();
        for row in rows {
            let key = string(row.get("paperKey"));
            let attempt = string(row.get("attemptKey"));
            if !seen.insert((key.clone(), attempt.clone())) {
                continue;
            }
            let mut capture = Captured::default();
            let mut headers = history(
                &mut self.port,
                &self.teacher_id,
                student,
                &frame,
                &mut self.requests,
                &mut capture,
            )?;
            let code = encode_code(&key.as_str().into())?;
            match headers.headers(&obj(&[
                ("studentId", student.key.clone().into()),
                ("paperCodes", Js::Arr(vec![code])),
            ])) {
                Ok(header) => {
                    let at =
                        string(header.get("observation").get("rows").array()[0].get("observedAt"));
                    out.sources.push(crate::envelope::SourceRead {
                        name: "dailytest_header".into(),
                        read_at: at,
                        scope: key.clone(),
                    });
                    // Scoped seeded mappings do not establish the existence of an attempt.
                    if let Some(mapping) = frame.headers.iter().find(|h| h.paper_key == key) {
                        let mut a = frame
                            .headers
                            .iter()
                            .find(|h| h.paper_key == key && h.attempt_key == attempt)
                            .unwrap_or(mapping)
                            .clone();
                        if a.attempt_key != attempt {
                            a.occurred_at = None;
                        }
                        a.attempt_key = attempt;
                        a.score = match row.get("sourceScore") {
                            Js::Num(n) => Some(*n),
                            _ => None,
                        };
                        if a.edition_mapping.is_none() || a.unit_mapping.is_none() {
                            out.unknowns.push("daily_mapping_unavailable".into());
                        }
                        out.attempts.push(a);
                    } else {
                        out.unknowns.push("daily_mapping_unavailable".into());
                    }
                }
                Err(_) => out.unknowns.push("daily_header_unavailable".into()),
            }
        }
        Ok(out)
    }
}

pub fn sections<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort>(
    backend: &mut SourceBackend<D, P, T>,
    student: &TextbookStudent,
    scope: &BookScope,
    edition: &str,
    leaves: &[Leaf],
    activity: Option<&str>,
) -> std::result::Result<SectionRead, BackendError> {
    let mut index = backend.observed_sections(student, scope)?;
    index
        .rows
        .retain(|row| title_in_edition(&row.source_title, edition));
    let mut out = SectionRead {
        index_observed: true,
        paper_observed: matches!(index.paper_search, PaperSearchStatus::Observed { .. }),
        ..Default::default()
    };
    if !out.paper_observed {
        out.unknowns
            .push("section_paper_search_unavailable_or_unqualified".into());
    }
    let mut selected = Vec::new();
    for leaf in leaves {
        for a in &leaf.minor.activities {
            if activity.is_some_and(|filter| filter != a) {
                continue;
            }
            for row in &index.rows {
                if title_matches(&row.source_title, &leaf.minor.code, a)
                    && !selected.contains(&row.source_title)
                {
                    selected.push(row.source_title.clone());
                }
            }
        }
    }
    for batch in selected.chunks(1.min(MAX_SOURCE_TITLES)) {
        match backend.selected_sections(student, scope, batch) {
            Ok(GradeOutcome::Graded(graded)) => {
                if !graded.paper_search_observed {
                    out.paper_observed = false;
                    out.unknowns.push("section_paper_search_unavailable".into());
                }
                for r in graded.records {
                    let Some((leaf, activity)) = leaves.iter().find_map(|leaf| {
                        leaf.minor
                            .activities
                            .iter()
                            .find(|a| title_matches(&r.source_title, &leaf.minor.code, a))
                            .map(|a| (leaf, a))
                    }) else {
                        continue;
                    };
                    out.records.push(Section {
                        student_key: r.student_key,
                        student_name: r.student_name,
                        course_key: r.course_key,
                        book_no: r.book_no,
                        unit: leaf.minor.code.clone(),
                        activity: activity.clone(),
                        source_title: r.source_title,
                        paper_key: r.paper_key,
                        attempt_key: r.attempt_key,
                        score: r.score,
                        source_date: r.source_date.literal,
                        date_precision: r.source_date.precision.into(),
                        items: r
                            .items
                            .into_iter()
                            .map(|i| Item {
                                outer_lecture_key: i.outer_lecture_key,
                                ordinal: i.ordinal,
                                label: i.lecture_key,
                                scoring: i.scoring.as_str().into(),
                            })
                            .collect(),
                    });
                }
            }
            _ => {
                for title in batch {
                    let scope = leaves
                        .iter()
                        .find_map(|leaf| {
                            leaf.minor
                                .activities
                                .iter()
                                .find(|a| title_matches(title, &leaf.minor.code, a))
                                .map(|a| format!("{} {}", leaf.minor.code, a))
                        })
                        .unwrap_or_else(|| edition.to_string());
                    out.unknowns
                        .push(format!("section_grade_unavailable:{scope}"));
                }
            }
        }
    }
    // Use an observed title pattern to query the absent title through grade's typed path.
    for leaf in leaves {
        if index.rows.iter().any(|r| {
            leaf.minor
                .activities
                .iter()
                .any(|a| title_matches(&r.source_title, &leaf.minor.code, a))
        }) {
            continue;
        }
        let template = index.rows.iter().find_map(|r| {
            leaves_template(&r.source_title)
                .map(|code| r.source_title.replacen(&code, &leaf.minor.code, 1))
        });
        if let Some(title) = template
            && matches!(
                backend.selected_sections(student, scope, &[title]),
                Ok(GradeOutcome::NotObservedInEither(_))
            )
        {
            out.absent_units.push(leaf.minor.code.clone());
        }
    }
    out.records.sort_by_key(|r| {
        leaves.iter().position(|l| l.minor.code == r.unit).map(|i| {
            (
                i,
                leaves[i]
                    .minor
                    .activities
                    .iter()
                    .position(|a| a == &r.activity)
                    .unwrap_or(usize::MAX),
            )
        })
    });
    Ok(out)
}
fn title_in_edition(title: &str, edition: &str) -> bool {
    let normalized = title.replace(['[', ']'], "");
    let tokens: Vec<_> = normalized.split_whitespace().collect();
    let label: Vec<_> = edition.split_whitespace().collect();
    !label.is_empty() && tokens.windows(label.len()).any(|window| window == label)
}
pub fn title_matches(title: &str, code: &str, activity: &str) -> bool {
    title.starts_with(&format!("[{activity}]"))
        && title.split_whitespace().any(|token| token == code)
}
fn leaves_template(title: &str) -> Option<String> {
    // The edition token occurs before the minor token. Do not confuse them.
    title
        .split_whitespace()
        .skip(3)
        .find(|token| {
            token.split('-').count() >= 2
                && token
                    .split('-')
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(str::to_string)
}

/// Timeout retry belongs below the source parser, before errors are qualified or latched.
pub struct RetryingTextbookPort<T>(pub T);
impl<T: TextbookSectionPort> TextbookSectionPort for RetryingTextbookPort<T> {
    fn progress(
        &mut self,
        s: &TextbookStudent,
        max: usize,
        p: edu_academy_sources::textbook::FetchPolicy,
    ) -> Result<edu_academy_sources::textbook::TextbookResponse> {
        retry_timeout(|| self.0.progress(s, max, p))
    }
    fn wrong_notes(
        &mut self,
        s: &TextbookStudent,
        b: &BookScope,
        max: usize,
        p: edu_academy_sources::textbook::FetchPolicy,
    ) -> Result<edu_academy_sources::textbook::TextbookResponse> {
        retry_timeout(|| self.0.wrong_notes(s, b, max, p))
    }
    fn detail(
        &mut self,
        s: &TextbookStudent,
        a: &str,
        max: usize,
        p: edu_academy_sources::textbook::FetchPolicy,
    ) -> Result<edu_academy_sources::textbook::TextbookResponse> {
        retry_timeout(|| self.0.detail(s, a, max, p))
    }
    fn paper_search(
        &mut self,
        s: &TextbookStudent,
        max: usize,
        p: edu_academy_sources::textbook::FetchPolicy,
    ) -> Result<edu_academy_sources::textbook::TextbookResponse> {
        retry_timeout(|| self.0.paper_search(s, max, p))
    }
}
