//! Original dailyrecord-read.ts composition. No source, owner factory or write port.
use crate::{Error, Js, Result, obj};
use edu_academy_core::{
    instant::{Grammar, epoch_ms},
    js::{Fault, test, trim},
    workflow::{read_failure, review::FIELDS},
};

fn invalid() -> Error {
    Error::new("tool_input_invalid")
}
fn schema(name: &str, value: &Js) -> Result<Js> {
    edu_academy_core::schema::validate(name, value).map_err(|_| invalid())
}
fn object(value: &Js) -> Result<()> {
    if matches!(value, Js::Obj(_)) {
        Ok(())
    } else {
        Err(invalid())
    }
}
fn strict(value: &Js, keys: &[&str]) -> Result<()> {
    object(value)?;
    if value
        .props()
        .iter()
        .any(|(k, _)| !keys.iter().any(|s| k == &crate::text(s)))
    {
        return Err(invalid());
    }
    Ok(())
}
fn id(value: &Js) -> Result<Js> {
    schema("dailyRecordIdSchema", value)
}
fn choice(value: &Js, values: &[&str]) -> Result<Js> {
    if values.iter().any(|v| value.is(v)) {
        Ok(value.clone())
    } else {
        Err(invalid())
    }
}
fn string(value: &Js) -> Result<Js> {
    if value.string() {
        Ok(value.clone())
    } else {
        Err(invalid())
    }
}
fn array(value: &Js, f: impl Fn(&Js) -> Result<Js>) -> Result<Js> {
    if !matches!(value, Js::Arr(_)) {
        return Err(invalid());
    }
    Ok(Js::Arr(
        value.array().iter().map(f).collect::<Result<Vec<_>>>()?,
    ))
}

/// Use the already-selected IANA timezone database, including historical Seoul
/// offsets and daylight saving. Never infer a fixed offset or process timezone.
pub fn academy_business_date(now: i64) -> Result<String> {
    use chrono::Datelike;
    let instant = chrono::DateTime::from_timestamp_millis(now).ok_or_else(invalid)?;
    let local = instant.with_timezone(&chrono_tz::Asia::Seoul);
    if !(1..=9999).contains(&local.year()) {
        return Err(invalid());
    }
    Ok(format!(
        "{:04}-{:02}-{:02}",
        local.year(),
        local.month(),
        local.day()
    ))
}
/// Launch clock, never an observation timestamp or the local process timezone.
pub fn wall_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

