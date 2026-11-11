//! Composition of the three synthetic ports behind the existing source
//! APIs (plan s4): name resolve, record read, prestudy and section reads.
use crate::{
    envelope::{Ask, AskOption, Stop},
    render::Verdict,
    synthetic::{
        NOW, Scenario, SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort, TEACHER_ID,
        TEACHER_LABEL,
    },
};
use chrono::NaiveDate;
use edu_academy_core::js::Js;
use edu_academy_sources::{
    data::{obj, string},
    native::{Backend, resolve_name},
    prestudy::PrestudySource,
    textbook_index::{BookScope, GradeOutcome, TextbookIndexSource, TextbookStudent},
};

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
        Self::Stop(Stop::new("source_unavailable", "원천을 읽지 못했습니다."))
    }
}

pub struct SourceBackend {
    pub day: Backend<SyntheticDayPort>,
    pub prestudy: PrestudySource<SyntheticPrestudyPort>,
    pub sections: TextbookIndexSource<SyntheticTextbookPort>,
    scenario: Scenario,
    source_reads: usize,
    read_seq: usize,
}

impl SourceBackend {
    pub fn new(scenario: Scenario) -> Self {
        let admission = obj(&[
            ("teacherId", TEACHER_ID.into()),
            ("teacherLabel", TEACHER_LABEL.into()),
            ("date", scenario.today.clone().into()),
            ("sourceBundleSha256", "synthetic-bundle".into()),
            (
                "students",
                Js::Arr(
                    scenario
                        .students
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
            day: Backend::new(SyntheticDayPort::new(scenario.clone()), admission),
            prestudy: PrestudySource::new(
                SyntheticPrestudyPort::new(scenario.clone()),
                TEACHER_LABEL,
                crate::synthetic::TEACHER_KEY,
            ),
            sections: TextbookIndexSource::new(
                SyntheticTextbookPort::new(scenario.clone()),
                TEACHER_LABEL,
            ),
            scenario,
            source_reads: 0,
            read_seq: 0,
        }
    }

    /// Logical source reads: one per record probe, prestudy read or section
    /// check. Counted for the Q01/Q02 tests.
    pub fn source_reads(&self) -> usize {
        self.source_reads
    }

    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    pub fn resolve_student(&mut self, name: &str) -> Result<ResolvedStudent, BackendError> {
        let name_js: Js = name.into();
        let rows = self
            .day
            .search_students_by_name(&name_js, None, None, 0)
            .map_err(|_| {
                BackendError::Stop(Stop::new("student_not_found", "학생을 찾지 못했습니다."))
            })?;
        let resolved = resolve_name(&name_js, &rows);
        let value = resolved.get("value");
        match string(value.get("status")).as_str() {
            "resolved" => {
                let key = string(value.get("studentKey"));
                let class_name = self
                    .scenario
                    .student_by_name(name)
                    .map(|s| s.class_name.clone());
                Ok(ResolvedStudent {
                    name: name.to_string(),
                    key,
                    class_name,
                })
            }
            _ => {
                let reason = string(value.get("reasonCode"));
                if reason == "student_name_ambiguous" {
                    let options = rows
                        .array()
                        .iter()
                        .map(|r| AskOption {
                            value: string(r.get("studentName")),
                            label_ko: string(r.get("studentName")),
                            meaning_ko: "같은 이름의 학생".into(),
                        })
                        .collect();
                    Err(BackendError::Ask(Ask {
                        field: "student".into(),
                        question_ko: "어느 학생인가요?".into(),
                        options,
                    }))
                } else {
                    Err(BackendError::Stop(Stop::new(
                        "student_not_found",
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
        self.read_seq += 1;
        let date_text = date.format("%Y-%m-%d").to_string();
        self.day.admission.set("date", date_text.clone().into());
        let scope = obj(&[
            ("sourceNamespace", "academy.lms.dayrecord".into()),
            ("teacherId", TEACHER_ID.into()),
            ("studentId", student_key.into()),
            ("lessonDate", date_text.into()),
        ]);
        let uuid = format!("synthetic-{}", self.read_seq);
        match self.day.read(&scope, NOW, &uuid) {
            Ok(result) => {
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
                    Ok(None)
                } else {
                    Err(BackendError::source_unavailable())
                }
            }
        }
    }

    pub fn prestudy_verdict(&mut self, student_name: &str, minor: &str) -> Verdict {
        self.source_reads += 1;
        let lists = match self.prestudy.read_lists(TEACHER_LABEL, student_name, NOW) {
            Ok(lists) => lists,
            Err(_) => return Verdict::Unknown,
        };
        let matches = |rows: &[edu_academy_sources::prestudy::PrestudyListRow]| {
            rows.iter()
                .any(|r| r.unit_label.split_whitespace().any(|token| token == minor))
        };
        if matches(&lists.waiting) {
            Verdict::Waiting
        } else if matches(&lists.completed) {
            Verdict::Completed
        } else {
            Verdict::NotObserved
        }
    }

    pub fn book_scope(
        &mut self,
        student: &ResolvedStudent,
        book_label: &str,
    ) -> Result<Option<BookScope>, BackendError> {
        let textbook_student = TextbookStudent::new(&student.key, &student.name)
            .map_err(|_| BackendError::source_unavailable())?;
        let bindings = self
            .sections
            .bindings(TEACHER_LABEL, &textbook_student)
            .map_err(|_| BackendError::source_unavailable())?;
        match bindings.iter().find(|b| b.book_label == book_label) {
            Some(binding) => BookScope::new(&binding.course_key, &binding.book_no)
                .map(Some)
                .map_err(|_| BackendError::source_unavailable()),
            None => Ok(None),
        }
    }

    pub fn section_verdict(
        &mut self,
        student: &TextbookStudent,
        scope: &BookScope,
        minor: &str,
        activity: &str,
    ) -> SectionCheck {
        self.source_reads += 1;
        let scope_label = format!("{minor} {activity}");
        let index = match self.sections.index(TEACHER_LABEL, student, scope) {
            Ok(index) => index,
            Err(_) => {
                return SectionCheck {
                    scope: scope_label,
                    verdict: Verdict::Unknown,
                    correct: None,
                    total: None,
                };
            }
        };
        let title = index.rows.iter().find(|r| {
            r.source_title.starts_with(&format!("[{activity}]"))
                && r.source_title
                    .split_whitespace()
                    .any(|token| token == minor)
        });
        let Some(title) = title else {
            return SectionCheck {
                scope: scope_label,
                verdict: Verdict::NotObserved,
                correct: None,
                total: None,
            };
        };
        let title = title.source_title.clone();
        match self.sections.grade(TEACHER_LABEL, student, scope, &[title]) {
            Ok(GradeOutcome::Graded(graded)) => {
                let record = graded.records.first();
                SectionCheck {
                    scope: scope_label,
                    verdict: Verdict::Found,
                    correct: record.map(|r| r.correct_count()),
                    total: record.map(|r| r.item_count()),
                }
            }
            Ok(GradeOutcome::NotObservedInEither(_)) => SectionCheck {
                scope: scope_label,
                verdict: Verdict::NotObservedInEitherSource,
                correct: None,
                total: None,
            },
            Err(_) => SectionCheck {
                scope: scope_label,
                verdict: Verdict::Unknown,
                correct: None,
                total: None,
            },
        }
    }
}
