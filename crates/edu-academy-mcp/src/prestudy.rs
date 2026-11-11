//! Prestudy MCP adapters over the E11a source layer: `academy_prestudy_get`
//! and `academy_prestudy_report_get`, as session loans (decision Q3).
//!
//! The TypeScript service (`dailyrecord.ts:393-433`) and backend
//! (`native-dailyrecord-backend.ts:209-242`) are the reference. Input is the
//! registry schema (checked by the protocol layer) plus the Zod rules a JSON
//! Schema cannot state: trimmed name length, unique section titles, the safe
//! text refinement, all counted in UTF-16 units. Every source read goes
//! through `PrestudySource` over `SessionPrestudyPort`, the port that binds
//! the teacher key; this adapter has no way to hand the source a port that
//! has no principal key.
use crate::{Error, Js, Result, obj, registry::Entry, release::ReleaseGraph, text};
use edu_academy_core::{
    js::{Fault, trim},
    workflow::read_failure,
};
use edu_academy_sources::{
    data::fault_from_error,
    http::TransportLimits,
    owner_admission::TransportScope,
    prestudy::{
        DEFAULT_FULL_MAX_BYTES, DEFAULT_SELECTED_MAX_BYTES, MAX_SOURCE_BYTES, PrestudyListRow,
        PrestudyReportObservation, PrestudyReportSection, PrestudySource, PrestudySummaryRow,
        SessionPrestudyPort, check_report_budget, select_report_view, utf8_json_len,
    },
    student_course::{COURSE_NS, ReadPort},
};

pub const TOOLS: [&str; 2] = ["academy_prestudy_get", "academy_prestudy_report_get"];

/// The `academy_*` names bind only on the `dailyrecord` endpoint.
pub fn is_prestudy_tool(endpoint: &str, name: &str) -> bool {
    endpoint == "dailyrecord" && TOOLS.contains(&name)
}

/// Fallback codes of the two TS services (`dailyrecord.ts:406,432`).
pub const GET_FALLBACK: &str = "prestudy_unqualified_no_automatic_retry";
pub const REPORT_FALLBACK: &str = "prestudy_report_unqualified_no_automatic_retry";

/// The independent dual-representation cap (`audit::METADATA_SOFT_BYTES`,
/// TS `toolResultBytes(...)>512*1024`).
const DUAL_CAP_BYTES: usize = 512 * 1024;

/// Response bound of the prestudy session (declared difference 5 of E11a).
/// The owner builds `NativeSession::installed(.., prestudy_transport_limits(),
/// ..)` with it, so the transport stops reading at the bound and a longer
/// body never reaches a parser. `NativeTransport` reads with
/// `.limit(response_bytes)` and the reader refuses a body of exactly
/// `response_bytes` bytes (`http.rs:3445`, observed: `native_response_read_failed`),
/// so the limit is the source bound plus one: a body of at most
/// `MAX_SOURCE_BYTES` bytes passes and one byte more is refused.
pub fn prestudy_transport_limits() -> TransportLimits {
    TransportLimits {
        response_bytes: MAX_SOURCE_BYTES + 1,
        ..TransportLimits::default()
    }
}

/// Exact configured identities, as the TS admission (`teacherId`,
/// `teacherLabel`, `students[{studentId, displayName}]`) plus the transport
/// owner and date the session scope needs.
#[derive(Clone)]
pub struct PrestudyAdmission {
    pub owner_id: String,
    pub teacher_label: String,
    pub teacher_key: String,
    pub college_key: String,
    pub date: String,
    /// `(student key, display name)`; at most 8 as in TS.
    pub students: Vec<(String, String)>,
}

/// Borrowed prestudy driver. Only a fictional owner graph qualifies, exactly
/// as the DailyRecord, student and paper loans; no native factory is provided.
pub struct PrestudyDriver<P: ReadPort> {
    port: P,
    admission: PrestudyAdmission,
    routing_admission: Js,
    requests: usize,
}

