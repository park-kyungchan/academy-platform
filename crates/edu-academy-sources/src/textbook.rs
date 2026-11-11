//! Textbook catalogue and PDF custody source APIs (E10a-1).
//! The catalogue is read through an admitted Academy operation; a PDF is
//! fetched only after its exact document reference is found in a fresh
//! catalogue read, from the qualified public storage URL and without any
//! Academy credential. No PDF bytes are extracted or interpreted here:
//! custody preserves the original bytes under a caller-supplied root and
//! this layer reports identity, size and digest only.
use crate::{
    data::*,
    dom::{self, Document},
    js_text::{js_collapse_whitespace, js_trim, utf16_len},
    owner_admission::TransportScope,
    student_course::{ReadPort, check_contract},
};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

pub const CATALOG_PATH: &str = "/servlet/controller.coursemanage.CourseManageServlet";
pub const MAX_CATALOG_BYTES: usize = 2_000_000;
pub const MAX_SAMPLE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_REFERENCE_BYTES: usize = 64 * 1024 * 1024;

/// Body bound for one catalogue read (mirrors `MAX_TEXTBOOK_CATALOG_BYTES`,
/// apps/academy/dc-gang-a/src/lms/textbookCatalogRead.ts:4).
pub fn catalog_body_limit() -> usize {
    MAX_CATALOG_BYTES
}
/// Body bound for one PDF by document kind: the 32 MiB sample cap
/// (textbookCatalogRead.ts:5) or the 64 MiB reference cap (:6), selected at
/// apps/academy/dc-gang-a/src/lms/academyHttpClient.ts:105.
pub fn pdf_body_limit(kind: DocumentKind) -> usize {
    if kind == DocumentKind::Sample {
        MAX_SAMPLE_BYTES
    } else {
        MAX_REFERENCE_BYTES
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchoolLevel {
    Elementary,
    Middle,
    High,
}
impl SchoolLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Elementary => "elementary",
            Self::Middle => "middle",
            Self::High => "high",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Elementary => "초등부",
            Self::Middle => "중등부",
            Self::High => "고등부",
        }
    }
    fn catalog_operation(self) -> &'static str {
        match self {
            Self::Elementary => "textbook.catalog.elementary",
            Self::Middle => "textbook.catalog",
            Self::High => "textbook.catalog.high",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Sample,
    Answer,
    Repair,
    Daily,
    Other,
}
impl DocumentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::Answer => "answer",
            Self::Repair => "repair",
            Self::Daily => "daily",
            Self::Other => "other",
        }
    }
    fn from_filename(filename: &str) -> Self {
        if filename.contains("_sample_") {
            Self::Sample
        } else if filename.contains("_answer_") || filename.contains("_answer.") {
            Self::Answer
        } else if filename.contains("_repair_") || filename.contains("_repair.") {
            Self::Repair
        } else if filename.contains("_daily_") || filename.contains("_daily.") {
            Self::Daily
        } else {
            Self::Other
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    pub kind: DocumentKind,
    pub document_ref: String,
    pub url: String,
    pub filename: String,
    pub source_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogObservation {
    pub schema_version: u8,
    pub status: String,
    pub school_level: SchoolLevel,
    pub observed_at: String,
    pub catalog_sha256: String,
    pub entries: Vec<CatalogEntry>,
    pub coverage: String,
    pub source_requests: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfMeta {
    pub school_level: SchoolLevel,
    pub kind: DocumentKind,
    pub document_ref: String,
    pub filename: String,
    pub source_url: String,
    pub source_labels: Vec<String>,
    pub catalog_sha256: String,
    pub observed_at: String,
    pub sha256: String,
    pub byte_length: usize,
    pub source_requests: usize,
}

/// One downloaded original. `bytes` exist only so `preserve_pdf` (and the
/// E10b adapter through `download_pdf`, which drops them) can custody the
/// exact original; no text is read from them in this layer.
#[derive(Clone, PartialEq, Eq)]
pub struct PdfObservation {
    pub meta: PdfMeta,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub custody: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreservedPdf {
    pub meta: PdfMeta,
    pub artifact: PdfArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogPage {
    pub entries: Vec<CatalogEntry>,
    pub total_observed_pdf_links: usize,
    pub total_matched: usize,
    pub returned: usize,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogPageRequest {
    pub document_kind: Option<DocumentKind>,
    pub query: Option<String>,
}

/// Transport-neutral response for both textbook fetches. The Academy
/// catalogue fetch is authenticated by the port owner; the storage fetch
/// must be made without any Academy credential or cookie.
#[derive(Clone)]
pub struct TextbookResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub bytes: Vec<u8>,
    pub final_url: Option<String>,
    pub redirected: bool,
    /// What the transport actually applied, reported by the port so the
    /// layer can refuse a response that silently used a session or
    /// followed a redirect (finding 2, rest).
    pub credentials_sent: bool,
    pub redirect_followed: bool,
}
impl TextbookResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Explicit per-request fetch policy carried by the port contract, so an
/// implementation can stop reading at `max_bytes` and cannot silently
/// attach a session or follow a redirect (TS storage fetch:
/// `credentials:'omit', redirect:'manual'`, academyHttpClient.ts:102).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchPolicy {
    pub send_credentials: bool,
    pub follow_redirects: bool,
}
/// Storage fetches never carry an Academy credential or cookie.
pub const STORAGE_FETCH: FetchPolicy = FetchPolicy {
    send_credentials: false,
    follow_redirects: false,
};
/// Catalogue fetches ride the authenticated owner session, unredirected.
pub const CATALOG_FETCH: FetchPolicy = FetchPolicy {
    send_credentials: true,
    follow_redirects: false,
};

/// Owner port for the two textbook fetches, mirroring the TS owner client.
/// Implementations own authentication and transport; this module owns every
/// qualification, bound and parse decision after bytes arrive. Each method
/// receives its body bound and policy; a body over the bound is refused
/// with the catalogued size code whether the port stopped early (by
/// returning that code) or returned too many bytes.
pub trait TextbookPort {
    fn catalog(
        &mut self,
        school_level: SchoolLevel,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse>;
    fn storage(
        &mut self,
        url: &str,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse>;
}

/// Catalogue port over an admitted `ReadPort` session. The supplied scope
/// only authorizes transport (the catalogue itself is not student-bound);
/// storage is not available through an Academy session and refuses here.
pub struct SessionTextbookPort<'a, P: ReadPort> {
    port: &'a mut P,
    scope: TransportScope,
}
impl<'a, P: ReadPort> SessionTextbookPort<'a, P> {
    pub fn new(port: &'a mut P, scope: TransportScope) -> Self {
        Self { port, scope }
    }
}
impl<P: ReadPort> TextbookPort for SessionTextbookPort<'_, P> {
    fn catalog(
        &mut self,
        school_level: SchoolLevel,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        // This session always sends its owner credential and never follows
        // a redirect; a caller policy asking for anything else cannot be
        // honoured here and refuses instead of being ignored.
        if !policy.send_credentials || policy.follow_redirects {
            return Err(Error::refusal("textbook_catalog_transport_failed"));
        }
        let operation = school_level.catalog_operation();
        check_contract(self.port.transport_admission(), operation)?;
        let crate::http::Response {
            status,
            headers,
            bytes,
            url,
            ..
        } = self.port.read(&self.scope, operation, &[])?;
        if bytes.len() > max_bytes {
            return Err(Error::refusal("textbook_catalog_size_rejected"));
        }
        Ok(TextbookResponse {
            status,
            headers,
            bytes,
            final_url: (!url.is_empty()).then_some(url),
            redirected: false,
            credentials_sent: true,
            redirect_followed: false,
        })
    }
    fn storage(
        &mut self,
        _url: &str,
        _max_bytes: usize,
        _policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        Err(Error::refusal("textbook_pdf_source_port_unavailable"))
    }
}

pub struct TextbookSource<P: TextbookPort> {
    port: P,
    principal_label: String,
    requests: usize,
}
impl<P: TextbookPort> TextbookSource<P> {
    pub fn new(port: P, principal_label: impl Into<String>) -> Self {
        Self {
            port,
            principal_label: principal_label.into(),
            requests: 0,
        }
    }
    pub fn requests(&self) -> usize {
        self.requests
    }
    pub fn read_catalog(
        &mut self,
        teacher_label: &str,
        school_level: SchoolLevel,
        observed_at: &str,
    ) -> Result<CatalogObservation> {
        if teacher_label != self.principal_label {
            return Err(Error::refusal("textbook_catalog_principal_rejected"));
        }
        let response = self
            .port
            .catalog(school_level, catalog_body_limit(), CATALOG_FETCH)?;
        self.requests += 1;
        if response.credentials_sent != CATALOG_FETCH.send_credentials
            || response.redirect_followed != CATALOG_FETCH.follow_redirects
        {
            return Err(Error::refusal("textbook_catalog_transport_failed"));
        }
        let bytes = bounded_body(response, MAX_CATALOG_BYTES, BodyKind::Catalog)?;
        let html = String::from_utf8(bytes)
            .map_err(|_| Error::refusal("textbook_catalog_response_rejected"))?;
        parse_catalog(&html, school_level, observed_at)
    }
    pub fn fetch_pdf(
        &mut self,
        teacher_label: &str,
        school_level: SchoolLevel,
        document_ref: &str,
        observed_at: &str,
    ) -> Result<PdfObservation> {
        if !is_document_ref(document_ref) {
            return Err(Error::refusal("textbook_pdf_input_rejected"));
        }
        let catalog = self.read_catalog(teacher_label, school_level, observed_at)?;
        let entry = catalog
            .entries
            .iter()
            .find(|e| e.document_ref == document_ref)
            .ok_or_else(|| Error::refusal("textbook_pdf_not_in_current_catalog"))?;
        let url = qualified_pdf_url(&entry.url)?;
        let response = self
            .port
            .storage(&url, pdf_body_limit(entry.kind), STORAGE_FETCH)
            .map_err(|error| {
                if error.message == "textbook_pdf_size_rejected" {
                    error
                } else {
                    Error::refusal("textbook_pdf_transport_failed")
                }
            })?;
        self.requests += 1;
        if response.credentials_sent != STORAGE_FETCH.send_credentials
            || response.redirect_followed != STORAGE_FETCH.follow_redirects
        {
            return Err(Error::refusal("textbook_pdf_transport_failed"));
        }
        if let Some(final_url) = &response.final_url
            && final_url != &url
        {
            return Err(Error::refusal("textbook_pdf_response_url_rejected"));
        }
        let bytes = bounded_body(response, pdf_body_limit(entry.kind), BodyKind::Pdf)?;
        let meta = PdfMeta {
            school_level,
            kind: entry.kind,
            document_ref: entry.document_ref.clone(),
            filename: entry.filename.clone(),
            source_url: entry.url.clone(),
            source_labels: entry.source_labels.clone(),
            catalog_sha256: catalog.catalog_sha256.clone(),
            observed_at: observed_at.to_string(),
            sha256: hash(&bytes),
            byte_length: bytes.len(),
            source_requests: 2,
        };
        Ok(PdfObservation { meta, bytes })
    }
    /// Fetch and custody one exact PDF; the original bytes are dropped here
    /// and only identity, size, digest and the artifact path leave.
    pub fn download_pdf(
        &mut self,
        teacher_label: &str,
        school_level: SchoolLevel,
        document_ref: &str,
        root: &Path,
        observed_at: &str,
    ) -> Result<PreservedPdf> {
        let pdf = self.fetch_pdf(teacher_label, school_level, document_ref, observed_at)?;
        if pdf.meta.document_ref != document_ref || pdf.meta.school_level != school_level {
            return Err(Error::refusal("textbook_pdf_identity_rejected"));
        }
        let artifact = preserve_pdf(root, &pdf)?;
        Ok(PreservedPdf {
            meta: pdf.meta,
            artifact,
        })
    }
}

fn is_document_ref(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Catalog,
    Pdf,
}
impl BodyKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Catalog => "textbook_catalog_",
            Self::Pdf => "textbook_pdf_",
        }
    }
}

/// Bounded body qualification shared by the catalogue and PDF fetches.
/// A compressed body is measured decoded: its declared length is not the
/// decoded length, so the length equality check applies only to identity
/// encodings. Bytes are already materialized by the port; the same bounds
/// refuse an oversized body here.
pub fn bounded_body(response: TextbookResponse, maximum: usize, kind: BodyKind) -> Result<Vec<u8>> {
    let prefix = kind.prefix();
    if response.status != 200 || response.redirected {
        return Err(Error::refusal(&format!("{prefix}response_rejected")));
    }
    let mime = response
        .header("content-type")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("");
    let mime = js_trim(mime).to_ascii_lowercase();
    let mime_ok = match kind {
        BodyKind::Catalog => mime == "text/html",
        BodyKind::Pdf => ["application/pdf", "application/octet-stream"].contains(&mime.as_str()),
    };
    if !mime_ok {
        return Err(Error::refusal(&format!("{prefix}content_type_rejected")));
    }
    let declared = response.header("content-length").map(|raw| {
        raw.trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n >= 1 && *n <= maximum)
    });
    if matches!(declared, Some(None)) {
        return Err(Error::refusal(&format!("{prefix}size_rejected")));
    }
    let declared = declared.flatten();
    let encoded_is_identity = response
        .header("content-encoding")
        .is_none_or(|v| v.trim().eq_ignore_ascii_case("identity"));
    let bytes = response.bytes;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error::refusal(&format!("{prefix}size_rejected")));
    }
    if let Some(size) = declared
        && encoded_is_identity
        && bytes.len() != size
    {
        return Err(Error::refusal(&format!("{prefix}size_rejected")));
    }
    if kind == BodyKind::Pdf {
        let tail_start = bytes.len().saturating_sub(2048);
        if !bytes.starts_with(b"%PDF-") || !bytes[tail_start..].windows(5).any(|w| w == b"%%EOF") {
            return Err(Error::refusal("textbook_pdf_signature_rejected"));
        }
    }
    Ok(bytes)
}

