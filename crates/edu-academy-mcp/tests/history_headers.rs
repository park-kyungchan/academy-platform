//! SOURCE-ONLY: all tests are UNEXECUTED by the source author.
//! Lead supplies canonical absolute roots and hash-pinned finite child executable.
use edu_academy_mcp::{
    Js,
    dailytest_history::{self as history, Source},
    digest,
    dispatch::{self, Reply},
    encode, obj, parse,
    process::{self, Control, Limits},
    release::{ConnectionGate, ReleaseGraph},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const AT: &str = "2040-01-04T12:00:00.000Z";
fn js(v: Value) -> Js {
    parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn input() -> Value {
    json!({"displayName":"  Fictional Learner  ","lessonDate":"2040-01-04","purpose":"Fictional read","currentRequestRef":"fictional-request","periods":["11","12"]})
}
fn row(paper: &str, attempt: &str) -> Value {
    json!({"studentKey":"77001","studentName":"Fictional Learner","paperKey":paper,"attemptKey":attempt,"sourceRowId":null,"sourceScore":87.5,"unlisted":"must strip"})
}
fn metadata() -> Value {
    json!({"status":"observed","scope":{"lessonDate":"2040-01-04","teacherId":"99001","studentId":"77001","sourceNamespace":"academy.lms.metadata","extra":"strip"},
    "results":[{"query":{"kind":"daily_history","periods":["11","12"]},"observation":{"status":"observed","studentKey":"77001","studentName":"Fictional Learner",
    "currentFilterQualification":{"selector":"1","observedAt":AT,"rows":[row("1","101"),row("2","102")]},
    "observations":[{"selector":"11","observedAt":AT,"operation":"DtResultSearch","page":1,"coverage":"one_observed_student_search_page_not_complete_history","fixedSourceFilter":{"sort_date":"11"},"rows":[row("2","103"),row("3","104")]},
    {"selector":"12","observedAt":AT,"operation":"DtResultSearch","page":1,"coverage":"one_observed_student_search_page_not_complete_history","fixedSourceFilter":{"sort_date":"12"},"rows":[]}],
    "coverage":"all_parsed_embedded_rows_on_each_observed_response_not_complete_history"}}]})
}
fn header(key: &str, code: &str) -> Value {
    json!({"status":"observed","requestedPaperKey":key,"paperCode":code,"observedAt":AT,
    "contentHeader":{"sourceSha256":"a".repeat(64),"metadataRows":[{"label":"시험지 제목","value":"  Source 발전  "},{"label":"평가구분","value":"DT/ZT"},{"label":"총 문제수","value":"04"}]}})
}
fn headers(args: &Js) -> Value {
    let rows = args
        .get("paperCodes")
        .array()
        .iter()
        .map(|c| {
            header(
                &edu_academy_sources::history::decode_code(c)
                    .unwrap()
                    .code_units(),
                &c.code_units(),
            )
        })
        .collect::<Vec<_>>();
    json!({"status":"observed","scope":{"studentId":"77001"},"observation":{"status":"observed","rows":rows}})
}
fn run(i: Value, m: Value, mutate: impl FnOnce(&mut Value)) -> edu_academy_mcp::Result<Js> {
    let mut mutate = Some(mutate);
    history::read(&js(i), |source, args| {
        Ok(if source == Source::Metadata {
            js(m.clone())
        } else {
            let mut h = headers(args);
            mutate.take().unwrap()(&mut h);
            js(h)
        })
    })
}
fn root(label: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var_os("S11_HISTORY_TEST_ROOT")
            .expect("explicit Lead root required; no temp fallback"),
    );
    assert!(base.is_absolute());
    assert_eq!(fs::canonicalize(&base).unwrap(), base);
    assert!(edu_academy_mcp::roots::canonical_absolute(&base));
    let path = base.join(format!(
        "history-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    path
}
fn put(path: &Path, bytes: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
fn retain(path: &Path, label: &str, v: &Js) {
    put(&path.join(format!("{label}.json")), &encode(v).unwrap());
}
#[test]
fn observed_partial_and_selection_pin_preserve_order_raw_fields_and_unknowns() {
    let root = root("pagination");
    let out = run(input(), metadata(), |_| {}).unwrap();
    retain(&root, "observed", &out);
    assert!(out.get("status").is("observed"));
    assert_eq!(out.get("sourcePaperCount"), &Js::Num(3.0));
    let h = out.get("periods").array()[0].get("rows").array()[0].get("header");
    assert!(h.get("title").is("  Source 발전  "));
    assert!(h.get("questionCount").is("04"));
    assert!(out.get("attemptDate").is("unqualified"));
    assert!(out.get("teacherCompletion").is("unqualified"));
    assert!(
        !String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("must strip")
    );
    let mut i = input();
    i["maxHeaders"] = json!(1);
    let first = run(i.clone(), metadata(), |_| {}).unwrap();
    retain(&root, "first", &first);
    assert!(first.get("status").is("partial"));
    assert_eq!(first.get("nextHeaderOffset"), &Js::Num(1.0));
    assert_eq!(first.get("selectionSha256"), out.get("selectionSha256"));
    // Independent literal ordered JSON pin: input scope order and extra fields
    // deliberately differ from the original Zod projection order.
    let mut expected_rows = vec![
        row("1", "101"),
        row("2", "102"),
        row("2", "103"),
        row("3", "104"),
    ];
    for row in &mut expected_rows {
        row.as_object_mut().unwrap().remove("unlisted");
    }
    let projected = |r: &Value| {
        obj(&[
            "studentKey",
            "studentName",
            "paperKey",
            "attemptKey",
            "sourceRowId",
            "sourceScore",
        ]
        .map(|k| (k, js(r[k].clone()))))
    };
    let selection = obj(&[
        (
            "scope",
            obj(&[
                ("sourceNamespace", "academy.lms.metadata".into()),
                ("studentId", "77001".into()),
                ("lessonDate", "2040-01-04".into()),
            ]),
        ),
        (
            "pages",
            Js::Arr(vec![
                obj(&[
                    ("selector", "1".into()),
                    (
                        "rows",
                        Js::Arr(expected_rows[..2].iter().map(projected).collect()),
                    ),
                ]),
                obj(&[
                    ("selector", "11".into()),
                    (
                        "rows",
                        Js::Arr(expected_rows[2..].iter().map(projected).collect()),
                    ),
                ]),
                obj(&[("selector", "12".into()), ("rows", Js::Arr(vec![]))]),
            ]),
        ),
    ]);
    assert!(
        first
            .get("selectionSha256")
            .is(&digest(&encode(&selection).unwrap()))
    );
    // Independently calculated from the literal JSON fixture (Python hashlib),
    // not an original-runtime capture or an executed Rust assertion.
    assert!(
        first
            .get("selectionSha256")
            .is("253d8d8a2e8a34106ec256b701820fea3072868da1ba7658e9ee3ad4a18a0673")
    );
    for offset in [1, 2, 3] {
        i["headerOffset"] = json!(offset);
        i["expectedSelectionSha256"] = json!(first.get("selectionSha256").code_units());
        let next = run(i.clone(), metadata(), |_| {}).unwrap();
        retain(&root, &format!("offset-{offset}"), &next);
        assert_eq!(
            next.get("headersRead"),
            &Js::Num(if offset < 3 { 1.0 } else { 0.0 })
        );
        // Original status compares total to this page, including terminal empty page.
        assert!(next.get("status").is("partial"));
    }
    let null = run(input(), metadata(), |h| {
        h["observation"]["rows"][0]["contentHeader"]["metadataRows"] =
            json!([{"label":"시험지 제목","value":""}]);
    })
    .unwrap();
    let h = null.get("periods").array()[0].get("rows").array()[0].get("header");
    assert_eq!(h.get("title"), &Js::Null);
    assert_eq!(h.get("category"), &Js::Null);
    assert_eq!(h.get("questionCount"), &Js::Null);
}
#[test]
fn strict_input_rejects_before_any_call_and_uses_utf16_and_literal_dates() {
    let root = root("strict");
    let mut bad = vec![];
    for (key, value) in [
        ("unexpected", json!(true)),
        ("lessonDate", json!("today")),
        ("lessonDate", json!("2040-02-30")),
        ("lessonDate", json!(null)),
        ("displayName", json!(" ")),
        ("displayName", json!("😀".repeat(101))),
        ("purpose", json!("\u{0000}")),
        ("currentRequestRef", json!("")),
        ("periods", json!([])),
        ("periods", json!(["11", "11"])),
        ("periods", json!(["1"])),
        ("maxHeaders", json!(0)),
        ("maxHeaders", json!(33)),
        ("maxHeaders", json!(1.5)),
        ("maxHeaders", json!("1")),
        ("headerOffset", json!(12001)),
        ("headerOffset", json!(1)),
        ("expectedSelectionSha256", json!("A".repeat(64))),
    ] {
        let mut i = input();
        i[key] = value;
        bad.push(i);
    }
    let mut calls = 0;
    for i in bad {
        assert_eq!(
            history::read(&js(i), |_, _| {
                calls += 1;
                panic!("must not call")
            })
            .unwrap_err()
            .code,
            "tool_input_invalid"
        );
    }
    assert_eq!(calls, 0);
    let mut i = input();
    i["displayName"] = json!(format!("  {}  ", "😀".repeat(100)));
    i["lessonDate"] = json!("2040-02-29");
    let prepared = history::prepare(&js(i)).unwrap();
    assert_eq!(prepared.get("displayName").units().len(), 200);
    retain(&root, "valid-edge", &prepared);
    assert!(history::encode_paper_code(&"1".into()).unwrap().is("#3"));
    let max = history::encode_paper_code(&"9007199254740991".into()).unwrap();
    assert!(
        edu_academy_sources::history::decode_code(&max)
            .unwrap()
            .is("9007199254740991")
    );
    assert_eq!(
        history::encode_paper_code(&"9007199254740992".into())
            .unwrap_err()
            .code,
        "unsafe_printed_paper_key"
    );
}
#[test]
fn metadata_and_header_scope_identity_and_duplicate_conflicts() {
    let root = root("conflicts");
    let paths = [
        "/scope/lessonDate",
        "/results/0/observation/studentKey",
        "/results/0/observation/studentName",
        "/results/0/query/periods/0",
        "/results/0/observation/observations/0/selector",
        "/results/0/observation/observations/0/fixedSourceFilter/sort_date",
        "/results/0/observation/currentFilterQualification/rows/0/studentKey",
        "/results/0/observation/currentFilterQualification/rows/0/studentName",
        "/results/0/observation/observations/0/rows/0/studentKey",
        "/results/0/observation/observations/0/rows/0/studentName",
    ];
    for (n, path) in paths.iter().enumerate() {
        let mut m = metadata();
        let value = if path.ends_with("lessonDate") {
            json!("2040-01-05")
        } else if path.ends_with("studentKey") {
            json!("99999")
        } else if path.ends_with("studentName") {
            json!("Other Learner")
        } else {
            json!("16")
        };
        *m.pointer_mut(path).unwrap() = value;
        let mut calls = 0;
        let error = history::read(&js(input()), |source, _| {
            calls += 1;
            assert_eq!(source, Source::Metadata);
            Ok(js(m.clone()))
        })
        .unwrap_err();
        assert_eq!(error.code, "dailytest_header_scope_conflict", "{path}");
        assert_eq!(calls, 1);
        retain(&root, &format!("metadata-{n}"), &error.code.into());
    }
    let mut m = metadata();
    m["results"][0]["observation"]["observations"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert_eq!(
        run(input(), m, |_| {}).unwrap_err().code,
        "dailytest_header_scope_conflict"
    );
    for (mode, expected) in [
        ("student", "dailytest_header_scope_conflict"),
        ("count", "dailytest_header_scope_conflict"),
        ("key", "dailytest_header_identity_conflict"),
        ("code", "dailytest_header_identity_conflict"),
        ("duplicate-key", "dailytest_header_identity_conflict"),
        ("duplicate-field", "dailytest_header_duplicate_field"),
    ] {
        let error = run(input(), metadata(), |h| match mode {
            "student" => h["scope"]["studentId"] = json!("99999"),
            "count" => {
                h["observation"]["rows"].as_array_mut().unwrap().pop();
            }
            "key" => h["observation"]["rows"][0]["requestedPaperKey"] = json!("9"),
            "code" => h["observation"]["rows"][0]["paperCode"] = json!("#Z"),
            "duplicate-key" => h["observation"]["rows"][1] = h["observation"]["rows"][0].clone(),
            _ => h["observation"]["rows"][0]["contentHeader"]["metadataRows"]
                .as_array_mut()
                .unwrap()
                .push(json!({"label":"시험지 제목","value":"second"})),
        })
        .unwrap_err();
        assert_eq!(error.code, expected);
        retain(&root, mode, &error.code.into());
    }
    for path in [
        "/scope/sourceNamespace",
        "/results/0/observation/coverage",
        "/results/0/observation/currentFilterQualification/observedAt",
        "/results/0/observation/currentFilterQualification/rows/0/attemptKey",
        "/results/0/observation/observations/0/operation",
        "/results/0/observation/observations/0/page",
    ] {
        let mut m = metadata();
        *m.pointer_mut(path).unwrap() = json!("malformed");
        assert!(run(input(), m, |_| {}).is_err(), "{path}");
    }
}
#[test]
fn selection_changed_offset_refusal_and_original_dual_result_cap() {
    let root = root("holds");
    let first = run(input(), metadata(), |_| {}).unwrap();
    let mut i = input();
    i["headerOffset"] = json!(4);
    i["expectedSelectionSha256"] = json!(first.get("selectionSha256").code_units());
    assert_eq!(
        run(i.clone(), metadata(), |_| panic!("paper must not open"))
            .unwrap_err()
            .code,
        "dailytest_header_offset_out_of_range"
    );
    i["headerOffset"] = json!(1);
    let mut m = metadata();
    m["results"][0]["observation"]["currentFilterQualification"]["rows"][0]["sourceScore"] =
        json!(88);
    assert_eq!(
        run(i, m, |_| panic!("paper must not open"))
            .unwrap_err()
            .code,
        "dailytest_header_selection_changed"
    );
    // Small output with many references to a single observed key, then cross cap
    // through legitimate UTF-16 sourceRowId strings repeated in both MCP halves.
    let mut large = metadata();
    let mut r = row("1", "101");
    r["sourceRowId"] = json!("x".repeat(2000));
    large["results"][0]["observation"]["currentFilterQualification"]["rows"] = json!(vec![r; 150]);
    assert_eq!(
        run(input(), large, |_| {}).unwrap_err().code,
        "dailytest_header_result_too_large"
    );
    retain(&root, "cap", &"dailytest_header_result_too_large".into());
}
#[test]
fn upstream_failure_projection_bounds_precedence_and_no_private_payloads() {
    let root = root("failures");
    for source in [Source::Metadata, Source::Paper] {
        for (n, count, expected) in [
            ("zero", json!(0), Some(0.0)),
            ("max", json!(100000), Some(100000.0)),
            ("negative", json!(-1), None),
            ("over", json!(100001), None),
            ("fraction", json!(1.5), None),
            ("string", json!("2"), None),
            ("unsafe", json!(9007199254740992u64), None),
        ] {
            let response = js(
                json!({"isError":true,"structuredContent":{"status":if source==Source::Metadata {"unavailable"}else{"blocked"},"reasonCode":"source_authentication_failed","code":"read_scope_not_admitted","sourceRequests":count,"effects":{"sourceRequests":7},"private":"PRIVATE_CANARY"}}),
            );
            let (metadata, failure) = history::source_failure(source, &response).unwrap();
            assert_eq!(
                metadata.get("sourceRequests"),
                &expected.map_or(Js::Null, Js::Num)
            );
            assert!(failure.get("category").is(if source == Source::Metadata {
                "authentication"
            } else {
                "policy_denial"
            }));
            assert!(!failure.get("automaticRetryAllowed").truthy());
            assert!(
                !String::from_utf8(encode(&failure).unwrap())
                    .unwrap()
                    .contains("PRIVATE_CANARY")
            );
            retain(
                &root,
                &format!("{source:?}-{n}"),
                &obj(&[("metadata", metadata), ("failure", failure)]),
            );
        }
    }
    for (code, expected, locator) in [
        (
            "read_scope_not_admitted",
            "read_scope_not_admitted",
            "academy-mcp:upstream-read-admission",
        ),
        (
            "source_failure_latched_no_retry",
            "source_failure_latched_no_retry",
            "academy-mcp:paper-source-latch",
        ),
        (
            "PRIVATE_CANARY",
            "source_headers_read_failed",
            "academy-mcp:paper-source-read",
        ),
    ] {
        let (m,f)=history::source_failure(Source::Paper,&js(json!({"structuredContent":{"status":"blocked","code":code,"sourceRequests":null,"effects":{"sourceRequests":null},"metrics":{"sourceRequests":2}}}))).unwrap();
        assert!(f.get("code").is(expected));
        assert!(f.get("evidenceLocator").is(locator));
        assert_eq!(m.get("sourceRequests"), &Js::Num(2.0));
    }
}

fn pin(path: &Path) -> Value {
    json!({"path":path,"sha256":digest(&fs::read(path).unwrap())})
}
fn document(path: &Path, value: Value) -> Value {
    put(path, &serde_json::to_vec(&value).unwrap());
    pin(path)
}
fn ordered_admission() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("allowedFrom", "2040-01-04".into()),
        ("allowedThrough", "2040-01-04".into()),
    ])
}
fn metadata_admission() -> Value {
    json!({"schemaVersion":"academy.metadata-admission/1","sourceNamespace":"academy.lms.metadata","teacherId":"99001","allowedFrom":"2040-01-04","allowedThrough":"2040-01-04","studentScope":"configured","relativeSelectors":["1","11","12"],"operations":["daily_history"]})
}
fn metadata_input() -> Js {
    obj(&[
        ("displayName", "Fictional Learner".into()),
        ("lessonDate", "2040-01-04".into()),
        ("purpose", "Fictional read".into()),
        ("currentRequestRef", "fictional-request".into()),
        (
            "queries",
            Js::Arr(vec![obj(&[
                ("kind", "daily_history".into()),
                ("periods", Js::Arr(vec!["11".into(), "12".into()])),
            ])]),
        ),
    ])
}
fn value(v: &Js) -> Value {
    serde_json::from_slice(&encode(v).unwrap()).unwrap()
}
fn raw_page(selector: &str, rows: Vec<Value>) -> Value {
    json!({"selector":selector,"operation":"DtResultSearch","page":1,"observedAt":AT,"relativeSelectors":["1","11","12"],"rawRows":rows})
}
fn raw_row(paper: u64, attempt: u64) -> Value {
    json!({"priNo":77001,"userName":"Fictional Learner","pNo":paper,"testingNo":attempt,"score":87.5,"_id":null,"sourceOnly":"untrusted"})
}
fn captures() -> Vec<Value> {
    let student = json!({"studentKey":"77001","studentName":"Fictional Learner"});
    vec![
        json!({"operation":"dated_students","input":value(&metadata_input()),"raw":[{"teacherKey":"99001","studentName":"Fictional Learner","date":"2040-01-04","studentKey":"77001"}]}),
        json!({"operation":"daily_page","input":{"student":student,"selector":"1"},"raw":raw_page("1",vec![raw_row(1,101),raw_row(2,102)])}),
        json!({"operation":"daily_page","input":{"student":student,"selector":"11"},"raw":raw_page("11",vec![raw_row(2,103),raw_row(3,104)])}),
        json!({"operation":"daily_page","input":{"student":student,"selector":"12"},"raw":raw_page("12",vec![])}),
    ]
}
struct Scene {
    install: PathBuf,
    state: PathBuf,
}
impl Scene {
    fn new(mode: &str, selected: &str) -> Self {
        let base = root(mode);
        for name in ["install", "state", "evidence"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(name))
                .unwrap();
        }
        let install = base.join("install");
        let state = base.join("state");
        let executable = PathBuf::from(
            std::env::var_os("S11_HISTORY_CHILD_EXECUTABLE")
                .expect("Lead-pinned finite example executable required"),
        );
        assert!(executable.is_absolute());
        assert_eq!(fs::canonicalize(&executable).unwrap(), executable);
        let expected =
            std::env::var("S11_HISTORY_CHILD_SHA256").expect("Lead must pin child SHA256");
        let bytes = fs::read(executable).unwrap();
        assert_eq!(digest(&bytes), expected);
        let bridge = install.join("bridge");
        put(&bridge, &bytes);
        fs::set_permissions(&bridge, fs::Permissions::from_mode(0o700)).unwrap();
        let launcher = install.join("launcher");
        put(&launcher, b"fictional launcher, never executed");
        let admission_path = install.join("admission.json");
        put(&admission_path, &encode(&ordered_admission()).unwrap());
        let paper_scopes = json!([{"endpoint":"paper1","studentIds":[if selected=="paper1" {"77001"}else{"77002"}]},{"endpoint":"paper2","studentIds":[if selected=="paper2" {"77001"}else{"77002"}]}]);
        let routing = document(
            &install.join("routing.json"),
            json!({"admissionPath":admission_path,"admissionSha256":pin(&admission_path)["sha256"],"paperScopes":paper_scopes}),
        );
        let owner = document(
            &install.join("owner.json"),
            json!({"schemaVersion":"academy.rust-owner/1","kind":"fictional","uid":rustix::process::geteuid().as_raw(),"installRoot":install,"stateRoot":state,"evidenceRoot":base.join("evidence")}),
        );
        let mut endpoints = serde_json::Map::new();
        for endpoint in ["dailyrecord", "paper1", "paper2"] {
            let args = vec![
                "--install-root".to_string(),
                install.to_str().unwrap().into(),
                "--endpoint".into(),
                endpoint.into(),
            ];
            let mut artifact = pin(&bridge);
            artifact["role"] = json!("entry");
            let identity = document(
                &install.join(format!("identity-{endpoint}.json")),
                json!({"schemaVersion":"academy.rust-endpoint/1","endpoint":endpoint,"semanticContractVersion":"s11-fictional","launchArgumentsSha256":digest(&serde_json::to_vec(&args).unwrap()),"artifacts":[artifact]}),
            );
            endpoints.insert(
                endpoint.into(),
                json!({"executable":pin(&bridge),"identity":identity,"args":args}),
            );
        }
        let endpoints = document(&install.join("endpoints.json"), Value::Object(endpoints));
        let original: Value = serde_json::from_slice(include_bytes!(
            "../../../migration/mcp-runtime-reference-v1/frozen/inventory/registry.json"
        ))
        .unwrap();
        let entries = original
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                [
                    "academy_student_metadata_get",
                    "academy_paper_headers",
                    "academy_dailytest_history_with_headers",
                ]
                .iter()
                .any(|n| e["upstreamName"] == *n)
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4);
        let registry = document(&install.join("registry.json"), json!(entries));
        let mut metadata_captures = captures();
        if mode == "metadata-failure" {
            metadata_captures[0]["error"] = json!("source_authentication_failed");
        }
        let mut policy = metadata_admission();
        if mode == "metadata-denied" {
            policy["relativeSelectors"] = json!(["1"]);
        }
        let mut papers = vec![];
        for (key, code) in [("1", "#3"), ("2", "#4"), ("3", "#5")] {
            let mut capture = json!({"operation":"paper_header","input":{"studentId":"77001","paperCode":code},"raw":{"paperKey":key,"paperCode":code,"studentId":"77001","observedAt":AT,
                "sourceText":format!("Fictional raw header for {key}"),"metadataRows":[{"label":"시험지 제목","value":"  Source 발전  "},{"label":"평가구분","value":"DT/ZT"},{"label":"총 문제수","value":"04"}]}});
            if mode == "paper-denied" {
                capture["error"] = json!("read_scope_not_admitted");
            }
            papers.push(capture);
        }
        let mut by_endpoint = serde_json::Map::new();
        by_endpoint.insert("dailyrecord".into(), json!(metadata_captures));
        by_endpoint.insert(selected.into(), json!(papers));
        let config = install.join("fictional-config.json");
        document(
            &config,
            json!({"fictionalOnly":true,"metadataAdmission":policy,"students":[{"studentId":"77001","displayName":"Fictional Learner"}],"managedNames":["Fictional Learner"],
            "paperStudents":{"dailyrecord":[],"paper1":paper_scopes[0]["studentIds"],"paper2":paper_scopes[1]["studentIds"]},"captures":by_endpoint}),
        );
        let mut artifact = pin(&config);
        artifact["role"] = json!("source");
        let build = document(
            &install.join("build.json"),
            json!({"schemaVersion":"academy.rust-build/1","sourceRevision":"0".repeat(40),"semanticContractVersion":"s11-fictional","cargoLockSha256":"0".repeat(64),"artifacts":[artifact]}),
        );
        document(
            &install.join("release.json"),
            json!({"schemaVersion":"academy.mcp-release/3","generation":"s11-history-fictional","bridge":pin(&bridge),"launcher":pin(&launcher),"build":build,"routing":routing,"owner":owner,"registry":registry,"endpoints":endpoints}),
        );
        Self { install, state }
    }
    fn graph(&self) -> ReleaseGraph {
        ReleaseGraph::open(&self.install).unwrap()
    }
    fn receipts(&self, suffix: &str) -> Vec<Js> {
        let path = self.install.parent().unwrap().join("evidence/receipts");
        match fs::read_dir(path) {
            Ok(entries) => entries
                .map(|e| e.unwrap().path())
                .filter(|p| p.file_name().unwrap().to_str().unwrap().ends_with(suffix))
                .map(|p| parse(&fs::read(p).unwrap()).unwrap())
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(e) => panic!("receipt failure: {e}"),
        }
    }
}
struct OwnedControl;
impl Control for OwnedControl {
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
fn invoke(scene: &Scene, input: &Js) -> Js {
    let mut gate = ConnectionGate::default();
    let mut control = OwnedControl;
    match dispatch::call(
        &scene.graph(),
        &mut gate,
        "academy_dailytest_history_with_headers",
        input,
        None,
        &Js::Num(1.0),
        &mut control,
        Limits {
            work: Duration::from_secs(10),
            ..Limits::default()
        },
    )
    .unwrap()
    {
        Reply::Tool(v) => v,
        Reply::ReceiptFailure { .. } => panic!("receipt evidence required"),
    }
}
fn lines(path: &Path) -> Vec<Js> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|s| parse(s.as_bytes()).unwrap())
        .collect()
}
#[test]
fn actual_source_drivers_route_independently_and_close_before_open() {
    process::install_binary_subreaper().unwrap();
    for selected in ["paper1", "paper2"] {
        let scene = Scene::new(&format!("normal-{selected}"), selected);
        let out = invoke(&scene, &js(input()));
        retain(&scene.state, "parent-output", &out);
        assert!(
            out.get("structuredContent").get("status").is("observed"),
            "{out:?}"
        );
        let events = lines(&scene.state.join("history-events.jsonl"));
        let lifecycle = events
            .iter()
            .filter(|e| !e.get("event").is("source"))
            .map(|e| (e.get("event").code_units(), e.get("endpoint").code_units()))
            .collect::<Vec<_>>();
        assert_eq!(
            lifecycle,
            vec![
                ("open".into(), "dailyrecord".into()),
                ("close".into(), "dailyrecord".into()),
                ("open".into(), selected.into()),
                ("close".into(), selected.into())
            ]
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| e.get("event").is("source"))
                .count(),
            7
        );
        for endpoint in ["dailyrecord", selected] {
            let requests = lines(&scene.state.join(format!("{endpoint}-requests.jsonl")));
            let tools = requests
                .iter()
                .filter(|r| r.get("method").is("tools/call"))
                .map(|r| r.get("params").get("name").code_units())
                .collect::<Vec<_>>();
            assert_eq!(
                tools,
                if endpoint == "dailyrecord" {
                    vec![
                        "academy_endpoint_identity",
                        "academy_runtime_read_admission",
                        "academy_student_metadata_get",
                    ]
                } else {
                    vec!["academy_endpoint_identity", "academy_paper_headers"]
                }
            );
        }
        let cleanup = scene.receipts("-cleanup.json");
        assert_eq!(cleanup.len(), 2);
        for c in cleanup {
            assert!(c.get("qualified").truthy());
            let pid = match c.get("pid") {
                Js::Num(n) => *n as u32,
                _ => panic!("owned pid required"),
            };
            assert!(
                !process::Identity::read(pid)
                    .is_some_and(|p| p.start.to_string() == c.get("startTicks").code_units())
            );
        }
        let pure = run(input(), metadata(), |h| {
            for r in h["observation"]["rows"].as_array_mut().unwrap() {
                let raw = format!(
                    "Fictional raw header for {}",
                    r["requestedPaperKey"].as_str().unwrap()
                );
                r["contentHeader"]["sourceSha256"] = json!(digest(raw.as_bytes()));
            }
        })
        .unwrap();
        assert_eq!(
            encode(&pure).unwrap(),
            encode(out.get("structuredContent")).unwrap()
        );
    }
}
#[test]
fn actual_denials_and_strict_input_do_not_retry_or_open_extra_children() {
    process::install_binary_subreaper().unwrap();
    for (mode, source, count, children) in [
        ("metadata-denied", "metadata", 0.0, 1),
        ("metadata-failure", "metadata", 1.0, 1),
        ("paper-denied", "paper", 1.0, 2),
    ] {
        let scene = Scene::new(mode, "paper1");
        let out = invoke(&scene, &js(input()));
        retain(&scene.state, "parent-output", &out);
        let data = out.get("structuredContent");
        assert!(out.get("isError").truthy(), "{out:?}");
        assert!(data.get("failedSource").is(source), "{out:?}");
        assert_eq!(data.get("sourceRequests"), &Js::Num(count));
        assert_eq!(scene.receipts("-cleanup.json").len(), children);
        let events = lines(&scene.state.join("history-events.jsonl"));
        assert_eq!(
            events.iter().filter(|e| e.get("event").is("open")).count(),
            children
        );
        if source == "metadata" {
            assert!(!scene.state.join("paper1-requests.jsonl").exists());
        }
    }
    let scene = Scene::new("strict-dispatch", "paper1");
    let mut bad = input();
    bad["periods"] = json!(["11", "11"]);
    let out = invoke(&scene, &js(bad));
    assert!(out.get("isError").truthy());
    assert!(!scene.state.join("history-events.jsonl").exists());
    assert!(scene.receipts("-cleanup.json").is_empty());
}

