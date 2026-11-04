//! Original dailytest-header-route.ts composition and two supervised lifetimes.
use crate::{Error, Js, Result, audit, encode, obj};
use edu_academy_core::{
    instant::{Grammar, epoch_ms},
    js::{Fault, test, trim},
};
use std::time::Instant;

const COVERAGE: &str = "one_observed_student_search_page_not_complete_history";
const HISTORY_COVERAGE: &str =
    "all_parsed_embedded_rows_on_each_observed_response_not_complete_history";
fn invalid() -> Error {
    Error::new("tool_input_invalid")
}
fn object(v: &Js) -> Result<()> {
    if matches!(v, Js::Obj(_)) {
        Ok(())
    } else {
        Err(invalid())
    }
}
fn strict(v: &Js, keys: &[&str]) -> Result<()> {
    object(v)?;
    if v.props()
        .iter()
        .any(|(k, _)| !keys.iter().any(|s| k == &crate::text(s)))
    {
        return Err(invalid());
    }
    Ok(())
}
fn string(v: &Js) -> Result<Js> {
    if v.string() {
        Ok(v.clone())
    } else {
        Err(invalid())
    }
}
fn literal(v: &Js, s: &str) -> Result<Js> {
    if v.is(s) {
        Ok(v.clone())
    } else {
        Err(invalid())
    }
}
fn key(v: &Js) -> Result<Js> {
    if v.string() && test("^[1-9][0-9]{0,15}$", v) {
        Ok(v.clone())
    } else {
        Err(invalid())
    }
}
fn hash(v: &Js) -> Result<Js> {
    if v.string() && test("^[a-f0-9]{64}$", v) {
        Ok(v.clone())
    } else {
        Err(invalid())
    }
}
fn instant(v: &Js) -> Result<Js> {
    if epoch_ms(v, Grammar::DailyRecord).is_some() {
        Ok(v.clone())
    } else {
        Err(invalid())
    }
}
fn schema(name: &str, v: &Js) -> Result<Js> {
    edu_academy_core::schema::validate(name, v).map_err(|_| invalid())
}
fn array(v: &Js, max: usize, mut f: impl FnMut(&Js) -> Result<Js>) -> Result<Js> {
    if !matches!(v, Js::Arr(_)) || v.array().len() > max {
        return Err(invalid());
    }
    Ok(Js::Arr(
        v.array().iter().map(&mut f).collect::<Result<Vec<_>>>()?,
    ))
}
fn selector(v: &Js) -> Result<Js> {
    if ["11", "12", "13", "14", "15", "16"].iter().any(|s| v.is(s)) {
        Ok(v.clone())
    } else {
        Err(invalid())
    }
}
fn integer(v: &Js, low: usize, high: usize) -> Result<usize> {
    match v {
        Js::Num(n)
            if n.is_finite() && n.fract() == 0.0 && *n >= low as f64 && *n <= high as f64 =>
        {
            Ok(*n as usize)
        }
        _ => Err(invalid()),
    }
}
/// The public schema has defaults, but does not coerce caller strings or dates.
pub fn prepare(v: &Js) -> Result<Js> {
    strict(
        v,
        &[
            "displayName",
            "lessonDate",
            "purpose",
            "currentRequestRef",
            "periods",
            "maxHeaders",
            "headerOffset",
            "expectedSelectionSha256",
        ],
    )?;
    string(v.get("displayName"))?;
    let name = trim(v.get("displayName").units());
    if name.is_empty() || name.len() > 200 {
        return Err(invalid());
    }
    let periods = array(v.get("periods"), 6, selector)?;
    if periods.array().is_empty()
        || periods
            .array()
            .iter()
            .enumerate()
            .any(|(i, p)| periods.array()[..i].contains(p))
    {
        return Err(invalid());
    }
    let limit = if v.has("maxHeaders") {
        integer(v.get("maxHeaders"), 1, 32)?
    } else {
        32
    };
    let offset = if v.has("headerOffset") {
        integer(v.get("headerOffset"), 0, 12000)?
    } else {
        0
    };
    let mut out = obj(&[
        ("displayName", Js::Str(name.to_vec())),
        (
            "lessonDate",
            schema("dailyRecordDateSchema", v.get("lessonDate"))?,
        ),
        ("purpose", schema("dailyRecordIdSchema", v.get("purpose"))?),
        (
            "currentRequestRef",
            schema("dailyRecordIdSchema", v.get("currentRequestRef"))?,
        ),
        ("periods", periods),
        ("maxHeaders", limit.into()),
        ("headerOffset", offset.into()),
    ]);
    if v.has("expectedSelectionSha256") {
        out.set(
            "expectedSelectionSha256",
            hash(v.get("expectedSelectionSha256"))?,
        );
    }
    if offset > 0 && !out.has("expectedSelectionSha256") {
        return Err(invalid());
    }
    Ok(out)
}
fn row(v: &Js) -> Result<Js> {
    object(v)?;
    let source_id = if v.get("sourceRowId") == &Js::Null {
        Js::Null
    } else {
        string(v.get("sourceRowId"))?
    };
    let score = match v.get("sourceScore") {
        Js::Null => Js::Null,
        Js::Num(n) if n.is_finite() => Js::Num(*n),
        _ => return Err(invalid()),
    };
    Ok(obj(&[
        ("studentKey", key(v.get("studentKey"))?),
        ("studentName", string(v.get("studentName"))?),
        ("paperKey", key(v.get("paperKey"))?),
        ("attemptKey", key(v.get("attemptKey"))?),
        ("sourceRowId", source_id),
        ("sourceScore", score),
    ]))
}
fn rows(v: &Js) -> Result<Js> {
    array(v, 2000, row)
}
fn scope(v: &Js) -> Result<Js> {
    object(v)?;
    Ok(obj(&[
        (
            "sourceNamespace",
            literal(v.get("sourceNamespace"), "academy.lms.metadata")?,
        ),
        ("studentId", key(v.get("studentId"))?),
        (
            "lessonDate",
            schema("dailyRecordDateSchema", v.get("lessonDate"))?,
        ),
    ]))
}
fn page(v: &Js) -> Result<Js> {
    object(v)?;
    if v.get("page") != &Js::Num(1.0) {
        return Err(invalid());
    }
    object(v.get("fixedSourceFilter"))?;
    Ok(obj(&[
        ("selector", selector(v.get("selector"))?),
        ("rows", rows(v.get("rows"))?),
        ("observedAt", instant(v.get("observedAt"))?),
        ("operation", literal(v.get("operation"), "DtResultSearch")?),
        ("page", Js::Num(1.0)),
        ("coverage", literal(v.get("coverage"), COVERAGE)?),
        (
            "fixedSourceFilter",
            obj(&[(
                "sort_date",
                selector(v.get("fixedSourceFilter").get("sort_date"))?,
            )]),
        ),
    ]))
}
fn metadata(v: &Js) -> Result<Js> {
    object(v)?;
    literal(v.get("status"), "observed")?;
    let results = array(v.get("results"), 1, |r| {
        object(r)?;
        object(r.get("query"))?;
        let q = obj(&[
            (
                "kind",
                literal(r.get("query").get("kind"), "daily_history")?,
            ),
            (
                "periods",
                array(r.get("query").get("periods"), usize::MAX, selector)?,
            ),
        ]);
        let o = r.get("observation");
        object(o)?;
        let c = o.get("currentFilterQualification");
        object(c)?;
        let current = obj(&[
            ("selector", literal(c.get("selector"), "1")?),
            ("observedAt", instant(c.get("observedAt"))?),
            ("rows", rows(c.get("rows"))?),
        ]);
        Ok(obj(&[
            ("query", q),
            (
                "observation",
                obj(&[
                    ("status", literal(o.get("status"), "observed")?),
                    ("studentKey", key(o.get("studentKey"))?),
                    ("studentName", string(o.get("studentName"))?),
                    ("currentFilterQualification", current),
                    (
                        "observations",
                        array(o.get("observations"), usize::MAX, page)?,
                    ),
                    ("coverage", literal(o.get("coverage"), HISTORY_COVERAGE)?),
                ]),
            ),
        ]))
    })?;
    if results.array().len() != 1 {
        return Err(invalid());
    }
    Ok(obj(&[
        ("status", "observed".into()),
        ("scope", scope(v.get("scope"))?),
        ("results", results),
    ]))
}
fn header_row(v: &Js) -> Result<Js> {
    object(v)?;
    let h = v.get("contentHeader");
    object(h)?;
    let fields = array(h.get("metadataRows"), 128, |r| {
        object(r)?;
        let label = string(r.get("label"))?;
        let value = string(r.get("value"))?;
        if label.units().len() > 200 || value.units().len() > 2000 {
            return Err(invalid());
        }
        Ok(obj(&[("label", label), ("value", value)]))
    })?;
    Ok(obj(&[
        ("status", literal(v.get("status"), "observed")?),
        ("paperCode", string(v.get("paperCode"))?),
        ("requestedPaperKey", key(v.get("requestedPaperKey"))?),
        ("observedAt", instant(v.get("observedAt"))?),
        (
            "contentHeader",
            obj(&[
                ("sourceSha256", hash(h.get("sourceSha256"))?),
                ("metadataRows", fields),
            ]),
        ),
    ]))
}
/// Conversion only. Never establishes lookup, admission or identity.
pub fn encode_paper_code(v: &Js) -> Result<Js> {
    key(v).map_err(|_| Error::new("invalid_printed_paper_key"))?;
    let mut n = v
        .code_units()
        .parse::<u64>()
        .map_err(|_| Error::new("unsafe_printed_paper_key"))?;
    if n > 9_007_199_254_740_991 {
        return Err(Error::new("unsafe_printed_paper_key"));
    }
    let alphabet = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut bytes = vec![];
    while n > 0 {
        bytes.push(alphabet[(n % 32) as usize]);
        n /= 32;
    }
    bytes.push(b'#');
    bytes.reverse();
    Ok(String::from_utf8(bytes).expect("fixed alphabet").into())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Metadata,
    Paper,
}
impl Source {
    fn name(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Paper => "paper",
        }
    }
}
/// Run the original pure consumer. Invoke must return only after its child closes.
pub fn read(value: &Js, mut invoke: impl FnMut(Source, &Js) -> Result<Js>) -> Result<Js> {
    let input = prepare(value)?;
    let raw = invoke(
        Source::Metadata,
        &obj(&[
            ("displayName", input.get("displayName").clone()),
            ("lessonDate", input.get("lessonDate").clone()),
            ("purpose", input.get("purpose").clone()),
            ("currentRequestRef", input.get("currentRequestRef").clone()),
            (
                "queries",
                Js::Arr(vec![obj(&[
                    ("kind", "daily_history".into()),
                    ("periods", input.get("periods").clone()),
                ])]),
            ),
        ]),
    )?;
    let result = metadata(&raw)?;
    let scope = result.get("scope");
    let h = result.get("results").array()[0].clone();
    let o = h.get("observation");
    let pages = o.get("observations").array();
    let current = o.get("currentFilterQualification");
    let conflict = || Error::new("dailytest_header_scope_conflict");
    if scope.get("lessonDate") != input.get("lessonDate")
        || o.get("studentKey") != scope.get("studentId")
        || o.get("studentName") != input.get("displayName")
        || h.get("query").get("periods") != input.get("periods")
        || pages.len() != input.get("periods").array().len()
    {
        return Err(conflict());
    }
    for (i, p) in pages.iter().enumerate() {
        if p.get("selector") != &input.get("periods").array()[i]
            || p.get("fixedSourceFilter").get("sort_date") != p.get("selector")
        {
            return Err(conflict());
        }
    }
    for p in std::iter::once(current).chain(pages.iter()) {
        if p.get("rows").array().iter().any(|r| {
            r.get("studentKey") != scope.get("studentId")
                || r.get("studentName") != input.get("displayName")
        }) {
            return Err(conflict());
        }
    }
    let selected_pages = Js::Arr(
        std::iter::once(current)
            .chain(pages.iter())
            .map(|p| {
                obj(&[
                    ("selector", p.get("selector").clone()),
                    ("rows", p.get("rows").clone()),
                ])
            })
            .collect(),
    );
    let sha = crate::digest(&encode(&obj(&[
        ("scope", scope.clone()),
        ("pages", selected_pages),
    ]))?);
    if input.has("expectedSelectionSha256") && !input.get("expectedSelectionSha256").is(&sha) {
        return Err(Error::new("dailytest_header_selection_changed"));
    }
    let mut unique = vec![];
    for r in std::iter::once(current)
        .chain(pages.iter())
        .flat_map(|p| p.get("rows").array())
    {
        if !unique.contains(r.get("paperKey")) {
            unique.push(r.get("paperKey").clone());
        }
    }
    let offset = integer(input.get("headerOffset"), 0, 12000)?;
    let limit = integer(input.get("maxHeaders"), 1, 32)?;
    if offset > unique.len() {
        return Err(Error::new("dailytest_header_offset_out_of_range"));
    }
    let selected = &unique[offset..unique.len().min(offset + limit)];
    let codes = selected
        .iter()
        .map(encode_paper_code)
        .collect::<Result<Vec<_>>>()?;
    let mut mapped: Vec<(Js, Js)> = vec![];
    if !codes.is_empty() {
        let h = invoke(
            Source::Paper,
            &obj(&[
                ("studentId", scope.get("studentId").clone()),
                ("paperCodes", Js::Arr(codes.clone())),
            ]),
        )?;
        object(&h)?;
        literal(h.get("status"), "observed")?;
        object(h.get("scope"))?;
        let student = key(h.get("scope").get("studentId"))?;
        object(h.get("observation"))?;
        literal(h.get("observation").get("status"), "observed")?;
        let entries = array(h.get("observation").get("rows"), 32, header_row)?;
        if &student != scope.get("studentId") || entries.array().len() != codes.len() {
            return Err(conflict());
        }
        for e in entries.array() {
            let Some(i) = selected
                .iter()
                .position(|k| k == e.get("requestedPaperKey"))
            else {
                return Err(Error::new("dailytest_header_identity_conflict"));
            };
            if e.get("paperCode") != &codes[i]
                || mapped.iter().any(|(k, _)| k == e.get("requestedPaperKey"))
            {
                return Err(Error::new("dailytest_header_identity_conflict"));
            }
            let get = |label: &str| -> Result<Js> {
                let matches = e
                    .get("contentHeader")
                    .get("metadataRows")
                    .array()
                    .iter()
                    .filter(|r| r.get("label").is(label))
                    .collect::<Vec<_>>();
                if matches.len() > 1 {
                    return Err(Error::new("dailytest_header_duplicate_field"));
                }
                Ok(matches
                    .first()
                    .map(|r| r.get("value"))
                    .filter(|v| v.truthy())
                    .cloned()
                    .unwrap_or(Js::Null))
            };
            let header = obj(&[
                ("title", get("시험지 제목")?),
                ("category", get("평가구분")?),
                ("questionCount", get("총 문제수")?),
                (
                    "sourceSha256",
                    e.get("contentHeader").get("sourceSha256").clone(),
                ),
                ("observedAt", e.get("observedAt").clone()),
            ]);
            mapped.push((e.get("requestedPaperKey").clone(), header));
        }
    }
    let periods = Js::Arr(
        std::iter::once(current)
            .chain(pages.iter())
            .map(|p| {
                obj(&[
                    ("selector", p.get("selector").clone()),
                    ("observedAt", p.get("observedAt").clone()),
                    ("coverage", COVERAGE.into()),
                    (
                        "rows",
                        Js::Arr(
                            p.get("rows")
                                .array()
                                .iter()
                                .map(|r| {
                                    let header =
                                        mapped.iter().find(|(k, _)| k == r.get("paperKey"));
                                    obj(&[
                                        ("paperKey", r.get("paperKey").clone()),
                                        ("attemptKey", r.get("attemptKey").clone()),
                                        ("sourceScore", r.get("sourceScore").clone()),
                                        ("sourceRowId", r.get("sourceRowId").clone()),
                                        ("header", header.map_or(Js::Null, |(_, h)| h.clone())),
                                        (
                                            "headerStatus",
                                            if header.is_some() {
                                                "observed"
                                            } else {
                                                "not_requested_limit"
                                            }
                                            .into(),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect(),
    );
    let next = offset + selected.len();
    let out = obj(&[
        (
            "status",
            if unique.len() > selected.len() {
                "partial"
            } else {
                "observed"
            }
            .into(),
        ),
        ("displayName", input.get("displayName").clone()),
        ("lessonDate", input.get("lessonDate").clone()),
        ("purpose", input.get("purpose").clone()),
        ("currentRequestRef", input.get("currentRequestRef").clone()),
        ("scope", scope.clone()),
        ("periods", periods),
        ("sourcePaperCount", unique.len().into()),
        ("headersRead", selected.len().into()),
        ("headerLimit", limit.into()),
        ("headerOffset", offset.into()),
        ("selectionSha256", sha.into()),
        (
            "nextHeaderOffset",
            if next < unique.len() {
                next.into()
            } else {
                Js::Null
            },
        ),
        (
            "calendarBounds",
            "source_relative_labels_only_not_exact_calendar_bounds".into(),
        ),
        (
            "coverage",
            "one_page_per_selector_not_complete_history".into(),
        ),
        ("bookEdition", "unqualified".into()),
        ("attemptDate", "unqualified".into()),
        ("teacherCompletion", "unqualified".into()),
        ("level", "source_title_only".into()),
        ("academyWrites", Js::Num(0.0)),
        ("parentSends", Js::Num(0.0)),
        ("nativePrintInvoked", false.into()),
        ("automaticRetryAllowed", false.into()),
        ("sourceContentIsUntrustedData", true.into()),
        ("execution", "read_only".into()),
    ]);
    if encode(&audit::tool_result(out.clone())?)?.len() > 512 * 1024 {
        return Err(Error::new("dailytest_header_result_too_large"));
    }
    Ok(out)
}
/// Original CompositeSourceUnavailable projection: only measured bounded counts.
pub fn source_failure(source: Source, response: &Js) -> Option<(Js, Js)> {
    let data = response.get("structuredContent");
    if !(source == Source::Metadata && data.get("status").is("unavailable")
        || source == Source::Paper && data.get("status").is("blocked"))
    {
        return None;
    }
    let mut count = data.get("sourceRequests");
    if count.absent() || count == &Js::Null {
        count = data.get("effects").get("sourceRequests");
    }
    if count.absent() || count == &Js::Null {
        count = data.get("metrics").get("sourceRequests");
    }
    let count = match count {
        Js::Num(n) if n.is_finite() && n.fract() == 0.0 && (0.0..=100000.0).contains(n) => {
            count.clone()
        }
        _ => Js::Null,
    };
    let failure = if source == Source::Metadata {
        let reason = data.get("reasonCode");
        edu_academy_core::workflow::read_failure::public_read_failure(
            &Fault::error(if reason.string() {
                reason.code_units()
            } else {
                String::new()
            }),
            "student_metadata_unqualified_no_automatic_retry",
        )
    } else {
        let (category, code, locator) = if data.get("code").is("read_scope_not_admitted") {
            (
                "policy_denial",
                "read_scope_not_admitted",
                "academy-mcp:upstream-read-admission",
            )
        } else if data.get("code").is("source_failure_latched_no_retry") {
            (
                "upstream_unavailable",
                "source_failure_latched_no_retry",
                "academy-mcp:paper-source-latch",
            )
        } else {
            (
                "upstream_unavailable",
                "source_headers_read_failed",
                "academy-mcp:paper-source-read",
            )
        };
        obj(&[
            ("category", category.into()),
            ("code", code.into()),
            ("evidenceLocator", locator.into()),
            ("automaticRetryAllowed", false.into()),
        ])
    };
    Some((
        obj(&[
            ("failedSource", source.name().into()),
            ("reasonCode", failure.get("code").clone()),
            ("sourceRequests", count),
        ]),
        failure,
    ))
}
/// No replacement scheduler: both finite lifetimes use the accepted supervisor.
#[allow(clippy::too_many_arguments)]
pub fn call(
    graph: &crate::release::ReleaseGraph,
    gate: &mut crate::release::ConnectionGate,
    args: &Js,
    request_id: &Js,
    control: &mut dyn crate::process::Control,
    limits: crate::process::Limits,
    started: Instant,
    occurrence: &str,
    failure_metadata: &mut Option<Js>,
    public_failure: &mut Option<Js>,
) -> Result<Js> {
    prepare(args)?;
    let entries = graph.registry.entries();
    let metadata = entries.iter().find(|e| {
        e.tool.get("name").is("academy_student_metadata_get") && e.endpoint == "dailyrecord"
    });
    let paper = entries
        .iter()
        .find(|e| e.upstream_name == "academy_paper_headers" && e.endpoint == "paper1")
        .or_else(|| {
            entries
                .iter()
                .find(|e| e.upstream_name == "academy_paper_headers" && e.endpoint == "paper2")
        });
    let (Some(metadata), Some(paper)) = (metadata, paper) else {
        return Err(Error::new("dailytest_header_upstream_not_configured"));
    };
    let mut context =
        crate::process::Context::new(graph, gate, control, request_id, started + limits.work);
    let data = read(args, |source, input| {
        context.verify()?;
        context.check()?;
        let target = if source == Source::Metadata {
            metadata
        } else {
            crate::routing::select(&graph.routing, &graph.registry, paper, input)?
        };
        let plan = crate::endpoint::plan(graph, &target.endpoint)?;
        let supervised =
            crate::process::supervise(&plan, &mut context, limits, |child, context| {
                let mut upstream = crate::upstream::Client::default();
                if let Err(error) = upstream.connect(child, context, &target.endpoint) {
                    *failure_metadata = upstream.failure_metadata;
                    return Err(error);
                }
                let response =
                    upstream.call(child, context, &target.upstream_name, input.clone(), || {})?;
                if let Some((metadata, failure)) = source_failure(source, &response) {
                    *failure_metadata = Some(metadata);
                    *public_failure = Some(failure);
                    return Err(Error::new("upstream_tool_error"));
                }
                if response.get("isError").truthy() || !response.get("structuredContent").truthy() {
                    if let crate::dailyrecord_read::InvokeFailure::Tool {
                        failure,
                        occurrence,
                    } = crate::dailyrecord_read::tool_failure(&response)
                    {
                        *public_failure = Some(failure);
                        *failure_metadata = Some(obj(&[("upstreamOccurrence", occurrence)]));
                    }
                    return Err(Error::new("upstream_tool_error"));
                }
                Ok(response.get("structuredContent").clone())
            });
        graph.receipt(
            &format!("{occurrence}-{}-cleanup.json", source.name()),
            &supervised.cleanup.evidence(),
        )?;
        let data = supervised.result?;
        // Full cleanup/readback precede any subsequent source's launch.
        context.verify()?;
        context.check()?;
        Ok(data)
    })?;
    context.verify()?;
    context.check()?;
    audit::tool_result(data)
}

/// This slice deliberately implements only the history query, not other metadata families.
pub(crate) fn driver_input(tool: &str, v: &Js) -> Result<Js> {
    if tool == "academy_runtime_read_admission" {
        strict(v, &[])?;
        return Ok(v.clone());
    }
    if tool == "academy_student_metadata_get" {
        strict(
            v,
            &[
                "displayName",
                "lessonDate",
                "purpose",
                "currentRequestRef",
                "queries",
            ],
        )?;
        if !matches!(v.get("queries"), Js::Arr(_)) || v.get("queries").array().len() != 1 {
            return Err(invalid());
        }
        let q = &v.get("queries").array()[0];
        strict(q, &["kind", "periods"])?;
        literal(q.get("kind"), "daily_history")?;
        let mut input = obj(
            &["displayName", "lessonDate", "purpose", "currentRequestRef"]
                .map(|k| (k, v.get(k).clone())),
        );
        input.set("periods", q.get("periods").clone());
        let prepared = prepare(&input)?;
        return Ok(obj(&[
            ("displayName", prepared.get("displayName").clone()),
            ("lessonDate", prepared.get("lessonDate").clone()),
            ("purpose", prepared.get("purpose").clone()),
            (
                "currentRequestRef",
                prepared.get("currentRequestRef").clone(),
            ),
            (
                "queries",
                Js::Arr(vec![obj(&[
                    ("kind", "daily_history".into()),
                    ("periods", prepared.get("periods").clone()),
                ])]),
            ),
        ]));
    }
    if tool == "academy_paper_headers" {
        strict(v, &["studentId", "paperCodes"])?;
        key(v.get("studentId"))?;
        let codes = array(v.get("paperCodes"), 32, |c| {
            edu_academy_sources::history::decode_code(c).map_err(|_| invalid())?;
            Ok(c.clone())
        })?;
        if codes.array().is_empty() {
            return Err(invalid());
        }
        return Ok(obj(&[
            ("studentId", v.get("studentId").clone()),
            ("paperCodes", codes),
        ]));
    }
    Err(Error::new("unknown_tool"))
}