/// Only an actual fixed-catalogue link can reach the public storage
/// transport. The literal must survive URL parsing unchanged.
pub fn qualified_pdf_url(literal: &str) -> Result<String> {
    let url = url::Url::parse(literal).map_err(|_| Error::refusal("textbook_pdf_link_rejected"))?;
    let path_ok = {
        let segments: Vec<&str> = url.path().split('/').collect();
        segments.len() == 6
            && segments[1] == "data"
            && segments[2] == "answer"
            && ["el", "mi", "hi"].contains(&segments[3])
            && ["book", "workbook", "fiveend"].contains(&segments[4])
            && {
                let name = segments[5];
                let mut chars = name.chars();
                name.len() > 4
                    && name.ends_with(".pdf")
                    && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
            }
    };
    if url.as_str() != literal
        || url.port().is_some()
        || url.scheme() != "https"
        || url.host_str() != Some("storage.studyq.net")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !path_ok
    {
        return Err(Error::refusal("textbook_pdf_link_rejected"));
    }
    Ok(url.as_str().to_string())
}

/// HTML rule for parsing non-negative integers: leading whitespace is
/// skipped and leading digits are used, so `" 2"` and `"2x"` are both 2.
fn html_span(value: Option<&str>, maximum: usize) -> usize {
    let digits: String = value
        .unwrap_or("1")
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<usize>().unwrap_or(1).clamp(1, maximum)
}

