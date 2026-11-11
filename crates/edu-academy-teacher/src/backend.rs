//! Composition of the three source ports behind the existing source APIs:
//! name resolve, record read, prestudy and section reads. Generic over the
//! port traits, so the synthetic ports here and any real ports later serve
//! `lesson::prepare` unchanged.
use crate::{
    envelope::{Ask, AskField, AskOption, Stop, StopCode},
    render::Verdict,
};
use chrono::NaiveDate;
use edu_academy_core::js::Js;
use edu_academy_sources::{
    data::{obj, string},
    native::{Backend, SourcePort, resolve_name},
    prestudy::{PrestudyPort, PrestudySource},
    textbook_index::{
        BookScope, GradeOutcome, TextbookIndexSource, TextbookSectionPort, TextbookStudent,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudentEntry {
    pub name: String,
    pub key: String,
    pub class_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NameHint {
    class_name: Option<String>,
    book_label: Option<String>,
}

enum NameSearch {
    Resolved(ResolvedStudent),
    Ambiguous(Vec<String>),
    NotFound,
}

/// The disambiguated form `이름 (반 이름)` or `이름 (반 이름, 교재)`.
fn parse_disambiguated(name: &str) -> Option<(String, NameHint)> {
    let pos = name.rfind(" (")?;
    if !name.ends_with(')') {
        return None;
    }
    let base = &name[..pos];
    let inline = &name[pos + 2..name.len() - 1];
    if base.is_empty() || inline.is_empty() {
        return None;
    }
    let (class_name, book_label) = match inline.split_once(", ") {
        Some((class_name, book_label)) => (
            class_name.trim().to_string(),
            Some(book_label.trim().to_string()),
        ),
        None => (inline.trim().to_string(), None),
    };
    if class_name.is_empty() {
        return None;
    }
    Some((
        base.to_string(),
        NameHint {
            class_name: Some(class_name),
            book_label,
        },
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedStudent {
    pub name: String,
    pub key: String,
    pub class_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayRecord {
    pub progress: String,
    pub homework: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SectionCheck {
    pub scope: String,
    pub verdict: Verdict,
    pub correct: Option<usize>,
    pub total: Option<usize>,
}

#[derive(Debug)]
pub enum BackendError {
    Stop(Stop),
    Ask(Ask),
}

impl BackendError {
    pub fn source_unavailable() -> Self {
        Self::Stop(Stop::new(
            StopCode::SourceUnavailable,
            "원천을 읽지 못했습니다.",
        ))
    }
}

pub enum BookResolution {
    Found(BookScope),
    NoBinding { bound_labels: Vec<String> },
}

/// Teacher identity and clock for one backend, given by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendIdentity {
    pub teacher_id: String,
    pub teacher_label: String,
    pub teacher_key: String,
    pub read_at: String,
}

pub struct SourceBackend<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort> {
    pub day: Backend<D>,
    pub prestudy: PrestudySource<P>,
    pub sections: TextbookIndexSource<T>,
    directory: Vec<StudentEntry>,
    book_labels_by_key: std::collections::BTreeMap<String, Vec<String>>,
    teacher_id: String,
    teacher_label: String,
    read_at: String,
    source_reads: usize,
    attempted: usize,
    succeeded: usize,
    read_seq: usize,
}

impl<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort> SourceBackend<D, P, T> {
    /// The teacher identity and the clock value (`read_at`) are parameters,
    /// never constants of a backend kind.
    pub fn new(
        day_port: D,
        prestudy_port: P,
        textbook_port: T,
        directory: Vec<StudentEntry>,
        identity: BackendIdentity,
    ) -> Self {
        let teacher_id = identity.teacher_id;
        let teacher_label = identity.teacher_label;
        let read_at = identity.read_at;
        let admission_date = read_at.get(..10).unwrap_or("2026-11-11").to_string();
        let admission = obj(&[
            ("teacherId", teacher_id.clone().into()),
            ("teacherLabel", teacher_label.clone().into()),
            ("date", admission_date.into()),
            ("sourceBundleSha256", "synthetic-bundle".into()),
            (
                "students",
                Js::Arr(
                    directory
                        .iter()
                        .map(|s| {
                            obj(&[
                                ("studentId", s.key.clone().into()),
                                ("displayName", s.name.clone().into()),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]);
        Self {
            day: Backend::new(day_port, admission),
            prestudy: PrestudySource::new(
                prestudy_port,
                teacher_label.clone(),
                identity.teacher_key,
            ),
            sections: TextbookIndexSource::new(textbook_port, teacher_label.clone()),
            directory,
            book_labels_by_key: std::collections::BTreeMap::new(),
            teacher_id,
            teacher_label,
            read_at,
            source_reads: 0,
            attempted: 0,
            succeeded: 0,
            read_seq: 0,
        }
    }

    pub fn read_at(&self) -> &str {
        &self.read_at
    }
    /// Logical source reads: one per record probe, prestudy read, bindings
    /// read, section index read or section grade. Counted for the tests.
    pub fn source_reads(&self) -> usize {
        self.source_reads
    }
    pub fn attempted_reads(&self) -> usize {
        self.attempted
    }
    pub fn succeeded_reads(&self) -> usize {
        self.succeeded
    }
    /// Snapshot of the lesson-data read counters, for a per-call difference.
    /// Name resolution is not a lesson-data read and is never counted here.
    pub fn read_counts(&self) -> (usize, usize) {
        (self.attempted, self.succeeded)
    }
    pub fn prestudy_port_calls(&self) -> usize {
        self.prestudy.requests()
    }
    pub fn textbook_port_calls(&self) -> usize {
        self.sections.requests()
    }

    fn entry_by_key(&self, key: &str) -> Option<&StudentEntry> {
        self.directory.iter().find(|s| s.key == key)
    }

    pub(crate) fn set_book_labels_by_key(
        &mut self,
        book_labels_by_key: std::collections::BTreeMap<String, Vec<String>>,
    ) {
        self.book_labels_by_key = book_labels_by_key;
    }

    fn book_labels_for(&self, key: &str) -> &[String] {
        self.book_labels_by_key.get(key).map_or(&[], Vec::as_slice)
    }

    fn book_label_for(&self, key: &str) -> &str {
        self.book_labels_for(key).first().map_or("", String::as_str)
    }

    fn student_not_found() -> BackendError {
        BackendError::Stop(Stop::new(
            StopCode::StudentNotFound,
            "학생을 찾지 못했습니다.",
        ))
    }

    fn search_name(&mut self, query: &str) -> Result<NameSearch, BackendError> {
        let name_js: Js = query.into();
        let rows = match self.day.search_students_by_name(&name_js, None, None, 0) {
            Ok(rows) => rows,
            Err(edu_academy_sources::native::StageError::StudentNameNotAdmitted) => {
                return Ok(NameSearch::NotFound);
            }
            Err(_) => return Err(BackendError::source_unavailable()),
        };
        let resolved = resolve_name(&name_js, &rows);
        let value = resolved.get("value");
        match string(value.get("status")).as_str() {
            "resolved" => {
                let key = string(value.get("studentKey"));
                let class_name = self.entry_by_key(&key).map(|s| s.class_name.clone());
                Ok(NameSearch::Resolved(ResolvedStudent {
                    name: query.to_string(),
                    key,
                    class_name,
                }))
            }
            _ if string(value.get("reasonCode")) == "student_name_ambiguous" => {
                let mut keys: Vec<String> = Vec::new();
                for row in rows.array() {
                    let key = string(row.get("studentKey"));
                    if !keys.contains(&key) {
                        keys.push(key);
                    }
                }
                Ok(NameSearch::Ambiguous(keys))
            }
            _ => Ok(NameSearch::NotFound),
        }
    }

    fn hint_matches(&self, student: &ResolvedStudent, hint: &NameHint) -> bool {
        if let Some(class_name) = &hint.class_name
            && student.class_name.as_deref() != Some(class_name.as_str())
        {
            return false;
        }
        if let Some(book_label) = &hint.book_label {
            let matches_book = self
                .book_labels_for(&student.key)
                .iter()
                .any(|label| label == book_label);
            if !matches_book {
                return false;
            }
        }
        true
    }

    fn disambiguate(
        &self,
        base: &str,
        keys: Vec<String>,
        hint: &NameHint,
    ) -> Result<ResolvedStudent, BackendError> {
        let mut candidates: Vec<(String, String, String)> = keys
            .iter()
            .map(|key| {
                let entry = self.entry_by_key(key);
                (
                    key.clone(),
                    entry.map_or_else(String::new, |entry| entry.class_name.clone()),
                    self.book_label_for(key).to_string(),
                )
            })
            .collect();
        if hint.class_name.is_some() || hint.book_label.is_some() {
            let matched: Vec<(String, String, String)> = candidates
                .iter()
                .filter(|(key, class_name, book_label)| {
                    hint.class_name
                        .as_ref()
                        .is_none_or(|expected| class_name == expected)
                        && hint.book_label.as_ref().is_none_or(|expected| {
                            book_label == expected
                                || self
                                    .book_labels_for(key)
                                    .iter()
                                    .any(|label| label == expected)
                        })
                })
                .cloned()
                .collect();
            if matched.len() == 1 {
                let (key, class_name, _) = matched.into_iter().next().expect("one match");
                return Ok(ResolvedStudent {
                    name: base.to_string(),
                    key,
                    class_name: Some(class_name),
                });
            }
            if matched.is_empty() {
                return Err(Self::student_not_found());
            }
            candidates = matched;
        }
        let options: Vec<AskOption> = candidates
            .iter()
            .map(|(_, class_name, book_label)| {
                let same_class = candidates
                    .iter()
                    .filter(|(_, other_class, _)| other_class == class_name)
                    .count()
                    > 1;
                let value = if same_class {
                    format!("{base} ({class_name}, {book_label})")
                } else {
                    format!("{base} ({class_name})")
                };
                AskOption {
                    value: value.clone(),
                    label_ko: value,
                    meaning_ko: "그 반의 학생".into(),
                }
            })
            .collect();
        let mut values: Vec<&str> = options.iter().map(|option| option.value.as_str()).collect();
        values.sort_unstable();
        values.dedup();
        if values.len() != options.len() {
            return Err(BackendError::Stop(Stop::new(
                StopCode::StudentAmbiguousUnresolvable,
                "이름, 반, 교재가 같아 두 학생을 구분할 수 없습니다. 반 명부를 수정해 주세요.",
            )));
        }
        Err(BackendError::Ask(Ask {
            field: AskField::Student,
            question_ko: "어느 학생인가요?".into(),
            options,
        }))
    }

    pub fn resolve_student(
        &mut self,
        name: &str,
        class_hint: Option<&str>,
    ) -> Result<ResolvedStudent, BackendError> {
        // Try the full given string as a name first; only a name that no
        // student has falls back to the `이름 (반 이름)` form.
        let full_hint = NameHint {
            class_name: class_hint.map(str::to_string),
            book_label: None,
        };
        match self.search_name(name)? {
            NameSearch::Resolved(student) => {
                if self.hint_matches(&student, &full_hint) {
                    Ok(student)
                } else {
                    Err(Self::student_not_found())
                }
            }
            NameSearch::Ambiguous(keys) => self.disambiguate(name, keys, &full_hint),
            NameSearch::NotFound => {
                let Some((base, inline_hint)) = parse_disambiguated(name) else {
                    return Err(Self::student_not_found());
                };
                match self.search_name(&base)? {
                    NameSearch::Resolved(student) => {
                        if self.hint_matches(&student, &inline_hint) {
                            Ok(student)
                        } else {
                            Err(Self::student_not_found())
                        }
                    }
                    NameSearch::Ambiguous(keys) => self.disambiguate(&base, keys, &inline_hint),
                    NameSearch::NotFound => Err(Self::student_not_found()),
                }
            }
        }
    }

    pub fn read_record(
        &mut self,
        student_key: &str,
        date: NaiveDate,
    ) -> Result<Option<DayRecord>, BackendError> {
        self.source_reads += 1;
        self.attempted += 1;
        self.read_seq += 1;
        let date_text = date.format("%Y-%m-%d").to_string();
        self.day.admission.set("date", date_text.clone().into());
        let scope = obj(&[
            ("sourceNamespace", "academy.lms.dayrecord".into()),
            ("teacherId", self.teacher_id.clone().into()),
            ("studentId", student_key.into()),
            ("lessonDate", date_text.into()),
        ]);
        let uuid = format!("synthetic-{}", self.read_seq);
        let read_at = self.read_at.clone();
        match self.day.read(&scope, &read_at, &uuid) {
            Ok(result) => {
                self.succeeded += 1;
                let fields = result.get("observation").get("fields");
                let value = |field: &str| -> String {
                    let entry = fields.get(field);
                    if entry.get("state").is("known") {
                        string(entry.get("value"))
                    } else {
                        String::new()
                    }
                };
                Ok(Some(DayRecord {
                    progress: value("progress"),
                    homework: value("homework"),
                }))
            }
            Err(error) => {
                if error.message == "native_dailyrecord_occurrence_unverified" {
                    self.succeeded += 1;
                    Ok(None)
                } else {
                    Err(BackendError::source_unavailable())
                }
            }
        }
    }

    pub fn prestudy_verdict(
        &mut self,
        student_name: &str,
        book_label: &str,
        minor: &str,
    ) -> Verdict {
        self.source_reads += 1;
        self.attempted += 1;
        let read_at = self.read_at.clone();
        let label = self.teacher_label.clone();
        let lists = match self.prestudy.read_lists(&label, student_name, &read_at) {
            Ok(lists) => lists,
            Err(_) => return Verdict::Unknown,
        };
        self.succeeded += 1;
        let in_book = |rows: &[edu_academy_sources::prestudy::PrestudyListRow]| {
            rows.iter().any(|r| {
                r.unit_label.contains(book_label)
                    && r.unit_label.split_whitespace().any(|token| token == minor)
            })
        };
        let any_minor = |rows: &[edu_academy_sources::prestudy::PrestudyListRow]| {
            rows.iter()
                .any(|r| r.unit_label.split_whitespace().any(|token| token == minor))
        };
        if in_book(&lists.waiting) {
            Verdict::Waiting
        } else if in_book(&lists.completed) {
            Verdict::Completed
        } else if any_minor(&lists.waiting) || any_minor(&lists.completed) {
            Verdict::NotInReadRange
        } else {
            Verdict::NotObserved
        }
    }

    pub fn resolve_book(
        &mut self,
        student: &ResolvedStudent,
        book_label: &str,
    ) -> Result<BookResolution, BackendError> {
        self.source_reads += 1;
        self.attempted += 1;
        let textbook_student = TextbookStudent::new(&student.key, &student.name)
            .map_err(|_| BackendError::source_unavailable())?;
        let label = self.teacher_label.clone();
        let bindings = self
            .sections
            .bindings(&label, &textbook_student)
            .map_err(|_| BackendError::source_unavailable())?;
        self.succeeded += 1;
        let bound_labels: Vec<String> = bindings.iter().map(|b| b.book_label.clone()).collect();
        match bindings.iter().find(|b| b.book_label == book_label) {
            Some(binding) => BookScope::new(&binding.course_key, &binding.book_no)
                .map(BookResolution::Found)
                .map_err(|_| BackendError::source_unavailable()),
            None => Ok(BookResolution::NoBinding { bound_labels }),
        }
    }

    /// Both section kinds of one minor from a single index read.
    pub fn section_checks(
        &mut self,
        student: &TextbookStudent,
        scope: &BookScope,
        minor: &str,
        activities: &[&str],
    ) -> Vec<SectionCheck> {
        self.source_reads += 1;
        self.attempted += 1;
        let label = self.teacher_label.clone();
        let index = match self.sections.index(&label, student, scope) {
            Ok(index) => index,
            Err(_) => {
                return activities
                    .iter()
                    .map(|activity| SectionCheck {
                        scope: format!("{minor} {activity}"),
                        verdict: Verdict::Unknown,
                        correct: None,
                        total: None,
                    })
                    .collect();
            }
        };
        self.succeeded += 1;
        let mut checks = Vec::new();
        for activity in activities {
            let scope_label = format!("{minor} {activity}");
            let title = index.rows.iter().find(|r| {
                r.source_title.starts_with(&format!("[{activity}]"))
                    && r.source_title
                        .split_whitespace()
                        .any(|token| token == minor)
            });
            let Some(title) = title else {
                checks.push(SectionCheck {
                    scope: scope_label,
                    verdict: Verdict::NotObserved,
                    correct: None,
                    total: None,
                });
                continue;
            };
            let title = title.source_title.clone();
            self.source_reads += 1;
            self.attempted += 1;
            match self.sections.grade(&label, student, scope, &[title]) {
                Ok(GradeOutcome::Graded(graded)) => {
                    self.succeeded += 1;
                    let record = graded.records.first();
                    checks.push(SectionCheck {
                        scope: scope_label,
                        verdict: Verdict::Found,
                        correct: record.map(|r| r.correct_count()),
                        total: record.map(|r| r.item_count()),
                    });
                }
                Ok(GradeOutcome::NotObservedInEither(_)) => {
                    self.succeeded += 1;
                    checks.push(SectionCheck {
                        scope: scope_label,
                        verdict: Verdict::NotObservedInEitherSource,
                        correct: None,
                        total: None,
                    });
                }
                Err(_) => checks.push(SectionCheck {
                    scope: scope_label,
                    verdict: Verdict::Unknown,
                    correct: None,
                    total: None,
                }),
            }
        }
        checks
    }
}
