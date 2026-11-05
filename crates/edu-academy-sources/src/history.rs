//! Explicit history-only source algorithms. No HTTP, credentials or native factory.
//! Ports return raw source observations, never MCP tool/composition successes.
use crate::data::*;
use edu_academy_core::{
    instant::{Grammar, epoch_ms},
    js::test,
};
const COVERAGE: &str = "one_observed_student_search_page_not_complete_history";
fn require(ok: bool, code: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(Error::error(code)) }
}
fn key(v: &Js) -> Result<Js> {
    // A page can contain 2,000 rows. Compile this source-constant grammar once,
    // rather than recompiling it for every identity in every result half.
    static KEY: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    require(
        v.string()
            && KEY
                .get_or_init(|| edu_academy_core::js::regex("^[1-9][0-9]{0,15}$"))
                .is_match(&v.code_units()),
        "native_metadata_row_identity_conflict",
    )?;
    Ok(v.clone())
}
fn source_key(v: &Js) -> Result<Js> {
    match v {
        Js::Num(n)
            if n.is_finite() && *n > 0.0 && *n <= 9_007_199_254_740_991.0 && n.fract() == 0.0 =>
        {
            Ok(format!("{n:.0}").into())
        }
        Js::Str(u)
            if !u.is_empty()
                && u.iter().all(|c| (48..=57).contains(c))
                && u.iter().any(|c| *c != 48) =>
        {
            Ok(v.clone())
        }
        _ => Err(Error::error("invalid_source_key")),
    }
}
fn date(v: &Js) -> bool {
    v.string() && edu_academy_core::schema::validate("dailyRecordDateSchema", v).is_ok()
}
fn instant(v: &Js) -> Result<Js> {
    require(
        epoch_ms(v, Grammar::DailyRecord).is_some(),
        "native_metadata_row_identity_conflict",
    )?;
    Ok(v.clone())
}
/// Opaque native observation. Only the qualified source reader can construct it;
/// transport/session authority and question bodies are never exported with it.
pub struct QualifiedHeader {
    teacher_id: String,
    raw: Js,
    source_sha256: String,
}
impl QualifiedHeader {
    pub(crate) fn from_page(
        scope: &crate::owner_admission::TransportScope,
        code: &Js,
        rows: &Js,
        source: &str,
        now: &str,
    ) -> Result<Self> {
        let paper = decode_code(code)?;
        require(source.len() <= 2_000_000, "source_headers_read_failed")?;
        Ok(Self {
            teacher_id: scope.teacher_id.clone(),
            raw: obj(&[
                ("studentId", scope.student_id.as_str().into()),
                ("paperKey", paper),
                ("paperCode", code.clone()),
                ("metadataRows", rows.clone()),
                ("observedAt", now.into()),
            ]),
            source_sha256: hash(source.as_bytes()),
        })
    }
}
/// Explicit source boundary. Implementations own authentication/transport and must
/// return the actual dated rows, raw embedded DT list and raw header observation.
/// Native header loans expose their immutable admission before construction.
/// Historical raw ports remain available for synthetic compatibility.
pub trait Port {
    fn dated_students(&mut self, input: &Js) -> Result<Js>;
    fn daily_page(&mut self, student: &Js, selector: &Js) -> Result<Js>;
    fn paper_header(&mut self, student: &Js, paper_code: &Js) -> Result<Js>;
    /// None declares the original raw capability, not a failed native read.
    fn paper_admission(&self) -> Option<&crate::owner_admission::TransportAdmission> {
        None
    }
    fn qualified_paper_header(
        &mut self,
        _student: &Js,
        _paper_code: &Js,
    ) -> Option<Result<QualifiedHeader>> {
        None
    }
}
/// Separately admitted source loan. Private fields prevent request arguments from
/// replacing its namespace policy, managed-name set or paper-student scope.
pub struct History<P: Port> {
    port: P,
    routing_admission: Js,
    metadata_admission: Js,
    students: Vec<Js>,
    managed_names: Vec<Js>,
    paper_students: Vec<Js>,
    latched: bool,
    requests: usize,
}
impl<P: Port> History<P> {
    pub fn new(
        port: P,
        routing_admission: Js,
        metadata_admission: Js,
        students: Vec<Js>,
        managed_names: Vec<Js>,
        paper_students: Vec<Js>,
    ) -> Result<Self> {
        let p = &metadata_admission;
        require(
            object(p)
                && p.props().iter().all(|(k, _)| {
                    [
                        "schemaVersion",
                        "sourceNamespace",
                        "teacherId",
                        "allowedFrom",
                        "allowedThrough",
                        "studentScope",
                        "relativeSelectors",
                        "operations",
                    ]
                    .iter()
                    .any(|s| k == &text(s))
                })
                && p.get("schemaVersion").is("academy.metadata-admission/1")
                && p.get("sourceNamespace").is("academy.lms.metadata"),
            "metadata_namespace_not_admitted",
        )?;
        key(p.get("teacherId"))?;
        require(
            p.get("teacherId") == routing_admission.get("teacherId"),
            "native_metadata_scope_refused",
        )?;
        for k in ["allowedFrom", "allowedThrough"] {
            require(
                p.get(k) == &Js::Null || date(p.get(k)),
                "metadata_namespace_not_admitted",
            )?;
        }
        require(
            p.get("allowedFrom") == &Js::Null
                || p.get("allowedThrough") == &Js::Null
                || p.get("allowedFrom").units() <= p.get("allowedThrough").units(),
            "metadata_namespace_not_admitted",
        )?;
        require(
            p.get("studentScope").is("configured") || p.get("studentScope").is("source_verified"),
            "metadata_namespace_not_admitted",
        )?;
        let selectors = p.get("relativeSelectors");
        require(
            !p.has("relativeSelectors")
                || array(selectors)
                    && selectors.array().len() <= 7
                    && selectors.array().iter().all(|s| {
                        ["1", "11", "12", "13", "14", "15", "16"]
                            .iter()
                            .any(|x| s.is(x))
                    }),
            "metadata_namespace_not_admitted",
        )?;
        let operations = p.get("operations");
        require(
            array(operations)
                && (1..=5).contains(&operations.array().len())
                && operations.array().iter().enumerate().all(|(i, op)| {
                    [
                        "student_papers",
                        "daily_metadata",
                        "daily_inventory",
                        "daily_history",
                        "printed_paper_metadata",
                    ]
                    .iter()
                    .any(|s| op.is(s))
                        && !operations.array()[..i].contains(op)
                }),
            "metadata_namespace_not_admitted",
        )?;
        for s in &students {
            key(s.get("studentId"))?;
            crate::native::literal_name(s.get("displayName"))?;
        }
        for n in &managed_names {
            crate::native::literal_name(n)?;
        }
        for s in &paper_students {
            key(s)?;
        }
        if let Some(native) = port.paper_admission() {
            native.validate()?;
            crate::student_course::require(
                native.teacher_id == string(p.get("teacherId"))
                    && paper_students
                        .iter()
                        .all(|id| native.student_ids.contains(&string(id))),
                "native_metadata_scope_refused",
            )?;
        }
        Ok(Self {
            port,
            routing_admission,
            metadata_admission,
            students,
            managed_names,
            paper_students,
            latched: false,
            requests: 0,
        })
    }
    pub fn routing_admission(&self) -> &Js {
        &self.routing_admission
    }
    pub fn paper_students(&self) -> &[Js] {
        &self.paper_students
    }
    fn authorize_metadata(&self, input: &Js) -> Result<()> {
        let p = &self.metadata_admission;
        let date = input.get("lessonDate");
        require(
            self.managed_names.contains(input.get("displayName"))
                && (p.get("studentScope").is("source_verified")
                    || self
                        .students
                        .iter()
                        .any(|s| s.get("displayName") == input.get("displayName")))
                && p.get("operations")
                    .array()
                    .iter()
                    .any(|s| s.is("daily_history"))
                && (p.get("allowedFrom") == &Js::Null
                    || date.units() >= p.get("allowedFrom").units())
                && (p.get("allowedThrough") == &Js::Null
                    || date.units() <= p.get("allowedThrough").units())
                && std::iter::once(&Js::from("1"))
                    .chain(input.get("queries").array()[0].get("periods").array())
                    .all(|s| p.get("relativeSelectors").array().contains(s)),
            "metadata_namespace_not_admitted",
        )
    }
    /// Actual dated occurrence identity is established before any DT page read.
    pub fn metadata(&mut self, input: &Js) -> Result<Js> {
        require(
            object(input)
                && date(input.get("lessonDate"))
                && input.get("displayName").string()
                && array(input.get("queries"))
                && input.get("queries").array().len() == 1,
            "tool_input_invalid",
        )?;
        let query = &input.get("queries").array()[0];
        let periods = query.get("periods");
        require(
            query.get("kind").is("daily_history")
                && array(periods)
                && (1..=6).contains(&periods.array().len())
                && periods.array().iter().enumerate().all(|(i, p)| {
                    ["11", "12", "13", "14", "15", "16"].iter().any(|s| p.is(s))
                        && !periods.array()[..i].contains(p)
                }),
            "tool_input_invalid",
        )?;
        self.authorize_metadata(input)?;
        require(!self.latched, "source_failure_latched_no_retry")?;
        let result = self.metadata_inner(input);
        if result.is_err() {
            self.latched = true;
        }
        result
    }
    fn metadata_inner(&mut self, input: &Js) -> Result<Js> {
        self.requests += 1;
        let rows = self.port.dated_students(input)?;
        require(array(&rows), "native_dated_name_scope_conflict")?;
        require(
            rows.array().iter().all(|r| {
                r.get("teacherKey") == self.metadata_admission.get("teacherId")
                    && r.get("studentName") == input.get("displayName")
                    && r.get("date") == input.get("lessonDate")
            }),
            "native_dated_name_scope_conflict",
        )?;
        require(rows.array().len() <= 1, "native_dated_name_ambiguous")?;
        require(
            rows.array().len() == 1,
            "native_dailyrecord_occurrence_unverified",
        )?;
        let student_key = key(rows.array()[0].get("studentKey"))?;
        require(
            self.students
                .iter()
                .filter(|s| s.get("displayName") == input.get("displayName"))
                .all(|s| s.get("studentId") == &student_key),
            "native_dated_name_scope_conflict",
        )?;
        let student = obj(&[
            ("studentKey", student_key.clone()),
            ("studentName", input.get("displayName").clone()),
        ]);
        let query = input.get("queries").array()[0].clone();
        self.requests += 1;
        let current_raw = self.port.daily_page(&student, &"1".into())?;
        require(
            array(current_raw.get("relativeSelectors"))
                && query
                    .get("periods")
                    .array()
                    .iter()
                    .all(|s| current_raw.get("relativeSelectors").array().contains(s)),
            "daily_history_selector_drift",
        )?;
        let current = project_page(&current_raw, &student, &"1".into())?;
        let mut observations = vec![];
        for selector in query.get("periods").array() {
            self.requests += 1;
            let raw = self.port.daily_page(&student, selector)?;
            observations.push(project_page(&raw, &student, selector)?);
        }
        Ok(obj(&[("status", "observed".into()), ("scope", obj(&[("sourceNamespace", "academy.lms.metadata".into()),
            ("teacherId", self.metadata_admission.get("teacherId").clone()), ("studentId", student_key), ("lessonDate", input.get("lessonDate").clone())])),
            ("results", Js::Arr(vec![obj(&[("query", query), ("observation", obj(&[("status", "observed".into()),
                ("studentKey", student.get("studentKey").clone()), ("studentName", student.get("studentName").clone()),
                ("currentFilterQualification", obj(&[("observedAt", current.get("observedAt").clone()), ("selector", "1".into()), ("rows", current.get("rows").clone())])),
                ("observations", Js::Arr(observations)), ("coverage", "all_parsed_embedded_rows_on_each_observed_response_not_complete_history".into())]))])]))]))
    }
    pub fn headers(&mut self, input: &Js) -> Result<Js> {
        require(
            object(input)
                && array(input.get("paperCodes"))
                && (1..=32).contains(&input.get("paperCodes").array().len()),
            "tool_input_invalid",
        )?;
        key(input.get("studentId"))?;
        for code in input.get("paperCodes").array() {
            decode_code(code)?;
        }
        require(
            self.paper_students.contains(input.get("studentId")),
            "read_scope_not_admitted",
        )?;
        require(!self.latched, "source_failure_latched_no_retry")?;
        let result = self.headers_inner(input);
        if result.is_err() {
            self.latched = true;
        }
        result
    }
    fn headers_inner(&mut self, input: &Js) -> Result<Js> {
        let mut rows = vec![];
        for code in input.get("paperCodes").array() {
            self.requests += 1;
            let (raw, native_hash) = if self.port.paper_admission().is_some() {
                let header = self
                    .port
                    .qualified_paper_header(input.get("studentId"), code)
                    .ok_or_else(|| Error::refusal("source_headers_read_failed"))??;
                crate::student_course::require(
                    header.teacher_id == string(self.metadata_admission.get("teacherId")),
                    "native_metadata_scope_refused",
                )?;
                (header.raw, Some(header.source_sha256))
            } else {
                (self.port.paper_header(input.get("studentId"), code)?, None)
            };
            let expected = decode_code(code)?;
            require(
                raw.get("paperKey") == &expected
                    && raw.get("paperCode") == code
                    && raw.get("studentId") == input.get("studentId"),
                "dailytest_header_identity_conflict",
            )?;
            let fields = raw.get("metadataRows");
            require(
                array(fields)
                    && fields.array().len() <= 128
                    && fields.array().iter().all(|r| {
                        r.get("label").string()
                            && r.get("label").units().len() <= 200
                            && r.get("value").string()
                            && r.get("value").units().len() <= 2000
                    }),
                "source_headers_read_failed",
            )?;
            // Native loans already hash the exact raw response at its qualified
            // source boundary. Never hash a reserialized header projection.
            let source_sha256 = if let Some(hash) = native_hash {
                hash
            } else {
                require(raw.get("sourceText").string(), "source_headers_read_failed")?;
                let source_text = String::from_utf16(raw.get("sourceText").units())
                    .map_err(|_| Error::error("source_headers_read_failed"))?;
                hash(source_text.as_bytes())
            };
            rows.push(obj(&[
                ("status", "observed".into()),
                ("paperCode", code.clone()),
                ("requestedPaperKey", expected),
                ("observedAt", instant(raw.get("observedAt"))?),
                (
                    "contentHeader",
                    obj(&[
                        ("sourceSha256", source_sha256.into()),
                        (
                            "metadataRows",
                            Js::Arr(
                                fields
                                    .array()
                                    .iter()
                                    .map(|r| {
                                        obj(&[
                                            ("label", r.get("label").clone()),
                                            ("value", r.get("value").clone()),
                                        ])
                                    })
                                    .collect(),
                            ),
                        ),
                    ]),
                ),
            ]));
        }
        Ok(obj(&[
            ("status", "observed".into()),
            (
                "scope",
                obj(&[("studentId", input.get("studentId").clone())]),
            ),
            (
                "observation",
                obj(&[("status", "observed".into()), ("rows", Js::Arr(rows))]),
            ),
        ]))
    }
    pub fn source_requests(&self) -> usize {
        self.requests
    }
}
/// Original embedded row projection, including identity and duplicate-attempt checks.
pub fn project_page(raw: &Js, student: &Js, selector: &Js) -> Result<Js> {
    require(
        raw.get("selector") == selector
            && raw.get("operation").is("DtResultSearch")
            && raw.get("page") == &Js::Num(1.0),
        "native_metadata_selector_conflict",
    )?;
    let list = raw.get("rawRows");
    require(
        array(list) && list.array().len() <= 2000,
        "student_search_list_rejected",
    )?;
    let mut rows = vec![];
    // Membership only: output retains original row order and exact identities.
    let mut attempts = std::collections::BTreeSet::new();
    for r in list.array() {
        require(object(r), "invalid_capture_shape")?;
        if source_key(r.get("priNo"))? != *student.get("studentKey") {
            continue;
        }
        crate::native::literal_name(r.get("userName"))?;
        require(
            r.get("userName") == student.get("studentName"),
            "student_search_identity_conflict",
        )?;
        let paper = key(&source_key(r.get("pNo"))?)?;
        let attempt = key(&source_key(r.get("testingNo"))?)?;
        require(
            attempts.insert((paper.units().to_vec(), attempt.units().to_vec())),
            "duplicate_student_search_row",
        )?;
        let raw_id = r.get("_id");
        let id = if raw_id.string() {
            raw_id.clone()
        } else if matches!(raw_id, Js::Num(_)) {
            json(raw_id).into()
        } else {
            Js::Null
        };
        static ROW_ID: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let id = if id.string()
            && ROW_ID
                .get_or_init(|| edu_academy_core::js::regex("^[A-Za-z0-9_-]{1,64}$"))
                .is_match(&id.code_units())
        {
            id
        } else {
            Js::Null
        };
        let score = match r.get("score") {
            Js::Num(n) if n.is_finite() => Js::Num(*n),
            _ => Js::Null,
        };
        rows.push(obj(&[
            ("studentKey", student.get("studentKey").clone()),
            ("studentName", student.get("studentName").clone()),
            ("paperKey", paper),
            ("attemptKey", attempt),
            ("sourceRowId", id),
            ("sourceScore", score),
        ]));
    }
    Ok(obj(&[
        ("selector", selector.clone()),
        ("rows", Js::Arr(rows)),
        ("observedAt", instant(raw.get("observedAt"))?),
        ("operation", "DtResultSearch".into()),
        ("page", Js::Num(1.0)),
        ("coverage", COVERAGE.into()),
        ("fixedSourceFilter", obj(&[("sort_date", selector.clone())])),
    ]))
}
pub fn decode_code(code: &Js) -> Result<Js> {
    require(
        code.string()
            && test(
                "^#[3456789ABCDEFGHJKLMNPQRSTUVWXYZ][23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{0,10}$",
                code,
            ),
        "invalid_printed_paper_code",
    )?;
    let alphabet = text("23456789ABCDEFGHJKLMNPQRSTUVWXYZ");
    let mut value = 0u64;
    for c in &code.units()[1..] {
        value = value * 32
            + alphabet
                .iter()
                .position(|v| v == c)
                .expect("validated alphabet") as u64;
    }
    require(
        value > 0 && value <= 9_007_199_254_740_991,
        "unsafe_printed_paper_key",
    )?;
    Ok(value.to_string().into())
}

/// Canonical display conversion only; it grants no paper/student lookup authority.
pub fn encode_code(value: &Js) -> Result<Js> {
    let key = crate::data::string(value);
    if !value.string()
        || !(1..=16).contains(&key.len())
        || key.starts_with('0')
        || !key.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(Error::refusal("invalid_printed_paper_key"));
    }
    let mut number: u64 = key.parse().expect("validated decimal grammar");
    if number > 9_007_199_254_740_991 {
        return Err(Error::refusal("unsafe_printed_paper_key"));
    }
    let alphabet = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut digits = Vec::new();
    while number > 0 {
        digits.push(alphabet[(number % 32) as usize]);
        number /= 32;
    }
    digits.reverse();
    let digits = String::from_utf8(digits).expect("ASCII alphabet");
    Ok(format!("#{digits}").into())
}
