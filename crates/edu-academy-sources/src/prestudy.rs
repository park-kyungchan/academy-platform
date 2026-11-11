//! Prestudy (예습영상) list, detail and AI-report source parsing (E11a).
//! Mirrors `apps/academy/dc-gang-a/src/lms/prestudyRead.ts` and the prestudy
//! half of `AcademyHttpClient` / `NativeDailyRecordBackend`: waiting and
//! completed list membership, the summary AI/star literals, the exact-record
//! detail form and the server-rendered report cards. Upload, list membership,
//! AI score and teacher review are distinct states; nothing here derives one
//! from another, retains a video URL, or invents a transcript. The UTF-8
//! output budget itself is an MCP-layer rule (E11b); this layer only exposes
//! the view and the byte measurement it is applied to. JavaScript text rules
//! (whitespace, trimming, UTF-16 length, bounded decoding) come from
//! `crate::js_text`, never re-derived here.
use crate::{
    data::*,
    dom::{self, Document},
    http::Response,
    js_text::{JS_WS_CLASS, decode_bounded, js_collapse_whitespace, js_trim, utf16_len},
    owner_admission::TransportScope,
    student_course::{ReadPort, check_contract, require},
};
use std::fmt;

pub const PRESTUDY_PATH: &str = "/servlet/controller.coursemanage.CourseManageServlet";
pub const PRESTUDY_REPORT_PATH: &str = "/app-ui/solution_common/prestudy/prestudy_ai_report.jsp";
pub const MAX_SOURCE_BYTES: usize = 8_000_000;
pub const MAX_SOURCE_UNITS: usize = 2_000_000;
pub const MAX_REPORT_TEXT_UNITS: usize = 60_000;
pub const MAX_ROWS: usize = 1000;
/// Backend shape limits (`native-dailyrecord-backend.ts:231-241`), counted
/// in UTF-16 units as JavaScript counts them.
pub const MAX_SECTIONS: usize = 16;
pub const MAX_BLOCKS_PER_SECTION: usize = 60;
pub const MAX_BLOCK_TEXT_UNITS: usize = 20_000;
pub const MAX_SECTION_TITLE_UNITS: usize = 500;
pub const MAX_SECTION_SUBTITLE_UNITS: usize = 1000;
pub const MAX_BLOCK_HEADING_UNITS: usize = 1000;
pub const MAX_SOURCE_UNIT_LABEL_UNITS: usize = 500;
pub const MAX_SOURCE_DATE_LITERAL_UNITS: usize = 200;
pub const MAX_ENCODING_RESULT_UNITS: usize = 200;
/// MCP-layer defaults E11b applies through `check_report_budget`.
pub const DEFAULT_SELECTED_MAX_BYTES: usize = 32 * 1024;
pub const DEFAULT_FULL_MAX_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Waiting,
    Completed,
}
impl ListKind {
    pub fn cmd(self) -> &'static str {
        match self {
            Self::Waiting => "WebUnPreStudy",
            Self::Completed => "WebCtPreStudy",
        }
    }
    fn shell_operation(self) -> &'static str {
        match self {
            Self::Waiting => "prestudy.waiting.shell",
            Self::Completed => "prestudy.completed.shell",
        }
    }
    fn search_operation(self) -> &'static str {
        match self {
            Self::Waiting => "prestudy.waiting.search",
            Self::Completed => "prestudy.completed.search",
        }
    }
}

fn len_debug(field: &str, value: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{field}: {} units", utf16_len(value))
}