impl std::fmt::Debug for PdfObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Length and digest only: PDF bytes never appear in debug output.
        write!(
            f,
            "PdfObservation {{ meta: {:?}, bytes_len: {}, bytes_sha256: {} }}",
            self.meta,
            self.bytes.len(),
            hash(&self.bytes)
        )
    }
}
impl std::fmt::Debug for TextbookResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Header names only, never values; body as length and digest.
        let header_names: Vec<&str> = self.headers.iter().map(|(k, _)| k.as_str()).collect();
        write!(
            f,
            "TextbookResponse {{ status: {}, header_names: {:?}, bytes_len: {}, bytes_sha256: {}, redirected: {}, credentials_sent: {}, redirect_followed: {} }}",
            self.status,
            header_names,
            self.bytes.len(),
            hash(&self.bytes),
            self.redirected,
            self.credentials_sent,
            self.redirect_followed
        )
    }
}

/// DOM `closest` includes the element itself; `dom::closest` walks only
/// ancestors, so a `tr` carrying the handler is checked first here.
fn closest_row<'a>(node: scraper::ElementRef<'a>) -> Option<scraper::ElementRef<'a>> {
    if node.value().name() == "tr" {
        Some(node)
    } else {
        dom::closest(node, "tr")
    }
}

fn collapse(text: &str) -> String {
    js_collapse_whitespace(text)
}

