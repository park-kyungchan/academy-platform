//! The single code-to-Korean verdict table, the lesson renderer and the
//! reusable display lint (DESIGN-P8-v2 part 1).
use crate::{
    display_labels as l,
    envelope::Coverage,
    facts::TeacherFactReport,
    lesson::{LessonData, StudentLesson, scan_line},
};
use serde::{Serialize, Serializer};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Found,
    NotObserved,
    NotObservedInEitherSource,
    NotInReadRange,
    NotYetVisible,
    Excluded,
    Eligible,
    NotEligible,
    Unknown,
    Unjudged,
    Waiting,
    Completed,
    Persisted,
    NotPersisted,
    Conflict,
    Unresolved,
    Confirmed,
    Deferred,
}

impl Verdict {
    pub fn code(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::NotObserved => "not_observed",
            Self::NotObservedInEitherSource => "not_observed_in_either_source",
            Self::NotInReadRange => "not_in_read_range",
            Self::NotYetVisible => "not_yet_visible",
            Self::Excluded => "excluded",
            Self::Eligible => "eligible",
            Self::NotEligible => "not_eligible",
            Self::Unknown => "unknown",
            Self::Unjudged => "unjudged",
            Self::Waiting => "waiting",
            Self::Completed => "completed",
            Self::Persisted => "persisted",
            Self::NotPersisted => "not_persisted",
            Self::Conflict => "conflict",
            Self::Unresolved => "unresolved",
            Self::Confirmed => "confirmed",
            Self::Deferred => "deferred",
        }
    }
    /// The one code-to-Korean table; codes appear only in `data`.
    pub fn label_ko(self) -> &'static str {
        crate::display_labels::verdict(self.code())
    }
    pub const ALL: [Verdict; 18] = [
        Self::Found,
        Self::NotObserved,
        Self::NotObservedInEitherSource,
        Self::NotInReadRange,
        Self::NotYetVisible,
        Self::Excluded,
        Self::Eligible,
        Self::NotEligible,
        Self::Unknown,
        Self::Unjudged,
        Self::Waiting,
        Self::Completed,
        Self::Persisted,
        Self::NotPersisted,
        Self::Conflict,
        Self::Unresolved,
        Self::Confirmed,
        Self::Deferred,
    ];
}

impl Serialize for Verdict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

impl<'de> serde::Deserialize<'de> for Verdict {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        Self::ALL
            .into_iter()
            .find(|v| v.code() == code)
            .ok_or_else(|| serde::de::Error::custom("unknown verdict code"))
    }
}

