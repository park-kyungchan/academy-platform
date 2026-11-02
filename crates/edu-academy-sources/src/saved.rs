//! Complete selected saved query algebra. Loader parity intentionally permits parent symlinks/modes.
use crate::data::*;
use edu_academy_core::{
    instant::{Grammar, epoch_ms},
    js::enumerate,
};
use std::{fs, path::Path};
pub const AUDIT_PATH: &str = "private/student-audit-20261014/audit.json";
pub const LESSON_PATH: &str = "private/lesson-20261014/overlay.json";
pub const PIN_PATH: &str = "tools/academy-mcp/architecture/source-pins.json";
pub const FACETS: [&str; 9] = [
    "books",
    "sections",
    "attempts",
    "assignments",
    "videos",
    "daily_tests",
    "daily_records",
    "teacher_events",
    "daily_record_drafts",
];
pub const MAX_BYTES: usize = 512 * 1024;
const DATE_PATTERN: &str = r"/^(?:(?:\d\d[2468][048]|\d\d[13579][26]|\d\d0[48]|[02468][048]00|[13579][26]00)-02-29|\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\d|30)|(?:02)-(?:0[1-9]|1\d|2[0-8])))$/";
fn issue(fields: &[(&str, Js)], path: &[Js], message: String) -> Js {
    let mut v = obj(fields);
    v.set("path", Js::Arr(path.to_vec()));
    v.set("message", message.into());
    v
}
fn received(v: &Js) -> &str {
    match v {
        Js::Null => "null",
        Js::Undefined => "undefined",
        Js::Bool(_) => "boolean",
        Js::Num(_) => "number",
        Js::Str(_) => "string",
        Js::Arr(_) => "array",
        _ => "object",
    }
}
fn typed(v: &Js, t: &str, p: &[Js], issues: &mut Vec<Js>) -> bool {
    let yes = match t {
        "object" => object(v),
        "string" => v.string(),
        "array" => array(v),
        "number" => number(v).is_some(),
        _ => false,
    };
    if !yes {
        issues.push(issue(
            &[("expected", t.into()), ("code", "invalid_type".into())],
            p,
            format!("Invalid input: expected {t}, received {}", received(v)),
        ));
    }
    yes
}
fn sized(v: &Js, min: usize, max: usize, p: &[Js], issues: &mut Vec<Js>) {
    let (origin, n, unit) = if v.string() {
        ("string", v.units().len(), "characters")
    } else {
        ("array", v.array().len(), "items")
    };
    for (bound, small) in [(min, true), (max, false)] {
        if if small { n < bound } else { n > bound } {
            issues.push(issue(
                &[
                    ("origin", origin.into()),
                    ("code", if small { "too_small" } else { "too_big" }.into()),
                    (if small { "minimum" } else { "maximum" }, bound.into()),
                    ("inclusive", true.into()),
                ],
                p,
                format!(
                    "Too {}: expected {origin} to have {}{bound} {unit}",
                    if small { "small" } else { "big" },
                    if small { ">=" } else { "<=" }
                ),
            ));
        }
    }
}
fn enum_value(v: &Js, values: &[&str], p: &[Js], issues: &mut Vec<Js>) {
    if !values.iter().any(|s| v.is(s)) {
        let message = if values.len() == 1 {
            format!("Invalid input: expected {}", json(&values[0].into()))
        } else {
            format!(
                "Invalid option: expected one of {}",
                values
                    .iter()
                    .map(|s| json(&(*s).into()))
                    .collect::<Vec<_>>()
                    .join("|")
            )
        };
        issues.push(issue(
            &[
                ("code", "invalid_value".into()),
                ("values", strings(values)),
            ],
            p,
            message,
        ));
    }
}
fn strict(v: &Js, keys: &[&str], p: &[Js], issues: &mut Vec<Js>) {
    let extra = v
        .props()
        .iter()
        .filter(|(k, _)| !keys.iter().any(|s| text(s) == *k))
        .map(|(k, _)| Js::Str(k.clone()))
        .collect::<Vec<_>>();
    if !extra.is_empty() {
        let m = format!(
            "Unrecognized key{}: {}",
            if extra.len() > 1 { "s" } else { "" },
            extra.iter().map(json).collect::<Vec<_>>().join(", ")
        );
        issues.push(issue(
            &[
                ("code", "unrecognized_keys".into()),
                ("keys", Js::Arr(extra)),
            ],
            p,
            m,
        ));
    }
}
fn path(parent: &[Js], key: impl Into<Js>) -> Vec<Js> {
    let mut p = parent.to_vec();
    p.push(key.into());
    p
}
/// Schema reconstruction owns member order for the cursor selection digest.
pub fn query_input(v: &Js) -> Result<Js> {
    let mut issues = vec![];
    if !typed(v, "object", &[], &mut issues) {
        return Err(Error::zod(issues));
    }
    let mut result = obj(&[]);
    let mode = if v.has("sourceMode") {
        v.get("sourceMode").clone()
    } else {
        "saved_context".into()
    };
    enum_value(
        &mode,
        &["saved_context"],
        &["sourceMode".into()],
        &mut issues,
    );
    result.set("sourceMode", mode);
    let q = v.get("query");
    let p = vec!["query".into()];
    let mut parsed = obj(&[]);
    if typed(q, "object", &p, &mut issues) {
        let kind = q.get("kind");
        let kinds = [
            "roster",
            "student_summary",
            "student_history",
            "student_records",
            "lesson_context",
        ];
        if !kinds.iter().any(|k| kind.is(k)) {
            issues.push(issue(
                &[
                    ("code", "invalid_union".into()),
                    ("errors", Js::Arr(vec![])),
                    ("note", "No matching discriminator".into()),
                    ("discriminator", "kind".into()),
                    ("options", strings(&kinds)),
                ],
                &path(&p, "kind"),
                format!(
                    "Invalid discriminator value. Expected {}",
                    kinds
                        .iter()
                        .map(|s| format!("'{s}'"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ),
            ));
        } else {
            parsed.set("kind", kind.clone());
            let mut keys = vec!["kind"];
            if !kind.is("roster") {
                keys.push("studentId");
                let id = q.get("studentId");
                if typed(id, "string", &path(&p, "studentId"), &mut issues) {
                    sized(id, 1, 256, &path(&p, "studentId"), &mut issues)
                }
                parsed.set("studentId", id.clone());
            }
            if kind.is("student_history") {
                keys.push("facets");
                if q.has("facets") {
                    let f = q.get("facets");
                    let cp = path(&p, "facets");
                    let start = issues.len();
                    if typed(f, "array", &cp, &mut issues) {
                        sized(f, 1, 9, &cp, &mut issues);
                        for (i, x) in f.array().iter().enumerate() {
                            enum_value(x, &FACETS, &path(&cp, i), &mut issues)
                        }
                        if issues.len() == start
                            && edu_academy_core::js::unique(f.array()).len() != f.array().len()
                        {
                            issues.push(issue(
                                &[("code", "custom".into())],
                                &cp,
                                "duplicate_facet".into(),
                            ));
                        }
                    }
                    parsed.set("facets", f.clone());
                }
            }
            if kind.is("student_records") {
                keys.extend(["facet", "bookEditionId"]);
                enum_value(q.get("facet"), &FACETS, &path(&p, "facet"), &mut issues);
                parsed.set("facet", q.get("facet").clone());
                if q.has("bookEditionId") {
                    let id = q.get("bookEditionId");
                    if typed(id, "string", &path(&p, "bookEditionId"), &mut issues) {
                        sized(id, 1, 256, &path(&p, "bookEditionId"), &mut issues)
                    }
                    parsed.set("bookEditionId", id.clone());
                }
            }
            if kind.is("lesson_context") {
                keys.push("date");
                let d = q.get("date");
                let cp = path(&p, "date");
                if typed(d, "string", &cp, &mut issues) && !valid_date(d) {
                    issues.push(issue(
                        &[
                            ("origin", "string".into()),
                            ("code", "invalid_format".into()),
                            ("format", "date".into()),
                            ("pattern", DATE_PATTERN.into()),
                        ],
                        &cp,
                        "Invalid ISO date".into(),
                    ));
                }
                parsed.set("date", d.clone());
            }
            strict(q, &keys, &p, &mut issues);
        }
    }
    result.set("query", parsed);
    if v.has("cursor") {
        let c = v.get("cursor");
        if typed(c, "string", &["cursor".into()], &mut issues) {
            sized(c, 0, 2048, &["cursor".into()], &mut issues)
        }
        result.set("cursor", c.clone());
    }
    let limit = if v.has("limit") {
        v.get("limit").clone()
    } else {
        Js::Num(10.)
    };
    if typed(&limit, "number", &["limit".into()], &mut issues) {
        let n = number(&limit).unwrap();
        if n.fract() != 0. || !n.is_finite() {
            issues.push(issue(
                &[
                    ("expected", "int".into()),
                    ("format", "safeint".into()),
                    ("code", "invalid_type".into()),
                ],
                &["limit".into()],
                "Invalid input: expected int, received number".into(),
            ));
        } else {
            for (bound, small) in [(1, true), (250, false)] {
                if if small {
                    n < f64::from(bound)
                } else {
                    n > f64::from(bound)
                } {
                    issues.push(issue(
                        &[
                            ("origin", "number".into()),
                            ("code", if small { "too_small" } else { "too_big" }.into()),
                            (
                                if small { "minimum" } else { "maximum" },
                                Js::Num(f64::from(bound)),
                            ),
                            ("inclusive", true.into()),
                        ],
                        &["limit".into()],
                        format!(
                            "Too {}: expected number to be {}{bound}",
                            if small { "small" } else { "big" },
                            if small { ">=" } else { "<=" }
                        ),
                    ));
                }
            }
        }
    }
    result.set("limit", limit);
    strict(
        v,
        &["sourceMode", "query", "cursor", "limit"],
        &[],
        &mut issues,
    );
    if issues.is_empty() {
        Ok(result)
    } else {
        Err(Error::zod(issues))
    }
}
pub fn tool_bytes(v: &Js) -> usize {
    json(&obj(&[
        (
            "content",
            Js::Arr(vec![obj(&[
                ("type", "text".into()),
                ("text", json(v).into()),
            ])]),
        ),
        ("structuredContent", v.clone()),
    ]))
    .len()
}
pub fn record_bytes(v: &Js) -> usize {
    let j = json(v);
    j.len() + json(&j.into()).len() - 2
}
pub fn paginate(
    rows: &[Js],
    query: &Js,
    snapshot: &str,
    limit: f64,
    cursor: Option<&Js>,
    budget: f64,
    dual: bool,
    separator: usize,
) -> Result<Js> {
    if !limit.is_finite() || limit.fract() != 0. || !(1. ..=250.).contains(&limit) {
        return Err(Error::refusal("invalid_page_limit"));
    }
    if budget != f64::INFINITY
        && (!budget.is_finite()
            || budget.fract() != 0.
            || budget < 1.
            || budget > 9007199254740991.)
    {
        return Err(Error::refusal("invalid_byte_budget"));
    }
    let selection = hash(json(query).as_bytes());
    let mut offset = 0usize;
    if let Some(c) = cursor {
        let decoded = (|| {
            let s = string(c);
            if !c.string()
                || s.is_empty()
                || c.units().len() > 2048
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            {
                return None;
            }
            let b = unbase64(&s).ok()?;
            let v = parse(&String::from_utf8_lossy(&b)).ok()?;
            let n = number(v.get("offset"))?;
            if v.props().len() != 4
                || number(v.get("version")) != Some(1.)
                || !sha_valid(v.get("snapshot"))
                || !sha_valid(v.get("selection"))
                || !v.get("snapshot").is(snapshot)
                || !v.get("selection").is(&selection)
                || n < 0.
                || n.fract() != 0.
                || n > rows.len() as f64
            {
                return None;
            }
            Some(n as usize)
        })();
        offset = decoded.ok_or_else(|| Error::refusal("cursor_scope_or_snapshot_conflict"))?;
    }
    let mut items = vec![];
    let mut bytes = 2;
    let mut limited = false;
    for row in rows.iter().skip(offset).take(limit as usize) {
        let cost = if dual {
            record_bytes(row)
        } else {
            json(row).len()
        } + if items.is_empty() { 0 } else { separator };
        if (bytes + cost) as f64 > budget {
            if items.is_empty() {
                return Err(Error::refusal("single_record_exceeds_response_budget"));
            }
            limited = true;
            break;
        }
        bytes += cost;
        items.push(row.clone());
    }
    let next = offset + items.len();
    let token = obj(&[
        ("version", Js::Num(1.)),
        ("snapshot", snapshot.into()),
        ("selection", selection.into()),
        ("offset", next.into()),
    ]);
    let mut page = obj(&[
        ("totalInSavedSelection", rows.len().into()),
        ("returned", items.len().into()),
        ("exhausted", (next >= rows.len()).into()),
        (
            "nextCursor",
            if next < rows.len() {
                base64(json(&token).as_bytes(), true).into()
            } else {
                Js::Null
            },
        ),
    ]);
    if limited {
        page.set("byteLimited", true.into())
    }
    Ok(obj(&[("items", Js::Arr(items)), ("page", page)]))
}
#[derive(Clone)]
pub struct SavedContext {
    pub audit: Js,
    pub lesson: Js,
    pub audit_hash: String,
    pub lesson_hash: String,
    pub snapshot: String,
}
fn fail_unless(ok: bool, code: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(Error::error(code)) }
}
fn instant(v: &Js) -> bool {
    epoch_ms(v, Grammar::DailyRecord).is_some()
}
pub fn parse_audit(v: Js) -> Result<Js> {
    fail_unless(
        v.get("schemaVersion").is("academy.audit.normalized/1.0.0")
            && v.get("collectedAt").string()
            && array(v.get("students"))
            && v.get("students").array().len() == 5,
        "audit_schema_or_roster_invalid",
    )?;
    let mut ids = vec![];
    for s in v.get("students").array() {
        let id = s.get("studentId");
        fail_unless(
            id.string()
                && !id.units().is_empty()
                && id.units().len() <= 256
                && !ids.contains(id)
                && s.get("name").string()
                && array(s.get("books"))
                && array(s.get("sections")),
            "student_scope_invalid",
        )?;
        ids.push(id.clone());
    }
    Ok(v)
}
fn rubric(x: &Js, id: &Js) -> Result<()> {
    fail_unless(
        x.get("schemaVersion").is("academy.student-eval-rubric/1")
            && x.get("studentId") == id
            && number(x.get("version")).is_some_and(|n| n >= 1. && n.fract() == 0.)
            && x.get("rubricId").truthy()
            && array(x.get("bookScope"))
            && !x.get("bookScope").array().is_empty()
            && array(x.get("sources"))
            && !x.get("sources").array().is_empty(),
        "rubric_identity_scope_invalid",
    )?;
    let d = x.get("targets").get("dailyTest");
    fail_unless(
        number(d.get("sessionsPerSubunit")) == Some(2.)
            && number(d.get("totalQuestions")) == Some(8.)
            && json(d.get("targetCorrect")) == "[5,6]"
            && json(d.get("exactPercent")) == "[62.5,75]"
            && json(d.get("aboveTargetCorrect")) == "[7,8]",
        "rubric_goal_invalid",
    )?;
    let p = x.get("persistence");
    fail_unless(
        x.get("teacherFinalIndependent").truthy()
            && x.get("automaticCompletion") == &Js::Bool(false)
            && x.get("otherStudentsApplicable") == &Js::Bool(false)
            && p.get("mode").is("versioned_private_json")
            && p.get("runtimeDatabaseImplemented") == &Js::Bool(false)
            && p.get("lmsWritePerformed") == &Js::Bool(false),
        "rubric_authority_invalid",
    )?;
    fail_unless(
        array(x.get("history"))
            && x.get("history").array().iter().any(|h| {
                h.get("version") == x.get("version")
                    && x.get("sources").array().contains(h.get("sourceMessageId"))
            }),
        "rubric_history_missing",
    )
}
pub fn parse_lesson(v: Js, audit: &Js) -> Result<Js> {
    let date = string(v.get("lessonDate"));
    fail_unless(
        v.get("schemaVersion").is("academy.lesson-overlay/1")
            && date.len() == 10
            && date.bytes().enumerate().all(|(i, c)| {
                if i == 4 || i == 7 {
                    c == b'-'
                } else {
                    c.is_ascii_digit()
                }
            })
            && instant(v.get("preparedAt"))
            && array(v.get("students")),
        "lesson_schema_invalid",
    )?;
    let allowed = audit
        .get("students")
        .array()
        .iter()
        .map(|s| s.get("studentId"))
        .collect::<Vec<_>>();
    let mut seen = vec![];
    for s in v.get("students").array() {
        let id = s.get("studentId");
        fail_unless(
            allowed.contains(&id)
                && !seen.contains(id)
                && array(s.get("teacherEvents"))
                && array(s.get("observations")),
            "lesson_student_scope_invalid",
        )?;
        seen.push(id.clone());
        if s.get("rubric").truthy() {
            rubric(s.get("rubric"), id)?
        }
        let draft = s.get("dailyRecordDraft").get("student").get("studentId");
        fail_unless(
            !draft.truthy() || draft == id,
            "draft_student_scope_invalid",
        )?;
        let mut ids = vec![];
        for e in s.get("teacherEvents").array() {
            fail_unless(
                e.get("studentId") == id
                    && ["teacher_report", "user_supplied_message_context"]
                        .iter()
                        .any(|x| e.get("sourceKind").is(x))
                    && e.get("id").string()
                    && !ids.contains(e.get("id"))
                    && instant(e.get("recordedAt"))
                    && (!e.has("facts") || array(e.get("facts")))
                    && (!e.has("rawLabels")
                        || (array(e.get("rawLabels"))
                            && e.get("rawLabels").array().iter().all(Js::string))),
                "lesson_event_invalid",
            )?;
            ids.push(e.get("id").clone());
        }
        for o in s.get("observations").array() {
            fail_unless(
                [
                    "assessment",
                    "assessment_timing",
                    "dailyTests",
                    "pendingVideo",
                    "pendingVideos",
                    "dailyRecord",
                ]
                .iter()
                .any(|k| o.get("kind").is(k))
                    && instant(o.get("observedAt")),
                "lesson_observation_invalid",
            )?;
        }
    }
    Ok(v)
}
impl SavedContext {
    pub fn from_bytes(a: &[u8], b: &[u8]) -> Result<Self> {
        fn decode(b: &[u8]) -> Result<Js> {
            let s = std::str::from_utf8(b).map_err(|_| {
                let mut e = Error::new(
                    "TypeError",
                    "The encoded data was not valid for encoding utf-8",
                );
                e.properties
                    .set("code", "ERR_ENCODING_INVALID_ENCODED_DATA".into());
                e
            })?;
            parse(s.strip_prefix('\u{feff}').unwrap_or(s))
        }
        let audit = parse_audit(decode(a)?)?;
        let lesson = parse_lesson(decode(b)?, &audit)?;
        let ah = hash(a);
        let bh = hash(b);
        let snapshot = hash(format!("{ah}:{bh}").as_bytes());
        Ok(Self {
            audit,
            lesson,
            audit_hash: ah,
            lesson_hash: bh,
            snapshot,
        })
    }
    fn record(&self, lesson: bool, pointer: &str, class: &str, value: Js) -> Js {
        self.record_units(lesson, text(pointer), class, value)
    }
    fn record_units(&self, lesson: bool, pointer: Text, class: &str, value: Js) -> Js {
        obj(&[
            ("classification", class.into()),
            (
                "source",
                obj(&[
                    ("path", if lesson { LESSON_PATH } else { AUDIT_PATH }.into()),
                    (
                        "sha256",
                        if lesson {
                            self.lesson_hash.clone()
                        } else {
                            self.audit_hash.clone()
                        }
                        .into(),
                    ),
                    ("jsonPointer", Js::Str(pointer)),
                ]),
            ),
            ("value", value),
        ])
    }
    pub fn records(&self, q: &Js) -> Result<Vec<Js>> {
        let students = self.audit.get("students").array();
        if q.get("kind").is("roster") {
            return Ok(students
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    self.record(
                        false,
                        &format!("/students/{i}"),
                        "saved_source_record",
                        obj(&[
                            ("studentId", s.get("studentId").clone()),
                            ("name", s.get("name").clone()),
                            (
                                "membershipBasis",
                                "preserved_authorized_managed_roster".into(),
                            ),
                            ("completeTeacherRosterVerified", false.into()),
                        ]),
                    )
                })
                .collect());
        }
        let i = students
            .iter()
            .position(|s| s.get("studentId") == q.get("studentId"))
            .ok_or_else(|| Error::refusal("student_not_in_preserved_roster"))?;
        let s = &students[i];
        let lp = self
            .lesson
            .get("students")
            .array()
            .iter()
            .position(|s| s.get("studentId") == q.get("studentId"));
        let supplement = lp.map(|i| &self.lesson.get("students").array()[i]);
        let prefix = format!("/students/{i}");
        if q.get("kind").is("student_history") {
            let facets = if q.has("facets") {
                q.get("facets").array().to_vec()
            } else {
                strings(&FACETS).array().to_vec()
            };
            let mut out = vec![];
            for f in facets {
                let mut records = self.records(&obj(&[
                    ("kind", "student_records".into()),
                    ("studentId", q.get("studentId").clone()),
                    ("facet", f.clone()),
                ]))?;
                for r in &mut records {
                    r.set("facet", f.clone())
                }
                out.extend(records)
            }
            return Ok(out);
        }
        if q.get("kind").is("student_summary") {
            return Ok(vec![self.record(false,&prefix,"derived_saved_summary",obj(&[("studentId",s.get("studentId").clone()),("name",s.get("name").clone()),("observedCounts",obj(&[("books",s.get("books").array().len().into()),("sectionRows",s.get("sections").array().len().into()),("historicalAttemptObservations",s.get("attempts").array().len().into()),("historicalVideoObservations",s.get("prepVideosHistorical").array().len().into())])),("availableFacets",strings(&FACETS)),("lessonSupplementPresent",supplement.is_some().into()),("recordedHistoryCoverageClaim",if s.get("fullCurrentHistoryVerified").absent(){Js::Null}else{s.get("fullCurrentHistoryVerified").clone()}),("countsMeaning","Preserved rows and observations; not unique assignments, current roster completeness or completion.".into())]))]);
        }
        let events = |field: &str| -> Vec<Js> {
            supplement.map_or(vec![], |s| {
                s.get(field)
                    .array()
                    .iter()
                    .enumerate()
                    .map(|(row, e)| {
                        self.record(
                            true,
                            &format!("/students/{}/{field}/{row}", lp.unwrap()),
                            if field == "observations" {
                                "source_observation_snapshot"
                            } else if e.get("sourceKind").is("teacher_report") {
                                "teacher_report"
                            } else {
                                "user_supplied_context"
                            },
                            e.clone(),
                        )
                    })
                    .collect()
            })
        };
        if q.get("kind").is("lesson_context") {
            if q.get("date") != self.lesson.get("lessonDate") {
                return Ok(vec![]);
            }
            let mut out = events("teacherEvents");
            out.extend(events("observations"));
            return Ok(out);
        }
        let facet = string(q.get("facet"));
        let edition = q.get("bookEditionId");
        if edition.truthy() && !matches!(facet.as_str(), "books" | "sections") {
            return Err(Error::refusal("book_filter_not_supported_for_facet"));
        }
        if edition.truthy()
            && !s
                .get("books")
                .array()
                .iter()
                .any(|b| b.get("sourceBookEditionId") == edition)
        {
            return Err(Error::refusal(
                "book_edition_not_in_preserved_student_scope",
            ));
        }
        if facet == "teacher_events" {
            return Ok(events("teacherEvents"));
        }
        if facet == "daily_record_drafts" {
            return Ok(supplement
                .filter(|s| s.get("dailyRecordDraft").truthy())
                .map_or(vec![], |s| {
                    vec![self.record(
                        true,
                        &format!("/students/{}/dailyRecordDraft", lp.unwrap()),
                        "unsaved_draft",
                        s.get("dailyRecordDraft").clone(),
                    )]
                }));
        }
        let field = match facet.as_str() {
            "books" => "books",
            "sections" => "sections",
            "attempts" => "attempts",
            "assignments" => "effectiveAssignments",
            "videos" => "prepVideosHistorical",
            "daily_tests" => "DTZT",
            "daily_records" => "liveDailyRecords",
            _ => return Err(Error::refusal("unknown_facet")),
        };
        let source = s.get(field);
        let rows = if facet == "daily_tests" && object(source) {
            enumerate(source.props())
                .iter()
                .map(|(k, v)| (k.to_vec(), v.clone()))
                .collect::<Vec<_>>()
        } else {
            source
                .array()
                .iter()
                .enumerate()
                .map(|(i, v)| (text(&i.to_string()), v.clone()))
                .collect()
        };
        Ok(rows
            .into_iter()
            .filter(|(_, v)| {
                !edition.truthy()
                    || v.get(if facet == "books" {
                        "sourceBookEditionId"
                    } else {
                        "bookEditionId"
                    }) == edition
            })
            .map(|(key, v)| {
                let mut pointer = text(&format!("{prefix}/{field}/"));
                for unit in key {
                    match unit {
                        126 => pointer.extend([126, 48]),
                        47 => pointer.extend([126, 49]),
                        _ => pointer.push(unit),
                    }
                }
                self.record_units(
                    false,
                    pointer,
                    if facet == "daily_records" {
                        "source_observation_snapshot"
                    } else {
                        "saved_source_record"
                    },
                    v,
                )
            })
            .collect())
    }
    pub fn query(&self, v: &Js, now: &str) -> Result<Js> {
        let input = query_input(v)?;
        let q = input.get("query");
        let mut base = obj(&[
            ("schemaVersion", Js::Num(1.)),
            ("sourceMode", "saved_context".into()),
            ("retrievedAt", now.into()),
            (
                "snapshot",
                obj(&[
                    ("sha256", self.snapshot.clone().into()),
                    ("collectedAt", self.audit.get("collectedAt").clone()),
                    ("lessonDate", self.lesson.get("lessonDate").clone()),
                    ("lessonPreparedAt", self.lesson.get("preparedAt").clone()),
                ]),
            ),
            (
                "scope",
                obj(&[
                    ("teacherLabel", "박경찬".into()),
                    ("intendedPopulation", "all_teacher_managed_students".into()),
                    (
                        "preservedStudentCount",
                        self.audit.get("students").array().len().into(),
                    ),
                    ("completeTeacherRosterVerified", false.into()),
                    ("totalTeacherManagedStudents", Js::Null),
                ]),
            ),
            ("coverage", "bounded_preserved_evidence".into()),
            ("currentAcademyServerQueried", false.into()),
            ("absenceProvesNonperformance", false.into()),
            (
                "unknowns",
                strings(&[
                    "The complete teacher-managed roster and complete current history are not established by this preserved subset.",
                ]),
            ),
        ]);
        let attempt = (|| {
            let rows = self.records(q)?;
            if q.get("kind").is("student_history") {
                let mut u = base.get("unknowns").clone();
                u.push("student_history includes only the selected saved facets; dated lesson_context source observations require a separate lesson_context query.".into());
                base.set("unknowns", u);
            }
            let mut envelope = base.clone();
            envelope.set("status", "partial".into());
            envelope.set("records", Js::Arr(vec![]));
            envelope.set(
                "page",
                obj(&[
                    ("totalInSavedSelection", rows.len().into()),
                    ("returned", input.get("limit").clone()),
                    ("exhausted", false.into()),
                    ("nextCursor", "A".repeat(2048).into()),
                    ("byteLimited", true.into()),
                ]),
            );
            let budget = MAX_BYTES as f64 - tool_bytes(&envelope) as f64 - 1024.;
            let paged = paginate(
                &rows,
                q,
                &self.snapshot,
                number(input.get("limit")).unwrap(),
                if input.has("cursor") {
                    Some(input.get("cursor"))
                } else {
                    None
                },
                budget,
                true,
                2,
            )?;
            let mut result = base.clone();
            result.set("status", "partial".into());
            result.set("records", paged.get("items").clone());
            result.set("page", paged.get("page").clone());
            if rows.is_empty() {
                let mut u = result.get("unknowns").clone();
                u.push("No matching preserved records; activity, historical existence and server deletion remain unknown.".into());
                result.set("unknowns", u)
            }
            if tool_bytes(&result) > MAX_BYTES {
                return Err(Error::refusal("selected_records_exceed_response_budget"));
            }
            Ok(result)
        })();
        match attempt {
            Ok(v) => Ok(v),
            Err(e) if e.properties.has("code") => {
                base.set("status", "unavailable".into());
                base.set("code", e.properties.get("code").clone());
                base.set("records", Js::Arr(vec![]));
                base.set(
                    "page",
                    obj(&[
                        ("totalInSavedSelection", Js::Null),
                        ("returned", Js::Num(0.)),
                        ("exhausted", false.into()),
                        ("nextCursor", Js::Null),
                    ]),
                );
                if tool_bytes(&base) > MAX_BYTES - 1024 {
                    Err(Error::refusal(
                        "fixed_source_metadata_exceeds_response_budget",
                    ))
                } else {
                    Ok(base)
                }
            }
            Err(e) => Err(e),
        }
    }
}
pub fn load(root: &Path) -> Result<SavedContext> {
    let b = fs::read(root.join(PIN_PATH)).map_err(|_| Error::error("source_pins_read_failed"))?;
    let pins = parse(&String::from_utf8_lossy(&b))?;
    let mut bytes = vec![];
    for path in [AUDIT_PATH, LESSON_PATH] {
        let pin = pins
            .get("files")
            .array()
            .iter()
            .find(|p| p.get("path").is(path))
            .ok_or_else(|| Error::refusal("input_pin_missing"))?;
        if !sha_valid(pin.get("sha256")) {
            return Err(Error::refusal("input_pin_missing"));
        }
        let file = root.join(path);
        let st =
            fs::symlink_metadata(&file).map_err(|_| Error::error("pinned_input_read_failed"))?;
        if !st.is_file() || st.len() > 32 * 1024 * 1024 {
            return Err(Error::refusal("pinned_input_file_invalid"));
        }
        let b = fs::read(&file).map_err(|_| Error::error("pinned_input_read_failed"))?;
        if !pin.get("sha256").is(&hash(&b)) {
            return Err(Error::refusal("pinned_input_changed"));
        }
        bytes.push(b);
    }
    SavedContext::from_bytes(&bytes[0], &bytes[1])
}
/// Pure native config schema only. It does not read owner configuration or start a client.
pub fn native_config(v: &Js) -> Result<Js> {
    let mut issues = vec![];
    if !typed(v, "object", &[], &mut issues) {
        return Err(Error::zod(issues));
    }
    enum_value(
        v.get("schemaVersion"),
        &["academy.native-dailyrecord-read/1"],
        &["schemaVersion".into()],
        &mut issues,
    );
    if typed(
        v.get("sourceBundlePath"),
        "string",
        &["sourceBundlePath".into()],
        &mut issues,
    ) && !string(v.get("sourceBundlePath")).starts_with('/')
    {
        issues.push(issue(
            &[
                ("origin", "string".into()),
                ("code", "invalid_format".into()),
                ("format", "starts_with".into()),
                ("prefix", "/".into()),
            ],
            &["sourceBundlePath".into()],
            "Invalid string: must start with \"/\"".into(),
        ));
    }
    for key in ["sourceBundleSha256"] {
        let p = vec![key.into()];
        if typed(v.get(key), "string", &p, &mut issues) && !sha_valid(v.get(key)) {
            issues.push(issue(
                &[
                    ("origin", "string".into()),
                    ("code", "invalid_format".into()),
                    ("format", "regex".into()),
                    ("pattern", "/^[a-f0-9]{64}$/".into()),
                ],
                &p,
                "Invalid string: must match pattern /^[a-f0-9]{64}$/".into(),
            ));
        }
    }
    if typed(v.get("date"), "string", &["date".into()], &mut issues) && !valid_date(v.get("date")) {
        issues.push(issue(
            &[
                ("origin", "string".into()),
                ("code", "invalid_format".into()),
                ("format", "date".into()),
                ("pattern", DATE_PATTERN.into()),
            ],
            &["date".into()],
            "Invalid ISO date".into(),
        ));
    }
    let s = v.get("students");
    if typed(s, "array", &["students".into()], &mut issues) {
        sized(s, 1, 8, &["students".into()], &mut issues);
        for (i, s) in s.array().iter().enumerate() {
            let p = vec!["students".into(), i.into()];
            if typed(s, "object", &p, &mut issues) {
                if typed(
                    s.get("studentId"),
                    "string",
                    &path(&p, "studentId"),
                    &mut issues,
                ) && !edu_academy_core::js::test("^[1-9][0-9]{0,15}$", s.get("studentId"))
                {
                    issues.push(issue(
                        &[
                            ("origin", "string".into()),
                            ("code", "invalid_format".into()),
                            ("format", "regex".into()),
                            ("pattern", "/^[1-9][0-9]{0,15}$/".into()),
                        ],
                        &path(&p, "studentId"),
                        "Invalid string: must match pattern /^[1-9][0-9]{0,15}$/".into(),
                    ));
                }
                if typed(
                    s.get("displayName"),
                    "string",
                    &path(&p, "displayName"),
                    &mut issues,
                ) {
                    sized(
                        s.get("displayName"),
                        1,
                        200,
                        &path(&p, "displayName"),
                        &mut issues,
                    )
                }
                strict(s, &["studentId", "displayName"], &p, &mut issues);
            }
        }
    }
    strict(
        v,
        &[
            "schemaVersion",
            "sourceBundlePath",
            "sourceBundleSha256",
            "date",
            "students",
        ],
        &[],
        &mut issues,
    );
    if issues.is_empty() {
        for key in ["studentId", "displayName"] {
            let a = s
                .array()
                .iter()
                .map(|s| s.get(key).clone())
                .collect::<Vec<_>>();
            if edu_academy_core::js::unique(&a).len() != a.len() {
                issues.push(obj(&[
                    ("code", "custom".into()),
                    ("message", "unique_admitted_students_required".into()),
                    ("path", Js::Arr(vec![])),
                ]));
                break;
            }
        }
    }
    if issues.is_empty() {
        Ok(v.clone())
    } else {
        Err(Error::zod(issues))
    }
}
pub fn student_search_rows(v: &Js) -> Result<Js> {
    let mut issues = vec![];
    let mut rows = vec![];
    if typed(v, "array", &[], &mut issues) {
        sized(v, 0, 201, &[], &mut issues);
        for (i, row) in v.array().iter().enumerate() {
            let p = vec![i.into()];
            if typed(row, "object", &p, &mut issues) {
                let key = row.get("studentKey");
                if typed(key, "string", &path(&p, "studentKey"), &mut issues)
                    && !edu_academy_core::js::test("^[1-9][0-9]{0,15}$", key)
                {
                    issues.push(issue(
                        &[
                            ("origin", "string".into()),
                            ("code", "invalid_format".into()),
                            ("format", "regex".into()),
                            ("pattern", "/^[1-9][0-9]{0,15}$/".into()),
                        ],
                        &path(&p, "studentKey"),
                        "Invalid string: must match pattern /^[1-9][0-9]{0,15}$/".into(),
                    ));
                }
                if typed(
                    row.get("studentName"),
                    "string",
                    &path(&p, "studentName"),
                    &mut issues,
                ) {
                    sized(
                        row.get("studentName"),
                        1,
                        200,
                        &path(&p, "studentName"),
                        &mut issues,
                    )
                }
                strict(row, &["studentKey", "studentName"], &p, &mut issues);
                rows.push(obj(&[
                    ("studentKey", key.clone()),
                    ("studentName", row.get("studentName").clone()),
                ]));
            }
        }
    }
    if issues.is_empty() {
        Ok(Js::Arr(rows))
    } else {
        Err(Error::zod(issues))
    }
}
