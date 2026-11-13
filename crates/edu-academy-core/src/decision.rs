//! Pure DailyRecord intake/review and disposable in-memory action composition.
//! Fakes supplied by the candidate driver are not authorization or policy authority.
use crate::{js::*, knowledge, templates, wire::TestPorts};
pub const FIELDS: [&str; 6] = [
    "progress",
    "homework",
    "lessonMemo",
    "attendance",
    "dailyTestAssessment",
    "previousHomeworkAssessment",
];
fn narrative(f: &str) -> bool {
    matches!(f, "progress" | "homework" | "lessonMemo")
}
fn unknown_assessment(f: &str, v: &Js) -> bool {
    matches!(f, "dailyTestAssessment" | "previousHomeworkAssessment") && v.is("-1")
}
pub fn valid_value(f: &str, v: &Js) -> bool {
    if !v.string()
        || v.units()
            .iter()
            .any(|u| matches!(*u,0..=8|11|12|14..=31|127))
    {
        return false;
    }
    match f {
        "attendance" => ["Y", "N", "L", " ", ""].iter().any(|s| v.is(s)),
        "dailyTestAssessment" => ["-1", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]
            .iter()
            .any(|s| v.is(s)),
        "previousHomeworkAssessment" => {
            ["-1", "0", "1", "2", "3", "4", "5"].iter().any(|s| v.is(s))
        }
        _ => crate::leaf::journal_wire_width(v.units()).expect("bounded text") <= 400,
    }
}
fn ordered(v: &Js, keys: &[&str]) -> Js {
    Js::Obj(
        keys.iter()
            .filter(|k| v.has(k))
            .map(|k| (text(k), v.get(k).clone()))
            .collect(),
    )
}
fn normalize_context(v: &Js) -> Js {
    let mut v = ordered(
        v,
        &[
            "schemaVersion",
            "purpose",
            "currentRequestRef",
            "scope",
            "selectedFields",
            "memoAnswer",
            "facts",
        ],
    );
    v.set(
        "scope",
        ordered(
            v.get("scope"),
            &["sourceNamespace", "teacherId", "studentId", "lessonDate"],
        ),
    );
    v.set(
        "memoAnswer",
        ordered(v.get("memoAnswer"), &["kind", "requestRef", "text"]),
    );
    let facts = v
        .get("facts")
        .array()
        .iter()
        .map(|f| {
            let mut f = ordered(
                f,
                &[
                    "ref",
                    "field",
                    "text",
                    "origin",
                    "explicitValue",
                    "requiredFragments",
                    "questionCounts",
                    "deadlines",
                    "stageClaims",
                ],
            );
            if !f.has("origin") {
                let mut pairs = f.props().to_vec();
                pairs.insert(3, (text("origin"), "user_supplied".into()));
                f = Js::Obj(pairs);
            }
            for (key, keys) in [
                ("questionCounts", &["count", "rendering"][..]),
                ("deadlines", &["date", "rendering"][..]),
                (
                    "stageClaims",
                    &["activity", "stage", "status", "rendering"][..],
                ),
            ] {
                f.set(
                    key,
                    Js::Arr(
                        f.get(key)
                            .array()
                            .iter()
                            .map(|x| ordered(x, keys))
                            .collect(),
                    ),
                );
            }
            f
        })
        .collect();
    v.set("facts", Js::Arr(facts));
    v
}
fn normalize_proposals(p: &Js) -> Js {
    Js::Arr(
        p.array()
            .iter()
            .map(|p| {
                let mut p = ordered(
                    p,
                    &[
                        "field",
                        "value",
                        "factRefs",
                        "decision",
                        "learningEntries",
                        "teacherReported",
                    ],
                );
                p.set(
                    "decision",
                    ordered(p.get("decision"), &["kind", "requestRef"]),
                );
                if p.has("learningEntries") {
                    p.set(
                        "learningEntries",
                        Js::Arr(
                            p.get("learningEntries")
                                .array()
                                .iter()
                                .map(|e| {
                                    ordered(
                                        e,
                                        &[
                                            "bookRef",
                                            "unitRef",
                                            "scope",
                                            "sectionRefs",
                                            "activity",
                                            "deadline",
                                        ],
                                    )
                                })
                                .collect(),
                        ),
                    );
                }
                if p.has("teacherReported") {
                    let mut t = ordered(
                        p.get("teacherReported"),
                        &["preserveBefore", "mappedText", "lines"],
                    );
                    t.set(
                        "lines",
                        Js::Arr(
                            t.get("lines")
                                .array()
                                .iter()
                                .map(|l| ordered(l, &["factRef", "text"]))
                                .collect(),
                        ),
                    );
                    p.set("teacherReported", t);
                }
                p
            })
            .collect(),
    )
}
fn normalized_target(t: &Js) -> Js {
    if t.absent() {
        Js::Null
    } else {
        ordered(
            t,
            &[
                "sourceNamespace",
                "teacherId",
                "studentId",
                "lessonDate",
                "courseId",
                "membershipId",
                "recordId",
            ],
        )
    }
}
fn normalize_observation(v: &Js) -> Js {
    let mut v = ordered(
        v,
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
    );
    v.set(
        "scope",
        ordered(
            v.get("scope"),
            &["sourceNamespace", "teacherId", "studentId", "lessonDate"],
        ),
    );
    v.set("target", normalized_target(v.get("target")));
    v.set("previousTarget", normalized_target(v.get("previousTarget")));
    let fields = Js::Obj(
        FIELDS
            .iter()
            .filter(|f| v.get("fields").has(f))
            .map(|f| {
                (
                    text(f),
                    ordered(
                        v.get("fields").get(f),
                        &["state", "value", "evidenceRefs", "reasonCode"],
                    ),
                )
            })
            .collect(),
    );
    v.set("fields", fields);
    let samples = v
        .get("toneSamples")
        .array()
        .iter()
        .map(|s| {
            let mut s = ordered(s, &["target", "observedAt", "text", "evidenceRefs"]);
            s.set("target", normalized_target(s.get("target")));
            s
        })
        .collect();
    v.set("toneSamples", Js::Arr(samples));
    v
}
fn base(context: &Js) -> Js {
    let w = templates::from_json(
        serde_json::from_str(include_str!("workflow-context.json"))
            .expect("source workflow constant"),
    );
    obj(&[
        ("workflowContext", w),
        ("userContext", context.clone()),
        ("academyWrites", Js::Num(0.0)),
        ("parentSends", Js::Num(0.0)),
        ("automaticRetryAllowed", false.into()),
        ("sourceContentIsUntrustedData", true.into()),
    ])
}
fn extend(mut a: Js, b: Js) -> Js {
    for (k, v) in b.props() {
        if let Js::Obj(p) = &mut a {
            if let Some((_, old)) = p.iter_mut().find(|(key, _)| key == k) {
                *old = v.clone();
            } else {
                p.push((k.clone(), v.clone()));
            }
        }
    }
    a
}
fn before_unknown(reason: &str) -> Js {
    obj(&[("state", "unknown".into()), ("reasonCode", reason.into())])
}
fn error_failure(message: &str, line: usize, column: usize) -> Js {
    obj(&[
        ("category", "upstream_unavailable".into()),
        (
            "code",
            "dailyrecord_source_unqualified_no_automatic_retry".into(),
        ),
        ("evidenceLocator", "academy-mcp:owner-read-boundary".into()),
        ("automaticRetryAllowed", false.into()),
        (
            "origin",
            obj(&[
                ("module", "dailyrecord.ts".into()),
                ("line", line.into()),
                ("column", column.into()),
            ]),
        ),
        ("errorKind", "Error".into()),
        ("errorSha256", sha(&("Error\0".to_owned() + message)).into()),
    ])
}
fn missing_observation_failure(fields: &Js) -> Js {
    // Zod4.4.3 invalid_type object issue at the missing discriminated-union field.
    // Construct the diagnostic from the source schema path, never from a case ID.
    let issues: Vec<serde_json::Value> = FIELDS
        .iter()
        .filter(|f| !fields.has(f))
        .map(|f| {
            let mut p = serde_json::Map::new();
            p.insert("code".into(), "invalid_type".into());
            p.insert("expected".into(), "object".into());
            p.insert("path".into(), serde_json::json!(["fields", f]));
            p.insert(
                "message".into(),
                "Invalid input: expected object, received undefined".into(),
            );
            serde_json::Value::Object(p)
        })
        .collect();
    // serde_json's default map ordering is not Zod insertion order. Pretty-print
    // these fixed issue keys explicitly, in the schema runtime's own order.
    let parts:Vec<_>=issues.iter().map(|v|format!("  {{\n    \"code\": \"invalid_type\",\n    \"expected\": \"object\",\n    \"path\": [\n      \"fields\",\n      {}\n    ],\n    \"message\": \"Invalid input: expected object, received undefined\"\n  }}",quote(&text(v["path"][1].as_str().expect("field name"))))).collect();
    let message = format!("[\n{}\n]", parts.join(",\n"));
    obj(&[
        ("category", "malformed_response".into()),
        ("code", "source_response_malformed".into()),
        ("evidenceLocator", "academy-mcp:response-contract".into()),
        ("automaticRetryAllowed", false.into()),
        ("errorKind", "Error".into()),
        (
            "errorSha256",
            sha(&("ZodError\0".to_owned() + &message)).into(),
        ),
    ])
}
fn qualify(observation: &Js, scope: &Js) -> Option<Js> {
    let t = observation.get("target");
    let p = observation.get("previousTarget");
    if FIELDS.iter().any(|f| !observation.get("fields").has(f)) {
        return Some(missing_observation_failure(observation.get("fields")));
    }
    if observation.get("scope") != scope
        || ordered(
            t,
            &["sourceNamespace", "teacherId", "studentId", "lessonDate"],
        ) != *scope
    {
        return Some(error_failure("source_scope_conflict", 174, 85));
    }
    if !p.absent()
        && ([
            "sourceNamespace",
            "teacherId",
            "studentId",
            "courseId",
            "membershipId",
        ]
        .iter()
        .any(|f| p.get(f) != t.get(f))
            || p.get("recordId") == t.get("recordId")
            || p.get("lessonDate").units() >= t.get("lessonDate").units())
    {
        return Some(error_failure("previous_record_scope_conflict", 176, 220));
    }
    if p.absent()
        && observation
            .get("fields")
            .get("previousHomeworkAssessment")
            .get("state")
            .is("known")
    {
        return Some(error_failure("previous_record_required", 177, 78));
    }
    for f in FIELDS {
        let v = observation.get("fields").get(f);
        if v.get("state").is("known") && !valid_value(f, v.get("value")) {
            return Some(error_failure("source_field_value_unqualified", 178, 130));
        }
    }
    for s in observation.get("toneSamples").array() {
        let st = s.get("target");
        if ["sourceNamespace", "teacherId", "studentId"]
            .iter()
            .any(|f| st.get(f) != t.get(f))
            || st.get("lessonDate").units() >= t.get("lessonDate").units()
        {
            return Some(error_failure("tone_sample_scope_conflict", 179, 225));
        }
    }
    None
}
/// One deletion-answer rule for both review paths and their supplied proposals.
pub(crate) fn memo_answer_blocker(c: &Js, proposal: Option<&Js>) -> Option<Js> {
    let clear = c.get("memoAnswer").get("kind").is("clear");
    match proposal {
        None if clear => Some("memo_clear_proposal_required".into()),
        Some(p) if p.get("field").is("lessonMemo") => {
            if clear && c.get("memoAnswer").get("requestRef") != c.get("currentRequestRef") {
                Some("memo_clear_answer_current_request_required".into())
            } else if p.get("decision").get("kind").is("clear")
                && c.get("memoAnswer").get("kind").is("content")
            {
                Some("memo_clear_answer_required".into())
            } else if clear && !p.get("decision").get("kind").is("clear") {
                Some("memo_clear_answer_requires_clear_decision".into())
            } else {
                None
            }
        }
        _ => None,
    }
}
fn proposal_blockers(p: &Js, c: &Js) -> Vec<Js> {
    let mut b = vec![];
    let field = p.get("field").code_units();
    let value = p.get("value");
    let facts = c.get("facts").array();
    if !c.get("selectedFields").array().contains(p.get("field")) {
        b.push("field_not_selected".into());
    }
    if p.get("decision").get("requestRef") != c.get("currentRequestRef") {
        b.push("current_request_ref_required".into());
    }
    if !valid_value(&field, value) {
        b.push("invalid_explicit_field_value".into());
    }
    if p.get("factRefs").array().iter().any(|r| {
        !facts
            .iter()
            .any(|f| f.get("ref") == r && f.get("field") == p.get("field"))
    }) {
        b.push("field_fact_reference_required".into());
    }
    if !narrative(&field) {
        let explicit: Vec<_> = facts
            .iter()
            .filter(|f| {
                p.get("factRefs").array().contains(f.get("ref"))
                    && f.get("field") == p.get("field")
                    && f.get("origin").is("user_supplied")
                    && f.has("explicitValue")
            })
            .collect();
        if !explicit.iter().any(|f| f.get("explicitValue") == value) {
            b.push("explicit_user_source_option_fact_required".into());
        }
        if unique(
            &explicit
                .iter()
                .map(|f| f.get("explicitValue").clone())
                .collect::<Vec<_>>(),
        )
        .len()
            > 1
        {
            b.push("conflicting_explicit_user_source_options".into());
        }
    }
    if unique(p.get("factRefs").array()).len() != p.get("factRefs").array().len() {
        b.push("duplicate_fact_ref".into());
    }
    if p.get("decision").get("kind").is("clear") && !value.is("") {
        b.push("clear_requires_empty_value".into());
    }
    if value.is("") && !p.get("decision").get("kind").is("clear") && narrative(&field) {
        b.push("explicit_clear_decision_required".into());
    }
    if field == "lessonMemo" && c.get("memoAnswer").get("kind").is("no_addition") {
        b.push("no_addition_preserves_existing_memo".into());
    }
    if field == "lessonMemo" && c.get("memoAnswer").get("kind").is("not_supplied") {
        b.push("today_memo_interview_required".into());
    }
    b.extend(memo_answer_blocker(c, Some(p)));
    b
}
fn hold_actions(scope: &Js, field: &Js, b: &[Js], entries: &Js) -> Js {
    let table = templates::from_json(
        serde_json::from_str(include_str!("hold-actions.json")).expect("source action guidance"),
    );
    Js::Arr(
        b.iter()
            .map(|code| {
                let s = code.code_units();
                let prefix = s.split(':').next().unwrap_or("");
                let hit = table
                    .array()
                    .iter()
                    .find(|r| regex(&r.get("pattern").code_units()).is_match(prefix));
                let action = hit.map_or("resolve_blocker".into(), |r| r.get("action").clone());
                let correction =
                    if action.is("teacher_interview") || action.is("owner_catalog_binding") {
                        obj(&[
                            ("scope", scope.clone()),
                            ("field", field.clone()),
                            (
                                "learningEntries",
                                if entries.absent() {
                                    Js::Null
                                } else {
                                    entries.clone()
                                },
                            ),
                            (
                                "appliesTo",
                                "this_student_edition_unit_activity_only".into(),
                            ),
                        ])
                    } else {
                        Js::Null
                    };
                obj(&[
                    ("field", field.clone()),
                    ("code", code.clone()),
                    ("action", action),
                    (
                        "guidance",
                        hit.map_or(Js::Null, |r| r.get("guidance").clone()),
                    ),
                    ("correctionScope", correction),
                ])
            })
            .collect(),
    )
}
fn check_lines(v: &Js) -> bool {
    test(r"^- [^\r\n]+(?:\n- [^\r\n]+)*$", v)
}
// Retain source comparisons: NaN is false for both, unlike negated contains.
#[allow(clippy::manual_range_contains)]
fn stale(v: &Js, now: f64) -> bool {
    crate::instant::epoch_ms(v, crate::instant::Grammar::DailyRecord).is_some_and(|observed| {
        let age = now - observed as f64;
        age < 0.0 || age > 300000.0
    })
}

