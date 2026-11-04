//! New intake is exercised through real owner-store publication and a fresh
//! Service/Store review consumer. All ports and records are explicitly fictional.
//! The caller must supply a disposable S11_INTAKE_TEST_ROOT; no native defaults.
use edu_academy_core::{
    js::{Fault, Js, Result, obj, sha, stringify},
    workflow::{
        Runtime,
        approval::Approval,
        codec, review, service,
        state::request_key,
        store::{Kind, Store, mkdir},
    },
};
use serde_json::json;
use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
const NOW: &str = "2040-01-04T12:00:00.000Z";
const FIRST_HANDLE: &str = "dailyrecord-observation:00000000-0000-4000-8000-000000000001";
struct Clock {
    now: String,
    sequence: Cell<u64>,
}
impl Clock {
    fn new(now: &str) -> Self {
        Self {
            now: now.into(),
            sequence: Cell::new(0),
        }
    }
}
impl Runtime for Clock {
    fn now(&self) -> String {
        self.now.clone()
    }
    fn epoch_ms(&self) -> u64 {
        // Intake freshness must use the same parsed Runtime::now as review.
        0
    }
    fn uuid(&self) -> Result<String> {
        let next = self.sequence.get() + 1;
        self.sequence.set(next);
        Ok(format!("00000000-0000-4000-8000-{next:012}"))
    }
}
struct Source {
    result: Js,
    error: Option<Fault>,
    calls: Vec<Js>,
}
impl Source {
    fn observed() -> Self {
        Self {
            result: obj(&[
                ("status", "observed".into()),
                ("observation", observation()),
                (
                    "subject",
                    js(json!({"displayName":"Invented Intake Student", "extra":"not persisted"})),
                ),
            ]),
            error: None,
            calls: vec![],
        }
    }
}
impl service::Source for Source {
    fn read(&mut self, scope: &Js, request: &Js, subject: &Js) -> Result<Js> {
        self.calls.push(obj(&[
            ("scope", scope.clone()),
            ("requestRef", request.clone()),
            ("subject", subject.clone()),
        ]));
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(self.result.clone()),
        }
    }
}
struct Backend;
impl service::Backend for Backend {
    fn configured(&self) -> bool {
        true
    }
    fn save(&mut self, _: &Js, _: &Js, _: &mut dyn Approval, _: &Js) -> Result<Js> {
        panic!("intake/review must never invoke save")
    }
}
struct Policy {
    parent: bool,
    line_calls: Cell<usize>,
}
impl Policy {
    fn new(parent: bool) -> Self {
        Self {
            parent,
            line_calls: Cell::new(0),
        }
    }
}
impl review::Policy for Policy {
    fn parent_pins(&self) -> Js {
        if self.parent {
            js(
                json!({"sourceRef":"fictional:intake-parent", "policySha256":"3".repeat(64),
                "safeguardSourceSha256":"4".repeat(64)}),
            )
        } else {
            Js::Null
        }
    }
    fn learning_pins(&self) -> Js {
        js(
            json!({"bundleSha256":"5".repeat(64), "configSha256":"6".repeat(64),
            "catalogSha256":"7".repeat(64)}),
        )
    }
    fn check_lines(&self, value: &Js) -> bool {
        self.line_calls.set(self.line_calls.get() + 1);
        value.units().starts_with(&[45, 32])
    }
    fn excluded_parent_language(&self, _: &Js) -> bool {
        false
    }
    fn scope_qualified(&self, _: &Js) -> bool {
        true
    }
    fn learning_review(&self, _: &Js, _: &Js, _: &Js) -> Vec<Js> {
        vec![]
    }
}
fn js(value: serde_json::Value) -> Js {
    codec::parse(&serde_json::to_vec(&value).unwrap()).unwrap()
}
fn root(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var("S11_INTAKE_TEST_ROOT")
            .expect("explicit disposable intake evidence root required"),
    )
    .join(format!(
        "{name}-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::SeqCst)
    ));
    mkdir(&root).unwrap();
    root
}
fn context() -> Js {
    obj(&[
        ("schemaVersion", "academy.dailyrecord-user-context/1".into()),
        ("purpose", "Fictional intake contract verification".into()),
        ("currentRequestRef", "fictional-request".into()),
        (
            "scope",
            obj(&[
                ("sourceNamespace", "fictional:intake".into()),
                ("teacherId", "teacher".into()),
                ("studentId", "student".into()),
                ("lessonDate", "2040-01-04".into()),
            ]),
        ),
        (
            "selectedFields",
            js(json!([
                "progress",
                "homework",
                "lessonMemo",
                "attendance",
                "dailyTestAssessment",
                "previousHomeworkAssessment"
            ])),
        ),
        (
            "memoAnswer",
            obj(&[
                ("kind", "no_addition".into()),
                ("requestRef", "fictional-request".into()),
            ]),
        ),
        ("facts", Js::Arr(vec![])),
    ])
}
fn observation() -> Js {
    let scope = context().get("scope").clone();
    let mut target = scope.clone();
    for (key, value) in [
        ("courseId", "course"),
        ("membershipId", "membership"),
        ("recordId", "current-record"),
    ] {
        target.set(key, value.into());
    }
    let mut previous = target.clone();
    previous.set("recordId", "previous-record".into());
    previous.set("lessonDate", "2040-01-03".into());
    let fields = obj(&[
        ("progress", known("- Existing progress")),
        ("homework", known("- Existing homework")),
        ("lessonMemo", known("- Existing memo")),
        ("attendance", known("Y")),
        ("dailyTestAssessment", known("0")),
        ("previousHomeworkAssessment", known("0")),
    ]);
    obj(&[
        ("scope", scope),
        ("target", target),
        ("previousTarget", previous.clone()),
        ("observedAt", NOW.into()),
        ("sourceRevision", "1".repeat(64).into()),
        ("wireDigest", "2".repeat(64).into()),
        ("fields", fields),
        (
            "toneSamples",
            Js::Arr(vec![obj(&[
                ("target", previous),
                ("observedAt", "2040-01-03T12:00:00.000Z".into()),
                ("text", "- Allowed fictional tone".into()),
                ("evidenceRefs", js(json!(["fictional-tone-evidence"]))),
            ])]),
        ),
        ("transmissionState", "not_sent".into()),
    ])
}
fn known(value: &str) -> Js {
    obj(&[
        ("state", "known".into()),
        ("value", value.into()),
        ("evidenceRefs", js(json!(["fictional-read"]))),
    ])
}
fn fact(field: &str, value: &str) -> Js {
    // origin intentionally omitted: the existing intake schema supplies its default.
    js(
        json!({"ref":format!("fact-{field}"), "field":field, "text":value,
        "explicitValue":value, "requiredFragments":[], "questionCounts":[],
        "deadlines":[], "stageClaims":[]}),
    )
}
fn proposal(field: &str, value: &str) -> Js {
    js(
        json!({"field":field, "value":value, "factRefs":[format!("fact-{field}")],
        "decision":{"kind":"replace", "requestRef":"fictional-request"}}),
    )
}
fn input() -> Js {
    obj(&[("context", context())])
}
fn run(
    store: &Store,
    source: &mut Source,
    policy: &Policy,
    clock: &Clock,
    input: &Js,
) -> Result<Js> {
    service::Service {
        store,
        source,
        policy,
        backend: &mut Backend,
        runtime: clock,
    }
    .intake(input)
}
fn set_observation(source: &mut Source, observation: Js) {
    source.result.set("observation", observation);
}
fn assert_no_publication(store: &Store, clock: &Clock) {
    assert_eq!(
        clock.sequence.get(),
        0,
        "no handle/temp/audit entropy consumed"
    );
    assert!(
        fs::read_dir(store.root().join("observation"))
            .unwrap()
            .next()
            .is_none()
    );
    assert!(
        fs::read_dir(store.root().join("audit"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn defaults_publish_owner_observation_with_attribution_and_audit() {
    let root = root("durable");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let mut source = Source::observed();
    let policy = Policy::new(true);
    let clock = Clock::new(NOW);
    let mut input = input();
    input.set(
        "actor",
        js(json!({"runtime":"fictional-runner", "sessionRef":"fictional-session"})),
    );
    let result = run(&store, &mut source, &policy, &clock, &input).unwrap();
    assert!(result.get("status").is("ready_for_review"));
    assert!(result.get("observationPersistence").is("owner_durable"));
    assert!(result.get("observationHandle").is(FIRST_HANDLE));
    assert!(result.get("blockers").array().is_empty());
    assert!(result.get("interviews").array().is_empty());
    assert_eq!(result.get("academyWrites"), &Js::Num(0.0));
    assert_eq!(result.get("parentSends"), &Js::Num(0.0));
    assert!(result.get("execution").is("not_supported"));
    assert_eq!(source.calls.len(), 1);
    assert!(matches!(source.calls[0].get("subject"), Js::Undefined));
    let stored = store.get(Kind::Observation, FIRST_HANDLE).unwrap();
    assert_eq!(stored.get("scope"), result.get("userContext").get("scope"));
    assert_eq!(stored.get("observation"), source.result.get("observation"));
    assert_eq!(
        stored.get("subject"),
        &js(json!({"displayName":"Invented Intake Student"}))
    );
    assert_eq!(stored.get("createdBy").get("declared"), input.get("actor"));
    assert!(
        result
            .get("attribution")
            .get("declaredRuntime")
            .is("fictional-runner")
    );
    let key = request_key(context().get("scope"), "fictional-request").unwrap();
    let audit = store.audit(&key, 10).unwrap();
    assert_eq!(audit.array().len(), 1);
    assert!(audit.array()[0].get("event").is("observation_recorded"));
    assert_eq!(
        audit.array()[0].get("observationHandle"),
        result.get("observationHandle")
    );
    assert_eq!(audit.array()[0].get("by"), result.get("attribution"));
    assert_eq!(
        audit.array()[0].get("sourceRevision"),
        result.get("source").get("sourceRevision")
    );
}

#[test]
fn duplicate_fields_reject_before_source_or_publication() {
    let root = root("duplicate");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let mut source = Source::observed();
    source.error = Some(Fault::error("must not be read"));
    let clock = Clock::new(NOW);
    let mut input = input();
    input.set(
        "suppliedValues",
        Js::Arr(vec![proposal("progress", "a"), proposal("progress", "b")]),
    );
    assert_eq!(
        run(&store, &mut source, &Policy::new(true), &clock, &input)
            .unwrap_err()
            .message,
        "duplicate_proposal_field"
    );
    assert!(source.calls.is_empty());
    assert_no_publication(&store, &clock);
}

#[test]
fn field_order_missingness_facts_options_and_memo_interview_remain_distinct() {
    let root = root("fields");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let mut source = Source::observed();
    let mut observation = observation();
    let mut fields = observation.get("fields").clone();
    fields.set(
        "progress",
        obj(&[
            ("state", "unknown".into()),
            ("reasonCode", "not_observed".into()),
        ]),
    );
    fields.set("homework", known("\u{feff}\u{a0}"));
    fields.set("lessonMemo", known(""));
    fields.set("attendance", known(" "));
    fields.set("dailyTestAssessment", known("-1"));
    observation.set("fields", fields);
    set_observation(&mut source, observation.clone());
    let mut input = input();
    let mut context = context();
    context.set("selectedFields", js(json!(["progress", "attendance"])));
    context.set("memoAnswer", js(json!({"kind":"not_supplied"})));
    context.set(
        "facts",
        Js::Arr(vec![
            fact("progress", "Progress fact"),
            fact("attendance", "Y"),
        ]),
    );
    input.set("context", context);
    input.set("suppliedValues", Js::Arr(vec![proposal("attendance", "Y")]));
    let result = run(
        &store,
        &mut source,
        &Policy::new(true),
        &Clock::new(NOW),
        &input,
    )
    .unwrap();
    assert!(result.get("status").is("interview_required"));
    assert!(result.get("observationPersistence").is("owner_durable"));
    let rows = result.get("fields").array();
    let expected = [
        ("progress", "user_facts_available", true),
        ("homework", "missing", false),
        ("lessonMemo", "preserve_existing", false),
        ("attendance", "explicit_user_value", true),
        ("dailyTestAssessment", "missing", false),
        ("previousHomeworkAssessment", "preserve_existing", false),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (field, disposition, selected)) in rows.iter().zip(expected) {
        assert!(row.get("field").is(field));
        assert!(row.get("disposition").is(disposition));
        assert_eq!(row.get("selected"), &Js::Bool(selected));
        assert_eq!(row.get("before"), observation.get("fields").get(field));
        assert_eq!(
            row.get("target"),
            observation.get(if field == "previousHomeworkAssessment" {
                "previousTarget"
            } else {
                "target"
            })
        );
    }
    assert_eq!(
        result.get("interviews"),
        &js(json!([
            {"field":"homework","reasonCode":"explicit_value_or_qualified_existing_value_required"},
            {"field":"lessonMemo","reasonCode":"today_student_specific_memo_required"},
            {"field":"dailyTestAssessment","reasonCode":"explicit_value_or_qualified_existing_value_required"}
        ]))
    );
    assert!(result.get("blockers").array().is_empty());
    assert!(
        result.get("userContext").get("facts").array()[0]
            .get("origin")
            .is("user_supplied")
    );
}

#[test]
fn invalid_supplied_option_blocks_publication_even_when_source_value_exists() {
    let root = root("proposal-blockers");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let mut input = input();
    let mut context = context();
    context.set("selectedFields", js(json!(["progress"])));
    input.set("context", context);
    let mut p = proposal("attendance", "invalid");
    p.set("factRefs", js(json!(["missing", "missing"])));
    p.set(
        "decision",
        js(json!({"kind":"set","requestRef":"other-request"})),
    );
    input.set("suppliedValues", Js::Arr(vec![p]));
    let clock = Clock::new(NOW);
    let result = run(
        &store,
        &mut Source::observed(),
        &Policy::new(true),
        &clock,
        &input,
    )
    .unwrap();
    assert!(result.get("status").is("held"));
    assert!(result.get("observationHandle").absent());
    assert!(!result.get("observationPersistence").is("owner_durable"));
    assert_eq!(
        result.get("blockers"),
        &js(json!([
            "attendance:field_not_selected",
            "attendance:current_request_ref_required",
            "attendance:invalid_explicit_field_value",
            "attendance:field_fact_reference_required",
            "attendance:explicit_user_source_option_fact_required",
            "attendance:duplicate_fact_ref"
        ]))
    );
    assert!(
        result.get("fields").array()[3]
            .get("disposition")
            .is("preserve_existing")
    );
    assert_no_publication(&store, &clock);
}

#[test]
fn freshness_is_inclusive_and_uses_shared_runtime_clock() {
    for (name, observed, accepted) in [
        ("age-zero", NOW, true),
        ("age-limit", "2040-01-04T11:55:00.000Z", true),
        ("age-over", "2040-01-04T11:54:59.999Z", false),
        ("future", "2040-01-04T12:00:00.001Z", false),
    ] {
        let root = root(name);
        let store = Store::open(&root.join("store"), &root).unwrap();
        let mut source = Source::observed();
        let mut observation = observation();
        observation.set("observedAt", observed.into());
        set_observation(&mut source, observation);
        let clock = Clock::new(NOW);
        let result = run(&store, &mut source, &Policy::new(true), &clock, &input()).unwrap();
        assert_eq!(source.calls.len(), 1);
        if accepted {
            assert!(result.get("status").is("ready_for_review"), "{name}");
            assert!(
                !store
                    .get(
                        Kind::Observation,
                        &result.get("observationHandle").code_units()
                    )
                    .unwrap()
                    .absent()
            );
        } else {
            assert!(result.get("status").is("held"), "{name}");
            assert_eq!(result.get("blockers"), &js(json!(["stale_source"])));
            assert_no_publication(&store, &clock);
        }
    }
    let root = root("invalid-clock");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new("invalid");
    let mut source = Source::observed();
    assert_eq!(
        run(&store, &mut source, &Policy::new(true), &clock, &input())
            .unwrap_err()
            .message,
        "workflow_clock_invalid"
    );
    assert_eq!(source.calls.len(), 1);
    assert_no_publication(&store, &clock);
}

#[test]
fn unavailable_and_unqualified_sources_preserve_safe_failure_without_a_handle() {
    for mode in [
        "unknown-error",
        "scope-conflict",
        "declared-unavailable",
        "no-previous-target",
    ] {
        let root = root(mode);
        let store = Store::open(&root.join("store"), &root).unwrap();
        let clock = Clock::new(NOW);
        let mut source = Source::observed();
        match mode {
            "unknown-error" => source.error = Some(Fault::type_error("fictional private detail")),
            "scope-conflict" => {
                let mut observation = observation();
                let mut target = observation.get("target").clone();
                target.set("studentId", "other-student".into());
                observation.set("target", target);
                set_observation(&mut source, observation);
            }
            "no-previous-target" => {
                let mut observation = observation();
                observation.set("previousTarget", Js::Null);
                set_observation(&mut source, observation);
            }
            _ => {
                source.result =
                    js(json!({"status":"unavailable", "reasonCode":"not a public reason"}))
            }
        }
        let result = run(&store, &mut source, &Policy::new(true), &clock, &input()).unwrap();
        assert!(result.get("status").is("held"));
        assert!(result.get("sourceAvailability").is("unavailable"));
        assert!(result.get("source").absent());
        assert!(result.get("observationHandle").absent());
        assert!(
            result.get("fields").array().iter().all(|r| r
                .get("before")
                .get("reasonCode")
                .is("source_not_read")
                && r.get("target").absent())
        );
        if mode == "unknown-error" {
            assert!(
                result.get("blockers").array()[0]
                    .is("dailyrecord_source_unqualified_no_automatic_retry")
            );
            assert!(result.get("failure").get("errorKind").is("TypeError"));
            assert!(
                result
                    .get("failure")
                    .get("errorSha256")
                    .is(&sha("TypeError\0fictional private detail"))
            );
            assert!(
                !stringify(&result)
                    .unwrap()
                    .contains("fictional private detail")
            );
        } else if mode == "declared-unavailable" {
            assert_eq!(
                result.get("blockers"),
                &js(json!(["source_read_unavailable"]))
            );
            assert!(result.get("failure").absent());
        } else {
            assert!(!result.get("failure").absent());
        }
        assert_no_publication(&store, &clock);
    }
}

#[test]
fn tone_filter_requires_an_available_parent_safeguard() {
    for parent in [false, true] {
        let root = root(if parent {
            "tone-present"
        } else {
            "tone-absent"
        });
        let store = Store::open(&root.join("store"), &root).unwrap();
        let mut source = Source::observed();
        let mut observation = observation();
        let mut tones = observation.get("toneSamples").clone();
        let mut rejected = tones.array()[0].clone();
        rejected.set("text", "Not a permitted hyphen line".into());
        tones.push(rejected);
        observation.set("toneSamples", tones.clone());
        set_observation(&mut source, observation);
        let policy = Policy::new(parent);
        let result = run(&store, &mut source, &policy, &Clock::new(NOW), &input()).unwrap();
        assert_eq!(
            result.get("toneSamples"),
            &if parent {
                Js::Arr(vec![tones.array()[0].clone()])
            } else {
                Js::Arr(vec![])
            }
        );
        assert_eq!(policy.line_calls.get(), if parent { 2 } else { 0 });
        assert!(
            result
                .get("toneSelection")
                .is("hyphen_lines_only_other_formats_ignored")
        );
    }
}

#[test]
fn intake_handle_is_consumed_by_fresh_store_and_service_review() {
    let root = root("fresh-review");
    let store_path = root.join("store");
    let clock = Clock::new(NOW);
    let policy = Policy::new(true);
    let mut context = context();
    context.set("selectedFields", js(json!(["progress"])));
    context.set("facts", Js::Arr(vec![fact("progress", "Updated progress")]));
    let intake = {
        let store = Store::open(&store_path, &root).unwrap();
        let mut source = Source::observed();
        let mut service = service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut Backend,
            runtime: &clock,
        };
        service
            .intake(&obj(&[("context", context.clone())]))
            .unwrap()
    };
    assert!(intake.get("status").is("ready_for_review"));
    let store = Store::open(&store_path, &root).unwrap();
    let mut source = Source::observed();
    let input = obj(&[
        ("context", context.clone()),
        ("observationHandle", intake.get("observationHandle").clone()),
        (
            "proposals",
            Js::Arr(vec![proposal("progress", "- Updated progress")]),
        ),
    ]);
    let result = service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut Backend,
        runtime: &clock,
    }
    .review(&input, true)
    .unwrap();
    assert!(
        result.get("status").is("prepared"),
        "{}",
        stringify(&result).unwrap()
    );
    assert_eq!(source.calls.len(), 1);
    assert_eq!(
        source.calls[0].get("subject"),
        &js(json!({"displayName":"Invented Intake Student"}))
    );
    let saved_review = store
        .get(Kind::Review, &result.get("reviewDigest").code_units())
        .unwrap();
    assert_eq!(
        saved_review.get("observationHandle"),
        intake.get("observationHandle")
    );
    assert_eq!(saved_review.get("patch").array().len(), 1);
    assert!(
        saved_review.get("patch").array()[0]
            .get("before")
            .is("- Existing progress")
    );
    assert!(
        saved_review.get("patch").array()[0]
            .get("after")
            .is("- Updated progress")
    );
    let mut wrong_request = context;
    wrong_request.set("currentRequestRef", "other-request".into());
    let mut wrong_input = input;
    wrong_input.set("context", wrong_request);
    let error = service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut Backend,
        runtime: &clock,
    }
    .review(&wrong_input, true)
    .unwrap_err();
    assert_eq!(error.message, "exact_backend_observation_handle_required");
    assert_eq!(source.calls.len(), 1, "wrong request must not read");
}

#[test]
fn audit_failure_preserves_publication_and_publish_failure_never_audits() {
    for fail_audit in [true, false] {
        let root = root(if fail_audit {
            "audit-failure"
        } else {
            "publish-failure"
        });
        let store_path = root.join("store");
        let store = Store::open(&store_path, &root).unwrap();
        let key = request_key(context().get("scope"), "fictional-request").unwrap();
        if fail_audit {
            // A file at this test-owned directory path rejects append_audit after
            // publication. No store/production hook or injected success result.
            fs::write(
                store.root().join("audit").join(sha(&key)),
                b"intentional audit failure",
            )
            .unwrap();
        } else {
            fs::remove_dir(store.root().join("observation")).unwrap();
            fs::write(
                store.root().join("observation"),
                b"intentional publication failure",
            )
            .unwrap();
        }
        let clock = Clock::new(NOW);
        let error = run(
            &store,
            &mut Source::observed(),
            &Policy::new(true),
            &clock,
            &input(),
        )
        .unwrap_err();
        // Audit mkdir at an existing file maps AlreadyExists to EEXIST; the
        // publication open beneath a regular-file parent maps the distinct error.
        assert_eq!(
            error.message,
            if fail_audit {
                "EEXIST"
            } else {
                "workflow_filesystem_failed"
            }
        );
        if fail_audit {
            let bytes = fs::read_dir(store.root().join("observation"))
                .unwrap()
                .map(|e| fs::read(e.unwrap().path()).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(bytes.len(), 1);
            drop(store);
            let reopened = Store::open(&store_path, &root).unwrap();
            let stored = reopened.get(Kind::Observation, FIRST_HANDLE).unwrap();
            assert_eq!(stored.get("observation"), &observation());
            assert!(stored.get("requestRef").is("fictional-request"));
            assert_eq!(
                clock.sequence.get(),
                2,
                "no retry after publication/temp creation"
            );
        } else {
            assert!(
                fs::read_dir(store.root().join("audit"))
                    .unwrap()
                    .next()
                    .is_none()
            );
            assert_eq!(
                clock.sequence.get(),
                2,
                "handle and one publication attempt only"
            );
        }
    }
}

#[test]
fn astral_request_crash_before_resolution_blocks_a_later_request_after_reopen() {
    use edu_academy_core::workflow::{approval::RequestApproval, state};
    struct CrashAfterFence(Clock);
    impl Runtime for CrashAfterFence {
        fn now(&self) -> String {
            self.0.now()
        }
        fn epoch_ms(&self) -> u64 {
            0
        }
        fn uuid(&self) -> Result<String> {
            self.0.uuid()
        }
        fn fenced(&self) -> Result<()> {
            Err(Fault::error("fictional_crash_before_resolution"))
        }
    }
    for physical in ["current-record", "record-😀"] {
        let root = root("unicode-fence");
        let store = Store::open(&root.join("store"), &root).unwrap();
        let scope = context().get("scope").clone();
        let reviewed = js(json!({"field":"progress", "effectRecordKey":physical,
            "before":"- Existing progress", "after":"- Updated progress"}));
        let mut approval = RequestApproval {
            store: &store,
            input: obj(&[
                ("requestRef", "req-😀".into()),
                ("scope", scope.clone()),
                ("reviewed", Js::Arr(vec![reviewed.clone()])),
            ]),
            runtime: &CrashAfterFence(Clock::new(NOW)),
        };
        let action = js(
            json!({"target":{"studentKey":"student", "teacherKey":"teacher", "date":"2040-01-04"},
            "field":"progress", "effectRecordKey":physical,"before":"- Existing progress","after":"- Updated progress"}),
        );
        assert_eq!(
            approval.consume(&action).unwrap_err().message,
            "fictional_crash_before_resolution"
        );
        drop(approval);
        let store = Store::open(&root.join("store"), &root).unwrap();
        let exact_key = state::execution_key(
            "req-😀",
            &scope,
            "progress",
            physical,
            &"- Existing progress".into(),
            &"- Updated progress".into(),
        )
        .unwrap();
        assert!(!store.get(Kind::Execution, &exact_key).unwrap().absent());
        assert_eq!(
            state::unresolved_on_field(&store, &scope, "progress", physical)
                .unwrap()
                .len(),
            1
        );
        let mut source = Source::observed();
        let mut observation = source.result.get("observation").clone();
        let mut target = observation.get("target").clone();
        target.set("recordId", physical.into());
        observation.set("target", target);
        source.result.set("observation", observation);
        let mut context = context();
        context.set("currentRequestRef", "later-🚀".into());
        context.set("selectedFields", js(json!(["progress"])));
        context.set("facts", Js::Arr(vec![fact("progress", "Updated progress")]));
        let mut backend = Backend; // Panics if the unresolved fence permits dispatch.
        let policy = Policy::new(true);
        let clock = Clock::new(NOW);
        let mut svc = service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock,
        };
        let intake = svc.intake(&obj(&[("context", context.clone())])).unwrap();
        let mut proposed = proposal("progress", "- Updated progress");
        proposed.set(
            "decision",
            js(json!({"kind":"replace", "requestRef":"later-🚀"})),
        );
        let mut input = obj(&[
            ("context", context),
            ("observationHandle", intake.get("observationHandle").clone()),
            ("proposals", Js::Arr(vec![proposed])),
        ]);
        let review = svc.review(&input, true).unwrap();
        assert!(
            review.get("status").is("prepared"),
            "{}",
            stringify(&review).unwrap()
        );
        input.set("reviewDigest", review.get("reviewDigest").clone());
        input.set("request", js(json!({"requestRef":"later-🚀","instruction":{"text":"Save this reviewed fictional field."}})));
        let held = svc.save(&input).unwrap();
        assert!(
            held.get("reasonCode")
                .is("uncertain_dispatch_reconcile_required"),
            "{}",
            stringify(&held).unwrap()
        );
        assert_eq!(held.get("academyWrites"), &Js::Num(0.0));
    }
}

#[test]
fn index_only_crash_allows_a_fresh_save_after_reopen() {
    use edu_academy_core::workflow::{approval::RequestApproval, state};
    struct CrashAfterIndex(Clock);
    impl Runtime for CrashAfterIndex {
        fn now(&self) -> String {
            self.0.now()
        }
        fn epoch_ms(&self) -> u64 {
            0
        }
        fn uuid(&self) -> Result<String> {
            self.0.uuid()
        }
        fn indexed(&self) -> Result<()> {
            Err(Fault::error("fictional_crash_after_index"))
        }
    }
    struct FictionalSave {
        dispatches: usize,
    }
    impl service::Backend for FictionalSave {
        fn configured(&self) -> bool {
            true
        }
        fn save(
            &mut self,
            scope: &Js,
            patches: &Js,
            approval: &mut dyn Approval,
            _: &Js,
        ) -> Result<Js> {
            assert_eq!(patches.array().len(), 1);
            let patch = &patches.array()[0];
            let action = obj(&[
                (
                    "target",
                    obj(&[
                        ("studentKey", scope.get("studentId").clone()),
                        ("teacherKey", scope.get("teacherId").clone()),
                        ("date", scope.get("lessonDate").clone()),
                    ]),
                ),
                ("attemptId", "fictional-attempt".into()),
                ("field", patch.get("field").clone()),
                ("effectRecordKey", "record-😀".into()),
                ("before", patch.get("before").clone()),
                ("after", patch.get("value").clone()),
            ]);
            assert!(approval.consume(&action)?.get("status").is("verified"));
            self.dispatches += 1;
            Ok(js(
                json!([{"field":"progress","status":"persisted","dispatched":true,"transport":"returned"}]),
            ))
        }
    }
    // Both retrying the never-dispatched request and using a new request are valid.
    for request in ["req-😀", "later-🚀"] {
        let root = root("index-only-recovery");
        let store = Store::open(&root.join("store"), &root).unwrap();
        let scope = context().get("scope").clone();
        let reviewed = js(json!({"field":"progress","effectRecordKey":"record-😀",
            "before":"- Existing progress","after":"- Updated progress"}));
        let key = state::execution_key(
            "req-😀",
            &scope,
            "progress",
            "record-😀",
            reviewed.get("before"),
            reviewed.get("after"),
        )
        .unwrap();
        let crash_clock = CrashAfterIndex(Clock::new(NOW));
        let mut approval = RequestApproval {
            store: &store,
            input: obj(&[
                ("requestRef", "req-😀".into()),
                ("scope", scope.clone()),
                ("reviewed", Js::Arr(vec![reviewed])),
            ]),
            runtime: &crash_clock,
        };
        let action = js(
            json!({"target":{"studentKey":"student","teacherKey":"teacher","date":"2040-01-04"},
            "field":"progress","effectRecordKey":"record-😀","before":"- Existing progress","after":"- Updated progress"}),
        );
        assert_eq!(
            approval.consume(&action).unwrap_err().message,
            "fictional_crash_after_index"
        );
        drop(approval);
        drop(store);
        let store = Store::open(&root.join("store"), &root).unwrap();
        assert_eq!(
            store.ledger(&state::lock_key(&scope).unwrap()).unwrap(),
            vec![key.clone()]
        );
        assert!(store.get(Kind::Execution, &key).unwrap().absent());
        assert_eq!(
            state::field_state(&store, &key).unwrap(),
            state::FieldState::Available
        );
        assert!(
            state::student_executions(&store, &scope)
                .unwrap()
                .is_empty()
        );
        let mut source = Source::observed();
        let mut observation = source.result.get("observation").clone();
        let mut target = observation.get("target").clone();
        target.set("recordId", "record-😀".into());
        observation.set("target", target);
        source.result.set("observation", observation);
        let mut context = context();
        context.set("currentRequestRef", request.into());
        context.set("selectedFields", js(json!(["progress"])));
        context.set("facts", Js::Arr(vec![fact("progress", "Updated progress")]));
        let mut backend = FictionalSave { dispatches: 0 };
        let policy = Policy::new(true);
        let clock = Clock::new(NOW);
        let mut svc = service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock,
        };
        let intake = svc.intake(&obj(&[("context", context.clone())])).unwrap();
        let mut proposed = proposal("progress", "- Updated progress");
        proposed.set(
            "decision",
            js(json!({"kind":"replace", "requestRef":request})),
        );
        let mut input = obj(&[
            ("context", context),
            ("observationHandle", intake.get("observationHandle").clone()),
            ("proposals", Js::Arr(vec![proposed])),
        ]);
        let review = svc.review(&input, true).unwrap();
        assert!(
            review.get("status").is("prepared"),
            "{}",
            stringify(&review).unwrap()
        );
        input.set("reviewDigest", review.get("reviewDigest").clone());
        input.set("request", js(json!({"requestRef":request,"instruction":{"text":"Save this reviewed fictional field."}})));
        let saved = svc.save(&input).unwrap();
        assert!(
            saved.get("status").is("saved"),
            "{}",
            stringify(&saved).unwrap()
        );
        assert_eq!(saved.get("academyWrites"), &Js::Num(1.0));
        assert_eq!(backend.dispatches, 1);
        let executions = state::student_executions(&store, &scope).unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].state, state::FieldState::SettledPersisted);
        assert!(executions[0].record.get("requestRef").is(request));
        if request != "req-😀" {
            assert!(store.get(Kind::Execution, &key).unwrap().absent());
            assert_eq!(
                store
                    .ledger(&state::lock_key(&scope).unwrap())
                    .unwrap()
                    .len(),
                2
            );
        }
    }
}