pub struct Prepared {
    base: Js,
    input: Js,
    owner: Js,
    held: Option<Js>,
}
impl Prepared {
    pub fn held(&self) -> Option<&Js> {
        self.held.as_ref()
    }
    fn hold(&self, reason: Js) -> Js {
        let mut out = self.base.clone();
        out.set("status", "unavailable".into());
        out.set("sourceAvailability", "unavailable".into());
        out.set("reasonCode", reason);
        out
    }
}
/// All input and owner/range checks happen before any business call or spawn.
pub fn prepare(value: &Js, admission: &Js, now: i64) -> Result<Prepared> {
    strict(
        value,
        &["displayName", "purpose", "currentRequestRef", "lessonDate"],
    )?;
    if !value.get("displayName").string() {
        return Err(invalid());
    }
    let name = trim(value.get("displayName").units());
    if name.is_empty() || name.len() > 200 {
        return Err(invalid());
    }
    let date = if !value.has("lessonDate") || value.get("lessonDate").is("today") {
        academy_business_date(now)?.into()
    } else {
        schema("dailyRecordDateSchema", value.get("lessonDate"))?
    };
    let input = obj(&[
        ("displayName", Js::Str(name.to_vec())),
        ("purpose", id(value.get("purpose"))?),
        ("currentRequestRef", id(value.get("currentRequestRef"))?),
        ("lessonDate", date.clone()),
    ]);
    let base = obj(&[
        ("displayName", input.get("displayName").clone()),
        ("lessonDate", date),
        ("timeZone", "Asia/Seoul".into()),
        ("purpose", input.get("purpose").clone()),
        ("currentRequestRef", input.get("currentRequestRef").clone()),
        ("academyWrites", Js::Num(0.0)),
        ("parentSends", Js::Num(0.0)),
        ("automaticRetryAllowed", false.into()),
        ("execution", "read_only".into()),
        ("sourceContentIsUntrustedData", true.into()),
    ]);
    let mut prepared = Prepared {
        base,
        input,
        owner: admission.clone(),
        held: None,
    };
    let qualified = (|| -> Result<()> {
        strict(
            admission,
            &[
                "sourceNamespace",
                "teacherId",
                "allowedFrom",
                "allowedThrough",
                "studentScope",
            ],
        )?;
        choice(admission.get("sourceNamespace"), &["academy.lms.dayrecord"])?;
        if !admission.get("teacherId").string()
            || !test("^[1-9][0-9]{0,15}$", admission.get("teacherId"))
        {
            return Err(invalid());
        }
        for key in ["allowedFrom", "allowedThrough"] {
            if admission.get(key) != &Js::Null {
                schema("dailyRecordDateSchema", admission.get(key))?;
            }
        }
        if admission.has("studentScope") {
            choice(
                admission.get("studentScope"),
                &["configured", "source_verified"],
            )?;
        }
        if admission.get("allowedFrom") != &Js::Null
            && admission.get("allowedThrough") != &Js::Null
            && admission.get("allowedFrom").units() > admission.get("allowedThrough").units()
        {
            return Err(invalid());
        }
        Ok(())
    })();
    if qualified.is_err() {
        prepared.held =
            Some(prepared.hold("dailyrecord_owner_read_admission_not_configured".into()));
    } else if (admission.get("allowedFrom") != &Js::Null
        && prepared.input.get("lessonDate").units() < admission.get("allowedFrom").units())
        || (admission.get("allowedThrough") != &Js::Null
            && prepared.input.get("lessonDate").units() > admission.get("allowedThrough").units())
    {
        prepared.held = Some(prepared.hold("lesson_date_out_of_admitted_scope".into()));
    }
    Ok(prepared)
}

