//! Prestudy (예습영상) list, detail and AI-report source parsing (E11a).
//! Mirrors `apps/academy/dc-gang-a/src/lms/prestudyRead.ts` and the prestudy
//! half of `AcademyHttpClient` / `NativeDailyRecordBackend`: waiting and
//! completed list membership, the summary AI/star literals, the exact-record
//! detail form and the server-rendered report cards. Upload, list membership,
//! AI score and teacher review are distinct states; nothing here derives one
//! from another, retains a video URL, or invents a transcript. The UTF-8
//! output budget itself is an MCP-layer rule (E11b); this layer only exposes
//! the view and the byte measurement it is applied to.
use crate::{
    data::*,
    dom::{self, Document},
    http::Response,
    owner_admission::TransportScope,
    student_course::{ReadPort, check_contract, require},
};

pub const PRESTUDY_PATH: &str = "/servlet/controller.coursemanage.CourseManageServlet";
pub const PRESTUDY_REPORT_PATH: &str = "/app-ui/solution_common/prestudy/prestudy_ai_report.jsp";
pub const MAX_SOURCE_CHARS: usize = 2_000_000;
pub const MAX_REPORT_TEXT_CHARS: usize = 60_000;
pub const MAX_ROWS: usize = 1000;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudySearchForm {
    pub req_cmd: String,
    pub college_key: String,
    pub member_type: String,
    pub teacher_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudyListRow {
    pub student_label: String,
    pub unit_label: String,
    pub date_literal: String,
    pub status_literal: String,
    pub prestudy_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudySummaryRow {
    pub student_label: String,
    pub unit_label: String,
    pub date_literal: String,
    pub ai_score_literal: String,
    pub rating_literal: String,
    pub report_available: bool,
    pub video_available: bool,
}

/// The three observed surfaces for one exact student. `teacher_judgment` is
/// always `None`: a source OK or AI score is never a teacher judgment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudyLists {
    pub waiting: Vec<PrestudyListRow>,
    pub completed: Vec<PrestudyListRow>,
    pub summary: Vec<PrestudySummaryRow>,
    pub teacher_judgment: Option<()>,
    pub observed_at: String,
    pub coverage: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudyDetail {
    pub prestudy_key: String,
    pub unit_label: Option<String>,
    pub date_literal: Option<String>,
    pub analysis_key: Option<String>,
    pub encoding_result: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudySourceRecord {
    pub prestudy_key: String,
    pub unit_label: Option<String>,
    pub date_literal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudyReportBlock {
    pub heading: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudyReportSection {
    pub title: String,
    pub subtitle: Option<String>,
    pub blocks: Vec<PrestudyReportBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrestudyReport {
    pub sections: Vec<PrestudyReportSection>,
    pub scores: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrestudyReportObservation {
    pub prestudy_key: String,
    pub source_record: PrestudySourceRecord,
    pub sections: Vec<PrestudyReportSection>,
    pub scores: Vec<f64>,
    pub encoding_result: Option<String>,
    pub observed_at: String,
    pub coverage: String,
}

/// Exact-title selection over the observed sections, in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrestudyReportView {
    pub selected: bool,
    pub requested_titles: Option<Vec<String>>,
    pub available_titles: Vec<String>,
    pub returned_titles: Vec<String>,
    pub missing_titles: Vec<String>,
    pub complete_requested_view: bool,
    pub complete_source_report: bool,
}

fn clean(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn reject_expired(html: &str) -> Result<()> {
    let doc_password = Document::parse(html).select("input").iter().any(|e| {
        e.value()
            .attr("type")
            .is_some_and(|t| t.eq_ignore_ascii_case("password"))
    });
    if html.contains("normal_session_error.jsp")
        || html.contains("popup_session_error.jsp")
        || html.contains("세션이 만료")
        || doc_password
    {
        return Err(Error::refusal("prestudy_authentication_required"));
    }
    require(
        html.encode_utf16().count() <= MAX_SOURCE_CHARS,
        "prestudy_source_rejected",
    )
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && !value.starts_with('0')
        && value.bytes().all(|b| b.is_ascii_digit())
        && value
            .parse::<u64>()
            .is_ok_and(|n| n <= 9_007_199_254_740_991)
}

fn field(form: scraper::ElementRef<'_>, name: &str) -> Option<String> {
    dom::select(form, &format!("input[name=\"{name}\"]"))
        .first()
        .map(|n| dom::value(*n))
}

/// The search form is the wire contract: hidden scope fields are read back
/// from the live page, never reconstructed from caller input.
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
            && !dom::select(*form, "input[name=\"student_name\"]").is_empty(),
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

fn onclick_text(row: scraper::ElementRef<'_>) -> String {
    dom::select(row, "[onclick]")
        .iter()
        .map(|n| n.value().attr("onclick").unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_list(
    html: &str,
    expected: ListKind,
    teacher_key: &str,
) -> Result<Vec<PrestudyListRow>> {
    parse_search_form(html, expected, teacher_key)?;
    let doc = Document::parse(html);
    let re = regex::Regex::new(r"prestudy_main\(\s*([0-9]+)\s*\)").expect("constant");
    let mut rows = Vec::new();
    for row in doc.select("tr") {
        let handler = onclick_text(row);
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

/// Exact-name match: the plain label, or the label with a source suffix in
/// parentheses. Never a substring match.
pub fn rows_for_student<T: HasStudentLabel + Clone>(rows: &[T], student_name: &str) -> Vec<T> {
    rows.iter()
        .filter(|r| {
            r.student_label() == student_name
                || r.student_label().starts_with(&format!("{student_name}("))
        })
        .cloned()
        .collect()
}

pub trait HasStudentLabel {
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

/// One uploaded-video detail record. It is bound to the exact student and
/// requested record through its own form fields; the AI report is reachable
/// only through the rendered `viewAiReport(analysis_key)` handler.
pub fn parse_detail(html: &str, student_key: &str, prestudy_key: &str) -> Result<PrestudyDetail> {
    reject_expired(html)?;
    let doc = Document::parse(html);
    let forms = doc.select("form[name=\"form1\"]");
    require(forms.len() == 1, "prestudy_detail_unqualified")?;
    let form = forms[0];
    let field = |name: &str| -> Result<Option<String>> {
        let nodes = dom::select(form, &format!("input[name=\"{name}\"]"));
        require(nodes.len() <= 1, "prestudy_detail_unqualified")?;
        Ok(nodes.first().and_then(|n| {
            let v = dom::value(*n);
            let v = v.trim();
            (!v.is_empty()).then(|| v.to_string())
        }))
    };
    let student_field = field("student_pri_no")?;
    let source_key = field("prestudy_key")?;
    require(
        student_field.is_some() && source_key.as_ref().is_some_and(|k| valid_key(k)),
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
    let re =
        regex::Regex::new(r#"viewAiReport\(\s*'([^']+)'\s*\)|viewAiReport\(\s*"([^"]+)"\s*\)"#)
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
        analysis_key.as_ref().is_none_or(|k| valid_key(k)),
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
pub fn qualify_report_key(observed_key: &str, prestudy_key: &str) -> Result<()> {
    require(
        observed_key == prestudy_key,
        "prestudy_report_result_unqualified",
    )
}

fn block_text(element: scraper::ElementRef<'_>) -> String {
    let mut out = String::new();
    for node in element.descendants() {
        match node.value() {
            scraper::Node::Text(text) => out.push_str(text),
            scraper::Node::Element(el) => {
                let tag = el.name();
                if tag.eq_ignore_ascii_case("br") {
                    out.push('\n');
                } else if tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("style") {
                    // Script/style text is not report narrative; the inline
                    // scores script is read separately from the raw source.
                }
            }
            _ => {}
        }
        if let Some(el) = node.value().as_element()
            && matches!(
                el.name().to_ascii_uppercase().as_str(),
                "P" | "DIV" | "LI" | "TR" | "H1" | "H2" | "H3" | "H4" | "H5" | "H6"
            )
        {
            out.push('\n');
        }
    }
    out.split('\n')
        .map(clean)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The AI report page is fully server-rendered: one card per section with an
/// `h1` title, an optional `h2` question, `h3`-headed narrative blocks and
/// the chart score array in an inline script. No transcript field exists.
pub fn parse_report(html: &str) -> Result<PrestudyReport> {
    reject_expired(html)?;
    if html.contains("리포트를 생성할 수 없습니다") {
        return Err(Error::refusal("prestudy_report_not_ready"));
    }
    let doc = Document::parse(html);
    let mut sections = Vec::new();
    let mut total_chars = 0usize;
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
        total_chars += kept
            .iter()
            .map(|b| b.text.encode_utf16().count())
            .sum::<usize>();
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
        total_chars <= MAX_REPORT_TEXT_CHARS,
        "prestudy_report_source_rejected",
    )?;
    let inline: String = doc
        .select("script")
        .iter()
        .filter(|s| s.value().attr("src").is_none())
        .map(|s| dom::content(*s))
        .collect::<Vec<_>>()
        .join("\n");
    let object_re = regex::Regex::new(r"\bscores\s*=\s*\{([^}]*)\}").expect("constant");
    let array_re = regex::Regex::new(r"\bscores\s*=\s*\[([^\]]*)\]").expect("constant");
    let (capture, in_object) = match object_re.captures(&inline) {
        Some(c) => (c[1].to_string(), true),
        None => match array_re.captures(&inline) {
            Some(c) => (c[1].to_string(), false),
            None => (String::new(), false),
        },
    };
    let number_re = if in_object {
        regex::Regex::new(r":\s*(-?\d+(?:\.\d+)?)").expect("constant")
    } else {
        regex::Regex::new(r"-?\d+(?:\.\d+)?").expect("constant")
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

/// Owner port for the five prestudy fetches. Implementations own
/// authentication and transport; this module owns every qualification,
/// bound and parse decision after bytes arrive.
pub trait PrestudyPort {
    fn shell(&mut self, kind: ListKind) -> Result<Response>;
    fn search(
        &mut self,
        kind: ListKind,
        form: &PrestudySearchForm,
        student_name: &str,
    ) -> Result<Response>;
    fn summary(&mut self) -> Result<Response>;
    fn detail(&mut self, prestudy_key: &str) -> Result<Response>;
    fn report(&mut self, analysis_key: &str) -> Result<Response>;
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
    fn read(&mut self, operation: &str, args: &[(String, String)]) -> Result<Response> {
        let code = if operation.starts_with("prestudy.report")
            || operation.starts_with("prestudy.detail")
        {
            "prestudy_report_source_port_unavailable"
        } else {
            "prestudy_source_port_unavailable"
        };
        check_contract(self.port.transport_admission(), operation)
            .map_err(|_| Error::refusal(code))?;
        self.port.read(&self.scope, operation, args)
    }
}
impl<P: ReadPort> PrestudyPort for SessionPrestudyPort<'_, P> {
    fn shell(&mut self, kind: ListKind) -> Result<Response> {
        self.read(kind.shell_operation(), &[])
    }
    fn search(
        &mut self,
        kind: ListKind,
        form: &PrestudySearchForm,
        student_name: &str,
    ) -> Result<Response> {
        self.read(
            kind.search_operation(),
            &[
                ("g_clg_no".into(), form.college_key.clone()),
                ("g_mem_type".into(), form.member_type.clone()),
                ("student_name".into(), student_name.to_string()),
            ],
        )
    }
    fn summary(&mut self) -> Result<Response> {
        self.read("prestudy.summary", &[])
    }
    fn detail(&mut self, prestudy_key: &str) -> Result<Response> {
        self.read(
            "prestudy.detail",
            &[("prestudy_key".into(), prestudy_key.to_string())],
        )
    }
    fn report(&mut self, analysis_key: &str) -> Result<Response> {
        self.read(
            "prestudy.report",
            &[("analysis_key".into(), analysis_key.to_string())],
        )
    }
}

fn page_text(response: Response) -> Result<String> {
    require(
        response.status == 200 && !response.stalled,
        "prestudy_source_rejected",
    )?;
    require(
        response.bytes.len() <= MAX_SOURCE_CHARS,
        "prestudy_source_rejected",
    )?;
    let text = String::from_utf8_lossy(&response.bytes).into_owned();
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
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
    fn principal(&self, teacher_label: &str) -> Result<()> {
        require(
            teacher_label == self.principal_label,
            "prestudy_report_principal_rejected",
        )
    }
    /// Waiting, completed and summary surfaces for one exact student, read in
    /// that order. An observed empty surface stays observed, never unavailable.
    pub fn read_lists(
        &mut self,
        teacher_label: &str,
        student_name: &str,
        observed_at: &str,
    ) -> Result<PrestudyLists> {
        self.principal(teacher_label)?;
        let mut lists = Vec::new();
        for kind in [ListKind::Waiting, ListKind::Completed] {
            let shell = page_text(self.port.shell(kind)?)?;
            self.requests += 1;
            let form = parse_search_form(&shell, kind, &self.teacher_key)?;
            let result = page_text(self.port.search(kind, &form, student_name)?)?;
            self.requests += 1;
            lists.push(rows_for_student(
                &parse_list(&result, kind, &self.teacher_key)?,
                student_name,
            ));
        }
        let summary_html = page_text(self.port.summary()?)?;
        self.requests += 1;
        let summary = rows_for_student(&parse_summary(&summary_html)?, student_name);
        Ok(PrestudyLists {
            waiting: lists[0].clone(),
            completed: lists[1].clone(),
            summary,
            teacher_judgment: None,
            observed_at: observed_at.to_string(),
            coverage:
                "exact_student_rows_on_waiting_completed_and_one_observed_summary_page_not_all_history"
                    .into(),
        })
    }
    /// The detail record is bound to the exact student and record before its
    /// analysis key is followed to the report page.
    pub fn read_report(
        &mut self,
        teacher_label: &str,
        student_key: &str,
        prestudy_key: &str,
        observed_at: &str,
    ) -> Result<PrestudyReportObservation> {
        self.principal(teacher_label)?;
        require(valid_key(prestudy_key), "prestudy_detail_unqualified")?;
        let detail_html = page_text(self.port.detail(prestudy_key)?)?;
        self.requests += 1;
        let detail = parse_detail(&detail_html, student_key, prestudy_key)?;
        let analysis_key = detail
            .analysis_key
            .clone()
            .ok_or_else(|| Error::refusal("prestudy_report_not_ready"))?;
        let report_html = page_text(self.port.report(&analysis_key)?)?;
        self.requests += 1;
        let report = parse_report(&report_html)?;
        qualify_report_key(&detail.prestudy_key, prestudy_key)?;
        let source_record = bind_source_record(&detail, prestudy_key)?;
        Ok(PrestudyReportObservation {
            prestudy_key: detail.prestudy_key.clone(),
            source_record,
            sections: report.sections,
            scores: report.scores,
            encoding_result: detail.encoding_result.clone(),
            observed_at: observed_at.to_string(),
            coverage: "exact_prestudy_detail_and_server_rendered_ai_report_sections_not_transcript"
                .into(),
        })
    }
}