fn row(fields: [&str; 6]) -> String {
    fields.join(" | ")
}
fn evidence(kind: &str, day: &str) -> String {
    format!("{kind} {}", l::date(day))
}
fn confirmed(day: &str) -> String {
    format!("{}({})", l::TEACHER, l::date(day))
}
fn report_state(report: Option<&TeacherFactReport>) -> String {
    match report {
        Some(r)
            if !r.reported_on.is_empty()
                && (r.completed == Some(true)
                    || r.status == "unfinished"
                    || r.status == "scheduled") =>
        {
            confirmed(&r.reported_on)
        }
        Some(r) if r.status == "deferred" => l::DEFERRED.into(),
        Some(r) if r.status == "not_applicable" => l::NOT_APPLICABLE.into(),
        _ => l::PHYSICAL.into(),
    }
}
fn reports<'a>(data: &'a LessonData, student: &str) -> Vec<&'a TeacherFactReport> {
    data.teacher_fact_reports
        .iter()
        .filter(|r| r.student == student)
        .collect()
}
fn stage_label(code: &str) -> &str {
    match code {
        "solving" => l::SOLVING,
        "grading" => l::GRADING,
        "correction" => l::CORRECTION,
        _ => code,
    }
}
fn student_rows(data: &LessonData, student: &StudentLesson) -> Vec<String> {
    let mut out = Vec::new();
    let who = &student.student;
    let Some(prev) = &student.previous_lesson else {
        return out;
    };
    let Some(homework) = scan_line(&prev.homework) else {
        return out;
    };
    let facts = reports(data, who);
    let assigned = facts.iter().find(|r| {
        r.kind == "assigned_stages"
            && r.assigned_on == prev.date
            && r.book == homework.book_label
            && r.unit == homework.minor
            && r.activity == homework.activity
    });
    let scope = format!(
        "{} {} {}",
        homework.book_label, homework.minor, homework.activity
    );
    if let Some(assignment) = assigned {
        for stage in &assignment.stages {
            let check = facts
                .iter()
                .find(|r| {
                    r.kind == "homework_stage_check"
                        && r.assigned_on == assignment.assigned_on
                        && r.stage == *stage
                })
                .copied();
            let state = report_state(check);
            let source = check.map_or_else(
                || evidence(l::ASSIGNMENT, &assignment.assigned_on),
                |r| evidence(l::FACTS, &r.reported_on),
            );
            let next = if check.is_some_and(|r| r.completed == Some(true)) {
                l::NOTE_NEXT.to_string()
            } else {
                l::stage_next(stage_label(stage))
            };
            out.push(row([
                who,
                &scope,
                &l::assigned(stage_label(stage)),
                &state,
                &source,
                &next,
            ]));
        }
    }
    for carry in &student.carry_over {
        let Some(item) = scan_line(&carry.homework_line) else {
            continue;
        };
        let assignment = facts.iter().find(|r| {
            r.kind == "assigned_stages"
                && r.assigned_on == carry.assigned_on
                && r.book == item.book_label
                && r.unit == item.minor
        });
        let labels = assignment.map(|r| r.item_labels.as_slice()).unwrap_or(&[]);
        let distinct = !labels.is_empty()
            && labels
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == labels.len();
        let count = if distinct {
            l::count_labels(labels.len())
        } else {
            l::anonymous_assignment_count(&carry.homework_line)
        };
        let scope = l::carry_scope(
            &format!("{} {} {}", item.book_label, item.minor, item.activity),
            &count,
            &carry.assigned_on,
        );
        let reported = facts
            .iter()
            .find(|r| r.kind == "not_brought" && r.assigned_on == carry.assigned_on)
            .map(|r| r.reported_on.as_str());
        let source = reported.map_or_else(
            || evidence(l::ASSIGNMENT, &carry.assigned_on),
            |d| evidence(l::TEACHER, d),
        );
        out.push(row([
            who,
            &scope,
            l::CARRY_CHECK,
            l::UNJUDGED,
            &source,
            l::CARRY_NEXT,
        ]));
    }
    if let Some(video) = student.checks.iter().find(|c| c.kind == "prestudy") {
        let scope = format!("{} {} {}", homework.book_label, homework.minor, l::VIDEO);
        let (state, source, next) = match video.verdict {
            Verdict::Waiting => (l::WAITING, l::VIDEO_WAIT, l::VIDEO_NEXT),
            Verdict::Completed => (l::SERVER, l::VIDEO_DONE, l::NOTE_NEXT),
            Verdict::NotObserved => (l::MISSING_VIDEO, l::VIDEO_BOTH, l::VIDEO_NEXT),
            other => (other.label_ko(), l::VIDEO_BOTH, l::VIDEO_NEXT),
        };
        let source = video
            .observed_on
            .as_deref()
            .map_or(source.to_string(), |date| evidence(source, date));
        out.push(row([who, &scope, l::VIDEO_CHECK, state, &source, next]));
        if homework.book_label.starts_with(l::GAUSS) {
            let scope = format!(
                "{} {} {}",
                homework.book_label,
                homework.minor,
                l::PRESTUDY_NOTE
            );
            let check = facts
                .iter()
                .find(|r| {
                    r.kind == "prestudy_notebook_check"
                        && r.book == homework.book_label
                        && r.unit == homework.minor
                })
                .copied();
            let state = report_state(check);
            let source = check.map_or_else(
                || evidence(l::ASSIGNMENT, &prev.date),
                |r| evidence(l::FACTS, &r.reported_on),
            );
            out.push(row([
                who,
                &scope,
                l::PRESTUDY_CHECK,
                &state,
                &source,
                l::CONCEPT_NEXT,
            ]));
        }
    }
    for check in student.checks.iter().filter(|c| c.kind == "section") {
        let scope = format!("{} {}", homework.book_label, check.scope);
        let state = if check.verdict == Verdict::NotObserved {
            l::MISSING_TITLE.to_string()
        } else if let (Some(c), Some(t)) = (check.correct, check.total) {
            format!("{}; {}", l::SERVER, l::anonymous_grade(c, t))
        } else {
            check.verdict.label_ko().into()
        };
        out.push(row([
            who,
            &scope,
            l::SECTION_CHECK,
            &state,
            l::SECTION_TITLES,
            l::SECTION_NEXT,
        ]));
    }
    // Keep each unresolved assigned notebook once per book; newer assignment
    // dates never rewrite older physical-inspection evidence.
    let mut books: Vec<String> = Vec::new();
    for r in facts
        .iter()
        .filter(|r| r.kind == "assigned_stages" && r.assigned_on <= prev.date)
    {
        if !books.contains(&r.book) {
            books.push(r.book.clone());
        }
    }
    for book in books {
        let mut assignments: Vec<_> = facts
            .iter()
            .filter(|r| r.kind == "assigned_stages" && r.book == book && r.assigned_on <= prev.date)
            .copied()
            .collect();
        assignments.sort_by(|a, b| a.assigned_on.cmp(&b.assigned_on));
        let mut units = Vec::new();
        let mut dates = Vec::new();
        for assignment in assignments {
            if !units.contains(&assignment.unit) {
                units.push(assignment.unit.clone());
            }
            if !dates.contains(&assignment.assigned_on) {
                dates.push(assignment.assigned_on.clone());
            }
        }
        let scope = format!("{book} {} {}", units.join("·"), l::SOLUTION_NOTE);
        let check = facts
            .iter()
            .find(|r| r.kind == "solution_notebook_check" && r.book == book)
            .copied();
        let state = report_state(check);
        let source = check.map_or_else(
            || {
                format!(
                    "{} {}",
                    l::ASSIGNMENT,
                    dates
                        .iter()
                        .map(|d| l::date(d))
                        .collect::<Vec<_>>()
                        .join("·")
                )
            },
            |r| evidence(l::FACTS, &r.reported_on),
        );
        out.push(row([
            who,
            &scope,
            l::SOLUTION_CHECK,
            &state,
            &source,
            l::SOLUTION_NEXT,
        ]));
    }
    let academy: Vec<_> = facts
        .iter()
        .filter(|r| {
            r.lesson_date == prev.date
                && matches!(
                    r.kind.as_str(),
                    "in_class_follow_up" | "in_academy_correction"
                )
        })
        .copied()
        .collect();
    if !academy.is_empty() {
        out.push(l::academy(who));
    }
    for r in academy {
        let state = report_state(Some(r));
        let source = evidence(l::FACTS, &r.reported_on);
        if r.kind == "in_class_follow_up" {
            let scope = format!("{} {} {}", r.book, r.unit, l::FOLLOW_UP);
            let action = format!("{}({})", r.action, l::PENDING);
            out.push(row([who, &scope, &action, &state, &source, l::FOLLOW_NEXT]));
        } else {
            let scope = format!(
                "{} {} {}",
                l::date(&r.at),
                time(&r.at),
                l::ACADEMY_CORRECTION
            );
            out.push(row([
                who,
                &scope,
                l::CORRECTION_CHECK,
                &state,
                &source,
                l::CORRECTION_NEXT,
            ]));
        }
    }
    out
}
fn time(at: &str) -> &str {
    at.get(11..16).unwrap_or("")
}
pub fn urgency(at: &str, today: &str) -> &'static str {
    let date = at.get(..10).unwrap_or(at);
    if date < today {
        l::DUE_PAST
    } else if date == today {
        l::TODAY
    } else {
        l::DUE_SOON
    }
}
fn stamp(at: &str) -> String {
    let local = chrono::DateTime::parse_from_rfc3339(at).ok().map(|d| {
        d.with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).unwrap())
            .to_rfc3339()
    });
    let at = local.as_deref().unwrap_or(at);
    format!("{} {}", l::date(at), time(at))
}
fn footer(data: &LessonData, coverage: &Coverage) -> Vec<String> {
    let class = data.class_name.as_deref();
    let names = data
        .students
        .iter()
        .map(|s| s.student.as_str())
        .collect::<Vec<_>>();
    let empty = names.is_empty();
    let no_previous = !empty && data.students.iter().all(|s| s.previous_lesson.is_none());
    let schedule = class.map_or_else(
        || {
            if no_previous {
                format!(
                    "{} {}",
                    data.students[0].class_name.as_deref().unwrap_or(""),
                    l::SCHEDULE_SUFFIX
                )
            } else {
                l::SCHEDULE.into()
            }
        },
        |c| format!("{c} {}", l::SCHEDULE_SUFFIX),
    );
    let fact_scope = if empty {
        format!("{}·{schedule}·{}", l::ALL_REMINDERS, l::EMPTY_ROSTER)
    } else if class.is_some() {
        format!(
            "{}·{schedule}·{}·{}",
            l::ALL_REMINDERS,
            l::ROSTER,
            l::STUDENT_FACTS
        )
    } else {
        format!("{}·{schedule}·{} {}", l::ALL_REMINDERS, names[0], l::FACT)
    };
    let mut groups = vec![("teacher_facts", format!("{}({fact_scope})", l::FACTS))];
    if !empty {
        if no_previous {
            groups.push((
                "registration",
                format!("{}({} {})", l::STUDENT_INFO, names.join("·"), l::BELONGS),
            ));
            let student = &data.students[0];
            let mut dates = student.searched_dates.clone();
            dates.sort();
            groups.push((
                "daily_record",
                format!(
                    "{}({} {}: {})",
                    l::RECORDS,
                    student.student,
                    l::search_dates(&dates),
                    l::NO_RECORD
                ),
            ));
        } else {
            let registered = if class.is_none() {
                let book = data.students[0]
                    .previous_lesson
                    .as_ref()
                    .and_then(|p| scan_line(&p.homework))
                    .map(|r| r.book_label)
                    .unwrap_or_default();
                format!("{}·{book}", names[0])
            } else {
                names.join("·")
            };
            groups.push(("registration", format!("{}({registered})", l::REGISTRATION)));
            let record_scope = if let Some(date) = &data.date_resolution {
                format!(
                    "{}: {}, {}: {}",
                    l::DATE_RESOLUTION,
                    l::date(date),
                    l::ASSIGNMENT_READ,
                    data.students[0]
                        .previous_lesson
                        .as_ref()
                        .map(|p| l::date(&p.date))
                        .unwrap_or_default()
                )
            } else if class.is_some() {
                let dates = data
                    .students
                    .iter()
                    .filter_map(|s| s.previous_lesson.as_ref().map(|p| p.date.clone()))
                    .collect::<Vec<_>>();
                if dates.windows(2).all(|p| p[0] == p[1]) {
                    format!(
                        "{} {}",
                        l::EACH_STUDENT,
                        dates.first().map(|s| l::date(s)).unwrap_or_default()
                    )
                } else {
                    data.students
                        .iter()
                        .filter_map(|s| {
                            s.previous_lesson
                                .as_ref()
                                .map(|p| format!("{} {}", s.student, l::date(&p.date)))
                        })
                        .collect::<Vec<_>>()
                        .join("·")
                }
            } else {
                format!(
                    "{}·{}",
                    names[0],
                    data.students[0]
                        .previous_lesson
                        .as_ref()
                        .map(|p| l::date(&p.date))
                        .unwrap_or_default()
                )
            };
            groups.push(("daily_record", format!("{}({record_scope})", l::RECORDS)));
            let scopes = data
                .students
                .iter()
                .filter_map(|s| {
                    s.checks
                        .iter()
                        .find(|c| c.kind == "prestudy")
                        .map(|c| format!("{} {}", s.student, c.scope))
                })
                .collect::<Vec<_>>()
                .join("·");
            if !scopes.is_empty() {
                let lists = if class.is_some() {
                    l::BOTH_LISTS
                } else {
                    l::BOTH_LISTS_SINGLE
                };
                groups.push(("prestudy", format!("{}({scopes}·{lists})", l::VIDEO)));
                let grade_state = if data.students.iter().any(|s| {
                    s.checks
                        .iter()
                        .any(|c| c.kind == "section" && c.correct.is_some())
                }) {
                    l::GRADES_READ
                } else {
                    l::NO_GRADES
                };
                groups.push((
                    "textbook_sections",
                    format!("{}({scopes}·{grade_state})", l::TITLES),
                ));
            }
        }
    }
    let mut lines = Vec::new();
    let mut line = l::footer(&stamp(&data.read_at));
    for (index, (code, group)) in groups.iter().enumerate() {
        let mut group = group.clone();
        if let Some(source) = coverage
            .sources
            .iter()
            .find(|s| s.name == *code && s.read_at != data.read_at)
        {
            group.push_str(&format!(" ({})", stamp(&source.read_at)));
        }
        let token = if index + 1 == groups.len() {
            format!("{group}.")
        } else {
            format!("{group},")
        };
        let separator = if line.is_empty() || line.ends_with(": ") {
            ""
        } else {
            " "
        };
        if line.chars().count() + separator.len() + token.chars().count() > 100
            && !line.ends_with(": ")
        {
            lines.push(line);
            line = token;
        } else {
            line.push_str(separator);
            line.push_str(&token);
        }
    }
    lines.push(line);
    lines
}

