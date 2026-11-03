//! Additional workflow schemas. Existing accepted sub-schemas remain their owner.
use crate::js::{Fault, Js, Result, obj, test, text, trim};
fn reject() -> Fault {
    Fault::error("workflow_input_rejected")
}
fn strict(v: &Js, allowed: &[&str]) -> Result<()> {
    if !matches!(v, Js::Obj(_))
        || v.props()
            .iter()
            .any(|(k, _)| !allowed.iter().any(|s| k == &text(s)))
    {
        Err(reject())
    } else {
        Ok(())
    }
}
fn string(v: &Js, min: usize, max: usize, trimmed: bool) -> Result<Js> {
    if !v.string() {
        return Err(reject());
    }
    let u = if trimmed { trim(v.units()) } else { v.units() };
    if u.len() < min || u.len() > max {
        return Err(reject());
    }
    Ok(Js::Str(u.to_vec()))
}
fn hash(v: &Js) -> Result<Js> {
    if !test("^[a-f0-9]{64}$", v) {
        return Err(reject());
    }
    Ok(v.clone())
}
fn date(v: &Js) -> Result<Js> {
    crate::knowledge::calendar_date(v)?;
    Ok(v.clone())
}
fn attr(v: &Js) -> Result<Js> {
    strict(
        v,
        &[
            "schemaVersion",
            "observedBy",
            "observedAt",
            "client",
            "process",
            "herdrPaneId",
            "declared",
        ],
    )?;
    if !v
        .get("schemaVersion")
        .is("academy.dailyrecord-attribution/1")
        || !v.get("observedBy").is("upstream") && !v.get("observedBy").is("neutral-bridge")
    {
        return Err(reject());
    }
    if crate::instant::epoch_ms(v.get("observedAt"), crate::instant::Grammar::DailyRecord).is_none()
    {
        return Err(reject());
    }
    Ok(v.clone())
}
fn instruction(v: &Js) -> Result<Js> {
    strict(v, &["text", "sessionRef", "receivedAt"])?;
    let mut out = obj(&[("text", string(v.get("text"), 1, 4000, true)?)]);
    if v.has("sessionRef") {
        out.set("sessionRef", string(v.get("sessionRef"), 1, 200, true)?);
    }
    if v.has("receivedAt") {
        if crate::instant::epoch_ms(v.get("receivedAt"), crate::instant::Grammar::DailyRecord)
            .is_none()
        {
            return Err(reject());
        }
        out.set("receivedAt", v.get("receivedAt").clone());
    }
    Ok(out)
}
fn request(v: &Js) -> Result<Js> {
    strict(v, &["requestRef", "instruction"])?;
    Ok(obj(&[
        (
            "requestRef",
            crate::schema::validate("dailyRecordIdSchema", v.get("requestRef"))?,
        ),
        ("instruction", instruction(v.get("instruction"))?),
    ]))
}
fn optional(out: &mut Js, v: &Js, key: &str, parse: fn(&Js) -> Result<Js>) -> Result<()> {
    if !v.get(key).absent() {
        out.set(key, parse(v.get(key))?);
    }
    Ok(())
}
pub fn validate(name: &str, v: &Js) -> Result<Js> {
    match name {
        "dailyRecordSaveSchema" => {
            strict(
                v,
                &[
                    "context",
                    "observationHandle",
                    "cismObservationHandle",
                    "reviewDigest",
                    "proposals",
                    "request",
                    "actor",
                    "attribution",
                ],
            )?;
            let mut subset = obj(&[]);
            for k in [
                "context",
                "observationHandle",
                "cismObservationHandle",
                "proposals",
                "actor",
                "attribution",
            ] {
                if v.has(k) {
                    subset.set(k, v.get(k).clone());
                }
            }
            let r = crate::schema::validate("dailyRecordReviewSchema", &subset)?;
            let mut out = obj(&[
                ("context", r.get("context").clone()),
                (
                    "observationHandle",
                    crate::schema::validate("dailyRecordIdSchema", r.get("observationHandle"))?,
                ),
            ]);
            optional(&mut out, &r, "cismObservationHandle", |v| Ok(v.clone()))?;
            out.set("reviewDigest", hash(v.get("reviewDigest"))?);
            out.set("proposals", r.get("proposals").clone());
            optional(&mut out, v, "request", request)?;
            optional(&mut out, &r, "actor", |v| Ok(v.clone()))?;
            optional(&mut out, &r, "attribution", attr)?;
            Ok(out)
        }
        "dailyRecordReconcileSchema" => {
            strict(v, &["context", "resolution", "actor", "attribution"])?;
            let mut out = obj(&[(
                "context",
                crate::schema::validate("dailyRecordContextSchema", v.get("context"))?,
            )]);
            if !v.get("resolution").absent() {
                let r = v.get("resolution");
                strict(r, &["request", "executions"])?;
                let es = r.get("executions");
                if !matches!(es, Js::Arr(_)) || es.array().is_empty() || es.array().len() > 6 {
                    return Err(reject());
                }
                let es = es
                    .array()
                    .iter()
                    .map(|e| {
                        strict(e, &["requestRef", "field"])?;
                        Ok(obj(&[
                            (
                                "requestRef",
                                crate::schema::validate(
                                    "dailyRecordIdSchema",
                                    e.get("requestRef"),
                                )?,
                            ),
                            (
                                "field",
                                crate::schema::validate("dailyRecordFieldSchema", e.get("field"))?,
                            ),
                        ]))
                    })
                    .collect::<Result<Vec<_>>>()?;
                out.set(
                    "resolution",
                    obj(&[
                        ("request", request(r.get("request"))?),
                        ("executions", Js::Arr(es)),
                    ]),
                );
            }
            optional(&mut out, v, "actor", |v| Ok(v.clone()))?;
            optional(&mut out, v, "attribution", attr)?;
            Ok(out)
        }
        "dailyRecordExecutionStatusSchema" => {
            strict(v, &["scope", "requestRef", "actor", "attribution"])?;
            let mut out = obj(&[(
                "scope",
                crate::schema::validate("dailyRecordScopeSchema", v.get("scope"))?,
            )]);
            optional(&mut out, v, "requestRef", |v| {
                crate::schema::validate("dailyRecordIdSchema", v)
            })?;
            optional(&mut out, v, "actor", |v| Ok(v.clone()))?;
            optional(&mut out, v, "attribution", attr)?;
            Ok(out)
        }
        "cismGetInput" => {
            strict(v, &["date"])?;
            Ok(obj(&[("date", date(v.get("date"))?)]))
        }
        "cismReviewInput" | "cismSaveInput" => {
            let allowed = if name == "cismSaveInput" {
                &[
                    "date",
                    "expectedReportSha256",
                    "request",
                    "changes",
                    "attribution",
                    "reviewDigest",
                ][..]
            } else {
                &[
                    "date",
                    "expectedReportSha256",
                    "request",
                    "changes",
                    "attribution",
                ][..]
            };
            strict(v, allowed)?;
            let r = v.get("request");
            strict(r, &["requestRef", "instruction"])?;
            let r = obj(&[
                ("requestRef", string(r.get("requestRef"), 1, 200, false)?),
                ("instruction", string(r.get("instruction"), 1, 8000, true)?),
            ]);
            let changes = v.get("changes");
            if !matches!(changes, Js::Arr(_))
                || changes.array().is_empty()
                || changes.array().len() > 80
            {
                return Err(reject());
            }
            let changes = changes
                .array()
                .iter()
                .map(|c| {
                    strict(c, &["fieldRef", "before", "after", "edit", "evidenceRefs"])?;
                    let edit = if c.get("edit").absent() {
                        "fill_empty".into()
                    } else {
                        c.get("edit").clone()
                    };
                    if !edit.is("fill_empty") && !edit.is("complete_existing_narrative") {
                        return Err(reject());
                    }
                    let refs = c.get("evidenceRefs");
                    if !matches!(refs, Js::Arr(_))
                        || refs.array().is_empty()
                        || refs.array().len() > 30
                    {
                        return Err(reject());
                    }
                    let refs = refs
                        .array()
                        .iter()
                        .map(|r| string(r, 1, 400, false))
                        .collect::<Result<Vec<_>>>()?;
                    Ok(obj(&[
                        ("fieldRef", hash(c.get("fieldRef"))?),
                        ("before", string(c.get("before"), 0, 6000, false)?),
                        ("after", string(c.get("after"), 1, 6000, true)?),
                        ("edit", edit),
                        ("evidenceRefs", Js::Arr(refs)),
                    ]))
                })
                .collect::<Result<Vec<_>>>()?;
            let mut out = obj(&[
                ("date", date(v.get("date"))?),
                ("expectedReportSha256", hash(v.get("expectedReportSha256"))?),
                ("request", r),
                ("changes", Js::Arr(changes)),
            ]);
            optional(&mut out, v, "attribution", attr)?;
            if name == "cismSaveInput" {
                out.set("reviewDigest", hash(v.get("reviewDigest"))?);
            }
            Ok(out)
        }
        _ => crate::schema::validate(name, v),
    }
}