struct RawPort {
    captures: std::collections::VecDeque<Value>,
}
impl RawPort {
    fn take(&mut self) -> edu_academy_sources::data::Result<Js> {
        let c = self
            .captures
            .pop_front()
            .expect("finite source inputs; no retry");
        if let Some(code) = c.get("error") {
            return Err(edu_academy_sources::data::Error::error(
                code.as_str().unwrap(),
            ));
        }
        Ok(js(c["raw"].clone()))
    }
}
impl edu_academy_sources::history::Port for RawPort {
    fn dated_students(&mut self, _: &Js) -> edu_academy_sources::data::Result<Js> {
        self.take()
    }
    fn daily_page(&mut self, _: &Js, _: &Js) -> edu_academy_sources::data::Result<Js> {
        self.take()
    }
    fn paper_header(&mut self, _: &Js, _: &Js) -> edu_academy_sources::data::Result<Js> {
        self.take()
    }
}
#[test]
fn actual_source_dated_identity_selector_and_row_checks_latch_failures() {
    let root = root("source-conflicts");
    for (mode, expected) in [
        ("teacher", "native_dated_name_scope_conflict"),
        ("date", "native_dated_name_scope_conflict"),
        ("name", "native_dated_name_scope_conflict"),
        ("ambiguous", "native_dated_name_ambiguous"),
        ("missing", "native_dailyrecord_occurrence_unverified"),
        ("configured-key", "native_dated_name_scope_conflict"),
        ("selector", "daily_history_selector_drift"),
        ("page-selector", "native_metadata_selector_conflict"),
        ("row-name", "student_search_identity_conflict"),
        ("duplicate", "duplicate_student_search_row"),
    ] {
        let mut c = captures();
        match mode {
            "teacher" => c[0]["raw"][0]["teacherKey"] = json!("99999"),
            "date" => c[0]["raw"][0]["date"] = json!("2040-01-05"),
            "name" => c[0]["raw"][0]["studentName"] = json!("Other Learner"),
            "ambiguous" => {
                let r = c[0]["raw"][0].clone();
                c[0]["raw"].as_array_mut().unwrap().push(r);
            }
            "missing" => c[0]["raw"] = json!([]),
            "configured-key" => c[0]["raw"][0]["studentKey"] = json!("99999"),
            "selector" => c[1]["raw"]["relativeSelectors"] = json!(["1"]),
            "page-selector" => c[2]["raw"]["selector"] = json!("16"),
            "row-name" => c[1]["raw"]["rawRows"][0]["userName"] = json!("Other Learner"),
            _ => {
                let r = c[1]["raw"]["rawRows"][0].clone();
                c[1]["raw"]["rawRows"].as_array_mut().unwrap().push(r);
            }
        }
        let mut source = edu_academy_sources::history::History::new(
            RawPort { captures: c.into() },
            ordered_admission(),
            js(metadata_admission()),
            vec![js(
                json!({"studentId":"77001","displayName":"Fictional Learner"}),
            )],
            vec!["Fictional Learner".into()],
            vec!["77001".into()],
        )
        .unwrap();
        let e = source.metadata(&metadata_input()).unwrap_err();
        assert_eq!(e.message, expected, "{mode}");
        let requests = source.source_requests();
        assert_eq!(
            source.metadata(&metadata_input()).unwrap_err().message,
            "source_failure_latched_no_retry"
        );
        assert_eq!(source.source_requests(), requests);
        retain(
            &root,
            mode,
            &obj(&[
                ("code", e.message.into()),
                ("sourceRequests", requests.into()),
            ]),
        );
    }
}
