//! Owner-bound paper1/paper2 source reads. No MCP wiring or runtime activation.
//! A printed key, a shared definition and an observed student attempt are distinct.
use crate::{
    data::*,
    owner_admission::{TransportAdmission, TransportScope},
    student_course::{self as course, Owner, ReadPort, Student, exact, key, require},
    student_metadata::{MetadataPolicy, PaperFormPin, StudentSource},
};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaperScope {
    Paper1,
    Paper2,
}
impl PaperScope {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "paper1" => Ok(Self::Paper1),
            "paper2" => Ok(Self::Paper2),
            _ => Err(Error::refusal("paper_scope_unresolved")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paper1 => "paper1",
            Self::Paper2 => "paper2",
        }
    }
}
/// Release/owner input only. Caller class/book/unit arguments never grant scope.
/// At most two distinct endpoints; every configured student has one exact route.
pub struct PaperScopes {
    entries: Vec<(PaperScope, Vec<String>)>,
}
impl PaperScopes {
    pub fn parse(value: &Js) -> Result<Self> {
        require(
            array(value) && (1..=2).contains(&value.array().len()),
            "paper_scope_unresolved",
        )?;
        let mut entries = vec![];
        let mut seen = HashSet::new();
        for row in value.array() {
            exact(row, &["endpoint", "studentIds"])?;
            require(row.get("endpoint").string(), "paper_scope_unresolved")?;
            let endpoint = PaperScope::parse(&string(row.get("endpoint")))?;
            require(
                !entries.iter().any(|(scope, _)| *scope == endpoint),
                "paper_scope_unresolved",
            )?;
            let ids = row.get("studentIds");
            require(
                array(ids) && (1..=200).contains(&ids.array().len()),
                "paper_scope_unresolved",
            )?;
            let mut students = vec![];
            for id in ids.array() {
                let id = key(id)?;
                require(seen.insert(id.clone()), "paper_scope_unresolved")?;
                students.push(id);
            }
            entries.push((endpoint, students));
        }
        Ok(Self { entries })
    }
    pub fn resolve(&self, student_id: &str) -> Result<PaperScope> {
        self.entries
            .iter()
            .find(|(_, ids)| ids.iter().any(|id| id == student_id))
            .map(|(scope, _)| *scope)
            .ok_or_else(|| Error::refusal("paper_scope_unresolved"))
    }
}

/// Validated explicit inputs. The private fields cannot be populated from a page.
pub struct PaperOwner {
    endpoint: PaperScope,
    owner_value: Js,
    owner: Owner,
    metadata_value: Js,
    form_value: Js,
    pins: PaperPins,
}
impl PaperOwner {
    pub fn parse(
        endpoint: PaperScope,
        routes: &PaperScopes,
        owner: &Js,
        metadata: &Js,
        form: &Js,
        pins: PaperPins,
    ) -> Result<Self> {
        let parsed = Owner::parse(owner)?;
        require(
            parsed
                .students
                .iter()
                .all(|s| routes.resolve(&s.student_key).ok() == Some(endpoint))
                && routes
                    .entries
                    .iter()
                    .any(|(scope, ids)| *scope == endpoint && ids.len() == parsed.students.len()),
            "paper_scope_unresolved",
        )?;
        MetadataPolicy::parse(metadata)?;
        PaperFormPin::parse(form)?;
        Ok(Self {
            endpoint,
            owner_value: owner.clone(),
            owner: parsed,
            metadata_value: metadata.clone(),
            form_value: form.clone(),
            pins,
        })
    }
}

/// Owns a single admitted port. A failed source occurrence latches every family.
/// A short-lived StudentSource borrows that port for the whole metadata operation;
/// it cannot escape or provide a fallback session/owner.
pub struct PaperSource<P: ReadPort> {
    port: P,
    owner: PaperOwner,
    transport: TransportAdmission,
    requests: usize,
    latched: bool,
    daily_pages: Vec<Js>,
    last_header: Option<crate::history::QualifiedHeader>,
}
struct BorrowedPort<'a, P: ReadPort> {
    port: &'a mut P,
    requests: &'a mut usize,
    daily_pages: &'a mut Vec<Js>,
    student: Option<Student>,
    pins: &'a PaperPins,
}
impl<P: ReadPort> ReadPort for BorrowedPort<'_, P> {
    fn transport_admission(&self) -> &TransportAdmission {
        self.port.transport_admission()
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<crate::http::Response> {
        require(*self.requests < 256, "paper_request_budget_exceeded")?;
        *self.requests += 1;
        let response = self.port.read(scope, operation, arguments)?;
        let html = std::str::from_utf8(&response.bytes)
            .map_err(|_| Error::refusal("student_read_encoding_rejected"))?;
        let d = document(html)?;
        let scripts = d.select("script:not([src])");
        require(scripts.len() <= 64, "paper_contract_limit")?;
        for script in scripts {
            script_tree(&crate::dom::content(script))?;
        }
        if operation == "metadata.daily" {
            display_text_contract(&d, self.pins)?;
            for option in d.select("select[name=sort_date] option") {
                require(
                    passive_text(option) == crate::dom::content(option),
                    "contract_drift",
                )?;
            }
            require(self.daily_pages.len() < 7, "paper_contract_limit")?;
            let student = self
                .student
                .as_ref()
                .ok_or_else(|| Error::refusal("read_scope_not_admitted"))?;
            self.daily_pages
                .push(inventory_detail(&d, student, self.pins)?);
        }
        Ok(response)
    }
}
impl<P: ReadPort> PaperSource<P> {
    pub fn new(port: P, owner: PaperOwner, transport: TransportAdmission) -> Result<Self> {
        owner.owner.validate_transport(&transport)?;
        let actual = serde_json::to_value(port.transport_admission())
            .map_err(|_| Error::refusal("student_read_owner_transport_conflict"))?;
        let expected = serde_json::to_value(&transport)
            .map_err(|_| Error::refusal("student_read_owner_transport_conflict"))?;
        require(actual == expected, "student_read_owner_transport_conflict")?;
        // Reuse the complete E09a constructor, including metadata teacher policy,
        // before accepting the source. Construction performs no source request.
        let mut source = Self {
            port,
            owner,
            transport,
            requests: 0,
            latched: false,
            daily_pages: vec![],
            last_header: None,
        };
        source.student_view(None)?;
        Ok(source)
    }
    fn student_view(
        &mut self,
        student: Option<Student>,
    ) -> Result<StudentSource<BorrowedPort<'_, P>>> {
        StudentSource::new(
            BorrowedPort {
                port: &mut self.port,
                requests: &mut self.requests,
                daily_pages: &mut self.daily_pages,
                student,
                pins: &self.owner.pins,
            },
            Owner::parse(&self.owner.owner_value)?,
            serde_json::from_value(
                serde_json::to_value(&self.transport)
                    .map_err(|_| Error::refusal("student_read_owner_transport_conflict"))?,
            )
            .map_err(|_| Error::refusal("student_read_owner_transport_conflict"))?,
            MetadataPolicy::parse(&self.owner.metadata_value)?,
            Some(PaperFormPin::parse(&self.owner.form_value)?),
        )
    }
    /// Immutable, credential-free admission for a History native source loan.
    pub fn transport_admission(&self) -> &TransportAdmission {
        &self.transport
    }
    /// A fresh qualified header, with the exact content-response hash and no
    /// raw question/body text. Failure never becomes a raw-port fallback.
    pub fn history_header(
        &mut self,
        student: &Js,
        code: &Js,
        date: &str,
        now: &str,
    ) -> Result<crate::history::QualifiedHeader> {
        let student = course::key(student)?;
        decode_code(code)?;
        self.last_header = None;
        self.read(
            "academy_paper_definition_lookup",
            &obj(&[
                ("studentId", student.as_str().into()),
                ("paperCode", code.clone()),
                ("view", "header".into()),
            ]),
            date,
            now,
        )?;
        self.last_header
            .take()
            .ok_or_else(|| Error::refusal("source_headers_read_failed"))
    }
    pub fn requests(&self) -> usize {
        self.requests
    }
    pub fn endpoint(&self) -> PaperScope {
        self.owner.endpoint
    }
    fn student(&self, input: &Js) -> Result<Student> {
        let id = key(input.get("studentId"))?;
        self.owner
            .owner
            .students
            .iter()
            .find(|s| s.student_key == id)
            .cloned()
            .ok_or_else(|| Error::refusal("read_scope_not_admitted"))
    }
    fn metadata(
        &mut self,
        student: &Student,
        lesson_date: &str,
        now: &str,
        query: Js,
    ) -> Result<Js> {
        let input = obj(&[
            (
                "scope",
                obj(&[
                    ("sourceNamespace", course::METADATA_NS.into()),
                    ("teacherId", self.owner.owner.teacher_key.as_str().into()),
                    ("studentId", student.student_key.as_str().into()),
                    ("lessonDate", lesson_date.into()),
                ]),
            ),
            ("purpose", "Explicit owner paper source read".into()),
            ("currentRequestRef", "paper-source-read".into()),
            ("queries", Js::Arr(vec![query])),
        ]);
        self.daily_pages.clear();
        let result = self.student_view(Some(student.clone()))?.metadata(
            "academy_student_metadata",
            &input,
            now,
        )?;
        let mut observation = result.get("results").array()[0].get("observation").clone();
        let query = input.get("queries").array()[0].get("kind");
        if query.is("daily_inventory") {
            append_inventory_detail(&mut observation, &self.daily_pages[0]);
        } else if query.is("daily_history") {
            let mut current = observation.get("currentFilterQualification").clone();
            append_inventory_detail(&mut current, &self.daily_pages[0]);
            observation.set("currentFilterQualification", current);
            let mut pages = observation.get("observations").array().to_vec();
            for (page, detail) in pages.iter_mut().zip(self.daily_pages.iter().skip(1)) {
                append_inventory_detail(page, detail);
            }
            observation.set("observations", Js::Arr(pages));
        }
        // Explicit row schema: identities were canonicalized by E09a; titles
        // are the enumerated visible source fields. Opaque source ids never
        // become caller-visible identifiers, including history/printed copies.
        fn rows(page: &mut Js) {
            if page.has("rows") {
                page.set(
                    "rows",
                    Js::Arr(
                        page.get("rows")
                            .array()
                            .iter()
                            .map(|row| {
                                let mut output = obj(&[]);
                                for name in [
                                    "studentKey",
                                    "studentName",
                                    "paperKey",
                                    "attemptKey",
                                    "sourceScore",
                                    "titles",
                                ] {
                                    output.set(name, row.get(name).clone());
                                }
                                output.set(
                                    "sourceRowIdSha256",
                                    hash(json(row.get("sourceRowId")).as_bytes()).into(),
                                );
                                output
                            })
                            .collect(),
                    ),
                );
            }
        }
        rows(&mut observation);
        if observation.has("currentFilterQualification") {
            let mut current = observation.get("currentFilterQualification").clone();
            rows(&mut current);
            observation.set("currentFilterQualification", current);
        }
        if observation.has("observations") {
            let mut pages = observation.get("observations").array().to_vec();
            for page in &mut pages {
                rows(page);
            }
            observation.set("observations", Js::Arr(pages));
        }
        self.daily_pages.clear();
        Ok(observation)
    }
}

/// Owner-supplied hashes, never derived from a response during admission.
/// Page hashes cover inert form/control/options/script structure. Real values
/// remain an R12 owner-assembly prerequisite, as with E09a PaperFormPin.
pub struct PaperPins {
    reader_context: String,
    ready: String,
    view: String,
    catalog: String,
    pool: String,
    converter: String,
    stylesheet_urls: HashSet<String>,
}
impl PaperPins {
    pub fn parse(v: &Js) -> Result<Self> {
        let mut fields = vec![
            "schemaVersion",
            "readerContext",
            "ready",
            "view",
            "catalog",
            "pool",
            "converter",
        ];
        if v.has("stylesheetUrls") {
            fields.push("stylesheetUrls");
        }
        exact(v, &fields)?;
        require(
            v.get("schemaVersion").is("academy.paper-pins/1"),
            "contract_drift",
        )?;
        for field in [
            "readerContext",
            "ready",
            "view",
            "catalog",
            "pool",
            "converter",
        ] {
            require(
                v.get(field).string() && sha_valid(v.get(field)),
                "contract_drift",
            )?;
        }
        // Optional for existing owners: omission admits no external stylesheets.
        // Publication belongs to R12, never to a page response or fingerprint.
        let mut stylesheet_urls = HashSet::new();
        if v.has("stylesheetUrls") {
            let urls = v.get("stylesheetUrls");
            require(array(urls) && urls.array().len() <= 64, "contract_drift")?;
            for value in urls.array() {
                require(value.string(), "contract_drift")?;
                let url = stylesheet_url(&string(value))?;
                require(stylesheet_urls.insert(url), "contract_drift")?;
            }
        }
        Ok(Self {
            stylesheet_urls,
            reader_context: string(v.get("readerContext")),
            ready: string(v.get("ready")),
            view: string(v.get("view")),
            catalog: string(v.get("catalog")),
            pool: string(v.get("pool")),
            converter: string(v.get("converter")),
        })
    }
}

const INFO_PATH: &str = "/servlet/controller.admin.system.testpage.TestPageRegServlet";
const VIEW_PATH: &str = "/servlet/controller.common.TestPageShowServlet";
const POOL_PATH: &str = "/servlet/controller.tutor.etest.TestPoolServlet";
const CONVERTER_PATH: &str = "/jsutil/module.js";
pub const PAPER_NS: &str = "academy.lms.paper";

