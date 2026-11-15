//! `edu lesson`: a lesson session's three reads from the read store (`day`, `record`,
//! `history`) and the capture of dates from the LMS through the family `dailyrecord`
//! (`refresh`, or a read of a date never captured). Line 1 of every answer is one JSON
//! object of fixed members; a refusal is `{"status":"<code>"}` with exit code 2. No name,
//! record text, cookie or credential is in line 1, in a refusal or in the run log.
#![forbid(unsafe_code)]
mod config;
mod engine;
mod lms;
use config::Config;
use edu_academy_core::workflow::Runtime;
use edu_academy_sources::{
    data::{Error, Result, valid_date},
    http::{Cancellation, Transport},
};
use serde_json::{Value, json};
use std::path::PathBuf;

pub const DEFAULT_CONFIG: &str = "/etc/education-platform/lesson.json";
/// `history` without `--since` starts this many days before today.
const HISTORY_DAYS: i64 = 56;
const HISTORY_LIMIT: usize = 24;

pub struct Outcome {
    pub stdout: String,
    pub code: i32,
}

/// One fixed code; an ambiguous name also carries how many students it names.
#[derive(Clone)]
pub(crate) struct Refusal {
    code: String,
    count: Option<usize>,
}
pub(crate) type Answer<T> = std::result::Result<T, Refusal>;
pub(crate) fn refuse(code: &str) -> Refusal {
    Refusal {
        code: code.into(),
        count: None,
    }
}
impl From<Error> for Refusal {
    /// The sources crate builds errors from fixed codes; anything else is not printed.
    fn from(error: Error) -> Self {
        let safe = (1..=64).contains(&error.message.len())
            && error
                .message
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        refuse(if safe {
            &error.message
        } else {
            "lesson_capture_failed"
        })
    }
}

enum Student {
    Id(String),
    Name(String),
}
enum Verb {
    Day {
        date: String,
        refresh: bool,
    },
    Record {
        date: String,
        student: Student,
        refresh: bool,
        exact: bool,
    },
    History {
        id: String,
        since: String,
    },
    Refresh {
        dates: Vec<String>,
    },
}

/// What the run did with the LMS, for line 1 and the run log.
#[derive(Default)]
pub(crate) struct Activity {
    pub login: bool,
    pub requests: usize,
    pub waited: u64,
}
impl Activity {
    fn members(&self, line: &mut Value) {
        line["login"] = json!(if self.login { "new" } else { "none" });
        line["requests"] = json!(self.requests);
        line["lock_wait_seconds"] = json!(self.waited);
    }
}

