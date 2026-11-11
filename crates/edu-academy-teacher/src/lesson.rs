//! `academy_lesson_prepare` as a library (plan s6).
use crate::{
    backend::{BackendError, DayRecord, ResolvedStudent, SourceBackend},
    envelope::{
        Ask, AskOption, Check, Coverage, Envelope, LessonData, PreviousLesson, SourceRead, Status,
        StudentLesson,
    },
    facts::TeacherFacts,
    render,
};
use chrono::{Datelike, NaiveDate};
use edu_academy_sources::textbook_index::TextbookStudent;
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
    NotFound { searched_range: String },
    Failed(BackendError),
}

/// Previous lesson per DESIGN part 1: schedule (with moved lessons, at most
/// 6 candidates), else a day scan of at most 14 days.
pub fn previous_lesson(
    backend: &mut SourceBackend,
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
        let range = match (candidates.last(), candidates.first()) {
            (Some(first), Some(last)) => format!(
                "{}부터 {}까지",
                first.format("%Y-%m-%d"),
                last.format("%Y-%m-%d")
            ),
            _ => format!("{} 이전", date.format("%Y-%m-%d")),
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
                Err(e) => return PreviousOutcome::Failed(e),
            }
        }
        return PreviousOutcome::NotFound {
            searched_range: range,
        };
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
            Err(e) => return PreviousOutcome::Failed(e),
        }
    }
    PreviousOutcome::NotFound {
        searched_range: format!(
            "{}부터 {}까지",
            start.format("%Y-%m-%d"),
            end.format("%Y-%m-%d")
        ),
    }
}

fn parse_request_date(input: Option<&str>, today: NaiveDate) -> Result<NaiveDate, Ask> {
    let ask = || Ask {
        field: "date".into(),
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

fn coverage(lesson_date: NaiveDate) -> Coverage {
    let scope = lesson_date.format("%Y-%m-%d").to_string();
    let read = |name: &str| SourceRead {
        name: name.into(),
        read_at: crate::synthetic::NOW.into(),
        scope: scope.clone(),
    };
    Coverage {
        sources: vec![
            read("daily_record"),
            read("prestudy"),
            read("textbook_sections"),
        ],
        unknowns: vec![],
    }
}

fn prepare_student(
    backend: &mut SourceBackend,
    facts: &dyn TeacherFacts,
    student: &ResolvedStudent,
    lesson_date: NaiveDate,
    requested_book: Option<&str>,
) -> Result<StudentLesson, BackendError> {
    let (previous, searched_range) = match previous_lesson(backend, facts, student, lesson_date) {
        PreviousOutcome::Found(found) => (
            Some(PreviousLesson {
                date: found.date.format("%Y-%m-%d").to_string(),
                how_found: found.how_found,
                progress: found.record.progress.clone(),
                homework: found.record.homework.clone(),
            }),
            None,
        ),
        PreviousOutcome::NotFound { searched_range } => (None, Some(searched_range)),
        PreviousOutcome::Failed(e) => return Err(e),
    };
    let mut checks = Vec::new();
    if let Some(prev) = &previous
        && let Some(start) = homework_minor_is_start(&prev.progress, &prev.homework)
    {
        let book_label = requested_book.unwrap_or(&start.book_label);
        checks.push(Check {
            kind: "prestudy".into(),
            scope: start.minor.clone(),
            verdict: backend.prestudy_verdict(&student.name, &start.minor),
            correct: None,
            total: None,
        });
        let textbook_student = TextbookStudent::new(&student.key, &student.name)
            .map_err(|_| BackendError::source_unavailable())?;
        let scope = backend.book_scope(student, book_label)?;
        for activity in ["필수예제", "유형다지기"] {
            let verdict = match &scope {
                Some(scope) => {
                    backend.section_verdict(&textbook_student, scope, &start.minor, activity)
                }
                None => crate::backend::SectionCheck {
                    scope: format!("{} {activity}", start.minor),
                    verdict: render::Verdict::Unknown,
                    correct: None,
                    total: None,
                },
            };
            checks.push(Check {
                kind: "section".into(),
                scope: verdict.scope,
                verdict: verdict.verdict,
                correct: verdict.correct,
                total: verdict.total,
            });
        }
    }
    Ok(StudentLesson {
        student: student.name.clone(),
        previous_lesson: previous,
        searched_range,
        checks,
        carry_over: vec![],
        excluded: vec![],
    })
}

pub fn prepare(
    backend: &mut SourceBackend,
    facts: &dyn TeacherFacts,
    req: &PrepareRequest,
    today: NaiveDate,
) -> Envelope {
    let lesson_date = match parse_request_date(req.date.as_deref(), today) {
        Ok(date) => date,
        Err(ask) => {
            return Envelope::ask(
                ask.question_ko.clone(),
                LessonData {
                    lesson_date: today.format("%Y-%m-%d").to_string(),
                    ..Default::default()
                },
                ask,
                Coverage::default(),
            );
        }
    };
    let mut data = LessonData {
        class_name: None,
        lesson_date: lesson_date.format("%Y-%m-%d").to_string(),
        students: vec![],
    };
    let cov = coverage(lesson_date);

    let names: Vec<String> = if let Some(student) = &req.student {
        vec![student.clone()]
    } else if let Some(class_name) = &req.class_name {
        data.class_name = Some(class_name.clone());
        match facts.class(class_name) {
            Some(class) => class.roster.clone(),
            None => {
                return Envelope::ask(
                    "어느 반인가요?".into(),
                    data,
                    Ask {
                        field: "class_name".into(),
                        question_ko: "어느 반인가요?".into(),
                        options: vec![],
                    },
                    cov,
                );
            }
        }
    } else {
        return Envelope::ask(
            "누구의 체크리스트인가요?".into(),
            data,
            Ask {
                field: "student".into(),
                question_ko: "누구의 체크리스트인가요?".into(),
                options: vec![],
            },
            cov,
        );
    };

    for name in &names {
        let student = match backend.resolve_student(name) {
            Ok(student) => student,
            Err(BackendError::Ask(ask)) => {
                return Envelope::ask(ask.question_ko.clone(), data, ask, cov);
            }
            Err(BackendError::Stop(stop)) => {
                let display = stop.message_ko.clone();
                return Envelope::stop(display, data, stop, cov);
            }
        };
        match prepare_student(backend, facts, &student, lesson_date, req.book.as_deref()) {
            Ok(entry) => data.students.push(entry),
            Err(BackendError::Ask(ask)) => {
                return Envelope::ask(ask.question_ko.clone(), data, ask, cov);
            }
            Err(BackendError::Stop(stop)) => {
                let display = stop.message_ko.clone();
                return Envelope::stop(display, data, stop, cov);
            }
        }
    }
    let display = render::lesson_prepare(&data);
    let mut cov = cov;
    if data.students.iter().any(|s| s.previous_lesson.is_none()) {
        cov.unknowns.push("previous_lesson_not_found".into());
    }
    // Keep the status type referenced for future held results.
    let _ = Status::Ok;
    Envelope::ok(display, data, cov)
}