/// Fixed read templates; installation alone grants no transport admission.
pub fn read_contracts() -> Vec<crate::owner_admission::OperationContract> {
    use crate::owner_admission::{OperationContract, ParameterValue, TransportParameter};
    let literal = |name: &str, value: &str| TransportParameter {
        name: name.into(),
        value: ParameterValue::Literal {
            value: value.into(),
        },
    };
    let argument = |name: &str, max_bytes| TransportParameter {
        name: name.into(),
        value: ParameterValue::Argument {
            name: name.into(),
            max_bytes,
        },
    };
    let op = |id: &str, method: &str, path: &str, query, body| OperationContract {
        id: id.into(),
        source_namespace: PAPER_NS.into(),
        method: method.into(),
        path: path.into(),
        effect: false,
        mirrored_parameters: vec![],
        query,
        body,
    };
    vec![
        op(
            "paper.definition",
            "GET",
            INFO_PATH,
            vec![
                literal("reqCmd", "testInfo"),
                argument("test_no", 16),
                argument("uniq", 64),
            ],
            vec![],
        ),
        op(
            "paper.header",
            "GET",
            VIEW_PATH,
            vec![
                literal("p_process", "ShowTestPage"),
                argument("p_testno", 16),
                argument("dummy", 64),
            ],
            vec![],
        ),
        op(
            "paper.content",
            "GET",
            VIEW_PATH,
            vec![
                literal("p_process", "TestPageTabSwitch"),
                argument("p_testno", 16),
                literal("seltab", "1"),
            ],
            vec![],
        ),
        op(
            "paper.pool",
            "GET",
            POOL_PATH,
            vec![literal("reqCmd", "Main")],
            vec![],
        ),
        op(
            "paper.converter",
            "GET",
            CONVERTER_PATH,
            vec![literal("update_date", "25071401")],
            vec![],
        ),
        op(
            "paper.catalog",
            "GET",
            course::DAILY_PATH,
            vec![literal("reqCmd", "DailyZeroTestMain")],
            vec![],
        ),
        op(
            "paper.definitions",
            "POST",
            course::DAILY_PATH,
            vec![literal("reqCmd", "DailyZeroTestSeach")],
            vec![
                argument("evalgubun", 16),
                argument("frm_cl1", 16),
                literal("frm_cl2", "-1"),
                literal("frm_cl3", "-1"),
            ],
        ),
    ]
}

fn decode_code(code: &Js) -> Result<Js> {
    crate::history::decode_code(code).map_err(|e| Error::refusal(&e.message))
}
fn bounded_text(value: &str, max: usize) -> Result<Js> {
    require(
        value.encode_utf16().count() <= max
            && !value.chars().any(
                |c| matches!(c, '\0'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{7f}'),
            ),
        "paper_output_budget_exceeded",
    )?;
    Ok(value.into())
}
// Read displayed text without copying executable or non-displayed subtrees.
// Keep script parsing and owner form fingerprints on their original raw source.
fn text_attributes_qualified(e: scraper::ElementRef<'_>) -> bool {
    let element = e.value();
    element.attrs().all(|(name, value)| {
        // Presentation attributes are inert under the reference textContent
        // contract. Their values never enter public text/structural projections.
        match name {
            "id" | "class" | "border" | "cellpadding" | "cellspacing" | "width" | "height"
            | "align" | "valign" | "bgcolor" | "color" | "face" | "size" | "lang" | "dir"
            | "nowrap" | "colspan" | "rowspan" | "background" | "title" | "hspace" | "vspace"
            | "frame" | "rules" | "cellborder" => true,
            "style" => css_declarations(value).is_some_and(|hiding| !hiding),
            "aria-hidden" => value.trim().eq_ignore_ascii_case("false"),
            _ => match element.name() {
                "form" => matches!(name, "name" | "action" | "method"),
                "select" => matches!(name, "name" | "value" | "disabled" | "multiple"),
                "option" => matches!(name, "value" | "selected" | "disabled"),
                "optgroup" => name == "disabled",
                "td" | "th" => matches!(name, "colspan" | "rowspan" | "onclick"),
                "a" => matches!(name, "href" | "onclick" | "target"),
                "button" => matches!(name, "name" | "type" | "value" | "disabled" | "onclick"),
                "textarea" => matches!(name, "name" | "readonly" | "disabled"),
                "details" | "dialog" => name == "open",
                _ => false,
            },
        }
    })
}
fn passive_text(e: scraper::ElementRef<'_>) -> String {
    let hidden = |e: scraper::ElementRef<'_>| {
        let element = e.value();
        // Only ordinary text containers participate. Embedded/media/foreign
        // and unknown elements have unqualified fallback/rendering semantics.
        !matches!(
            element.name(),
            "html"
                | "body"
                | "main"
                | "article"
                | "section"
                | "header"
                | "footer"
                | "nav"
                | "aside"
                | "div"
                | "span"
                | "p"
                | "a"
                | "b"
                | "i"
                | "em"
                | "strong"
                | "u"
                | "s"
                | "small"
                | "sub"
                | "sup"
                | "font"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "br"
                | "hr"
                | "pre"
                | "code"
                | "blockquote"
                | "ul"
                | "ol"
                | "li"
                | "dl"
                | "dt"
                | "dd"
                | "table"
                | "caption"
                | "thead"
                | "tbody"
                | "tfoot"
                | "tr"
                | "th"
                | "td"
                | "form"
                | "fieldset"
                | "legend"
                | "label"
                | "button"
                | "select"
                | "option"
                | "optgroup"
                | "details"
                | "summary"
                | "dialog"
        ) || !text_attributes_qualified(e)
            || matches!(element.name(), "dialog" | "details") && element.attr("open").is_none()
    };
    if e.value().name() == "select"
        || hidden(e)
        || e.ancestors()
            .filter_map(scraper::ElementRef::wrap)
            .any(hidden)
    {
        return String::new();
    }
    let mut text = String::new();
    // Each declaration is qualified independently. Normal-flow displacement
    // remains reachable by document scrolling; it is not a geometry budget.
    let mut pending = e.children().rev().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        if let Some(element) = scraper::ElementRef::wrap(node)
            && (element.value().name() == "select" || hidden(element))
        {
            continue;
        }
        if let scraper::node::Node::Text(value) = node.value() {
            text.push_str(value);
        }
        pending.extend(node.children().rev());
    }
    text
}
fn document_title(d: &crate::dom::Document) -> Result<Js> {
    let titles = d.select("title");
    let qualified = d.select(":root > head > title");
    require(
        titles.len() <= 1 && titles.len() == qualified.len(),
        "contract_drift",
    )?;
    let Some(title) = qualified.first() else {
        return Ok("".into());
    };
    require(
        title
            .ancestors()
            .filter_map(scraper::ElementRef::wrap)
            .chain(std::iter::once(*title))
            .all(|e| {
                let namespace: &str = e.value().name.ns.as_ref();
                namespace == "http://www.w3.org/1999/xhtml" && text_attributes_qualified(e)
            })
            && title
                .children()
                .all(|node| matches!(node.value(), scraper::node::Node::Text(_))),
        "contract_drift",
    )?;
    bounded_text(&crate::dom::content(*title), 1000)
}
// Canonical absolute URL identity, using the fixed source origin solely as a
// base. This parses strings; it never fetches a stylesheet or follows a redirect.
fn stylesheet_url(value: &str) -> Result<String> {
    require(value.len() <= 4096, "contract_drift")?;
    // All displayed source templates (VIEW_PATH and course::DAILY_PATH) are
    // documents in /servlet/. Document-relative hrefs must use that directory,
    // not the origin root. BASE overrides are separately refused before use.
    let base = url::Url::parse(crate::http::ORIGIN)
        .expect("fixed origin")
        .join(VIEW_PATH)
        .expect("fixed paper document path");
    let mut url = base
        .join(value)
        .map_err(|_| Error::refusal("contract_drift"))?;
    require(
        matches!(url.scheme(), "https" | "http")
            && url.username().is_empty()
            && url.password().is_none(),
        "contract_drift",
    )?;
    url.set_fragment(None);
    Ok(url.into())
}

// CSS Syntax Level 3 sections 3.3/4: preprocess before consuming escapes,
// then retain token identity. This is a rejecting subset, not a CSS renderer.
// Strings, functions, at-rules and unsupported tokens never enter a grammar.
// Comments are consumed BETWEEN tokens, never by concatenating token spellings.
// Budgets: 32768 input bytes, 8192 tokens, 1024 declarations/rules; no recursion.
#[derive(Clone, Debug, PartialEq)]
enum CssToken {
    Ident(String),
    Number(f64),
    Dimension(f64, String),
    Percentage(f64),
    Hash(String),
    Punct(char),
}
fn css_preprocess(source: &str) -> Option<Vec<char>> {
    if source.len() > 32_768 {
        return None;
    }
    let mut chars = source.chars().peekable();
    let mut result = Vec::new();
    while let Some(c) = chars.next() {
        result.push(match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                '\n'
            }
            '\u{c}' => '\n',
            '\0' => '\u{fffd}',
            // Rust str cannot contain surrogate code points. Escaped surrogate
            // values are replaced in css_escape, just like NULL/out-of-range.
            _ => c,
        });
    }
    Some(result)
}
fn css_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n')
}
fn css_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}
fn css_name(c: char) -> bool {
    css_name_start(c) || c.is_ascii_digit() || c == '-'
}
fn css_valid_escape(chars: &[char], i: usize) -> bool {
    chars.get(i) == Some(&'\\') && chars.get(i + 1) != Some(&'\n')
}
fn css_ident_start(chars: &[char], i: usize) -> bool {
    match chars.get(i).copied() {
        Some('-') => {
            chars
                .get(i + 1)
                .is_some_and(|c| css_name_start(*c) || *c == '-')
                || css_valid_escape(chars, i + 1)
        }
        Some(c) if css_name_start(c) => true,
        Some('\\') => css_valid_escape(chars, i),
        _ => false,
    }
}
fn css_escape(chars: &[char], i: &mut usize) -> Option<char> {
    if !css_valid_escape(chars, *i) {
        return None;
    }
    *i += 1;
    let mut n = 0;
    let mut digits = 0;
    while digits < 6 {
        let Some(digit) = chars.get(*i).and_then(|c| c.to_digit(16)) else {
            break;
        };
        n = n * 16 + digit;
        digits += 1;
        *i += 1;
    }
    if digits != 0 {
        if chars.get(*i).is_some_and(|c| css_whitespace(*c)) {
            *i += 1;
        }
        return Some(if n == 0 {
            '\u{fffd}'
        } else {
            char::from_u32(n).unwrap_or('\u{fffd}')
        });
    }
    let c = chars.get(*i).copied().unwrap_or('\u{fffd}');
    *i += usize::from(*i < chars.len());
    Some(c)
}
fn css_ident(chars: &[char], i: &mut usize) -> Option<String> {
    let mut value = String::new();
    while let Some(c) = chars.get(*i).copied() {
        if css_name(c) {
            value.push(c);
            *i += 1;
        } else if css_valid_escape(chars, *i) {
            value.push(css_escape(chars, i)?);
        } else {
            break;
        }
    }
    Some(value.to_ascii_lowercase())
}
fn css_number_start(chars: &[char], i: usize) -> bool {
    let mut j = i;
    if matches!(chars.get(j), Some('+' | '-')) {
        j += 1;
    }
    chars.get(j).is_some_and(char::is_ascii_digit)
        || chars.get(j) == Some(&'.') && chars.get(j + 1).is_some_and(char::is_ascii_digit)
}
fn css_numeric(chars: &[char], i: &mut usize) -> Option<f64> {
    let start = *i;
    if matches!(chars.get(*i), Some('+' | '-')) {
        *i += 1;
    }
    while chars.get(*i).is_some_and(char::is_ascii_digit) {
        *i += 1;
    }
    if chars.get(*i) == Some(&'.') && chars.get(*i + 1).is_some_and(char::is_ascii_digit) {
        *i += 1;
        while chars.get(*i).is_some_and(char::is_ascii_digit) {
            *i += 1;
        }
    }
    if matches!(chars.get(*i), Some('e' | 'E')) {
        let mut j = *i + 1;
        if matches!(chars.get(j), Some('+' | '-')) {
            j += 1;
        }
        if chars.get(j).is_some_and(char::is_ascii_digit) {
            *i = j + 1;
            while chars.get(*i).is_some_and(char::is_ascii_digit) {
                *i += 1;
            }
        }
    }
    let n: f64 = chars[start..*i].iter().collect::<String>().parse().ok()?;
    n.is_finite().then_some(n)
}
fn css_tokens(source: &str) -> Option<Vec<CssToken>> {
    let chars = css_preprocess(source)?;
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if css_whitespace(chars[i]) {
            i += 1;
            continue;
        }
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            if i == chars.len() {
                return None;
            }
            i += 2;
            continue;
        }
        let token = if css_number_start(&chars, i) {
            let n = css_numeric(&chars, &mut i)?;
            if css_ident_start(&chars, i) {
                CssToken::Dimension(n, css_ident(&chars, &mut i)?)
            } else if chars.get(i) == Some(&'%') {
                i += 1;
                CssToken::Percentage(n)
            } else {
                CssToken::Number(n)
            }
        } else if css_ident_start(&chars, i) {
            let ident = css_ident(&chars, &mut i)?;
            // Function/url tokens have no admitted value grammar. Do not
            // reinterpret their contents or their escaped name as punctuation.
            if chars.get(i) == Some(&'(') {
                return None;
            }
            CssToken::Ident(ident)
        } else if chars[i] == '#'
            && (chars.get(i + 1).is_some_and(|c| css_name(*c)) || css_valid_escape(&chars, i + 1))
        {
            i += 1;
            CssToken::Hash(css_ident(&chars, &mut i)?)
        } else {
            let c = chars[i];
            i += 1;
            if !matches!(
                c,
                ':' | ';' | '{' | '}' | '!' | ',' | '.' | '>' | '+' | '~' | '*'
            ) {
                return None;
            }
            CssToken::Punct(c)
        };
        tokens.push(token);
        if tokens.len() > 8192 {
            return None;
        }
    }
    Some(tokens)
}