impl<P: ReadPort> PrestudyDriver<P> {
    /// The transport admission must carry exactly this owner, teacher and
    /// student set (as `Owner::validate_transport` for the other families).
    pub fn new(port: P, admission: PrestudyAdmission, routing_admission: Js) -> Result<Self> {
        let p = port.transport_admission();
        let valid = !admission.students.is_empty()
            && admission.students.len() <= 8
            && p.owner_id == admission.owner_id
            && p.teacher_id == admission.teacher_key
            && p.teacher_label == admission.teacher_label
            && p.college_id == admission.college_key
            && p.student_ids.len() == admission.students.len()
            && admission
                .students
                .iter()
                .all(|(key, _)| p.student_ids.contains(key));
        if !valid {
            return Err(Error::new("student_read_owner_transport_conflict"));
        }
        Ok(Self {
            port,
            admission,
            routing_admission,
            requests: 0,
        })
    }
    pub fn source_requests(&self) -> usize {
        self.requests
    }
    pub fn check_binding(&self, graph: &ReleaseGraph) -> Result<()> {
        if graph.owner.kind != "fictional" {
            return Err(Error::new("business_owner_binding_unqualified"));
        }
        let admission = graph.admission()?;
        if crate::encode(&admission)? != crate::encode(&self.routing_admission)?
            || !graph
                .routing
                .get("admissionSha256")
                .is(&crate::digest(&crate::encode(&self.routing_admission)?))
        {
            return Err(Error::new("release_upstream_admission_mismatch"));
        }
        Ok(())
    }
    /// One call at the wall clock.
    pub fn call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js> {
        self.call_at(tool, input, &crate::audit::wall_time())
    }
    /// One call with an explicit observation instant (tests pin bytes with it).
    pub fn call_at(
        &mut self,
        tool: &str,
        input: &Js,
        observed_at: &str,
    ) -> edu_academy_core::js::Result<Js> {
        match tool {
            "academy_prestudy_get" => self.get(input, observed_at),
            "academy_prestudy_report_get" => self.report(input, observed_at),
            _ => Err(Fault::error("unknown_tool")),
        }
    }

    fn scope(&self, student_key: &str) -> TransportScope {
        TransportScope {
            owner_id: self.admission.owner_id.clone(),
            source_namespace: COURSE_NS.into(),
            teacher_id: self.admission.teacher_key.clone(),
            student_id: student_key.into(),
            date: self.admission.date.clone(),
        }
    }
    /// Admission (`native-dailyrecord-backend.ts:210-211`): the exact name of
    /// an admitted student, else refuse before any read. A name that is not
    /// well-formed UTF-16 can match no admitted name.
    fn admitted(&self, display_name: &[u16]) -> std::result::Result<(String, String), Fault> {
        String::from_utf16(display_name)
            .ok()
            .and_then(|name| {
                self.admission
                    .students
                    .iter()
                    .find(|(_, n)| n == &name)
                    .cloned()
            })
            .ok_or_else(|| Fault::error("prestudy_student_not_admitted"))
    }