pub fn lesson_prepare(data: &LessonData, coverage: &Coverage) -> String {
    let who = data
        .class_name
        .as_deref()
        .or_else(|| data.students.first().map(|s| s.student.as_str()))
        .unwrap_or("");
    let mut out = vec![l::title(who, &data.lesson_date), l::PARTS[0].into()];
    if data.reminders.is_empty() {
        out.push(l::NONE.into());
    } else {
        out.push(l::REMINDER_HEADER.into());
        for r in &data.reminders {
            let who = r.student.as_deref().unwrap_or(l::NEW_STUDENT);
            let due = format!(
                "{} {} {}",
                urgency(&r.at, &data.today),
                l::date(&r.at),
                time(&r.at)
            );
            let action = format!("{}({})", r.action, l::PENDING);
            let state = if r.reported_on.is_empty() {
                l::UNAVAILABLE.into()
            } else {
                confirmed(&r.reported_on)
            };
            out.push(row([
                who,
                &due,
                &action,
                &state,
                &evidence(l::FACTS, &r.reported_on),
                l::REMINDER_NEXT,
            ]));
        }
    }
    out.push(l::PARTS[1].into());
    let rows = data
        .students
        .iter()
        .flat_map(|s| student_rows(data, s))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        out.push(l::NONE.into());
    } else {
        out.push(l::ACTION_HEADER.into());
        out.extend(rows);
    }
    out.push(l::PARTS[2].into());
    let mut unknown = Vec::new();
    if data.students.is_empty()
        && let Some(class) = &data.class_name
    {
        unknown.push(l::empty_roster(class));
    }
    for s in &data.students {
        if s.read_failed {
            unknown.push(l::unread_record(&s.student));
        } else if s.previous_lesson.is_none() {
            unknown.push(l::missing_previous(&s.student));
            unknown.push(l::missing_homework(&s.student));
        }
    }
    let missing = data
        .students
        .iter()
        .filter(|s| {
            s.checks
                .iter()
                .any(|c| c.kind == "section" && c.verdict == Verdict::NotObserved)
        })
        .filter_map(|s| {
            s.previous_lesson
                .as_ref()
                .and_then(|p| scan_line(&p.homework))
                .map(|h| (s.student.as_str(), h.minor))
        })
        .collect::<Vec<_>>();
    if missing.len() == 1 {
        unknown.push(l::unread_grades(
            &l::possessive(missing[0].0, &missing[0].1),
            false,
        ));
    } else if !missing.is_empty() {
        unknown.push(l::unread_grades(
            &missing
                .iter()
                .map(|(s, m)| format!("{s} {m}"))
                .collect::<Vec<_>>()
                .join("·"),
            true,
        ));
    }
    if unknown.is_empty() {
        out.push(l::NONE.into());
    } else {
        out.extend(unknown);
    }
    out.push(l::PARTS[3].into());
    let mut next = Vec::new();
    let reminder = data
        .reminders
        .iter()
        .filter(|r| r.student.is_some())
        .min_by_key(|r| {
            (
                if urgency(&r.at, &data.today) == l::DUE_PAST {
                    0
                } else if urgency(&r.at, &data.today) == l::TODAY {
                    1
                } else {
                    2
                },
                r.at.as_str(),
            )
        });
    if let Some(r) = reminder {
        next.push(l::reminder_next(
            r.student.as_deref().unwrap_or(l::NEW_STUDENT),
            urgency(&r.at, &data.today),
            time(&r.at),
            &r.action,
        ));
    }
    if data.students.is_empty() {
        if let Some(class) = &data.class_name {
            next.push(l::roster_next(class));
        }
    } else if let Some(s) = data.students.iter().find(|s| {
        s.checks
            .iter()
            .any(|c| c.kind == "prestudy" && c.verdict == Verdict::Waiting)
    }) {
        let unit = &s
            .checks
            .iter()
            .find(|c| c.kind == "prestudy")
            .unwrap()
            .scope;
        next.push(l::video_next(&s.student, unit));
    } else if let Some(s) = data.students.first() {
        if let Some(h) = s
            .previous_lesson
            .as_ref()
            .and_then(|p| scan_line(&p.homework))
        {
            next.push(l::notebook_next(&s.student, &h.minor));
        } else {
            next.push(l::previous_next(&s.student));
        }
    }
    if next.is_empty() {
        out.push(l::NONE.into());
    } else {
        out.extend(next.into_iter().take(3));
    }
    out.extend(footer(data, coverage));
    out.push(l::CLOSING.into());
    out.join("\n")
}