#[derive(Clone, Copy)]
enum CssGrammar {
    Keywords(&'static [&'static str]),
    Lengths(usize),
    PositiveLength,
    Size,
    LineHeight,
    Spacing,
    Border,
    Color(bool),
    Colors,
    Opacity,
    Weight,
    Zero(bool),
}
// Closed property -> value grammars. Geometry policy: each nonzero absolute
// length is nonnegative and <=128px (font size/line height also >=1px).
// Unitless lengths only admit zero. Zero in known length units is equivalent
// to zero, including the retained 0vh control. Relative nonzero lengths refuse.
// Width/height allow auto or >0..100%; line-height allows 0.5..4 unitless.
// Bounds apply per value, never across declarations, ancestors, matched
// elements or siblings. Flow-only CSS cannot position or paint a box over
// another box. Unsupported/inherited/global values and functions refuse.
// Token/rule/tree-work limits separately bound parsing and projection cost.
const CSS_PROPERTIES: &[(&str, CssGrammar)] = &[
    (
        "display",
        CssGrammar::Keywords(&[
            "block",
            "inline",
            "inline-block",
            "table",
            "table-row",
            "table-cell",
            "table-caption",
            "table-row-group",
            "table-header-group",
            "table-footer-group",
            "list-item",
            "flow-root",
            "flex",
            "inline-flex",
            "grid",
            "inline-grid",
        ]),
    ),
    ("visibility", CssGrammar::Keywords(&["visible"])),
    ("content-visibility", CssGrammar::Keywords(&["visible"])),
    ("opacity", CssGrammar::Opacity),
    ("color", CssGrammar::Color(false)),
    ("font-size", CssGrammar::PositiveLength),
    (
        "font-family",
        CssGrammar::Keywords(&[
            "serif",
            "sans-serif",
            "monospace",
            "cursive",
            "fantasy",
            "system-ui",
        ]),
    ),
    ("font-weight", CssGrammar::Weight),
    (
        "font-style",
        CssGrammar::Keywords(&["normal", "italic", "oblique"]),
    ),
    (
        "font-variant",
        CssGrammar::Keywords(&["normal", "small-caps"]),
    ),
    (
        "text-align",
        CssGrammar::Keywords(&["left", "right", "center", "justify", "start", "end"]),
    ),
    (
        "text-transform",
        CssGrammar::Keywords(&["none", "capitalize", "uppercase", "lowercase"]),
    ),
    (
        "white-space",
        CssGrammar::Keywords(&[
            "normal",
            "nowrap",
            "pre",
            "pre-wrap",
            "pre-line",
            "break-spaces",
        ]),
    ),
    (
        "word-break",
        CssGrammar::Keywords(&["normal", "break-all", "keep-all"]),
    ),
    (
        "overflow-wrap",
        CssGrammar::Keywords(&["normal", "break-word", "anywhere"]),
    ),
    ("direction", CssGrammar::Keywords(&["ltr", "rtl"])),
    (
        "box-sizing",
        CssGrammar::Keywords(&["content-box", "border-box"]),
    ),
    (
        "border-collapse",
        CssGrammar::Keywords(&["collapse", "separate"]),
    ),
    (
        "border-style",
        CssGrammar::Keywords(&[
            "none", "hidden", "solid", "dotted", "dashed", "double", "groove", "ridge", "inset",
            "outset",
        ]),
    ),
    ("table-layout", CssGrammar::Keywords(&["auto", "fixed"])),
    ("position", CssGrammar::Keywords(&["static"])),
    ("overflow", CssGrammar::Keywords(&["visible"])),
    ("overflow-x", CssGrammar::Keywords(&["visible"])),
    ("overflow-y", CssGrammar::Keywords(&["visible"])),
    ("transform", CssGrammar::Keywords(&["none"])),
    ("translate", CssGrammar::Keywords(&["none"])),
    ("clip", CssGrammar::Keywords(&["auto"])),
    ("clip-path", CssGrammar::Keywords(&["none"])),
    ("border-color", CssGrammar::Colors),
    ("margin", CssGrammar::Lengths(4)),
    ("padding", CssGrammar::Lengths(4)),
    ("margin-top", CssGrammar::Lengths(1)),
    ("margin-right", CssGrammar::Lengths(1)),
    ("margin-bottom", CssGrammar::Lengths(1)),
    ("margin-left", CssGrammar::Lengths(1)),
    ("padding-top", CssGrammar::Lengths(1)),
    ("padding-right", CssGrammar::Lengths(1)),
    ("padding-bottom", CssGrammar::Lengths(1)),
    ("padding-left", CssGrammar::Lengths(1)),
    ("text-indent", CssGrammar::Lengths(1)),
    ("width", CssGrammar::Size),
    ("height", CssGrammar::Size),
    ("min-width", CssGrammar::Size),
    ("max-width", CssGrammar::Size),
    ("min-height", CssGrammar::Size),
    ("max-height", CssGrammar::Size),
    ("line-height", CssGrammar::LineHeight),
    ("letter-spacing", CssGrammar::Spacing),
    ("word-spacing", CssGrammar::Spacing),
    ("border", CssGrammar::Border),
    ("border-top", CssGrammar::Border),
    ("border-right", CssGrammar::Border),
    ("border-bottom", CssGrammar::Border),
    ("border-left", CssGrammar::Border),
    ("border-width", CssGrammar::Lengths(4)),
    ("border-top-width", CssGrammar::Lengths(1)),
    ("border-right-width", CssGrammar::Lengths(1)),
    ("border-bottom-width", CssGrammar::Lengths(1)),
    ("border-left-width", CssGrammar::Lengths(1)),
    ("border-radius", CssGrammar::Lengths(4)),
    ("border-spacing", CssGrammar::Lengths(2)),
    ("outline", CssGrammar::Zero(true)),
    ("outline-width", CssGrammar::Zero(false)),
    ("outline-offset", CssGrammar::Zero(false)),
    ("outline-style", CssGrammar::Keywords(&["none"])),
    ("box-shadow", CssGrammar::Keywords(&["none"])),
    ("text-shadow", CssGrammar::Keywords(&["none"])),
    ("rotate", CssGrammar::Keywords(&["none"])),
    ("scale", CssGrammar::Keywords(&["none"])),
    ("filter", CssGrammar::Keywords(&["none"])),
    ("backdrop-filter", CssGrammar::Keywords(&["none"])),
    ("mix-blend-mode", CssGrammar::Keywords(&["normal"])),
    ("mask", CssGrammar::Keywords(&["none"])),
    ("mask-image", CssGrammar::Keywords(&["none"])),
    ("overflow-block", CssGrammar::Keywords(&["visible"])),
    ("overflow-inline", CssGrammar::Keywords(&["visible"])),
];
fn css_keyword(tokens: &[CssToken], values: &[&str]) -> bool {
    matches!(tokens, [CssToken::Ident(value)] if values.contains(&value.as_str()))
}
fn css_length(token: &CssToken, positive: bool) -> bool {
    let minimum = if positive { 1.0 } else { 0.0 };
    match token {
        CssToken::Number(n) => !positive && *n == 0.0,
        CssToken::Dimension(n, unit) if unit == "px" => (minimum..=128.0).contains(n),
        CssToken::Dimension(n, unit) => {
            !positive
                && *n == 0.0
                && [
                    "em", "rem", "ex", "ch", "vw", "vh", "vmin", "vmax", "cm", "mm", "q", "in",
                    "pt", "pc",
                ]
                .contains(&unit.as_str())
        }
        _ => false,
    }
}
fn css_color(token: &CssToken, decorative: bool) -> bool {
    match token {
        CssToken::Ident(s) => {
            // Background paints have no admitted property grammar. Restrict
            // foreground paint to this dark palette; bright/white/transparent
            // text cannot qualify against the ordinary light canvas. Legacy
            // presentation attributes remain inert under the Lead's policy.
            let names: &[&str] = if decorative {
                &[
                    "black",
                    "silver",
                    "gray",
                    "white",
                    "maroon",
                    "red",
                    "purple",
                    "fuchsia",
                    "green",
                    "lime",
                    "olive",
                    "yellow",
                    "navy",
                    "blue",
                    "teal",
                    "aqua",
                    "orange",
                    "transparent",
                ]
            } else {
                &[
                    "black", "gray", "maroon", "red", "purple", "fuchsia", "green", "olive",
                    "navy", "blue", "teal",
                ]
            };
            names.contains(&s.as_str())
        }
        CssToken::Hash(s) => {
            if !matches!(s.len(), 3 | 6) || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
                return false;
            }
            if decorative {
                return true;
            }
            let rgb = if s.len() == 3 {
                s.chars().flat_map(|c| [c, c]).collect::<String>()
            } else {
                s.clone()
            };
            let channel = |i| u32::from_str_radix(&rgb[i..i + 2], 16).expect("qualified hex");
            // Closed foreground value bound: weighted encoded RGB <=128/255.
            // This deliberately refuses light paints; it does not simulate CSS
            // cascade, a device theme, or owner-pinned external stylesheets.
            2126 * channel(0) + 7152 * channel(2) + 722 * channel(4) <= 1_280_000
        }
        _ => false,
    }
}
fn css_value_qualified(grammar: CssGrammar, tokens: &[CssToken]) -> bool {
    match grammar {
        CssGrammar::Keywords(values) => css_keyword(tokens, values),
        CssGrammar::Lengths(max) => {
            !tokens.is_empty() && tokens.len() <= max && tokens.iter().all(|t| css_length(t, false))
        }
        CssGrammar::PositiveLength => tokens.len() == 1 && css_length(&tokens[0], true),
        CssGrammar::Size => {
            css_keyword(tokens, &["auto"])
                || matches!(tokens, [CssToken::Percentage(n)] if *n > 0.0 && *n <= 100.0)
                || tokens.len() == 1 && css_length(&tokens[0], true)
        }
        CssGrammar::LineHeight => {
            css_keyword(tokens, &["normal"])
                || matches!(tokens, [CssToken::Number(n)] if (0.5..=4.0).contains(n))
                || tokens.len() == 1 && css_length(&tokens[0], true)
        }
        CssGrammar::Spacing => {
            css_keyword(tokens, &["normal"]) || tokens.len() == 1 && css_length(&tokens[0], false)
        }
        CssGrammar::Color(transparent) => tokens.len() == 1 && css_color(&tokens[0], transparent),
        CssGrammar::Colors => {
            !tokens.is_empty() && tokens.len() <= 4 && tokens.iter().all(|t| css_color(t, true))
        }
        CssGrammar::Opacity => {
            // Only fully opaque text qualifies; arbitrarily small nonzero
            // alpha must not recreate foreground erasure through compositing.
            matches!(tokens, [CssToken::Number(n)] if *n == 1.0)
                || matches!(tokens, [CssToken::Percentage(n)] if *n == 100.0)
        }
        CssGrammar::Zero(none) => {
            none && css_keyword(tokens, &["none"])
                || tokens.len() == 1
                    && css_length(&tokens[0], false)
                    && matches!(&tokens[0], CssToken::Number(n) | CssToken::Dimension(n, _) if *n == 0.0)
        }
        CssGrammar::Weight => {
            css_keyword(tokens, &["normal", "bold", "bolder", "lighter"])
                || matches!(tokens, [CssToken::Number(n)] if (100.0..=900.0).contains(n) && *n % 100.0 == 0.0)
        }
        // Require exactly width/style/color. This avoids implicit medium widths,
        // omitted terms and shorthand parsing ambiguities; transparent border
        // color does not hide its text, but width still obeys the geometry bound.
        CssGrammar::Border => {
            tokens.len() == 3
                && css_length(&tokens[0], false)
                && css_keyword(
                    &tokens[1..2],
                    &[
                        "none", "solid", "dotted", "dashed", "double", "groove", "ridge", "inset",
                        "outset",
                    ],
                )
                && css_color(&tokens[2], true)
        }
    }
}
fn css_declaration_parts(tokens: &[CssToken]) -> Option<(&str, &[CssToken])> {
    let [CssToken::Ident(property), CssToken::Punct(':'), rest @ ..] = tokens else {
        return None;
    };
    let value = if let [value @ .., CssToken::Punct('!'), CssToken::Ident(important)] = rest {
        if important != "important" {
            return None;
        }
        value
    } else {
        rest
    };
    Some((property.as_str(), value))
}
fn css_declaration_tokens(tokens: &[CssToken]) -> Option<()> {
    let mut count = 0;
    for declaration in tokens.split(|t| *t == CssToken::Punct(';')) {
        if declaration.is_empty() {
            continue;
        }
        count += 1;
        if count > 1024 {
            return None;
        }
        let (property, value) = css_declaration_parts(declaration)?;
        let (_, grammar) = CSS_PROPERTIES.iter().find(|(name, _)| *name == property)?;
        if !css_value_qualified(*grammar, value) {
            return None;
        }
    }
    Some(())
}
fn css_declarations(source: &str) -> Option<bool> {
    css_declaration_tokens(&css_tokens(source)?).map(|()| false)
}