    fn get(&mut self, input: &Js, observed_at: &str) -> edu_academy_core::js::Result<Js> {
        let input = parse_get(input)?;
        let base = base_envelope(&input.common, None);
        let outcome = self
            .read_get(&input.common, observed_at)
            .and_then(|observed| {
                // `{...base, status:'observed', ...observation}` and the dual cap
                // (`dailyrecord.ts:401-405`).
                let mut out = base.clone();
                out.set("status", "observed".into());
                for (key, value) in observed.props() {
                    out.set(&String::from_utf16_lossy(key), value.clone());
                }
                dual_cap(&out, "prestudy_result_too_large")?;
                Ok(out)
            });
        Ok(match outcome {
            Ok(result) => result,
            Err(fault) => failure_result(base, &fault, GET_FALLBACK),
        })
    }
    fn read_get(&mut self, common: &Common, observed_at: &str) -> std::result::Result<Js, Fault> {
        let (student_key, student_name) = self.admitted(&common.name)?;
        let scope = self.scope(&student_key);
        let teacher_label = self.admission.teacher_label.clone();
        let teacher_key = self.admission.teacher_key.clone();
        let (lists, requests) = {
            let port = SessionPrestudyPort::new(&mut self.port, scope);
            let mut source = PrestudySource::new(port, teacher_label.clone(), teacher_key);
            let lists = source.read_lists(&teacher_label, &student_name, observed_at);
            (lists, source.requests())
        };
        self.requests += requests;
        let lists = lists.map_err(source_fault)?;
        // Backend shapes (`native-dailyrecord-backend.ts:213-218`).
        let waiting = list_rows(&lists.waiting)?;
        let completed = list_rows(&lists.completed)?;
        let summary = summary_rows(&lists.summary)?;
        // Service cross-check (`dailyrecord.ts:400`).
        if student_name.encode_utf16().ne(common.name.iter().copied())
            || lists.teacher_judgment.is_some()
        {
            return Err(Fault::error("prestudy_scope_conflict"));
        }
        Ok(obj(&[
            (
                "student",
                obj(&[
                    ("studentId", student_key.as_str().into()),
                    ("displayName", student_name.as_str().into()),
                ]),
            ),
            ("waiting", waiting),
            ("completed", completed),
            ("summary", summary),
            ("teacherJudgment", Js::Null),
            ("observedAt", lists.observed_at.as_str().into()),
            ("coverage", lists.coverage.as_str().into()),
        ]))
    }

