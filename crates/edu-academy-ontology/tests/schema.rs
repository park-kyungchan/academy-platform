//! Schema and completed engine-ledger contract controls on disposable stores.
use edu_academy_core::workflow::{Runtime, ledger_cases::Clock};
use edu_academy_ontology::SCHEMA;
use ont::{
    Answer, Context, Failure,
    action::{ApplyActionRequest, apply_action},
    search::{SearchRequest, search},
};
use ont_core::{Id, Namespace};
use ont_store::{Store, schema_mode::InstallArgs};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::PathBuf,
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn id(ty: &str, key: &str) -> Id {
    Id {
        object_type: ty.into(),
        namespace: Namespace {
            instance: "academy://owner".into(),
            account: "teacher".into(),
        },
        native_key: hash(key),
    }
}
fn request_id(action: char, key: &str, digest: &str) -> String {
    let input = if action == 's' {
        key.to_owned()
    } else {
        format!("{key}\n{digest}")
    };
    format!("{action}-{}", &hash(&input)[..62])
}
fn result(answer: Answer) -> Value {
    match answer {
        Answer::Enveloped { result, .. } | Answer::Plain(result) => result,
        other => panic!("unexpected answer: {other:?}"),
    }
}
fn failure(failure: Failure) -> (String, Value) {
    match failure {
        Failure::Refused(r) => (r.code.into(), Value::Object(r.extra)),
        other => panic!("internal: {other:?}"),
    }
}
fn applied(answer: std::result::Result<Answer, Failure>) -> String {
    result(answer.unwrap())["outcome"].as_str().unwrap().into()
}
fn refused(answer: std::result::Result<Answer, Failure>, code: &str, failed: &[&str]) {
    let (actual, extra) = failure(answer.unwrap_err());
    assert_eq!(actual, code);
    if !failed.is_empty() {
        assert_eq!(extra["failed"], json!(failed));
    }
    println!("REFUSAL code={actual} extra={extra}");
}
fn apply(
    ctx: &Context,
    action: &str,
    params: Value,
    rid: &str,
) -> std::result::Result<Answer, Failure> {
    apply_action(
        ctx,
        &ApplyActionRequest {
            op: None,
            action_type: action.into(),
            parameters: params,
            agent: None,
            session: None,
            expected: None,
            request_id: Some(rid.into()),
        },
    )
}
struct Scene {
    root: PathBuf,
    ctx: Context,
}
impl Scene {
    fn new() -> Self {
        let parent = PathBuf::from(
            std::env::var("S10_TEST_ROOT").expect("explicit synthetic root required"),
        );
        let root = parent.join(format!(
            "ontology-schema-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .unwrap();
        let path = root.join("academy.sqlite");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        let mut store = Store::open_schema(&path).unwrap();
        let pin = store
            .install_schema(
                SCHEMA.as_bytes(),
                InstallArgs {
                    installed_at: Clock.epoch_ms() / 1000,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(pin.sha256, hash(SCHEMA));
        drop(store);
        let mut ctx = Context::new(path, Clock.epoch_ms() / 1000);
        ctx.client = "edu-academy".into();
        ctx.agent = Some("ledger".into());
        Self { root, ctx }
    }
    fn register(&self, group: &str) -> String {
        applied(apply(
            &self.ctx,
            "register_student",
            json!({"scope":id("student",group),"source_namespace":"invented","teacher_id":"11","student_id":group}),
            &request_id('s', group, ""),
        ))
    }
    fn params(group: &str, key: &str) -> Value {
        json!({"attempt":id("save_attempt",key),"student":id("student",group),"field":"homework","record_key":"R1","source_namespace":"invented","student_id":group,"lesson_date":"2040-01-03","request_lesson_date":"2040-01-04","course_id":"C1","request_ref":"request","execution_sha256":hash("execution"),"execution_key":key})
    }
    fn begin(&self, group: &str, key: &str) -> std::result::Result<Answer, Failure> {
        apply(
            &self.ctx,
            "begin_attempt",
            Self::params(group, key),
            &request_id('b', key, &hash("execution")),
        )
    }
    fn outcome(
        &self,
        key: &str,
        digest: &str,
        settled: bool,
    ) -> std::result::Result<Answer, Failure> {
        apply(
            &self.ctx,
            if settled {
                "settle_outcome"
            } else {
                "record_outcome"
            },
            json!({"attempt":id("save_attempt",key),"outcome_sha256":hash(digest)}),
            &request_id(if settled { 't' } else { 'o' }, key, &hash(digest)),
        )
    }
    fn reconcile(
        &self,
        key: &str,
        digest: &str,
        state: &str,
    ) -> std::result::Result<Answer, Failure> {
        apply(
            &self.ctx,
            "record_reconciliation",
            json!({"attempt":id("save_attempt",key),"reconciliation_sha256":hash(digest),"state":state}),
            &request_id('r', key, &hash(digest)),
        )
    }
    fn search(&self, filter: Value) -> Value {
        result(
            search(
                &self.ctx,
                &SearchRequest::parse(
                    &serde_json::to_vec(
                        &json!({"object_type":"save_attempt","where":filter,"limit":1000}),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
    }
    fn open_filter(params: &Value) -> Value {
        json!({"all":[{"eq":["student",params["student"]]},{"in":["state",["dispatch_outcome_unknown","reconciliation_required"]]},{"eq":["field",params["field"]]},{"any":[{"eq":["record_key",params["record_key"]]},{"all":[{"eq":["source_namespace",params["source_namespace"]]},{"eq":["student_id",params["student_id"]]},{"eq":["lesson_date",params["lesson_date"]]},{"eq":["course_id",params["course_id"]]}]}]}]})
    }
    fn get(&self, key: &str) -> Value {
        result(
            ont::get::get(
                &self.ctx,
                &ont::get::GetRequest {
                    op: None,
                    id: id("save_attempt", key),
                },
            )
            .unwrap(),
        )
    }
}
impl Drop for Scene {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn schema_checked_by_engine() {
    let scene = Scene::new();
    let mut store = Store::open_schema_existing(&scene.ctx.store).unwrap();
    assert_eq!(store.schema_pins().unwrap()["academy"].sha256, hash(SCHEMA));
    assert_eq!(
        fs::metadata(&scene.ctx.store).unwrap().mode() & 0o777,
        0o600
    );
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    assert_eq!(
        schema["views"]["pending_attempts"]["filter"],
        json!({"all":[{"in":["state",["dispatch_outcome_unknown","reconciliation_required"]]}]})
    );
}
#[test]
fn engine_answer_table() {
    let s = Scene::new();
    refused(
        s.begin("S1", "A"),
        "submission_criteria",
        &["scope_missing"],
    );
    assert_eq!(s.register("S1"), "applied");
    assert_eq!(s.register("S1"), "already_applied");
    assert_eq!(applied(s.begin("S1", "A")), "applied");
    assert_eq!(applied(s.begin("S1", "A")), "already_applied");
    let mut other = Scene::params("S1", "A");
    other["execution_sha256"] = json!(hash("other execution"));
    refused(
        apply(
            &s.ctx,
            "begin_attempt",
            other,
            &request_id('b', "A", &hash("other execution")),
        ),
        "submission_criteria",
        &["attempt_exists", "field_fenced"],
    );
    refused(s.begin("S1", "B"), "submission_criteria", &["field_fenced"]);
    refused(
        s.reconcile("A", "too-early", "reconciled_persisted"),
        "submission_criteria",
        &["outcome_missing"],
    );
    assert_eq!(applied(s.outcome("A", "outcome", false)), "applied");
    assert_eq!(applied(s.outcome("A", "outcome", false)), "already_applied");
    refused(
        s.outcome("A", "another outcome", false),
        "submission_criteria",
        &["outcome_exists"],
    );
    refused(
        s.reconcile("A", "bad-state", "reconciliation_required"),
        "submission_criteria",
        &["state_not_reconciled"],
    );
    refused(s.begin("S1", "B"), "submission_criteria", &["field_fenced"]);
    assert_eq!(
        applied(s.reconcile("A", "closing", "reconciled_not_persisted")),
        "applied"
    );
    assert_eq!(
        applied(s.reconcile("A", "closing", "reconciled_not_persisted")),
        "already_applied"
    );
    refused(
        s.reconcile("A", "another closing", "reconciled_conflict"),
        "submission_criteria",
        &["attempt_not_open"],
    );
    refused(
        s.outcome("A", "closed outcome", false),
        "submission_criteria",
        &["attempt_not_open", "outcome_exists"],
    );
    assert_eq!(applied(s.begin("S1", "B")), "applied");
    assert_eq!(applied(s.outcome("B", "settled", true)), "applied");
    refused(
        s.reconcile("B", "rec closed", "reconciled_conflict"),
        "submission_criteria",
        &["attempt_not_open"],
    );
    refused(
        s.outcome("B", "closed settled", true),
        "submission_criteria",
        &["attempt_not_open", "outcome_exists"],
    );
    refused(
        s.outcome("missing", "missing outcome", false),
        "rule_reference_unavailable",
        &[],
    );
    refused(
        s.reconcile("missing", "missing rec", "reconciled_conflict"),
        "rule_reference_unavailable",
        &[],
    );
    let after = s.search(json!({"eq":["student",id("student","S1")]}));
    assert_eq!(
        after["objects"].as_array().unwrap().len(),
        2,
        "missing mutation creates no object"
    );
    assert_eq!(after["basis"]["qualified"], true);
}
#[test]
fn engine_parallel_begins() {
    for same in [true, false] {
        let s = Scene::new();
        s.register("S1");
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|i| {
                let ctx = s.ctx.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let key = if same || i == 0 { "A" } else { "B" };
                    barrier.wait();
                    match apply(
                        &ctx,
                        "begin_attempt",
                        Scene::params("S1", key),
                        &request_id('b', key, &hash("execution")),
                    ) {
                        Ok(a) => result(a)["outcome"].as_str().unwrap().to_owned(),
                        Err(e) => {
                            let (code, extra) = failure(e);
                            assert_eq!(code, "submission_criteria");
                            assert_eq!(extra["failed"], json!(["field_fenced"]));
                            "field_fenced".into()
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        let answers = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            answers.iter().filter(|a| *a == "applied").count(),
            1,
            "exactly one atomic admission: {answers:?}"
        );
        assert!(answers.contains(&if same {
            "already_applied".into()
        } else {
            "field_fenced".into()
        }));
        println!("PARALLEL same_key={same}: {answers:?}");
    }
}
#[test]
fn engine_fence_matches_open_filter() {
    // Changing any one occurrence member must release an occurrence-only fence.
    // The unchanged record key independently fences every occurrence.
    let changes = [
        ("field", "progress"),
        ("record_key", "R2"),
        ("source_namespace", "other"),
        ("student_id", "S2"),
        ("lesson_date", "2040-01-02"),
        ("course_id", "C2"),
    ];
    for closed in [false, true] {
        for (property, value) in changes {
            for same_record in [false, true] {
                let s = Scene::new();
                s.register("S1");
                assert_eq!(applied(s.begin("S1", "A")), "applied");
                if closed {
                    assert_eq!(applied(s.outcome("A", "settled", true)), "applied");
                }
                let mut p = Scene::params("S1", "B");
                p["record_key"] = json!(if same_record { "R1" } else { "R2" });
                p[property] = json!(value);
                let found = s.search(Scene::open_filter(&p));
                assert_eq!(
                    found["basis"]["qualified"], true,
                    "{property}/{same_record}/{closed}"
                );
                let fenced = !found["objects"].as_array().unwrap().is_empty();
                let answer = apply(
                    &s.ctx,
                    "begin_attempt",
                    p,
                    &request_id('b', "B", &hash("execution")),
                );
                if fenced {
                    refused(answer, "submission_criteria", &["field_fenced"]);
                } else {
                    assert_eq!(applied(answer), "applied");
                }
            }
        }
    }
}
#[test]
fn identity_is_readable() {
    let s = Scene::new();
    for group in ["S1", "S2"] {
        s.register(group);
        assert_eq!(applied(s.begin(group, group)), "applied");
        assert_eq!(applied(s.outcome(group, "settled", true)), "applied");
    }
    let mut p = Scene::params("S1", "previous");
    p["field"] = json!("previousHomeworkAssessment");
    assert_eq!(
        applied(apply(
            &s.ctx,
            "begin_attempt",
            p.clone(),
            &request_id('b', "previous", &hash("execution"))
        )),
        "applied"
    );
    let found = s.search(Scene::open_filter(&p));
    assert_eq!(found["basis"]["qualified"], true);
    assert_eq!(found["objects"].as_array().unwrap().len(), 1);
    assert_eq!(found["basis"]["scanned"], 2, "only S1 heads are candidates");
    let by_date =
        s.search(json!({"all":[{"eq":["student_id","S1"]},{"eq":["lesson_date","2040-01-03"]}]}));
    assert_eq!(by_date["basis"]["qualified"], true);
    assert_eq!(by_date["objects"].as_array().unwrap().len(), 2);
    let raw = s.get("previous");
    println!("IDENTITY {raw}");
    let fields = &raw["objects"][0]["observations"][0]["fields"];
    assert_eq!(fields["lesson_date"]["value"], "2040-01-03");
    assert_eq!(fields["request_lesson_date"]["value"], "2040-01-04");
    let rendered = result(
        ont::render::render(
            &s.ctx,
            &ont::render::RenderRequest {
                op: None,
                view: "pending_attempts".into(),
                args: Default::default(),
            },
        )
        .unwrap(),
    );
    let markdown = rendered["markdown"].as_str().unwrap();
    assert!(markdown.contains("S1 | 2040-01-03 | C1 | previousHomeworkAssessment"));
    assert_eq!(rendered["rows"], 1);
    println!("VIEW {markdown}");
}
#[test]
fn enumeration_preserves_original_key() {
    use edu_academy_core::workflow::ledger::AttemptLedger;
    let s = Scene::new();
    let original = "original-execution-key-\u{10000}";
    assert!(
        adapter(&s)
            .begin(original, &target("S1"), &hash("execution"), &Clock)
            .unwrap()
    );
    assert_eq!(adapter(&s).attempts("S1", 10).unwrap()[0].key, original);
    assert_eq!(adapter(&s).open_on(&target("S1")).unwrap()[0].key, original);
}
#[test]
#[ignore = "manual ledger cost measurement"]
fn engine_costs_with_other_students() {
    let s = Scene::new();
    s.register("measured");
    for count in 0..=900 {
        if [0, 300, 900].contains(&count) {
            let key = format!("measured-{count}");
            let started = Instant::now();
            assert!(
                adapter(&s)
                    .begin(&key, &target("measured"), &hash("execution"), &Clock)
                    .unwrap()
            );
            let begin = started.elapsed();
            let started = Instant::now();
            let answer = s.search(Scene::open_filter(&Scene::params("measured", &key)));
            let open = started.elapsed();
            assert_eq!(answer["basis"]["qualified"], true);
            assert_eq!(answer["objects"].as_array().unwrap().len(), 1);
            println!(
                "COST other_closed={count} begin_register_ms={:.3} open_search_ms={:.3} scanned={}",
                begin.as_secs_f64() * 1000.0,
                open.as_secs_f64() * 1000.0,
                answer["basis"]["scanned"]
            );
            assert_eq!(applied(s.outcome(&key, "settled", true)), "applied");
        }
        if count == 900 {
            break;
        }
        let key = format!("other-{count}");
        s.register(&key);
        assert_eq!(applied(s.begin(&key, &key)), "applied");
        assert_eq!(applied(s.outcome(&key, "outcome", false)), "applied");
        assert_eq!(
            applied(s.reconcile(&key, "reconciliation", "reconciled_persisted")),
            "applied"
        );
    }
    // Separate store: exactly 300 attempts with three attempt actions each.
    let size = Scene::new();
    size.register("size-student");
    let baseline = fs::metadata(&size.ctx.store).unwrap().len();
    for count in 0..300 {
        let key = format!("size-{count}");
        assert_eq!(applied(size.begin("size-student", &key)), "applied");
        assert_eq!(applied(size.outcome(&key, "outcome", false)), "applied");
        assert_eq!(
            applied(size.reconcile(&key, "reconciliation", "reconciled_persisted")),
            "applied"
        );
    }
    let bytes = fs::metadata(&size.ctx.store).unwrap().len();
    let found = size.search(json!({"eq":["student",id("student","size-student")]}));
    assert_eq!(found["objects"].as_array().unwrap().len(), 300);
    println!(
        "SIZE attempts=300 three_actions_each=true sqlite_bytes={bytes} bytes_per_attempt={:.3} baseline_bytes={baseline} delta_per_attempt={:.3} edits_bytes={}",
        bytes as f64 / 300.0,
        (bytes - baseline) as f64 / 300.0,
        fs::metadata(size.root.join("edits.jsonl")).unwrap().len()
    );
}

#[test]
fn legacy_root_refused() {
    use edu_academy_core::workflow::{ledger_cases::MemoryLedger, store::Store as CoreStore};
    let s = Scene::new();
    let core_root = s.root.join("store");
    fs::create_dir(core_root.clone()).unwrap();
    fs::create_dir(core_root.join("ledger")).unwrap();
    fs::write(core_root.join("ledger/old"), "invented").unwrap();
    assert_eq!(
        CoreStore::open_with_ledger(&core_root, &s.root, Box::<MemoryLedger>::default())
            .err()
            .unwrap()
            .message,
        "workflow_ledger_legacy_root"
    );
    assert_eq!(
        edu_academy_ontology::open_store(&core_root, &s.root, &Clock)
            .err()
            .unwrap()
            .message,
        "workflow_ledger_legacy_root"
    );
    assert!(!s.root.join("store.ledger").exists());
    fs::remove_file(core_root.join("ledger/old")).unwrap();
    assert!(
        edu_academy_ontology::open_store(&core_root, &s.root, &Clock)
            .unwrap()
            .ledgered()
    );
    assert!(
        edu_academy_ontology::open_store(&core_root, &s.root, &Clock)
            .unwrap()
            .ledgered()
    );
    assert_eq!(
        CoreStore::open(&core_root, &s.root).err().unwrap().message,
        "workflow_ledger_legacy_root"
    );
}

fn adapter(s: &Scene) -> edu_academy_ontology::EngineLedger {
    edu_academy_ontology::EngineLedger::new(s.ctx.store.clone(), &Clock)
}
fn target(group: &str) -> edu_academy_core::workflow::ledger::AttemptTarget {
    edu_academy_core::workflow::ledger::AttemptTarget {
        group: group.into(),
        source_namespace: "invented".into(),
        teacher_id: "11".into(),
        scope_student_id: group.into(),
        request_lesson_date: "2040-01-04".into(),
        field: "homework".into(),
        record_key: "R1".into(),
        request_ref: "request".into(),
        occurrence: [
            "invented".into(),
            group.into(),
            "2040-01-03".into(),
            "C1".into(),
        ],
    }
}
#[test]
fn adapter_answer_mapping_and_closed_attempts() {
    use edu_academy_core::workflow::state::FieldState as F;
    let s = Scene::new();
    let ledger = adapter(&s);
    let t = target("S1");
    let mut invalid = t.clone();
    invalid.field = "outside_the_field_enum".into();
    let fault = ledger
        .begin("invalid", &invalid, &hash("execution"), &Clock)
        .unwrap_err();
    assert_eq!(fault.message, "workflow_ledger_unavailable");
    assert_eq!(
        fault.properties.get("engineCode").code_units(),
        "invalid_parameter"
    );
    assert!(ledger.attempt("invalid").unwrap().is_none());
    assert!(ledger.attempt("missing").unwrap().is_none());
    assert_eq!(
        ledger
            .record_outcome("missing", &hash("out"), F::ReconciliationRequired, &Clock)
            .unwrap_err()
            .message,
        "workflow_ledger_unavailable"
    );
    assert_eq!(
        ledger
            .record_reconciliation("missing", &hash("rec"), F::ReconciledConflict, &Clock)
            .unwrap_err()
            .message,
        "workflow_ledger_unavailable"
    );
    assert!(ledger.begin("A", &t, &hash("execution"), &Clock).unwrap());
    for key in ["missing", "A"] {
        assert_eq!(
            ledger
                .record_outcome(key, &hash("unsupported"), F::DispatchUnknown, &Clock)
                .unwrap_err()
                .message,
            "workflow_ledger_unavailable"
        );
        assert_eq!(
            ledger
                .record_reconciliation(key, &hash("unsupported"), F::DispatchUnknown, &Clock)
                .unwrap_err()
                .message,
            "workflow_ledger_unavailable"
        );
    }
    assert!(
        !ledger.begin("A", &t, &hash("execution"), &Clock).unwrap(),
        "already_applied must be false"
    );
    assert!(
        !ledger
            .begin("A", &t, &hash("changed execution"), &Clock)
            .unwrap(),
        "attempt_exists plus field_fenced must be false"
    );
    assert_eq!(
        ledger
            .begin("B", &t, &hash("execution"), &Clock)
            .unwrap_err()
            .message,
        "workflow_attempt_fenced"
    );
    let initial = ledger.attempt("A").unwrap().unwrap();
    assert_eq!(initial.key, "A");
    assert_eq!(initial.state, F::DispatchUnknown);
    assert_eq!(initial.execution, hash("execution"));
    assert!(initial.outcome.is_none());
    assert!(initial.reconciliation.is_none());
    assert_eq!(
        ledger
            .record_reconciliation("A", &hash("early"), F::ReconciledPersisted, &Clock)
            .unwrap_err()
            .message,
        "workflow_ledger_unavailable"
    );
    assert!(
        ledger
            .record_outcome("A", &hash("out"), F::ReconciliationRequired, &Clock)
            .unwrap()
    );
    assert!(
        !ledger
            .record_outcome("A", &hash("out"), F::ReconciliationRequired, &Clock)
            .unwrap()
    );
    assert!(
        !ledger
            .record_outcome("A", &hash("different out"), F::SettledPersisted, &Clock)
            .unwrap()
    );
    assert_eq!(
        ledger.attempt("A").unwrap().unwrap().state,
        F::ReconciliationRequired
    );
    assert!(
        ledger
            .record_reconciliation("A", &hash("rec"), F::ReconciledConflict, &Clock)
            .unwrap()
    );
    assert!(
        !ledger
            .record_reconciliation("A", &hash("rec"), F::ReconciledConflict, &Clock)
            .unwrap()
    );
    let closed = ledger.attempt("A").unwrap().unwrap();
    for state in [F::ReconciledPersisted, F::Available] {
        assert!(
            !ledger
                .record_reconciliation("A", &hash("replacement rec"), state, &Clock)
                .unwrap()
        );
    }
    for state in [
        F::ReconciliationRequired,
        F::SettledPersisted,
        F::DispatchUnknown,
    ] {
        assert!(
            !ledger
                .record_outcome("A", &hash("replacement out"), state, &Clock)
                .unwrap()
        );
    }
    let unchanged = ledger.attempt("A").unwrap().unwrap();
    assert_eq!(unchanged.state, closed.state);
    assert_eq!(unchanged.outcome, closed.outcome);
    assert_eq!(unchanged.reconciliation, closed.reconciliation);
    assert!(ledger.begin("B", &t, &hash("execution"), &Clock).unwrap());
    assert!(
        ledger
            .record_outcome("B", &hash("settled"), F::SettledPersisted, &Clock)
            .unwrap()
    );
    assert!(
        !ledger
            .record_reconciliation("B", &hash("closed rec"), F::ReconciledConflict, &Clock)
            .unwrap()
    );
    assert!(
        !ledger
            .record_outcome("B", &hash("closed out"), F::SettledPersisted, &Clock)
            .unwrap()
    );
    let settled = ledger.attempt("B").unwrap().unwrap();
    assert_eq!(settled.state, F::SettledPersisted);
    assert_eq!(settled.outcome, Some(hash("settled")));
    assert!(settled.reconciliation.is_none());
}
#[test]
fn adapter_parallel_begins_without_student_lock() {
    for same in [true, false] {
        let s = Scene::new();
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|i| {
                let ledger = adapter(&s);
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    ledger.begin(
                        if same || i == 0 { "A" } else { "B" },
                        &target("S1"),
                        &hash("execution"),
                        &Clock,
                    )
                })
            })
            .collect::<Vec<_>>();
        let answers = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            answers.iter().filter(|r| matches!(r, Ok(true))).count(),
            1,
            "exactly one newly admitted begin"
        );
        if same {
            assert_eq!(answers.iter().filter(|r| matches!(r, Ok(false))).count(), 1);
        } else {
            assert_eq!(
                answers
                    .iter()
                    .filter(|r| r
                        .as_ref()
                        .is_err_and(|e| e.message == "workflow_attempt_fenced"))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn adapter_preserves_all_reconciled_states() {
    use edu_academy_core::workflow::state::FieldState as F;
    let s = Scene::new();
    let ledger = adapter(&s);
    let t = target("S1");
    for (key, state) in [
        ("persisted", F::ReconciledPersisted),
        ("not-persisted", F::ReconciledNotPersisted),
        ("conflict", F::ReconciledConflict),
    ] {
        assert!(ledger.begin(key, &t, &hash("execution"), &Clock).unwrap());
        assert!(
            ledger
                .record_outcome(key, &hash("outcome"), F::ReconciliationRequired, &Clock)
                .unwrap()
        );
        assert!(
            ledger
                .record_reconciliation(key, &hash(key), state, &Clock)
                .unwrap()
        );
        let a = ledger.attempt(key).unwrap().unwrap();
        assert_eq!(a.state, state);
        assert_eq!(a.reconciliation, Some(hash(key)));
    }
}

#[test]
fn previous_target_date() {
    let s = Scene::new();
    let ledger = adapter(&s);
    let mut previous = target("S1");
    previous.field = "previousHomeworkAssessment".into();
    assert!(
        ledger
            .begin("previous", &previous, &hash("execution"), &Clock)
            .unwrap()
    );
    use edu_academy_core::workflow::ledger::AttemptLedger;
    assert_eq!(ledger.open_on(&previous).unwrap()[0].key, "previous");
    assert_eq!(ledger.attempts("S1", 10).unwrap()[0].key, "previous");
    let raw = s.get("previous");
    let fields = &raw["objects"][0]["observations"][0]["fields"];
    assert_eq!(fields["lesson_date"]["value"], "2040-01-03");
    assert_eq!(fields["request_lesson_date"]["value"], "2040-01-04");
    let mut alias = previous.clone();
    alias.request_lesson_date = "2040-01-05".into();
    alias.record_key = "R2".into();
    assert_eq!(
        ledger
            .begin("alias", &alias, &hash("execution"), &Clock)
            .unwrap_err()
            .message,
        "workflow_attempt_fenced"
    );
    alias.occurrence[2] = "2040-01-02".into();
    assert!(
        ledger
            .begin("other-date", &alias, &hash("execution"), &Clock)
            .unwrap()
    );
}

#[test]
fn shared_cases_on_engine() {
    use edu_academy_core::workflow::ledger_cases;
    let factory = |root: &std::path::Path, owner: &std::path::Path| {
        edu_academy_ontology::open_store(root, owner, &Clock)
    };
    ledger_cases::run(&factory);
    ledger_cases::run_ledgered(&factory);
}

fn append_changed(s: &Scene, key: &str, change: impl FnOnce(&mut ont_core::Observation)) {
    let mut db = Store::open_schema_existing_writable(&s.ctx.store).unwrap();
    let head = db.head(&id("save_attempt", key)).unwrap().unwrap();
    let mut observation = head.head.observation;
    observation.supersedes = Some(observation.revision.clone());
    observation.revision = hash("invented correction");
    observation.change = ont_core::Change::Correction;
    change(&mut observation);
    db.append_schema(
        &[ont_store::Append {
            id: head.id,
            scope: head.scope,
            observation,
        }],
        Clock.epoch_ms() / 1000,
    )
    .unwrap();
}
#[test]
fn stored_key_tamper_fails_every_read() {
    use edu_academy_core::workflow::ledger::AttemptLedger;
    let s = Scene::new();
    let ledger = adapter(&s);
    assert!(
        ledger
            .begin("key", &target("S1"), &hash("execution"), &Clock)
            .unwrap()
    );
    append_changed(&s, "key", |o| {
        o.fields.insert(
            "execution_key".into(),
            ont_core::Knowledge::Value(json!("tampered")),
        );
    });
    assert_eq!(
        ledger.attempt("key").unwrap_err().message,
        "workflow_ledger_unavailable"
    );
    assert_eq!(
        ledger.attempts("S1", 10).unwrap_err().message,
        "workflow_ledger_unavailable"
    );
    assert_eq!(
        ledger.open_on(&target("S1")).unwrap_err().message,
        "workflow_ledger_unavailable"
    );
}
#[test]
fn unqualified_and_refused_reads_are_errors() {
    use edu_academy_core::workflow::ledger::AttemptLedger;
    let s = Scene::new();
    let ledger = adapter(&s);
    ledger
        .begin("key", &target("S1"), &hash("execution"), &Clock)
        .unwrap();
    append_changed(&s, "key", |o| o.coverage = ont_core::Coverage::Partial);
    assert_eq!(
        ledger.attempts("S1", 10).unwrap_err().message,
        "workflow_ledger_unavailable"
    );
    assert_eq!(
        ledger.open_on(&target("S1")).unwrap_err().message,
        "workflow_ledger_unavailable"
    );
    let missing = edu_academy_ontology::EngineLedger::new(s.root.join("missing.sqlite"), &Clock);
    assert_eq!(
        missing.attempt("key").unwrap_err().message,
        "workflow_ledger_unavailable"
    );
    assert_eq!(
        missing.attempts("S1", 10).unwrap_err().message,
        "workflow_ledger_unavailable"
    );
    assert_eq!(
        missing.open_on(&target("S1")).unwrap_err().message,
        "workflow_ledger_unavailable"
    );
}
#[test]
fn attempts_and_open_on_are_paged_without_bodies() {
    use edu_academy_core::workflow::ledger::AttemptLedger;
    let s = Scene::new();
    let ledger = adapter(&s);
    ledger
        .begin("seed", &target("S1"), &hash("execution"), &Clock)
        .unwrap();
    let mut db = Store::open_schema_existing_writable(&s.ctx.store).unwrap();
    let head = db.head(&id("save_attempt", "seed")).unwrap().unwrap();
    let rows = (0..1002)
        .map(|i| {
            let key = format!("invented-page-{i}");
            let mut observation = head.head.observation.clone();
            observation.revision = hash(&key);
            observation.fields.insert(
                "execution_key".into(),
                ont_core::Knowledge::Value(json!(key)),
            );
            ont_store::Append {
                id: id("save_attempt", &key),
                scope: head.scope.clone(),
                observation,
            }
        })
        .collect::<Vec<_>>();
    db.append_schema(&rows, Clock.epoch_ms() / 1000).unwrap();
    drop(db);
    assert_eq!(ledger.attempts("S1", 0).unwrap().len(), 0);
    assert_eq!(ledger.attempts("S1", 3).unwrap().len(), 3);
    let attempts = ledger.attempts("S1", usize::MAX).unwrap();
    assert_eq!(attempts.len(), 1003);
    assert_eq!(
        attempts
            .iter()
            .map(|a| &a.key)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1003
    );
    assert_eq!(ledger.open_on(&target("S1")).unwrap().len(), 1003);
    assert!(!s.root.join("execution").exists());
}
#[test]
fn open_on_reads_one_student() {
    use edu_academy_core::workflow::{ledger::AttemptLedger, state::FieldState};
    let s = Scene::new();
    let ledger = adapter(&s);
    for group in ["S1", "S2"] {
        ledger
            .begin(group, &target(group), &hash("execution"), &Clock)
            .unwrap();
        ledger
            .record_outcome(group, &hash("out"), FieldState::SettledPersisted, &Clock)
            .unwrap();
    }
    assert!(
        ledger.open_on(&target("S1")).unwrap().is_empty(),
        "closed attempts must be excluded"
    );
    ledger
        .begin("open", &target("S1"), &hash("execution"), &Clock)
        .unwrap();
    assert_eq!(
        ledger
            .open_on(&target("S1"))
            .unwrap()
            .iter()
            .map(|a| a.key.as_str())
            .collect::<Vec<_>>(),
        ["open"]
    );
    let raw = s.search(Scene::open_filter(&Scene::params("S1", "open")));
    assert_eq!(raw["basis"]["qualified"], true);
    assert_eq!(raw["basis"]["scanned"], 2);
    let rendered = result(
        ont::render::render(
            &s.ctx,
            &ont::render::RenderRequest {
                op: None,
                view: "pending_attempts".into(),
                args: Default::default(),
            },
        )
        .unwrap(),
    );
    assert_eq!(rendered["rows"], 1);
    assert!(
        rendered["markdown"]
            .as_str()
            .unwrap()
            .contains("S1 | 2040-01-03 | C1 | homework")
    );
    assert_eq!(ledger.attempts("S2", 10).unwrap().len(), 1);
}
#[test]
fn fence_matches_open_on() {
    use edu_academy_core::workflow::{ledger::AttemptLedger, state::FieldState};
    for closed in [false, true] {
        let s = Scene::new();
        let ledger = adapter(&s);
        let original = target("S1");
        ledger
            .begin("seed", &original, &hash("execution"), &Clock)
            .unwrap();
        if closed {
            ledger
                .record_outcome("seed", &hash("out"), FieldState::SettledPersisted, &Clock)
                .unwrap();
        }
        for case in 0..14 {
            let mut candidate = original.clone();
            match case {
                0 => {}
                1 => candidate.field = "progress".into(),
                2 => candidate.record_key = "other-record".into(),
                3..=6 => {
                    candidate.occurrence[case - 3] = match case {
                        5 => "2040-01-02",
                        _ => "other",
                    }
                    .into()
                }
                7 => {
                    candidate.group = "S2".into();
                    candidate.scope_student_id = "S2".into();
                }
                8 => {
                    candidate.record_key = "other".into();
                    candidate.occurrence[2] = "2040-01-02".into();
                }
                9 => {
                    candidate.request_ref = "other-request".into();
                    candidate.request_lesson_date = "2040-01-05".into();
                }
                10..=13 => {
                    candidate.record_key = "different-record".into();
                    candidate.occurrence[case - 10] =
                        if case == 12 { "2040-01-02" } else { "other" }.into();
                }
                _ => unreachable!(),
            }
            let matches = !ledger.open_on(&candidate).unwrap().is_empty();
            let expected = !closed && ![1, 7, 8, 10, 11, 12, 13].contains(&case);
            assert_eq!(matches, expected, "selection case={case} closed={closed}");
            let key = format!("candidate-{case}");
            let admitted = ledger.begin(&key, &candidate, &hash("execution"), &Clock);
            if matches {
                assert_eq!(admitted.unwrap_err().message, "workflow_attempt_fenced");
            } else {
                assert!(admitted.unwrap());
                ledger
                    .record_outcome(&key, &hash("out"), FieldState::SettledPersisted, &Clock)
                    .unwrap();
            }
        }
    }
}
#[test]
fn reopen_after_restart_and_store_beside_root() {
    use edu_academy_core::workflow::{ledger::AttemptLedger, store::Store as CoreStore};
    let s = Scene::new();
    let root = s.root.join("store");
    assert_eq!(
        edu_academy_ontology::open_store(&s.root, &s.root, &Clock)
            .err()
            .unwrap()
            .message,
        "workflow_ledger_root_required"
    );
    assert_eq!(
        edu_academy_ontology::open_store(&s.root.join("../escape"), &s.root, &Clock)
            .err()
            .unwrap()
            .message,
        "workflow_ledger_root_required"
    );
    let core = edu_academy_ontology::open_store(&root, &s.root, &Clock).unwrap();
    assert!(core.ledgered());
    assert!(!root.join("academy.sqlite").exists());
    assert_eq!(
        fs::metadata(s.root.join("store.ledger")).unwrap().mode() & 0o777,
        0o700
    );
    let path = s.root.join("store.ledger/academy.sqlite");
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    let ledger = edu_academy_ontology::EngineLedger::new(path.clone(), &Clock);
    assert!(
        ledger
            .begin("original", &target("S1"), &hash("execution"), &Clock)
            .unwrap()
    );
    let before = fs::read(&path).unwrap();
    drop(core);
    assert!(
        edu_academy_ontology::open_store(&root, &s.root, &Clock)
            .unwrap()
            .ledgered()
    );
    assert_eq!(fs::read(&path).unwrap(), before, "reopen installs nothing");
    assert_eq!(
        edu_academy_ontology::EngineLedger::new(path, &Clock)
            .attempts("S1", 10)
            .unwrap()[0]
            .key,
        "original"
    );
    assert_eq!(
        CoreStore::open(&root, &s.root).err().unwrap().message,
        "workflow_ledger_legacy_root"
    );
    let legacy = s.root.join("old");
    fs::create_dir(&legacy).unwrap();
    fs::create_dir(legacy.join("execution")).unwrap();
    fs::write(
        legacy
            .join("execution")
            .join(format!("{}.json", "a".repeat(64))),
        "invented",
    )
    .unwrap();
    assert_eq!(
        edu_academy_ontology::open_store(&legacy, &s.root, &Clock)
            .err()
            .unwrap()
            .message,
        "workflow_ledger_legacy_root"
    );
    assert!(!s.root.join("old.ledger").exists());
}

#[test]
fn startup_writable_open_upgrades_old_projection() {
    use edu_academy_core::workflow::ledger::AttemptLedger;
    let s = Scene::new();
    let root = s.root.join("store");
    edu_academy_ontology::open_store(&root, &s.root, &Clock).unwrap();
    let path = s.root.join("store.ledger/academy.sqlite");
    let ledger = edu_academy_ontology::EngineLedger::new(path.clone(), &Clock);
    ledger
        .begin("original", &target("S1"), &hash("execution"), &Clock)
        .unwrap();
    // Only the disposable fixture is changed: reproduce the older projection marker.
    let output = std::process::Command::new("python3").args(["-c", "import sqlite3,sys\nwith sqlite3.connect(sys.argv[1]) as db:\n db.execute('UPDATE candidate_generation SET semantics_version=0 WHERE singleton=1')"]).arg(&path).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        Store::open_schema_existing(&path).err().unwrap().code,
        "store_upgrade_required"
    );
    edu_academy_ontology::open_store(&root, &s.root, &Clock)
        .expect("startup writable open must upgrade older projection before reads");
    assert_eq!(ledger.attempts("S1", 10).unwrap()[0].key, "original");
    assert!(Store::open_schema_existing(&path).is_ok());
}

#[test]
fn closed_attempt_answers_false() {
    use edu_academy_core::workflow::state::FieldState as F;
    let s = Scene::new();
    let ledger = adapter(&s);
    for (key, settled) in [("settled", true), ("reconciled", false)] {
        assert!(
            ledger
                .begin(key, &target("S1"), &hash("execution"), &Clock)
                .unwrap()
        );
        assert!(
            ledger
                .record_outcome(
                    key,
                    &hash("out"),
                    if settled {
                        F::SettledPersisted
                    } else {
                        F::ReconciliationRequired
                    },
                    &Clock
                )
                .unwrap()
        );
        if !settled {
            assert!(
                ledger
                    .record_reconciliation(key, &hash("rec"), F::ReconciledConflict, &Clock)
                    .unwrap()
            );
        }
        let before = ledger.attempt(key).unwrap().unwrap();
        assert!(
            !ledger
                .record_outcome(key, &hash("changed-out"), F::ReconciliationRequired, &Clock)
                .unwrap()
        );
        assert!(
            !ledger
                .record_reconciliation(key, &hash("changed-rec"), F::ReconciledPersisted, &Clock)
                .unwrap()
        );
        let after = ledger.attempt(key).unwrap().unwrap();
        assert_eq!(
            (
                after.key,
                after.state,
                after.execution,
                after.outcome,
                after.reconciliation
            ),
            (
                before.key,
                before.state,
                before.execution,
                before.outcome,
                before.reconciliation
            )
        );
    }
    ledger
        .begin("open", &target("S1"), &hash("execution"), &Clock)
        .unwrap();
    assert_eq!(
        ledger
            .record_reconciliation("open", &hash("rec"), F::ReconciledConflict, &Clock)
            .unwrap_err()
            .message,
        "workflow_ledger_unavailable"
    );
    assert!(ledger.attempt("open").unwrap().unwrap().outcome.is_none());
}

#[test]
fn sibling_is_one_for_every_spelling() {
    use edu_academy_core::workflow::{
        ledger_cases::{execution, scope, target as body_target},
        store::{Kind, Store as CoreStore, ledger_sibling},
    };
    for first in 0..3 {
        let s = Scene::new();
        let root = s.root.join("store");
        let spellings = [root.clone(), root.join(""), root.join(".")];
        let expected = s.root.join("store.ledger");
        let initial = edu_academy_ontology::open_store(&spellings[first], &s.root, &Clock).unwrap();
        assert!(expected.join("academy.sqlite").exists());
        assert!(!root.join(".ledger").exists());
        assert!(!root.join("..ledger").exists());
        for spelling in &spellings {
            assert_eq!(
                CoreStore::open(spelling, &s.root).err().unwrap().message,
                "workflow_ledger_legacy_root",
                "the sibling guard must refuse even before any body exists"
            );
        }
        let scope = scope("12");
        let e = execution(
            &initial,
            &scope,
            "progress",
            "record".into(),
            &body_target(&scope),
            "review",
        );
        assert!(
            initial
                .publish(Kind::Execution, "first", &e, &Clock)
                .unwrap()
                .0
        );
        for spelling in &spellings {
            assert_eq!(ledger_sibling(spelling), Some(expected.clone()));
            assert_eq!(
                CoreStore::open(spelling, &s.root).err().unwrap().message,
                "workflow_ledger_legacy_root"
            );
            let reopened = edu_academy_ontology::open_store(spelling, &s.root, &Clock).unwrap();
            assert!(
                !reopened
                    .publish(Kind::Execution, "first", &e, &Clock)
                    .unwrap()
                    .0
            );
            assert_eq!(
                reopened
                    .publish(Kind::Execution, "second", &e, &Clock)
                    .unwrap_err()
                    .message,
                "workflow_attempt_fenced"
            );
        }
    }
    assert!(edu_academy_core::workflow::store::ledger_sibling(std::path::Path::new("/")).is_none());
    assert!(edu_academy_core::workflow::store::ledger_sibling(std::path::Path::new(".")).is_none());
    assert!(
        edu_academy_core::workflow::store::ledger_sibling(std::path::Path::new("/a/..")).is_none()
    );
}

#[test]
fn begin_again_with_another_target_is_false() {
    let s = Scene::new();
    let ledger = adapter(&s);
    let original = target("S1");
    let digest = hash("execution");
    assert!(ledger.begin("applied", &original, &digest, &Clock).unwrap());
    assert_eq!(
        ledger
            .begin("refused", &original, &digest, &Clock)
            .unwrap_err()
            .message,
        "workflow_attempt_fenced"
    );
    let mut another = original.clone();
    another.record_key = "R2".into();
    another.occurrence[3] = "C2".into();
    assert!(
        ledger.begin("refused", &another, &digest, &Clock).unwrap(),
        "a refused action must not consume its request id"
    );
    assert!(
        !ledger.begin("applied", &another, &digest, &Clock).unwrap(),
        "an existing key answers false before checking its changed target"
    );
    assert_eq!(
        ledger.attempt("applied").unwrap().unwrap().execution,
        digest
    );
}

#[test]
fn open_store_refuses_parent_components() {
    let s = Scene::new();
    assert_eq!(
        edu_academy_ontology::open_store(&s.root.join("x/../store"), &s.root, &Clock)
            .err()
            .unwrap()
            .message,
        "workflow_ledger_root_required"
    );
    assert!(!s.root.join("x").exists());
    assert!(!s.root.join("store").exists());
}