/// Data projection, NOT deserialization into Fault/TrustedContext. Kept at this
/// boundary because the accepted core currently exposes only native projection.
pub fn public_failure_from_data(value: &Js) -> Js {
    project_failure(value, 0)
}
fn project_failure(value: &Js, depth: usize) -> Js {
    let mut fault = Fault::error(if value.get("code").string() {
        value.get("code").code_units()
    } else {
        String::new()
    });
    fault.properties = obj(&[
        ("httpStatus", value.get("sourceHttpStatus").clone()),
        ("sourceOperation", value.get("sourceOperation").clone()),
    ]);
    let native = read_failure::public_read_failure(&fault, "upstream_tool_error");
    let mut out = obj(&[
        "category",
        "code",
        "evidenceLocator",
        "automaticRetryAllowed",
    ]
    .map(|k| (k, native.get(k).clone())));
    for k in ["sourceHttpStatus", "sourceOperation"] {
        if native.has(k) {
            out.set(k, native.get(k).clone());
        }
    }
    let origin = value.get("origin");
    let positive = |v: &Js| matches!(v, Js::Num(n) if n.is_finite() && n.fract() == 0.0 && *n > 0.0 && *n <= 9_007_199_254_740_991.0);
    if [
        "academy-client.mjs",
        "dailyrecord-server.mjs",
        "server.mjs",
        "neutral-server.ts",
        "native-dailyrecord-backend.ts",
        "dailyrecord.ts",
        "prestudyRead.ts",
        "textbookCatalogRead.ts",
        "cismWeeklyProtocol.ts",
        "academyHttpClient.ts",
        "native-pool-backend.ts",
        "native-pool-socket.ts",
        "tool-audit.ts",
    ]
    .iter()
    .any(|m| origin.get("module").is(m))
        && positive(origin.get("line"))
        && positive(origin.get("column"))
    {
        // Original publicFailureFromData retains the qualified origin object.
        out.set("origin", origin.clone());
    }
    if value.get("errorSha256").string() && test("^[a-f0-9]{64}$", value.get("errorSha256")) {
        out.set("errorSha256", value.get("errorSha256").clone());
    }
    if choice(
        value.get("errorKind"),
        &["Error", "TypeError", "RangeError", "SyntaxError"],
    )
    .is_ok()
    {
        out.set("errorKind", value.get("errorKind").clone());
    }
    if value.get("cause").truthy() && depth < 2 {
        out.set("cause", project_failure(value.get("cause"), depth + 1));
    }
    out
}
#[derive(Debug)]
pub enum InvokeFailure {
    Tool { failure: Js, occurrence: Js },
    Transport(Error),
}
pub fn tool_failure(result: &Js) -> InvokeFailure {
    let raw = result.get("structuredContent");
    let failure = if raw.get("failure").truthy() {
        public_failure_from_data(raw.get("failure"))
    } else {
        let text = if result.get("content").array().len() == 1
            && result.get("content").array()[0].get("type").is("text")
        {
            result.get("content").array()[0].get("text").clone()
        } else {
            "".into()
        };
        let code = if raw.get("code").string() {
            raw.get("code").clone()
        } else if raw.get("reasonCode").string() {
            raw.get("reasonCode").clone()
        } else if text.string()
            && text
                .units()
                .starts_with(&crate::text("Input validation error:"))
        {
            "tool_input_invalid".into()
        } else {
            text
        };
        read_failure::public_read_failure(&Fault::error(code.code_units()), "upstream_tool_error")
    };
    let meta = result.get("_meta").get("academy/audit").get("occurrence");
    let occurrence = safe_occurrence(if meta.absent() {
        raw.get("occurrence")
    } else {
        meta
    });
    InvokeFailure::Tool {
        failure,
        occurrence,
    }
}
pub fn safe_occurrence(value: &Js) -> Js {
    if value.string()
        && test(
            "^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$",
            value,
        )
    {
        value.clone()
    } else {
        Js::Null
    }
}
fn held_exception(p: &Prepared, reason: &str, error: InvokeFailure) -> Js {
    let mut out = p.hold(reason.into());
    if let InvokeFailure::Tool {
        failure,
        occurrence,
    } = error
    {
        out.set("failure", failure);
        if occurrence.truthy() {
            out.set("upstreamOccurrence", occurrence);
        }
    }
    out
}

