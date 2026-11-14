//! Read-only capture of one student's DailyTest history (family `dailytest`).
//!
//! Source parity (I3). Each request is the operation `metadata.daily`, the wire
//! of TypeScript `academyHttpClient.ts` `#dailyTestInventory` (`sort_date`,
//! `stu_name`). Parsers reached:
//! - `student_metadata::metadata_rows_by_expression`, from TypeScript
//!   `assessmentReadProtocol.ts` `inspectAssessmentSearchMetadata`: the rows come
//!   from the one whole-page `testResultList: JSON.parse('...')` expression; no
//!   other script content, token count, regular expression or template decides.
//!   `StudentSource::daily` is NOT used: it parses the same page with the
//!   syntax-tree reader whose 256-token script budget refuses a real page.
//! - the `select[name=sort_date]` options: TypeScript `readDailyTestHistory`
//!   (`controls` named `sort_date`: exactly one, with the option list) read
//!   through the crate DOM. NOT compared against a browser DOM: a disabled option
//!   is skipped here, TypeScript lists it.
//! - `student_course::response_text`: the shared password-page / session-text /
//!   status check of the metadata family.
use crate::{
    capture::{Options, Student, context, scope, validate_raw_root},
    data::{Error, Js, Result, json as to_json, string},
    dom::{self, Document},
    owner_admission::TransportAdmission,
    student_course::{
        METADATA_NS, ReadPort, Student as SourceStudent, check_contract, response_text,
        verify_form_ownership,
    },
    student_metadata::metadata_rows_by_expression,
    textbook_index::TextbookStudent,
};
use edu_academy_core::workflow::Runtime;
use serde_json::{Value, json};
use std::collections::HashSet;

/// The only operation of the family.
pub(crate) const OPERATIONS: &[&str] = &["metadata.daily"];
/// The selector literals the source code knows: `1` is the current filter.
const PERIODS: [&str; 7] = ["1", "11", "12", "13", "14", "15", "16"];

fn fail(code: &str) -> Error {
    Error::refusal(code)
}
/// Call before opening the owner session: no credentials, lock or login on refusal.
pub fn preflight(a: &TransportAdmission, students: &[Student], options: &Options) -> Result<()> {
    a.validate()?;
    if let Some(dir) = &options.raw_dir {
        validate_raw_root(dir)?;
    }
    if !(1..=200).contains(&students.len())
        || options.max_requests == 0
        || !options.spool.is_absolute()
    {
        return Err(fail("capture_input_invalid"));
    }
    for op in OPERATIONS {
        check_contract(a, op)?;
    }
    let mut ids = HashSet::new();
    for student in students {
        TextbookStudent::new(&student.student_id, &student.student_name)?;
        if !ids.insert(&student.student_id) {
            return Err(fail("capture_duplicate_student"));
        }
        a.authorize(&scope(a, student, &options.date, METADATA_NS))?;
    }
    Ok(())
}

/// Enabled options of the single `sort_date` control, in page order.
fn selectors(html: &str) -> Result<Vec<(String, String)>> {
    let d = Document::parse(html);
    let controls = d.select("input[name=sort_date],select[name=sort_date]");
    if controls.len() != 1 || controls[0].value().name() != "select" {
        return Err(fail("daily_history_selector_drift"));
    }
    let control = controls[0];
    if control.value().attr("form").is_some() {
        return Err(fail("daily_history_selector_drift"));
    }
    if let Some(form) = dom::closest(control, "form") {
        verify_form_ownership(&d, form)?;
    }
    let mut found: Vec<(String, String)> = vec![];
    for option in dom::select(control, "option") {
        if option.value().attr("disabled").is_some() {
            continue;
        }
        let value = dom::value(option);
        let label = dom::content(option);
        if found.len() >= 64
            || found.iter().any(|(v, _)| v == &value)
            || label.encode_utf16().count() > 200
        {
            return Err(fail("daily_history_selector_drift"));
        }
        found.push((value, label));
    }
    Ok(found)
}

fn read_page<P: ReadPort>(
    port: &mut P,
    scope: &crate::owner_admission::TransportScope,
    student: &SourceStudent,
    selector: &str,
) -> Result<String> {
    let response = port
        .read(
            scope,
            "metadata.daily",
            &[
                ("sort_date".into(), selector.into()),
                ("stu_name".into(), student.student_name.clone()),
            ],
        )
        .map_err(|e| context(e, "capture_daily_read_failed"))?;
    response_text(response).map_err(|e| context(e, "capture_daily_read_failed"))
}