fn latin_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[A-Za-z_]{4,}").expect("constant"))
}
pub fn display_lint(display: &str) -> Result<(), String> {
    if latin_re().is_match(display) {
        return Err("non_korean_display_token".into());
    }
    let lines = display.lines().collect::<Vec<_>>();
    let checklist = lines.iter().any(|s| {
        l::PARTS.contains(s)
            || s.contains('|')
            || s.starts_with(l::READ_SOURCES)
            || *s == l::CLOSING
    });
    if !checklist {
        return if display.trim().is_empty() {
            Err("empty_display".into())
        } else {
            Ok(())
        };
    }
    if lines.last() != Some(&l::CLOSING) {
        return Err("closing_missing".into());
    }
    let positions = l::PARTS
        .iter()
        .map(|heading| lines.iter().position(|line| line == heading))
        .collect::<Option<Vec<_>>>()
        .ok_or("heading_missing")?;
    if positions.windows(2).any(|p| p[0] >= p[1])
        || l::PARTS
            .iter()
            .any(|h| lines.iter().filter(|line| *line == h).count() != 1)
    {
        return Err("heading_order".into());
    }
    if lines
        .iter()
        .any(|line| line.contains('|') && line.split(" | ").count() != 6)
    {
        return Err("row_field_count".into());
    }
    Ok(())
}