#[derive(Clone, PartialEq, Eq)]
pub struct PrestudySearchForm {
    pub req_cmd: String,
    pub college_key: String,
    pub member_type: String,
    pub teacher_key: String,
}
impl fmt::Debug for PrestudySearchForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrestudySearchForm")
            .field("req_cmd", &self.req_cmd)
            .field("college_key_units", &utf16_len(&self.college_key))
            .field("member_type_units", &utf16_len(&self.member_type))
            .field("teacher_key_units", &utf16_len(&self.teacher_key))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PrestudyListRow {
    pub student_label: String,
    pub unit_label: String,
    pub date_literal: String,
    pub status_literal: String,
    pub prestudy_key: String,
}
impl fmt::Debug for PrestudyListRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrestudyListRow {{ ")?;
        len_debug("student_label", &self.student_label, f)?;
        write!(f, ", ")?;
        len_debug("unit_label", &self.unit_label, f)?;
        write!(f, ", ")?;
        len_debug("date_literal", &self.date_literal, f)?;
        write!(f, ", ")?;
        len_debug("status_literal", &self.status_literal, f)?;
        write!(
            f,
            ", prestudy_key: {} units }}",
            utf16_len(&self.prestudy_key)
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PrestudySummaryRow {
    pub student_label: String,
    pub unit_label: String,
    pub date_literal: String,
    pub ai_score_literal: String,
    pub rating_literal: String,
    pub report_available: bool,
    pub video_available: bool,
}
impl fmt::Debug for PrestudySummaryRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrestudySummaryRow {{ ")?;
        len_debug("student_label", &self.student_label, f)?;
        write!(f, ", ")?;
        len_debug("unit_label", &self.unit_label, f)?;
        write!(
            f,
            ", report_available: {}, video_available: {} }}",
            self.report_available, self.video_available
        )
    }
}

/// The three observed surfaces for one exact student. `teacher_judgment` is
/// always `None`: a source OK or AI score is never a teacher judgment.
#[derive(Clone, PartialEq, Eq)]
pub struct PrestudyLists {
    pub waiting: Vec<PrestudyListRow>,
    pub completed: Vec<PrestudyListRow>,
    pub summary: Vec<PrestudySummaryRow>,
    pub teacher_judgment: Option<()>,
    pub observed_at: String,
    pub coverage: String,
}
impl fmt::Debug for PrestudyLists {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrestudyLists")
            .field("waiting", &self.waiting.len())
            .field("completed", &self.completed.len())
            .field("summary", &self.summary.len())
            .field("teacher_judgment", &self.teacher_judgment)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PrestudyDetail {
    pub prestudy_key: String,
    pub unit_label: Option<String>,
    pub date_literal: Option<String>,
    pub analysis_key: Option<String>,
    pub encoding_result: Option<String>,
}
impl fmt::Debug for PrestudyDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PrestudyDetail {{ prestudy_key: {} units, unit_label: ",
            utf16_len(&self.prestudy_key)
        )?;
        match &self.unit_label {
            Some(v) => len_debug("Some", v, f)?,
            None => write!(f, "None")?,
        }
        write!(f, ", analysis_key: {} }}", self.analysis_key.is_some())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PrestudySourceRecord {
    pub prestudy_key: String,
    pub unit_label: Option<String>,
    pub date_literal: Option<String>,
}
impl fmt::Debug for PrestudySourceRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PrestudySourceRecord {{ prestudy_key: {} units }}",
            utf16_len(&self.prestudy_key)
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PrestudyReportBlock {
    pub heading: Option<String>,
    pub text: String,
}
impl fmt::Debug for PrestudyReportBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrestudyReportBlock {{ heading: ",)?;
        match &self.heading {
            Some(v) => write!(f, "Some({} units)", utf16_len(v))?,
            None => write!(f, "None")?,
        }
        write!(f, ", text: {} units }}", utf16_len(&self.text))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PrestudyReportSection {
    pub title: String,
    pub subtitle: Option<String>,
    pub blocks: Vec<PrestudyReportBlock>,
}
impl fmt::Debug for PrestudyReportSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PrestudyReportSection {{ title: {} units, blocks: {} }}",
            utf16_len(&self.title),
            self.blocks.len()
        )
    }
}

#[derive(Clone, PartialEq)]
pub struct PrestudyReport {
    pub sections: Vec<PrestudyReportSection>,
    pub scores: Vec<f64>,
}
impl fmt::Debug for PrestudyReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrestudyReport")
            .field("sections", &self.sections.len())
            .field("scores", &self.scores.len())
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct PrestudyReportObservation {
    pub prestudy_key: String,
    pub source_record: PrestudySourceRecord,
    pub sections: Vec<PrestudyReportSection>,
    pub scores: Vec<f64>,
    pub encoding_result: Option<String>,
    pub observed_at: String,
    pub coverage: String,
}
impl fmt::Debug for PrestudyReportObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrestudyReportObservation")
            .field("prestudy_key_units", &utf16_len(&self.prestudy_key))
            .field("sections", &self.sections.len())
            .field("scores", &self.scores.len())
            .finish_non_exhaustive()
    }
}

/// Exact-title selection over the observed sections, in source order.
#[derive(Clone, PartialEq, Eq)]
pub struct PrestudyReportView {
    pub selected: bool,
    pub requested_titles: Option<Vec<String>>,
    pub available_titles: Vec<String>,
    pub returned_titles: Vec<String>,
    pub missing_titles: Vec<String>,
    pub complete_requested_view: bool,
    pub complete_source_report: bool,
}
impl fmt::Debug for PrestudyReportView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrestudyReportView")
            .field("selected", &self.selected)
            .field("available_titles", &self.available_titles.len())
            .field("returned_titles", &self.returned_titles.len())
            .field("missing_titles", &self.missing_titles.len())
            .field("complete_requested_view", &self.complete_requested_view)
            .field("complete_source_report", &self.complete_source_report)
            .finish()
    }
}

/// `JSON.stringify` of a score array: numbers print as JavaScript prints
/// them, including exponent form for a value below 1e-6 or at least 1e21
/// (`1e-7`, `1e+23`). The number text comes from the shared JS stringifier,
/// never from Rust `Display`.
pub fn scores_json(scores: &[f64]) -> String {
    let parts: Vec<String> = scores
        .iter()
        .map(|n| stringify(&Js::Num(*n)).unwrap_or_else(|_| "null".into()))
        .collect();
    format!("[{}]", parts.join(","))
}