struct Row {
    paper_key: String,
    attempt_key: String,
    period: String,
    period_ordinal: usize,
    row_ordinal: usize,
    source_row_id: Option<String>,
    source_score: Option<String>,
}

/// Every offered period of one student: the page `1` first (it also lists the
/// periods), then one request per other offered period among 11 to 16.
pub(crate) fn read_student<P: ReadPort>(
    mut port: P,
    student: &Student,
    options: &Options,
    runtime: &dyn Runtime,
) -> Result<Vec<Value>> {
    let who = TextbookStudent::new(&student.student_id, &student.student_name)
        .map_err(|e| context(e, "capture_student_invalid"))?;
    let source = SourceStudent {
        student_key: who.student_key().into(),
        student_name: who.student_name().into(),
    };
    let scope = scope(
        port.transport_admission(),
        student,
        &options.date,
        METADATA_NS,
    );
    let current = read_page(&mut port, &scope, &source, "1")?;
    let offered = selectors(&current).map_err(|e| context(e, "capture_daily_selector_failed"))?;
    let periods: Vec<(usize, &str)> = offered
        .iter()
        .enumerate()
        .filter(|(_, (value, _))| PERIODS.contains(&value.as_str()))
        .map(|(i, (value, _))| (i + 1, value.as_str()))
        .collect();
    // `1` is the filter the first request already applied; without it the
    // current page's rows have no position in the list.
    if !periods.iter().any(|(_, value)| *value == "1") {
        return Err(context(
            fail("daily_history_selector_drift"),
            "capture_daily_selector_failed",
        ));
    }
    let mut rows: Vec<Row> = vec![];
    for (ordinal, selector) in periods {
        let html = if selector == "1" {
            current.clone()
        } else {
            let html = read_page(&mut port, &scope, &source, selector)?;
            // Every fetched page passes the control checks of the first one, and the
            // requested selector must be one of its enabled options, before its rows
            // are taken. (TypeScript `readDailyTestHistory` checks the first page only
            // and never which option a page marks as selected: neither is added here.)
            let options =
                selectors(&html).map_err(|e| context(e, "capture_daily_selector_failed"))?;
            if !options.iter().any(|(value, _)| value == selector) {
                return Err(context(
                    fail("daily_history_selector_drift"),
                    "capture_daily_selector_failed",
                ));
            }
            html
        };
        let page = metadata_rows_by_expression(&html, &source)
            .map_err(|e| context(e, "capture_daily_parse_failed"))?;
        for (i, row) in page.iter().enumerate() {
            rows.push(Row {
                paper_key: string(row.get("paperKey")),
                attempt_key: string(row.get("attemptKey")),
                period: selector.into(),
                period_ordinal: ordinal,
                row_ordinal: i + 1,
                source_row_id: row
                    .get("sourceRowId")
                    .string()
                    .then(|| string(row.get("sourceRowId"))),
                source_score: match row.get("sourceScore") {
                    Js::Num(_) => Some(to_json(row.get("sourceScore"))),
                    _ => None,
                },
            });
        }
    }
    // `student|attempt` when that attempt key is unique in the whole read; an
    // attempt key seen more than once (several periods, or several papers) gets
    // `|paper|period` on every occurrence, which is unique: a page never repeats a
    // (paper, attempt) pair.
    let mut seen = HashSet::new();
    let mut repeated = HashSet::new();
    for row in &rows {
        if !seen.insert(&row.attempt_key) {
            repeated.insert(row.attempt_key.clone());
        }
    }
    Ok(rows
        .iter()
        .map(|row| {
            let base = format!("{}|{}", student.student_id, row.attempt_key);
            let key = if repeated.contains(&row.attempt_key) {
                format!("{base}|{}|{}", row.paper_key, row.period)
            } else {
                base
            };
            json!({"type":"paper_attempt","key":key,"observed_at":runtime.epoch_ms()/1000,
                "student_id":student.student_id,"student":student.student_id,
                "attempt_key":row.attempt_key,"paper_key":row.paper_key,
                "period":row.period,"period_ordinal":row.period_ordinal,"row_ordinal":row.row_ordinal,
                "source_row_id":row.source_row_id,"source_score":row.source_score})
        })
        .collect())
}

#[cfg(test)]
#[path = "dailytest_tests.rs"]
mod tests;