// An inline rejection normally makes only its subtree opaque. A rejected
// positioning/paint declaration can also cover or displace OTHER subtrees,
// including from an empty sibling. Such a document cannot qualify any text.
// This scan grants no CSS admission: it uses the same preprocessing/identifier
// escape rules solely to detect cross-box hazards even when another value is
// outside the rejecting tokenizer's grammar (e.g. width:calc(...)). Unknown
// declaration names also fail closed. Comments/strings cannot hide a later
// declaration, and quoted data is never treated as a property name.
fn css_inline_rejection_is_local(element: scraper::ElementRef<'_>) -> bool {
    let source = element.value().attr("style").expect("selected style");
    if css_declarations(source) == Some(false) {
        return true;
    }
    // Table columns style cells that are not their DOM descendants. Any
    // unqualified column/group declaration therefore affects other subtrees,
    // including inherited or escaped visibility:collapse. Keep qualified
    // visible typography/layout on columns, but never isolate their rejection.
    if matches!(element.value().name(), "col" | "colgroup") {
        return false;
    }
    let Some(chars) = css_preprocess(source) else {
        return false;
    };
    // CSS display roles are independent of the native tag. Unsupported roles
    // can style non-descendant table cells or generate anonymous boxes. Admit
    // local rejection only for known flow roles or explicit display:none; use
    // the same declaration shape/important/value grammar as normal admission.
    // Tokenize once, not once per display occurrence. Unsupported lexical forms
    // cannot establish a local display role, regardless of declaration order.
    let display_grammar = CSS_PROPERTIES
        .iter()
        .find(|(name, _)| *name == "display")
        .expect("closed display grammar")
        .1;
    let visibility_grammar = CSS_PROPERTIES
        .iter()
        .find(|(name, _)| *name == "visibility")
        .expect("closed visibility grammar")
        .1;
    let roles_are_local = css_tokens(source).is_some_and(|tokens| {
        tokens.split(|t| *t == CssToken::Punct(';')).all(|declaration| {
            if declaration.is_empty() { return true; }
            let Some((property, value)) = css_declaration_parts(declaration) else {
                return !matches!(declaration.first(), Some(CssToken::Ident(name)) if name == "display" || name == "visibility");
            };
            match property {
                "display" => css_value_qualified(display_grammar, value) || css_keyword(value, &["none"]),
                // Collapse can clip cells spanning into a different row/group,
                // as well as column cells outside its DOM subtree (CSS2.2 17.5.5).
                // Explicit hidden remains local; unknown/inherited roles refuse.
                "visibility" => css_value_qualified(visibility_grammar, value) || css_keyword(value, &["hidden"]),
                _ => true,
            }
        })
    });
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(chars.len());
        } else if matches!(chars[i], '\'' | '"') {
            let quote = chars[i];
            i += 1;
            while i < chars.len() && chars[i] != quote && chars[i] != '\n' {
                if css_valid_escape(&chars, i) {
                    let _ = css_escape(&chars, &mut i);
                } else {
                    i += 1;
                }
            }
            i += usize::from(i < chars.len() && chars[i] == quote);
        } else if css_ident_start(&chars, i) {
            let Some(property) = css_ident(&chars, &mut i) else {
                return false;
            };
            // Whitespace and complete comments can separate a property and :.
            let mut end = i;
            loop {
                while chars.get(end).is_some_and(|c| css_whitespace(*c)) {
                    end += 1;
                }
                if chars.get(end) != Some(&'/') || chars.get(end + 1) != Some(&'*') {
                    break;
                }
                end += 2;
                while end < chars.len() && !(chars[end] == '*' && chars.get(end + 1) == Some(&'/'))
                {
                    end += 1;
                }
                end = (end + 2).min(chars.len());
            }
            if chars.get(end) == Some(&':') {
                let local = if matches!(property.as_str(), "display" | "visibility") {
                    roles_are_local
                } else {
                    matches!(
                        property.as_str(),
                        "background" | "background-color" | "font"
                    ) || CSS_PROPERTIES.iter().any(|(name, _)| *name == property)
                        && !matches!(
                            property.as_str(),
                            "position"
                                | "transform"
                                | "translate"
                                | "rotate"
                                | "scale"
                                | "outline"
                                | "outline-width"
                                | "outline-offset"
                                | "outline-style"
                                | "box-shadow"
                                | "text-shadow"
                                | "filter"
                                | "backdrop-filter"
                                | "mix-blend-mode"
                                | "margin"
                                | "margin-top"
                                | "margin-right"
                                | "margin-bottom"
                                | "margin-left"
                                | "text-indent"
                        )
                };
                if !local {
                    return false;
                }
            }
        } else {
            i += 1;
        }
    }
    true
}