/// Rowspan-aware row labels: carried cells keep their logical columns, so a
/// shifted physical row keeps the grade/book label of the rows above it.
fn row_labels(doc: &Document) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    for table in doc.select("table") {
        let mut rows: Vec<_> = dom::select(table, "tr")
            .into_iter()
            .filter(|row| {
                dom::closest(*row, "table")
                    .is_some_and(|t| format!("{:?}", t.id()) == format!("{:?}", table.id()))
            })
            .collect();
        // Match `table.rows` order (TS textbookCatalogRead.ts:90): header
        // rows first, then body rows, then footer rows, each in document
        // order (the sort is stable).
        let section_rank = |row: &scraper::ElementRef<'_>| {
            ["thead", "tbody", "tfoot"].iter().find_map(|tag| {
                dom::closest(*row, tag)
                    .filter(|section| {
                        dom::closest(*section, "table")
                            .is_some_and(|t| format!("{:?}", t.id()) == format!("{:?}", table.id()))
                    })
                    .map(|_| match *tag {
                        "thead" => 0,
                        "tbody" => 1,
                        _ => 2,
                    })
            })
        };
        rows.sort_by_key(|row| section_rank(row).unwrap_or(1));
        let mut spans: HashMap<usize, (String, usize)> = HashMap::new();
        for row in &rows {
            let mut cells: BTreeMap<usize, String> = BTreeMap::new();
            let mut expired = Vec::new();
            for (column, (text, remaining)) in spans.iter_mut() {
                cells.insert(*column, text.clone());
                *remaining -= 1;
                if *remaining == 0 {
                    expired.push(*column);
                }
            }
            for column in expired {
                spans.remove(&column);
            }
            let mut column = 0usize;
            for cell in dom::select(*row, ":scope > td, :scope > th") {
                while cells.contains_key(&column) {
                    column += 1;
                }
                let width = html_span(cell.value().attr("colspan"), 100);
                let height = html_span(cell.value().attr("rowspan"), 1000);
                let text = collapse(&dom::content(cell));
                for i in 0..width {
                    cells.insert(column + i, text.clone());
                    if height > 1 {
                        spans.insert(column + i, (text.clone(), height - 1));
                    }
                }
                column += width;
            }
            labels.insert(
                format!("{:?}", row.id()),
                cells.values().cloned().collect::<Vec<_>>().join(" "),
            );
        }
    }
    labels
}