fn target(value: &Js) -> Result<Js> {
    strict(
        value,
        &[
            "sourceNamespace",
            "teacherId",
            "studentId",
            "lessonDate",
            "courseId",
            "membershipId",
            "recordId",
        ],
    )?;
    let scope = obj(&["sourceNamespace", "teacherId", "studentId", "lessonDate"]
        .map(|k| (k, value.get(k).clone())));
    let mut out = schema("dailyRecordScopeSchema", &scope)?;
    for k in ["courseId", "membershipId", "recordId"] {
        out.set(k, id(value.get(k))?);
    }
    Ok(out)
}
fn before(value: &Js) -> Result<Js> {
    object(value)?;
    if value.get("state").is("known") {
        let refs = array(value.get("evidenceRefs"), id)?;
        if refs.array().is_empty() {
            return Err(invalid());
        }
        Ok(obj(&[
            ("state", "known".into()),
            ("value", string(value.get("value"))?),
            ("evidenceRefs", refs),
        ]))
    } else {
        choice(value.get("state"), &["unknown"])?;
        Ok(obj(&[
            ("state", "unknown".into()),
            ("reasonCode", id(value.get("reasonCode"))?),
        ]))
    }
}
fn intake(value: &Js) -> Result<Js> {
    object(value)?;
    let fields = array(value.get("fields"), |v| {
        object(v)?;
        if !matches!(v.get("selected"), Js::Bool(_)) {
            return Err(invalid());
        }
        Ok(obj(&[
            ("field", schema("dailyRecordFieldSchema", v.get("field"))?),
            ("selected", v.get("selected").clone()),
            ("disposition", string(v.get("disposition"))?),
            (
                "target",
                if v.get("target") == &Js::Null {
                    Js::Null
                } else {
                    target(v.get("target"))?
                },
            ),
            ("before", before(v.get("before"))?),
        ]))
    })?;
    if fields.array().len() != 6
        || FIELDS.iter().any(|f| {
            fields
                .array()
                .iter()
                .filter(|v| v.get("field").is(f))
                .count()
                != 1
        })
    {
        return Err(invalid());
    }
    let source = if value.get("source") == &Js::Null {
        Js::Null
    } else {
        let s = value.get("source");
        object(s)?;
        if epoch_ms(s.get("observedAt"), Grammar::DailyRecord).is_none() {
            return Err(invalid());
        }
        for k in ["sourceRevision", "wireDigest"] {
            if !s.get(k).string() || !test("^[a-f0-9]{64}$", s.get(k)) {
                return Err(invalid());
            }
        }
        obj(&[
            ("observedAt", s.get("observedAt").clone()),
            ("sourceRevision", s.get("sourceRevision").clone()),
            ("wireDigest", s.get("wireDigest").clone()),
            (
                "transmissionState",
                choice(
                    s.get("transmissionState"),
                    &["unknown", "not_sent", "already_sent"],
                )?,
            ),
        ])
    };
    let mut out = obj(&[
        (
            "userContext",
            schema("dailyRecordContextSchema", value.get("userContext"))?,
        ),
        (
            "status",
            choice(
                value.get("status"),
                &["held", "interview_required", "ready_for_review"],
            )?,
        ),
        (
            "sourceAvailability",
            choice(
                value.get("sourceAvailability"),
                &["observed", "unavailable"],
            )?,
        ),
        ("blockers", array(value.get("blockers"), id)?),
        ("source", source),
        ("fields", fields),
        (
            "interviews",
            array(value.get("interviews"), |v| {
                object(v)?;
                Ok(obj(&[
                    ("field", schema("dailyRecordFieldSchema", v.get("field"))?),
                    ("reasonCode", id(v.get("reasonCode"))?),
                ]))
            })?,
        ),
    ]);
    if value.has("observationHandle") {
        out.set(
            "observationHandle",
            if value.get("observationHandle") == &Js::Null {
                Js::Null
            } else {
                id(value.get("observationHandle"))?
            },
        );
    }
    if value.has("observationPersistence") {
        out.set(
            "observationPersistence",
            choice(
                value.get("observationPersistence"),
                &["owner_durable", "process_local"],
            )?,
        );
    }
    Ok(out)
}

