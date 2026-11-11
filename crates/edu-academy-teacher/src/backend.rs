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
    pub fn prestudy_port_calls(&self) -> usize {
        self.prestudy.requests()
    }
    pub fn textbook_port_calls(&self) -> usize {
        self.sections.requests()
    }

    fn entry_by_key(&self, key: &str) -> Option<&StudentEntry> {
        self.directory.iter().find(|s| s.key == key)
    }

    pub fn resolve_student(
        &mut self,
        name: &str,
        class_hint: Option<&str>,
    ) -> Result<ResolvedStudent, BackendError> {
        self.attempted += 1;
        // The disambiguated form `이름 (반 이름)` names the class inline.
        let (base, inline_class) = match name.rfind(" (") {
            Some(pos) if name.ends_with(')') => {
                (&name[..pos], Some(&name[pos + 2..name.len() - 1]))
            }
            _ => (name, None),
        };
        let hint = inline_class.or(class_hint);
        let name_js: Js = base.into();
        let rows = self
            .day
            .search_students_by_name(&name_js, None, None, 0)
            .map_err(|error| {
                if matches!(
                    error,
                    edu_academy_sources::native::StageError::StudentNameNotAdmitted
                ) {
                    BackendError::Stop(Stop::new(
                        StopCode::StudentNotFound,
                        "학생을 찾지 못했습니다.",
                    ))
                } else {
                    BackendError::source_unavailable()
                }
            })?;
        let resolved = resolve_name(&name_js, &rows);
        let value = resolved.get("value");
        match string(value.get("status")).as_str() {
            "resolved" => {
                let key = string(value.get("studentKey"));
                let class_name = self.entry_by_key(&key).map(|s| s.class_name.clone());
                if let Some(hint) = hint
                    && class_name.as_deref() != Some(hint)
                {
                    return Err(BackendError::Stop(Stop::new(
                        StopCode::StudentNotFound,
                        "학생을 찾지 못했습니다.",
                    )));
                }
                self.succeeded += 1;
                Ok(ResolvedStudent {
                    name: base.to_string(),
                    key,
                    class_name,
                })
            }
            _ => {
                let reason = string(value.get("reasonCode"));
                if reason == "student_name_ambiguous" {
                    let mut keys: Vec<String> = Vec::new();
                    for row in rows.array() {
                        let key = string(row.get("studentKey"));
                        if !keys.contains(&key) {
                            keys.push(key);
                        }
                    }
                    if let Some(hint) = hint {
                        let matched: Vec<&StudentEntry> = self
                            .directory
                            .iter()
                            .filter(|s| {
                                s.name == base && s.class_name == hint && keys.contains(&s.key)
                            })
                            .collect();
                        if matched.len() == 1 {
                            self.succeeded += 1;
                            return Ok(ResolvedStudent {
                                name: base.to_string(),
                                key: matched[0].key.clone(),
                                class_name: Some(matched[0].class_name.clone()),
                            });
                        }
                    }
                    let options = keys
                        .iter()
                        .map(|key| {
                            let class =
                                self.entry_by_key(key).map_or("", |s| s.class_name.as_str());
                            let value = format!("{base} ({class})");
                            AskOption {
                                value: value.clone(),
                                label_ko: value,
                                meaning_ko: "그 반의 학생".into(),
                            }
                        })
                        .collect();
                    Err(BackendError::Ask(Ask {
                        field: AskField::Student,
                        question_ko: "어느 학생인가요?".into(),
                        options,
                    }))
                } else {
                    Err(BackendError::Stop(Stop::new(
                        StopCode::StudentNotFound,
                        "학생을 찾지 못했습니다.",
                    )))
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
