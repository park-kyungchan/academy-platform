//! Composition of the three source ports behind the existing source APIs:
//! name resolve, record read, prestudy and section reads. Generic over the
//! port traits, so the synthetic ports here and any real ports later serve
//! `lesson::prepare` unchanged.
use crate::{
    display_labels as l,
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

enum NameSearch {
    Resolved(ResolvedStudent),
    Ambiguous(Vec<String>),
    NotFound,
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
        Self::Stop(Stop::new(StopCode::SourceUnavailable, l::SOURCE_FAILURE))
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

    fn student_not_found() -> BackendError {
        BackendError::Stop(Stop::new(StopCode::StudentNotFound, l::STUDENT_MISSING))
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

    /// Book labels of one candidate, read from the bindings port at the
    /// moment of use. One logical lesson-data read, counted like any other.
    fn read_book_labels(&mut self, student: &ResolvedStudent) -> Result<Vec<String>, BackendError> {
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
        Ok(bindings.iter().map(|b| b.book_label.clone()).collect())
    }

    fn candidates_from_keys(&self, base: &str, keys: &[String]) -> Vec<ResolvedStudent> {
        keys.iter()
            .map(|key| ResolvedStudent {
                name: base.to_string(),
                key: key.clone(),
                class_name: self.entry_by_key(key).map(|s| s.class_name.clone()),
            })
            .collect()
    }

    /// The one function that builds the option strings for the candidates
    /// of `base`. Answer resolution compares a given string for equality
    /// with these strings; it never parses the string. When two candidates
    /// share a class, each such candidate's book label is read from the
    /// bindings port (one read per such candidate).
    fn candidate_options(
        &mut self,
        base: &str,
        candidates: &[ResolvedStudent],
    ) -> Result<Vec<(ResolvedStudent, AskOption)>, BackendError> {
        let mut options = Vec::new();
        for candidate in candidates {
            let class_name = candidate.class_name.as_deref().unwrap_or("");
            let same_class = candidates
                .iter()
                .filter(|other| other.class_name.as_deref().unwrap_or("") == class_name)
                .count()
                > 1;
            let value = if same_class {
                let mut labels = self.read_book_labels(candidate)?;
                labels.sort();
                let book_label = labels.first().map_or("", String::as_str);
                format!("{base} ({class_name}, {book_label})")
            } else {
                format!("{base} ({class_name})")
            };
            options.push((
                candidate.clone(),
                AskOption {
                    value: value.clone(),
                    label_ko: value,
                    meaning_ko: l::STUDENT_MEANING.into(),
                },
            ));
        }
        Ok(options)
    }

    fn ambiguous_unresolvable() -> BackendError {
        BackendError::Stop(Stop::new(
            StopCode::StudentAmbiguousUnresolvable,
            l::STUDENT_AMBIGUOUS,
        ))
    }

    fn ask_for_options(options: &[(ResolvedStudent, AskOption)]) -> BackendError {
        let mut values: Vec<&str> = options.iter().map(|(_, o)| o.value.as_str()).collect();
        values.sort_unstable();
        values.dedup();
        if values.len() != options.len() {
            return Self::ambiguous_unresolvable();
        }
        BackendError::Ask(Ask {
            field: AskField::Student,
            question_ko: l::STUDENT_QUESTION.into(),
            options: options.iter().map(|(_, o)| o.clone()).collect(),
        })
    }

    fn ask_class(candidates: &[ResolvedStudent]) -> BackendError {
        let mut options: Vec<AskOption> = Vec::new();
        for candidate in candidates {
            if let Some(class_name) = &candidate.class_name
                && !class_name.trim().is_empty()
                && !options.iter().any(|option| option.value == *class_name)
            {
                options.push(AskOption {
                    value: class_name.clone(),
                    label_ko: class_name.clone(),
                    meaning_ko: l::ACTUAL_CLASS_MEANING.into(),
                });
            }
        }
        if options.is_empty() {
            return Self::student_not_found();
        }
        BackendError::Ask(Ask {
            field: AskField::ClassName,
            question_ko: l::ACTUAL_CLASS_QUESTION.into(),
            options,
        })
    }

    fn disambiguate(
        &mut self,
        base: &str,
        keys: Vec<String>,
        class_hint: Option<&str>,
    ) -> Result<ResolvedStudent, BackendError> {
        let candidates = self.candidates_from_keys(base, &keys);
        let filtered: Vec<ResolvedStudent> = match class_hint {
            Some(expected) => candidates
                .iter()
                .filter(|c| c.class_name.as_deref() == Some(expected))
                .cloned()
                .collect(),
            None => candidates.clone(),
        };
        match filtered.as_slice() {
            [] => Err(Self::ask_class(&candidates)),
            [only] if class_hint.is_some() => Ok(only.clone()),
            _ => {
                let options = self.candidate_options(base, &filtered)?;
                Err(Self::ask_for_options(&options))
            }
        }
    }

    pub fn resolve_student(
        &mut self,
        name: &str,
        class_hint: Option<&str>,
    ) -> Result<ResolvedStudent, BackendError> {
        // Try the full given string as a name first.
        match self.search_name(name)? {
            NameSearch::Resolved(student) => {
                return match class_hint {
                    Some(expected) if student.class_name.as_deref() != Some(expected) => {
                        Err(Self::ask_class(std::slice::from_ref(&student)))
                    }
                    _ => Ok(student),
                };
            }
            NameSearch::Ambiguous(keys) => return self.disambiguate(name, keys, class_hint),
            NameSearch::NotFound => {}
        }
        // The given string may be an answer to a "which student" question:
        // compare it for equality with the option strings this server
        // generates for the candidates of each known name.
        let mut bases: Vec<String> = Vec::new();
        for entry in &self.directory {
            if name
                .strip_prefix(&entry.name)
                .is_some_and(|tail| tail.starts_with(" ("))
                && !bases.contains(&entry.name)
            {
                bases.push(entry.name.clone());
            }
        }
        let mut matches = Vec::new();
        for base in bases {
            let candidates = match self.search_name(&base)? {
                NameSearch::Resolved(student) => vec![student],
                NameSearch::Ambiguous(keys) => self.candidates_from_keys(&base, &keys),
                NameSearch::NotFound => continue,
            };
            let options = self.candidate_options(&base, &candidates)?;
            matches.extend(
                options
                    .into_iter()
                    .filter(|(_, option)| option.value == name)
                    .map(|(student, _)| student),
            );
        }
        match matches.as_slice() {
            [] => Err(Self::student_not_found()),
            [only] => match class_hint {
                Some(expected) if only.class_name.as_deref() != Some(expected) => {
                    Err(Self::ask_class(std::slice::from_ref(only)))
                }
                _ => Ok(only.clone()),
            },
            _ => Err(Self::ambiguous_unresolvable()),
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

    pub fn prestudy_observation(
        &mut self,
        student_name: &str,
        book_label: &str,
        minor: &str,
    ) -> (Verdict, Option<String>) {
        self.source_reads += 1;
        self.attempted += 1;
        let read_at = self.read_at.clone();
        let label = self.teacher_label.clone();
        let lists = match self.prestudy.read_lists(&label, student_name, &read_at) {
            Ok(lists) => lists,
            Err(_) => return (Verdict::Unknown, None),
        };
        self.succeeded += 1;
        let in_book = |rows: &[edu_academy_sources::prestudy::PrestudyListRow]| {
            rows.iter()
                .find(|r| {
                    r.unit_label.contains(book_label)
                        && r.unit_label.split_whitespace().any(|token| token == minor)
                })
                .map(|row| row.date_literal.clone())
        };
        let any_minor = |rows: &[edu_academy_sources::prestudy::PrestudyListRow]| {
            rows.iter()
                .any(|r| r.unit_label.split_whitespace().any(|token| token == minor))
        };
        if let Some(row) = in_book(&lists.waiting) {
            (Verdict::Waiting, Some(row))
        } else if let Some(row) = in_book(&lists.completed) {
            (Verdict::Completed, Some(row))
        } else if any_minor(&lists.waiting) || any_minor(&lists.completed) {
            (Verdict::NotInReadRange, None)
        } else {
            (Verdict::NotObserved, None)
        }
    }

    pub fn prestudy_verdict(&mut self, student: &str, book: &str, minor: &str) -> Verdict {
        self.prestudy_observation(student, book, minor).0
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort};

    #[test]
    fn missing_class_metadata_never_builds_an_empty_ask() {
        let student = ResolvedStudent {
            name: "가상학생A".into(),
            key: "1001".into(),
            class_name: None,
        };
        let result = SourceBackend::<SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort>::ask_class(&[student]);
        let BackendError::Stop(stop) = result else {
            panic!("missing class must stop");
        };
        assert_eq!(stop.code, StopCode::StudentNotFound);
    }
}
