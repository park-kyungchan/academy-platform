//! `academy_lesson_prepare` as a library (plan s6). The typed result of
//! this tool lives here; the envelope carries it as a JSON value.
use crate::{
    backend::{BackendError, BookResolution, DayRecord, ResolvedStudent, SourceBackend},
    envelope::{Ask, AskField, AskOption, Coverage, Envelope, SourceRead, Stop, StopCode},
    facts::{NotBrought, TeacherFacts},
    render::{self, Verdict},
};
use chrono::{Datelike, NaiveDate};
use edu_academy_sources::{
    native::SourcePort, prestudy::PrestudyPort, textbook_index::TextbookSectionPort,
    textbook_index::TextbookStudent,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Debug, Default)]
pub struct PrepareRequest {
    pub student: Option<String>,
    pub class_name: Option<String>,
    pub date: Option<String>,
    pub book: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRef {
    pub book_label: String,
    pub minor: String,
    pub activity: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub kind: String,
    pub scope: String,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviousLesson {
    pub date: String,
    pub how_found: String,
    pub progress: String,
    pub homework: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarryOverItem {
    pub homework_line: String,
    pub assigned_on: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StudentLesson {
    pub student: String,
    pub previous_lesson: Option<PreviousLesson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searched_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searched_to: Option<String>,
    pub read_failed: bool,
    pub checks: Vec<Check>,
    pub carry_over: Vec<CarryOverItem>,
    pub excluded: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LessonData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    pub lesson_date: String,
    pub students: Vec<StudentLesson>,
}

fn book_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(가우스|다빈치)( 플러스)? [0-9]+-[0-9]+(-[0-9]+)?").expect("constant")
    })
}
fn minor_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[0-9]+-[0-9]+").expect("constant"))
}

const ACTIVITIES: [&str; 4] = ["필수예제", "유형다지기", "실력쌓기", "대단원총괄"];

/// Scan one record line for its book label, minor and activity.
pub fn scan_line(line: &str) -> Option<LineRef> {
    let text = line.trim().strip_prefix("- ").unwrap_or(line.trim());
    let book = book_re().find(text)?;
    let rest = &text[book.end()..];
    let minor = minor_re().find(rest)?;
    let after = &rest[minor.end()..];
    let activity = ACTIVITIES.iter().find(|a| after.contains(**a))?;
    Some(LineRef {
        book_label: book.as_str().to_string(),
        minor: minor.as_str().to_string(),
        activity: (*activity).to_string(),
    })
}

/// Homework at a minor start: the homework minor differs from the progress
/// minor, or the line says the minor starts.
pub fn homework_minor_is_start(progress: &str, homework: &str) -> Option<LineRef> {
    let homework_ref = scan_line(homework)?;
    let progress_minor = scan_line(progress).map(|r| r.minor);
    if progress_minor.as_deref() != Some(homework_ref.minor.as_str()) || homework.contains("시작")
    {
        Some(homework_ref)
    } else {
        None
    }
}

pub struct FoundPrevious {
    pub date: NaiveDate,
    pub how_found: String,
    pub record: DayRecord,
}

pub enum PreviousOutcome {
    Found(FoundPrevious),
    NotFound { from: NaiveDate, to: NaiveDate },
    Failed,
}

