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
            require(self.daily_pages.len() < 7, "paper_contract_limit")?;
            let student = self
                .student
                .as_ref()
                .ok_or_else(|| Error::refusal("read_scope_not_admitted"))?;
            self.daily_pages.push(inventory_detail(&d, student)?);
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
}
impl PaperPins {
    pub fn parse(v: &Js) -> Result<Self> {
        exact(
            v,
            &[
                "schemaVersion",
                "readerContext",
                "ready",
                "view",
                "catalog",
                "pool",
                "converter",
            ],
        )?;
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
        Ok(Self {
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
fn normalized(e: scraper::ElementRef<'_>) -> String {
    crate::dom::content(e)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn document(html: &str) -> Result<crate::dom::Document> {
    require(
        html.len() <= 8_000_000 && html.encode_utf16().count() <= 2_000_000,
        "paper_source_size_limit",
    )?;
    let d = crate::dom::Document::parse(html);
    require(d.select("*").len() <= 20_000, "paper_document_limit")?;
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
    for script in scripts {
        let mut attributes = script.value().attrs().collect::<Vec<_>>();
        require(attributes.len() <= 32, "paper_contract_limit")?;
        attributes.sort_unstable_by_key(|(name, _)| *name);
        let mut attrs = vec![];
        for (name, value) in attributes {
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
            ("attributes", Js::Arr(attrs)),
            ("ancestors", Js::Arr(ancestors)),
            (
                "sourceSha256",
                hash(crate::dom::content(script).as_bytes()).into(),
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
        course::response_text(self.port.read(scope, id, &args)?)
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
            inspect_header(&text, &key)?
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
            let header = inspect_header(&child, &key)?;
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
                ("name", form.get("name").clone()),
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
                    "fieldNames",
                    Js::Arr(
                        form.get("controls")
                            .array()
                            .iter()
                            .map(|c| c.get("name").clone())
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
                !functions.has(&id) && functions.props().len() < 64,
                "paper_contract_limit",
            )?;
            let raw = source
                .get(n.start..n.end)
                .ok_or_else(|| Error::refusal("contract_drift"))?;
            functions.set(
                &id,
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
        .filter(|s| {
            let name =
                format!("{} {}", string(s.get("name")), string(s.get("id"))).to_ascii_lowercase();
            ![
                "student", "stu_", "pri_", "teacher", "password", "cookie", "session",
            ]
            .iter()
            .any(|n| name.contains(n))
        })
        .cloned()
        .collect();
    Ok(obj(&[
        ("schemaVersion", 1.into()),
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
            let lower = name.to_ascii_lowercase();
            if lower.contains("code") || lower.contains("key") || lower.contains("32") {
                names.insert(name.clone());
                if ["codeToKey", "keyToCode"].contains(&name.as_str()) || name.contains("32") {
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
                }
            }
        }
        if let Some(Js::Str(ref u)) = n.scalar()
            && u.len() == 32
            && u.iter()
                .all(|c| (*c as u8).is_ascii_alphanumeric() && *c < 128)
            && u.iter().collect::<HashSet<_>>().len() == 32
        {
            alphabets.insert(String::from_utf16(u).expect("ASCII"));
        }
    }
    Ok(obj(&[
        ("sourceSha256", hash(source.as_bytes()).into()),
        ("functions", functions),
        (
            "relatedFunctionNames",
            Js::Arr(names.iter().map(|n| n.as_str().into()).collect()),
        ),
        (
            "alphabetCandidates",
            Js::Arr(alphabets.iter().map(|n| n.as_str().into()).collect()),
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
fn inspect_header(source: &str, key: &Js) -> Result<Js> {
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
    let mut headings = vec![];
    for node in d.select("h1,h2,h3,h4,h5,h6,.test-title,.test_title,#testTitle,#test-title") {
        let value = normalized(node);
        require(headings.len() < 64, "printed_paper_view_header_limit")?;
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
        let label = dom::content(th).trim().to_string();
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
        (
            "documentTitle",
            bounded_text(
                &d.select("title")
                    .first()
                    .map(|e| dom::content(*e))
                    .unwrap_or_default(),
                1000,
            )?,
        ),
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
                        && crate::dom::content(*o) == label
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
                    "handlers",
                    Js::Arr(
                        columns
                            .iter()
                            .flat_map(|c| c.get("handlers").array().to_vec())
                            .collect(),
                    ),
                ),
                (
                    "links",
                    Js::Arr(
                        columns
                            .iter()
                            .flat_map(|c| c.get("links").array().to_vec())
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
            ("assessmentFilter", category.into()),
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
                    let label = dom::content(*th).split_whitespace().collect::<String>();
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
        for tr in dom::select(table, "tr")
            .into_iter()
            .filter(|tr| dom::closest(*tr, "table").is_some_and(|p| p.id() == table.id()))
        {
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
            "handlers",
            Js::Arr(
                handlers
                    .iter()
                    .map(|e| bounded_text(e.value().attr("onclick").unwrap_or(""), 4096))
                    .collect::<Result<_>>()?,
            ),
        ),
        (
            "links",
            Js::Arr(
                links
                    .iter()
                    .map(|e| bounded_text(e.value().attr("href").unwrap_or(""), 4096))
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
fn inventory_detail(d: &crate::dom::Document, student: &Student) -> Result<Js> {
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
                ("label", bounded_text(&dom::content(option), 200)?),
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
            ("sourceRowId", row.get("sourceRowId").clone()),
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
