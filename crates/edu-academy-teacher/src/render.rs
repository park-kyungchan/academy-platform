//! The single code-to-Korean verdict table, the lesson renderer and the
//! reusable display lint (DESIGN-P8-v2 part 1).
use crate::{
    display_labels as l,
    envelope::Coverage,
    facts::TeacherFactReport,
    lesson::{LessonData, StudentLesson, homework_items, scan_line},
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

/// Reversible display-cell encoding: escape backslashes first, then controls and pipes.
pub fn cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\u007C")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}
fn row(fields: [&str; 6]) -> String {
    fields.map(cell).join(" | ")
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
fn reports<'a>(data: &'a LessonData, student: &StudentLesson) -> Vec<&'a TeacherFactReport> {
    data.teacher_fact_reports
        .iter()
        .filter(|r| r.student_key.as_deref() == Some(&student.student_key))
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
// A partially named fact is usable only if exactly one assignment matches it.
fn applies(
    check: &TeacherFactReport,
    assignment: &TeacherFactReport,
    assignments: &[&TeacherFactReport],
) -> bool {
    let matches = |a: &&TeacherFactReport| {
        (check.assigned_on.is_empty() || check.assigned_on == a.assigned_on)
            && (check.book.is_empty() || check.book == a.book)
            && (check.unit.is_empty() || check.unit == a.unit)
            && (check.activity.is_empty() || check.activity == a.activity)
            && (check.stage.is_empty() || a.stages.contains(&check.stage))
    };
    let candidates: Vec<_> = assignments.iter().copied().filter(matches).collect();
    candidates.len() == 1
        && std::ptr::eq(candidates[0], assignment)
        && !check.reported_on.is_empty()
        && check.reported_on >= assignment.assigned_on
}
struct NotebookGroup {
    book: String,
    state: String,
    reported: Option<String>,
    units: Vec<String>,
    dates: Vec<String>,
}
fn student_rows(data: &LessonData, student: &StudentLesson) -> Vec<String> {
    let mut out = Vec::new();
    let who = &student.student;
    let facts = reports(data, student);
    let assignments: Vec<_> = facts
        .iter()
        .copied()
        .filter(|r| r.kind == "assigned_stages")
        .collect();
    let cutoff = student
        .previous_lesson
        .as_ref()
        .map_or(data.lesson_date.as_str(), |p| p.date.as_str());
    let homework = student
        .previous_lesson
        .as_ref()
        .map(|p| homework_items(&p.homework))
        .unwrap_or_default();
    for item in &homework {
        let assignment = assignments.iter().find(|a| {
            a.assigned_on == cutoff
                && a.book == item.book_label
                && a.unit == item.minor
                && a.activity == item.activity
        });
        if let Some(assignment) = assignment {
            let scope = format!("{} {} {}", item.book_label, item.minor, item.activity);
            for stage in &assignment.stages {
                let check = facts
                    .iter()
                    .copied()
                    .filter(|r| {
                        r.kind == "homework_stage_check"
                            && r.stage == *stage
                            && applies(r, assignment, &assignments)
                    })
                    .max_by_key(|r| &r.reported_on);
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
    }
    for carry in &student.carry_over {
        for item in homework_items(&carry.homework_line) {
            let assignment = assignments.iter().find(|a| {
                a.assigned_on == carry.assigned_on
                    && a.book == item.book_label
                    && a.unit == item.minor
                    && a.activity == item.activity
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
            let reported = assignment.and_then(|a| {
                facts
                    .iter()
                    .copied()
                    .find(|r| r.kind == "not_brought" && applies(r, a, &assignments))
            });
            let source = reported.map_or_else(
                || evidence(l::ASSIGNMENT, &carry.assigned_on),
                |r| evidence(l::TEACHER, &r.reported_on),
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
    }
    for video in student.checks.iter().filter(|c| c.kind == "prestudy") {
        let scope = format!("{} {} {}", video.book, video.scope, l::VIDEO);
        let (state, source, next) = match video.verdict {
            Verdict::Waiting => (l::WAITING, l::VIDEO_WAIT, l::VIDEO_NEXT),
            Verdict::Completed => (l::SERVER, l::VIDEO_DONE, l::NOTE_NEXT),
            Verdict::NotObserved => (l::MISSING_VIDEO, l::VIDEO_BOTH, l::VIDEO_NEXT),
            other => (other.label_ko(), l::VIDEO_BOTH, l::VIDEO_NEXT),
        };
        let source = video
            .observed_on
            .as_deref()
            .map_or(source.to_string(), |d| evidence(source, d));
        out.push(row([who, &scope, l::VIDEO_CHECK, state, &source, next]));
        if video.book.starts_with(l::GAUSS) {
            let assignment = assignments
                .iter()
                .find(|a| a.book == video.book && a.unit == video.scope && a.assigned_on == cutoff);
            let check = assignment.and_then(|a| {
                facts
                    .iter()
                    .copied()
                    .filter(|r| r.kind == "prestudy_notebook_check" && applies(r, a, &assignments))
                    .max_by_key(|r| &r.reported_on)
            });
            let scope = format!("{} {} {}", video.book, video.scope, l::PRESTUDY_NOTE);
            let source = check.map_or_else(
                || evidence(l::ASSIGNMENT, cutoff),
                |r| evidence(l::FACTS, &r.reported_on),
            );
            out.push(row([
                who,
                &scope,
                l::PRESTUDY_CHECK,
                &report_state(check),
                &source,
                l::CONCEPT_NEXT,
            ]));
        }
        // Keep one complete minor block together in record order.
        for check in student.checks.iter().filter(|c| {
            c.kind == "section"
                && c.book == video.book
                && c.scope.starts_with(&format!("{} ", video.scope))
        }) {
            let scope = format!("{} {}", check.book, check.scope);
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
    }
    // Aggregate unknown assignments per book, but never combine inspected and uninspected units.
    let mut ordered = assignments
        .iter()
        .copied()
        .filter(|a| a.assigned_on.as_str() <= cutoff)
        .collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.assigned_on.cmp(&b.assigned_on));
    let mut groups: Vec<NotebookGroup> = Vec::new();
    for a in ordered {
        let check = facts
            .iter()
            .copied()
            .filter(|r| r.kind == "solution_notebook_check" && applies(r, a, &assignments))
            .max_by_key(|r| &r.reported_on);
        let state = report_state(check);
        let reported = check.map(|r| r.reported_on.clone());
        if let Some(g) = groups
            .iter_mut()
            .find(|g| g.book == a.book && g.state == state && g.reported == reported)
        {
            if !g.units.contains(&a.unit) {
                g.units.push(a.unit.clone());
            }
            if !g.dates.contains(&a.assigned_on) {
                g.dates.push(a.assigned_on.clone());
            }
        } else {
            groups.push(NotebookGroup {
                book: a.book.clone(),
                state,
                reported,
                units: vec![a.unit.clone()],
                dates: vec![a.assigned_on.clone()],
            });
        }
    }
    for NotebookGroup {
        book,
        state,
        reported,
        units,
        dates,
    } in groups
    {
        let scope = format!("{book} {} {}", units.join("·"), l::SOLUTION_NOTE);
        let source = reported.map_or_else(
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
            |d| evidence(l::FACTS, &d),
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
        .copied()
        .filter(|r| {
            matches!(
                r.kind.as_str(),
                "in_class_follow_up" | "in_academy_correction"
            ) && !matches!(r.status.as_str(), "closed" | "completed")
                && r.completed != Some(true)
                && if student.previous_lesson.is_some() {
                    r.lesson_date == cutoff
                } else {
                    !r.lesson_date.is_empty() && r.lesson_date.as_str() <= cutoff
                }
        })
        .collect();
    if !academy.is_empty() {
        out.push(l::academy(&cell(who)));
    }
    for r in academy {
        let state = report_state(Some(r));
        let source = evidence(l::FACTS, &r.reported_on);
        if r.kind == "in_class_follow_up" {
            let scope = format!("{} {} {}", r.book, r.unit, l::FOLLOW_UP);
            let action = format!("{}({})", r.action, l::PENDING);
            out.push(row([who, &scope, &action, &state, &source, l::FOLLOW_NEXT]));
        } else {
            let at = local_time(&r.at);
            let scope = format!("{} {} {}", l::date(&at), time(&at), l::ACADEMY_CORRECTION);
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

fn local_time(at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(at)
        .map(|d| {
            d.with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).unwrap())
                .to_rfc3339()
        })
        .unwrap_or_else(|_| at.to_string())
}
fn time(at: &str) -> &str {
    at.get(11..16).unwrap_or("")
}
pub fn urgency(at: &str, today: &str) -> &'static str {
    let local = local_time(at);
    let date = local.get(..10).unwrap_or(&local);
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
        format!(
            "{}·{schedule}·{}",
            l::ALL_REMINDERS,
            if data.display_omitted {
                l::ROSTER
            } else {
                l::EMPTY_ROSTER
            }
        )
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
                coverage
                    .sources
                    .iter()
                    .filter(|r| r.name == "registration")
                    .map(|r| r.scope.replacen(' ', "·", 1))
                    .collect::<Vec<_>>()
                    .join("·")
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
                let unread = data
                    .students
                    .iter()
                    .flat_map(|s| {
                        s.checks
                            .iter()
                            .filter(|c| {
                                c.kind == "section"
                                    && (c.verdict == Verdict::NotObserved
                                        || c.verdict == Verdict::Unknown)
                            })
                            .map(|c| format!("{} {} {}", s.student, c.book, c.scope))
                    })
                    .collect::<Vec<_>>();
                let detail = if grade_state == l::GRADES_READ && !unread.is_empty() {
                    format!("·{}({})", l::NO_GRADES, unread.join("·"))
                } else {
                    String::new()
                };
                groups.push((
                    "textbook_sections",
                    format!("{}({scopes}·{grade_state}{detail})", l::TITLES),
                ));
            }
        }
    }
    // Legacy aggregation is valid only for complete, equally timed scopes.
    // Otherwise display each successful read separately with its own timestamp.
    let mut actual = Vec::new();
    for (code, group) in groups {
        let reads: Vec<_> = coverage
            .sources
            .iter()
            .filter(|r| {
                r.name == code
                    || (code == "registration" && no_previous && r.name == "student_info")
            })
            .collect();
        if reads.is_empty() {
            continue;
        }
        let same_time = reads.iter().all(|r| r.read_at == reads[0].read_at);
        let scope_complete = match code {
            "prestudy" => data.students.iter().all(|s| {
                s.checks.iter().filter(|c| c.kind == "prestudy").count() <= 1
                    && s.checks
                        .iter()
                        .filter(|c| c.kind == "prestudy")
                        .all(|c| c.verdict != Verdict::Unknown)
            }),
            "textbook_sections" => data.students.iter().all(|s| {
                s.checks.iter().filter(|c| c.kind == "prestudy").count() <= 1
                    && s.checks
                        .iter()
                        .filter(|c| c.kind == "section")
                        .all(|c| c.title_read)
            }),
            "daily_record" => data
                .students
                .iter()
                .all(|s| !s.read_failed && reads.iter().any(|r| r.scope.starts_with(&s.student))),
            "registration" => {
                no_previous
                    || data
                        .students
                        .iter()
                        .all(|s| reads.iter().any(|r| r.scope.starts_with(&s.student)))
            }
            _ => true,
        };
        if same_time && scope_complete {
            actual.push((code.to_string(), group, reads[0].read_at.clone()));
        } else {
            let label = match code {
                "prestudy" => l::VIDEO,
                "textbook_sections" => l::TITLES,
                "registration" => l::REGISTRATION,
                _ => l::RECORDS,
            };
            for read in reads {
                actual.push((
                    code.into(),
                    format!("{}({})", label, read.scope),
                    read.read_at.clone(),
                ));
            }
        }
    }
    let groups = actual;
    let mut lines = Vec::new();
    let mut line = l::footer(&stamp(&data.read_at));
    for (index, (_code, group, read_at)) in groups.iter().enumerate() {
        let mut group = cell(group);
        if read_at != &data.read_at {
            group.push_str(&format!(" ({})", stamp(read_at)));
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
        .display_subject
        .as_deref()
        .or(data.class_name.as_deref())
        .or_else(|| data.students.first().map(|s| s.student.as_str()))
        .unwrap_or("");
    let mut out = vec![l::title(&cell(who), &data.lesson_date), l::PARTS[0].into()];
    if data.reminders.is_empty() {
        out.push(l::NONE.into());
    } else {
        out.push(l::REMINDER_HEADER.into());
        for r in &data.reminders {
            let who = r.student.as_deref().unwrap_or(l::NEW_STUDENT);
            let local = local_time(&r.at);
            let due = format!(
                "{} {} {}",
                urgency(&r.at, &data.today),
                l::date(&local),
                time(&local)
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
        && !data.display_omitted
        && let Some(class) = &data.class_name
    {
        unknown.push(l::empty_roster(&cell(class)));
    }
    for s in &data.students {
        if s.read_failed {
            unknown.push(l::unread_record(&cell(&s.student)));
        } else if s.previous_lesson.is_none() {
            unknown.push(l::missing_previous(&cell(&s.student)));
            unknown.push(l::missing_homework(&cell(&s.student)));
        } else if let Some(prev) = &s.previous_lesson {
            if prev.homework.trim().trim_start_matches("- ") == l::NONE {
                unknown.push(l::no_homework(&cell(&s.student)));
            } else if homework_items(&prev.homework).is_empty() {
                unknown.push(l::unread_homework(&cell(&s.student)));
            }
        }
        for c in &s.checks {
            if c.verdict == Verdict::Unknown {
                unknown.push(l::unread_scope(&cell(&format!(
                    "{} {} {} {}",
                    s.student,
                    c.book,
                    c.scope,
                    if c.kind == "prestudy" {
                        l::VIDEO
                    } else if c.title_read {
                        l::GRADES_READ
                    } else {
                        l::TITLES
                    }
                ))));
            }
        }
    }
    for issue in &coverage.unknowns {
        if let Some(scope) = issue.strip_prefix("registration_read_failed:") {
            unknown.push(l::unread_scope(&cell(&format!(
                "{} {}",
                l::REGISTRATION,
                scope
            ))));
        }
    }
    let missing = data
        .students
        .iter()
        .flat_map(|s| {
            let missing: Vec<_> = s
                .checks
                .iter()
                .filter(|c| c.kind == "section" && c.verdict == Verdict::NotObserved)
                .collect();
            missing
                .iter()
                .filter_map(|c| {
                    let minor = c.scope.split_whitespace().next().unwrap_or("");
                    let all_missing = s
                        .checks
                        .iter()
                        .filter(|v| {
                            v.kind == "section"
                                && v.book == c.book
                                && v.scope.starts_with(&format!("{minor} "))
                        })
                        .all(|v| v.verdict == Verdict::NotObserved);
                    if all_missing && c.scope != format!("{minor} {}", l::REQUIRED) {
                        return None;
                    }
                    Some((
                        s.student.as_str(),
                        if all_missing {
                            minor.into()
                        } else {
                            c.scope.clone()
                        },
                    ))
                })
                .collect::<Vec<_>>()
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
                local_time(&r.at),
            )
        });
    if let Some(r) = reminder {
        next.push(l::reminder_next(
            &cell(r.student.as_deref().unwrap_or(l::NEW_STUDENT)),
            urgency(&r.at, &data.today),
            time(&local_time(&r.at)),
            &cell(&r.action),
        ));
    }
    if data.students.is_empty() {
        if let Some(class) = &data.class_name
            && !data.display_omitted
        {
            next.push(l::roster_next(&cell(class)));
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
        next.push(l::video_next(&cell(&s.student), &cell(unit)));
    } else if let Some(s) = data.students.first() {
        if let Some(h) = s
            .previous_lesson
            .as_ref()
            .and_then(|p| scan_line(&p.homework))
        {
            next.push(l::notebook_next(&cell(&s.student), &cell(&h.minor)));
        } else {
            next.push(l::previous_next(&cell(&s.student)));
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

pub fn bounded_lesson_prepare(data: &LessonData, coverage: &mut Coverage) -> String {
    let mut visible = data.clone();
    visible.display_subject = data
        .class_name
        .clone()
        .or_else(|| data.students.first().map(|s| s.student.clone()));
    let mut omitted: Option<String> = None;
    let mut omitted_reminder: Option<String> = None;
    loop {
        let mut display = lesson_prepare(&visible, coverage);
        if let Some(first) = &omitted {
            display = display.replacen(
                &format!("\n{}\n", l::PARTS[3]),
                &format!("\n{}\n{}\n", l::omitted(&cell(first)), l::PARTS[3]),
                1,
            );
        }
        if let Some(first) = &omitted_reminder {
            display = display.replacen(
                &format!("\n{}\n", l::PARTS[3]),
                &format!(
                    "\n{}\n{}\n",
                    l::omitted_reminders(&cell(first)),
                    l::PARTS[3]
                ),
                1,
            );
        }
        if display.len() <= 6000 && display.lines().count() <= 60 {
            if let Some(first) = omitted {
                coverage
                    .unknowns
                    .push(format!("display_truncated:{first}:60_lines_or_6000_bytes"));
            }
            if let Some(first) = omitted_reminder {
                coverage.unknowns.push(format!(
                    "reminders_truncated:{first}:60_lines_or_6000_bytes"
                ));
            }
            return display;
        }
        if let Some(student) = visible.students.pop() {
            visible.display_omitted = true;
            omitted = Some(student.student);
            continue;
        }
        if let Some(reminder) = visible.reminders.pop() {
            omitted_reminder = Some(reminder.student.unwrap_or_else(|| l::NEW_STUDENT.into()));
            continue;
        }
        return display; // prepare's lint converts even an oversized fixed body into a short stop.
    }
}

fn latin_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[A-Za-z_]{4,}").expect("constant"))
}
pub fn display_lint(display: &str) -> Result<(), String> {
    if display.len() > 6000 || display.lines().count() > 60 {
        return Err("display_limit".into());
    }
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