fn clean(value: &str) -> String {
    js_collapse_whitespace(value)
}

/// The TS session-expired test (`prestudyRead.ts:20`): a regular expression
/// over the raw text, case-insensitive. The crate's broader login challenge
/// expression (`http.rs` authenticate path) additionally matches the login
/// form; this is the prestudy expression exactly.
fn reject_expired(html: &str) -> Result<()> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        // ASCII-only case folding and ASCII `\b`, as JavaScript `/i` without
        // the `u` flag; `\s` is the S1 whitespace class.
        regex::Regex::new(&format!(
            r#"(?i-u:normal_session_error\.jsp|popup_session_error\.jsp)|세션이 만료|(?i-u:<input\b)[^>]*(?i-u:type){ws}*={ws}*["']?(?i-u:password)"#,
            ws = JS_WS_CLASS
        ))
        .expect("constant")
    });
    if re.is_match(html) {
        return Err(Error::refusal("prestudy_authentication_required"));
    }
    require(
        utf16_len(html) <= MAX_SOURCE_UNITS,
        "prestudy_source_rejected",
    )
}

/// The parser key pattern (`prestudyRead.ts:120,132`):
/// `^[1-9][0-9]{0,15}$` and nothing else — no safe-integer cap here.
fn key_pattern(value: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[1-9][0-9]{0,15}$").expect("constant"))
        .is_match(value)
}

/// The entry guard (`academyReadProtocol.ts:46-49` `readLiteralKey`): the
/// parser pattern plus the safe-integer bound, with the TS entry code.
pub fn literal_key(value: &str) -> Result<String> {
    require(
        key_pattern(value)
            && value
                .parse::<u64>()
                .is_ok_and(|n| n <= 9_007_199_254_740_991),
        "invalid_read_key",
    )?;
    Ok(value.to_string())
}

/// The entry guard (`academyReadProtocol.ts:42-45` `readLiteralName`):
/// non-blank after the JS trim, at most 200 UTF-16 units.
pub fn literal_name(value: &str) -> Result<String> {
    require(
        !js_trim(value).is_empty() && utf16_len(value) <= 200,
        "invalid_read_name",
    )?;
    Ok(value.to_string())
}

fn field(form: scraper::ElementRef<'_>, name: &str) -> Option<String> {
    dom::select(form, &format!("[name=\"{name}\"]"))
        .first()
        .map(|n| dom::value(*n))
}

/// The search form is the wire contract: hidden scope fields are read back
/// from the live page, never reconstructed from caller input. Any element
/// with the name supplies the value (`prestudyRead.ts:36`).
pub fn parse_search_form(
    html: &str,
    expected: ListKind,
    teacher_key: &str,
) -> Result<PrestudySearchForm> {
    reject_expired(html)?;
    let doc = Document::parse(html);
    let forms = doc.select("form[name=\"form1\"]");
    let form = forms
        .first()
        .ok_or_else(|| Error::refusal("prestudy_search_form_missing"))?;
    let get = |name: &str| field(*form, name);
    let req_cmd = get("reqCmd");
    let college = get("g_clg_no");
    let member = get("g_mem_type");
    let teacher = get("g_pri_no");
    require(
        req_cmd.as_deref() == Some(expected.cmd())
            && college.as_ref().is_some_and(|v| !v.is_empty())
            && member.is_some()
            && teacher.as_deref() == Some(teacher_key)
            && !dom::select(*form, "[name=\"student_name\"]").is_empty(),
        "prestudy_search_form_drift",
    )?;
    Ok(PrestudySearchForm {
        req_cmd: req_cmd.unwrap_or_default(),
        college_key: college.unwrap_or_default(),
        member_type: member.unwrap_or_default(),
        teacher_key: teacher.unwrap_or_default(),
    })
}

fn row_cells(row: scraper::ElementRef<'_>) -> Vec<String> {
    dom::select(row, ":scope > td")
        .iter()
        .map(|c| clean(&dom::content(*c)))
        .collect()
}