#[test]
fn supplementary_request_refs_remain_exact_through_save_and_distinct_store_keys() {
    use edu_academy_core::workflow::state;
    struct FictionalSave(usize);
    impl service::Backend for FictionalSave {
        fn configured(&self) -> bool {
            true
        }
        fn save(
            &mut self,
            scope: &Js,
            patches: &Js,
            approval: &mut dyn Approval,
            _: &Js,
        ) -> Result<Js> {
            let patch = &patches.array()[0];
            let action = obj(&[
                (
                    "target",
                    obj(&[
                        ("studentKey", scope.get("studentId").clone()),
                        ("teacherKey", scope.get("teacherId").clone()),
                        ("date", scope.get("lessonDate").clone()),
                    ]),
                ),
                ("attemptId", "fictional-exact-ref".into()),
                ("field", patch.get("field").clone()),
                ("effectRecordKey", "current-record".into()),
                ("before", patch.get("before").clone()),
                ("after", patch.get("value").clone()),
            ]);
            assert!(approval.consume(&action)?.get("status").is("verified"));
            self.0 += 1;
            Ok(js(
                json!([{"field":"progress","status":"persisted","dispatched":true,"transport":"returned"}]),
            ))
        }
    }
    let root = root("exact-request-refs");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let scope = context().get("scope").clone();
    let clock = Clock::new(NOW);
    let policy = Policy::new(true);
    let mut source = Source::observed();
    let mut backend = FictionalSave(0);
    let mut keys = vec![];
    for request in ["req😀", "req😃"] {
        let mut context = context();
        context.set("currentRequestRef", request.into());
        context.set("selectedFields", js(json!(["progress"])));
        context.set("facts", Js::Arr(vec![fact("progress", "Updated progress")]));
        let mut svc = service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock,
        };
        let intake = svc.intake(&obj(&[("context", context.clone())])).unwrap();
        let mut proposed = proposal("progress", "- Updated progress");
        proposed.set(
            "decision",
            js(json!({"kind":"replace", "requestRef":request})),
        );
        let mut input = obj(&[
            ("context", context),
            ("observationHandle", intake.get("observationHandle").clone()),
            ("proposals", Js::Arr(vec![proposed])),
        ]);
        let review = svc.review(&input, true).unwrap();
        assert!(review.get("status").is("prepared"));
        input.set("reviewDigest", review.get("reviewDigest").clone());
        let wrong = if request == "req😀" {
            "req😃"
        } else {
            "req😀"
        };
        input.set(
            "request",
            js(json!({"requestRef":wrong,"instruction":{"text":"Fictional save."}})),
        );
        assert!(
            svc.save(&input)
                .unwrap()
                .get("reasonCode")
                .is("user_request_ref_mismatch")
        );
        input.set(
            "request",
            js(json!({"requestRef":request,"instruction":{"text":"Fictional save."}})),
        );
        let saved = svc.save(&input).unwrap();
        assert!(
            saved.get("status").is("saved"),
            "{}",
            stringify(&saved).unwrap()
        );
        let key = request_key(&scope, request).unwrap();
        let stored = store.get(Kind::Request, &key).unwrap();
        assert!(stored.get("request").get("requestRef").is(request));
        keys.push(key);
    }
    assert_ne!(keys[0], keys[1]);
    assert_eq!(backend.0, 2);
    let executions = state::student_executions(&store, &scope).unwrap();
    assert_eq!(executions.len(), 2);
    assert_ne!(executions[0].key, executions[1].key);
    for request in ["req😀", "req😃"] {
        assert_eq!(
            executions
                .iter()
                .filter(|e| e.record.get("requestRef").is(request))
                .count(),
            1
        );
    }
    assert!(
        source
            .calls
            .iter()
            .all(|c| c.get("requestRef").is("req😀") || c.get("requestRef").is("req😃"))
    );
}
