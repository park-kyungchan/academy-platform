//! SOURCE-ONLY: every test below is UNEXECUTED by the source author.
//! Root supplies a pre-existing canonical absolute root and pinned Cargo example.
//! All fictional child files and Store observations remain for independent readback.
use edu_academy_core::{
    instant::{Grammar, epoch_ms},
    workflow::{
        review::FIELDS,
        store::{Kind, Store},
    },
};
use edu_academy_mcp::{
    Js,
    dailyrecord_read::{self as read, InvokeFailure},
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
fn now() -> i64 {
    epoch_ms(&"2040-01-04T12:00:00.000Z".into(), Grammar::DailyRecord).unwrap()
}
fn input() -> Js {
    obj(&[
        ("displayName", "  Fictional Learner  ".into()),
        ("purpose", "Fictional read".into()),
        ("currentRequestRef", "fictional-request".into()),
        ("lessonDate", "2040-01-04".into()),
    ])
}
fn routing_admission() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("allowedFrom", "2040-01-04".into()),
        ("allowedThrough", "2040-01-04".into()),
    ])
}
fn case_root(label: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var_os("S11_PARENT_GET_TEST_ROOT").expect("explicit Root test root required"),
    );
    assert!(base.is_absolute());
    assert_eq!(fs::canonicalize(&base).unwrap(), base);
    assert!(edu_academy_mcp::roots::canonical_absolute(&base));
    let path = base.join(format!(
        "pure-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    path
}
fn retain(root: &Path, label: &str, v: &Js) {
    put(&root.join(format!("{label}.json")), &encode(v).unwrap());
}
fn resolved() -> Js {
    obj(&[
        ("status", "resolved".into()),
        ("studentKey", "77001".into()),
        ("displayName", "Fictional Learner".into()),
    ])
}
fn intake(context: &Js, durable: bool) -> Js {
    let mut target = context.get("scope").clone();
    for (k, v) in [
        ("courseId", "course"),
        ("membershipId", "membership"),
        ("recordId", "record"),
    ] {
        target.set(k, v.into());
    }
    obj(&[
        ("userContext", context.clone()),
        ("status", "interview_required".into()),
        ("sourceAvailability", "observed".into()),
        ("blockers", Js::Arr(vec![])),
        (
            "source",
            obj(&[
                ("observedAt", "2040-01-04T12:00:00.000Z".into()),
                ("sourceRevision", "1".repeat(64).into()),
                ("wireDigest", "2".repeat(64).into()),
                ("transmissionState", "already_sent".into()),
            ]),
        ),
        (
            "fields",
            Js::Arr(
                FIELDS
                    .iter()
                    .map(|f| {
                        obj(&[
                            ("field", (*f).into()),
                            ("selected", true.into()),
                            ("disposition", "preserve_existing".into()),
                            ("target", target.clone()),
                            (
                                "before",
                                obj(&[
                                    ("state", "known".into()),
                                    ("value", "preserved source text".into()),
                                    ("evidenceRefs", Js::Arr(vec!["fictional:evidence".into()])),
                                ]),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "interviews",
            Js::Arr(vec![obj(&[
                ("field", "lessonMemo".into()),
                ("reasonCode", "today_student_specific_memo_required".into()),
            ])]),
        ),
        (
            "observationHandle",
            "dailyrecord-observation:fictional".into(),
        ),
        (
            "observationPersistence",
            if durable {
                "owner_durable"
            } else {
                "process_local"
            }
            .into(),
        ),
    ])
}
fn put(path: &Path, bytes: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
fn pin(path: &Path) -> Value {
    json!({"path":path,"sha256":digest(&fs::read(path).unwrap())})
}
fn document(path: &Path, value: Value) -> Value {
    put(path, &serde_json::to_vec(&value).unwrap());
    pin(path)
}
struct Scene {
    install: PathBuf,
    state: PathBuf,
}
impl Scene {
    fn new(label: &str, routing: &Js, endpoint: &str) -> Self {
        let selected = PathBuf::from(
            std::env::var_os("S11_PARENT_GET_TEST_ROOT")
                .expect("Root must supply S11_PARENT_GET_TEST_ROOT"),
        );
        assert!(selected.is_absolute());
        assert_eq!(fs::canonicalize(&selected).unwrap(), selected);
        assert!(edu_academy_mcp::roots::canonical_absolute(&selected));
        let base = selected.join(format!(
            "parent-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .expect("fresh fictional child required");
        for name in ["install", "evidence", "state"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(name))
                .unwrap();
        }
        let install = base.join("install");
        let state = base.join("state");
        let bridge = install.join("bridge");
        let executable = PathBuf::from(
            std::env::var_os("S11_PARENT_GET_CHILD_EXECUTABLE")
                .expect("Root must pin the Cargo-native example executable"),
        );
        assert!(executable.is_absolute());
        assert_eq!(fs::canonicalize(&executable).unwrap(), executable);
        let expected =
            std::env::var("S11_PARENT_GET_CHILD_SHA256").expect("Root must pin executable SHA256");
        let bytes = fs::read(&executable).unwrap();
        assert_eq!(digest(&bytes), expected);
        put(&bridge, &bytes);
        fs::set_permissions(&bridge, fs::Permissions::from_mode(0o700)).unwrap();
        document(
            &install.join("fictional-config.json"),
            json!({"fictionalOnly":true,"mode":label}),
        );
        let launcher = install.join("launcher");
        put(&launcher, b"fictional launcher; never launched");
        let admission_path = install.join("admission.json");
        put(&admission_path, &encode(routing).unwrap());
        let routing = document(
            &install.join("routing.json"),
            json!({"admissionPath":admission_path,"admissionSha256":pin(&admission_path)["sha256"]}),
        );
        let owner = document(
            &install.join("owner.json"),
            json!({"schemaVersion":"academy.rust-owner/1","kind":"fictional",
            "uid":rustix::process::geteuid().as_raw(),"installRoot":install,"evidenceRoot":base.join("evidence"),"stateRoot":state}),
        );
        let args = vec![
            "--install-root".to_owned(),
            install.to_str().unwrap().into(),
            "--endpoint".into(),
            endpoint.into(),
        ];
        let mut artifact = pin(&bridge);
        artifact["role"] = json!("entry");
        let identity = document(
            &install.join("identity.json"),
            json!({"schemaVersion":"academy.rust-endpoint/1","endpoint":endpoint,
            "semanticContractVersion":"s11-fictional","launchArgumentsSha256":digest(&serde_json::to_vec(&args).unwrap()),"artifacts":[artifact]}),
        );
        let mut endpoints_map = serde_json::Map::new();
        endpoints_map.insert(
            endpoint.into(),
            json!({"executable":pin(&bridge),"identity":identity,"args":args}),
        );
        let endpoints = document(
            &install.join("endpoints.json"),
            Value::Object(endpoints_map),
        );
        // The six actual pinned endpoint schemas, unchanged; the admission probe
        // is endpoint-local, just as in existing RpcSession inventory.
        let original: Value = serde_json::from_slice(include_bytes!(
            "../../../migration/mcp-runtime-reference-v1/frozen/inventory/registry.json"
        ))
        .unwrap();
        let names = [
            "academy_dailyrecord_intake",
            "academy_dailyrecord_review",
            "academy_dailyrecord_save",
            "academy_dailyrecord_reconcile",
            "academy_dailyrecord_execution_status",
            "academy_student_resolve",
            "academy_dailyrecord_get",
        ];
        let mut entries = Vec::new();
        for name in names {
            let mut e = original
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["upstreamName"] == name)
                .expect("pinned actual tool required")
                .clone();
            e["endpoint"] = json!(endpoint);
            entries.push(e);
        }
        let registry = document(&install.join("registry.json"), json!(entries));
        let source = install.join("source.bin");
        put(&source, b"fictional pinned source");
        let mut artifact = pin(&source);
        artifact["role"] = json!("source");
        let build = document(
            &install.join("build.json"),
            json!({"schemaVersion":"academy.rust-build/1","sourceRevision":"0".repeat(40),
            "semanticContractVersion":"s11-fictional","cargoLockSha256":"0".repeat(64),"artifacts":[artifact]}),
        );
        let pointer=serde_json::to_vec(&json!({"schemaVersion":"academy.mcp-release/3","generation":"business-fictional-v1",
            "bridge":pin(&bridge),"launcher":pin(&launcher),"build":build,"routing":routing,"owner":owner,"registry":registry,"endpoints":endpoints})).unwrap();
        put(&install.join("release.json"), &pointer);
        Self { install, state }
    }
    fn graph(&self) -> ReleaseGraph {
        ReleaseGraph::open(&self.install).unwrap()
    }
}

#[test]
fn read_preserves_observed_interviews_and_durable_vs_process_local_handle() {
    let root = case_root("observed");
    for durable in [false, true] {
        let p = read::prepare(&input(), &routing_admission(), now()).unwrap();
        let mut order = vec![];
        let out = read::read(
            p,
            |name, args| {
                order.push(name.to_owned());
                if name == "academy_student_resolve" {
                    assert!(args.get("displayName").is("Fictional Learner"));
                    return Ok(resolved());
                }
                assert_eq!(args.get("context").get("selectedFields").array().len(), 6);
                assert!(
                    args.get("context")
                        .get("memoAnswer")
                        .get("kind")
                        .is("not_supplied")
                );
                assert_eq!(args.get("context").get("facts"), &Js::Arr(vec![]));
                Ok(intake(args.get("context"), durable))
            },
            now,
        )
        .unwrap();
        assert_eq!(
            order,
            ["academy_student_resolve", "academy_dailyrecord_intake"]
        );
        assert!(out.get("status").is("observed"));
        assert!(
            out.get("preparation")
                .get("status")
                .is("interview_required")
        );
        assert!(
            out.get("preparation")
                .get("requiredOnlyForAuthoring")
                .truthy()
        );
        assert!(out.get("transmissionState").is("already_sent"));
        assert_eq!(out.get("authoringHandle") == &Js::Null, !durable);
        if durable {
            assert!(
                out.get("authoringHandle")
                    .get("boundTo")
                    .get("currentRequestRef")
                    .is("fictional-request")
            );
        }
        assert_eq!(
            out.props()
                .iter()
                .map(|(k, _)| String::from_utf16(k).unwrap())
                .collect::<Vec<_>>(),
            [
                "displayName",
                "lessonDate",
                "timeZone",
                "purpose",
                "currentRequestRef",
                "academyWrites",
                "parentSends",
                "automaticRetryAllowed",
                "execution",
                "sourceContentIsUntrustedData",
                "status",
                "sourceAvailability",
                "scope",
                "route",
                "fields",
                "source",
                "preparation",
                "transmissionState",
                "authoringHandle"
            ]
        );
        retain(
            &root,
            if durable { "durable" } else { "process-local" },
            &out,
        );
    }
}
#[test]
fn strict_input_and_owner_date_holds_make_zero_calls() {
    let root = case_root("holds");
    for key in [
        "teacherId",
        "studentKey",
        "attribution",
        "admission",
        "proposals",
        "actor",
    ] {
        let mut v = input();
        v.set(key, "forged".into());
        assert!(read::prepare(&v, &routing_admission(), now()).is_err());
    }
    for v in ["2040-02-30", "2100-02-29", ""] {
        let mut i = input();
        i.set("lessonDate", v.into());
        assert!(read::prepare(&i, &routing_admission(), now()).is_err());
    }
    for key in ["purpose", "currentRequestRef"] {
        let mut i = input();
        i.set(key, "x".repeat(257).into());
        assert!(read::prepare(&i, &routing_admission(), now()).is_err());
    }
    let mut astral = input();
    astral.set("displayName", "😀".repeat(101).into());
    assert!(read::prepare(&astral, &routing_admission(), now()).is_err());
    let mut future = input();
    future.set("lessonDate", "2040-01-05".into());
    for (label, i, a, reason) in [
        (
            "missing",
            input(),
            Js::Null,
            "dailyrecord_owner_read_admission_not_configured",
        ),
        (
            "range",
            future,
            routing_admission(),
            "lesson_date_out_of_admitted_scope",
        ),
    ] {
        let out = read::read(
            read::prepare(&i, &a, now()).unwrap(),
            |_, _| panic!("hold must not call"),
            now,
        )
        .unwrap();
        assert!(out.get("reasonCode").is(reason));
        retain(&root, label, &out);
    }
}
#[test]
fn resolution_and_intake_contract_failures_stop_without_retry() {
    let root = case_root("contracts");
    for mode in [
        "ambiguous",
        "malformed",
        "different",
        "resolve_tool",
        "intake_tool",
        "duplicate",
        "scope",
        "future",
        "stale",
        "blocker",
        "bad_target",
        "bad_before",
        "bad_hash",
        "bad_interview",
    ] {
        let mut calls = 0;
        let out = read::read(
            read::prepare(&input(), &routing_admission(), now()).unwrap(),
            |name, args| {
                calls += 1;
                if name == "academy_student_resolve" {
                    return match mode {
                        "ambiguous" => Ok(obj(&[
                            ("status", "unavailable".into()),
                            ("reasonCode", "student_name_ambiguous".into()),
                        ])),
                        "malformed" => Ok(obj(&[
                            ("status", "resolved".into()),
                            ("studentKey", "0".into()),
                            ("displayName", "Fictional Learner".into()),
                        ])),
                        "different" => {
                            let mut r = resolved();
                            r.set("displayName", "Another".into());
                            Ok(r)
                        }
                        "resolve_tool" => Err(read::tool_failure(&obj(&[
                            ("isError", true.into()),
                            (
                                "structuredContent",
                                obj(&[("code", "student_name_not_admitted".into())]),
                            ),
                        ]))),
                        _ => Ok(resolved()),
                    };
                }
                if mode == "intake_tool" {
                    return Err(InvokeFailure::Tool {
                        failure: read::public_failure_from_data(&obj(&[
                            ("code", "source_authentication_failed".into()),
                            ("category", "forged".into()),
                            ("evidenceLocator", "forged".into()),
                        ])),
                        occurrence: "00000000-0000-4000-8000-000000000001".into(),
                    });
                }
                let mut v = intake(args.get("context"), true);
                match mode {
                    "duplicate" => {
                        let mut fields = v.get("fields").array().to_vec();
                        fields[5] = fields[0].clone();
                        v.set("fields", Js::Arr(fields));
                    }
                    "scope" => {
                        let mut c = v.get("userContext").clone();
                        let mut s = c.get("scope").clone();
                        s.set("studentId", "77002".into());
                        c.set("scope", s);
                        v.set("userContext", c);
                    }
                    "future" | "stale" | "bad_hash" => {
                        let mut s = v.get("source").clone();
                        if mode == "bad_hash" {
                            s.set("wireDigest", "invalid".into());
                        } else {
                            s.set(
                                "observedAt",
                                if mode == "future" {
                                    "2040-01-04T12:00:00.001Z"
                                } else {
                                    "2040-01-04T11:54:59.999Z"
                                }
                                .into(),
                            );
                        }
                        v.set("source", s);
                    }
                    "blocker" => v.set("blockers", Js::Arr(vec!["source_held".into()])),
                    "bad_target" | "bad_before" => {
                        let mut f = v.get("fields").array().to_vec();
                        f[0].set(
                            if mode == "bad_target" {
                                "target"
                            } else {
                                "before"
                            },
                            obj(&[]),
                        );
                        v.set("fields", Js::Arr(f));
                    }
                    "bad_interview" => v.set(
                        "interviews",
                        Js::Arr(vec![obj(&[
                            ("field", "unknown".into()),
                            ("reasonCode", "held".into()),
                        ])]),
                    ),
                    _ => (),
                }
                Ok(v)
            },
            now,
        )
        .unwrap();
        let expected = match mode {
            "ambiguous" => "student_name_ambiguous",
            "different" => "student_name_scope_conflict",
            "malformed" | "resolve_tool" => "student_resolve_unqualified_no_automatic_retry",
            "scope" => "dailyrecord_read_scope_conflict",
            "future" | "stale" => "stale_source",
            "blocker" => "source_held",
            _ => "dailyrecord_source_unqualified_no_automatic_retry",
        };
        assert!(out.get("reasonCode").is(expected), "{mode}: {out:?}");
        assert_eq!(
            calls,
            if ["ambiguous", "malformed", "different", "resolve_tool"].contains(&mode) {
                1
            } else {
                2
            }
        );
        if mode == "intake_tool" {
            assert!(out.get("failure").get("category").is("authentication"));
            assert!(!encode(&out).unwrap().windows(6).any(|x| x == b"forged"));
        }
        retain(&root, mode, &out);
    }
}
#[test]
fn seoul_today_rollover_leap_and_freshness_inclusive_edges() {
    let root = case_root("clock");
    for (instant, date) in [
        ("2040-02-28T14:59:59.999Z", "2040-02-28"),
        ("2040-02-28T15:00:00.000Z", "2040-02-29"),
        ("2040-02-29T15:00:00.000Z", "2040-03-01"),
        ("2100-02-28T15:00:00.000Z", "2100-03-01"),
    ] {
        let ms = epoch_ms(&instant.into(), Grammar::DailyRecord).unwrap();
        assert_eq!(read::academy_business_date(ms).unwrap(), date);
        let mut i = input();
        i.set("lessonDate", "today".into());
        let mut a = routing_admission();
        a.set("allowedFrom", Js::Null);
        a.set("allowedThrough", Js::Null);
        let out = read::read(
            read::prepare(&i, &a, ms).unwrap(),
            |_, _| {
                Ok(obj(&[
                    ("status", "unavailable".into()),
                    ("reasonCode", "student_name_not_found".into()),
                ]))
            },
            || ms,
        )
        .unwrap();
        assert!(out.get("lessonDate").is(date));
        retain(&root, date, &out);
    }
    for age in [0, 300000] {
        let out = read::read(
            read::prepare(&input(), &routing_admission(), now()).unwrap(),
            |name, args| {
                Ok(if name == "academy_student_resolve" {
                    resolved()
                } else {
                    intake(args.get("context"), false)
                })
            },
            || now() + age,
        )
        .unwrap();
        assert!(out.get("status").is("observed"));
        retain(&root, &format!("age-{age}"), &out);
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
fn receipts(scene: &Scene, suffix: &str) -> Vec<Js> {
    fs::read_dir(scene.install.parent().unwrap().join("evidence"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_str().unwrap().ends_with(suffix))
        .map(|p| parse(&fs::read(p).unwrap()).unwrap())
        .collect()
}
fn invoke(scene: &Scene, i: &Js, work: Duration) -> Js {
    let mut gate = ConnectionGate::default();
    let mut control = OwnedControl;
    let reply = dispatch::call(
        &scene.graph(),
        &mut gate,
        "academy_dailyrecord_get",
        i,
        Some(&obj(&[
            ("name", "parent-test".into()),
            ("version", "1".into()),
        ])),
        &Js::Num(1.0),
        &mut control,
        Limits {
            work,
            ..Limits::default()
        },
    )
    .unwrap();
    match reply {
        Reply::Tool(v) => v,
        Reply::ReceiptFailure { .. } => panic!("receipt evidence required"),
    }
}
#[test]
fn actual_same_child_resolve_intake_store_and_failure_cleanup() {
    // Dedicated finite test process only. Root must use --test-threads=1.
    process::install_binary_subreaper().unwrap();
    for mode in ["normal", "close_after_intake", "stall_intake"] {
        let scene = Scene::new(mode, &routing_admission(), "dailyrecord");
        let out = invoke(
            &scene,
            &input(),
            if mode == "stall_intake" {
                Duration::from_millis(500)
            } else {
                Duration::from_secs(10)
            },
        );
        put(
            &scene.state.join("parent-output.json"),
            &encode(&out).unwrap(),
        );
        let calls = fs::read_to_string(scene.state.join("child-requests.jsonl"))
            .unwrap()
            .lines()
            .map(|s| parse(s.as_bytes()).unwrap())
            .collect::<Vec<_>>();
        assert!(!calls.is_empty());
        assert!(calls.iter().all(|v| v.get("pid") == calls[0].get("pid")));
        assert!(
            calls
                .iter()
                .all(|v| v.get("startTicks") == calls[0].get("startTicks"))
        );
        let tools = calls
            .iter()
            .filter(|v| v.get("request").get("method").is("tools/call"))
            .collect::<Vec<_>>();
        assert_eq!(
            tools
                .iter()
                .map(|v| v.get("request").get("params").get("name").code_units())
                .collect::<Vec<_>>(),
            [
                "academy_endpoint_identity",
                "academy_runtime_read_admission",
                "academy_student_resolve",
                "academy_dailyrecord_intake"
            ]
        );
        let forwarded = tools
            .last()
            .unwrap()
            .get("request")
            .get("params")
            .get("arguments")
            .clone();
        assert!(
            forwarded
                .get("attribution")
                .get("observedBy")
                .is("neutral-bridge")
        );
        assert!(
            forwarded
                .get("attribution")
                .get("client")
                .get("name")
                .is("parent-test")
        );
        let cleanup = receipts(&scene, "-cleanup.json");
        assert_eq!(cleanup.len(), 1);
        assert!(cleanup[0].get("qualified").truthy());
        let pid = match cleanup[0].get("pid") {
            Js::Num(n) => *n as u32,
            _ => panic!("pid required"),
        };
        assert!(
            !process::Identity::read(pid)
                .is_some_and(|p| p.start.to_string() == cleanup[0].get("startTicks").code_units())
        );
        let data = out.get("structuredContent");
        if mode == "stall_intake" {
            assert!(data.get("code").is("upstream_deadline_exceeded"), "{out:?}");
            assert!(
                receipts(&scene, "-failure.json")[0]
                    .get("localWorkflowEffects")
                    .get("intakeIssued")
                    .truthy()
            );
        } else {
            let store = Store::open(&scene.state.join("workflow"), &scene.state).unwrap();
            let files = fs::read_dir(store.root().join("observation"))
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>();
            assert_eq!(files.len(), 1);
            let stored = parse(&fs::read(&files[0]).unwrap()).unwrap();
            let record = stored.get("record").clone();
            assert_eq!(record.get("createdBy"), forwarded.get("attribution"));
            assert!(record.get("requestRef").is("fictional-request"));
            if mode == "normal" {
                assert!(data.get("status").is("observed"), "{out:?}");
                assert!(
                    data.get("preparation")
                        .get("status")
                        .is("interview_required")
                );
                let handle = data.get("authoringHandle").get("observationHandle");
                assert_eq!(
                    store.get(Kind::Observation, &handle.code_units()).unwrap(),
                    record
                );
                let child = fs::read_to_string(scene.state.join("child-responses.jsonl"))
                    .unwrap()
                    .lines()
                    .map(|s| parse(s.as_bytes()).unwrap())
                    .last()
                    .unwrap();
                let original = child.get("result").get("structuredContent");
                assert_eq!(
                    data.get("fields")
                        .array()
                        .iter()
                        .map(|f| f.get("before"))
                        .collect::<Vec<_>>(),
                    original
                        .get("fields")
                        .array()
                        .iter()
                        .map(|f| f.get("before"))
                        .collect::<Vec<_>>()
                );
                let receipt = receipts(&scene, "-result.json");
                assert_eq!(receipt[0].get("steps").array().len(), 2);
                assert_eq!(
                    receipt[0]
                        .get("localWorkflowEffects")
                        .get("observationHandle"),
                    handle
                );
            } else {
                assert!(
                    data.get("reasonCode")
                        .is("dailyrecord_source_unqualified_no_automatic_retry")
                );
                assert!(
                    receipts(&scene, "-result.json")[0]
                        .get("localWorkflowEffects")
                        .get("intakeIssued")
                        .truthy()
                );
            }
        }
    }
    let scene = Scene::new("owner_hold", &Js::Null, "dailyrecord");
    let held = invoke(&scene, &input(), Duration::from_secs(1));
    assert!(
        held.get("structuredContent")
            .get("reasonCode")
            .is("dailyrecord_owner_read_admission_not_configured")
    );
    assert!(!scene.state.join("child-requests.jsonl").exists());
    assert!(receipts(&scene, "-cleanup.json").is_empty());
}

#[test]
fn preserved_original_protocol_get_body_and_strict_rejection() {
    // Pure consumer-contract comparison using immutable actual original id8.
    // Intermediate port values are DERIVED from that final body, not an
    // unchanged-original intake execution or a same-child/public-Client proof.
    let root = case_root("original");
    let raw = include_bytes!(
        "../../../migration/mcp-runtime-reference-v1/frozen/protocol/commands/neutral/stdout.bin"
    );
    assert_eq!(
        digest(raw),
        "8998846f023154baf767f5d88748e0f86cee17268b4da98f1ebe026bc69813ec"
    );
    let rows = raw
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| parse(line).unwrap())
        .collect::<Vec<_>>();
    let expected = rows
        .iter()
        .find(|r| r.get("id") == &Js::Num(8.0))
        .unwrap()
        .get("result")
        .get("structuredContent");
    let stdin = include_bytes!(
        "../../../migration/mcp-runtime-reference-v1/frozen/protocol/commands/neutral/stdin.bin"
    );
    assert_eq!(
        digest(stdin),
        "31969c2844a5b02118f6f60fdd9984202bcb587f2705629ecb2ec3817e8299d7"
    );
    let requests = stdin
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| parse(line).unwrap())
        .collect::<Vec<_>>();
    let args = requests
        .iter()
        .find(|r| r.get("id") == &Js::Num(8.0))
        .unwrap()
        .get("params")
        .get("arguments");
    let mut admission = routing_admission();
    admission.set("allowedFrom", Js::Null);
    admission.set("allowedThrough", Js::Null);
    let observed = epoch_ms(
        expected.get("source").get("observedAt"),
        Grammar::DailyRecord,
    )
    .unwrap();
    let out = read::read(
        read::prepare(args, &admission, observed).unwrap(),
        |name, input| {
            Ok(if name == "academy_student_resolve" {
                obj(&[
                    ("status", "resolved".into()),
                    ("studentKey", expected.get("scope").get("studentId").clone()),
                    ("displayName", expected.get("displayName").clone()),
                ])
            } else {
                obj(&[
                    ("userContext", input.get("context").clone()),
                    ("status", expected.get("preparation").get("status").clone()),
                    ("sourceAvailability", "observed".into()),
                    ("blockers", Js::Arr(vec![])),
                    ("source", expected.get("source").clone()),
                    ("fields", expected.get("fields").clone()),
                    (
                        "interviews",
                        expected.get("preparation").get("interviews").clone(),
                    ),
                    ("observationHandle", "fictional:process-local".into()),
                    ("observationPersistence", "process_local".into()),
                ])
            })
        },
        || observed,
    )
    .unwrap();
    assert_eq!(encode(&out).unwrap(), encode(expected).unwrap());
    retain(&root, "candidate-original-body", &out);
    let denied = requests
        .iter()
        .find(|r| r.get("id") == &Js::Num(9.0))
        .unwrap()
        .get("params")
        .get("arguments");
    assert!(denied.has("teacherId"));
    assert!(read::prepare(denied, &admission, observed).is_err());
    let original_denial = rows
        .iter()
        .find(|r| r.get("id") == &Js::Num(9.0))
        .unwrap()
        .get("result");
    assert!(original_denial.get("isError").truthy());
    assert!(
        original_denial
            .get("structuredContent")
            .get("code")
            .is("tool_input_invalid")
    );
}

#[test]
fn v2_routing_summary_keeps_student_scope_in_upstream_probe() {
    let root = case_root("v2-probe");
    let mut summary = routing_admission();
    summary.set("studentScope", "source_verified".into());
    let projected = edu_academy_mcp::upstream::read_admission(&summary);
    retain(&root, "original-summary", &summary);
    retain(&root, "actual-projection", &projected);
    assert_eq!(
        encode(&projected).unwrap(),
        encode(&summary).unwrap(),
        "actual OwnerV2 read summary has no schemaVersion; preserve its admission digest"
    );
    let mut full_v2 = summary.clone();
    full_v2.set("schemaVersion", "academy.owner-admission/2".into());
    assert_eq!(
        encode(&edu_academy_mcp::upstream::read_admission(&full_v2)).unwrap(),
        encode(&summary).unwrap(),
        "full V2 owner and its routing summary must bind the same bytes"
    );
    let mut full_v2 = routing_admission();
    full_v2.set("schemaVersion", "academy.owner-admission/2".into());
    let mut configured = routing_admission();
    configured.set("studentScope", "configured".into());
    let defaulted = edu_academy_mcp::upstream::read_admission(&full_v2);
    retain(&root, "v2-defaulted-scope", &defaulted);
    assert_eq!(encode(&defaulted).unwrap(), encode(&configured).unwrap());
    let mut full_v1 = summary;
    full_v1.set("schemaVersion", "academy.owner-admission/1".into());
    let v1 = edu_academy_mcp::upstream::read_admission(&full_v1);
    retain(&root, "v1-does-not-gain-scope", &v1);
    assert_eq!(encode(&v1).unwrap(), encode(&routing_admission()).unwrap());
}