fn counts(v: &Js) -> Vec<u64> {
    let s = v.code_units();
    let r = regex(r"([0-9]{1,6})\s*문항");
    r.captures_iter(&s)
        .filter(|m| {
            m.get(1)
                .is_some_and(|n| n.start() == 0 || !s.as_bytes()[n.start() - 1].is_ascii_digit())
        })
        .filter_map(|m| m[1].parse().ok())
        .collect()
}
fn count_render(c: &Js) -> bool {
    let n = match c.get("count") {
        Js::Num(n) => *n as u64,
        _ => return false,
    };
    let s = c.get("rendering").code_units();
    let r = regex(&format!(r"{n}\s*문항"));
    r.find_iter(&s).any(|m| {
        (m.start() == 0 || !s.as_bytes()[m.start() - 1].is_ascii_digit())
            && (m.end() == s.len() || !s.as_bytes()[m.end()].is_ascii_digit())
    })
}
fn deadlines(v: &Js) -> Vec<String> {
    let s = v.code_units();
    regex(r"(?:([0-9]{4})[-./]([0-9]{1,2})[-./]([0-9]{1,2})|([0-9]{1,2})\s*월\s*([0-9]{1,2})\s*일)")
        .captures_iter(&s)
        .map(|m| {
            if m.get(1).is_some() {
                format!(
                    "{}-{}-{}",
                    &m[1],
                    m[2].parse::<u32>().unwrap_or(0),
                    m[3].parse::<u32>().unwrap_or(0)
                )
            } else {
                format!(
                    "{}-{}",
                    m[4].parse::<u32>().unwrap_or(0),
                    m[5].parse::<u32>().unwrap_or(0)
                )
            }
        })
        .collect()
}
fn semantic_fact_blockers(p: &Js, facts: &[&Js], b: &mut Vec<Js>) -> Vec<Js> {
    let mut constraints = vec![];
    for f in facts {
        constraints.extend(f.get("requiredFragments").array().iter().cloned());
        for k in ["questionCounts", "deadlines", "stageClaims"] {
            constraints.extend(f.get(k).array().iter().map(|x| x.get("rendering").clone()));
        }
    }
    if facts.iter().any(|f| {
        f.get("questionCounts")
            .array()
            .iter()
            .any(|c| !count_render(c))
    }) {
        b.push("count_rendering_must_preserve_count".into());
    }
    let value = p.get("value");
    let missing: Vec<_> = constraints
        .into_iter()
        .filter(|f| !includes(value.units(), f.units()))
        .collect();
    if !missing.is_empty() {
        b.push("supplied_fact_fragment_missing".into());
    }
    if narrative(&p.get("field").code_units()) {
        let fact_text = Js::Str(join(
            &facts
                .iter()
                .map(|f| f.get("text").units().to_vec())
                .collect::<Vec<_>>(),
            "\n",
        ));
        let mut supported = counts(&fact_text);
        let mut dates = deadlines(&fact_text);
        let claims: Vec<_> = facts
            .iter()
            .flat_map(|f| f.get("stageClaims").array())
            .collect();
        for f in facts {
            for c in f.get("questionCounts").array() {
                if count_render(c)
                    && let Js::Num(n) = c.get("count")
                {
                    supported.push(*n as u64);
                }
            }
            for d in f.get("deadlines").array() {
                let s = d.get("date").code_units();
                dates.push(format!(
                    "{}-{}-{}",
                    &s[..4],
                    s[5..7].parse::<u32>().unwrap_or(0),
                    s[8..10].parse::<u32>().unwrap_or(0)
                ));
                dates.push(format!(
                    "{}-{}",
                    s[5..7].parse::<u32>().unwrap_or(0),
                    s[8..10].parse::<u32>().unwrap_or(0)
                ));
                dates.extend(deadlines(d.get("rendering")));
            }
        }
        if counts(value).iter().any(|c| !supported.contains(c)) {
            b.push("unsupported_question_count".into());
        }
        if deadlines(value).iter().any(|d| !dates.contains(d)) {
            b.push("unsupported_deadline".into());
        }
        if test("완료", value)
            && !facts.iter().any(|f| {
                test("완료", f.get("text"))
                    || f.get("stageClaims")
                        .array()
                        .iter()
                        .any(|s| s.get("status").is("complete"))
            })
        {
            b.push("unsupported_completion_claim".into());
        }
        for (stage, pattern) in [
            ("solving", r"(?:문제\s*)?풀이\s*(?:까지\s*)?완료"),
            ("grading", r"채점(?:·검사)?\s*(?:까지\s*)?완료"),
            (
                "correction",
                r"(?:오답\s*(?:수정|정리|처리)|재풀이)\s*(?:까지\s*)?완료",
            ),
            ("teacher_review", r"(?:교사\s*확인|검토)\s*(?:까지\s*)?완료"),
        ] {
            if test(pattern, value) {
                let local: Vec<_> = claims.iter().filter(|s| s.get("stage").is(stage)).collect();
                if local.iter().any(|s| !s.get("status").is("complete")) {
                    b.push(format!("stage_completion_mismatch:{stage}").into());
                }
                if !local.iter().any(|s| s.get("status").is("complete"))
                    && !facts.iter().any(|f| test(pattern, f.get("text")))
                {
                    b.push(format!("unsupported_stage_completion_claim:{stage}").into());
                }
            }
        }
    }
    missing
}
/// One finite source-equivalent intake/review composition; no durable workflow.
/// The clock/UUID provider is mandatory and used before constructing hash inputs.
pub fn review(input: &Js, ports: &TestPorts, events: &mut Vec<Js>) -> Result<Js> {
    review_core(input, ports, events, None)
}
pub struct ReviewTrace {
    pub calls: Vec<Js>,
    pub uuids: Vec<Js>,
}
pub fn review_extended(
    input: &Js,
    ports: &TestPorts,
    events: &mut Vec<Js>,
    trace: &mut ReviewTrace,
) -> Result<Js> {
    review_core(input, ports, events, Some(trace))
}
fn review_core(
    input: &Js,
    ports: &TestPorts,
    events: &mut Vec<Js>,
    mut trace: Option<&mut ReviewTrace>,
) -> Result<Js> {
    let intake_input = if input.has("intake") {
        input.get("intake").clone()
    } else {
        obj(&[("context", input.get("context").clone())])
    };
    let intake_input = if trace.is_some() {
        crate::schema::validate("dailyRecordIntakeSchema", &intake_input)?
    } else {
        intake_input
    };
    let context = normalize_context(intake_input.get("context"));
    let now_epoch = if trace.is_some() {
        match input.get("ports").get("nowEpochMs") {
            Js::Num(n) if n.is_finite() => *n,
            _ => return Err(Fault::admission("explicit_test_ports_required")),
        }
    } else {
        crate::instant::epoch_ms(
            &ports.now.as_str().into(),
            crate::instant::Grammar::DailyRecord,
        )
        .unwrap_or(0) as f64
    };
    let supplied = intake_input.get("suppliedValues").array();
    let supplied_fields: Vec<_> = supplied.iter().map(|p| p.get("field").clone()).collect();
    if unique(&supplied_fields).len() != supplied_fields.len() {
        return Err(Fault::error("duplicate_proposal_field"));
    }
    let proposals = normalize_proposals(input.get("proposals"));
    let scope = context.get("scope");
    let request = context.get("currentRequestRef");
    let raw = if let Some(first) = input.get("observations").array().first() {
        first
    } else {
        input.get("observation")
    };
    events.push(obj(&[
        ("event", "recorded_read".into()),
        ("scope", scope.clone()),
        ("requestRef", request.clone()),
    ]));
    let observation = if raw.absent() {
        Js::Null
    } else {
        normalize_observation(raw)
    };
    let failure = if observation.absent() {
        None
    } else {
        if trace.is_some() {
            match crate::schema::validate("dailyRecordObservationSchema", raw) {
                Err(e) => Some(schema_failure(&e)),
                Ok(_) => qualify(&observation, scope),
            }
        } else {
            qualify(&observation, scope)
        }
    };
    let source = if observation.absent() || failure.is_some() {
        Js::Null
    } else {
        observation
    };
    let source_available = !source.absent();
    let mut intake_blockers = vec![];
    if let Some(f) = &failure {
        intake_blockers.push(f.get("code").clone());
    } else if !source_available {
        intake_blockers.push("source_read_unavailable".into());
    } else if stale(source.get("observedAt"), now_epoch) {
        intake_blockers.push("stale_source".into());
    }
    let safeguard = !matches!(input.get("safeguard"), Js::Bool(false));
    let learning = !matches!(input.get("learning"), Js::Null);
    let mut interviews = vec![];
    let mut intake_fields = vec![];
    for field in FIELDS {
        let current = if source_available {
            source.get("fields").get(field).clone()
        } else {
            before_unknown("source_not_read")
        };
        let existing = current.get("state").is("known")
            && !unknown_assessment(field, current.get("value"))
            && (!matches!(field, "attendance" | "progress" | "homework")
                || !trim(current.get("value").units()).is_empty());
        let explicit = supplied.iter().find(|p| p.get("field").is(field));
        let supplied = explicit.is_some_and(|p| proposal_blockers(p, &context).is_empty());
        let facts = context
            .get("facts")
            .array()
            .iter()
            .any(|f| f.get("field").is(field));
        if field == "lessonMemo" {
            if context.get("memoAnswer").get("kind").is("not_supplied") {
                interviews.push(obj(&[
                    ("field", field.into()),
                    ("reasonCode", "today_student_specific_memo_required".into()),
                ]));
            }
        } else if !existing && !supplied && (!matches!(field, "progress" | "homework") || !facts) {
            interviews.push(obj(&[
                ("field", field.into()),
                (
                    "reasonCode",
                    "explicit_value_or_qualified_existing_value_required".into(),
                ),
            ]));
        }
        intake_fields.push(obj(&[
            ("field", field.into()),
            ("before", current),
            (
                "selected",
                context
                    .get("selectedFields")
                    .array()
                    .contains(&field.into())
                    .into(),
            ),
            (
                "disposition",
                if existing {
                    "preserve_existing"
                } else if supplied {
                    "explicit_user_value"
                } else if facts && matches!(field, "progress" | "homework") {
                    "user_facts_available"
                } else {
                    "missing"
                }
                .into(),
            ),
            (
                "target",
                if !source_available {
                    Js::Null
                } else if field == "previousHomeworkAssessment" {
                    source.get("previousTarget").clone()
                } else {
                    source.get("target").clone()
                },
            ),
        ]));
    }
    for p in supplied {
        for b in proposal_blockers(p, &context) {
            intake_blockers
                .push(format!("{}:{}", p.get("field").code_units(), b.code_units()).into());
        }
    }
    let handle = if source_available && intake_blockers.is_empty() {
        if let Some(t) = trace.as_deref_mut() {
            t.uuids.push(ports.uuid.clone().into());
        }
        format!("dailyrecord-observation:{}", ports.uuid).into()
    } else {
        Js::Null
    };
    let summary = if source_available {
        ordered(
            &source,
            &[
                "observedAt",
                "sourceRevision",
                "wireDigest",
                "transmissionState",
            ],
        )
    } else {
        Js::Null
    };
    let samples = if source_available {
        source
            .get("toneSamples")
            .array()
            .iter()
            .filter(|s| safeguard && check_lines(s.get("text")))
            .cloned()
            .collect()
    } else {
        vec![]
    };
    let intake = extend(
        base(&context),
        obj(&[
            (
                "status",
                if !intake_blockers.is_empty() {
                    "held"
                } else if !interviews.is_empty() {
                    "interview_required"
                } else {
                    "ready_for_review"
                }
                .into(),
            ),
            ("observationHandle", handle.clone()),
            ("observationPersistence", "process_local".into()),
            ("attribution", Js::Null),
            ("fields", Js::Arr(intake_fields)),
            ("interviews", Js::Arr(interviews)),
            ("blockers", Js::Arr(intake_blockers)),
            ("source", summary),
            ("toneSamples", Js::Arr(samples)),
            (
                "toneSelection",
                "hyphen_lines_only_other_formats_ignored".into(),
            ),
            (
                "sourceAvailability",
                if source_available {
                    "observed"
                } else {
                    "unavailable"
                }
                .into(),
            ),
            ("failure", failure.unwrap_or(Js::Null)),
            ("execution", "not_supported".into()),
        ]),
    );
    let mut review_input = if input.has("review") {
        input.get("review").clone()
    } else {
        let mut r = obj(&[
            ("context", input.get("context").clone()),
            ("proposals", input.get("proposals").clone()),
        ]);
        if !handle.absent() {
            r.set("observationHandle", handle.clone());
        }
        r
    };
    if let Some(t) = trace.as_deref_mut() {
        t.calls.push("daily.DailyRecordService.review".into());
        review_input = crate::schema::validate("dailyRecordReviewSchema", &review_input)?;
    }
    let context = if trace.is_some() {
        normalize_context(review_input.get("context"))
    } else {
        context
    };
    let proposals = if trace.is_some() {
        normalize_proposals(review_input.get("proposals"))
    } else {
        proposals
    };
    let scope = context.get("scope");
    let request = context.get("currentRequestRef");
    if trace.is_some()
        && review_input.get("observationHandle").truthy()
        && (handle.absent()
            || review_input.get("observationHandle") != &handle
            || source.get("scope") != scope
            || intake_input.get("context").get("currentRequestRef") != request)
    {
        return Err(Fault::error("exact_backend_observation_handle_required"));
    }
    if unique(
        &proposals
            .array()
            .iter()
            .map(|p| p.get("field").clone())
            .collect::<Vec<_>>(),
    )
    .len()
        != proposals.array().len()
    {
        return Err(Fault::error("duplicate_proposal_field"));
    }
    let handle = if trace.is_some() {
        if review_input.get("observationHandle").absent() {
            Js::Null
        } else {
            review_input.get("observationHandle").clone()
        }
    } else {
        handle
    };
    let held = !handle.absent();
    let memo_blocker = proposals
        .array()
        .iter()
        .all(|p| !p.get("field").is("lessonMemo"))
        .then(|| memo_answer_blocker(&context, None))
        .flatten();
    let mut global = if held {
        vec![]
    } else {
        vec!["qualified_before_source_unavailable".into()]
    };
    global.extend(memo_blocker.clone());
    if trace.is_some() && review_input.get("cismObservationHandle").truthy() {
        global.push("exact_fresh_cism_observation_handle_required".into());
    }
    for fact in context.get("facts").array() {
        if fact.get("origin").is("cism_section") {
            global.push("cism_fact_not_bound_to_exact_observed_section".into());
        }
    }
    if held && stale(source.get("observedAt"), now_epoch) {
        global.push("stale_source".into());
    }
    let fresh = held && global.is_empty();
    if fresh {
        events.push(obj(&[
            ("event", "recorded_read".into()),
            ("scope", scope.clone()),
            ("requestRef", request.clone()),
        ]));
    }
    let mut fresh_source = Js::Null;
    if fresh {
        let raw = input.get("observations").array().get(1).unwrap_or(raw);
        if raw.absent() {
            global.push("source_read_unavailable".into());
        } else if trace.is_some() {
            match crate::schema::validate("dailyRecordObservationSchema", raw) {
                Err(_) => global.push("source_response_malformed".into()),
                Ok(_) => {
                    let observed = normalize_observation(raw);
                    if let Some(failure) = qualify(&observed, scope) {
                        global.push(failure.get("code").clone());
                    } else {
                        if stale(observed.get("observedAt"), now_epoch) {
                            global.push("stale_source".into());
                        }
                        let field_values = |o: &Js| {
                            Js::Obj(
                                FIELDS
                                    .iter()
                                    .map(|f| {
                                        (
                                            text(f),
                                            ordered(
                                                o.get("fields").get(f),
                                                if o.get("fields").get(f).get("state").is("known") {
                                                    &["state", "value"]
                                                } else {
                                                    &["state", "reasonCode"]
                                                },
                                            ),
                                        )
                                    })
                                    .collect(),
                            )
                        };
                        if [
                            "target",
                            "previousTarget",
                            "wireDigest",
                            "sourceRevision",
                            "transmissionState",
                        ]
                        .iter()
                        .any(|k| observed.get(k) != source.get(k))
                            || field_values(&observed) != field_values(&source)
                        {
                            global.push("source_before_or_contract_drift".into());
                        }
                        fresh_source = observed;
                    }
                }
            }
        } else {
            fresh_source = source.clone();
        }
    }
    let mut rows = vec![];
    for p in proposals.array() {
        let field = p.get("field").code_units();
        let value = p.get("value");
        let before = if held {
            source.get("fields").get(&field).clone()
        } else {
            before_unknown("qualified_before_source_unavailable")
        };
        let target = if !held {
            Js::Null
        } else if field == "previousHomeworkAssessment" {
            source.get("previousTarget").clone()
        } else {
            source.get("target").clone()
        };
        let mut b = global.clone();
        b.extend(proposal_blockers(p, &context));
        if !before.get("state").is("known") {
            b.push("unknown_before".into());
        }
        if target.absent() {
            b.push(
                if field == "previousHomeworkAssessment" {
                    "independent_previous_record_required"
                } else {
                    "qualified_current_record_required"
                }
                .into(),
            );
        }
        let changed = !before.get("state").is("known") || before.get("value") != value;
        if changed
            && before.get("state").is("known")
            && !before.get("value").is("")
            && !unknown_assessment(&field, before.get("value"))
            && p.get("decision").get("kind").is("set")
        {
            b.push("existing_value_requires_explicit_replace".into());
        }
        let facts: Vec<_> = context
            .get("facts")
            .array()
            .iter()
            .filter(|f| f.get("field") == p.get("field"))
            .collect();
        let missing = semantic_fact_blockers(p, &facts, &mut b);
        if changed && !value.is("") && narrative(&field) {
            if safeguard && !check_lines(value) {
                b.push("parent_hyphen_lines_required".into());
            }
            if matches!(field.as_str(), "progress" | "homework") {
                if !learning {
                    b.push("canonical_learning_policy_not_connected".into());
                } else {
                    let teacher = p.get("teacherReported");
                    if !teacher.absent() {
                        if matches!(input.get("learning").get("scopeQualified"), Js::Bool(false)) {
                            b.push("student_namespace_bridge_unverified_ask_teacher".into());
                        }
                        if field != "progress" {
                            b.push("teacher_reported_progress_only".into());
                        }
                        let preserve = matches!(teacher.get("preserveBefore"), Js::Bool(true));
                        let prefix = if preserve && before.get("state").is("known") {
                            before.get("value").units().to_vec()
                        } else {
                            vec![]
                        };
                        if preserve
                            && (!before.get("state").is("known") || !before.get("value").truthy())
                        {
                            b.push("teacher_reported_before_required".into());
                        }
                        if teacher.get("lines").array().iter().any(|l| {
                            let t = l.get("text").units();
                            !t.starts_with(&text("- "))
                                || t.contains(&10)
                                || !p.get("factRefs").array().contains(l.get("factRef"))
                                || !facts.iter().any(|f| {
                                    f.get("ref") == l.get("factRef")
                                        && f.get("origin").is("user_supplied")
                                        && includes(
                                            f.get("text").units(),
                                            t.get(2..).unwrap_or(&[]),
                                        )
                                })
                        }) {
                            b.push("teacher_reported_exact_fact_required".into());
                        }
                        let mut parts = vec![prefix, teacher.get("mappedText").units().to_vec()];
                        parts.extend(
                            teacher
                                .get("lines")
                                .array()
                                .iter()
                                .map(|l| l.get("text").units().to_vec()),
                        );
                        if join(
                            &parts
                                .into_iter()
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>(),
                            "\n",
                        ) != value.units()
                        {
                            b.push("teacher_reported_render_mismatch".into());
                        }
                        if !teacher.get("mappedText").truthy()
                            && !p.get("learningEntries").array().is_empty()
                        {
                            b.push("teacher_reported_mapping_mismatch".into());
                        }
                    }
                    if teacher.absent() || teacher.get("mappedText").truthy() {
                        let reviewed = if teacher.absent() {
                            value.clone()
                        } else {
                            teacher.get("mappedText").clone()
                        };
                        let entries = if p.get("learningEntries").absent() {
                            Js::Null
                        } else {
                            p.get("learningEntries").clone()
                        };
                        events.push(obj(&[
                            ("event", "fake_learning_review".into()),
                            ("value", reviewed),
                            ("entries", entries),
                        ]));
                        b.extend(
                            input
                                .get("learning")
                                .get("blockers")
                                .array()
                                .iter()
                                .cloned(),
                        );
                    }
                }
            }
            if !safeguard {
                b.push("canonical_parent_safeguard_not_connected".into());
            } else if test(r"교사\s*(?:최종\s*)?완료\s*판정", value) {
                b.push("parent_internal_language_excluded".into());
            }
        }
        let b = unique(&b);
        let entries = if p.get("learningEntries").absent() {
            Js::Null
        } else {
            p.get("learningEntries").clone()
        };
        let holds = hold_actions(scope, p.get("field"), &b, p.get("learningEntries"));
        rows.push(obj(&[
            ("field", p.get("field").clone()),
            ("target", target),
            ("before", before),
            ("after", value.clone()),
            ("factRefs", p.get("factRefs").clone()),
            ("learningEntries", entries),
            ("learningSuggestedRendering", Js::Null),
            ("learningSuggestionMeaning", Js::Null),
            (
                "disposition",
                if !b.is_empty() {
                    "blocked"
                } else if changed {
                    "proposed"
                } else {
                    "noop"
                }
                .into(),
            ),
            ("blockers", Js::Arr(b)),
            ("missingFactFragments", Js::Arr(missing)),
            ("holdActions", holds),
            (
                "semanticFactReview",
                "teacher_review_required_not_proven_by_fragment_checks".into(),
            ),
        ]));
    }
    let patch = Js::Arr(
        rows.iter()
            .filter(|r| r.get("disposition").is("proposed"))
            .map(|r| {
                obj(&[
                    ("field", r.get("field").clone()),
                    ("target", r.get("target").clone()),
                    (
                        "before",
                        if r.get("before").get("state").is("known") {
                            r.get("before").get("value").clone()
                        } else {
                            Js::Null
                        },
                    ),
                    ("after", r.get("after").clone()),
                    ("factRefs", r.get("factRefs").clone()),
                    ("learningEntries", r.get("learningEntries").clone()),
                ])
            })
            .collect(),
    );
    let interviews = if context.get("memoAnswer").get("kind").is("not_supplied") {
        vec![obj(&[
            ("field", "lessonMemo".into()),
            ("reasonCode", "today_student_specific_memo_required".into()),
        ])]
    } else {
        memo_blocker
            .into_iter()
            .map(|code| obj(&[("field", "lessonMemo".into()), ("reasonCode", code)]))
            .collect()
    };
    let input_digest = digest(
        &obj(&[
            ("context", context.clone()),
            ("proposals", proposals),
            (
                "cismObservationHandle",
                if trace.is_some() && review_input.get("cismObservationHandle").truthy() {
                    review_input.get("cismObservationHandle").clone()
                } else {
                    Js::Null
                },
            ),
        ]),
        Family::Admission,
    )?;
    let parent = if safeguard {
        obj(&[
            ("sourceRef", "synthetic-parent-policy".into()),
            ("policySha256", "3".repeat(64).into()),
            ("safeguardSourceSha256", "4".repeat(64).into()),
        ])
    } else {
        Js::Null
    };
    let learning = if learning {
        obj(&[
            ("bundleSha256", "5".repeat(64).into()),
            ("configSha256", "6".repeat(64).into()),
            ("catalogSha256", "7".repeat(64).into()),
        ])
    } else {
        Js::Null
    };
    let body = obj(&[
        ("observationHandle", handle),
        ("inputDigest", input_digest.into()),
        ("scope", scope.clone()),
        ("requestRef", request.clone()),
        ("userContextDigest", digest(&context, Family::Cism)?.into()),
        ("parentSafeguard", parent),
        ("cismSource", Js::Null),
        ("learningPolicy", learning),
        (
            "sourceRevision",
            if held {
                source.get("sourceRevision").clone()
            } else {
                Js::Null
            },
        ),
        (
            "wireDigest",
            if held {
                source.get("wireDigest").clone()
            } else {
                Js::Null
            },
        ),
        (
            "transmissionState",
            if !fresh_source.absent() {
                fresh_source.get("transmissionState").clone()
            } else if held {
                source.get("transmissionState").clone()
            } else {
                "unknown".into()
            },
        ),
        ("rows", Js::Arr(rows.clone())),
        ("patch", patch),
    ]);
    let review_digest = digest(&body, Family::Cism)?;
    let status =
        if rows.iter().any(|r| !r.get("blockers").array().is_empty()) || !interviews.is_empty() {
            "held"
        } else {
            "prepared"
        };
    let review = extend(
        extend(base(&context), body),
        obj(&[
            ("status", status.into()),
            ("interviews", Js::Arr(interviews)),
            ("reviewDigest", review_digest.into()),
            ("reviewRecord", Js::Null),
            ("execution", "not_supported".into()),
            (
                "saveAdmission",
                "separate_trusted_native_owner_required".into(),
            ),
        ]),
    );
    Ok(obj(&[("intake", intake), ("review", review)]))
}

