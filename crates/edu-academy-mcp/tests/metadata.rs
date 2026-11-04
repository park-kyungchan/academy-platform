//! SOURCE-ONLY: UNEXECUTED. Finite raw source captures, actual service and child.
//! Lead supplies explicit test root and hash-pinned release child; no fallback.
use edu_academy_mcp::{Js, business, digest, encode, obj, parse};
use edu_academy_sources::{
    data::{Error, Result},
    history::{History, Port},
    metadata,
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
const AT: &str = "2040-01-04T12:00:00.000Z";
const COVERAGE: &str = "one_observed_student_search_page_not_complete_history";
const KINDS: [&str; 5] = [
    "student_papers",
    "daily_metadata",
    "daily_inventory",
    "daily_history",
    "printed_paper_metadata",
];
static NEXT: AtomicU64 = AtomicU64::new(0);
fn js(v: Value) -> Js {
    parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn value(v: &Js) -> Value {
    serde_json::from_slice(&encode(v).unwrap()).unwrap()
}
fn root(label: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var_os("S11_METADATA_TEST_ROOT").expect("Lead-owned root required"),
    );
    assert!(base.is_absolute());
    assert_eq!(fs::canonicalize(&base).unwrap(), base);
    assert!(edu_academy_mcp::roots::canonical_absolute(&base));
    let path = base.join(format!(
        "metadata-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    path
}
fn put(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn retain(root: &Path, label: &str, output: &Js) {
    put(
        &root.join(format!("{label}.json")),
        &encode(output).unwrap(),
    );
}
fn period() -> Value {
    json!({"from":"2040-01-01","through":"2040-01-04"})
}
fn query(kind: &str) -> Value {
    match kind {
        "student_papers" => json!({"kind":kind,"period":period()}),
        "printed_paper_metadata" => json!({"kind":kind,"paperCode":"#3","period":period()}),
        "daily_history" => json!({"kind":kind,"periods":["12","11"]}),
        _ => json!({"kind":kind}),
    }
}
fn scope() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.metadata".into()),
        ("teacherId", "99001".into()),
        ("studentId", "77001".into()),
        ("lessonDate", "2040-01-04".into()),
    ])
}
fn input(queries: Vec<Value>, by_name: bool) -> Js {
    let mut i = if by_name {
        obj(&[
            ("displayName", "Fictional Learner".into()),
            ("lessonDate", "2040-01-04".into()),
        ])
    } else {
        obj(&[])
    };
    i.set("purpose", "Fictional metadata read".into());
    i.set("currentRequestRef", "fictional-request".into());
    if !by_name {
        i.set("scope", scope());
    }
    i.set("queries", js(json!(queries)));
    i
}
fn routing() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("allowedFrom", "2040-01-04".into()),
        ("allowedThrough", "2040-01-04".into()),
    ])
}
fn admission() -> Value {
    json!({"schemaVersion":"academy.metadata-admission/1","sourceNamespace":"academy.lms.metadata","teacherId":"99001","allowedFrom":"2040-01-01","allowedThrough":"2040-01-31","studentScope":"configured","relativeSelectors":["1","11","12"],"operations":KINDS})
}
fn student() -> Value {
    json!({"studentKey":"77001","studentName":"Fictional Learner"})
}
fn row(paper: u64, attempt: u64) -> Value {
    json!({"priNo":77001,"userName":"Fictional Learner","pNo":paper,"testingNo":attempt,"_id":"row_1","score":0,"title":"  Source title  ","testName":"DT","unlisted":"PRIVATE_CANARY"})
}
fn projected(paper: &str, attempt: &str) -> Value {
    json!({"studentKey":"77001","studentName":"Fictional Learner","paperKey":paper,"attemptKey":attempt,"sourceRowId":"row_1","sourceScore":0,"titles":{"title":"  Source title  ","testName":"DT"}})
}
fn daily(selector: &str, rows: Vec<Value>) -> Value {
    json!({"selector":selector,"operation":"DtResultSearch","page":1,"observedAt":AT,"relativeSelectors":["1","11","12"],"rawRows":rows,
        "filterContracts":{"functions":{"untrusted":"PRIVATE_CANARY"}},"displayRows":["PRIVATE_CANARY"]})
}
fn assessment(rows: Vec<Value>) -> Value {
    json!({"operation":"UserBySearchTestResult","page":1,"sourcePeriod":{"from":"2040-01-01","to":"2040-01-04"},"observedAt":AT,"rawRows":rows,"diagnostic":"PRIVATE_CANARY"})
}
fn capture(operation: &str, input: Value, raw: Value) -> Value {
    json!({"operation":operation,"input":input,"raw":raw})
}
fn captures(i: &Js, by_name: bool) -> Vec<Value> {
    let mut c = vec![];
    if by_name {
        c.push(capture("dated_students",value(i),json!([{"teacherKey":"99001","studentName":"Fictional Learner","date":"2040-01-04","studentKey":"77001"}])));
    }
    for q in i.get("queries").array() {
        if q.get("kind").is("student_papers") || q.get("kind").is("printed_paper_metadata") {
            c.push(capture(
                "student_paper_page",
                json!({"student":student(),"period":period()}),
                assessment(vec![row(1, 101), row(2, 102)]),
            ));
        } else {
            c.push(capture(
                "daily_page",
                json!({"student":student(),"selector":"1"}),
                daily("1", vec![row(2, 102), row(1, 101)]),
            ));
            if q.get("kind").is("daily_history") {
                for selector in q.get("periods").array() {
                    let s = selector.code_units();
                    c.push(capture(
                        "daily_page",
                        json!({"student":student(),"selector":s}),
                        daily(&s, vec![row(3, 103)]),
                    ));
                }
            }
        }
    }
    c
}
struct RawPort {
    captures: VecDeque<Value>,
    events: Arc<Mutex<Vec<Value>>>,
    absent: Option<String>,
}
impl RawPort {
    fn take(&mut self, operation: &str, input: &Js) -> Result<Js> {
        let c = self
            .captures
            .pop_front()
            .expect("finite source budget, no retry");
        assert_eq!(c["operation"], operation);
        assert_eq!(c["input"], value(input));
        self.events
            .lock()
            .unwrap()
            .push(json!({"operation":operation,"input":value(input)}));
        if let Some(error) = c.get("error") {
            return Err(Error::new(
                c.get("errorName")
                    .and_then(Value::as_str)
                    .unwrap_or("Error"),
                error.as_str().unwrap(),
            ));
        }
        Ok(js(c["raw"].clone()))
    }
}
impl Port for RawPort {
    fn dated_students(&mut self, input: &Js) -> Result<Js> {
        self.take("dated_students", input)
    }
    fn daily_page(&mut self, s: &Js, selector: &Js) -> Result<Js> {
        self.take(
            "daily_page",
            &obj(&[("student", s.clone()), ("selector", selector.clone())]),
        )
    }
    fn paper_header(&mut self, _: &Js, _: &Js) -> Result<Js> {
        panic!("metadata never invokes paper headers")
    }
    fn student_paper_page(&mut self, s: &Js, period: &Js) -> Result<Js> {
        self.take(
            "student_paper_page",
            &obj(&[("student", s.clone()), ("period", period.clone())]),
        )
    }
    fn metadata_method_available(&self, kind: &str) -> bool {
        self.absent.as_deref() != Some(kind)
    }
}
type Events = Arc<Mutex<Vec<Value>>>;
fn source_with(
    c: Vec<Value>,
    policy: Value,
    configured: Vec<Js>,
    managed: Vec<Js>,
    absent: Option<String>,
) -> (History<RawPort>, Events) {
    let events = Arc::new(Mutex::new(vec![]));
    let source = History::new(
        RawPort {
            captures: c.into(),
            events: events.clone(),
            absent,
        },
        routing(),
        js(policy),
        configured,
        managed,
        vec![],
    )
    .unwrap();
    (source, events)
}
fn source(c: Vec<Value>) -> (History<RawPort>, Events) {
    source_with(
        c,
        admission(),
        vec![js(
            json!({"studentId":"77001","displayName":"Fictional Learner"}),
        )],
        vec!["Fictional Learner".into()],
        None,
    )
}
fn driver(c: Vec<Value>) -> (business::History<RawPort>, Events) {
    let (s, e) = source(c);
    (business::History::from_source(s), e)
}
fn observed_page(rows: Vec<Value>, inventory: bool, selector: Option<&str>) -> Value {
    let mut p = json!({"rows":rows,"operation":"DtResultSearch","page":1,"sourcePeriod":null,"observedAt":AT,"coverage":COVERAGE,"generatedMembership":"unqualified","fixedSourceFilter":{"sort_date":selector.unwrap_or("1")}});
    if inventory {
        p["expectedPaperDefinitions"] = json!("not_observed");
    }
    if let Some(s) = selector {
        p["selector"] = json!(s);
    }
    p
}
fn expected_observation(kind: &str) -> Value {
    match kind {
        "student_papers" => {
            json!({"rows":[projected("1","101"),projected("2","102")],"operation":"UserBySearchTestResult","page":1,"sourcePeriod":{"from":"2040-01-01","to":"2040-01-04"},"observedAt":AT,"coverage":COVERAGE,"generatedMembership":"unqualified"})
        }
        "daily_history" => {
            json!({"status":"observed","studentKey":"77001","studentName":"Fictional Learner","currentFilterQualification":{"selector":"1","observedAt":AT,"rows":[projected("2","102"),projected("1","101")]},
            "observations":[observed_page(vec![projected("3","103")],true,Some("12")),observed_page(vec![projected("3","103")],true,Some("11"))],
            "calendarBounds":"source_relative_labels_only_not_exact_calendar_bounds","expectedPaperDefinitions":"not_observed","coverage":"all_parsed_embedded_rows_on_each_observed_response_not_complete_history"})
        }
        "printed_paper_metadata" => {
            json!({"paperCode":"#3","paperKey":"1","status":"observed","rows":[projected("1","101")],"sourcePeriod":{"from":"2040-01-01","to":"2040-01-04"},"page":1,"observedAt":AT,"coverage":"observed_source_pages_not_complete_history","matchedSourceFamily":"student_assessment","generatedMembership":"unqualified",
            "searchedSources":[{"family":"student_assessment","operation":"UserBySearchTestResult","page":1,"sourcePeriod":{"from":"2040-01-01","to":"2040-01-04"},"requestedPeriodApplied":true,"coverage":COVERAGE,"observedAt":AT}]})
        }
        _ => observed_page(
            vec![projected("2", "102"), projected("1", "101")],
            kind == "daily_inventory",
            None,
        ),
    }
}
#[test]
fn all_query_kinds_get_and_selected_batch_match_original_projection() {
    let root = root("positive");
    for by_name in [true, false] {
        for kind in KINDS {
            let i = input(vec![query(kind)], by_name);
            let c = captures(&i, by_name);
            let count = c.len();
            let (mut d, events) = driver(c);
            let out = d.student_metadata_call(&i, by_name).unwrap();
            retain(&root, &format!("{by_name}-{kind}"), &out);
            let mut expected = json!({"purpose":"Fictional metadata read","currentRequestRef":"fictional-request","academyWrites":0,"parentSends":0,"automaticRetryAllowed":false,"execution":"read_only","status":"observed","scope":value(&scope()),"results":[{"query":query(kind),"observation":expected_observation(kind)}]});
            if !by_name {
                expected["sourceContentIsUntrustedData"] = json!(true);
            }
            assert_eq!(value(&out), expected, "{by_name}-{kind}");
            assert_eq!(events.lock().unwrap().len(), count);
            assert!(
                !String::from_utf8(encode(&out).unwrap())
                    .unwrap()
                    .contains("PRIVATE_CANARY")
            );
        }
    }
    // Duplicate queries are allowed by the original; results and calls keep order.
    let i = input(
        vec![
            query("daily_inventory"),
            query("student_papers"),
            query("daily_history"),
            query("daily_inventory"),
        ],
        false,
    );
    let (mut d, e) = driver(captures(&i, false));
    let out = d.student_metadata_call(&i, false).unwrap();
    assert_eq!(
        out.get("results")
            .array()
            .iter()
            .map(|r| r.get("query").clone())
            .collect::<Vec<_>>(),
        i.get("queries").array()
    );
    assert_eq!(e.lock().unwrap().len(), 6);
    retain(&root, "ordered-batch", &out);
}
#[test]
fn strict_inputs_and_authority_fields_hold_before_any_source_call() {
    let root = root("strict");
    for by_name in [true, false] {
        for kind in KINDS {
            let good = input(vec![query(kind)], by_name);
            let mut bad = vec![];
            for (k, v) in [
                ("authority", json!(true)),
                ("metadataAdmission", admission()),
                ("purpose", json!("\u{0000}")),
                ("currentRequestRef", json!("")),
                ("queries", json!([])),
                ("queries", json!(vec![query(kind); 9])),
            ] {
                let mut i = good.clone();
                i.set(k, js(v));
                bad.push(i);
            }
            let mut q = query(kind);
            q["sourceVerified"] = json!(true);
            let mut i = good.clone();
            i.set("queries", js(json!([q])));
            bad.push(i);
            if kind == "student_papers" || kind == "printed_paper_metadata" {
                for p in [
                    json!({"from":"2040-01-05","through":"2040-01-04"}),
                    json!({"from":"2040-02-30","through":"2040-03-01"}),
                    json!({"from":"2040-01-01","through":"2040-01-04","proof":true}),
                ] {
                    let mut q = query(kind);
                    q["period"] = p;
                    let mut i = good.clone();
                    i.set("queries", js(json!([q])));
                    bad.push(i);
                }
            }
            if kind == "printed_paper_metadata" {
                for code in ["#2", "#I", "#0", "#3lower", "#333333333333"] {
                    let mut q = query(kind);
                    q["paperCode"] = json!(code);
                    let mut i = good.clone();
                    i.set("queries", js(json!([q])));
                    bad.push(i);
                }
            }
            if kind == "daily_history" {
                for selectors in [
                    json!([]),
                    json!(["11", "11"]),
                    json!(["1"]),
                    json!(["17"]),
                    json!(vec!["11"; 7]),
                ] {
                    let mut q = query(kind);
                    q["periods"] = selectors;
                    let mut i = good.clone();
                    i.set("queries", js(json!([q])));
                    bad.push(i);
                }
            }
            if by_name {
                for name in [" ".to_string(), "😀".repeat(101)] {
                    let mut i = good.clone();
                    i.set("displayName", name.into());
                    bad.push(i);
                }
                let mut i = good.clone();
                i.set("lessonDate", "2040-02-30".into());
                bad.push(i);
            } else {
                let mut i = good.clone();
                let mut s = scope();
                s.set("authority", true.into());
                i.set("scope", s);
                bad.push(i);
                let mut i = good.clone();
                let mut s = scope();
                s.set("sourceNamespace", "academy.lms.dayrecord".into());
                i.set("scope", s);
                bad.push(i);
            }
            let (mut d, e) = driver(vec![]);
            for i in bad {
                assert_eq!(
                    d.student_metadata_call(&i, by_name).unwrap_err().message,
                    "tool_input_invalid"
                );
            }
            assert!(e.lock().unwrap().is_empty());
            retain(
                &root,
                &format!("{by_name}-{kind}"),
                &"tool_input_invalid".into(),
            );
        }
    }
    let mut i = input(vec![query("daily_metadata")], true);
    i.set("displayName", format!("  {}  ", "😀".repeat(100)).into());
    i.set("lessonDate", "2040-02-29".into());
    assert_eq!(
        metadata::prepare(&i, true)
            .unwrap()
            .get("displayName")
            .units()
            .len(),
        200
    );
}
#[test]
fn metadata_policy_and_selected_scope_are_independent_preflight_holds() {
    let root = root("admission");
    for by_name in [true, false] {
        for kind in KINDS {
            for mode in ["operation", "lesson-date", "selector", "period", "method"] {
                if mode == "selector" && kind == "student_papers"
                    || mode == "period"
                        && !["student_papers", "printed_paper_metadata"].contains(&kind)
                {
                    continue;
                }
                let mut p = admission();
                match mode {
                    "operation" => {
                        p["operations"] = json!(
                            KINDS
                                .iter()
                                .filter(|k| **k != kind)
                                .copied()
                                .collect::<Vec<_>>()
                        )
                    }
                    "lesson-date" => p["allowedThrough"] = json!("2040-01-03"),
                    "selector" => p["relativeSelectors"] = json!([]),
                    "period" => p["allowedFrom"] = json!("2040-01-02"),
                    _ => (),
                }
                let (s, e) = source_with(
                    vec![],
                    p,
                    vec![js(
                        json!({"studentId":"77001","displayName":"Fictional Learner"}),
                    )],
                    vec!["Fictional Learner".into()],
                    if mode == "method" {
                        Some(kind.into())
                    } else {
                        None
                    },
                );
                let out = business::History::from_source(s)
                    .student_metadata_call(&input(vec![query(kind)], by_name), by_name)
                    .unwrap();
                assert!(out.get("status").is("unavailable"));
                assert!(out.get("reasonCode").is(if mode == "method" {
                    "native_metadata_method_unavailable"
                } else {
                    "metadata_namespace_not_admitted"
                }));
                assert_eq!(out.get("sourceRequests"), &Js::Num(0.0));
                assert!(e.lock().unwrap().is_empty());
                retain(&root, &format!("{by_name}-{kind}-{mode}"), &out);
            }
        }
    }
    // source_verified may admit a managed name, but never an unselected batch ID.
    let mut p = admission();
    p["studentScope"] = json!("source_verified");
    let (s, e) = source_with(vec![], p, vec![], vec!["Fictional Learner".into()], None);
    let out = business::History::from_source(s)
        .student_metadata_call(&input(vec![query("daily_metadata")], false), false)
        .unwrap();
    assert!(out.get("reasonCode").is("native_metadata_scope_refused"));
    assert!(e.lock().unwrap().is_empty());
    for (p, names) in [
        (admission(), vec![]),
        (Value::Null, vec!["Fictional Learner".into()]),
    ] {
        let (s, e) = source_with(
            vec![],
            p,
            vec![js(
                json!({"studentId":"77001","displayName":"Fictional Learner"}),
            )],
            names,
            None,
        );
        let out = business::History::from_source(s)
            .student_metadata_call(&input(vec![query("daily_metadata")], true), true)
            .unwrap();
        assert!(out.get("reasonCode").is("metadata_namespace_not_admitted"));
        assert!(e.lock().unwrap().is_empty());
    }
    let i = input(vec![query("daily_metadata")], false);
    let (s, e) = source_with(
        captures(&i, false),
        Value::Null,
        vec![js(
            json!({"studentId":"77001","displayName":"Fictional Learner"}),
        )],
        vec![],
        None,
    );
    let mut d = business::History::from_source(s);
    assert!(
        d.student_metadata_call(&i, false)
            .unwrap()
            .get("status")
            .is("observed")
    );
    let mut other = i.clone();
    let mut scope = scope();
    scope.set("lessonDate", "2040-01-05".into());
    other.set("scope", scope);
    assert!(
        d.student_metadata_call(&other, false)
            .unwrap()
            .get("reasonCode")
            .is("native_metadata_scope_refused")
    );
    assert_eq!(e.lock().unwrap().len(), 1);
}
#[test]
fn all_query_kinds_scope_and_query_conflicts_are_source_failures() {
    let root = root("service-conflicts");
    for by_name in [true, false] {
        for kind in KINDS {
            let i = metadata::prepare(&input(vec![query(kind)], by_name), by_name).unwrap();
            let original = obj(&[
                ("scope", scope()),
                (
                    "results",
                    Js::Arr(vec![obj(&[
                        ("query", i.get("queries").array()[0].clone()),
                        ("observation", js(expected_observation(kind))),
                    ])]),
                ),
            ]);
            for mode in ["date", "query", "count", "namespace", "malformed-results"] {
                let mut bad = original.clone();
                match mode {
                    "date" => {
                        let mut s = scope();
                        s.set("lessonDate", "2040-01-05".into());
                        bad.set("scope", s);
                    }
                    "query" => {
                        let mut rows = bad.get("results").array().to_vec();
                        rows[0].set("query", obj(&[("kind", "unrequested".into())]));
                        bad.set("results", Js::Arr(rows));
                    }
                    "count" => bad.set("results", Js::Arr(vec![])),
                    "namespace" => {
                        let mut s = scope();
                        s.set("sourceNamespace", "fictional.foreign".into());
                        bad.set("scope", s);
                    }
                    _ => bad.set("results", true.into()),
                }
                let e = metadata::qualify_observation(&bad, &i, by_name).unwrap_err();
                assert_eq!(
                    e.message,
                    if ["namespace", "malformed-results"].contains(&mode) {
                        "source_response_malformed"
                    } else if by_name {
                        "native_metadata_row_identity_conflict"
                    } else {
                        "metadata_scope_or_query_conflict"
                    }
                );
                retain(
                    &root,
                    &format!("{by_name}-{kind}-{mode}"),
                    &e.message.into(),
                );
            }
        }
    }
}
#[test]
fn every_query_kind_malformed_source_maps_to_source_response_malformed() {
    let root = root("malformed");
    for by_name in [true, false] {
        for kind in KINDS {
            for mode in ["rows", "instant", "row-key", "row-shape"] {
                let i = input(vec![query(kind)], by_name);
                let mut c = captures(&i, by_name);
                let n = usize::from(by_name);
                match mode {
                    "rows" => c[n]["raw"]["rawRows"] = json!(false),
                    "instant" => c[n]["raw"]["observedAt"] = json!("2040-02-30T00:00:00Z"),
                    "row-key" => c[n]["raw"]["rawRows"][0]["pNo"] = json!(null),
                    _ => c[n]["raw"]["rawRows"][0] = json!(true),
                }
                let (mut d, e) = driver(c);
                let out = d.student_metadata_call(&i, by_name).unwrap();
                assert!(out.get("status").is("unavailable"));
                assert!(
                    out.get("reasonCode").is("source_response_malformed"),
                    "{out:?}"
                );
                assert!(out.get("failure").get("category").is("malformed_response"));
                assert_eq!(e.lock().unwrap().len(), n + 1);
                assert_eq!(out.get("sourceRequests"), &Js::from(n + 1));
                retain(&root, &format!("{by_name}-{kind}-{mode}"), &out);
            }
        }
    }
}
#[test]
fn all_query_kinds_oversize_keeps_unavailable_envelope_and_measured_calls() {
    let root = root("cap");
    for by_name in [true, false] {
        for kind in KINDS {
            let i = input(vec![query(kind)], by_name);
            let mut c = captures(&i, by_name);
            let n = usize::from(by_name);
            c[n]["raw"]["rawRows"] = json!(
                (0..1500)
                    .map(|a| {
                        let mut r = row(1, 10000 + a);
                        r["title"] = json!("😀".repeat(500));
                        r
                    })
                    .collect::<Vec<_>>()
            );
            // Prove the valid raw source crosses the unchanged dual-MCP cap.
            let (mut raw, _) = source(c.clone());
            let observation = raw.student_metadata(&i, by_name).unwrap();
            assert!(
                encode(&edu_academy_mcp::audit::tool_result(observation).unwrap())
                    .unwrap()
                    .len()
                    > 512 * 1024
            );
            let count = c.len();
            let (mut d, e) = driver(c);
            let out = d.student_metadata_call(&i, by_name).unwrap();
            assert!(out.get("status").is("unavailable"));
            assert!(
                out.get("reasonCode")
                    .is("student_metadata_unqualified_no_automatic_retry")
            );
            assert_eq!(out.get("sourceRequests"), &Js::from(count));
            assert_eq!(e.lock().unwrap().len(), count);
            assert!(!out.has("scope") && !out.has("results"));
            assert!(!out.get("automaticRetryAllowed").truthy());
            assert_eq!(out.has("purpose"), by_name);
            assert_eq!(out.get("failure").has("errorKind"), by_name);
            assert_eq!(out.get("failure").has("errorSha256"), by_name);
            assert!(
                encode(&edu_academy_mcp::audit::tool_result(out.clone()).unwrap())
                    .unwrap()
                    .len()
                    <= 512 * 1024
            );
            retain(&root, &format!("{by_name}-{kind}"), &out);
        }
    }
}
#[test]
fn every_query_kind_failure_catalog_absence_and_shared_latch_never_retry() {
    let root = root("failure");
    for by_name in [true, false] {
        for kind in KINDS {
            let i = input(vec![query(kind)], by_name);
            for (code, category, reason, name) in [
                (
                    "source_authentication_failed",
                    "authentication",
                    "source_authentication_failed",
                    "Error",
                ),
                (
                    "PRIVATE_CANARY",
                    "upstream_unavailable",
                    "student_metadata_unqualified_no_automatic_retry",
                    "Error",
                ),
                (
                    "PRIVATE_CANARY",
                    "malformed_response",
                    "source_response_malformed",
                    "SyntaxError",
                ),
            ] {
                let mut c = captures(&i, by_name);
                let n = usize::from(by_name);
                c[n]["error"] = json!(code);
                c[n]["errorName"] = json!(name);
                let (mut d, e) = driver(c);
                let out = d.student_metadata_call(&i, by_name).unwrap();
                assert!(out.get("reasonCode").is(reason));
                assert!(out.get("failure").get("category").is(category));
                assert_eq!(out.get("sourceRequests"), &Js::from(n + 1));
                assert_eq!(e.lock().unwrap().len(), n + 1);
                assert!(
                    !String::from_utf8(encode(&out).unwrap())
                        .unwrap()
                        .contains("PRIVATE_CANARY")
                );
                let later = d.student_metadata_call(&i, by_name).unwrap();
                assert!(
                    later
                        .get("reasonCode")
                        .is("source_failure_latched_no_retry")
                );
                assert_eq!(later.get("sourceRequests"), &Js::Num(0.0));
                assert_eq!(e.lock().unwrap().len(), n + 1);
                retain(&root, &format!("{by_name}-{kind}-{name}-{category}"), &out);
            }
            let (d, e) = driver(vec![]);
            let out = d
                .without_metadata_backend()
                .student_metadata_call(&i, by_name)
                .unwrap();
            assert!(
                out.get("reasonCode")
                    .is("student_metadata_backend_not_configured")
            );
            assert!(!out.has("failure"));
            assert!(!out.has("sourceRequests"));
            assert_eq!(out.has("purpose"), by_name);
            assert!(e.lock().unwrap().is_empty());
            retain(&root, &format!("{by_name}-{kind}-absent"), &out);
        }
    }
}
#[test]
fn printed_lookup_fallback_is_only_after_an_observed_miss() {
    let root = root("printed");
    for mode in ["assessment", "daily", "missing", "error"] {
        let i = input(vec![query("printed_paper_metadata")], false);
        let mut c = captures(&i, false);
        if mode != "assessment" {
            c[0]["raw"]["rawRows"] = json!([]);
        }
        if mode == "error" {
            c[0]["error"] = json!("source_authentication_failed");
        }
        if mode == "daily" || mode == "missing" {
            c.push(capture(
                "daily_page",
                json!({"student":student(),"selector":"1"}),
                daily(
                    "1",
                    if mode == "daily" {
                        vec![row(1, 105)]
                    } else {
                        vec![]
                    },
                ),
            ));
        }
        let count = c.len();
        let (mut d, e) = driver(c);
        let out = d.student_metadata_call(&i, false).unwrap();
        if mode == "error" {
            assert!(out.get("status").is("unavailable"));
        } else {
            let o = out.get("results").array()[0].get("observation");
            assert!(o.get("status").is(if mode == "missing" {
                "not_found_in_scope"
            } else {
                "observed"
            }));
            assert_eq!(o.get("searchedSources").array().len(), count);
            if mode == "daily" {
                assert!(o.get("matchedSourceFamily").is("daily_zero"));
                assert_eq!(o.get("sourcePeriod"), &Js::Null);
                assert_eq!(
                    o.get("searchedSources").array()[1].get("requestedPeriodApplied"),
                    &Js::Bool(false)
                );
            }
            if mode == "missing" {
                assert_eq!(o.get("matchedSourceFamily"), &Js::Null);
                assert_eq!(o.get("sourcePeriod").get("to"), &Js::from("2040-01-04"));
            }
        }
        assert_eq!(e.lock().unwrap().len(), count);
        retain(&root, mode, &out);
    }
}
#[test]
fn dated_identity_and_query_source_conflicts_stop_at_the_actual_boundary() {
    let root = root("identity");
    for (mode, code) in [
        ("teacher", "native_dated_name_scope_conflict"),
        ("date", "native_dated_name_scope_conflict"),
        ("name", "native_dated_name_scope_conflict"),
        ("key", "native_dated_name_scope_conflict"),
        ("ambiguous", "native_dated_name_ambiguous"),
        ("missing", "native_dailyrecord_occurrence_unverified"),
    ] {
        let i = input(vec![query("daily_metadata")], true);
        let mut c = captures(&i, true);
        match mode {
            "teacher" => c[0]["raw"][0]["teacherKey"] = json!("99002"),
            "date" => c[0]["raw"][0]["date"] = json!("2040-01-05"),
            "name" => c[0]["raw"][0]["studentName"] = json!("Other Learner"),
            "key" => c[0]["raw"][0]["studentKey"] = json!("77002"),
            "ambiguous" => {
                let r = c[0]["raw"][0].clone();
                c[0]["raw"].as_array_mut().unwrap().push(r);
            }
            _ => c[0]["raw"] = json!([]),
        }
        let (mut d, e) = driver(c);
        let out = d.student_metadata_call(&i, true).unwrap();
        assert!(out.get("reasonCode").is(code));
        assert_eq!(e.lock().unwrap().len(), 1);
        retain(&root, mode, &out);
    }
    for (kind, mode, code) in [
        (
            "student_papers",
            "period",
            "native_metadata_period_conflict",
        ),
        (
            "printed_paper_metadata",
            "operation",
            "native_metadata_operation_conflict",
        ),
        (
            "daily_metadata",
            "selector",
            "native_metadata_selector_conflict",
        ),
        (
            "daily_inventory",
            "page",
            "native_metadata_selector_conflict",
        ),
        ("daily_history", "selectors", "daily_history_selector_drift"),
    ] {
        let i = input(vec![query(kind)], false);
        let mut c = captures(&i, false);
        match mode {
            "period" => c[0]["raw"]["sourcePeriod"]["to"] = json!("2040-01-03"),
            "operation" => c[0]["raw"]["operation"] = json!("DtResultSearch"),
            "selector" => c[0]["raw"]["selector"] = json!("11"),
            "page" => c[0]["raw"]["page"] = json!(2),
            _ => c[0]["raw"]["relativeSelectors"] = json!(["1"]),
        }
        let (mut d, e) = driver(c);
        let out = d.student_metadata_call(&i, false).unwrap();
        assert!(out.get("reasonCode").is(code), "{out:?}");
        assert_eq!(e.lock().unwrap().len(), 1);
        retain(&root, kind, &out);
    }
}
#[test]
fn titles_length_unknown_scores_and_other_students_never_gain_meaning() {
    let root = root("unknowns");
    let i = input(vec![query("daily_inventory")], false);
    let mut c = captures(&i, false);
    c[0]["raw"]["rawRows"][0]["score"] = json!("0");
    c[0]["raw"]["rawRows"][0]["_id"] = json!(true);
    c[0]["raw"]["rawRows"][0]["examName"] = json!("😀".repeat(1000));
    c[0]["raw"]["rawRows"][0]["testpageName"] = json!("x".repeat(1999));
    let mut other = row(9, 109);
    other["priNo"] = json!(77002);
    other["userName"] = json!("Other Learner");
    c[0]["raw"]["rawRows"].as_array_mut().unwrap().push(other);
    let (mut d, _) = driver(c);
    let out = d.student_metadata_call(&i, false).unwrap();
    let o = out.get("results").array()[0].get("observation");
    assert_eq!(o.get("rows").array().len(), 2);
    let r = &o.get("rows").array()[0];
    assert_eq!(r.get("sourceScore"), &Js::Null);
    assert_eq!(r.get("sourceRowId"), &Js::Null);
    assert!(!r.get("titles").has("examName"));
    assert_eq!(r.get("titles").get("testpageName").units().len(), 1999);
    assert!(o.get("expectedPaperDefinitions").is("not_observed"));
    assert!(o.get("generatedMembership").is("unqualified"));
    retain(&root, "observed", &out);
}

