//! V3 finite, lossless source comparison adapter. Source refusals and adapter
//! admission failures are different channels. No oracle files are read by Rust.
use crate::{
    decision,
    js::*,
    knowledge, templates,
    wire::{TestPorts, Wire},
};
use serde::Deserialize;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub op: String,
    pub wire: Wire,
    pub ports: Option<TestPorts>,
}
fn keys(v: &Js, required: &[&str], optional: &[&str]) -> bool {
    matches!(v, Js::Obj(_))
        && required.iter().all(|k| v.has(k))
        && v.props()
            .iter()
            .all(|(k, _)| required.iter().chain(optional).any(|n| k == &text(n)))
}
fn array(v: &Js, max: usize) -> bool {
    matches!(v,Js::Arr(a) if a.len()<=max)
}
fn string(v: &Js, max: usize) -> bool {
    v.string() && v.units().len() <= max
}
fn safe_text(v: &Js, max: usize) -> bool {
    string(v, max)
        && !trim(v.units()).is_empty()
        && !v
            .units()
            .iter()
            .any(|u| matches!(*u,0..=8|11|12|14..=31|127))
}
fn id(v: &Js) -> bool {
    safe_text(v, 256)
}
fn ids(v: &Js, min: usize, max: usize) -> bool {
    array(v, max) && v.array().len() >= min && v.array().iter().all(id)
}
fn one(v: &Js, values: &[&str]) -> bool {
    values.iter().any(|s| v.is(s))
}
fn date(v: &Js) -> bool {
    knowledge::calendar_date(v).is_ok()
}
fn utc(v: &Js) -> bool {
    if !v.string() {
        return false;
    }
    let s = v.code_units();
    (s.len() == 20 || s.len() == 24)
        && test(
            r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T(?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9](?:\.[0-9]{3})?Z$",
            v,
        )
        && date(&s[..10].into())
}
fn scope(v: &Js) -> bool {
    keys(
        v,
        &["sourceNamespace", "teacherId", "studentId", "lessonDate"],
        &[],
    ) && ["sourceNamespace", "teacherId", "studentId"]
        .iter()
        .all(|k| id(v.get(k)))
        && date(v.get("lessonDate"))
}
fn target(v: &Js) -> bool {
    keys(
        v,
        &[
            "sourceNamespace",
            "teacherId",
            "studentId",
            "lessonDate",
            "courseId",
            "membershipId",
            "recordId",
        ],
        &[],
    ) && [
        "sourceNamespace",
        "teacherId",
        "studentId",
        "courseId",
        "membershipId",
        "recordId",
    ]
    .iter()
    .all(|k| id(v.get(k)))
        && date(v.get("lessonDate"))
}
fn known_field(v: &Js) -> bool {
    if v.get("state").is("known") {
        keys(v, &["state", "value", "evidenceRefs"], &[])
            && string(v.get("value"), 8192)
            && ids(v.get("evidenceRefs"), 1, 32)
    } else {
        v.get("state").is("unknown")
            && keys(v, &["state", "reasonCode"], &[])
            && id(v.get("reasonCode"))
    }
}
fn fact(f: &Js) -> bool {
    if !keys(
        f,
        &[
            "ref",
            "field",
            "text",
            "requiredFragments",
            "questionCounts",
            "deadlines",
            "stageClaims",
        ],
        &["origin", "explicitValue"],
    ) || !id(f.get("ref"))
        || !one(f.get("field"), &decision::FIELDS)
        || !safe_text(f.get("text"), 4096)
        || f.has("origin") && !one(f.get("origin"), &["user_supplied", "cism_section"])
        || f.has("explicitValue") && !string(f.get("explicitValue"), 8192)
    {
        return false;
    }
    if f.has("explicitValue")
        && !one(f.get("field"), &["progress", "homework", "lessonMemo"])
        && !decision::valid_value(&f.get("field").code_units(), f.get("explicitValue"))
    {
        return false;
    }
    array(f.get("requiredFragments"),32)&&f.get("requiredFragments").array().iter().all(|t|safe_text(t,4096))&&
    array(f.get("questionCounts"),32)&&f.get("questionCounts").array().iter().all(|c|keys(c,&["count","rendering"],&[])&&matches!(c.get("count"),Js::Num(n) if n.fract()==0.0&&(0.0..=100000.0).contains(n))&&safe_text(c.get("rendering"),256))&&
    array(f.get("deadlines"),16)&&f.get("deadlines").array().iter().all(|d|keys(d,&["date","rendering"],&[])&&date(d.get("date"))&&safe_text(d.get("rendering"),256)&&deadline_render(d))&&
    array(f.get("stageClaims"),32)&&f.get("stageClaims").array().iter().all(|s|keys(s,&["activity","stage","status","rendering"],&[])&&safe_text(s.get("activity"),256)&&one(s.get("stage"),&["solving","grading","correction","teacher_review","assigned_homework"])&&one(s.get("status"),&["in_progress","complete","unconfirmed","assigned"])&&safe_text(s.get("rendering"),256))
}
fn deadline_render(d: &Js) -> bool {
    let s = d.get("date").code_units();
    let y = s[..4].parse::<u32>().expect("admitted date");
    let m = s[5..7].parse::<u32>().expect("admitted date");
    let day = s[8..].parse::<u32>().expect("admitted date");
    let r = d.get("rendering").code_units();
    regex(r"(?:([0-9]{4})[-./]([0-9]{1,2})[-./]([0-9]{1,2})|([0-9]{1,2})\s*월\s*([0-9]{1,2})\s*일)")
        .captures_iter(&r)
        .all(|c| {
            if c.get(1).is_some() {
                c[1].parse::<u32>() == Ok(y)
                    && c[2].parse::<u32>() == Ok(m)
                    && c[3].parse::<u32>() == Ok(day)
            } else {
                c[4].parse::<u32>() == Ok(m) && c[5].parse::<u32>() == Ok(day)
            }
        })
}
fn context(c: &Js) -> bool {
    if !keys(
        c,
        &[
            "schemaVersion",
            "purpose",
            "currentRequestRef",
            "scope",
            "selectedFields",
            "memoAnswer",
            "facts",
        ],
        &[],
    ) || !c
        .get("schemaVersion")
        .is("academy.dailyrecord-user-context/1")
        || !safe_text(c.get("purpose"), 4096)
        || !id(c.get("currentRequestRef"))
        || !scope(c.get("scope"))
        || !array(c.get("selectedFields"), 6)
        || c.get("selectedFields").array().is_empty()
        || !c
            .get("selectedFields")
            .array()
            .iter()
            .all(|f| one(f, &decision::FIELDS))
        || unique(c.get("selectedFields").array()).len() != c.get("selectedFields").array().len()
        || !array(c.get("facts"), 64)
        || !c.get("facts").array().iter().all(fact)
    {
        return false;
    }
    let facts: Vec<_> = c
        .get("facts")
        .array()
        .iter()
        .map(|f| f.get("ref").clone())
        .collect();
    if unique(&facts).len() != facts.len() {
        return false;
    }
    let m = c.get("memoAnswer");
    match m.get("kind").code_units().as_str() {
        "not_supplied" => keys(m, &["kind"], &[]),
        "no_addition" => keys(m, &["kind", "requestRef"], &[]) && id(m.get("requestRef")),
        "content" => {
            keys(m, &["kind", "requestRef", "text"], &[])
                && id(m.get("requestRef"))
                && safe_text(m.get("text"), 4096)
        }
        _ => false,
    }
}
fn proposal(p: &Js) -> bool {
    if !keys(
        p,
        &["field", "value", "factRefs", "decision"],
        &["learningEntries", "teacherReported"],
    ) || !one(p.get("field"), &decision::FIELDS)
        || !string(p.get("value"), 8192)
        || !ids(p.get("factRefs"), 1, 32)
        || !keys(p.get("decision"), &["kind", "requestRef"], &[])
        || !one(p.get("decision").get("kind"), &["set", "replace", "clear"])
        || !id(p.get("decision").get("requestRef"))
    {
        return false;
    }
    if p.has("learningEntries")
        && (!array(p.get("learningEntries"), 64)
            || !p.get("learningEntries").array().iter().all(|e| {
                keys(
                    e,
                    &[
                        "bookRef",
                        "unitRef",
                        "scope",
                        "sectionRefs",
                        "activity",
                        "deadline",
                    ],
                    &[],
                ) && id(e.get("bookRef"))
                    && id(e.get("unitRef"))
                    && one(e.get("scope"), &["unit", "sections"])
                    && ids(e.get("sectionRefs"), 0, 64)
                    && string(e.get("activity"), 400)
                    && !e.get("activity").units().is_empty()
                    && (matches!(e.get("deadline"), Js::Null) || date(e.get("deadline")))
            }))
    {
        return false;
    }
    if p.has("teacherReported") {
        let t = p.get("teacherReported");
        if !keys(t, &["preserveBefore", "mappedText", "lines"], &[])
            || !matches!(t.get("preserveBefore"), Js::Bool(_))
            || !string(t.get("mappedText"), 8192)
            || !array(t.get("lines"), 32)
            || t.get("lines").array().is_empty()
            || !t.get("lines").array().iter().all(|l| {
                keys(l, &["factRef", "text"], &[])
                    && id(l.get("factRef"))
                    && string(l.get("text"), 1000)
                    && l.get("text").units().len() >= 3
            })
        {
            return false;
        }
    }
    true
}
fn observation(o: &Js) -> bool {
    if matches!(o, Js::Null) {
        return true;
    }
    if !keys(
        o,
        &[
            "scope",
            "target",
            "previousTarget",
            "observedAt",
            "sourceRevision",
            "wireDigest",
            "fields",
            "toneSamples",
            "transmissionState",
        ],
        &[],
    ) || !scope(o.get("scope"))
        || !target(o.get("target"))
        || !(matches!(o.get("previousTarget"), Js::Null) || target(o.get("previousTarget")))
        || !utc(o.get("observedAt"))
        || !["sourceRevision", "wireDigest"]
            .iter()
            .all(|k| test("^[a-f0-9]{64}$", o.get(k)))
        || !array(o.get("toneSamples"), 3)
        || !one(
            o.get("transmissionState"),
            &["not_sent", "already_sent", "unknown"],
        )
    {
        return false;
    }
    let fields = o.get("fields");
    if !keys(fields, &[], &decision::FIELDS) || fields.props().iter().any(|(_, v)| !known_field(v))
    {
        return false;
    }
    // Missing fields are deliberately retained for the original Zod read-failure diagnostic.
    o.get("toneSamples").array().iter().all(|s| {
        keys(s, &["target", "observedAt", "text", "evidenceRefs"], &[])
            && target(s.get("target"))
            && utc(s.get("observedAt"))
            && string(s.get("text"), 8192)
            && ids(s.get("evidenceRefs"), 1, 32)
    })
}
fn bounded_json(v: &Js, depth: usize) -> bool {
    if depth > 64 || matches!(v, Js::Cycle) {
        return false;
    }
    match v {
        Js::Arr(a) | Js::Decorated(a, _) => a.iter().all(|v| bounded_json(v, depth + 1)),
        Js::Obj(p) | Js::NullPrototype(p) | Js::Accessor(p) | Js::SymbolKeys(p) => {
            p.iter().all(|(_, v)| bounded_json(v, depth + 1))
        }
        _ => true,
    }
}
pub fn execute(request: Request) -> std::result::Result<String, &'static str> {
    if request.op != "semantic_v3" {
        return Err("s08_protocol_input_rejected");
    }
    let input = request.wire.decode()?;
    let mut events = vec![];
    let op = input.get("op").code_units();
    let admitted = match op.as_str() {
        "json" => {
            keys(&input, &["op", "family", "value"], &[])
                && one(input.get("family"), &["learning", "admission", "cism"])
                && (input.get("family").is("learning") || bounded_json(input.get("value"), 0))
        }
        "template" | "knowledge" | "calendar_date" => keys(&input, &["op", "value"], &[]),
        "known" => keys(&input, &["op", "value", "refs"], &[]),
        "unknown" => keys(&input, &["op", "reason"], &[]),
        "constants" => keys(&input, &["op"], &[]),
        "http_action" => keys(&input, &["op", "action", "state"], &[]),
        "review" => {
            keys(
                &input,
                &["op", "observation", "context", "proposals"],
                &["learning", "safeguard"],
            ) && context(input.get("context"))
                && array(input.get("proposals"), 6)
                && !input.get("proposals").array().is_empty()
                && input.get("proposals").array().iter().all(proposal)
                && observation(input.get("observation"))
                && (!input.has("safeguard") || matches!(input.get("safeguard"), Js::Bool(_)))
                && (!input.has("learning")
                    || matches!(input.get("learning"), Js::Null)
                    || keys(input.get("learning"), &[], &["blockers", "scopeQualified"])
                        && (!input.get("learning").has("blockers")
                            || ids(input.get("learning").get("blockers"), 0, 64))
                        && (!input.get("learning").has("scopeQualified")
                            || matches!(input.get("learning").get("scopeQualified"), Js::Bool(_))))
                && request.ports.as_ref().is_some_and(|p| {
                    utc(&p.now.as_str().into())
                        && regex(
                            "^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$",
                        )
                        .is_match(&p.uuid)
                })
        }
        _ => false,
    };
    if !admitted {
        return Err("s08_semantic_admission_required");
    }
    let result = match op.as_str() {
        "json" => json_family(&input, &mut events),
        "template" => templates::render(input.get("value")),
        "calendar_date" => knowledge::calendar_date(input.get("value")),
        "knowledge" => {
            knowledge::assert_knowledge(input.get("value")).map(|()| input.get("value").clone())
        }
        "known" => knowledge::known(input.get("value"), input.get("refs")),
        "unknown" => knowledge::unknown(input.get("reason")),
        "constants" => Ok(templates::constants()),
        "review" => decision::review(
            &input,
            request.ports.as_ref().expect("admitted test provider"),
            &mut events,
        ),
        "http_action" => decision::fake_http_action(&input, &mut events),
        _ => unreachable!("admitted operation"),
    };
    if result.as_ref().is_err_and(|e| e.name == "AdapterAdmission") {
        return Err("s08_semantic_admission_required");
    }
    let outcome = match result {
        Ok(v) => obj(&[("kind", "return".into()), ("value", v)]),
        Err(e) => obj(&[("kind", "throw".into()), ("error", e.value())]),
    };
    let frame = obj(&[
        ("outcome", outcome),
        ("events", Js::Arr(events)),
        (
            "effects",
            obj(&[
                ("deniedTransportAttempts", Js::Num(0.0)),
                ("forbiddenFakeDispatches", Js::Num(0.0)),
            ]),
        ),
    ]);
    stringify(&frame)
        .map(|s| s + "\n")
        .map_err(|_| "s08_semantic_output_rejected")
}
fn json_family(input: &Js, events: &mut Vec<Js>) -> Result<Js> {
    let family = if input.get("family").is("learning") {
        Family::Learning
    } else if input.get("family").is("admission") {
        Family::Admission
    } else {
        Family::Cism
    };
    let value = input.get("value");
    let bytes = encode(value, family, events)?;
    let hashed=encode(value,family,events)?.ok_or_else(||Fault::type_error("The \"data\" argument must be of type string or an instance of Buffer, TypedArray, or DataView. Received undefined"))?;
    let mut out = obj(&[("sha256", sha(&hashed).into())]);
    if let Some(b) = bytes {
        out.set("bytes", b.into());
    }
    Ok(out)
}
