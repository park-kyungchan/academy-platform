//! All-groups, date-scoped passive capture. Never calls a save or a selected-record route.
use crate::{
    capture::{Student, scope, validate_raw_root},
    data::{Error, Result, string, valid_date},
    dom::{self, Document},
    http,
    owner_admission::TransportAdmission,
    student_course::{DAY_NS, ReadPort, check_contract, response_text, verify_form_ownership},
};
use edu_academy_core::workflow::Runtime;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
#[path = "dailyrecord_store.rs"]
mod store;
use store::{Limits, ensure_dirs, publish};
pub(crate) const OPERATION: &str = "student.day.groups";
pub struct Options {
    pub dates: Vec<String>,
    pub max_requests: usize,
    pub spool: PathBuf,
    pub raw_dir: Option<PathBuf>,
}
#[derive(Serialize)]
pub struct DateReport {
    pub date: String,
    pub status: String,
    pub code: Option<String>,
    pub rows: usize,
    pub stored: usize,
    pub outside_roster: usize,
    pub cut_fields: usize,
    pub rows_before: usize,
}
fn fail(code: &str) -> Error {
    Error::refusal(code)
}
pub fn dates(value: &str) -> Result<Vec<String>> {
    let dates: Vec<String> = value.split(',').map(str::to_string).collect();
    validate_dates(&dates)?;
    Ok(dates)
}
fn validate_dates(dates: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    if !(1..=31).contains(&dates.len())
        || dates
            .iter()
            .any(|d| !valid_date(&d.as_str().into()) || !seen.insert(d))
    {
        return Err(fail("capture_input_invalid"));
    }
    Ok(())
}
fn digits(s: &str) -> bool {
    (1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}
/// Syntax and documents before any session. Window checks stay per date, before its request.
pub fn preflight(a: &TransportAdmission, students: &[Student], o: &Options) -> Result<()> {
    validate_dates(&o.dates)?;
    a.validate()?;
    check_contract(a, OPERATION)?;
    if !(1..=200).contains(&students.len()) || o.max_requests == 0 || !o.spool.is_absolute() {
        return Err(fail("capture_input_invalid"));
    }
    let mut seen = HashSet::new();
    if students
        .iter()
        .any(|s| !digits(&s.student_id) || !seen.insert(&s.student_id))
    {
        return Err(fail("capture_input_invalid"));
    }
    a.authorize(&scope(a, &students[0], &a.allowed_from, DAY_NS))?;
    if let Some(raw) = &o.raw_dir {
        validate_raw_root(raw)?;
    }
    Ok(())
}
pub fn refused(dates: &[String], code: &str) -> Vec<DateReport> {
    dates
        .iter()
        .enumerate()
        .map(|(i, d)| DateReport {
            date: d.clone(),
            status: if i == 0 { "refused" } else { "not_run" }.into(),
            code: if i == 0 { Some(code.into()) } else { None },
            rows: 0,
            stored: 0,
            outside_roster: 0,
            cut_fields: 0,
            rows_before: 0,
        })
        .collect()
}
/// Initialize the two sources even if the first date subsequently refuses.
pub fn initialize(o: &Options) -> Result<()> {
    validate_dates(&o.dates)?;
    if !o.spool.is_absolute() || o.max_requests == 0 {
        return Err(fail("capture_input_invalid"));
    }
    ensure_dirs(&o.spool)
}
/// Exactly one operation per date, one budget for the run. NativeSession owns the login and lease.
pub fn run<P: ReadPort>(
    port: &mut P,
    students: &[Student],
    o: &Options,
    clock: &dyn Runtime,
) -> Vec<DateReport> {
    let result = initialize(o).and_then(|_| preflight(port.transport_admission(), students, o));
    if let Err(e) = result {
        return refused(&o.dates, &e.message);
    }
    let mut reports = refused(&o.dates, "capture_input_invalid");
    let mut stopped = false;
    for (i, date) in o.dates.iter().enumerate() {
        if stopped {
            continue;
        }
        let report = &mut reports[i];
        let result = (|| {
            let a = port.transport_admission();
            let s = scope(a, &students[0], date, DAY_NS);
            a.authorize(&s)?;
            check_contract(a, OPERATION)?;
            if i >= o.max_requests {
                return Err(fail("capture_request_limit"));
            }
            port.authenticate_for_reads()?;
            let response =
                port.read(&s, OPERATION, &[("std_ymd".into(), date.replace('-', ""))])?;
            let observed = port
                .observed_at()
                .unwrap_or_else(|| clock.epoch_ms() / 1000);
            let html = response_text(response)?;
            let p = parse_page(
                &html,
                date,
                &port.transport_admission().teacher_id,
                students,
                observed,
            )?;
            report.rows = p.rows;
            report.stored = p.records.len();
            report.outside_roster = p.outside;
            report.cut_fields = p.cut;
            report.rows_before = publish(
                &o.spool,
                date,
                &p,
                clock.epoch_ms() / 1000,
                Limits::default(),
            )?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                report.status = "published".into();
                report.code = None;
            }
            Err(e) => {
                report.status = "refused".into();
                report.code = Some(e.message);
                stopped = true;
            }
        }
    }
    reports
}
struct Page {
    records: Vec<Value>,
    rows: usize,
    outside: usize,
    cut: usize,
    observed: u64,
}
fn one<'a>(e: scraper::ElementRef<'a>, selector: &str) -> Result<scraper::ElementRef<'a>> {
    let nodes = dom::select(e, selector);
    if nodes.len() != 1 {
        return Err(fail("capture_day_structure_invalid"));
    }
    Ok(nodes[0])
}
fn parse_page(
    html: &str,
    date: &str,
    teacher: &str,
    students: &[Student],
    observed: u64,
) -> Result<Page> {
    let doc = Document::parse(html);
    // Check raw closing tokens: the HTML parser repairs a truncated document.
    let lower = html.to_ascii_lowercase();
    if !lower.trim_end().ends_with("</html>")
        || !lower.contains("</body>")
        || !lower.contains("</form>")
    {
        return Err(fail("capture_day_incomplete"));
    }
    for (selector, value, code) in [
        ("input[name=std_date]", date, "capture_day_date_conflict"),
        (
            "input[name=teacher_pri_no]",
            teacher,
            "capture_day_teacher_conflict",
        ),
    ] {
        let nodes = doc.select(selector);
        if nodes.len() != 1 || dom::value(nodes[0]) != value {
            return Err(fail(code));
        }
    }
    let groups = doc.select("select[name=grp_seq]");
    if groups.len() != 1
        || !matches!(dom::value(groups[0]).as_str(), "" | "0")
        || groups[0].value().attr("multiple").is_some()
        || groups[0].value().attr("disabled").is_some()
    {
        return Err(fail("capture_day_group_conflict"));
    }
    let form =
        dom::closest(groups[0], "form").ok_or_else(|| fail("capture_day_structure_invalid"))?;
    verify_form_ownership(&doc, form)?;
    let tables: Vec<_> = doc
        .select("table")
        .into_iter()
        .filter(|t| http::capture_headers(*t).is_ok())
        .collect();
    if tables.len() != 1 {
        return Err(fail("day_record_column_contract_rejected"));
    }
    let table = tables[0];
    if dom::closest(table, "form").is_none_or(|f| f.id() != form.id()) {
        return Err(fail("capture_day_structure_invalid"));
    }
    let rows: Vec<_> = dom::select(table, "tr")
        .into_iter()
        .filter(|r| dom::closest(*r, "table").is_some_and(|t| t.id() == table.id()))
        .skip(2)
        .collect();
    let roster: HashSet<_> = students.iter().map(|s| s.student_id.as_str()).collect();
    let mut p = Page {
        records: vec![],
        rows: rows.len(),
        outside: 0,
        cut: 0,
        observed,
    };
    let mut keys = HashMap::new();
    for (ordinal, row) in rows.into_iter().enumerate() {
        let cells: Vec<_> = row
            .children()
            .filter_map(scraper::ElementRef::wrap)
            .collect();
        if cells.len() != 13
            || cells.iter().any(|c| {
                c.value().name() != "td"
                    || c.value().attr("colspan").unwrap_or("1") != "1"
                    || c.value().attr("rowspan").unwrap_or("1") != "1"
            })
        {
            return Err(fail("capture_day_structure_invalid"));
        }
        let (fields, binding) = http::bind_capture_row(&doc, row)?;
        if string(binding.get("i")) != ordinal.to_string() {
            return Err(fail("capture_day_ordinal_gap"));
        }
        let name = one(cells[0], "[onclick]")?;
        let attendance = one(cells[8], "select")?;
        let selected_value = dom::value(attendance);
        let options = dom::select(attendance, "option");
        let selected = options
            .iter()
            .rev()
            .find(|e| e.value().attr("selected").is_some())
            .copied()
            .or_else(|| {
                options
                    .iter()
                    .find(|e| e.value().attr("disabled").is_none())
                    .copied()
            })
            .ok_or_else(|| fail("capture_day_structure_invalid"))?;
        let mut r = json!({"type":"lesson_record","observed_at":observed,"source_namespace":DAY_NS,"lesson_date":date,"row_ordinal":ordinal,"student_id":string(binding.get("stu_pri_no")),"student_name":dom::content(name).trim(),"membership_key":string(binding.get("cm_seq")),"course_id":string(binding.get("course_seq")),"course_label":dom::content(cells[1]).trim(),"attendance":string(fields.get("attendance")),"attendance_label":dom::content(selected).trim(),"progress":string(fields.get("progress")),"homework":string(fields.get("homework")),"lesson_memo":string(fields.get("lessonMemo")),"student_memo":string(fields.get("persistentMemo")),"previous_lesson_date_literal":dom::content(cells[3]).trim(),"previous_progress":dom::value(one(cells[4],"textarea[readonly]")?),"previous_homework":dom::value(one(cells[5],"textarea[readonly]")?),"previous_lesson_memo":dom::value(one(cells[6],"textarea[readonly]")?),"daily_test_assessment":dom::value(one(cells[9],"input[type=hidden][name=h_daily_test]")?),"previous_homework_assessment":dom::value(one(cells[5],"input[type=hidden][name=h_homework_rate]")?),"cut_fields":0});
        if selected_value != r["attendance"]
            || [
                "progress",
                "homework",
                "lessonMemo",
                "persistentMemo",
                "attendance",
            ]
            .iter()
            .any(|k| !fields.get(k).string())
        {
            return Err(fail("capture_day_structure_invalid"));
        }
        for (source, target) in [
            ("record_seq", "record_key"),
            ("previous_record_key", "previous_record_key"),
        ] {
            let value = string(binding.get(source));
            if !value.is_empty() {
                r[target] = json!(value);
            }
        }
        // Compare the page's original values, not their cut forms; keep the first ordinal.
        let key = record_key(&r)?;
        let mut compared = r.clone();
        compared
            .as_object_mut()
            .expect("object")
            .remove("row_ordinal");
        let duplicate = if let Some(old) = keys.insert(key, compared.clone()) {
            if old != compared {
                return Err(fail("duplicate_key"));
            }
            true
        } else {
            false
        };
        bound_row(&mut r)?;
        if !roster.contains(r["student_id"].as_str().expect("validated")) {
            p.outside += 1;
            continue;
        }
        if duplicate {
            continue;
        }
        p.cut += r["cut_fields"].as_u64().expect("count") as usize;
        p.records.push(r);
    }
    Ok(p)
}
const CUT: &[(&str, usize)] = &[
    ("progress", 6000),
    ("homework", 6000),
    ("lesson_memo", 6000),
    ("student_memo", 6000),
    ("previous_progress", 6000),
    ("previous_homework", 6000),
    ("previous_lesson_memo", 6000),
    ("student_name", 200),
    ("course_label", 200),
    ("attendance_label", 64),
    ("previous_lesson_date_literal", 64),
];
const CODES: &[(&str, usize)] = &[
    ("student_id", 32),
    ("membership_key", 32),
    ("course_id", 32),
    ("record_key", 32),
    ("previous_record_key", 32),
    ("attendance", 16),
    ("daily_test_assessment", 16),
    ("previous_homework_assessment", 16),
];
fn escaped_len(s: &str) -> usize {
    serde_json::to_string(s).expect("string").len() - 2
}
fn bound_row(r: &mut Value) -> Result<()> {
    record_key(r)?;
    for &(name, bound) in CODES {
        if let Some(v) = r.get(name)
            && v.as_str().is_none_or(|s| escaped_len(s) > bound)
        {
            return Err(fail("capture_day_code_too_long"));
        }
    }
    let mut count = 0;
    for &(name, bound) in CUT {
        let text = r[name]
            .as_str()
            .ok_or_else(|| fail("capture_day_structure_invalid"))?;
        if escaped_len(text) > bound {
            let marker = format!(" [cut: {} bytes in the LMS]", text.len());
            let mut end = 0;
            let mut bytes = escaped_len(&marker);
            for c in text.chars() {
                let n = escaped_len(c.encode_utf8(&mut [0; 4]));
                if bytes + n > bound {
                    break;
                }
                bytes += n;
                end += c.len_utf8();
            }
            r[name] = json!(format!("{}{}", &text[..end], marker));
            count += 1;
        }
    }
    r["cut_fields"] = json!(count);
    Ok(())
}
fn record_key(r: &Value) -> Result<String> {
    let id = r["student_id"]
        .as_str()
        .filter(|s| digits(s))
        .ok_or_else(|| fail("capture_day_key_invalid"))?;
    let membership = r["membership_key"]
        .as_str()
        .filter(|s| digits(s))
        .ok_or_else(|| fail("capture_day_key_invalid"))?;
    let date = r["lesson_date"]
        .as_str()
        .filter(|d| valid_date(&(*d).into()))
        .ok_or_else(|| fail("capture_day_key_invalid"))?;
    Ok(format!("{id}|{membership}|{date}"))
}
#[cfg(test)]
#[path = "dailyrecord_tests.rs"]
mod tests;