/// Previous lesson per DESIGN part 1: schedule (with moved lessons, at most
/// 6 candidates), else a day scan of at most 14 days. A failed read of the
/// latest candidate stays a failure: an older record is never substituted.
pub fn previous_lesson<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort>(
    backend: &mut SourceBackend<D, P, T>,
    facts: &dyn TeacherFacts,
    student: &ResolvedStudent,
    date: NaiveDate,
) -> PreviousOutcome {
    let class = student
        .class_name
        .as_deref()
        .and_then(|name| facts.class(name));
    if let Some(class) = class.filter(|c| !c.weekdays.is_empty()) {
        let moved = facts.moved_lessons(&class.name).to_vec();
        let mut candidates: Vec<NaiveDate> = Vec::new();
        let mut day = date;
        while candidates.len() < 6 {
            let Some(prev) = day.pred_opt() else { break };
            day = prev;
            if moved.iter().any(|m| m.from == day) {
                continue;
            }
            let is_moved_to = moved.iter().any(|m| m.to == day);
            if is_moved_to || class.weekdays.contains(&day.weekday()) {
                candidates.push(day);
            }
            if date.signed_duration_since(day).num_days() > 366 {
                break;
            }
        }
        let (from, to) = match (candidates.last(), candidates.first()) {
            (Some(first), Some(last)) => (*first, *last),
            _ => (date, date),
        };
        for candidate in candidates {
            match backend.read_record(&student.key, candidate) {
                Ok(Some(record)) => {
                    let how_found = if moved.iter().any(|m| m.to == candidate) {
                        "moved_lesson"
                    } else {
                        "schedule"
                    };
                    return PreviousOutcome::Found(FoundPrevious {
                        date: candidate,
                        how_found: how_found.into(),
                        record,
                    });
                }
                Ok(None) => continue,
                Err(_) => return PreviousOutcome::Failed,
            }
        }
        return PreviousOutcome::NotFound { from, to };
    }
    // Day scan.
    let end = date.pred_opt().unwrap_or(date);
    let start = (1..=14).fold(date, |d, _| d.pred_opt().unwrap_or(d));
    for offset in 1..=14i64 {
        let Some(candidate) = date.checked_sub_signed(chrono::Duration::days(offset)) else {
            break;
        };
        match backend.read_record(&student.key, candidate) {
            Ok(Some(record)) => {
                return PreviousOutcome::Found(FoundPrevious {
                    date: candidate,
                    how_found: "day_scan".into(),
                    record,
                });
            }
            Ok(None) => continue,
            Err(_) => return PreviousOutcome::Failed,
        }
    }
    PreviousOutcome::NotFound {
        from: start,
        to: end,
    }
}

fn parse_request_date(input: Option<&str>, today: NaiveDate) -> Result<NaiveDate, Ask> {
    let ask = || Ask {
        field: AskField::Date,
        question_ko: "날짜를 알려주세요.".into(),
        options: vec![AskOption {
            value: today.format("%Y-%m-%d").to_string(),
            label_ko: "오늘".into(),
            meaning_ko: "오늘 날짜".into(),
        }],
    };
    let Some(raw) = input.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(today);
    };
    if raw == "오늘" {
        return Ok(today);
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(date);
    }
    let korean = regex::Regex::new(r"^(\d{1,2})월\s*(\d{1,2})일$").expect("constant");
    if let Some(caps) = korean.captures(raw) {
        let month: u32 = caps[1].parse().map_err(|_| ask())?;
        let day: u32 = caps[2].parse().map_err(|_| ask())?;
        return NaiveDate::from_ymd_opt(today.year(), month, day).ok_or_else(ask);
    }
    Err(ask())
}

fn data_value(data: &LessonData) -> serde_json::Value {
    serde_json::to_value(data).expect("lesson data serializes")
}

fn ask_envelope(display: String, data: &LessonData, ask: Ask, coverage: Coverage) -> Envelope {
    Envelope::ask(display, data_value(data), ask, coverage).unwrap_or_else(|_| {
        Envelope::stop(
            "원천을 읽지 못했습니다.".into(),
            data_value(data),
            Stop::new(StopCode::SourceUnavailable, "원천을 읽지 못했습니다."),
            Coverage::default(),
        )
    })
}

fn class_options(facts: &dyn TeacherFacts) -> Vec<AskOption> {
    facts
        .class_names()
        .into_iter()
        .map(|name| AskOption {
            value: name.clone(),
            label_ko: name,
            meaning_ko: "반 전체 준비".into(),
        })
        .collect()
}

fn carry_over_of(facts: &dyn TeacherFacts, student_key: &str) -> Vec<CarryOverItem> {
    facts
        .not_brought(student_key)
        .into_iter()
        .filter(|item: &NotBrought| !item.judged)
        .map(|item| CarryOverItem {
            homework_line: item.homework_line,
            assigned_on: item.assigned_on.format("%Y-%m-%d").to_string(),
        })
        .collect()
}

struct PreparedStudent {
    lesson: StudentLesson,
    sources: Vec<SourceRead>,
    unknowns: Vec<String>,
}