    fn report(&mut self, input: &Js, observed_at: &str) -> edu_academy_core::js::Result<Js> {
        let input = parse_report(input)?;
        let base = base_envelope(&input.common, Some(&input.prestudy_key));
        let outcome = self.read_report(&input, observed_at);
        Ok(match outcome {
            Ok(result) => result,
            Err(fault) => failure_result(base, &fault, REPORT_FALLBACK),
        })
    }
    /// The whole result in the TS key order: `base`, then the observation
    /// keys, then `status`, the selected `sections` (same position) and `view`
    /// (`dailyrecord.ts:421-425` spreads `observation` after `base`).
    fn read_report(
        &mut self,
        input: &ReportInput,
        observed_at: &str,
    ) -> std::result::Result<Js, Fault> {
        let (student_key, student_name) = self.admitted(&input.common.name)?;
        let scope = self.scope(&student_key);
        let teacher_label = self.admission.teacher_label.clone();
        let teacher_key = self.admission.teacher_key.clone();
        let (observation, requests) = {
            let port = SessionPrestudyPort::new(&mut self.port, scope);
            let mut source = PrestudySource::new(port, teacher_label.clone(), teacher_key);
            let observation = source.read_report(
                &teacher_label,
                &student_name,
                &student_key,
                &input.prestudy_key,
                observed_at,
            );
            (observation, source.requests())
        };
        self.requests += requests;
        let observation = observation.map_err(source_fault)?;
        // Service re-binding (`dailyrecord.ts:416-417`): a second, independent
        // identity gate after the source and backend gates.
        rebind(
            &observation,
            &student_name,
            &input.common.name,
            &input.prestudy_key,
        )?;
        let selected = input.section_titles.is_some();
        // Titles with a lone surrogate equal no source title in JavaScript;
        // a sentinel no HTML text can carry keeps that in `select_report_view`.
        let requested: Option<Vec<String>> = input.section_titles.as_ref().map(|titles| {
            titles
                .iter()
                .enumerate()
                .map(|(i, units)| {
                    String::from_utf16(units).unwrap_or_else(|_| format!("\u{0}lone-surrogate-{i}"))
                })
                .collect()
        });
        let report = edu_academy_sources::prestudy::PrestudyReport {
            sections: observation.sections.clone(),
            scores: observation.scores.clone(),
        };
        let (sections, view) =
            select_report_view(&report, requested.as_deref()).map_err(source_fault)?;
        let missing_js: Vec<Js> = match &input.section_titles {
            Some(titles) => titles
                .iter()
                .filter(|units| {
                    !observation
                        .sections
                        .iter()
                        .any(|s| s.title.encode_utf16().eq(units.iter().copied()))
                })
                .map(|units| Js::Str(units.clone()))
                .collect(),
            None => vec![],
        };
        debug_assert_eq!(missing_js.len(), view.missing_titles.len());
        let strings =
            |values: &[String]| Js::Arr(values.iter().map(|s| s.as_str().into()).collect());
        let view_js = obj(&[
            ("mode", if selected { "selected" } else { "full" }.into()),
            (
                "requestedSectionTitles",
                match &input.section_titles {
                    Some(titles) => Js::Arr(titles.iter().map(|u| Js::Str(u.clone())).collect()),
                    None => Js::Null,
                },
            ),
            ("availableSectionTitles", strings(&view.available_titles)),
            ("returnedSectionTitles", strings(&view.returned_titles)),
            ("missingSectionTitles", Js::Arr(missing_js.clone())),
            ("completeRequestedView", view.complete_requested_view.into()),
            ("completeSourceReport", view.complete_source_report.into()),
        ]);
        let mut out = obj(&[
            (
                "student",
                obj(&[
                    ("studentId", student_key.as_str().into()),
                    ("displayName", student_name.as_str().into()),
                ]),
            ),
            ("prestudyKey", observation.prestudy_key.as_str().into()),
            ("sourceRecord", source_record_js(&observation)),
            ("sections", sections_js(&observation.sections)),
            ("scores", scores_js(&observation.scores)),
            (
                "encodingResult",
                observation
                    .encoding_result
                    .as_deref()
                    .map_or(Js::Null, Js::from),
            ),
            ("observedAt", observation.observed_at.as_str().into()),
            ("coverage", observation.coverage.as_str().into()),
        ]);
        out.set(
            "status",
            if missing_js.is_empty() {
                "observed"
            } else {
                "partial"
            }
            .into(),
        );
        out.set("sections", sections_js(&sections));
        out.set("view", view_js);
        // The budget is the UTF-8 byte length of the one JSON payload the
        // caller receives (the whole result, base included), excluding the
        // MCP envelope; never clipped. The dual-representation cap is
        // independent (`dailyrecord.ts:428-431`).
        let mut whole = base_envelope(&input.common, Some(&input.prestudy_key));
        for (key, value) in out.props() {
            whole.set(&String::from_utf16_lossy(key), value.clone());
        }
        let payload = crate::stringify(&whole)
            .map_err(|_| Fault::error("mcp_result_serialization_failed"))?;
        let budget = input.max_output_bytes.unwrap_or(if selected {
            DEFAULT_SELECTED_MAX_BYTES
        } else {
            DEFAULT_FULL_MAX_BYTES
        });
        check_report_budget(utf8_json_len(&payload), budget).map_err(source_fault)?;
        dual_cap(&whole, "prestudy_report_result_too_large")?;
        Ok(whole)
    }
}

/// Service re-binding (`dailyrecord.ts:416-417`): the student name, the record
/// key and the `sourceRecord` key must all equal the request.
fn rebind(
    observation: &PrestudyReportObservation,
    student_name: &str,
    requested_name: &[u16],
    prestudy_key: &str,
) -> std::result::Result<(), Fault> {
    if observation.prestudy_key != prestudy_key
        || observation.source_record.prestudy_key != prestudy_key
        || student_name
            .encode_utf16()
            .ne(requested_name.iter().copied())
    {
        return Err(Fault::error("prestudy_report_scope_conflict"));
    }
    Ok(())
}

// ---------------------------------------------------------------- input ----

struct Common {
    /// The trimmed display name in UTF-16 units (Zod `.trim()` output).
    name: Vec<u16>,
    purpose: Js,
    current_request_ref: Js,
}
struct GetInput {
    common: Common,
}
struct ReportInput {
    common: Common,
    prestudy_key: String,
    section_titles: Option<Vec<Vec<u16>>>,
    max_output_bytes: Option<usize>,
}