pub fn parse_list(
    html: &str,
    expected: ListKind,
    teacher_key: &str,
) -> Result<Vec<PrestudyListRow>> {
    parse_search_form(html, expected, teacher_key)?;
    let doc = Document::parse(html);
    let re = regex::Regex::new(&format!(
        r"prestudy_main\({JS_WS_CLASS}*([0-9]+){JS_WS_CLASS}*\)"
    ))
    .expect("constant");
    let mut rows = Vec::new();
    for row in doc.select("tr") {
        let handler = dom::select(row, "[onclick]")
            .iter()
            .map(|n| n.value().attr("onclick").unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ");
        let Some(key) = re.captures(&handler).map(|c| c[1].to_string()) else {
            continue;
        };
        let cells = row_cells(row);
        require(
            cells.len() >= 4 && !cells[0].is_empty() && !cells[1].is_empty(),
            "prestudy_row_unqualified",
        )?;
        rows.push(PrestudyListRow {
            student_label: cells[0].clone(),
            unit_label: cells[1].clone(),
            date_literal: cells.get(2).cloned().unwrap_or_default(),
            status_literal: cells.get(3).cloned().unwrap_or_default(),
            prestudy_key: key,
        });
    }
    require(rows.len() <= MAX_ROWS, "prestudy_row_unqualified")?;
    Ok(rows)
}

pub fn parse_summary(html: &str) -> Result<Vec<PrestudySummaryRow>> {
    reject_expired(html)?;
    let doc = Document::parse(html);
    let mut rows = Vec::new();
    for row in doc.select("tr") {
        let handlers: Vec<String> = dom::select(row, "[onclick]")
            .iter()
            .map(|n| n.value().attr("onclick").unwrap_or("").to_string())
            .collect();
        let video = handlers.iter().any(|h| h.contains("playVideo("));
        let report = handlers.iter().any(|h| h.contains("openReport("));
        if !video && !report {
            continue;
        }
        let cells = row_cells(row);
        require(
            cells.len() >= 5 && !cells[0].is_empty() && !cells[1].is_empty(),
            "prestudy_summary_row_unqualified",
        )?;
        rows.push(PrestudySummaryRow {
            student_label: cells[0].clone(),
            unit_label: cells[1].clone(),
            date_literal: cells.get(2).cloned().unwrap_or_default(),
            ai_score_literal: cells.get(3).cloned().unwrap_or_default(),
            rating_literal: cells.get(4).cloned().unwrap_or_default(),
            report_available: report,
            video_available: video,
        });
    }
    require(rows.len() <= MAX_ROWS, "prestudy_summary_row_unqualified")?;
    Ok(rows)
}

pub(crate) trait HasStudentLabel {
    fn student_label(&self) -> &str;
}
impl HasStudentLabel for PrestudyListRow {
    fn student_label(&self) -> &str {
        &self.student_label
    }
}
impl HasStudentLabel for PrestudySummaryRow {
    fn student_label(&self) -> &str {
        &self.student_label
    }
}

/// Exact-name match: the plain label, or the label with a source suffix in
/// parentheses. Never a substring match.
pub(crate) fn rows_for_student<T: HasStudentLabel + Clone>(
    rows: &[T],
    student_name: &str,
) -> Vec<T> {
    rows.iter()
        .filter(|r| {
            r.student_label() == student_name
                || r.student_label().starts_with(&format!("{student_name}("))
        })
        .cloned()
        .collect()
}

/// One uploaded-video detail record (`prestudyRead.ts:106-135`). Field
/// values are returned raw whenever their trimmed form is non-empty; the
/// binding comparisons therefore refuse a whitespace-padded id or key
/// exactly as TS does.
pub fn parse_detail(html: &str, student_key: &str, prestudy_key: &str) -> Result<PrestudyDetail> {
    reject_expired(html)?;
    let doc = Document::parse(html);
    let forms = doc.select("form[name=\"form1\"]");
    require(forms.len() == 1, "prestudy_detail_unqualified")?;
    let form = forms[0];
    let field = |name: &str| -> Result<Option<String>> {
        // `input[name=...]` as `prestudyRead.ts:113` (the search form reads
        // any element, `:36`; the detail form reads inputs only).
        let nodes = dom::select(form, &format!("input[name=\"{name}\"]"));
        require(nodes.len() <= 1, "prestudy_detail_unqualified")?;
        Ok(nodes.first().and_then(|n| {
            let v = dom::value(*n);
            (!js_trim(&v).is_empty()).then_some(v)
        }))
    };
    let student_field = field("student_pri_no")?;
    let source_key = field("prestudy_key")?;
    require(
        student_field.is_some() && source_key.as_ref().is_some_and(|k| key_pattern(k)),
        "prestudy_detail_unqualified",
    )?;
    let source_key = source_key.unwrap();
    require(
        student_field.as_deref() == Some(student_key),
        "prestudy_detail_student_mismatch",
    )?;
    require(
        source_key == prestudy_key,
        "prestudy_detail_record_mismatch",
    )?;
    let re = regex::Regex::new(&format!(
        r#"viewAiReport\({ws}*'([^']+)'{ws}*\)|viewAiReport\({ws}*"([^"]+)"{ws}*\)"#,
        ws = JS_WS_CLASS
    ))
    .expect("constant");
    let mut analysis_key: Option<String> = None;
    for node in doc.select("[onclick]") {
        if let Some(c) = re.captures(node.value().attr("onclick").unwrap_or("")) {
            let key = c
                .get(1)
                .or_else(|| c.get(2))
                .map(|m| m.as_str().to_string());
            if let Some(key) = key {
                require(
                    analysis_key.as_ref().is_none_or(|k| k == &key),
                    "prestudy_detail_unqualified",
                )?;
                analysis_key = Some(key);
            }
        }
    }
    require(
        analysis_key.as_ref().is_none_or(|k| key_pattern(k)),
        "prestudy_detail_unqualified",
    )?;
    Ok(PrestudyDetail {
        prestudy_key: source_key,
        unit_label: field("sunit_name")?,
        date_literal: field("create_datetime")?,
        analysis_key,
        encoding_result: field("encoding_result")?
            .map(|v| clean(&v))
            .filter(|v| !v.is_empty()),
    })
}

/// Backend-equivalent re-binding of the detail to the requested record: the
/// observed key must equal the request before its source record is used.
#[doc(hidden)]
pub fn bind_source_record(
    detail: &PrestudyDetail,
    prestudy_key: &str,
) -> Result<PrestudySourceRecord> {
    require(
        detail.prestudy_key == prestudy_key,
        "prestudy_report_scope_conflict",
    )?;
    Ok(PrestudySourceRecord {
        prestudy_key: detail.prestudy_key.clone(),
        unit_label: detail.unit_label.clone(),
        date_literal: detail.date_literal.clone(),
    })
}

/// The assembled observation key must equal the requested key; a divergent
/// assembled result is unqualified, never projected.
#[doc(hidden)]
pub fn qualify_report_key(observed_key: &str, prestudy_key: &str) -> Result<()> {
    require(
        observed_key == prestudy_key,
        "prestudy_report_result_unqualified",
    )
}

/// `blockText` of `prestudyRead.ts:141-154`: text nodes appended, `BR` gives
/// a newline without descent, `SCRIPT` and `STYLE` are skipped with their
/// descendants, and the block newline follows the children.
fn block_text(element: scraper::ElementRef<'_>) -> String {
    fn walk(el: scraper::ElementRef<'_>, out: &mut String) {
        let tag = el.value().name().to_ascii_uppercase();
        if tag == "BR" {
            out.push('\n');
            return;
        }
        if tag == "SCRIPT" || tag == "STYLE" {
            return;
        }
        for child in el.children() {
            match child.value() {
                scraper::Node::Text(text) => out.push_str(text),
                scraper::Node::Element(_) => {
                    if let Some(child) = scraper::ElementRef::wrap(child) {
                        walk(child, out);
                    }
                }
                _ => {}
            }
        }
        if matches!(
            tag.as_str(),
            "P" | "DIV" | "LI" | "TR" | "H1" | "H2" | "H3" | "H4" | "H5" | "H6"
        ) {
            out.push('\n');
        }
    }
    let mut out = String::new();
    walk(element, &mut out);
    out.split('\n')
        .map(clean)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The AI report page is fully server-rendered (`prestudyRead.ts:161-195`):
/// one card per section, `h3` narrative blocks, inline chart scores. No
/// transcript field exists on this page and none is derived.
pub fn parse_report(html: &str) -> Result<PrestudyReport> {
    reject_expired(html)?;
    if html.contains("리포트를 생성할 수 없습니다") {
        return Err(Error::refusal("prestudy_report_not_ready"));
    }
    let doc = Document::parse(html);
    let mut sections = Vec::new();
    let mut total_units = 0usize;
    for card in doc.select("div") {
        let class = card.value().attr("class").unwrap_or("");
        if !class.contains("max-w-5xl")
            || !class.contains("rounded-2xl")
            || dom::select(card, "h1").is_empty()
        {
            continue;
        }
        let title = clean(&dom::content(*dom::select(card, "h1").first().unwrap()));
        if title.is_empty() {
            continue;
        }
        let subtitle = dom::select(card, "h2")
            .first()
            .map(|h| clean(&dom::content(*h)))
            .filter(|s| !s.is_empty());
        let has_h3 = !dom::select(card, "h3").is_empty();
        let mut blocks: Vec<PrestudyReportBlock> = Vec::new();
        for el in dom::select(card, "h3, div.text-sm, p") {
            if el.value().name() == "h3" {
                blocks.push(PrestudyReportBlock {
                    heading: {
                        let h = clean(&dom::content(el));
                        (!h.is_empty()).then_some(h)
                    },
                    text: String::new(),
                });
                continue;
            }
            if el.value().name() == "p" && has_h3 {
                continue;
            }
            let text = block_text(el);
            if text.is_empty() {
                continue;
            }
            if blocks.is_empty() {
                blocks.push(PrestudyReportBlock {
                    heading: None,
                    text: String::new(),
                });
            }
            let current = blocks.last_mut().unwrap();
            if current.text.is_empty() {
                current.text = text;
            } else {
                current.text = format!("{}\n{}", current.text, text);
            }
        }
        let kept: Vec<_> = blocks.into_iter().filter(|b| !b.text.is_empty()).collect();
        total_units += kept.iter().map(|b| utf16_len(&b.text)).sum::<usize>();
        if !kept.is_empty() {
            sections.push(PrestudyReportSection {
                title,
                subtitle,
                blocks: kept,
            });
        }
    }
    require(!sections.is_empty(), "prestudy_report_unqualified")?;
    require(
        total_units <= MAX_REPORT_TEXT_UNITS,
        "prestudy_report_source_rejected",
    )?;
    let inline: String = doc
        .select("script")
        .iter()
        .filter(|s| s.value().attr("src").is_none())
        .map(|s| dom::content(*s))
        .collect::<Vec<_>>()
        .join("\n");
    let ws = JS_WS_CLASS;
    // `\bscores\s*=\s*...` with JavaScript's ASCII `\b` and the S1 `\s`.
    let object_re =
        regex::Regex::new(&format!(r"(?-u:\b)scores{ws}*={ws}*\{{([^}}]*)\}}")).expect("constant");
    let array_re =
        regex::Regex::new(&format!(r"(?-u:\b)scores{ws}*={ws}*\[([^\]]*)\]")).expect("constant");
    let (capture, in_object) = match object_re.captures(&inline) {
        Some(c) => (c[1].to_string(), true),
        None => match array_re.captures(&inline) {
            Some(c) => (c[1].to_string(), false),
            None => (String::new(), false),
        },
    };
    let number_re = if in_object {
        regex::Regex::new(&format!(r":{ws}*(-?[0-9]+(?:\.[0-9]+)?)")).expect("constant")
    } else {
        regex::Regex::new(r"-?[0-9]+(?:\.[0-9]+)?").expect("constant")
    };
    let scores: Vec<f64> = number_re
        .captures_iter(&capture)
        .filter_map(|c| {
            c.get(1)
                .or_else(|| c.get(0))
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .filter(|n| n.is_finite())
        })
        .take(20)
        .collect();
    Ok(PrestudyReport { sections, scores })
}

/// The backend shape limits, enforced on the assembled report observation
/// exactly where the TS backend enforces them; a violation is the Zod
/// refusal TS produces (`source_response_malformed`). Public (hidden) so the
/// limit table test can reach limits the parser cannot produce, such as more
/// than 20 scores.
#[doc(hidden)]
pub fn check_report_shape(observation: &PrestudyReportObservation) -> Result<()> {
    let code = "source_response_malformed";
    require(
        (1..=MAX_SECTIONS).contains(&observation.sections.len()),
        code,
    )?;
    for section in &observation.sections {
        require(
            !section.title.is_empty() && utf16_len(&section.title) <= MAX_SECTION_TITLE_UNITS,
            code,
        )?;
        require(
            section
                .subtitle
                .as_ref()
                .is_none_or(|s| utf16_len(s) <= MAX_SECTION_SUBTITLE_UNITS),
            code,
        )?;
        require(
            (1..=MAX_BLOCKS_PER_SECTION).contains(&section.blocks.len()),
            code,
        )?;
        for block in &section.blocks {
            require(
                !block.text.is_empty() && utf16_len(&block.text) <= MAX_BLOCK_TEXT_UNITS,
                code,
            )?;
            require(
                block
                    .heading
                    .as_ref()
                    .is_none_or(|h| utf16_len(h) <= MAX_BLOCK_HEADING_UNITS),
                code,
            )?;
        }
    }
    let record = &observation.source_record;
    require(
        record
            .unit_label
            .as_ref()
            .is_none_or(|v| !v.is_empty() && utf16_len(v) <= MAX_SOURCE_UNIT_LABEL_UNITS),
        code,
    )?;
    require(
        record
            .date_literal
            .as_ref()
            .is_none_or(|v| !v.is_empty() && utf16_len(v) <= MAX_SOURCE_DATE_LITERAL_UNITS),
        code,
    )?;
    require(
        observation
            .encoding_result
            .as_ref()
            .is_none_or(|v| utf16_len(v) <= MAX_ENCODING_RESULT_UNITS),
        code,
    )?;
    require(observation.scores.len() <= 20, code)?;
    Ok(())
}

/// Exact-title selection in source order. A requested title that occurs
/// twice in the source is ambiguous and refuses with no narrative selected.
pub fn select_report_view(
    report: &PrestudyReport,
    requested: Option<&[String]>,
) -> Result<(Vec<PrestudyReportSection>, PrestudyReportView)> {
    let available: Vec<String> = report.sections.iter().map(|s| s.title.clone()).collect();
    let Some(requested) = requested else {
        return Ok((
            report.sections.clone(),
            PrestudyReportView {
                selected: false,
                requested_titles: None,
                available_titles: available.clone(),
                returned_titles: available.clone(),
                missing_titles: vec![],
                complete_requested_view: true,
                complete_source_report: true,
            },
        ));
    };
    require(
        !requested.iter().any(|t| {
            available
                .iter()
                .filter(|a| a.as_str() == t.as_str())
                .count()
                > 1
        }),
        "prestudy_report_section_ambiguous",
    )?;
    let missing: Vec<String> = requested
        .iter()
        .filter(|t| !available.contains(t))
        .cloned()
        .collect();
    let sections: Vec<_> = report
        .sections
        .iter()
        .filter(|s| requested.contains(&s.title))
        .cloned()
        .collect();
    let view = PrestudyReportView {
        selected: true,
        requested_titles: Some(requested.to_vec()),
        available_titles: available,
        returned_titles: sections.iter().map(|s| s.title.clone()).collect(),
        missing_titles: missing.clone(),
        complete_requested_view: missing.is_empty(),
        complete_source_report: sections.len() == report.sections.len(),
    };
    Ok((sections, view))
}

/// UTF-8 byte length of one JSON payload. E11b measures the assembled tool
/// result with this helper and refuses the whole result over budget; source
/// text is never clipped.
pub fn utf8_json_len(payload: &str) -> usize {
    payload.len()
}

/// The MCP-layer budget rule, exposed here so E11b applies the exact code.
pub fn check_report_budget(payload_utf8_len: usize, max_output_bytes: usize) -> Result<()> {
    require(
        payload_utf8_len <= max_output_bytes,
        "prestudy_report_result_too_large",
    )
}

/// Owner port for the five prestudy fetches. Every method receives the body
/// bound for the read; implementations own authentication and transport and
/// stop at the bound, and this layer still checks what it receives.
pub trait PrestudyPort {
    /// The admitted transport principal key, when the port carries one.
    fn principal_key(&self) -> Option<&str> {
        None
    }
    fn shell(&mut self, kind: ListKind, bound: usize) -> Result<Response>;
    fn search(
        &mut self,
        kind: ListKind,
        form: &PrestudySearchForm,
        student_name: &str,
        bound: usize,
    ) -> Result<Response>;
    fn summary(&mut self, bound: usize) -> Result<Response>;
    fn detail(&mut self, prestudy_key: &str, bound: usize) -> Result<Response>;
    fn report(&mut self, analysis_key: &str, bound: usize) -> Result<Response>;
}

/// Prestudy port over an admitted `ReadPort` session. Each fetch checks its
/// exact operation contract before the read; a missing contract is the
/// source-port absence the TS backend reports, never a guessed request.
pub struct SessionPrestudyPort<'a, P: ReadPort> {
    port: &'a mut P,
    scope: TransportScope,
}
impl<'a, P: ReadPort> SessionPrestudyPort<'a, P> {
    pub fn new(port: &'a mut P, scope: TransportScope) -> Self {
        Self { port, scope }
    }
    fn read(
        &mut self,
        operation: &str,
        args: &[(String, String)],
        bound: usize,
    ) -> Result<Response> {
        let code = if operation.starts_with("prestudy.report")
            || operation.starts_with("prestudy.detail")
        {
            "prestudy_report_source_port_unavailable"
        } else {
            "prestudy_source_port_unavailable"
        };
        check_contract(self.port.transport_admission(), operation)
            .map_err(|_| Error::refusal(code))?;
        let response = self.port.read(&self.scope, operation, args)?;
        require(response.bytes.len() <= bound, "prestudy_source_rejected")?;
        Ok(response)
    }
}
impl<P: ReadPort> PrestudyPort for SessionPrestudyPort<'_, P> {
    fn principal_key(&self) -> Option<&str> {
        Some(&self.scope.teacher_id)
    }
    fn shell(&mut self, kind: ListKind, bound: usize) -> Result<Response> {
        self.read(kind.shell_operation(), &[], bound)
    }
    fn search(
        &mut self,
        kind: ListKind,
        form: &PrestudySearchForm,
        student_name: &str,
        bound: usize,
    ) -> Result<Response> {
        self.read(
            kind.search_operation(),
            &[
                ("g_clg_no".into(), form.college_key.clone()),
                ("g_mem_type".into(), form.member_type.clone()),
                ("student_name".into(), student_name.to_string()),
            ],
            bound,
        )
    }
    fn summary(&mut self, bound: usize) -> Result<Response> {
        self.read("prestudy.summary", &[], bound)
    }
    fn detail(&mut self, prestudy_key: &str, bound: usize) -> Result<Response> {
        self.read(
            "prestudy.detail",
            &[("prestudy_key".into(), prestudy_key.to_string())],
            bound,
        )
    }
    fn report(&mut self, analysis_key: &str, bound: usize) -> Result<Response> {
        self.read(
            "prestudy.report",
            &[("analysis_key".into(), analysis_key.to_string())],
            bound,
        )
    }
}

