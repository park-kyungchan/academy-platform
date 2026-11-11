//! `academy_lesson_prepare` as a library (plan s6). The typed result of
//! this tool lives here; the envelope carries it as a JSON value.
use crate::{
    backend::{BackendError, BookResolution, DayRecord, ResolvedStudent, SourceBackend},
    display_labels as l,
    envelope::{Ask, AskField, AskOption, Coverage, Envelope, SourceRead, Stop, StopCode},
    facts::{DateRange, NotBrought, Reminder, TeacherFactReport, TeacherFacts},
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
    pub starts_minor: bool,
    pub book_label: String,
    pub minor: String,
    pub activity: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    #[serde(default)]
    pub book: String,
    #[serde(default)]
    pub title_read: bool,
    #[serde(default)]
    pub observed_on: Option<String>,
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
    #[serde(default)]
    pub student_key: String,
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub searched_dates: Vec<String>,
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
    #[serde(skip)]
    pub display_omitted: bool,
    #[serde(skip)]
    pub display_subject: Option<String>,
    #[serde(default)]
    pub admitted_dates: Option<DateRange>,
    #[serde(default)]
    pub read_at: String,
    #[serde(default)]
    pub today: String,
    #[serde(default)]
    pub date_resolution: Option<String>,
    #[serde(default)]
    pub teacher_fact_reports: Vec<TeacherFactReport>,
    #[serde(default)]
    pub reminders: Vec<Reminder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    pub lesson_date: String,
    pub students: Vec<StudentLesson>,
}

fn book_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(l::BOOK_PATTERN).expect("constant"))
}
fn minor_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[0-9]+-[0-9]+").expect("constant"))
}

const ACTIVITIES: [&str; 4] = [l::REQUIRED, l::PRACTICE, l::ADVANCED, l::CHAPTER];

/// Scan one record line for its book label, minor and activity.
pub fn scan_line(line: &str) -> Option<LineRef> {
    let text = line.trim().strip_prefix("- ").unwrap_or(line.trim());
    let mut books = book_re().find_iter(text);
    let book = books.next()?;
    let end = books.next().map_or(text.len(), |b| b.start());
    let rest = &text[book.end()..end];
    let minor = minor_re().find(rest)?;
    let after = &rest[minor.end()..];
    let activity = ACTIVITIES.iter().find(|a| after.contains(**a))?;
    Some(LineRef {
        starts_minor: rest.contains(l::START),
        book_label: book.as_str().to_string(),
        minor: minor.as_str().to_string(),
        activity: (*activity).to_string(),
    })
}

/// Every book-delimited homework item, including items on one physical line.
pub fn homework_items(text: &str) -> Vec<LineRef> {
    let starts: Vec<_> = book_re().find_iter(text).map(|m| m.start()).collect();
    starts
        .iter()
        .enumerate()
        .filter_map(|(i, start)| {
            scan_line(&text[*start..starts.get(i + 1).copied().unwrap_or(text.len())])
        })
        .collect()
}