fn invalid() -> Fault {
    Fault::error("tool_input_invalid")
}

/// `dailyRecordIdSchema` (`dailyrecord-context.ts:5,8`): 1 to 256 UTF-16
/// units, not blank under the JS trim, no control characters other than
/// TAB, LF and CR.
fn safe_text(value: &Js) -> std::result::Result<Js, Fault> {
    let units = value.units();
    if !value.string()
        || units.is_empty()
        || units.len() > 256
        || trim(units).is_empty()
        || units
            .iter()
            .any(|u| matches!(u, 0..=8 | 0xb | 0xc | 0xe..=0x1f | 0x7f))
    {
        return Err(invalid());
    }
    Ok(value.clone())
}
/// `^[1-9][0-9]{0,15}$`.
fn key_units(units: &[u16]) -> bool {
    (1..=16).contains(&units.len())
        && (0x31..=0x39).contains(&units[0])
        && units.iter().all(|u| (0x30..=0x39).contains(u))
}
fn exact_keys(input: &Js, allowed: &[&str]) -> std::result::Result<(), Fault> {
    if !matches!(input, Js::Obj(_))
        || input
            .props()
            .iter()
            .any(|(k, _)| !allowed.iter().any(|name| k == &text(name)))
    {
        return Err(invalid());
    }
    Ok(())
}
fn common(input: &Js) -> std::result::Result<Common, Fault> {
    let name = input.get("displayName");
    if !input.has("displayName") || !name.string() {
        return Err(invalid());
    }
    let trimmed = trim(name.units());
    if trimmed.is_empty() || trimmed.len() > 200 {
        return Err(invalid());
    }
    if !input.has("purpose") || !input.has("currentRequestRef") {
        return Err(invalid());
    }
    Ok(Common {
        name: trimmed.to_vec(),
        purpose: safe_text(input.get("purpose"))?,
        current_request_ref: safe_text(input.get("currentRequestRef"))?,
    })
}
fn parse_get(input: &Js) -> std::result::Result<GetInput, Fault> {
    exact_keys(input, &["displayName", "purpose", "currentRequestRef"])?;
    Ok(GetInput {
        common: common(input)?,
    })
}
fn parse_report(input: &Js) -> std::result::Result<ReportInput, Fault> {
    exact_keys(
        input,
        &[
            "displayName",
            "prestudyKey",
            "sectionTitles",
            "maxOutputBytes",
            "purpose",
            "currentRequestRef",
        ],
    )?;
    let common = common(input)?;
    let key = input.get("prestudyKey");
    if !input.has("prestudyKey") || !key.string() || !key_units(key.units()) {
        return Err(invalid());
    }
    let section_titles = if input.has("sectionTitles") {
        let titles = input.get("sectionTitles");
        let list = titles.array();
        if !matches!(titles, Js::Arr(_))
            || !(1..=16).contains(&list.len())
            || list
                .iter()
                .any(|t| !t.string() || t.units().is_empty() || t.units().len() > 500)
        {
            return Err(invalid());
        }
        // `unique_section_titles_required`: exact UTF-16 equality.
        let mut seen = std::collections::HashSet::new();
        if !list.iter().all(|t| seen.insert(t.units().to_vec())) {
            return Err(invalid());
        }
        Some(list.iter().map(|t| t.units().to_vec()).collect())
    } else {
        None
    };
    let max_output_bytes = if input.has("maxOutputBytes") {
        match input.get("maxOutputBytes") {
            Js::Num(n) if n.is_finite() && n.fract() == 0.0 && (1024.0..=524288.0).contains(n) => {
                Some(*n as usize)
            }
            _ => return Err(invalid()),
        }
    } else {
        None
    };
    Ok(ReportInput {
        common,
        prestudy_key: String::from_utf16_lossy(key.units()),
        section_titles,
        max_output_bytes,
    })
}

// ------------------------------------------------------------- envelope ----