/// The calendar date in Asia/Seoul (UTC+9, no daylight saving time since 1988),
/// `offset_days` from the date of `epoch_ms`.
pub fn seoul_date(epoch_ms: u64, offset_days: i64) -> String {
    let days = ((epoch_ms / 1000 + 9 * 3600) / 86_400) as i64 + offset_days;
    // Days to the civil calendar (proleptic Gregorian), H. Hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn usage() -> Refusal {
    refuse("lesson_usage_invalid")
}
fn digits(s: &str, max: usize) -> bool {
    (1..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}
fn student(word: &str) -> Answer<Student> {
    if word.bytes().all(|b| b.is_ascii_digit()) {
        return if digits(word, 12) {
            Ok(Student::Id(word.into()))
        } else {
            Err(refuse("lesson_student_invalid"))
        };
    }
    if word.is_empty() || word.len() > 200 || word.chars().any(char::is_control) {
        return Err(refuse("lesson_student_invalid"));
    }
    Ok(Student::Name(word.into()))
}
/// The verb and its words, fully checked before the configuration, the engine or the LMS.
fn verb(words: &[String], today: &str, now_ms: u64) -> Answer<Verb> {
    let (name, rest) = words.split_first().ok_or_else(usage)?;
    let mut positional = vec![];
    let (mut refresh, mut exact, mut since) = (false, false, None);
    let mut rest = rest.iter();
    while let Some(word) = rest.next() {
        let seen = match word.as_str() {
            "--refresh" => std::mem::replace(&mut refresh, true),
            "--exact" => std::mem::replace(&mut exact, true),
            "--since" => since
                .replace(rest.next().ok_or_else(usage)?.clone())
                .is_some(),
            w if w.starts_with("--") => true,
            w => {
                positional.push(w.to_string());
                false
            }
        };
        if seen {
            return Err(usage());
        }
    }
    let shape = match name.as_str() {
        "day" => positional.len() == 1 && !exact && since.is_none(),
        "record" => positional.len() == 2 && since.is_none(),
        "history" => positional.len() == 1 && !refresh && !exact,
        "refresh" => (1..=31).contains(&positional.len()) && !refresh && !exact && since.is_none(),
        _ => false,
    };
    if !shape {
        return Err(usage());
    }
    let dates = match name.as_str() {
        "record" | "day" => positional[..1].to_vec(),
        "refresh" => positional.clone(),
        _ => vec![],
    };
    let mut unique = dates.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != dates.len() {
        return Err(usage());
    }
    if dates
        .iter()
        .chain(&since)
        .any(|d| !valid_date(&d.as_str().into()))
    {
        return Err(refuse("lesson_date_invalid"));
    }
    if dates.iter().any(|d| d.as_str() > today) {
        return Err(refuse("lesson_date_in_future"));
    }
    let mut positional = positional.into_iter();
    let mut next = || positional.next().expect("checked count");
    Ok(match name.as_str() {
        "day" => Verb::Day {
            date: next(),
            refresh,
        },
        "record" => Verb::Record {
            date: next(),
            student: student(&next())?,
            refresh,
            exact,
        },
        "history" => match student(&next())? {
            Student::Id(id) => Verb::History {
                id,
                since: since.unwrap_or_else(|| seoul_date(now_ms, -HISTORY_DAYS)),
            },
            Student::Name(_) => return Err(refuse("lesson_student_invalid")),
        },
        _ => Verb::Refresh { dates },
    })
}
/// `[--config PATH] lesson VERB ...`: the configuration path and the checked verb.
fn parse(args: &[String], today: &str, now_ms: u64) -> (PathBuf, Answer<Verb>) {
    let (path, rest) = match args {
        [flag, path, rest @ ..] if flag == "--config" => (PathBuf::from(path), rest),
        rest => (PathBuf::from(DEFAULT_CONFIG), rest),
    };
    match rest {
        [lesson, words @ ..] if lesson == "lesson" => (path, verb(words, today, now_ms)),
        _ => (path, Err(usage())),
    }
}
fn verb_name(args: &[String]) -> &str {
    ["day", "record", "history", "refresh"]
        .into_iter()
        .find(|v| args.iter().skip_while(|a| *a != "lesson").nth(1) == Some(&v.to_string()))
        .unwrap_or("unknown")
}

/// Run `edu` in process. `connect` makes the transport of one attempt to open the LMS
/// session; it is called only when a date is captured.
pub fn run<T: Transport>(
    args: &[String],
    clock: &dyn Runtime,
    connect: &mut dyn FnMut(&Cancellation) -> Result<T>,
) -> Outcome {
    let now_ms = clock.epoch_ms();
    let today = seoul_date(now_ms, 0);
    let (path, verb) = parse(args, &today, now_ms);
    let config = config::load(&path);
    let mut activity = Activity::default();
    let result = match (&verb, &config) {
        (Err(refusal), _) | (Ok(_), Err(refusal)) => Err(refusal.clone()),
        (Ok(verb), Ok(config)) => execute(verb, config, clock, connect, &mut activity),
    };
    let status = match &result {
        Ok(_) => "ok".to_string(),
        Err(refusal) => refusal.code.clone(),
    };
    let (mut stdout, mut code) = match result {
        Ok(text) => (text, 0),
        Err(refusal) => {
            let mut line = json!({"status": refusal.code});
            if let Some(count) = refusal.count {
                line["count"] = json!(count);
            }
            (format!("{line}\n"), 2)
        }
    };
    if let Ok(config) = &config {
        let dates = match &verb {
            Ok(Verb::Refresh { dates }) => dates.len(),
            Ok(_) => 1,
            Err(_) => 0,
        };
        let line = json!({"time": now_ms / 1000, "verb": verb_name(args), "dates": dates,
            "login": if activity.login { "new" } else { "none" }, "requests": activity.requests,
            "lock_wait_seconds": activity.waited, "status": status});
        if config::append_run(config, &line).is_err() {
            (stdout, code) = ("{\"status\":\"lesson_run_log_failed\"}\n".into(), 2);
        }
    }
    Outcome { stdout, code }
}

/// Line 1 of a date: the coverage object's time, its age and its counts.
fn coverage_line(store: &engine::Store, date: &str) -> Answer<Value> {
    let c = store
        .coverage(date)?
        .ok_or_else(|| refuse("lesson_coverage_missing"))?;
    Ok(
        json!({"status":"ok","date":date,"captured_at":c.observed_at,
        "age_seconds":store.now().saturating_sub(c.observed_at),"rows":c.rows,"stored":c.stored,
        "outside_roster":c.outside_roster,"cut_fields":c.cut_fields}),
    )
}
fn execute<T: Transport>(
    verb: &Verb,
    config: &Config,
    clock: &dyn Runtime,
    connect: &mut dyn FnMut(&Cancellation) -> Result<T>,
    activity: &mut Activity,
) -> Answer<String> {
    let store = engine::Store::new(config, clock);
    store.check_schema()?;
    lms::prepare_spool(config)?;
    store.index()?;
    let mut capture = |dates: &[String], activity: &mut Activity| {
        let captured = lms::capture(config, dates, clock, connect, activity);
        let indexed = store.index();
        captured.and(indexed)
    };
    let (mut line, body) = match verb {
        Verb::History { id, since } => {
            let filter = json!({"all":[{"eq":["student_id",id]},{"ge":["lesson_date",since]}]});
            let mut rows = store.search("lesson_record", filter)?;
            rows.sort_by_key(|r| {
                (
                    std::cmp::Reverse(engine::text(r, "lesson_date")),
                    engine::number(r, "row_ordinal"),
                )
            });
            rows.truncate(HISTORY_LIMIT);
            let oldest = rows.iter().map(engine::observed_at).min();
            let table = store.render("record_history", json!({"student_id":id,"since":since}))?;
            let line = json!({"status":"ok","since":since,"rows":rows.len(),"captured_at":oldest,
                "age_seconds":oldest.map(|t| store.now().saturating_sub(t))});
            (line, table)
        }
        Verb::Refresh { dates } => {
            capture(dates, activity)?;
            let lines = dates
                .iter()
                .map(|d| {
                    let mut line = coverage_line(&store, d)?;
                    line.as_object_mut().expect("object").remove("status");
                    Ok(line.to_string())
                })
                .collect::<Answer<Vec<_>>>()?;
            (json!({"status":"ok","dates":dates.len()}), lines.join("\n"))
        }
        Verb::Day { date, refresh } | Verb::Record { date, refresh, .. } => {
            if *refresh || store.coverage(date)?.is_none() {
                capture(std::slice::from_ref(date), activity)?;
            }
            let mut line = coverage_line(&store, date)?;
            let body = match verb {
                Verb::Record { student, exact, .. } => {
                    let id = resolve(&store, date, student)?;
                    line["student_id"] = json!(id);
                    if *exact {
                        exact_rows(&store, date, &id)?
                    } else {
                        store.render("daily_record", json!({"lesson_date":date,"student_id":id}))?
                    }
                }
                _ => store.render("daily_records", json!({"lesson_date":date}))?,
            };
            (line, body)
        }
    };
    activity.members(&mut line);
    Ok(format!("{line}\n{body}\n"))
}
/// A name is matched against the rows of that date; it must name exactly one student id.
fn resolve(store: &engine::Store, date: &str, student: &Student) -> Answer<String> {
    let name = match student {
        Student::Id(id) => return Ok(id.clone()),
        Student::Name(name) => name,
    };
    let filter = json!({"all":[{"eq":["lesson_date",date]},{"eq":["student_name",name]}]});
    let mut ids: Vec<String> = store
        .search("lesson_record", filter)?
        .iter()
        .map(|r| engine::text(r, "student_id"))
        .collect();
    ids.sort();
    ids.dedup();
    match ids.len() {
        0 => Err(refuse("lesson_student_not_found")),
        1 => Ok(ids.remove(0)),
        n => Err(Refusal {
            code: "lesson_student_ambiguous".into(),
            count: Some(n),
        }),
    }
}
/// Each row of the student on the date as one JSON object of its members (a missing
/// member is left out) and `observed_at`, in the page's order.
fn exact_rows(store: &engine::Store, date: &str, id: &str) -> Answer<String> {
    let filter = json!({"all":[{"eq":["lesson_date",date]},{"eq":["student_id",id]}]});
    let mut rows = store.search("lesson_record", filter)?;
    rows.sort_by_key(|r| engine::number(r, "row_ordinal"));
    Ok(rows
        .iter()
        .map(|r| {
            let mut members = engine::members(r);
            members.insert("observed_at".into(), json!(engine::observed_at(r)));
            Value::Object(members).to_string()
        })
        .collect::<Vec<_>>()
        .join("\n"))
}