fn css_stylesheet_declarations(source: &str, total_rules: &mut usize) -> Option<()> {
    let tokens = css_tokens(source)?;
    let mut remaining = tokens.as_slice();
    while !remaining.is_empty() {
        *total_rules += 1;
        if *total_rules > 1024 {
            return None;
        }
        let open = remaining.iter().position(|t| *t == CssToken::Punct('{'))?;
        let selector = &remaining[..open];
        // Closed simple-selector subset. Unsupported selectors refuse even if
        // a renderer might ignore them. Escaped braces stay Ident data.
        if selector.is_empty()
            || !selector.iter().all(|t| {
                matches!(
                    t,
                    CssToken::Ident(_)
                        | CssToken::Hash(_)
                        | CssToken::Punct('.' | ':' | ',' | '>' | '+' | '~' | '*')
                )
            })
        {
            return None;
        }
        let tail = &remaining[open + 1..];
        let close = tail.iter().position(|t| *t == CssToken::Punct('}'))?;
        css_declaration_tokens(&tail[..close])?;
        remaining = &tail[close + 1..];
    }
    Some(())
}
#[cfg(test)]
fn stylesheet_hiding(source: &str) -> Option<bool> {
    css_stylesheet_declarations(source, &mut 0).map(|()| false)
}
fn display_text_contract(d: &crate::dom::Document, pins: &PaperPins) -> Result<()> {
    // Declarative shadow roots replace the host's rendered light DOM. Omitting
    // only TEMPLATE would still export unslotted host text; until projection
    // understands shadow/slot composition, refuse the complete document.
    // Reject any declared mode, including unsupported values, without learning
    // browser-specific activation rules from an authenticated response.
    require(
        d.select("template[shadowrootmode]").is_empty(),
        "contract_drift",
    )?;
    // Open dialogs are out of normal flow in the native HTML rendering model,
    // even with no author position declaration. Their box can cover siblings;
    // subtree omission cannot qualify those siblings. Do not infer a safe
    // author override without cascade/rendering qualification. Closed dialogs
    // remain self-hidden in passive_text.
    require(d.select("dialog[open]").is_empty(), "contract_drift")?;
    for element in d.select("[style]") {
        require(css_inline_rejection_is_local(element), "contract_drift")?;
    }
    let styles = d.select("style");
    require(styles.len() <= 64, "contract_drift")?;
    let sources = styles
        .iter()
        .map(|style| crate::dom::content(*style))
        .collect::<Vec<_>>();
    require(
        sources.iter().map(String::len).sum::<usize>() <= 32_768,
        "contract_drift",
    )?;
    // Parse each rule once. There is no matching-element multiplier or layout
    // arithmetic; shared bytes/rules/style limits remain work admission only.
    let mut total_rules = 0;
    for source in sources {
        css_stylesheet_declarations(&source, &mut total_rules)
            .ok_or_else(|| Error::refusal("contract_drift"))?;
    }
    for link in d.select("link") {
        if link.value().attr("rel").is_some_and(|rel| {
            rel.split_ascii_whitespace()
                .any(|t| t.eq_ignore_ascii_case("stylesheet"))
        }) {
            require(d.select("base[href]").is_empty(), "contract_drift")?;
            let href = link
                .value()
                .attr("href")
                .ok_or_else(|| Error::refusal("contract_drift"))?;
            require(
                pins.stylesheet_urls.contains(&stylesheet_url(href)?),
                "contract_drift",
            )?;
        }
    }
    Ok(())
}
fn normalized(e: scraper::ElementRef<'_>) -> String {
    passive_text(e)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
// Bound repeated text/visibility work across all projections before selecting
// any output roots. One iterative tree pass charges node depth, text bytes and
// attribute bytes in both ancestor and descendant directions. Thus many
// overlapping roots cannot multiply an otherwise admitted document unchecked.
fn document_projection_work(d: &scraper::Html, budget: usize) -> Result<()> {
    let mut work = 0usize;
    let mut pending = d
        .tree
        .root()
        .children()
        .rev()
        .map(|node| (node, 1usize, 0usize))
        .collect::<Vec<_>>();
    while let Some((node, depth, ancestor_weight)) = pending.pop() {
        let own_weight = match node.value() {
            scraper::node::Node::Element(e) => e.attrs().fold(1usize, |weight, (name, value)| {
                weight
                    .saturating_add(name.len())
                    .saturating_add(value.len())
                    .saturating_add(1)
            }),
            scraper::node::Node::Text(text) => 1usize.saturating_add(text.len()),
            // Comment contents are never read by display-text traversal; retain
            // the existing two-million-unit inert-comment positive control.
            _ => 1,
        };
        work = work.saturating_add(
            depth
                .saturating_mul(own_weight)
                .saturating_add(ancestor_weight),
        );
        require(work <= budget, "paper_document_limit")?;
        let ancestor_weight = ancestor_weight.saturating_add(own_weight);
        pending.extend(
            node.children()
                .rev()
                .map(|node| (node, depth.saturating_add(1), ancestor_weight)),
        );
    }
    Ok(())
}
fn document_preflight(html: &str) -> Result<()> {
    require(
        html.len() <= 8_000_000 && html.encode_utf16().count() <= 2_000_000,
        "paper_source_size_limit",
    )?;
    // Conservative source grammar bound before HTML parsing/raw-location
    // binding, including markers inside comments/strings. Counting bytes is
    // not another HTML parser and cannot miss an actual markup opener.
    require(
        html.bytes().filter(|b| *b == b'<').take(4097).count() <= 4096,
        "paper_document_limit",
    )?;
    {
        // Reuse the native HTML5 parser for a tree-only preflight. Reject costly
        // shapes before Document's raw-location binding or any text projection.
        // Qualified input is then parsed by the unchanged source-location owner.
        let tree = scraper::Html::parse_document(html);
        require(
            tree.select(&scraper::Selector::parse("*").expect("fixed selector"))
                .count()
                <= 20_000,
            "paper_document_limit",
        )?;
        document_projection_work(&tree, 1_000_000)?;
    }
    Ok(())
}
fn document(html: &str) -> Result<crate::dom::Document> {
    document_preflight(html)?;
    let d = crate::dom::Document::parse(html);
    require(
        !d.select("input").iter().any(|e| {
            e.value()
                .attr("type")
                .is_some_and(|t| t.eq_ignore_ascii_case("password"))
        }) && !html.contains("webUserPass")
            && !html.contains("세션이 만료")
            && !html.contains("normal_session_error.jsp"),
        "authentication_required",
    )?;
    require(
        d.select("select script,select style,select template,select noscript")
            .is_empty(),
        "contract_drift",
    )?;
    Ok(d)
}
fn script_tree(source: &str) -> Result<crate::script::Node> {
    require(source.len() <= 2_000_000, "paper_source_size_limit")?;
    crate::student_metadata::metadata_script_budget(source)?;
    let tree = crate::script::Parser::new(source)
        .and_then(|mut parser| parser.program())
        .map_err(|_| Error::refusal("contract_drift"))?;
    for n in crate::script::walk(&tree).map_err(|_| Error::refusal("contract_drift"))? {
        require(n.kind != "Property" || !n.computed, "contract_drift")?;
    }
    Ok(tree)
}
/// Structural fingerprint for an explicitly supplied owner pin. This function
/// performs no admission or I/O; a source page can never update a PaperPins value.
pub fn contract_fingerprint(html: &str) -> Result<String> {
    Ok(hash(json(&page_structure(&document(html)?)?).as_bytes()))
}
/// Independently supplied owner pin for the complete observed script context.
/// Method hashes alone cannot qualify preceding abrupt completion or later
/// indirect mutations. Include source order, attributes and HTML ancestry;
/// never learn/update this pin from a dispatched response.
pub fn reader_context_fingerprint(html: &str) -> Result<String> {
    reader_context(&document(html)?)
}
fn reader_context(d: &crate::dom::Document) -> Result<String> {
    let scripts = d.select("script");
    require(scripts.len() <= 64, "paper_contract_limit")?;
    let mut context = vec![];
    // Pin every element and attribute by default: SVG animation, embedded
    // contexts and future attributes cannot bypass an execution-attribute list.
    // The independent owner pin must qualify any real dynamic compatibility.
    let elements = d.select("*");
    for script in elements {
        let mut attributes = script.value().attrs().collect::<Vec<_>>();
        require(attributes.len() <= 64, "paper_contract_limit")?;
        attributes.sort_unstable_by_key(|(name, _)| *name);
        let mut attrs = vec![];
        for (name, value) in attributes {
            bounded_text(name, 256)?;
            attrs.push(obj(&[
                ("name", name.into()),
                ("value", bounded_text(value, 4096)?),
            ]));
        }
        let mut ancestors = vec![];
        for ancestor in script.ancestors().filter_map(scraper::ElementRef::wrap) {
            require(ancestors.len() < 64, "paper_contract_limit")?;
            ancestors.push(ancestor.value().name().into());
        }
        context.push(obj(&[
            ("tag", script.value().name().into()),
            ("attributes", Js::Arr(attrs)),
            ("ancestors", Js::Arr(ancestors)),
            (
                "sourceSha256",
                if matches!(script.value().name(), "script" | "style") {
                    hash(crate::dom::content(script).as_bytes()).into()
                } else {
                    Js::Null
                },
            ),
        ]));
    }
    Ok(hash(json(&Js::Arr(context)).as_bytes()))
}
fn page_structure(d: &crate::dom::Document) -> Result<Js> {
    use crate::dom;
    let forms = d.select("form");
    require(forms.len() <= 32, "paper_contract_limit")?;
    let mut result = vec![];
    for form in forms {
        course::verify_form_ownership(d, form)?;
        let controls = dom::select(form, "input,select,textarea,button");
        require(controls.len() <= 128, "paper_contract_limit")?;
        result.push(obj(&[
            ("name", form.value().attr("name").map_or(Js::Null, Js::from)),
            ("id", form.value().attr("id").map_or(Js::Null, Js::from)),
            (
                "action",
                form.value().attr("action").map_or(Js::Null, Js::from),
            ),
            (
                "method",
                form.value().attr("method").map_or(Js::Null, Js::from),
            ),
            (
                "controls",
                Js::Arr(
                    controls
                        .into_iter()
                        .map(control_structure)
                        .collect::<Result<_>>()?,
                ),
            ),
            (
                "disabledGroups",
                dom::select(form, "fieldset[disabled],optgroup[disabled]")
                    .len()
                    .into(),
            ),
        ]));
    }
    let selects = d.select("select");
    require(selects.len() <= 64, "paper_contract_limit")?;
    let scripts = d.select("script:not([src])");
    require(scripts.len() <= 64, "paper_contract_limit")?;
    let mut script_hashes = vec![];
    for script in scripts {
        let source = dom::content(script);
        script_tree(&source)?;
        script_hashes.push(hash(source.as_bytes()).into());
    }
    Ok(obj(&[
        ("forms", Js::Arr(result)),
        (
            "selects",
            Js::Arr(
                selects
                    .into_iter()
                    .map(control_structure)
                    .collect::<Result<_>>()?,
            ),
        ),
        ("inlineScriptHashes", Js::Arr(script_hashes)),
        (
            "externalScripts",
            Js::Arr(
                d.select("script[src]")
                    .iter()
                    .map(|s| bounded_text(s.value().attr("src").unwrap_or(""), 4096))
                    .collect::<Result<_>>()?,
            ),
        ),
    ]))
}
fn control_structure(e: scraper::ElementRef<'_>) -> Result<Js> {
    use crate::dom;
    let options = dom::select(e, "option");
    require(options.len() <= 256, "paper_contract_limit")?;
    let mut attributes = obj(&[("tag", e.value().name().into())]);
    for attr in ["name", "id", "type", "form"] {
        attributes.set(
            attr,
            e.value()
                .attr(attr)
                .map_or(Ok(Js::Null), |s| bounded_text(s, 4096))?,
        );
    }
    attributes.set("value", bounded_text(&dom::value(e), 4096)?);
    for attr in ["checked", "disabled", "required", "multiple"] {
        attributes.set(attr, e.value().attr(attr).is_some().into());
    }
    attributes.set(
        "options",
        Js::Arr(
            options
                .iter()
                .map(|o| {
                    Ok(obj(&[
                        ("value", bounded_text(&dom::value(*o), 4096)?),
                        ("label", bounded_text(&dom::content(*o), 4096)?),
                        ("selected", o.value().attr("selected").is_some().into()),
                        ("disabled", o.value().attr("disabled").is_some().into()),
                        (
                            "disabledGroup",
                            dom::closest(*o, "optgroup")
                                .is_some_and(|g| g.value().attr("disabled").is_some())
                                .into(),
                        ),
                    ]))
                })
                .collect::<Result<_>>()?,
        ),
    );
    Ok(attributes)
}

impl<P: ReadPort> PaperSource<P> {
    fn contract(&self, id: &str) -> Result<()> {
        let expected = read_contracts()
            .into_iter()
            .find(|op| op.id == id)
            .ok_or_else(|| Error::refusal("paper_operation_not_supported"))?;
        require(
            self.transport.operations.iter().any(|actual| {
                actual.id == id
                    && serde_json::to_value(actual).ok() == serde_json::to_value(&expected).ok()
            }),
            "paper_contract_not_admitted",
        )
    }
    fn read_page(
        &mut self,
        scope: &TransportScope,
        id: &str,
        args: Vec<(String, String)>,
    ) -> Result<String> {
        if id.starts_with("metadata.") {
            course::check_contract(&self.transport, id)?;
        } else {
            self.contract(id)?;
        }
        self.transport.request(scope, id, &args, false)?;
        require(self.requests < 256, "paper_request_budget_exceeded")?;
        self.requests += 1;
        let response = self.port.read(scope, id, &args)?;
        // The existing response validator owns status/UTF-8/auth semantics, but
        // also binds raw HTML locations. Preflight its qualified input first;
        // preserve the existing error precedence for invalid transport input.
        if response.status == 200
            && !response.stalled
            && response.bytes.len() <= 8_000_000
            && let Ok(html) = std::str::from_utf8(&response.bytes)
            && html.encode_utf16().count() <= 2_000_000
        {
            document_preflight(html)?;
        }
        course::response_text(response)
    }
    fn shell(&mut self, student: &Student, date: &str) -> Result<crate::dom::Document> {
        let scope = self.owner.owner.scope(student, date, course::METADATA_NS);
        let html = self.read_page(&scope, "metadata.paper.initial", vec![])?;
        let d = document(&html)?;
        PaperFormPin::parse(&self.owner.form_value)?.verify(&d)?;
        require(
            course::field(&d, "form[name=userForm] [name=pri_no]")? == self.owner.owner.teacher_key,
            "student_paper_owner_drift",
        )?;
        // College is a pinned fixed control, independently matched to the owner.
        require(
            course::field(&d, "form[name=search] [name=clg_no]")? == self.owner.owner.college_key,
            "student_paper_college_scope_conflict",
        )?;
        require(
            reader_context(&d)? == self.owner.pins.reader_context,
            "contract_drift",
        )?;
        Ok(d)
    }
    /// Six source-tool shapes and their class2 aliases. Date/time are trusted
    /// explicit owner context, not guessed from a relative source label.
    pub fn read(
        &mut self,
        tool: &str,
        input: &Js,
        lesson_date: &str,
        observed_at: &str,
    ) -> Result<Js> {
        let name = if let Some(name) = tool.strip_prefix("class2_") {
            require(
                self.owner.endpoint == PaperScope::Paper2,
                "paper_scope_unresolved",
            )?;
            name
        } else {
            tool
        };
        let request = PaperRequest::parse(name, input, self.owner.endpoint)?;
        let student = self.student(input)?;
        course::date(&lesson_date.into())?;
        course::instant(observed_at)?;
        let paper_scope = self.owner.owner.scope(&student, lesson_date, PAPER_NS);
        // Every complete operation is admitted before the first request.
        if !matches!(request, PaperRequest::Inventory(_) | PaperRequest::Printed) {
            self.transport.authorize(&paper_scope)?;
        }
        let meta_scope = self
            .owner
            .owner
            .scope(&student, lesson_date, course::METADATA_NS);
        match &request {
            PaperRequest::Definition(_) | PaperRequest::Headers(_) | PaperRequest::Contracts => {
                self.transport.authorize(&meta_scope)?;
                course::check_contract(&self.transport, "metadata.paper.initial")?;
            }
            _ => {}
        }
        for id in request.contracts() {
            self.contract(id)?;
        }
        require(!self.latched, "source_failure_latched_no_retry")?;
        let before = self.requests;
        let result = self.execute(
            request,
            input,
            &student,
            lesson_date,
            observed_at,
            &paper_scope,
        );
        let result = result.and_then(|mut observation| {
            observation.set("teacherJudgment", Js::Null);
            let status = if observation.has("status") {
                observation.get("status").clone()
            } else {
                "observed".into()
            };
            let mut scope = obj(&[("studentId", student.student_key.as_str().into())]);
            if input.has("paperCode") {
                scope.set("paperCode", input.get("paperCode").clone());
            }
            let output = obj(&[
                ("status", status),
                ("endpoint", self.owner.endpoint.as_str().into()),
                ("scope", scope),
                ("observation", observation),
                (
                    "metrics",
                    obj(&[
                        ("sourceRequests", (self.requests - before).into()),
                        ("totalSourceRequests", self.requests.into()),
                    ]),
                ),
                ("academyWrites", 0.into()),
                ("parentSends", 0.into()),
                ("automaticRetryAllowed", false.into()),
            ]);
            require(
                json(&output).len() <= 8 * 1024 * 1024,
                "paper_output_budget_exceeded",
            )?;
            Ok(output)
        });
        if result.is_err() && self.requests > before {
            self.latched = true;
        }
        result
    }
    fn execute(
        &mut self,
        request: PaperRequest,
        input: &Js,
        s: &Student,
        date: &str,
        now: &str,
        scope: &TransportScope,
    ) -> Result<Js> {
        match request {
            PaperRequest::Inventory(periods) => {
                let q = if let Some(periods) = periods {
                    obj(&[("kind", "daily_history".into()), ("periods", periods)])
                } else {
                    obj(&[("kind", "daily_inventory".into())])
                };
                self.metadata(s, date, now, q)
            }
            PaperRequest::Printed => self.metadata(
                s,
                date,
                now,
                obj(&[
                    ("kind", "printed_paper_metadata".into()),
                    ("paperCode", input.get("paperCode").clone()),
                    ("period", input.get("period").clone()),
                ]),
            ),
            PaperRequest::Definition(view) => {
                let shell = self.shell(s, date)?;
                let reader = reader_method(
                    &shell,
                    if view { "viewDetailTest" } else { "ready" },
                    if view {
                        &self.owner.pins.view
                    } else {
                        &self.owner.pins.ready
                    },
                )?;
                self.definition(scope, input.get("paperCode"), view, &reader, now)
            }
            PaperRequest::Headers(view) => {
                let shell = self.shell(s, date)?;
                let reader = reader_method(
                    &shell,
                    if view { "viewDetailTest" } else { "ready" },
                    if view {
                        &self.owner.pins.view
                    } else {
                        &self.owner.pins.ready
                    },
                )?;
                let mut rows = vec![];
                for code in input.get("paperCodes").array() {
                    rows.push(self.definition(scope, code, view, &reader, now)?);
                    require(
                        json(&Js::Arr(rows.clone())).len() <= 8 * 1024 * 1024,
                        "paper_output_budget_exceeded",
                    )?;
                }
                Ok(obj(&[
                    ("status", "observed".into()),
                    ("rows", Js::Arr(rows)),
                    ("concurrencyLimit", 1.into()),
                    ("academyWrites", 0.into()),
                    ("parentSends", 0.into()),
                ]))
            }
            PaperRequest::Contracts => {
                let shell = self.shell(s, date)?;
                reader_method(&shell, "ready", &self.owner.pins.ready)?;
                reader_method(&shell, "viewDetailTest", &self.owner.pins.view)?;
                let pool = self.read_page(scope, "paper.pool", vec![])?;
                require(
                    contract_fingerprint(&pool)? == self.owner.pins.pool,
                    "contract_drift",
                )?;
                Ok(obj(&[
                    ("status", "inert_contracts_observed".into()),
                    ("studentPaperForm", inert_page(&shell)?),
                    ("testPool", inert_page(&document(&pool)?)?),
                    ("sourceStudentRowsRetained", 0.into()),
                    ("sourceHandlersExecuted", false.into()),
                ]))
            }
            PaperRequest::Source(kind) => match kind.as_str() {
                "printed_code" => {
                    let text = self.read_page(scope, "paper.converter", vec![])?;
                    require(
                        hash(text.as_bytes()) == self.owner.pins.converter,
                        "contract_drift",
                    )?;
                    converter(&text)
                }
                "daily_test_catalog" => {
                    let text = self.read_page(scope, "paper.catalog", vec![])?;
                    require(
                        contract_fingerprint(&text)? == self.owner.pins.catalog,
                        "contract_drift",
                    )?;
                    let mut value = inert_page(&document(&text)?)?;
                    value.set("sourceOperation", "GET DailyZeroTestMain".into());
                    value.set("observedAt", now.into());
                    Ok(value)
                }
                _ => self.definitions(scope, input, &kind, now),
            },
        }
    }
    fn definition(
        &mut self,
        scope: &TransportScope,
        code: &Js,
        view: bool,
        reader: &str,
        now: &str,
    ) -> Result<Js> {
        let key = decode_code(code)?;
        let id = if view {
            "paper.header"
        } else {
            "paper.definition"
        };
        let args = vec![
            (
                if view { "p_testno" } else { "test_no" }.into(),
                string(&key),
            ),
            (if view { "dummy" } else { "uniq" }.into(), now.into()),
        ];
        let text = self.read_page(scope, id, args)?;
        let information = if view {
            inspect_header(&text, &key, &self.owner.pins)?
        } else {
            parse_definition(&text)?
        };
        let mut value = obj(&[
            ("status", "observed".into()),
            ("paperCode", code.clone()),
            ("requestedPaperKey", key.clone()),
            ("information", information),
            ("readerSha256", reader.into()),
            ("observedAt", now.into()),
            ("studentAssignment", "unqualified".into()),
            ("handlersInvoked", false.into()),
            ("academyWrites", 0.into()),
            ("parentSends", 0.into()),
        ]);
        if view {
            qualify_frame(value.get("information").get("frames"), &key)?;
            let child = self.read_page(
                scope,
                "paper.content",
                vec![("p_testno".into(), string(&key))],
            )?;
            let header = inspect_header(&child, &key, &self.owner.pins)?;
            require(
                header.get("frames").array().is_empty(),
                "printed_paper_frame_unqualified",
            )?;
            self.last_header = Some(crate::history::QualifiedHeader::from_page(
                scope,
                code,
                header.get("metadataRows"),
                &child,
                now,
            )?);
            value.set("contentHeader", header);
            let mut information = value.get("information").clone();
            information.set(
                "frames",
                Js::Arr(
                    information
                        .get("frames")
                        .array()
                        .iter()
                        .map(|frame| {
                            obj(&[
                                ("tag", frame.get("tag").clone()),
                                ("srcSha256", hash(json(frame.get("src")).as_bytes()).into()),
                            ])
                        })
                        .collect(),
                ),
            );
            value.set("information", information);
        } else {
            value.set(
                "sourceOperation",
                "GET TestPageRegServlet reqCmd=testInfo".into(),
            );
            value.set(
                "identityEvidence",
                "owner_code_decode_and_exact_information_request".into(),
            );
        }
        Ok(value)
    }
}

enum PaperRequest {
    Inventory(Option<Js>),
    Printed,
    Definition(bool),
    Headers(bool),
    Contracts,
    Source(String),
}
impl PaperRequest {
    fn parse(tool: &str, v: &Js, endpoint: PaperScope) -> Result<Self> {
        let fields = |required: &[&str], optional: &[&str]| -> Result<()> {
            let mut seen = HashSet::new();
            require(
                object(v)
                    && v.props().len() <= required.len() + optional.len()
                    && v.props().iter().all(|(k, _)| seen.insert(k))
                    && required.iter().all(|s| v.has(s))
                    && v.props()
                        .iter()
                        .all(|(k, _)| required.iter().chain(optional).any(|s| k == &text(s))),
                "tool_input_invalid",
            )
        };
        let view = |default: bool| -> Result<bool> {
            if !v.has("view") {
                return Ok(default);
            }
            require(
                v.get("view").is("header") || v.get("view").is("definition"),
                "tool_input_invalid",
            )?;
            Ok(v.get("view").is("header"))
        };
        match tool {
            "academy_daily_test_inventory" => {
                fields(&["studentId"], &["periods"])?;
                if v.has("periods") {
                    let a = v.get("periods");
                    require(
                        array(a) && (1..=6).contains(&a.array().len()),
                        "tool_input_invalid",
                    )?;
                    let mut seen = HashSet::new();
                    for s in a.array() {
                        require(
                            ["11", "12", "13", "14", "15", "16"].iter().any(|t| s.is(t)),
                            "tool_input_invalid",
                        )?;
                        require(seen.insert(string(s)), "tool_input_invalid")?;
                    }
                    Ok(Self::Inventory(Some(a.clone())))
                } else {
                    Ok(Self::Inventory(None))
                }
            }
            "academy_printed_paper_lookup" => {
                exact(v, &["studentId", "paperCode", "period"])?;
                decode_code(v.get("paperCode"))?;
                exact(v.get("period"), &["from", "through"])?;
                let from = course::date(v.get("period").get("from"))?;
                let through = course::date(v.get("period").get("through"))?;
                require(from <= through, "tool_input_invalid")?;
                Ok(Self::Printed)
            }
            "academy_paper_definition_lookup" => {
                fields(&["studentId", "paperCode"], &["view"])?;
                decode_code(v.get("paperCode"))?;
                Ok(Self::Definition(view(false)?))
            }
            "academy_paper_headers" => {
                fields(
                    &["studentId", "paperCodes"],
                    if endpoint == PaperScope::Paper2 {
                        &["view"]
                    } else {
                        &[]
                    },
                )?;
                let codes = v.get("paperCodes");
                require(
                    array(codes) && (1..=32).contains(&codes.array().len()),
                    "tool_input_invalid",
                )?;
                let mut seen = HashSet::new();
                for code in codes.array() {
                    decode_code(code)?;
                    require(seen.insert(string(code)), "read_scope_not_admitted")?;
                }
                Ok(Self::Headers(view(true)?))
            }
            "academy_paper_read_contracts" => {
                exact(v, &["studentId"])?;
                Ok(Self::Contracts)
            }
            "academy_source_read_contracts" => {
                fields(&["studentId", "kind"], &["unitCodes", "unitNames"])?;
                let kind = v.get("kind");
                require(
                    [
                        "printed_code",
                        "daily_test_catalog",
                        "middle1_definitions",
                        "elementary5_fraction_definitions",
                    ]
                    .iter()
                    .any(|s| kind.is(s)),
                    "tool_input_invalid",
                )?;
                if kind.is("middle1_definitions") {
                    let units = v.get("unitCodes");
                    require(
                        array(units) && (1..=14).contains(&units.array().len()),
                        "daily_definition_unit_scope_rejected",
                    )?;
                    let mut seen = HashSet::new();
                    for unit in units.array() {
                        require(
                            unit.string() && regex_unit(&string(unit)) && seen.insert(string(unit)),
                            "daily_definition_unit_scope_rejected",
                        )?;
                    }
                    if v.has("unitNames") {
                        let names = v.get("unitNames");
                        require(
                            object(names) && names.props().len() == units.array().len(),
                            "tool_input_invalid",
                        )?;
                        for unit in units.array() {
                            let n = names.get(&string(unit));
                            require(
                                n.string() && !n.units().is_empty() && n.units().len() <= 100,
                                "tool_input_invalid",
                            )?;
                            course::name(n)?;
                        }
                    }
                } else {
                    require(
                        !v.has("unitCodes") && !v.has("unitNames"),
                        "paper_query_combination_not_supported",
                    )?;
                }
                Ok(Self::Source(string(kind)))
            }
            _ => Err(Error::refusal("paper_tool_not_supported")),
        }
    }
    fn contracts(&self) -> Vec<&'static str> {
        match self {
            Self::Definition(false) | Self::Headers(false) => vec!["paper.definition"],
            Self::Definition(true) | Self::Headers(true) => vec!["paper.header", "paper.content"],
            Self::Contracts => vec!["paper.pool"],
            Self::Source(kind) => match kind.as_str() {
                "printed_code" => vec!["paper.converter"],
                "daily_test_catalog" => vec!["paper.catalog"],
                _ => vec!["paper.catalog", "paper.definitions"],
            },
            _ => vec![],
        }
    }
}
fn regex_unit(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 3
        && b[1] == b'-'
        && match b[0] {
            b'5' => (b'1'..=b'7').contains(&b[2]),
            b'6' => (b'1'..=b'5').contains(&b[2]),
            b'7' => (b'1'..=b'2').contains(&b[2]),
            _ => false,
        }
}

