//! Ordered validation for the retained Zod 4.4.3 schema closure.
//! This is a data-schema interpreter, not JavaScript execution.
use crate::{decision, instant, js::*, knowledge};
#[derive(Clone)]
enum S {
    Str(Vec<C>),
    Num(i64, i64),
    Bool,
    Obj(Vec<(&'static str, S)>),
    Arr(Box<S>, usize, usize),
    Enum(Vec<&'static str>),
    Union(&'static str, Vec<(&'static str, S)>),
    Optional(Box<S>),
    Nullable(Box<S>),
    Default(Box<S>, Js),
    Refined(Box<S>, &'static str),
}
#[derive(Clone)]
enum C {
    Min(usize),
    Max(usize),
    Safe,
    Trim,
    Regex(&'static str),
    Date,
    Instant,
    IsoDate,
}
fn st(max: usize) -> S {
    S::Str(vec![C::Max(max)])
}
fn txt(max: usize) -> S {
    S::Str(vec![C::Min(1), C::Max(max), C::Safe])
}
fn limited_text(max: usize) -> S {
    S::Str(vec![C::Min(1), C::Max(4096), C::Safe, C::Max(max)])
}
fn id() -> S {
    S::Str(vec![C::Min(1), C::Max(4096), C::Safe, C::Max(256)])
}
fn raw_id() -> S {
    S::Str(vec![C::Min(1), C::Max(256)])
}
fn date() -> S {
    S::Str(vec![C::Regex(r"^\d{4}-\d{2}-\d{2}$"), C::Date])
}
fn hash() -> S {
    S::Str(vec![C::Regex("^[a-f0-9]{64}$")])
}
fn fields() -> S {
    S::Enum(decision::FIELDS.to_vec())
}
fn en(v: &[&'static str]) -> S {
    S::Enum(v.to_vec())
}
fn lit(v: &'static str) -> S {
    en(&[v])
}
fn o(v: Vec<(&'static str, S)>) -> S {
    S::Obj(v)
}
fn ar(v: S, min: usize, max: usize) -> S {
    S::Arr(Box::new(v), min, max)
}
fn opt(v: S) -> S {
    S::Optional(Box::new(v))
}
fn nullable(v: S) -> S {
    S::Nullable(Box::new(v))
}
fn refine(v: S, name: &'static str) -> S {
    S::Refined(Box::new(v), name)
}
fn scope() -> S {
    o(vec![
        ("sourceNamespace", id()),
        ("teacherId", id()),
        ("studentId", id()),
        ("lessonDate", date()),
    ])
}
fn target() -> S {
    let S::Obj(mut p) = scope() else {
        unreachable!()
    };
    p.extend([
        ("courseId", id()),
        ("membershipId", id()),
        ("recordId", id()),
    ]);
    o(p)
}
fn fact() -> S {
    refine(
        o(vec![
            ("ref", id()),
            ("field", fields()),
            ("text", txt(4096)),
            (
                "origin",
                S::Default(
                    Box::new(en(&["user_supplied", "cism_section"])),
                    "user_supplied".into(),
                ),
            ),
            ("explicitValue", opt(st(8192))),
            ("requiredFragments", ar(txt(4096), 0, 32)),
            (
                "questionCounts",
                ar(
                    o(vec![
                        ("count", S::Num(0, 100000)),
                        ("rendering", limited_text(256)),
                    ]),
                    0,
                    32,
                ),
            ),
            (
                "deadlines",
                ar(
                    refine(
                        o(vec![("date", date()), ("rendering", limited_text(256))]),
                        "deadline",
                    ),
                    0,
                    16,
                ),
            ),
            (
                "stageClaims",
                ar(
                    o(vec![
                        ("activity", limited_text(256)),
                        (
                            "stage",
                            en(&[
                                "solving",
                                "grading",
                                "correction",
                                "teacher_review",
                                "assigned_homework",
                            ]),
                        ),
                        (
                            "status",
                            en(&["in_progress", "complete", "unconfirmed", "assigned"]),
                        ),
                        ("rendering", limited_text(256)),
                    ]),
                    0,
                    32,
                ),
            ),
        ]),
        "fact",
    )
}
fn context() -> S {
    refine(
        o(vec![
            ("schemaVersion", lit("academy.dailyrecord-user-context/1")),
            ("purpose", txt(4096)),
            ("currentRequestRef", id()),
            ("scope", scope()),
            ("selectedFields", ar(fields(), 1, 6)),
            (
                "memoAnswer",
                S::Union(
                    "kind",
                    vec![
                        ("not_supplied", o(vec![("kind", lit("not_supplied"))])),
                        (
                            "no_addition",
                            o(vec![("kind", lit("no_addition")), ("requestRef", id())]),
                        ),
                        (
                            "content",
                            o(vec![
                                ("kind", lit("content")),
                                ("requestRef", id()),
                                ("text", txt(4096)),
                            ]),
                        ),
                    ],
                ),
            ),
            ("facts", ar(fact(), 0, 64)),
        ]),
        "context",
    )
}
fn proposal() -> S {
    o(vec![
        ("field", fields()),
        ("value", st(8192)),
        ("factRefs", ar(id(), 1, 32)),
        (
            "decision",
            o(vec![
                ("kind", en(&["set", "replace", "clear"])),
                ("requestRef", id()),
            ]),
        ),
        (
            "learningEntries",
            opt(ar(
                o(vec![
                    ("bookRef", id()),
                    ("unitRef", id()),
                    ("scope", en(&["unit", "sections"])),
                    ("sectionRefs", ar(id(), 0, 64)),
                    ("activity", S::Str(vec![C::Min(1), C::Max(400)])),
                    ("deadline", nullable(date())),
                ]),
                0,
                64,
            )),
        ),
        (
            "teacherReported",
            opt(o(vec![
                ("preserveBefore", S::Bool),
                ("mappedText", st(8192)),
                (
                    "lines",
                    ar(
                        o(vec![
                            ("factRef", id()),
                            ("text", S::Str(vec![C::Min(3), C::Max(1000)])),
                        ]),
                        1,
                        32,
                    ),
                ),
            ])),
        ),
    ])
}
fn short() -> S {
    S::Str(vec![C::Trim, C::Min(1), C::Max(200)])
}
fn actor() -> S {
    o(vec![
        ("runtime", short()),
        ("sessionRef", opt(short())),
        ("paneRef", opt(short())),
        ("model", opt(short())),
    ])
}
fn observation() -> S {
    let known = S::Union(
        "state",
        vec![
            (
                "known",
                o(vec![
                    ("state", lit("known")),
                    ("value", st(8192)),
                    ("evidenceRefs", ar(id(), 1, 32)),
                ]),
            ),
            (
                "unknown",
                o(vec![("state", lit("unknown")), ("reasonCode", id())]),
            ),
        ],
    );
    o(vec![
        ("scope", scope()),
        ("target", target()),
        ("previousTarget", nullable(target())),
        ("observedAt", S::Str(vec![C::Instant])),
        ("sourceRevision", hash()),
        ("wireDigest", hash()),
        (
            "fields",
            o(decision::FIELDS
                .iter()
                .map(|f| (*f, known.clone()))
                .collect()),
        ),
        (
            "toneSamples",
            ar(
                o(vec![
                    ("target", target()),
                    ("observedAt", S::Str(vec![C::Instant])),
                    ("text", st(8192)),
                    ("evidenceRefs", ar(id(), 1, 32)),
                ]),
                0,
                3,
            ),
        ),
        (
            "transmissionState",
            en(&["not_sent", "already_sent", "unknown"]),
        ),
    ])
}
fn selection() -> S {
    refine(
        o(vec![
            ("reportId", id()),
            ("periodStart", date()),
            ("periodEnd", date()),
            ("section", lit("learning_support")),
        ]),
        "period",
    )
}
fn evidence() -> S {
    o(vec![
        ("runtime", raw_id()),
        ("requestRef", raw_id()),
        ("requestTextSha256", hash()),
        ("currentUserEventRef", raw_id()),
    ])
}
fn admission() -> S {
    let S::Obj(p) = evidence() else {
        unreachable!()
    };
    let mut v = vec![("schemaVersion", lit("academy.dailyrecord-save-admission/1"))];
    v.extend(p);
    v.push((
        "admitted",
        ar(
            o(vec![
                ("studentId", raw_id()),
                ("lessonDate", S::Str(vec![C::IsoDate])),
                ("field", fields()),
                ("effectRecordKey", raw_id()),
                ("after", st(8192)),
            ]),
            1,
            32,
        ),
    ));
    o(v)
}
fn named(name: &str) -> Option<S> {
    Some(match name {
        "dailyRecordFieldSchema" => fields(),
        "dailyRecordIdSchema" => id(),
        "dailyRecordDateSchema" => date(),
        "dailyRecordScopeSchema" => scope(),
        "dailyRecordFactSchema" => fact(),
        "dailyRecordContextSchema" => context(),
        "dailyRecordObservationSchema" => observation(),
        "dailyRecordSaveAdmissionSchema" => admission(),
        "dailyRecordSaveRequestEvidenceSchema" => evidence(),
        "dailyRecordIntakeSchema" => o(vec![
            ("context", context()),
            (
                "suppliedValues",
                S::Default(Box::new(ar(proposal(), 0, 6)), Js::Arr(vec![])),
            ),
            ("actor", opt(actor())),
            ("attribution", opt(attribution())),
        ]),
        "dailyRecordReviewSchema" => o(vec![
            ("context", context()),
            ("observationHandle", opt(id())),
            ("cismObservationHandle", opt(id())),
            ("proposals", ar(proposal(), 1, 6)),
            ("actor", opt(actor())),
            ("attribution", opt(attribution())),
        ]),
        "cismSectionSchema" => o(vec![("context", context()), ("selection", selection())]),
        "cismObservationSchema" => o(vec![
            ("scope", scope()),
            ("selection", selection()),
            ("observedAt", S::Str(vec![C::Instant])),
            ("sourceRevision", hash()),
            ("sourceReferenceDate", date()),
            ("sectionLabel", lit("학습부진아 지도")),
            ("text", st(20000)),
            ("evidenceRefs", ar(id(), 1, 32)),
        ]),
        _ => return None,
    })
}
fn attribution() -> S {
    o(vec![
        ("schemaVersion", lit("academy.dailyrecord-attribution/1")),
        ("observedBy", en(&["neutral-bridge", "upstream"])),
        ("observedAt", S::Str(vec![C::Instant])),
        (
            "client",
            nullable(o(vec![("name", short()), ("version", short())])),
        ),
        (
            "process",
            o(vec![
                ("pid", S::Num(1, i64::MAX)),
                ("ppid", S::Num(0, i64::MAX)),
                ("uid", nullable(S::Num(0, i64::MAX))),
                ("parentCommand", nullable(short())),
                ("bridgeSha256", nullable(hash())),
            ]),
        ),
        ("herdrPaneId", nullable(short())),
        ("declared", nullable(actor())),
    ])
}
const DATE_SOURCE: &str = r"(?:(?:\d\d[2468][048]|\d\d[13579][26]|\d\d0[48]|[02468][048]00|[13579][26]00)-02-29|\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\d|30)|(?:02)-(?:0[1-9]|1\d|2[0-8])))";
pub fn pretty(v: &Js, depth: usize) -> String {
    let pad = |d| "  ".repeat(d);
    match v {
        Js::Arr(a) if !a.is_empty() => format!(
            "[\n{}\n{}]",
            a.iter()
                .map(|v| format!("{}{}", pad(depth + 1), pretty(v, depth + 1)))
                .collect::<Vec<_>>()
                .join(",\n"),
            pad(depth)
        ),
        Js::Obj(p) if !p.is_empty() => format!(
            "{{\n{}\n{}}}",
            enumerate(p)
                .iter()
                .map(|(k, v)| format!("{}{}: {}", pad(depth + 1), quote(k), pretty(v, depth + 1)))
                .collect::<Vec<_>>()
                .join(",\n"),
            pad(depth)
        ),
        _ => stringify(v).expect("issue JSON"),
    }
}
fn received(v: &Js) -> &'static str {
    match v {
        Js::Null => "null",
        Js::Undefined | Js::Hole => "undefined",
        Js::Str(_) => "string",
        Js::Num(n) if n.is_nan() => "NaN",
        Js::Num(_) => "number",
        Js::Bool(_) => "boolean",
        Js::Arr(_) | Js::Sparse(_) => "array",
        _ => "object",
    }
}
fn path(v: &[Js]) -> Js {
    Js::Arr(v.to_vec())
}
fn issue(issues: &mut Vec<Js>, p: &[Js], fields: &[(&str, Js)], message: String) {
    let mut v = obj(fields);
    v.set("path", path(p));
    v.set("message", message.into());
    issues.push(v);
}
fn custom(issues: &mut Vec<Js>, p: &[Js], message: &str, super_refine: bool) {
    if super_refine {
        issues.push(obj(&[
            ("code", "custom".into()),
            ("message", message.into()),
            ("path", path(p)),
        ]));
    } else {
        issue(issues, p, &[("code", "custom".into())], message.into());
    }
}
fn type_issue(issues: &mut Vec<Js>, p: &[Js], expected: &str, v: &Js, union: bool) {
    let mut f = vec![
        ("expected", expected.into()),
        ("code", "invalid_type".into()),
    ];
    if union {
        f.swap(0, 1);
    }
    issue(
        issues,
        p,
        &f,
        format!(
            "Invalid input: expected {expected}, received {}",
            received(v)
        ),
    );
}
fn size(issues: &mut Vec<Js>, p: &[Js], origin: &str, n: usize, min: bool) {
    let mut f = vec![
        ("origin", origin.into()),
        ("code", if min { "too_small" } else { "too_big" }.into()),
        (if min { "minimum" } else { "maximum" }, n.into()),
        ("inclusive", true.into()),
    ];
    let unit = if origin == "string" {
        "characters"
    } else {
        "items"
    };
    issue(
        issues,
        p,
        &f,
        format!(
            "Too {}: expected {origin} to have {}{n} {unit}",
            if min { "small" } else { "big" },
            if min { ">=" } else { "<=" }
        ),
    );
    f.clear();
}
fn parse(s: &S, v: &Js, p: &[Js], issues: &mut Vec<Js>, abort: &mut bool) -> Js {
    let start_abort = *abort;
    match s {
        S::Optional(s) => {
            if matches!(v, Js::Undefined) {
                v.clone()
            } else {
                parse(s, v, p, issues, abort)
            }
        }
        S::Nullable(s) => {
            if matches!(v, Js::Null) {
                Js::Null
            } else {
                parse(s, v, p, issues, abort)
            }
        }
        S::Default(s, d) => {
            if matches!(v, Js::Undefined) {
                d.clone()
            } else {
                parse(s, v, p, issues, abort)
            }
        }
        S::Refined(s, name) => {
            let mut local = false;
            let r = parse(s, v, p, issues, &mut local);
            if !local {
                refinement(name, &r, p, issues);
            }
            *abort = start_abort || local;
            r
        }
        S::Str(checks) => {
            if !v.string() {
                type_issue(issues, p, "string", v, false);
                *abort = true;
                return v.clone();
            }
            let normalized = if checks.iter().any(|c| matches!(c, C::Trim)) {
                Js::Str(trim(v.units()).to_vec())
            } else {
                v.clone()
            };
            let v = &normalized;
            for c in checks {
                match c {
                    C::Min(n) if v.units().len() < *n => size(issues, p, "string", *n, true),
                    C::Max(n) if v.units().len() > *n => size(issues, p, "string", *n, false),
                    C::Safe
                        if trim(v.units()).is_empty()
                            || v.units()
                                .iter()
                                .any(|u| matches!(*u,0..=8|11|12|14..=31|127)) =>
                    {
                        custom(issues, p, "nonempty_safe_text_required", false)
                    }
                    C::Regex(re) if !test(re, v) => {
                        let pat = format!("/{re}/");
                        issue(
                            issues,
                            p,
                            &[
                                ("origin", "string".into()),
                                ("code", "invalid_format".into()),
                                ("format", "regex".into()),
                                ("pattern", pat.clone().into()),
                            ],
                            format!("Invalid string: must match pattern {pat}"),
                        );
                    }
                    C::Date if knowledge::calendar_date(v).is_err() => {
                        custom(issues, p, "real_calendar_date_required", false)
                    }
                    C::Instant | C::IsoDate => {
                        let instant = matches!(c, C::Instant);
                        let valid = if instant {
                            instant::epoch_ms(v, instant::Grammar::DailyRecord).is_some()
                        } else {
                            knowledge::calendar_date(v).is_ok()
                        };
                        if !valid {
                            let re = if instant {
                                format!(
                                    r"/^{DATE_SOURCE}T(?:(?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d(?:\.\d+)?)?(?:Z|([+-](?:[01]\d|2[0-3]):[0-5]\d)))$/"
                                )
                            } else {
                                format!("/^{DATE_SOURCE}$/")
                            };
                            issue(
                                issues,
                                p,
                                &[
                                    ("origin", "string".into()),
                                    ("code", "invalid_format".into()),
                                    ("format", if instant { "datetime" } else { "date" }.into()),
                                    ("pattern", re.into()),
                                ],
                                format!(
                                    "Invalid ISO {}",
                                    if instant { "datetime" } else { "date" }
                                ),
                            );
                        }
                    }
                    _ => {}
                }
            }
            v.clone()
        }
        S::Bool => {
            if !matches!(v, Js::Bool(_)) {
                type_issue(issues, p, "boolean", v, false);
                *abort = true;
            }
            v.clone()
        }
        S::Num(min, max) => {
            match v {
                Js::Num(n) if n.is_finite() => {
                    if n.fract() != 0.0 {
                        issue(
                            issues,
                            p,
                            &[
                                ("expected", "int".into()),
                                ("format", "safeint".into()),
                                ("code", "invalid_type".into()),
                            ],
                            "Invalid input: expected int, received number".into(),
                        );
                        *abort = true;
                    } else {
                        for (limit, small) in [(*min, true), (*max, false)] {
                            if if small {
                                *n < (limit as f64)
                            } else {
                                *n > (limit as f64)
                            } {
                                issue(
                                    issues,
                                    p,
                                    &[
                                        ("origin", "number".into()),
                                        (
                                            "code",
                                            if small { "too_small" } else { "too_big" }.into(),
                                        ),
                                        (
                                            if small { "minimum" } else { "maximum" },
                                            Js::Num(limit as f64),
                                        ),
                                        ("inclusive", true.into()),
                                    ],
                                    format!(
                                        "Too {}: expected number to be {}{limit}",
                                        if small { "small" } else { "big" },
                                        if small { ">=" } else { "<=" }
                                    ),
                                );
                            }
                        }
                    }
                }
                _ => {
                    type_issue(issues, p, "number", v, false);
                    *abort = true;
                }
            }
            v.clone()
        }
        S::Enum(vs) => {
            if !vs.iter().any(|s| v.is(s)) {
                let message = if vs.len() == 1 {
                    format!("Invalid input: expected {}", quote(&text(vs[0])))
                } else {
                    format!(
                        "Invalid option: expected one of {}",
                        vs.iter()
                            .map(|s| quote(&text(s)))
                            .collect::<Vec<_>>()
                            .join("|")
                    )
                };
                issue(
                    issues,
                    p,
                    &[("code", "invalid_value".into()), ("values", strings(vs))],
                    message,
                );
                *abort = true;
            }
            v.clone()
        }
        S::Arr(s, min, max) => {
            if !matches!(v, Js::Arr(_)) {
                type_issue(issues, p, "array", v, false);
                *abort = true;
                return v.clone();
            }
            let mut out = vec![];
            for (i, x) in v.array().iter().enumerate() {
                let mut p = p.to_vec();
                p.push(i.into());
                out.push(parse(s, x, &p, issues, abort));
            }
            if v.array().len() < *min {
                size(issues, p, "array", *min, true);
            }
            if v.array().len() > *max {
                size(issues, p, "array", *max, false);
            }
            Js::Arr(out)
        }
        S::Obj(shape) => {
            if !matches!(
                v,
                Js::Obj(_) | Js::NullPrototype(_) | Js::Accessor(_) | Js::SymbolKeys(_)
            ) {
                type_issue(issues, p, "object", v, false);
                *abort = true;
                return v.clone();
            }
            let mut out = Js::Obj(vec![]);
            for (k, s) in shape {
                let mut cp = p.to_vec();
                cp.push((*k).into());
                let value = parse(s, v.get(k), &cp, issues, abort);
                if !matches!(value, Js::Undefined) || v.has(k) {
                    out.set(k, value);
                }
            }
            let extras: Vec<_> = enumerate(v.props())
                .iter()
                .filter(|(k, _)| !shape.iter().any(|(name, _)| k == &text(name)))
                .map(|(k, _)| Js::Str(k.clone()))
                .collect();
            if !extras.is_empty() {
                issue(
                    issues,
                    p,
                    &[
                        ("code", "unrecognized_keys".into()),
                        ("keys", Js::Arr(extras.clone())),
                    ],
                    format!(
                        "Unrecognized key{}: {}",
                        if extras.len() > 1 { "s" } else { "" },
                        extras
                            .iter()
                            .map(|s| quote(s.units()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
                *abort = true;
            }
            out
        }
        S::Union(key, options) => {
            if !matches!(
                v,
                Js::Obj(_) | Js::NullPrototype(_) | Js::Accessor(_) | Js::SymbolKeys(_)
            ) {
                type_issue(issues, p, "object", v, true);
                *abort = true;
                return v.clone();
            }
            if let Some((_, s)) = options.iter().find(|(k, _)| v.get(key).is(k)) {
                parse(s, v, p, issues, abort)
            } else {
                let mut cp = p.to_vec();
                cp.push((*key).into());
                issue(
                    issues,
                    &cp,
                    &[
                        ("code", "invalid_union".into()),
                        ("errors", Js::Arr(vec![])),
                        ("note", "No matching discriminator".into()),
                        ("discriminator", (*key).into()),
                        (
                            "options",
                            strings(&options.iter().map(|(key, _)| *key).collect::<Vec<_>>()),
                        ),
                    ],
                    format!(
                        "Invalid discriminator value. Expected {}",
                        options
                            .iter()
                            .map(|(key, _)| format!("'{key}'"))
                            .collect::<Vec<_>>()
                            .join(" | ")
                    ),
                );
                *abort = true;
                v.clone()
            }
        }
    }
}
fn refinement(name: &str, v: &Js, p: &[Js], issues: &mut Vec<Js>) {
    match name {
        "context" => {
            let a = v.get("selectedFields").array();
            if unique(a).len() != a.len() {
                custom(issues, p, "duplicate_selected_field", true);
            }
            let a: Vec<_> = v
                .get("facts")
                .array()
                .iter()
                .map(|f| f.get("ref").clone())
                .collect();
            if unique(&a).len() != a.len() {
                custom(issues, p, "duplicate_fact_ref", true);
            }
        }
        "fact"
            if !matches!(v.get("explicitValue"), Js::Undefined)
                && [
                    "attendance",
                    "dailyTestAssessment",
                    "previousHomeworkAssessment",
                ]
                .iter()
                .any(|f| v.get("field").is(f))
                && !decision::valid_value(&v.get("field").code_units(), v.get("explicitValue")) =>
        {
            custom(issues, p, "explicit_source_option_required", true);
        }
        "period" if v.get("periodStart").units() > v.get("periodEnd").units() => {
            custom(issues, p, "ordered_reporting_period_required", false);
        }
        "deadline" => {
            let s = v.get("date").code_units();
            if s.len() == 10 {
                let y = s[..4].parse::<u32>().unwrap_or(0);
                let m = s[5..7].parse::<u32>().unwrap_or(0);
                let d = s[8..].parse::<u32>().unwrap_or(0);
                let re = regex(
                    r"(?:(\d{4})[-./](\d{1,2})[-./](\d{1,2})|(\d{1,2})\s*월\s*(\d{1,2})\s*일)",
                );
                if re.captures_iter(&v.get("rendering").code_units()).any(|c| {
                    if c.get(1).is_some() {
                        c[1].parse() != Ok(y) || c[2].parse() != Ok(m) || c[3].parse() != Ok(d)
                    } else {
                        c[4].parse() != Ok(m) || c[5].parse() != Ok(d)
                    }
                }) {
                    custom(issues, p, "deadline_rendering_must_preserve_date", false);
                }
            }
        }
        _ => {}
    }
}
pub fn validate(name: &str, v: &Js) -> Result<Js> {
    let s = named(name).ok_or_else(|| Fault::admission("unknown_schema"))?;
    let mut issues = vec![];
    let result = parse(&s, v, &[], &mut issues, &mut false);
    if issues.is_empty() {
        Ok(result)
    } else {
        Err(Fault {
            name: "ZodError".into(),
            message: pretty(&Js::Arr(issues.clone()), 0),
            issues: Some(issues),
            ..Fault::error("")
        })
    }
}