/// One bounded page body: at most 8,000,000 bytes, strict UTF-8, at most
/// 2,000,000 UTF-16 units (the crate body rule), then the leading U+FEFF is
/// removed. Strict decoding is a declared difference from the lossy TS
/// decode: invalid UTF-8 refuses instead of substituting characters.
fn page_text(response: Response, bound: usize) -> Result<String> {
    require(
        response.status == 200 && !response.stalled,
        "prestudy_source_rejected",
    )?;
    require(response.bytes.len() <= bound, "prestudy_source_rejected")?;
    let text = decode_bounded(&response.bytes, MAX_SOURCE_BYTES, MAX_SOURCE_UNITS)
        .map_err(|_| Error::refusal("prestudy_source_rejected"))?;
    Ok(text.strip_prefix('\u{FEFF}').unwrap_or(&text).to_string())
}

pub struct PrestudySource<P: PrestudyPort> {
    port: P,
    principal_label: String,
    teacher_key: String,
    requests: usize,
}
impl<P: PrestudyPort> PrestudySource<P> {
    pub fn new(
        port: P,
        principal_label: impl Into<String>,
        teacher_key: impl Into<String>,
    ) -> Self {
        Self {
            port,
            principal_label: principal_label.into(),
            teacher_key: teacher_key.into(),
            requests: 0,
        }
    }
    pub fn requests(&self) -> usize {
        self.requests
    }
    fn principal_lists(&self, teacher_label: &str) -> Result<()> {
        require(
            teacher_label == self.principal_label,
            "prestudy_principal_rejected",
        )?;
        if let Some(key) = self.port.principal_key() {
            require(key == self.teacher_key, "prestudy_principal_rejected")?;
        }
        Ok(())
    }
    fn principal_report(&self, teacher_label: &str) -> Result<()> {
        require(
            teacher_label == self.principal_label,
            "prestudy_report_principal_rejected",
        )?;
        if let Some(key) = self.port.principal_key() {
            require(
                key == self.teacher_key,
                "prestudy_report_principal_rejected",
            )?;
        }
        Ok(())
    }
    /// Waiting, completed and summary surfaces for one exact student, read
    /// in that order. An observed empty surface stays observed.
    pub fn read_lists(
        &mut self,
        teacher_label: &str,
        student_name: &str,
        observed_at: &str,
    ) -> Result<PrestudyLists> {
        self.principal_lists(teacher_label)?;
        let student_name = literal_name(student_name)?;
        let mut lists = Vec::new();
        for kind in [ListKind::Waiting, ListKind::Completed] {
            let shell = page_text(self.port.shell(kind, MAX_SOURCE_BYTES)?, MAX_SOURCE_BYTES)?;
            self.requests += 1;
            let form = parse_search_form(&shell, kind, &self.teacher_key)?;
            let result = page_text(
                self.port
                    .search(kind, &form, &student_name, MAX_SOURCE_BYTES)?,
                MAX_SOURCE_BYTES,
            )?;
            self.requests += 1;
            lists.push(rows_for_student(
                &parse_list(&result, kind, &self.teacher_key)?,
                &student_name,
            ));
        }
        let summary_html = page_text(self.port.summary(MAX_SOURCE_BYTES)?, MAX_SOURCE_BYTES)?;
        self.requests += 1;
        let summary = rows_for_student(&parse_summary(&summary_html)?, &student_name);
        Ok(PrestudyLists {
            waiting: lists[0].clone(),
            completed: lists[1].clone(),
            summary,
            teacher_judgment: None,
            observed_at: observed_at.to_string(),
            coverage: "exact_student_rows_on_waiting_completed_and_one_observed_summary_page_not_all_history".into(),
        })
    }
    /// The detail record is bound to the exact student and record before its
    /// analysis key is followed to the report page.
    pub fn read_report(
        &mut self,
        teacher_label: &str,
        student_name: &str,
        student_key: &str,
        prestudy_key: &str,
        observed_at: &str,
    ) -> Result<PrestudyReportObservation> {
        self.principal_report(teacher_label)?;
        // Guards in the TS order (`academyHttpClient.ts:141`): name, then keys.
        literal_name(student_name)?;
        let student_key = literal_key(student_key)?;
        let prestudy_key = literal_key(prestudy_key)?;
        let detail_html = page_text(
            self.port.detail(&prestudy_key, MAX_SOURCE_BYTES)?,
            MAX_SOURCE_BYTES,
        )?;
        self.requests += 1;
        let detail = parse_detail(&detail_html, &student_key, &prestudy_key)?;
        let analysis_key = detail
            .analysis_key
            .clone()
            .ok_or_else(|| Error::refusal("prestudy_report_not_ready"))?;
        let report_html = page_text(
            self.port.report(&analysis_key, MAX_SOURCE_BYTES)?,
            MAX_SOURCE_BYTES,
        )?;
        self.requests += 1;
        let report = parse_report(&report_html)?;
        qualify_report_key(&detail.prestudy_key, &prestudy_key)?;
        let source_record = bind_source_record(&detail, &prestudy_key)?;
        let observation = PrestudyReportObservation {
            prestudy_key: detail.prestudy_key.clone(),
            source_record,
            sections: report.sections,
            scores: report.scores,
            encoding_result: detail.encoding_result.clone(),
            observed_at: observed_at.to_string(),
            coverage: "exact_prestudy_detail_and_server_rendered_ai_report_sections_not_transcript"
                .into(),
        };
        check_report_shape(&observation)?;
        Ok(observation)
    }
}