/// `base` of `dailyrecord.ts:395,411`: the request echo, then the fixed flags.
fn base_envelope(common: &Common, prestudy_key: Option<&str>) -> Js {
    let mut base = obj(&[
        ("purpose", common.purpose.clone()),
        ("currentRequestRef", common.current_request_ref.clone()),
        ("displayName", Js::Str(common.name.clone())),
    ]);
    if let Some(key) = prestudy_key {
        base.set("prestudyKey", key.into());
    }
    base.set("academyWrites", Js::Num(0.0));
    base.set("parentSends", Js::Num(0.0));
    base.set("automaticRetryAllowed", false.into());
    base.set("sourceContentIsUntrustedData", true.into());
    base.set("execution", "read_only".into());
    if prestudy_key.is_some() {
        base.set(
            "studentContent",
            "teacher_only_do_not_store_or_relay".into(),
        );
    }
    base
}
fn dual_cap(value: &Js, code: &str) -> std::result::Result<(), Fault> {
    let bytes = crate::audit::tool_result(value.clone())
        .and_then(|v| crate::encode(&v))
        .map_err(|e| Fault::error(e.code))?;
    if bytes.len() > DUAL_CAP_BYTES {
        return Err(Fault::error(code));
    }
    Ok(())
}

/// The twelve report and detail codes of `read-failure.ts:63-75` with their
/// category and locator. The shared Rust catalogue
/// (`edu-academy-core/src/workflow/read_failure.rs`) does not list them, so
/// without this table each would collapse to the tool fallback; TS publishes
/// them as themselves. Every other code goes through the shared catalogue.
const REPORT_CODES: [(&str, &str, &str); 12] = [
    (
        "prestudy_report_source_port_unavailable",
        "endpoint_absence",
        "src/native-dailyrecord-backend.ts:sourcePort",
    ),
    (
        "prestudy_report_principal_rejected",
        "policy_denial",
        "source-owner:prestudy-detail",
    ),
    (
        "prestudy_detail_student_mismatch",
        "policy_denial",
        "source-owner:prestudy-detail",
    ),
    (
        "prestudy_detail_record_mismatch",
        "contract_drift",
        "source-owner:prestudy-detail",
    ),
    (
        "prestudy_detail_unqualified",
        "contract_drift",
        "source-owner:prestudy-detail",
    ),
    (
        "prestudy_report_scope_conflict",
        "contract_drift",
        "source-owner:prestudy-detail",
    ),
    (
        "prestudy_report_result_unqualified",
        "contract_drift",
        "source-owner:prestudy-detail",
    ),
    (
        "prestudy_report_unqualified",
        "contract_drift",
        "source-owner:prestudy-report",
    ),
    (
        "prestudy_report_source_rejected",
        "malformed_response",
        "source-owner:prestudy-report",
    ),
    (
        "prestudy_report_not_ready",
        "partial_coverage",
        "source-owner:prestudy-report",
    ),
    (
        "prestudy_report_result_too_large",
        "partial_coverage",
        "source-owner:prestudy-report",
    ),
    (
        "prestudy_report_section_ambiguous",
        "identity_ambiguity",
        "source-owner:prestudy-report",
    ),
];

/// The public failure projection (`publicReadFailure`): the shared catalogue,
/// with the twelve report codes of [`REPORT_CODES`] kept as themselves.
/// `prestudy_principal_rejected` is in neither TS nor Rust catalogue and
/// collapses to the fallback, as in TS.
fn public_failure_of(fault: &Fault, fallback: &str) -> Js {
    let mut failure = read_failure::public_read_failure(fault, fallback);
    if let Some((code, category, locator)) = REPORT_CODES
        .iter()
        .find(|(code, _, _)| *code == fault.message)
    {
        failure.set("category", (*category).into());
        failure.set("code", (*code).into());
        failure.set("evidenceLocator", (*locator).into());
    }
    failure
}