fn reader_method(d: &crate::dom::Document, method: &str, expected: &str) -> Result<String> {
    let scripts = d.select("script:not([src])");
    require(scripts.len() <= 64, "paper_contract_limit")?;
    let mut bindings = 0;
    let mut hashes = vec![];
    for script in scripts {
        let source = crate::dom::content(script);
        let ast = script_tree(&source)?;
        let active = ast
            .children
            .iter()
            .filter(|n| n.kind == "VariableDeclaration")
            .flat_map(|n| &n.children)
            .filter(|n| n.kind == "VariableDeclarator")
            .map(|n| (n.start, n.end))
            .collect::<HashSet<_>>();
        for n in crate::script::walk(&ast)? {
            // The projection retains assignment targets and source spans.
            // Refuse unresolved/computed writes too, without evaluating keys.
            let raw = source
                .get(n.start..n.end)
                .ok_or_else(|| Error::refusal("contract_drift"))?;
            let mutating = n.kind == "AssignmentExpression"
                || n.kind == "Expression"
                    && (raw.starts_with("delete ")
                        || raw.starts_with("delete(")
                        || raw.starts_with("delete/")
                        || raw.starts_with("++")
                        || raw.starts_with("--")
                        || raw.ends_with("++")
                        || raw.ends_with("--"));
            if mutating && let Some(target) = n.children.first() {
                require(
                    !crate::script::walk(target)?.iter().any(|node| {
                        node.kind == "Identifier" && node.name == "testInfoDialogManager"
                    }),
                    "contract_drift",
                )?;
            }
            if n.kind != "VariableDeclarator"
                || n.children
                    .first()
                    .and_then(crate::script::Node::reference)
                    .as_deref()
                    != Some("testInfoDialogManager")
            {
                continue;
            }
            require(
                active.contains(&(n.start, n.end))
                    && script.value().attr("type").is_none_or(|t| {
                        matches!(
                            t.trim().to_ascii_lowercase().as_str(),
                            "" | "text/javascript" | "application/javascript"
                        )
                    })
                    && script.value().attr("nomodule").is_none()
                    && crate::dom::closest(script, "template").is_none()
                    && crate::dom::closest(script, "noscript").is_none(),
                "contract_drift",
            )?;
            bindings += 1;
            let object = n
                .children
                .get(1)
                .filter(|n| n.kind == "ObjectExpression")
                .ok_or_else(|| Error::refusal("contract_drift"))?;
            let mut names = HashSet::new();
            for property in &object.children {
                require(
                    property.kind == "Property"
                        && !property.computed
                        && !property.name.is_empty()
                        && names.insert(property.name.clone()),
                    "contract_drift",
                )?;
                if property.name != method {
                    continue;
                }
                let function = property
                    .children
                    .last()
                    .filter(|n| n.kind == "FunctionExpression")
                    .ok_or_else(|| Error::refusal("contract_drift"))?;
                let raw = source
                    .get(function.start..function.end)
                    .ok_or_else(|| Error::refusal("contract_drift"))?;
                require(
                    raw.starts_with("function") && raw.len() <= 20_000,
                    "contract_drift",
                )?;
                hashes.push(hash(raw.as_bytes()));
            }
        }
    }
    require(
        bindings == 1 && hashes.len() == 1 && hashes[0] == expected,
        "contract_drift",
    )?;
    Ok(hashes.remove(0))
}
fn inert_page(d: &crate::dom::Document) -> Result<Js> {
    // Validate ownership and all recursive JavaScript boundaries even though
    // values/scripts are intentionally absent from exported page metadata.
    let structure = page_structure(d)?;
    let mut attribute_count = 0usize;
    for element in d.select("*") {
        let attributes = element.value().attrs().collect::<Vec<_>>();
        require(attributes.len() <= 64, "paper_contract_limit")?;
        for (name, value) in attributes {
            bounded_text(name, 256)?;
            bounded_text(value, 4096)?;
            attribute_count += 1;
        }
    }
    let forms = structure
        .get("forms")
        .array()
        .iter()
        .map(|form| {
            let method = string(form.get("method")).to_ascii_lowercase();
            let method = match method.as_str() {
                "get" => "get".into(),
                "post" => "post".into(),
                _ if form.get("method") == &Js::Null => Js::Null,
                _ => "unqualified".into(),
            };
            obj(&[
                ("nameSha256", hash(json(form.get("name")).as_bytes()).into()),
                (
                    "actionSha256",
                    if form.get("action").string() {
                        hash(string(form.get("action")).as_bytes()).into()
                    } else {
                        Js::Null
                    },
                ),
                ("method", method),
                (
                    "fieldNameHashes",
                    Js::Arr(
                        form.get("controls")
                            .array()
                            .iter()
                            .map(|c| hash(json(c.get("name")).as_bytes()).into())
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    let mut functions = obj(&[]);
    for script in d.select("script:not([src])") {
        let source = crate::dom::content(script);
        let ast = script_tree(&source)?;
        for n in crate::script::walk(&ast)? {
            if n.kind != "FunctionDeclaration" {
                continue;
            }
            let Some(id) = n.children.first().and_then(crate::script::Node::reference) else {
                continue;
            };
            require(
                !functions.has(&hash(id.as_bytes())) && functions.props().len() < 64,
                "paper_contract_limit",
            )?;
            let raw = source
                .get(n.start..n.end)
                .ok_or_else(|| Error::refusal("contract_drift"))?;
            bounded_text(&id, 4096)?;
            functions.set(
                &hash(id.as_bytes()),
                obj(&[
                    ("sourceSha256", hash(raw.as_bytes()).into()),
                    ("classification", "inert_source_not_executed".into()),
                ]),
            );
        }
    }
    let selects = structure
        .get("selects")
        .array()
        .iter()
        .map(|select| {
            obj(&[
                ("tag", select.get("tag").clone()),
                (
                    "nameSha256",
                    hash(json(select.get("name")).as_bytes()).into(),
                ),
                ("optionCount", select.get("options").array().len().into()),
                ("contractSha256", hash(json(select).as_bytes()).into()),
            ])
        })
        .collect();
    Ok(obj(&[
        ("schemaVersion", 2.into()),
        ("attributeCount", attribute_count.into()),
        ("unknownAttributes", "opaque_in_source_sha256".into()),
        ("forms", Js::Arr(forms)),
        ("selects", Js::Arr(selects)),
        ("functions", functions),
        ("sourceSha256", hash(d.source.as_bytes()).into()),
        (
            "scriptSourceHashes",
            Js::Arr(
                structure
                    .get("externalScripts")
                    .array()
                    .iter()
                    .map(|src| hash(string(src).as_bytes()).into())
                    .collect(),
            ),
        ),
        ("sourceHandlersExecuted", false.into()),
        ("accountValuesExported", false.into()),
        ("sourceStudentRowsRetained", 0.into()),
        ("academyWrites", 0.into()),
        ("parentSends", 0.into()),
    ]))
}
fn converter(source: &str) -> Result<Js> {
    let ast = script_tree(source)?;
    let mut functions = obj(&[]);
    let mut names = std::collections::BTreeSet::new();
    let mut alphabets = std::collections::BTreeSet::new();
    let mut opaque_functions = vec![];
    for n in crate::script::walk(&ast)? {
        let name = if n.kind == "Property"
            && n.children
                .last()
                .is_some_and(|v| v.kind == "FunctionExpression")
        {
            Some(n.name.clone())
        } else if n.kind == "FunctionDeclaration" {
            n.children.first().and_then(crate::script::Node::reference)
        } else {
            None
        };
        if let Some(name) = name {
            // These are the only source-backed semantic names. A source pin
            // grants integrity, not permission to export arbitrary identifiers.
            if matches!(name.as_str(), "codeToKey" | "keyToCode") {
                names.insert(name.clone());
                require(
                    !functions.has(&name) && n.end - n.start <= 10_000,
                    "printed_code_program_ambiguous",
                )?;
                functions.set(
                    &name,
                    obj(&[
                        (
                            "sourceSha256",
                            hash(&source.as_bytes()[n.start..n.end]).into(),
                        ),
                        ("classification", "inert_source_not_executed".into()),
                    ]),
                );
            } else {
                opaque_functions.push(obj(&[
                    ("nameSha256", hash(name.as_bytes()).into()),
                    (
                        "sourceSha256",
                        hash(&source.as_bytes()[n.start..n.end]).into(),
                    ),
                ]));
            }
        }
        if let Some(Js::Str(ref u)) = n.scalar()
            && u.len() == 32
            && u.iter()
                .all(|c| (*c as u8).is_ascii_alphanumeric() && *c < 128)
            && u.iter().collect::<HashSet<_>>().len() == 32
        {
            alphabets.insert(hash(String::from_utf16(u).expect("ASCII").as_bytes()));
        }
    }
    Ok(obj(&[
        ("sourceSha256", hash(source.as_bytes()).into()),
        ("functions", functions),
        ("opaqueFunctions", Js::Arr(opaque_functions)),
        (
            "relatedFunctionNames",
            Js::Arr(names.iter().map(|n| n.as_str().into()).collect()),
        ),
        (
            "alphabetCandidates",
            Js::Arr(
                alphabets
                    .iter()
                    .map(|digest| {
                        obj(&[
                            ("sourceSha256", digest.as_str().into()),
                            ("classification", "opaque_alphabet_candidate".into()),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("sourceHandlersExecuted", false.into()),
    ]))
}

/// Reuse the existing strict, iterative XML reader. The inforesult tag is
/// renamed only in markup, never in text/CDATA, with no alternative record tags.
/// This keeps DTD/entity/root/namespace validation in a single parser.
pub fn parse_definition(source: &str) -> Result<Js> {
    require(source.len() <= 262_144, "printed_paper_info_response_limit")?;
    let mut renamed = String::with_capacity(source.len());
    let mut i = 0;
    let mut count = 0;
    let mut depth = 0usize;
    while i < source.len() {
        let rest = &source[i..];
        if rest.starts_with("<!--") || rest.starts_with("<![CDATA[") || rest.starts_with("<?") {
            let (prefix, suffix) = if rest.starts_with("<!--") {
                (4, "-->")
            } else if rest.starts_with("<![CDATA[") {
                (9, "]]>")
            } else {
                (2, "?>")
            };
            let end = rest[prefix..]
                .find(suffix)
                .map(|n| prefix + n + suffix.len())
                .ok_or_else(|| Error::refusal("printed_paper_info_xml_rejected"))?;
            renamed.push_str(&rest[..end]);
            i += end;
            continue;
        }
        if rest.starts_with('<') {
            let end = rest
                .find('>')
                .ok_or_else(|| Error::refusal("printed_paper_info_xml_rejected"))?;
            let tag = &rest[..=end];
            require(!tag.starts_with("<!"), "printed_paper_info_xml_rejected")?;
            let token = tag
                .trim_start_matches('<')
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()
                .unwrap_or("");
            require(
                token != "record" && token != "parsererror",
                "printed_paper_info_xml_rejected",
            )?;
            if !tag.starts_with("</") && !tag.ends_with("/>") {
                depth += 1;
            }
            require(depth <= 32, "paper_parser_resource_limit")?;
            if tag.starts_with("</") {
                depth = depth.saturating_sub(1);
            }
            if token == "inforesult" {
                require(
                    tag == "<inforesult>" || tag == "</inforesult>",
                    "printed_paper_info_xml_rejected",
                )?;
                if tag == "<inforesult>" {
                    count += 1;
                }
                renamed.push_str(if tag.starts_with("</") {
                    "</record>"
                } else {
                    "<record>"
                });
            } else {
                renamed.push_str(tag);
            }
            i += end + 1;
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            renamed.push_str(&rest[..end]);
            i += end;
        }
    }
    require(count == 1, "printed_paper_info_result_rejected")?;
    let text = crate::http::xml_record(&renamed)
        .map_err(|_| Error::refusal("printed_paper_info_xml_rejected"))?;
    let fields = text.split("##item##").collect::<Vec<_>>();
    require(
        (9..=32).contains(&fields.len()),
        "printed_paper_info_fields_rejected",
    )?;
    let scalar = |index: usize, max| -> Result<Js> {
        let value = fields[index].trim();
        require(!value.is_empty(), "printed_paper_info_value_rejected")?;
        bounded_text(value, max).map_err(|_| Error::refusal("printed_paper_info_value_rejected"))
    };
    let numeric = |index: usize| -> Result<Js> {
        let value = scalar(index, 15)?;
        require(
            value.units().iter().all(|c| (48..=57).contains(c)),
            "printed_paper_info_number_rejected",
        )?;
        Ok(value)
    };
    let count: usize = string(&numeric(3)?)
        .parse()
        .map_err(|_| Error::refusal("printed_paper_info_question_count_rejected"))?;
    require(
        (1..=10_000).contains(&count),
        "printed_paper_info_question_count_rejected",
    )?;
    let type_no = numeric(4)?;
    let level = numeric(5)?;
    let label = if type_no.is("1180") {
        match string(&level).as_str() {
            "1" => "기본".into(),
            "2" => "발전".into(),
            "3" => "실력".into(),
            "4" => "심화".into(),
            "5" => "총괄".into(),
            _ => Js::Null,
        }
    } else {
        Js::Null
    };
    Ok(obj(&[
        ("definitionNo", numeric(0)?),
        ("title", scalar(1, 512)?),
        ("questionCount", count.into()),
        ("typeNo", type_no),
        ("levelType", level),
        ("levelOrder", numeric(6)?),
        ("majorCategory", scalar(7, 512)?),
        ("category", scalar(8, 512)?),
        ("sourceDisplayLevelLabel", label),
        ("sourceSha256", hash(source.as_bytes()).into()),
        ("sourceFieldCount", fields.len().into()),
        ("fileLocationExported", false.into()),
        (
            "definitionNoMeaning",
            "source_testNo_not_a_verified_paper_key_echo".into(),
        ),
        ("studentAttemptBinding", "unqualified".into()),
        ("studentScore", "unobserved".into()),
    ]))
}
fn inspect_header(source: &str, key: &Js, pins: &PaperPins) -> Result<Js> {
    use crate::dom;
    require(
        source.len() <= 2_000_000,
        "printed_paper_view_response_limit",
    )?;
    let d = document(source)?;
    // A document base can change even root-relative frame URLs to a foreign
    // origin. Fixed dispatch must not qualify a different observed resource.
    require(
        d.select("base[href]").is_empty(),
        "printed_paper_frame_unqualified",
    )?;
    display_text_contract(&d, pins)?;
    let echoes = d.select("input[name=p_testno]");
    require(echoes.len() <= 1, "dailytest_header_identity_conflict")?;
    for input in echoes {
        require(input.value().attr("form").is_none(), "contract_drift")?;
        if let Some(form) = dom::closest(input, "form") {
            course::verify_form_ownership(&d, form)?;
        }
        require(
            dom::value(input) == string(key),
            "dailytest_header_identity_conflict",
        )?;
    }
    let heading_nodes =
        d.select("h1,h2,h3,h4,h5,h6,.test-title,.test_title,#testTitle,#test-title");
    require(heading_nodes.len() <= 64, "printed_paper_view_header_limit")?;
    let mut headings = vec![];
    for node in heading_nodes {
        let value = normalized(node);
        let value = bounded_text(&value, 1000)?;
        if !value.units().is_empty() && !headings.contains(&value) {
            headings.push(value);
        }
    }
    let labels = [
        "시험지 제목",
        "평가구분",
        "시험구분",
        "학년",
        "학기",
        "제작일자",
        "총 문제수",
        "객관식 문제수",
        "주관식 문제수",
        "서술형 문제수",
        "출제범위",
    ];
    let mut rows = vec![];
    for th in d.select("th") {
        let label = passive_text(th).trim().to_string();
        if !labels.contains(&label.as_str()) {
            continue;
        }
        let mut next = th.next_sibling();
        let mut values = vec![];
        while let Some(node) = next {
            next = node.next_sibling();
            let Some(e) = scraper::ElementRef::wrap(node) else {
                continue;
            };
            if e.value().name() != "td" {
                break;
            }
            values.push(normalized(e));
        }
        require(rows.len() < 64, "printed_paper_view_metadata_limit")?;
        rows.push(obj(&[
            ("label", label.into()),
            ("value", bounded_text(&values.join(" "), 2000)?),
        ]));
    }
    let frames = d.select("iframe,frame");
    require(
        frames.len() <= 64 && frames.iter().all(|f| f.value().attr("srcdoc").is_none()),
        "printed_paper_frame_unqualified",
    )?;
    Ok(obj(&[
        ("documentTitle", document_title(&d)?),
        ("headings", Js::Arr(headings)),
        ("metadataRows", Js::Arr(rows)),
        (
            "frames",
            Js::Arr(
                frames
                    .iter()
                    .map(|f| {
                        Ok(obj(&[
                            ("tag", f.value().name().to_ascii_uppercase().into()),
                            (
                                "src",
                                f.value()
                                    .attr("src")
                                    .map_or(Ok(Js::Null), |src| bounded_text(src, 4096))?,
                            ),
                        ]))
                    })
                    .collect::<Result<_>>()?,
            ),
        ),
        ("sourceSha256", hash(source.as_bytes()).into()),
        ("questionBodiesExported", false.into()),
        ("sourceHandlersExecuted", false.into()),
    ]))
}
fn qualify_frame(frames: &Js, key: &Js) -> Result<()> {
    require(
        array(frames) && frames.array().len() == 1 && frames.array()[0].get("src").string(),
        "printed_paper_frame_unqualified",
    )?;
    let base = url::Url::parse(crate::http::ORIGIN).expect("origin");
    let url = base
        .join(&string(frames.array()[0].get("src")))
        .map_err(|_| Error::refusal("printed_paper_frame_unqualified"))?;
    let pairs = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<Vec<_>>();
    require(
        url.origin() == base.origin()
            && url.path() == VIEW_PATH
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && pairs.len() == 3
            && [
                ("p_process", "TestPageTabSwitch"),
                ("p_testno", &string(key)),
                ("seltab", "1"),
            ]
            .iter()
            .all(|(k, v)| {
                pairs
                    .iter()
                    .filter(|(name, value)| name == k && value == v)
                    .count()
                    == 1
            }),
        "printed_paper_frame_unqualified",
    )
}

impl<P: ReadPort> PaperSource<P> {
    fn definitions(
        &mut self,
        scope: &TransportScope,
        input: &Js,
        kind: &str,
        now: &str,
    ) -> Result<Js> {
        let elementary = kind == "elementary5_fraction_definitions";
        let grade = if elementary { "154" } else { "124" };
        let label = if elementary {
            "[22개정] 초5"
        } else {
            "[22개정] 중1"
        };
        let units = if elementary {
            vec!["2-2".into()]
        } else {
            input.get("unitCodes").array().to_vec()
        };
        let names = if elementary {
            obj(&[("2-2", "분수의 곱셈".into())])
        } else {
            input.get("unitNames").clone()
        };
        let html = self.read_page(scope, "paper.catalog", vec![])?;
        require(
            contract_fingerprint(&html)? == self.owner.pins.catalog,
            "contract_drift",
        )?;
        let d = document(&html)?;
        display_text_contract(&d, &self.owner.pins)?;
        let forms = d.select("form[name=form1]");
        require(
            forms.len() == 1
                && forms[0]
                    .value()
                    .attr("method")
                    .is_some_and(|v| v.eq_ignore_ascii_case("post")),
            "contract_drift",
        )?;
        course::verify_form_ownership(&d, forms[0])?;
        let category = course::field(&d, "form[name=form1] [name=evalgubun]")?;
        require(
            !category.is_empty() && category.len() <= 16,
            "contract_drift",
        )?;
        let grades = d.select("select#cl1");
        require(
            grades.len() == 1
                && crate::dom::select(grades[0], "option").iter().any(|o| {
                    crate::dom::value(*o) == grade
                        && passive_text(*o) == label
                        && o.value().attr("disabled").is_none()
                }),
            "contract_drift",
        )?;
        let text = self.read_page(
            scope,
            "paper.definitions",
            vec![
                ("evalgubun".into(), category.clone()),
                ("frm_cl1".into(), grade.into()),
            ],
        )?;
        let selected = document(&text)?;
        display_text_contract(&selected, &self.owner.pins)?;
        let echoes = selected.select("[name=frm_cl1]");
        require(
            echoes.len() <= 1 && echoes.iter().all(|e| crate::dom::value(*e) == grade),
            "daily_definition_identity_conflict",
        )?;
        for echo in echoes {
            if let Some(form) = crate::dom::closest(echo, "form") {
                course::verify_form_ownership(&selected, form)?;
            }
        }
        let source_rows = definition_table_rows(&selected)?;
        let mut rows = vec![];
        for columns in &source_rows {
            if !(2..=20).contains(&columns.len()) {
                continue;
            }
            let cells = columns
                .iter()
                .map(|c| c.get("text").clone())
                .collect::<Vec<_>>();
            let joined = cells.iter().map(string).collect::<Vec<_>>().join(" ");
            let matched = units
                .iter()
                .filter(|u| {
                    unit_in_text(&joined, &string(u))
                        || names.has(&string(u))
                            && cells.iter().any(|c| {
                                string(c).split_whitespace().collect::<String>()
                                    == string(names.get(&string(u)))
                                        .split_whitespace()
                                        .collect::<String>()
                            })
                })
                .cloned()
                .collect::<Vec<_>>();
            if matched.is_empty() {
                continue;
            }
            require(
                joined.encode_utf16().count() <= 8000 && rows.len() < 256,
                "daily_definition_collection_limit",
            )?;
            rows.push(obj(&[
                ("matchedUnitCodes", Js::Arr(matched)),
                ("cells", Js::Arr(cells)),
                ("columns", Js::Arr(columns.clone())),
                (
                    "handlerHashes",
                    Js::Arr(
                        columns
                            .iter()
                            .flat_map(|c| c.get("handlerHashes").array().to_vec())
                            .collect(),
                    ),
                ),
                (
                    "linkHashes",
                    Js::Arr(
                        columns
                            .iter()
                            .flat_map(|c| c.get("linkHashes").array().to_vec())
                            .collect(),
                    ),
                ),
            ]));
        }
        let empty = rows.is_empty();
        let headers = selected.select("th");
        require(headers.len() <= 256, "paper_contract_limit")?;
        Ok(obj(&[
            ("status", "observed".into()),
            ("gradeSourceKey", grade.into()),
            ("gradeSourceLabel", label.into()),
            ("assessmentFilterSha256", hash(category.as_bytes()).into()),
            ("requestedUnitCodes", Js::Arr(units)),
            ("rows", Js::Arr(rows)),
            (
                "tableHeaders",
                Js::Arr(
                    headers
                        .iter()
                        .map(|e| bounded_text(&normalized(*e), 4000))
                        .collect::<Result<_>>()?,
                ),
            ),
            (
                "catalogRowLabels",
                Js::Arr(if empty {
                    source_rows
                        .iter()
                        .take(128)
                        .map(|r| Js::Arr(r.iter().take(2).map(|c| c.get("text").clone()).collect()))
                        .collect()
                } else {
                    vec![]
                }),
            ),
            ("observedAt", now.into()),
            (
                "coverage",
                "one_observed_definition_search_response_actual_requested_units_only".into(),
            ),
            ("sourceHandlersExecuted", false.into()),
            ("academyWrites", 0.into()),
            ("parentSends", 0.into()),
        ]))
    }
}
fn unit_in_text(text: &str, unit: &str) -> bool {
    text.match_indices(unit).any(|(at, _)| {
        let end = at + unit.len();
        (at == 0 || !text.as_bytes()[at - 1].is_ascii_digit())
            && (end == text.len() || !text.as_bytes()[end].is_ascii_digit())
    })
}
fn definition_table_rows(d: &crate::dom::Document) -> Result<Vec<Vec<Js>>> {
    use crate::dom;
    let all = d.select("tr");
    require(all.len() <= 2000, "source_table_row_limit")?;
    let tables = d
        .select("table")
        .into_iter()
        .filter(|t| {
            dom::select(*t, "th").iter().any(|th| {
                dom::closest(*th, "table").is_some_and(|p| p.id() == t.id()) && {
                    let label = passive_text(*th).split_whitespace().collect::<String>();
                    label.contains("숙제수행도DT") || label.contains("수업이해도DT")
                }
            })
        })
        .collect::<Vec<_>>();
    let mut result = vec![];
    let mut expanded_bytes = 0usize;
    if tables.is_empty() {
        for tr in all {
            let cells = dom::select(tr, ":scope > td");
            if cells.is_empty() {
                continue;
            }
            require(cells.len() <= 32, "source_table_span_rejected")?;
            let row = cells
                .iter()
                .map(|c| table_cell(*c))
                .collect::<Result<Vec<_>>>()?;
            expanded_bytes += json(&Js::Arr(row.clone())).len();
            require(
                expanded_bytes <= 8 * 1024 * 1024,
                "paper_output_budget_exceeded",
            )?;
            result.push(row);
        }
        return Ok(result);
    }
    for table in tables {
        let mut carry = std::collections::BTreeMap::<usize, (Js, usize)>::new();
        let mut previous_group = None;
        for tr in dom::select(table, "tr")
            .into_iter()
            .filter(|tr| dom::closest(*tr, "table").is_some_and(|p| p.id() == table.id()))
        {
            // HTML rowspans cannot cross row groups. Compare native parent
            // identity, including implicit TBODYs and distinct same-tag groups,
            // before applying any carried cells to the next source row.
            let group = tr.parent().map(|parent| parent.id());
            if group != previous_group {
                require(carry.is_empty(), "source_table_unterminated_rowspan")?;
                previous_group = group;
            }
            let mut row = std::collections::BTreeMap::new();
            for (&index, (cell, remaining)) in &mut carry {
                row.insert(index, cell.clone());
                *remaining -= 1;
            }
            carry.retain(|_, (_, remaining)| *remaining > 0);
            let mut column = 0;
            for cell in dom::select(tr, ":scope > td,:scope > th") {
                while row.contains_key(&column) {
                    column += 1;
                }
                let span = |name: &str, max| -> Result<usize> {
                    let n = cell
                        .value()
                        .attr(name)
                        .unwrap_or("1")
                        .parse::<usize>()
                        .map_err(|_| Error::refusal("source_table_span_rejected"))?;
                    require((1..=max).contains(&n), "source_table_span_rejected")?;
                    Ok(n)
                };
                let columns = span("colspan", 32)?;
                let rows = span("rowspan", 2000)?;
                require(column + columns <= 32, "source_table_span_rejected")?;
                let value = table_cell(cell)?;
                for index in column..column + columns {
                    require(
                        row.insert(index, value.clone()).is_none(),
                        "source_table_overlap_rejected",
                    )?;
                    if rows > 1 {
                        carry.insert(index, (value.clone(), rows - 1));
                    }
                }
                column += columns;
            }
            require(
                row.keys().copied().eq(0..row.len()),
                "source_table_span_rejected",
            )?;
            let row = row.into_values().collect::<Vec<_>>();
            expanded_bytes += json(&Js::Arr(row.clone())).len();
            require(
                expanded_bytes <= 8 * 1024 * 1024,
                "paper_output_budget_exceeded",
            )?;
            result.push(row);
            require(result.len() <= 2000, "source_table_row_limit")?;
        }
        require(carry.is_empty(), "source_table_unterminated_rowspan")?;
    }
    Ok(result)
}
fn table_cell(cell: scraper::ElementRef<'_>) -> Result<Js> {
    use crate::dom;
    let mut handlers = dom::select(cell, "[onclick]");
    if cell.value().attr("onclick").is_some() {
        handlers.insert(0, cell);
    }
    let links = dom::select(cell, "a[href]");
    require(
        handlers.len() <= 64 && links.len() <= 64,
        "paper_contract_limit",
    )?;
    Ok(obj(&[
        ("text", bounded_text(&normalized(cell), 4000)?),
        ("tag", cell.value().name().to_ascii_uppercase().into()),
        (
            "handlerHashes",
            Js::Arr(
                handlers
                    .iter()
                    .map(|e| {
                        bounded_text(e.value().attr("onclick").unwrap_or(""), 4096)
                            .map(|value| hash(string(&value).as_bytes()).into())
                    })
                    .collect::<Result<_>>()?,
            ),
        ),
        (
            "linkHashes",
            Js::Arr(
                links
                    .iter()
                    .map(|e| {
                        bounded_text(e.value().attr("href").unwrap_or(""), 4096)
                            .map(|value| hash(string(&value).as_bytes()).into())
                    })
                    .collect::<Result<_>>()?,
            ),
        ),
    ]))
}

fn append_inventory_detail(page: &mut Js, detail: &Js) {
    for field in [
        "filterContracts",
        "displayRows",
        "gradingPresence",
        "handlersInvoked",
    ] {
        page.set(field, detail.get(field).clone());
    }
}
fn inventory_detail(d: &crate::dom::Document, student: &Student, pins: &PaperPins) -> Result<Js> {
    display_text_contract(d, pins)?;
    use crate::dom;
    let page = inert_page(d)?;
    let mut controls = vec![];
    // Only the named relative-period SELECT is a public filter capability.
    // Do not reuse owner-pin control_structure: it intentionally retains raw
    // values/IDs for fingerprinting and must never project arbitrary controls.
    let allowed = |value: &str| matches!(value, "1" | "11" | "12" | "13" | "14" | "15" | "16");
    for control in d.select("select[name=sort_date]") {
        require(
            controls.is_empty() && control.value().attr("form").is_none(),
            "contract_drift",
        )?;
        if let Some(form) = dom::closest(control, "form") {
            course::verify_form_ownership(d, form)?;
        }
        let value = dom::value(control);
        require(allowed(&value), "daily_history_selector_drift")?;
        let source_options = dom::select(control, "option");
        require(source_options.len() <= 64, "paper_contract_limit")?;
        let mut options = vec![];
        for option in source_options {
            let value = dom::value(option);
            if option.value().attr("disabled").is_some() || !allowed(&value) {
                continue;
            }
            options.push(obj(&[
                ("value", value.into()),
                ("label", bounded_text(&passive_text(option), 200)?),
            ]));
        }
        controls.push(obj(&[
            ("tag", "select".into()),
            ("name", "sort_date".into()),
            ("value", value.into()),
            ("options", Js::Arr(options)),
        ]));
    }
    let rows = crate::student_metadata::metadata_rows(&d.source, student)?;
    let mut display = vec![];
    let mut display_ids = HashSet::new();
    let mut id_index = std::collections::HashMap::<String, Vec<scraper::ElementRef<'_>>>::new();
    for element in d.select("[id]") {
        id_index
            .entry(element.value().attr("id").unwrap_or("").into())
            .or_default()
            .push(element);
    }
    let mut bytes = 0;
    for row in rows {
        if !row.get("sourceRowId").string() {
            continue;
        }
        let ids = id_index
            .get(&string(row.get("sourceRowId")))
            .map_or(&[][..], Vec::as_slice);
        require(ids.len() <= 1, "contract_drift")?;
        let Some(node) = ids.first().filter(|e| e.value().name() == "tr") else {
            continue;
        };
        require(
            display_ids.insert(string(row.get("sourceRowId"))),
            "contract_drift",
        )?;
        let cells = dom::select(*node, ":scope > td");
        require(cells.len() <= 32, "paper_contract_limit")?;
        let value = obj(&[
            ("paperKey", row.get("paperKey").clone()),
            ("attemptKey", row.get("attemptKey").clone()),
            (
                "sourceRowIdSha256",
                hash(json(row.get("sourceRowId")).as_bytes()).into(),
            ),
            (
                "cells",
                Js::Arr(
                    cells
                        .iter()
                        .map(|e| bounded_text(&normalized(*e), 4000))
                        .collect::<Result<_>>()?,
                ),
            ),
        ]);
        bytes += json(&value).len();
        require(bytes <= 8 * 1024 * 1024, "paper_output_budget_exceeded")?;
        display.push(value);
    }
    Ok(obj(&[
        (
            "filterContracts",
            obj(&[
                ("forms", page.get("forms").clone()),
                ("functions", page.get("functions").clone()),
                ("controls", Js::Arr(controls)),
                ("scriptSourceHashes", page.get("scriptSourceHashes").clone()),
            ]),
        ),
        ("displayRows", Js::Arr(display)),
        (
            "gradingPresence",
            "source_score_alone_not_teacher_review".into(),
        ),
        ("handlersInvoked", false.into()),
    ]))
}

#[cfg(test)]
mod css_tests {
    use super::*;

    fn escaped_first(value: &str, whitespace: &str) -> String {
        let c = value.chars().next().unwrap();
        format!("\\{:x}{whitespace}{}", c as u32, &value[c.len_utf8()..])
    }

    #[test]
    fn every_allowlisted_property_has_a_bounded_token_grammar() {
        let mut records = Vec::new();
        for (property, grammar) in CSS_PROPERTIES {
            let (good, bad) = match grammar {
                CssGrammar::Keywords(words) => (words[0], "unqualified"),
                CssGrammar::Lengths(_) => ("128px", "128.01px"),
                CssGrammar::PositiveLength => ("128px", "0px"),
                CssGrammar::Size => ("100%", "100.01%"),
                CssGrammar::LineHeight => ("4", "4.01"),
                CssGrammar::Spacing => ("128px", "128.01px"),
                CssGrammar::Border => ("128px solid transparent", "128.01px solid transparent"),
                CssGrammar::Color(_) | CssGrammar::Colors => ("#123456", "#12345"),
                CssGrammar::Opacity => ("1", "1.01"),
                CssGrammar::Weight => ("900", "901"),
                CssGrammar::Zero(_) => ("0px", "1px"),
            };
            let mut cases = vec![
                ("in-bound", format!("{property}:{good}"), true),
                ("out-of-bound", format!("{property}:{bad}"), false),
                (
                    "escaped",
                    format!("{}:{good}", escaped_first(property, " ")),
                    true,
                ),
                (
                    "uppercase",
                    format!(
                        "{}:{}",
                        property.to_ascii_uppercase(),
                        good.to_ascii_uppercase()
                    ),
                    true,
                ),
                ("important", format!("{property}:{good}!IMPORTANT"), true),
                (
                    "comments-between-tokens",
                    format!("{property}/**/:/**/{good}/**/!/**/important"),
                    true,
                ),
                (
                    "comments-inside-identifier",
                    format!("{}/**/{}:{good}", &property[..1], &property[1..]),
                    false,
                ),
                (
                    "extra-token",
                    format!("{property}:{good} unexpected"),
                    false,
                ),
            ];
            for (name, newline) in [
                ("CR", "\r"),
                ("LF", "\n"),
                ("FF", "\u{c}"),
                ("CRLF", "\r\n"),
            ] {
                cases.push((
                    name,
                    format!("{}:{good}", escaped_first(property, newline)),
                    true,
                ));
            }
            // Exercise escaped value identifiers, hash digits and units too.
            let encoded_value = if let Some(hex) = good.strip_prefix('#') {
                format!("#{}", escaped_first(hex, "\r\n"))
            } else if good.starts_with(|c: char| c.is_ascii_alphabetic()) {
                escaped_first(good, "\r\n")
            } else if good.contains("px") {
                good.replace("px", "\\70\r\nx")
            } else {
                good.to_owned()
            };
            cases.push(("escaped-value", format!("{property}:{encoded_value}"), true));
            for (case, css, admitted) in cases {
                assert_eq!(
                    css_declarations(&css) == Some(false),
                    admitted,
                    "{property}/{case}"
                );
                assert_eq!(
                    stylesheet_hiding(&format!("h1{{{css}}}")) == Some(false),
                    admitted,
                    "STYLE {property}/{case}"
                );
                records.push(obj(&[
                    ("property", (*property).into()),
                    ("case", case.into()),
                    ("admitted", admitted.into()),
                ]));
            }
        }
        assert_eq!(records.len(), CSS_PROPERTIES.len() * 13);
        if let Ok(root) = std::env::var("E09B_FIX1_EVIDENCE") {
            std::fs::write(
                std::path::Path::new(&root).join("css-property-matrix.json"),
                json(&Js::Arr(records)),
            )
            .unwrap();
        }
    }

    #[test]
    fn document_projection_work_has_exact_and_composed_bounds() {
        let plain = crate::dom::Document::parse("<div>X</div>");
        assert!(document_projection_work(&plain.tree, 23).is_ok());
        assert!(document_projection_work(&plain.tree, 22).is_err());
        let attribute = crate::dom::Document::parse("<div class=x>X</div>");
        assert!(document_projection_work(&attribute.tree, 51).is_ok());
        assert!(document_projection_work(&attribute.tree, 50).is_err());
        assert!(document_projection_work(&attribute.tree, 52).is_ok());
        // Large inherited style text is charged for every potential descendant
        // re-read, even when that CSS contains only otherwise harmless comments.
        let large_attribute = crate::dom::Document::parse(&format!(
            "<div style='/*{}*/'>{}</div>",
            "a".repeat(4000),
            "<span>X</span>".repeat(1000)
        ));
        assert!(document_projection_work(&large_attribute.tree, 1_000_000).is_err());
        assert!(document(&"<!--".repeat(4096)).is_ok());
        assert!(document(&"<!--".repeat(4097)).is_err());
    }

    #[test]
    fn css_syntax_preprocessing_and_token_identity_are_preserved() {
        assert_eq!(
            css_preprocess("a\r\nb\rc\u{c}d\0"),
            Some("a\nb\nc\nd\u{fffd}".chars().collect())
        );
        for escape in ["\\0 ", "\\d800 ", "\\110000 ", "\\"] {
            let tokens = css_tokens(escape).unwrap();
            assert_eq!(tokens, vec![CssToken::Ident("\u{fffd}".into())]);
        }
        assert_eq!(
            css_tokens("n/**/one"),
            Some(vec![
                CssToken::Ident("n".into()),
                CssToken::Ident("one".into())
            ])
        );
        assert_eq!(
            css_tokens("b\\3b order"),
            Some(vec![CssToken::Ident("b;order".into())])
        );
        for source in [
            "display:\\6e\r\none",
            "display:\\00006e\r\none",
            "d\\69\r\nsplay:none",
            "display:n\\6f\u{c}ne",
            "display:bl/**/ock",
            "d/**/isplay:block",
            "padding:1/**/px",
            "border:1e4px solid black",
            "border:10000\\70 x solid black",
            "width:calc(1px)",
            "width:var(--x)",
            "left:1vh",
            "left:-1px",
            "font-size:1e999px",
            "font-size:0px",
            "unknown:0",
            "color:red\\;display\\:none",
            "display:block;\\7d x\\7b display:none",
            "color:'red'",
            "color:red;/*",
            "color:red\u{b}",
            "color:red\0",
            "opacity:1!important!important",
        ] {
            assert_ne!(
                css_declarations(source),
                Some(false),
                "unqualified syntax admitted"
            );
            assert_ne!(stylesheet_hiding(&format!("h1{{{source}}}")), Some(false));
        }
        for source in [
            "border:128px solid black",
            "width:128px",
            "padding:0 128px",
            "padding:0e4px",
            "padding:1.28e2px",
            "padding:0vh",
            "color:r\\65\r\nd",
            "color:red!im\\70\r\nortant",
        ] {
            assert_eq!(
                css_declarations(source),
                Some(false),
                "positive syntax refused"
            );
        }
        assert!(css_tokens(&"x".repeat(32769)).is_none());
        assert!(css_tokens(&";".repeat(8193)).is_none());
        assert!(css_declarations(&"color:red;".repeat(1025)).is_none());
        assert!(stylesheet_hiding(&"h1{color:red}".repeat(1025)).is_none());
    }
}