fn prepare_student<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort>(
    backend: &mut SourceBackend<D, P, T>,
    facts: &dyn TeacherFacts,
    student: &ResolvedStudent,
    lesson_date: NaiveDate,
    requested_book: Option<&str>,
) -> Result<PreparedStudent, BackendError> {
    let read_at = backend.read_at().to_string();
    let mut sources: Vec<SourceRead> = Vec::new();
    let mut unknowns: Vec<String> = Vec::new();
    let carry_over = carry_over_of(facts, &student.key);
    let (previous, searched_from, searched_to, read_failed) =
        match previous_lesson(backend, facts, student, lesson_date) {
            PreviousOutcome::Found(found) => {
                sources.push(SourceRead {
                    name: "daily_record".into(),
                    read_at: read_at.clone(),
                    scope: format!("{} {}", student.name, found.date.format("%Y-%m-%d")),
                });
                (
                    Some(PreviousLesson {
                        date: found.date.format("%Y-%m-%d").to_string(),
                        how_found: found.how_found,
                        progress: found.record.progress.clone(),
                        homework: found.record.homework.clone(),
                    }),
                    None,
                    None,
                    false,
                )
            }
            PreviousOutcome::NotFound { from, to } => {
                sources.push(SourceRead {
                    name: "daily_record".into(),
                    read_at: read_at.clone(),
                    scope: format!(
                        "{} {}/{}",
                        student.name,
                        from.format("%Y-%m-%d"),
                        to.format("%Y-%m-%d")
                    ),
                });
                unknowns.push(format!("previous_lesson_not_found:{}", student.name));
                (
                    None,
                    Some(from.format("%Y-%m-%d").to_string()),
                    Some(to.format("%Y-%m-%d").to_string()),
                    false,
                )
            }
            PreviousOutcome::Failed => {
                unknowns.push(format!("daily_record_read_failed:{}", student.name));
                (None, None, None, true)
            }
        };
    let mut checks = Vec::new();
    if let Some(prev) = &previous
        && let Some(start) = homework_minor_is_start(&prev.progress, &prev.homework)
    {
        let prestudy = backend.prestudy_verdict(&student.name, &start.book_label, &start.minor);
        if prestudy != Verdict::Unknown {
            sources.push(SourceRead {
                name: "prestudy".into(),
                read_at: read_at.clone(),
                scope: format!("{} {} {}", student.name, start.book_label, start.minor),
            });
        }
        if prestudy == Verdict::Unknown {
            unknowns.push(format!("prestudy_unknown:{}:{}", student.name, start.minor));
        }
        checks.push(Check {
            kind: "prestudy".into(),
            scope: start.minor.clone(),
            verdict: prestudy,
            correct: None,
            total: None,
        });
        let effective_book = requested_book.unwrap_or(&start.book_label);
        let textbook_student = match TextbookStudent::new(&student.key, &student.name) {
            Ok(student_ref) => student_ref,
            Err(_) => {
                return Ok(PreparedStudent {
                    lesson: StudentLesson {
                        student: student.name.clone(),
                        previous_lesson: previous,
                        searched_from,
                        searched_to,
                        read_failed: true,
                        checks,
                        carry_over,
                        excluded: vec![],
                    },
                    sources,
                    unknowns,
                });
            }
        };
        let scope = match backend.resolve_book(student, effective_book)? {
            BookResolution::Found(scope) => scope,
            BookResolution::NoBinding { bound_labels } => {
                if bound_labels.is_empty() {
                    for activity in ["필수예제", "유형다지기"] {
                        unknowns.push(format!("section_unknown:{}:{}", student.name, start.minor));
                        checks.push(Check {
                            kind: "section".into(),
                            scope: format!("{} {activity}", start.minor),
                            verdict: Verdict::Unknown,
                            correct: None,
                            total: None,
                        });
                    }
                    return Ok(PreparedStudent {
                        lesson: StudentLesson {
                            student: student.name.clone(),
                            previous_lesson: previous,
                            searched_from,
                            searched_to,
                            read_failed,
                            checks,
                            carry_over,
                            excluded: vec![],
                        },
                        sources,
                        unknowns,
                    });
                }
                return Err(BackendError::Ask(Ask {
                    field: AskField::Book,
                    question_ko: "어느 교재인가요?".into(),
                    options: bound_labels
                        .into_iter()
                        .map(|label| AskOption {
                            value: label.clone(),
                            label_ko: label,
                            meaning_ko: "그 학생에게 등록된 교재".into(),
                        })
                        .collect(),
                }));
            }
        };
        let section_checks = backend.section_checks(
            &textbook_student,
            &scope,
            &start.minor,
            &["필수예제", "유형다지기"],
        );
        if section_checks
            .iter()
            .any(|check| check.verdict != Verdict::Unknown)
        {
            sources.push(SourceRead {
                name: "textbook_sections".into(),
                read_at,
                scope: format!("{} {} {}", student.name, effective_book, start.minor),
            });
        }
        for check in section_checks {
            if check.verdict == Verdict::Unknown {
                unknowns.push(format!("section_unknown:{}:{}", student.name, check.scope));
            }
            checks.push(Check {
                kind: "section".into(),
                scope: check.scope,
                verdict: check.verdict,
                correct: check.correct,
                total: check.total,
            });
        }
    }
    Ok(PreparedStudent {
        lesson: StudentLesson {
            student: student.name.clone(),
            previous_lesson: previous,
            searched_from,
            searched_to,
            read_failed,
            checks,
            carry_over,
            excluded: vec![],
        },
        sources,
        unknowns,
    })
}