/// Homework at a minor start: the homework minor differs from the progress
/// minor, or the line says the minor starts.
pub fn homework_minor_is_start(progress: &str, homework: &str) -> Option<LineRef> {
    let homework_ref = scan_line(homework)?;
    let continuing = homework_items(progress)
        .iter()
        .any(|p| p.book_label == homework_ref.book_label && p.minor == homework_ref.minor);
    if !continuing || homework_ref.starts_minor {
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

fn candidate_dates(
    facts: &dyn TeacherFacts,
    student: &ResolvedStudent,
    date: NaiveDate,
) -> Vec<NaiveDate> {
    if let Some(class) = student
        .class_name
        .as_deref()
        .and_then(|name| facts.class(name))
        .filter(|c| !c.weekdays.is_empty())
    {
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
        return candidates;
    }
    (1..=14)
        .filter_map(|offset| date.checked_sub_signed(chrono::Duration::days(offset)))
        .collect()
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
    let candidates = candidate_dates(facts, student, date);
    if student
        .class_name
        .as_deref()
        .and_then(|name| facts.class(name))
        .is_some_and(|c| !c.weekdays.is_empty())
    {
        let moved = student
            .class_name
            .as_deref()
            .map_or(&[][..], |name| facts.moved_lessons(name));
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
        question_ko: l::DATE_QUESTION.into(),
        options: vec![AskOption {
            value: today.format("%Y-%m-%d").to_string(),
            label_ko: l::TODAY.into(),
            meaning_ko: l::TODAY_MEANING.into(),
        }],
    };
    let Some(raw) = input.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(today);
    };
    if raw == l::TODAY {
        return Ok(today);
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(date);
    }
    let korean = regex::Regex::new(l::DATE_PATTERN).expect("constant");
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
    Envelope::ask(display, data_value(data), ask, coverage.clone()).unwrap_or_else(|reason| {
        let mut coverage = coverage;
        coverage.unknowns.push(format!("ask_invalid:{reason}"));
        Envelope::stop(
            l::DISPLAY_FAILURE.into(),
            data_value(data),
            Stop::new(StopCode::DisplayInvalid, l::DISPLAY_FAILURE),
            coverage,
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
            meaning_ko: l::CLASS_MEANING.into(),
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
    require_on_date: bool,
    search_absent: Option<&[String]>,
) -> Result<PreparedStudent, BackendError> {
    let read_at = backend.read_at().to_string();
    let mut sources = Vec::new();
    let mut unknowns = Vec::new();
    let carry_over = carry_over_of(facts, &student.key);
    let on_date = if search_absent.is_some() {
        false
    } else if require_on_date {
        backend.read_record(&student.key, lesson_date)?.is_some()
    } else {
        true
    };
    let outcome = if let Some(dates) = search_absent {
        let from = dates
            .iter()
            .min()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or(lesson_date);
        let to = dates
            .iter()
            .max()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or(lesson_date);
        PreviousOutcome::NotFound { from, to }
    } else if on_date {
        previous_lesson(backend, facts, student, lesson_date)
    } else {
        PreviousOutcome::NotFound {
            from: lesson_date,
            to: lesson_date,
        }
    };
    let (previous, searched_from, searched_to, read_failed) = match outcome {
        PreviousOutcome::Found(found) => {
            sources.push(SourceRead {
                name: "daily_record".into(),
                read_at: read_at.clone(),
                scope: format!("{} {}", student.name, found.date),
            });
            (
                Some(PreviousLesson {
                    date: found.date.to_string(),
                    how_found: found.how_found,
                    progress: found.record.progress,
                    homework: found.record.homework,
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
                scope: format!("{} {}/{}", student.name, from, to),
            });
            unknowns.push(format!("previous_lesson_not_found:{}", student.name));
            (None, Some(from.to_string()), Some(to.to_string()), false)
        }
        PreviousOutcome::Failed => {
            unknowns.push(format!("daily_record_read_failed:{}", student.name));
            (None, None, None, true)
        }
    };
    let mut checks = Vec::new();
    if let Some(prev) = &previous {
        let mut read_minors = Vec::new();
        let items = homework_items(&prev.homework);
        for item in &items {
            if read_minors.contains(&(item.book_label.clone(), item.minor.clone())) {
                continue;
            }
            read_minors.push((item.book_label.clone(), item.minor.clone()));
            // Start requirements are evaluated per book, not against another book's progress.
            let continuing = homework_items(&prev.progress)
                .iter()
                .any(|p| p.book_label == item.book_label && p.minor == item.minor);
            let starts_minor = items.iter().any(|other| {
                other.book_label == item.book_label
                    && other.minor == item.minor
                    && other.starts_minor
            });
            if continuing && !starts_minor {
                // The footer names registration only after this actual read, even when
                // a continuing minor needs no new prestudy/index requirement.
                if backend
                    .resolve_book(student, requested_book.unwrap_or(&item.book_label))
                    .is_ok()
                {
                    sources.push(SourceRead {
                        name: "registration".into(),
                        read_at: read_at.clone(),
                        scope: format!("{} {}", student.name, item.book_label),
                    });
                } else {
                    unknowns.push(format!(
                        "registration_read_failed:{}:{}",
                        student.name, item.book_label
                    ));
                }
                continue;
            }
            let (prestudy, observed_on) =
                backend.prestudy_observation(&student.name, &item.book_label, &item.minor);
            if prestudy != Verdict::Unknown {
                sources.push(SourceRead {
                    name: "prestudy".into(),
                    read_at: read_at.clone(),
                    scope: format!("{} {} {}", student.name, item.book_label, item.minor),
                });
            } else {
                unknowns.push(format!("prestudy_unknown:{}:{}", student.name, item.minor));
            }
            checks.push(Check {
                book: item.book_label.clone(),
                title_read: false,
                observed_on,
                kind: "prestudy".into(),
                scope: item.minor.clone(),
                verdict: prestudy,
                correct: None,
                total: None,
            });
            let effective_book = requested_book.unwrap_or(&item.book_label);
            let textbook_student = TextbookStudent::new(&student.key, &student.name)
                .map_err(|_| BackendError::source_unavailable())?;
            let resolved = backend.resolve_book(student, effective_book)?;
            sources.push(SourceRead {
                name: "registration".into(),
                read_at: read_at.clone(),
                scope: format!("{} {}", student.name, effective_book),
            });
            let scope = match resolved {
                BookResolution::Found(scope) => Some(scope),
                BookResolution::NoBinding { bound_labels } => {
                    if !bound_labels.is_empty() {
                        return Err(BackendError::Ask(Ask {
                            field: AskField::Book,
                            question_ko: l::BOOK_QUESTION.into(),
                            options: bound_labels
                                .into_iter()
                                .map(|label| AskOption {
                                    value: label.clone(),
                                    label_ko: label,
                                    meaning_ko: l::BOOK_MEANING.into(),
                                })
                                .collect(),
                        }));
                    }
                    None
                }
            };
            if let Some(scope) = scope {
                let section_checks = backend.section_checks(
                    &textbook_student,
                    &scope,
                    &item.minor,
                    &[l::REQUIRED, l::PRACTICE],
                );
                if section_checks.iter().any(|c| c.title_read) {
                    sources.push(SourceRead {
                        name: "textbook_sections".into(),
                        read_at: read_at.clone(),
                        scope: format!("{} {} {}", student.name, effective_book, item.minor),
                    });
                }
                for c in section_checks {
                    if c.verdict == Verdict::Unknown {
                        unknowns.push(format!("section_unknown:{}:{}", student.name, c.scope));
                    }
                    checks.push(Check {
                        book: effective_book.into(),
                        title_read: c.title_read,
                        observed_on: None,
                        kind: "section".into(),
                        scope: c.scope,
                        verdict: c.verdict,
                        correct: c.correct,
                        total: c.total,
                    });
                }
            } else {
                for activity in [l::REQUIRED, l::PRACTICE] {
                    checks.push(Check {
                        book: effective_book.into(),
                        title_read: false,
                        observed_on: None,
                        kind: "section".into(),
                        scope: format!("{} {activity}", item.minor),
                        verdict: Verdict::Unknown,
                        correct: None,
                        total: None,
                    });
                }
            }
        }
    }
    Ok(PreparedStudent {
        lesson: StudentLesson {
            student_key: student.key.clone(),
            class_name: student.class_name.clone(),
            searched_dates: if let Some(dates) = search_absent {
                dates.to_vec()
            } else if on_date {
                candidate_dates(facts, student, lesson_date)
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            } else {
                vec![lesson_date.to_string()]
            },
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
    let class_arg = trimmed(&req.class_name).filter(|v| v != l::NO_CLASS);
    let book_arg = trimmed(&req.book);
    let previous_requested = req
        .date
        .as_deref()
        .is_some_and(|s| s.trim() == l::PREVIOUS_DATE);
    let lesson_date = match parse_request_date(
        if previous_requested {
            None
        } else {
            req.date.as_deref()
        },
        today,
    ) {
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
        admitted_dates: facts.admitted_dates().cloned(),
        read_at: facts
            .read_at()
            .unwrap_or_else(|| backend.read_at())
            .to_string(),
        today: today.to_string(),
        ..Default::default()
    };

    if !previous_requested
        && let Some(range) = facts.admitted_dates()
        && (data.lesson_date < range.from || data.lesson_date > range.to)
    {
        let display = l::outside(&data.lesson_date, &range.from, &range.to);
        return Envelope::stop(
            display.clone(),
            data_value(&data),
            Stop::new(StopCode::DateNotAdmitted, &display),
            Coverage::default(),
        );
    }
    let (names, class_hint): (Vec<String>, Option<String>) = if let Some(student) = &student_arg {
        (vec![student.clone()], class_arg.clone())
    } else if let Some(class_name) = &class_arg {
        data.class_name = Some(class_name.clone());
        match facts.class(class_name) {
            Some(class) => (class.roster.clone(), Some(class_name.clone())),
            None => {
                let ask = Ask {
                    field: AskField::ClassName,
                    question_ko: l::CLASS_QUESTION.into(),
                    options: class_options(facts),
                };
                return ask_envelope(l::CLASS_QUESTION.into(), &data, ask, Coverage::default());
            }
        }
    } else {
        let ask = Ask {
            field: AskField::ClassName,
            question_ko: l::CLASS_QUESTION.into(),
            options: class_options(facts),
        };
        return ask_envelope(l::CLASS_QUESTION.into(), &data, ask, Coverage::default());
    };

    let mut coverage = Coverage::default();
    let mut resolved_students = Vec::new();
    for name in &names {
        let name = name.trim();
        let student = match backend.resolve_student(name, class_hint.as_deref()) {
            Ok(student) => student,
            Err(BackendError::Ask(mut ask)) => {
                if ask.field == AskField::ClassName
                    && let Some(class_name) = &data.class_name
                {
                    let message = l::roster_mismatch(class_name, name);
                    return Envelope::stop(
                        message.clone(),
                        data_value(&data),
                        Stop::new(StopCode::RosterClassMismatch, &message),
                        coverage,
                    );
                }
                if ask.field == AskField::ClassName && ask.options.len() == 1 {
                    let actual = ask.options[0].value.clone();
                    let requested = class_hint.as_deref().unwrap_or("");
                    ask.question_ko = l::disagreement(name, &actual, requested);
                    ask.options[0].meaning_ko = l::class_answer(name, &actual);
                    ask.options.push(AskOption {
                        value: String::new(),
                        label_ko: l::NO_CLASS.into(),
                        meaning_ko: l::no_class_answer(name),
                    });
                }
                return ask_envelope(ask.question_ko.clone(), &data, ask, coverage);
            }
            Err(BackendError::Stop(stop)) => {
                if stop.code == StopCode::StudentNotFound
                    && let Some(class) = &data.class_name
                {
                    let display = l::roster_missing(class, name);
                    return Envelope::stop(
                        display.clone(),
                        data_value(&data),
                        Stop::new(StopCode::StudentNotFound, &display),
                        coverage,
                    );
                }
                let display = stop.message_ko.clone();
                return Envelope::stop(display, data_value(&data), stop, coverage);
            }
        };
        coverage.sources.push(SourceRead {
            name: "student_info".into(),
            read_at: backend.read_at().into(),
            scope: student.name.clone(),
        });
        resolved_students.push(student);
    }
    let mut effective_date = lesson_date;
    let mut resolution_absent: Option<Vec<String>> = None;
    if previous_requested && !resolved_students.is_empty() {
        let resolved = if data.class_name.is_some() {
            let mut found = None;
            let scheduled_lower = resolved_students
                .iter()
                .flat_map(|s| candidate_dates(facts, s, today))
                .min();
            let admitted_lower = facts
                .admitted_dates()
                .and_then(|r| NaiveDate::parse_from_str(&r.from, "%Y-%m-%d").ok());
            let lower = scheduled_lower
                .into_iter()
                .chain(admitted_lower)
                .min()
                .unwrap_or(today);
            let mut searched = Vec::new();
            'days: for offset in 1..=today.signed_duration_since(lower).num_days() {
                let Some(day) = today.checked_sub_signed(chrono::Duration::days(offset)) else {
                    break;
                };
                searched.push(day.to_string());
                for student in &resolved_students {
                    match backend.read_record(&student.key, day) {
                        Ok(Some(_)) => {
                            found = Some(day);
                            break 'days;
                        }
                        Ok(None) => {}
                        Err(_) => {
                            return Envelope::stop(
                                l::SOURCE_FAILURE.into(),
                                data_value(&data),
                                Stop::new(StopCode::SourceUnavailable, l::SOURCE_FAILURE),
                                coverage,
                            );
                        }
                    }
                }
            }
            if found.is_none() {
                resolution_absent = Some(searched);
            }
            found
        } else {
            match resolved_students
                .first()
                .map(|student| previous_lesson(backend, facts, student, today))
            {
                Some(PreviousOutcome::Found(found)) => Some(found.date),
                Some(PreviousOutcome::NotFound { .. }) => {
                    resolution_absent = Some(
                        candidate_dates(facts, &resolved_students[0], today)
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                    );
                    None
                }
                _ => {
                    return Envelope::stop(
                        l::SOURCE_FAILURE.into(),
                        data_value(&data),
                        Stop::new(StopCode::SourceUnavailable, l::SOURCE_FAILURE),
                        coverage,
                    );
                }
            }
        };
        if let Some(date) = resolved {
            effective_date = date;
            data.date_resolution = Some(date.to_string());
            data.lesson_date = date.to_string();
            coverage.sources.push(SourceRead {
                name: "date_resolution".into(),
                read_at: backend.read_at().into(),
                scope: format!(
                    "{} {}",
                    data.class_name
                        .as_deref()
                        .unwrap_or(&resolved_students[0].name),
                    date
                ),
            });
            if let Some(range) = facts.admitted_dates()
                && (data.lesson_date < range.from || data.lesson_date > range.to)
            {
                let display = l::outside(&data.lesson_date, &range.from, &range.to);
                return Envelope::stop(
                    display.clone(),
                    data_value(&data),
                    Stop::new(StopCode::DateNotAdmitted, &display),
                    coverage,
                );
            }
        }
    }
    for student in &resolved_students {
        match prepare_student(
            backend,
            facts,
            student,
            effective_date,
            book_arg.as_deref(),
            previous_requested && data.class_name.is_some() && resolution_absent.is_none(),
            resolution_absent.as_deref(),
        ) {
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
            l::SOURCE_FAILURE.into(),
            data_value(&data),
            Stop::new(StopCode::SourceUnavailable, l::SOURCE_FAILURE),
            coverage,
        );
    }
    data.teacher_fact_reports = facts
        .reports()
        .into_iter()
        .filter(|r| {
            data.students
                .iter()
                .any(|s| r.student_key.as_deref() == Some(&s.student_key))
        })
        .collect();
    data.reminders = facts
        .reminders()
        .into_iter()
        .filter(|r| r.status == "open")
        .collect();
    if facts.read_at().is_some() {
        coverage.sources.insert(
            0,
            SourceRead {
                name: "teacher_facts".into(),
                read_at: data.read_at.clone(),
                scope: "reminders,schedule,roster,reports".into(),
            },
        );
    }
    let display = render::bounded_lesson_prepare(&data, &mut coverage);
    if let Err(reason) = render::display_lint(&display) {
        coverage.unknowns.push(format!("display_invalid:{reason}"));
        return Envelope::stop(
            l::DISPLAY_FAILURE.into(),
            data_value(&data),
            Stop::new(StopCode::DisplayInvalid, l::DISPLAY_FAILURE),
            coverage,
        );
    }
    Envelope::ok(display, data_value(&data), coverage)
}