fn pin(path: &Path) -> Value {
    json!({"path":path,"sha256":digest(&fs::read(path).unwrap())})
}
fn document(path: &Path, data: Value) -> Value {
    put(path, &serde_json::to_vec(&data).unwrap());
    pin(path)
}
struct Scene {
    install: PathBuf,
    state: PathBuf,
}
impl Scene {
    fn new(label: &str, i: &Js, by_name: bool, mode: &str) -> Self {
        let base = root(label);
        for dir in ["install", "state", "evidence"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(dir))
                .unwrap();
        }
        let install = base.join("install");
        let state = base.join("state");
        let executable = PathBuf::from(
            std::env::var_os("S11_METADATA_CHILD_EXECUTABLE")
                .expect("Lead-pinned release fictional_paper_endpoint required"),
        );
        assert!(executable.is_absolute());
        assert_eq!(fs::canonicalize(&executable).unwrap(), executable);
        let expected =
            std::env::var("S11_METADATA_CHILD_SHA256").expect("Lead must pin release child hash");
        let bytes = fs::read(&executable).unwrap();
        assert_eq!(digest(&bytes), expected);
        assert!(
            bytes.len() <= edu_academy_mcp::release::ARTIFACT_BYTES,
            "release child must fit unchanged artifact cap"
        );
        let bridge = install.join("bridge");
        put(&bridge, &bytes);
        fs::set_permissions(&bridge, fs::Permissions::from_mode(0o700)).unwrap();
        let launcher = install.join("launcher");
        put(&launcher, b"fictional launcher, never executed");
        let admission_path = install.join("admission.json");
        put(&admission_path, &encode(&routing()).unwrap());
        let routing_pin = document(
            &install.join("routing.json"),
            json!({"admissionPath":admission_path,"admissionSha256":pin(&admission_path)["sha256"],"paperScopes":[]}),
        );
        let owner = document(
            &install.join("owner.json"),
            json!({"schemaVersion":"academy.rust-owner/1","kind":"fictional","uid":rustix::process::geteuid().as_raw(),"installRoot":install,"stateRoot":state,"evidenceRoot":base.join("evidence")}),
        );
        let args = vec![
            "--install-root".to_string(),
            install.to_str().unwrap().into(),
            "--endpoint".into(),
            "dailyrecord".into(),
        ];
        let mut entry = pin(&bridge);
        entry["role"] = json!("entry");
        let identity = document(
            &install.join("identity.json"),
            json!({"schemaVersion":"academy.rust-endpoint/1","endpoint":"dailyrecord","semanticContractVersion":"s11-fictional","launchArgumentsSha256":digest(&serde_json::to_vec(&args).unwrap()),"artifacts":[entry]}),
        );
        let endpoints = document(
            &install.join("endpoints.json"),
            json!({"dailyrecord":{"executable":pin(&bridge),"identity":identity,"args":args}}),
        );
        let original: Value = serde_json::from_slice(include_bytes!(
            "../../../migration/mcp-runtime-reference-v1/frozen/inventory/registry.json"
        ))
        .unwrap();
        let entries = original
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                ["academy_student_metadata_get", "academy_student_metadata"]
                    .iter()
                    .any(|n| e["upstreamName"] == *n)
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 2);
        let registry = document(&install.join("registry.json"), json!(entries));
        let mut raw = captures(i, by_name);
        if mode == "oversize" {
            raw[usize::from(by_name)]["raw"]["rawRows"] = json!(
                (0..1500)
                    .map(|a| {
                        let mut r = row(1, 10000 + a);
                        r["title"] = json!("x".repeat(1500));
                        r
                    })
                    .collect::<Vec<_>>()
            );
        }
        let config = install.join("fictional-config.json");
        document(
            &config,
            json!({"fictionalOnly":true,"metadataAdmission":admission(),"students":[{"studentId":"77001","displayName":"Fictional Learner"}],"managedNames":["Fictional Learner"],"paperStudents":{"dailyrecord":[]},"captures":{"dailyrecord":raw},"metadataBackendAbsent":mode=="backend-absent","responseFault":"none"}),
        );
        let mut artifact = pin(&config);
        artifact["role"] = json!("source");
        let build = document(
            &install.join("build.json"),
            json!({"schemaVersion":"academy.rust-build/1","sourceRevision":"0".repeat(40),"semanticContractVersion":"s11-fictional","cargoLockSha256":"0".repeat(64),"artifacts":[artifact]}),
        );
        document(
            &install.join("release.json"),
            json!({"schemaVersion":"academy.mcp-release/3","generation":"s11-metadata-fictional","bridge":pin(&bridge),"launcher":pin(&launcher),"build":build,"routing":routing_pin,"owner":owner,"registry":registry,"endpoints":endpoints}),
        );
        Self { install, state }
    }
}
struct OwnedControl;
impl edu_academy_mcp::process::Control for OwnedControl {
    fn poll(
        &mut self,
        _: &Js,
        _: &mut dyn FnMut() -> edu_academy_mcp::Result<()>,
    ) -> edu_academy_mcp::Result<()> {
        Ok(())
    }
    fn adopt_orphans(&self) -> bool {
        true
    }
}
fn lines(path: &Path) -> Vec<Js> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| parse(line.as_bytes()).unwrap())
        .collect()
}
fn child(scene: &Scene, i: &Js, by_name: bool, strict_probe: bool) -> Js {
    use edu_academy_mcp::{
        endpoint,
        process::{self, Context, Limits},
        release::{ConnectionGate, ReleaseGraph},
        upstream,
    };
    let graph = ReleaseGraph::open(&scene.install).unwrap();
    let plan = endpoint::plan(&graph, "dailyrecord").unwrap();
    let mut gate = ConnectionGate::default();
    let mut control = OwnedControl;
    let request = Js::Num(1.0);
    let limits = Limits {
        work: std::time::Duration::from_secs(10),
        ..Limits::default()
    };
    let mut context = Context::new(
        &graph,
        &mut gate,
        &mut control,
        &request,
        std::time::Instant::now() + limits.work,
    );
    let supervised = process::supervise(&plan, &mut context, limits, |child, context| {
        let mut upstream = upstream::Client::default();
        upstream.connect(child, context, "dailyrecord")?;
        let tool = if by_name {
            "academy_student_metadata_get"
        } else {
            "academy_student_metadata"
        };
        if strict_probe {
            let mut bad = i.clone();
            bad.set(
                "queries",
                js(json!([{"kind":"daily_history","periods":["11","11"]}])),
            );
            let rejected = upstream.call(child, context, tool, bad, || {})?;
            assert!(rejected.get("isError").truthy());
            assert!(
                rejected.get("content").array()[0]
                    .get("text")
                    .is("tool_input_invalid")
            );
            retain(&scene.state, "strict-result", &rejected);
        }
        upstream.call(child, context, tool, i.clone(), || {})
    });
    retain(&scene.state, "cleanup", &supervised.cleanup.evidence());
    assert!(supervised.cleanup.evidence().get("qualified").truthy());
    let cleanup = supervised.cleanup.evidence();
    let pid = match cleanup.get("pid") {
        Js::Num(n) => *n as u32,
        _ => panic!("actual child pid required"),
    };
    assert!(
        !process::Identity::read(pid)
            .is_some_and(|p| p.start.to_string() == cleanup.get("startTicks").code_units())
    );
    let out = supervised.result.unwrap();
    retain(&scene.state, "child-result", &out);
    out
}
#[test]
fn real_release_child_routes_get_and_selected_batches_without_canned_success() {
    edu_academy_mcp::process::install_binary_subreaper().unwrap();
    for by_name in [true, false] {
        let i = input(KINDS.iter().map(|k| query(k)).collect(), by_name);
        let scene = Scene::new(&format!("child-{by_name}"), &i, by_name, "normal");
        let out = child(&scene, &i, by_name, true);
        assert!(!out.get("isError").truthy());
        let (mut d, _) = driver(captures(&i, by_name));
        let expected = d.student_metadata_call(&i, by_name).unwrap();
        // Exact bytes through the real RPC session, not just a fabricated status.
        assert_eq!(
            encode(&out).unwrap(),
            encode(&edu_academy_mcp::audit::tool_result(expected).unwrap()).unwrap()
        );
        let events = lines(&scene.state.join("history-events.jsonl"));
        assert!(events.first().unwrap().get("event").is("open"));
        assert!(events.last().unwrap().get("event").is("close"));
        assert_eq!(
            events
                .iter()
                .filter(|e| e.get("event").is("source"))
                .count(),
            captures(&i, by_name).len()
        );
        let requests = lines(&scene.state.join("dailyrecord-requests.jsonl"));
        let calls = requests
            .iter()
            .filter(|r| r.get("method").is("tools/call"))
            .map(|r| r.get("params").get("name").code_units())
            .collect::<Vec<_>>();
        assert_eq!(
            calls,
            vec![
                "academy_endpoint_identity",
                "academy_runtime_read_admission",
                if by_name {
                    "academy_student_metadata_get"
                } else {
                    "academy_student_metadata"
                },
                if by_name {
                    "academy_student_metadata_get"
                } else {
                    "academy_student_metadata"
                }
            ]
        );
    }
    for mode in ["oversize", "backend-absent"] {
        let i = input(vec![query("student_papers")], true);
        let scene = Scene::new(mode, &i, true, mode);
        let out = child(&scene, &i, true, false);
        assert!(!out.get("isError").truthy());
        let body = out.get("structuredContent");
        assert!(body.get("status").is("unavailable"));
        assert!(body.get("reasonCode").is(if mode == "oversize" {
            "student_metadata_unqualified_no_automatic_retry"
        } else {
            "student_metadata_backend_not_configured"
        }));
        let count = lines(&scene.state.join("history-events.jsonl"))
            .iter()
            .filter(|e| e.get("event").is("source"))
            .count();
        assert_eq!(count, if mode == "oversize" { 2 } else { 0 });
        if mode == "oversize" {
            assert_eq!(body.get("sourceRequests"), &Js::Num(2.0));
            assert!(encode(&out).unwrap().len() <= 512 * 1024);
        }
    }
}