pub fn prepare<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort>(
    backend: &mut SourceBackend<D, P, T>,
    facts: &dyn TeacherFacts,
    req: &PrepareRequest,
    today: NaiveDate,
) -> Envelope {
    // Per-call lesson-data read counts: a backend kept across calls gives
    // the same status rule on every call from this difference alone.
    let (attempted_at_start, succeeded_at_start) = backend.read_counts();
    let trimmed = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let student_arg = trimmed(&req.student);
    let class_arg = trimmed(&req.class_name);
    let book_arg = trimmed(&req.book);
    let lesson_date = match parse_request_date(req.date.as_deref(), today) {
        Ok(date) => date,
        Err(ask) => {
            let data = LessonData {
                lesson_date: today.format("%Y-%m-%d").to_string(),
                ..Default::default()
            };
            return ask_envelope(ask.question_ko.clone(), &data, ask, Coverage::default());
        }
    };
    let mut data = LessonData {
        class_name: None,
        lesson_date: lesson_date.format("%Y-%m-%d").to_string(),
        students: vec![],
    };

    let (names, class_hint): (Vec<String>, Option<String>) = if let Some(student) = &student_arg {
        (vec![student.clone()], class_arg.clone())
    } else if let Some(class_name) = &class_arg {
        data.class_name = Some(class_name.clone());
        match facts.class(class_name) {
            Some(class) => (class.roster.clone(), Some(class_name.clone())),
            None => {
                let ask = Ask {
                    field: AskField::ClassName,
                    question_ko: "어느 반인가요?".into(),
                    options: class_options(facts),
                };
                return ask_envelope("어느 반인가요?".into(), &data, ask, Coverage::default());
            }
        }
    } else {
        let ask = Ask {
            field: AskField::ClassName,
            question_ko: "어느 반인가요?".into(),
            options: class_options(facts),
        };
        return ask_envelope(
            "어느 반인가요?\n반 이름 대신 학생 이름을 알려주셔도 됩니다.".into(),
            &data,
            ask,
            Coverage::default(),
        );
    };

    let mut coverage = Coverage::default();
    for name in &names {
        let name = name.trim();
        let student = match backend.resolve_student(name, class_hint.as_deref()) {
            Ok(student) => student,
            Err(BackendError::Ask(ask)) => {
                if ask.field == AskField::ClassName
                    && let Some(class_name) = &data.class_name
                {
                    let message = format!(
                        "{class_name}의 반 명부와 학생 기록이 {name} 학생의 반에 대해 일치하지 않아 반 전체 수업 준비를 중단했습니다. 반 명부를 수정하거나 학생 이름을 지정해 주세요."
                    );
                    return Envelope::stop(
                        message.clone(),
                        data_value(&data),
                        Stop::new(StopCode::StudentNotFound, &message),
                        coverage,
                    );
                }
                return ask_envelope(ask.question_ko.clone(), &data, ask, coverage);
            }
            Err(BackendError::Stop(stop)) => {
                let display = stop.message_ko.clone();
                return Envelope::stop(display, data_value(&data), stop, coverage);
            }
        };
        match prepare_student(backend, facts, &student, lesson_date, book_arg.as_deref()) {
            Ok(prepared) => {
                coverage.sources.extend(prepared.sources);
                coverage.unknowns.extend(prepared.unknowns);
                data.students.push(prepared.lesson);
            }
            Err(BackendError::Ask(ask)) => {
                return ask_envelope(ask.question_ko.clone(), &data, ask, coverage);
            }
            Err(BackendError::Stop(stop)) => {
                let display = stop.message_ko.clone();
                return Envelope::stop(display, data_value(&data), stop, coverage);
            }
        }
    }
    let (attempted_now, succeeded_now) = backend.read_counts();
    if attempted_now > attempted_at_start && succeeded_now == succeeded_at_start {
        return Envelope::stop(
            "원천을 읽지 못했습니다.".into(),
            data_value(&data),
            Stop::new(StopCode::SourceUnavailable, "원천을 읽지 못했습니다."),
            coverage,
        );
    }
    let display = render::lesson_prepare(&data);
    Envelope::ok(display, data_value(&data), coverage)
}