fn literal_key(v: &Js) -> Result<()> {
    if !v.string()
        || (!test(r"^[1-9][0-9]{0,15}$", v)
            || v.code_units().parse::<u64>().unwrap_or(u64::MAX) > 9007199254740991)
    {
        Err(Fault::error("invalid_read_key"))
    } else {
        Ok(())
    }
}
fn http_target(v: &Js) -> Result<()> {
    exact(
        v,
        &[
            "studentName",
            "date",
            "groupId",
            "teacherKey",
            "studentKey",
            "courseKey",
            "recordKey",
            "membershipKey",
        ],
    )?;
    if !v.get("studentName").string()
        || trim(v.get("studentName").units()).is_empty()
        || v.get("studentName").units().len() > 200
    {
        return Err(Fault::error("invalid_read_name"));
    }
    knowledge::calendar_date(v.get("date"))?;
    for k in [
        "groupId",
        "teacherKey",
        "studentKey",
        "courseKey",
        "recordKey",
        "membershipKey",
    ] {
        literal_key(v.get(k))?;
    }
    Ok(())
}
fn validate_action(a: &Js) -> Result<()> {
    exact(
        a,
        &[
            "attemptId",
            "target",
            "field",
            "effectRecordKey",
            "before",
            "after",
            "wireDigest",
        ],
    )?;
    require_text(a.get("attemptId"), "attempt")?;
    if !test(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,80}$", a.get("attemptId")) {
        return Err(Fault::error("journal_http_attempt_rejected"));
    }
    http_target(a.get("target"))?;
    literal_key(a.get("effectRecordKey"))?;
    let f = a.get("field").code_units();
    if !FIELDS.contains(&f.as_str())
        || !a.get("before").string()
        || !a.get("after").string()
        || !test("^[a-f0-9]{64}$", a.get("wireDigest"))
    {
        return Err(Fault::error("journal_http_action_rejected"));
    }
    if narrative(&f)
        && crate::leaf::journal_wire_width(a.get("after").units()).expect("bounded text") > 400
    {
        return Err(Fault::error("journal_http_text_too_long"));
    }
    if !narrative(&f) && !valid_value(&f, a.get("after")) {
        return Err(Fault::error(match f.as_str() {
            "attendance" => "journal_http_attendance_rejected",
            "dailyTestAssessment" => "journal_http_daily_scale_rejected",
            _ => "journal_http_homework_scale_rejected",
        }));
    }
    Ok(())
}
fn verified_state(a: &Js, s: &Js) -> Result<()> {
    exact(
        s,
        &[
            "target",
            "fields",
            "previousRecordKey",
            "previousDate",
            "wireDigest",
        ],
    )?;
    http_target(s.get("target"))?;
    if digest(s.get("target"), Family::Learning)? != digest(a.get("target"), Family::Learning)?
        || s.get("wireDigest") != a.get("wireDigest")
    {
        return Err(Fault::error("journal_http_source_drift"));
    }
    let previous = a.get("field").is("previousHomeworkAssessment");
    let record = if previous {
        s.get("previousRecordKey")
    } else {
        s.get("target").get("recordKey")
    };
    if a.get("effectRecordKey") != record {
        return Err(Fault::error("journal_http_effect_record_conflict"));
    }
    if previous {
        knowledge::calendar_date(s.get("previousDate"))?;
        literal_key(s.get("previousRecordKey"))?;
        if s.get("previousDate").units() >= s.get("target").get("date").units()
            || s.get("previousRecordKey") == s.get("target").get("recordKey")
        {
            return Err(Fault::error("journal_http_previous_occurrence_unverified"));
        }
    }
    if !s.get("fields").get(&a.get("field").code_units()).string() {
        return Err(Fault::error("journal_http_before_unknown"));
    }
    Ok(())
}
/// Disposable local fake only. Real admission, transport errors, locking and
/// uncertain-write reconciliation remain S10/S11; there is no external port here.
pub fn fake_http_action(input: &Js, events: &mut Vec<Js>) -> Result<Js> {
    let a = input.get("action");
    validate_action(a)?;
    let before = input.get("state");
    events.push(obj(&[
        ("event", "fake_read".into()),
        ("target", a.get("target").clone()),
    ]));
    verified_state(a, before)?;
    if before.get("fields").get(&a.get("field").code_units()) != a.get("before") {
        return Err(Fault::error("journal_http_before_drift"));
    }
    let mut after = before.clone();
    let noop = a.get("before") == a.get("after");
    if !noop {
        events.push(obj(&[
            ("event", "fake_approval".into()),
            ("action", a.clone()),
        ]));
        let _digest = digest(a, Family::Learning)?;
        events.push(obj(&[
            ("event", "fake_write".into()),
            ("action", a.clone()),
        ]));
        let mut fields = after.get("fields").clone();
        fields.set(&a.get("field").code_units(), a.get("after").clone());
        after.set("fields", fields);
        events.push(obj(&[
            ("event", "fake_read".into()),
            ("target", a.get("target").clone()),
        ]));
        verified_state(a, &after)?;
    }
    Ok(obj(&[
        ("status", if noop { "noop" } else { "persisted" }.into()),
        ("action", a.clone()),
        ("before", before.clone()),
        ("after", after),
        (
            "transport",
            if noop { "not_invoked" } else { "returned" }.into(),
        ),
        ("parentSends", Js::Num(0.0)),
        ("automaticRetryAllowed", false.into()),
    ]))
}

fn schema_failure(e: &Fault) -> Js {
    obj(&[
        ("category", "malformed_response".into()),
        ("code", "source_response_malformed".into()),
        ("evidenceLocator", "academy-mcp:response-contract".into()),
        ("automaticRetryAllowed", false.into()),
        ("errorKind", "Error".into()),
        (
            "errorSha256",
            sha(&("ZodError\0".to_owned() + &e.message)).into(),
        ),
    ])
}