/// Production passes a closure borrowing ONE actual Client, OwnedChild and
/// Context for both calls. This pure layer never constructs or replaces them.
pub fn read(
    p: Prepared,
    mut invoke: impl FnMut(&str, &Js) -> std::result::Result<Js, InvokeFailure>,
    mut now: impl FnMut() -> i64,
) -> Result<Js> {
    if let Some(held) = &p.held {
        return Ok(held.clone());
    }
    let resolved = match invoke("academy_student_resolve", &p.input) {
        Ok(value) => value,
        Err(error) => {
            return Ok(held_exception(
                &p,
                "student_resolve_unqualified_no_automatic_retry",
                error,
            ));
        }
    };
    let qualified = (|| -> Result<Js> {
        object(&resolved)?;
        if resolved.get("status").is("unavailable") {
            id(resolved.get("reasonCode"))?;
            if resolved.has("upstreamOccurrence")
                && (!resolved.get("upstreamOccurrence").string()
                    || !test(
                        "^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}|00000000-0000-0000-0000-000000000000|ffffffff-ffff-ffff-ffff-ffffffffffff)$",
                        resolved.get("upstreamOccurrence"),
                    ))
            {
                return Err(invalid());
            }
            let mut out = p.hold(resolved.get("reasonCode").clone());
            if resolved.get("failure").truthy() {
                out.set("failure", public_failure_from_data(resolved.get("failure")));
            }
            if resolved.get("upstreamOccurrence").truthy() {
                out.set(
                    "upstreamOccurrence",
                    resolved.get("upstreamOccurrence").clone(),
                );
            }
            return Ok(out);
        }
        choice(resolved.get("status"), &["resolved"])?;
        if !resolved.get("studentKey").string()
            || !test("^[1-9][0-9]{0,15}$", resolved.get("studentKey"))
        {
            return Err(invalid());
        }
        string(resolved.get("displayName"))?;
        Ok(Js::Null)
    })();
    match qualified {
        Err(_) => return Ok(p.hold("student_resolve_unqualified_no_automatic_retry".into())),
        Ok(out) if out != Js::Null => return Ok(out),
        _ => (),
    }
    if resolved.get("displayName") != p.input.get("displayName") {
        return Ok(p.hold("student_name_scope_conflict".into()));
    }
    let context = schema(
        "dailyRecordContextSchema",
        &obj(&[
            ("schemaVersion", "academy.dailyrecord-user-context/1".into()),
            ("purpose", p.input.get("purpose").clone()),
            (
                "currentRequestRef",
                p.input.get("currentRequestRef").clone(),
            ),
            (
                "scope",
                obj(&[
                    ("sourceNamespace", p.owner.get("sourceNamespace").clone()),
                    ("teacherId", p.owner.get("teacherId").clone()),
                    ("studentId", resolved.get("studentKey").clone()),
                    ("lessonDate", p.input.get("lessonDate").clone()),
                ]),
            ),
            (
                "selectedFields",
                Js::Arr(FIELDS.iter().map(|f| (*f).into()).collect()),
            ),
            ("memoAnswer", obj(&[("kind", "not_supplied".into())])),
            ("facts", Js::Arr(vec![])),
        ]),
    )?;
    let raw = match invoke(
        "academy_dailyrecord_intake",
        &obj(&[("context", context.clone())]),
    ) {
        Ok(value) => value,
        Err(error) => {
            return Ok(held_exception(
                &p,
                "dailyrecord_source_unqualified_no_automatic_retry",
                error,
            ));
        }
    };
    let intake = match intake(&raw) {
        Ok(value) => value,
        Err(_) => return Ok(p.hold("dailyrecord_source_unqualified_no_automatic_retry".into())),
    };
    if !intake.get("sourceAvailability").is("observed")
        || intake.get("source") == &Js::Null
        || !intake.get("blockers").array().is_empty()
    {
        let mut out = p.hold(
            intake
                .get("blockers")
                .array()
                .first()
                .cloned()
                .unwrap_or_else(|| "dailyrecord_source_unqualified_no_automatic_retry".into()),
        );
        out.set("intakeStatus", intake.get("status").clone());
        return Ok(out);
    }
    if crate::encode(intake.get("userContext").get("scope"))?
        != crate::encode(context.get("scope"))?
    {
        return Ok(p.hold("dailyrecord_read_scope_conflict".into()));
    }
    let observed = epoch_ms(intake.get("source").get("observedAt"), Grammar::DailyRecord)
        .ok_or_else(invalid)?;
    if !(0..=300000).contains(&now().saturating_sub(observed)) {
        return Ok(p.hold("stale_source".into()));
    }
    let mut out = p.base.clone();
    for (key, value) in [
        ("status", "observed".into()),
        ("sourceAvailability", "observed".into()),
        ("scope", context.get("scope").clone()),
        (
            "route",
            Js::Arr(vec![
                "academy_student_resolve".into(),
                "academy_dailyrecord_intake".into(),
            ]),
        ),
        ("fields", intake.get("fields").clone()),
        ("source", intake.get("source").clone()),
        (
            "preparation",
            obj(&[
                ("status", intake.get("status").clone()),
                ("interviews", intake.get("interviews").clone()),
                ("requiredOnlyForAuthoring", true.into()),
            ]),
        ),
        (
            "transmissionState",
            intake.get("source").get("transmissionState").clone(),
        ),
        (
            "authoringHandle",
            if intake.get("observationPersistence").is("owner_durable")
                && intake.get("observationHandle").truthy()
            {
                obj(&[
                    ("observationHandle", intake.get("observationHandle").clone()),
                    ("persistence", "owner_durable".into()),
                    (
                        "boundTo",
                        obj(&[
                            ("scope", context.get("scope").clone()),
                            (
                                "currentRequestRef",
                                context.get("currentRequestRef").clone(),
                            ),
                        ]),
                    ),
                ])
            } else {
                Js::Null
            },
        ),
    ] {
        out.set(key, value);
    }
    Ok(out)
}