pub fn parse_catalog(
    html: &str,
    school_level: SchoolLevel,
    observed_at: &str,
) -> Result<CatalogObservation> {
    let lower = html.to_ascii_lowercase();
    let password_input = regex::Regex::new(r#"(?i)<input\b[^>]*type\s*=\s*["']?password"#)
        .expect("constant")
        .is_match(&lower);
    if html.contains("normal_session_error.jsp")
        || html.contains("popup_session_error.jsp")
        || html.contains("세션이 만료")
        || password_input
    {
        return Err(Error::refusal("textbook_catalog_authentication_required"));
    }
    let doc = Document::parse(html);
    let level_labels = ["초등부", "중등부", "고등부"];
    let selected: Vec<_> = doc
        .select(".tab.tab-on")
        .into_iter()
        .filter(|node| level_labels.contains(&js_trim(&dom::content(*node))))
        .collect();
    if selected.len() != 1 || js_trim(&dom::content(selected[0])) != school_level.label() {
        return Err(Error::refusal("textbook_catalog_school_level_rejected"));
    }
    let labels = row_labels(&doc);
    let call_re =
        regex::Regex::new(r#"fullUrlDownloadFile\s*\(\s*(?:'([^'"<>\s]+)'|"([^'"<>\s]+)")\s*\)"#)
            .expect("constant");
    let mut entries: Vec<CatalogEntry> = Vec::new();
    let mut by_ref: HashMap<String, usize> = HashMap::new();
    for node in doc.select("[onclick], a[href]") {
        for candidate in [
            node.value().attr("onclick").unwrap_or(""),
            node.value().attr("href").unwrap_or(""),
        ] {
            for capture in call_re.captures_iter(candidate) {
                let literal = capture
                    .get(1)
                    .or_else(|| capture.get(2))
                    .expect("one quote form")
                    .as_str();
                if !literal.contains("/data/answer/") {
                    continue;
                }
                let url = qualified_pdf_url(literal)?;
                let document_ref = hash(
                    serde_json::json!([school_level.as_str(), url])
                        .to_string()
                        .as_bytes(),
                );
                let label = match closest_row(node) {
                    Some(row) => labels
                        .get(&format!("{:?}", row.id()))
                        .cloned()
                        .unwrap_or_default(),
                    None => node
                        .parent()
                        .and_then(scraper::ElementRef::wrap)
                        .map(|p| collapse(&dom::content(p)))
                        .unwrap_or_default(),
                };
                if label.is_empty() || utf16_len(&label) > 2000 {
                    return Err(Error::refusal("textbook_catalog_row_unqualified"));
                }
                let filename = url.rsplit('/').next().unwrap_or_default().to_string();
                if let Some(&index) = by_ref.get(&document_ref) {
                    if !entries[index].source_labels.contains(&label) {
                        entries[index].source_labels.push(label);
                    }
                } else {
                    by_ref.insert(document_ref.clone(), entries.len());
                    entries.push(CatalogEntry {
                        kind: DocumentKind::from_filename(&filename),
                        document_ref,
                        url,
                        filename,
                        source_labels: vec![label],
                    });
                }
            }
        }
    }
    if entries.is_empty() || entries.len() > 1000 {
        return Err(Error::refusal("textbook_catalog_pdf_links_unqualified"));
    }
    Ok(CatalogObservation {
        schema_version: 1,
        status: "observed".into(),
        school_level,
        observed_at: observed_at.to_string(),
        catalog_sha256: hash(html.as_bytes()),
        entries,
        coverage: "all_observed_pdf_links_in_one_catalog_document".into(),
        source_requests: 1,
    })
}

/// Tool-side filtering and paging over one full catalogue observation, as
/// the TS tool layer does them. Inputs are the already-validated tool
/// arguments; out-of-range paging is refused, never clamped silently.
pub fn page_catalog(
    observation: &CatalogObservation,
    request: CatalogPageRequest,
    offset: usize,
    limit: usize,
) -> Result<CatalogPage> {
    if limit == 0 || limit > 100 || offset > 1000 {
        return Err(Error::refusal("tool_input_invalid"));
    }
    let query = match &request.query {
        Some(raw) => {
            let trimmed = js_trim(raw);
            if trimmed.is_empty() || utf16_len(trimmed) > 200 {
                return Err(Error::refusal("tool_input_invalid"));
            }
            Some(trimmed.to_lowercase())
        }
        None => None,
    };
    let matched: Vec<CatalogEntry> = observation
        .entries
        .iter()
        .filter(|e| {
            request.document_kind.is_none_or(|k| e.kind == k)
                && query.as_ref().is_none_or(|q| {
                    e.filename.to_lowercase().contains(q)
                        || e.source_labels.iter().any(|s| s.to_lowercase().contains(q))
                })
        })
        .cloned()
        .collect();
    let selected: Vec<CatalogEntry> = matched.iter().skip(offset).take(limit).cloned().collect();
    let end = offset + selected.len();
    Ok(CatalogPage {
        total_observed_pdf_links: observation.entries.len(),
        total_matched: matched.len(),
        returned: selected.len(),
        next_offset: (end < matched.len()).then_some(end),
        entries: selected,
    })
}

/// Content-addressed original bytes, private to the verified owner launch.
/// The root is caller-supplied; an existing artifact is preserved, never
/// overwritten, and every return is proven by a bounded readback digest.
/// Symlinks are checked before anything is created, a new artifact is
/// written to a temporary name and published by hard link (which fails if
/// the final name exists), and the returned path is absolute.
pub fn preserve_pdf(root: &Path, pdf: &PdfObservation) -> Result<PdfArtifact> {
    let bytes = &pdf.bytes;
    if hash(bytes) != pdf.meta.sha256 || bytes.len() != pdf.meta.byte_length {
        return Err(Error::refusal("textbook_pdf_digest_rejected"));
    }
    let tail_start = bytes.len().saturating_sub(2048);
    if !bytes.starts_with(b"%PDF-") || !bytes[tail_start..].windows(5).any(|w| w == b"%%EOF") {
        return Err(Error::refusal("textbook_pdf_signature_rejected"));
    }
    let directory_rejected = || Error::refusal("textbook_artifact_directory_rejected");
    let readback_rejected = || Error::refusal("textbook_artifact_readback_rejected");
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| directory_rejected())?
            .join(root)
    };
    // Check the root and every existing ancestor before creating anything:
    // a symlinked ancestor would redirect creation outside the owner root.
    let mut ancestor = Some(absolute.as_path());
    while let Some(path) = ancestor {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(directory_rejected());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(directory_rejected()),
        }
        ancestor = path.parent();
    }
    let missing: Vec<PathBuf> = {
        let mut missing = Vec::new();
        let mut cursor = Some(absolute.as_path());
        while let Some(path) = cursor {
            if path.exists() {
                break;
            }
            missing.push(path.to_path_buf());
            cursor = path.parent();
        }
        missing
    };
    // Create each missing directory with its mode at creation time.
    for created in missing.iter().rev() {
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(created) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(directory_rejected()),
        }
    }
    let canonical = std::fs::canonicalize(&absolute).map_err(|_| directory_rejected())?;
    if canonical != absolute {
        return Err(directory_rejected());
    }
    let path = canonical.join(format!("{}.pdf", pdf.meta.sha256));
    let publish = |path: &Path| -> Result<()> {
        let metadata = std::fs::symlink_metadata(path).map_err(|_| readback_rejected())?;
        // The regular-file check comes before any read: a FIFO or device
        // at the target name refuses without blocking, and the read is
        // bounded by the expected size plus one byte.
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(readback_rejected());
        }
        use std::io::Read;
        // Open without following a swapped-in symlink and without blocking
        // on a swapped-in FIFO, then prove the opened file is the checked
        // one (same device and inode, still a regular file).
        #[cfg(unix)]
        let opened = {
            let fd = rustix::fs::open(
                path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .map_err(|_| readback_rejected())?;
            let file = std::fs::File::from(fd);
            let opened_meta = file.metadata().map_err(|_| readback_rejected())?;
            use std::os::unix::fs::MetadataExt;
            if opened_meta.dev() != metadata.dev()
                || opened_meta.ino() != metadata.ino()
                || !opened_meta.is_file()
            {
                return Err(readback_rejected());
            }
            file
        };
        #[cfg(not(unix))]
        let mut opened = std::fs::File::open(path).map_err(|_| readback_rejected())?;
        let mut readback = Vec::new();
        opened
            .take((bytes.len() + 1) as u64)
            .read_to_end(&mut readback)
            .map_err(|_| readback_rejected())?;
        if readback.len() != bytes.len() || hash(&readback) != pdf.meta.sha256 {
            return Err(readback_rejected());
        }
        Ok(())
    };
    if std::fs::symlink_metadata(&path).is_ok() {
        publish(&path)?;
    } else {
        // Write beside the target and publish without replacing: a hard
        // link fails if the final name appeared meanwhile, and a leftover
        // temporary file from an interrupted write is simply removed.
        static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temporary = canonical.join(format!(
            ".{}.pdf.tmp-{}-{}",
            pdf.meta.sha256,
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let written = (|| -> Result<()> {
            use std::io::Write;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary).map_err(|_| readback_rejected())?;
            file.write_all(bytes).map_err(|_| readback_rejected())?;
            file.sync_all().map_err(|_| readback_rejected())?;
            Ok(())
        })();
        if let Err(error) = written {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        match std::fs::hard_link(&temporary, &path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(directory_rejected());
            }
        }
        let _ = std::fs::remove_file(&temporary);
        publish(&path)?;
    }
    let size = std::fs::symlink_metadata(&path)
        .map_err(|_| readback_rejected())?
        .len();
    Ok(PdfArtifact {
        path,
        sha256: pdf.meta.sha256.clone(),
        bytes: size,
        custody: "native_owner_filesystem".into(),
    })
}