/// `{...base, status:'unavailable', reasonCode, failure}` through the finite
/// catalogue (`read-failure.ts:241`). No source text enters it.
fn failure_result(base: Js, fault: &Fault, fallback: &str) -> Js {
    let failure = public_failure_of(fault, fallback);
    let mut out = base;
    out.set("status", "unavailable".into());
    out.set("reasonCode", failure.get("code").clone());
    out.set("failure", failure);
    out
}
/// Map a source error. The Zod-shaped refusal is a `ZodError` as in TS.
fn source_fault(error: edu_academy_sources::data::Error) -> Fault {
    let mut fault = fault_from_error(error);
    if fault.message == "source_response_malformed" {
        fault.name = "ZodError".into();
    }
    fault
}
/// The public failure of an error code, for callers and tests that need the
/// mapping alone (`prestudy_principal_rejected` is not catalogued in TS and
/// collapses to the tool fallback).
pub fn public_failure(code: &str, fallback: &str) -> Js {
    public_failure_of(&Fault::error(code), fallback)
}

// ---------------------------------------------------------- projections ----

fn zod(message: bool) -> std::result::Result<(), Fault> {
    if message {
        Ok(())
    } else {
        let mut fault = Fault::error("source_response_malformed");
        fault.name = "ZodError".into();
        Err(fault)
    }
}
fn units(value: &str) -> usize {
    value.encode_utf16().count()
}
fn key_ok(value: &str) -> bool {
    key_units(&value.encode_utf16().collect::<Vec<_>>())
}
fn list_rows(rows: &[PrestudyListRow]) -> std::result::Result<Js, Fault> {
    zod(rows.len() <= 1000)?;
    let mut out = vec![];
    for row in rows {
        zod(units(&row.unit_label) <= 2000
            && units(&row.date_literal) <= 200
            && units(&row.status_literal) <= 2000
            && key_ok(&row.prestudy_key))?;
        out.push(obj(&[
            ("unitLabel", row.unit_label.as_str().into()),
            ("dateLiteral", row.date_literal.as_str().into()),
            ("statusLiteral", row.status_literal.as_str().into()),
            ("prestudyKey", row.prestudy_key.as_str().into()),
        ]));
    }
    Ok(Js::Arr(out))
}
fn summary_rows(rows: &[PrestudySummaryRow]) -> std::result::Result<Js, Fault> {
    zod(rows.len() <= 1000)?;
    let mut out = vec![];
    for row in rows {
        zod(units(&row.unit_label) <= 2000
            && units(&row.date_literal) <= 200
            && units(&row.ai_score_literal) <= 200
            && units(&row.rating_literal) <= 200)?;
        out.push(obj(&[
            ("unitLabel", row.unit_label.as_str().into()),
            ("dateLiteral", row.date_literal.as_str().into()),
            ("aiScoreLiteral", row.ai_score_literal.as_str().into()),
            ("ratingLiteral", row.rating_literal.as_str().into()),
            ("reportAvailable", row.report_available.into()),
            ("videoAvailable", row.video_available.into()),
        ]));
    }
    Ok(Js::Arr(out))
}
fn source_record_js(observation: &PrestudyReportObservation) -> Js {
    let record = &observation.source_record;
    obj(&[
        ("prestudyKey", record.prestudy_key.as_str().into()),
        (
            "unitLabel",
            record.unit_label.as_deref().map_or(Js::Null, Js::from),
        ),
        (
            "dateLiteral",
            record.date_literal.as_deref().map_or(Js::Null, Js::from),
        ),
    ])
}
fn sections_js(sections: &[PrestudyReportSection]) -> Js {
    Js::Arr(
        sections
            .iter()
            .map(|section| {
                obj(&[
                    ("title", section.title.as_str().into()),
                    (
                        "subtitle",
                        section.subtitle.as_deref().map_or(Js::Null, Js::from),
                    ),
                    (
                        "blocks",
                        Js::Arr(
                            section
                                .blocks
                                .iter()
                                .map(|block| {
                                    obj(&[
                                        (
                                            "heading",
                                            block.heading.as_deref().map_or(Js::Null, Js::from),
                                        ),
                                        ("text", block.text.as_str().into()),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect(),
    )
}
fn scores_js(scores: &[f64]) -> Js {
    Js::Arr(scores.iter().map(|n| Js::Num(*n)).collect())
}

// ----------------------------------------------------------------- loan ----

/// Protocol-facing loan, so `RpcSession` never owns a source port directly.
pub trait PrestudyCall {
    fn check_prestudy_binding(&self, graph: &ReleaseGraph) -> Result<()>;
    fn prestudy_call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js>;
    fn prestudy_source_requests(&self) -> usize;
}
impl<P: ReadPort> PrestudyCall for PrestudyDriver<P> {
    fn check_prestudy_binding(&self, graph: &ReleaseGraph) -> Result<()> {
        self.check_binding(graph)
    }
    fn prestudy_call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js> {
        self.call(tool, input)
    }
    fn prestudy_source_requests(&self) -> usize {
        self.source_requests()
    }
}

/// Exact endpoint/upstream binding for the two prestudy tools.
pub(crate) fn prestudy_entry<'a>(
    graph: &'a ReleaseGraph,
    endpoint: &str,
    name: &str,
) -> Result<&'a Entry> {
    if !is_prestudy_tool(endpoint, name) {
        return Err(Error::new("business_endpoint_mismatch"));
    }
    let mut entries = graph
        .registry
        .entries()
        .iter()
        .filter(|e| e.endpoint == endpoint && e.upstream_name == name);
    let first = entries
        .next()
        .ok_or_else(|| Error::new("business_tool_not_found"))?;
    if entries.next().is_some() {
        return Err(Error::new("business_tool_binding_ambiguous"));
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use edu_academy_sources::prestudy::{
        PrestudyReportBlock, PrestudyReportSection, PrestudySourceRecord,
    };

    fn observation(key: &str, record_key: &str) -> PrestudyReportObservation {
        PrestudyReportObservation {
            prestudy_key: key.into(),
            source_record: PrestudySourceRecord {
                prestudy_key: record_key.into(),
                unit_label: None,
                date_literal: None,
            },
            sections: vec![PrestudyReportSection {
                title: "t".into(),
                subtitle: None,
                blocks: vec![PrestudyReportBlock {
                    heading: None,
                    text: "x".into(),
                }],
            }],
            scores: vec![],
            encoding_result: None,
            observed_at: "2040-01-02T03:04:00.000Z".into(),
            coverage: "c".into(),
        }
    }

    // PRC:41-46 ("source identity mismatch fails even if the echoed requested
    // key and student match"): the service gate is independent of the source.
    #[test]
    fn rebind_refuses_a_mismatched_record_key_or_source_record_key_or_name() {
        let name: Vec<u16> = "Invented Student".encode_utf16().collect();
        rebind(
            &observation("12345", "12345"),
            "Invented Student",
            &name,
            "12345",
        )
        .unwrap();
        for (key, record, who) in [
            ("12346", "12345", "Invented Student"),
            ("12345", "12346", "Invented Student"),
            ("12345", "12345", "Invented Other"),
        ] {
            let fault = rebind(&observation(key, record), who, &name, "12345").unwrap_err();
            assert_eq!(fault.message, "prestudy_report_scope_conflict");
            let out = failure_result(obj(&[]), &fault, REPORT_FALLBACK);
            assert_eq!(
                out.get("reasonCode").code_units(),
                "prestudy_report_scope_conflict"
            );
            assert_eq!(
                out.get("failure").get("category").code_units(),
                "contract_drift"
            );
        }
    }

    #[test]
    fn the_twelve_codes_are_unique_and_none_is_the_fallback() {
        let mut seen = std::collections::BTreeSet::new();
        for (code, _, _) in REPORT_CODES {
            assert!(seen.insert(code), "{code}");
            assert!(code.starts_with("prestudy_report_") || code.starts_with("prestudy_detail_"));
            assert_ne!(code, REPORT_FALLBACK);
        }
        assert_eq!(seen.len(), 12);
    }
}
