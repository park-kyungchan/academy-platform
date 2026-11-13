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
    fn excluded_parent_language(&self, value: &Js) -> bool {
        edu_academy_core::templates::parent_internal(value)
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
    let mut fields = obj(&[
        ("progress", known("- Existing progress")),
        ("homework", known("- Existing homework")),
        ("lessonMemo", known("- Existing memo")),
        ("attendance", known("Y")),
        ("dailyTestAssessment", known("0")),
        ("previousHomeworkAssessment", known("0")),
    ]);
    for field in review::FIELDS {
        let previous_field = field == "previousHomeworkAssessment";
        let mut value = fields.get(field).clone();
        value.set(
            "effectTarget",
            if previous_field {
                "synthetic-previous-target"
            } else {
                "synthetic-target"
            }
            .into(),
        );
        value.set(
            "effectRecordKey",
            if previous_field {
                previous.get("recordId")
            } else {
                target.get("recordId")
            }
            .clone(),
        );
        fields.set(field, value);
    }
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
        let target = observation().get("target").clone();
        store
            .publish(
                Kind::Review,
                "crash-review",
                &obj(&[(
                    "patch",
                    Js::Arr(vec![obj(&[
                        ("field", "progress".into()),
                        ("target", target.clone()),
                    ])]),
                )]),
                &Clock::new(NOW),
            )
            .unwrap();
        let reviewed = js(
            json!({"field":"progress", "effectRecordKey":physical,"effectTarget":"synthetic-target",
            "before":"- Existing progress", "after":"- Updated progress"}),
        );
        let mut approval = RequestApproval {
            store: &store,
            input: obj(&[
                ("requestRef", "req-😀".into()),
                ("reviewDigest", "crash-review".into()),
                ("scope", scope.clone()),
                ("reviewed", Js::Arr(vec![reviewed.clone()])),
            ]),
            runtime: &CrashAfterFence(Clock::new(NOW)),
        };
        let action = js(
            json!({"target":{"studentKey":"student", "teacherKey":"teacher", "date":"2040-01-04"},
            "field":"progress", "effectRecordKey":physical,"effectTarget":"synthetic-target","before":"- Existing progress","after":"- Updated progress"}),
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
            &"synthetic-target".into(),
        )
        .unwrap();
        assert!(!store.get(Kind::Execution, &exact_key).unwrap().absent());
        assert_eq!(
            state::unresolved_on_field(&store, &scope, "progress", physical, &target)
                .unwrap()
                .len(),
            1
        );
        let mut source = Source::observed();
        let mut observation = source.result.get("observation").clone();
        let mut target = observation.get("target").clone();
        target.set("recordId", physical.into());
        observation.set("target", target);
        let mut progress = observation.get("fields").get("progress").clone();
        progress.set("effectRecordKey", physical.into());
        let mut fields = observation.get("fields").clone();
        fields.set("progress", progress);
        observation.set("fields", fields);
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
        assert!(held.get("earliestSettleAtEpochMs").absent());
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
                ("effectTarget", patch.get("effectTarget").clone()),
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
        let reviewed = js(
            json!({"field":"progress","effectRecordKey":"record-😀","effectTarget":"synthetic-target",
            "before":"- Existing progress","after":"- Updated progress"}),
        );
        let key = state::execution_key(
            "req-😀",
            &scope,
            "progress",
            "record-😀",
            reviewed.get("before"),
            reviewed.get("after"),
            &"synthetic-target".into(),
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
            "field":"progress","effectRecordKey":"record-😀","effectTarget":"synthetic-target","before":"- Existing progress","after":"- Updated progress"}),
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
        let mut fields = observation.get("fields").clone();
        let mut progress = fields.get("progress").clone();
        progress.set("effectRecordKey", "record-😀".into());
        fields.set("progress", progress);
        observation.set("fields", fields);
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
            saved.get("status").is("held"),
            "{}",
            stringify(&saved).unwrap()
        );
        assert_eq!(saved.get("academyWrites"), &Js::Num(1.0));
        assert_eq!(backend.dispatches, 1);
        let executions = state::student_executions(&store, &scope).unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(
            executions[0].state,
            state::FieldState::ReconciliationRequired
        );
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
                ("effectTarget", patch.get("effectTarget").clone()),
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
            saved.get("status").is("held"),
            "{}",
            stringify(&saved).unwrap()
        );
        let key = request_key(&scope, request).unwrap();
        let stored = store.get(Kind::Request, &key).unwrap();
        assert!(stored.get("request").get("requestRef").is(request));
        keys.push(key);
    }
    assert_ne!(keys[0], keys[1]);
    assert_eq!(backend.0, 1);
    let executions = state::student_executions(&store, &scope).unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(
        executions
            .iter()
            .filter(|e| e.record.get("requestRef").is("req😀"))
            .count(),
        1
    );
    assert!(
        source
            .calls
            .iter()
            .all(|c| c.get("requestRef").is("req😀") || c.get("requestRef").is("req😃"))
    );
}

#[test]
fn historical_parent_codes_remain_readable_but_new_wording_requires_admission() {
    let old = "- 가우스 5-1-2 [1P6A] 준비";
    for (after, blocked) in [
        (old, false),
        ("- 가우스 (1U0T) 준비", true),
        ("- 가우스 5-1-2 준비", false),
    ] {
        let root = root("historical-parent-guard");
        let store = Store::open(&root.join("store"), &root).unwrap();
        let mut source = Source::observed();
        let mut observed = source.result.get("observation").clone();
        let mut fields = observed.get("fields").clone();
        let mut progress = fields.get("progress").clone();
        progress.set("value", old.into());
        fields.set("progress", progress);
        observed.set("fields", fields);
        set_observation(&mut source, observed.clone());
        let policy = Policy::new(true);
        let clock = Clock::new(NOW);
        let mut context = context();
        context.set("selectedFields", js(json!(["progress"])));
        context.set(
            "facts",
            Js::Arr(vec![fact("progress", after.trim_start_matches("- "))]),
        );
        let mut svc = service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut Backend,
            runtime: &clock,
        };
        let intake = svc.intake(&obj(&[("context", context.clone())])).unwrap();
        assert!(
            intake.get("status").is("ready_for_review"),
            "{}",
            stringify(&intake).unwrap()
        );
        let before = store
            .get(
                Kind::Observation,
                &intake.get("observationHandle").code_units(),
            )
            .unwrap();
        assert_eq!(before.get("observation"), &observed);
        let review = svc
            .review(
                &obj(&[
                    ("context", context),
                    ("observationHandle", intake.get("observationHandle").clone()),
                    ("proposals", Js::Arr(vec![proposal("progress", after)])),
                ]),
                true,
            )
            .unwrap();
        let row = &review.get("rows").array()[0];
        assert_eq!(
            row.get("blockers")
                .array()
                .iter()
                .any(|v| v.is("parent_internal_language_excluded")),
            blocked,
            "{}",
            stringify(&review).unwrap()
        );
        assert_eq!(
            store
                .get(
                    Kind::Observation,
                    &intake.get("observationHandle").code_units()
                )
                .unwrap(),
            before
        );
    }
}

// These fictional dispatches use the real RequestApproval/Store fence, then lose
// their answer. No native effect, final completion or saved claim is fabricated.
struct BoundBackend(Vec<Js>);
impl service::Backend for BoundBackend {
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
        for p in patches.array() {
            let action = obj(&[
                (
                    "target",
                    obj(&[
                        ("studentKey", scope.get("studentId").clone()),
                        ("teacherKey", scope.get("teacherId").clone()),
                        ("date", scope.get("lessonDate").clone()),
                    ]),
                ),
                (
                    "attemptId",
                    format!("invented-{}", p.get("field").code_units()).into(),
                ),
                ("field", p.get("field").clone()),
                ("effectTarget", p.get("effectTarget").clone()),
                ("effectRecordKey", p.get("effectRecordKey").clone()),
                ("before", p.get("before").clone()),
                ("after", p.get("value").clone()),
            ]);
            assert!(approval.consume(&action)?.get("status").is("verified"));
            self.0.push(action);
        }
        Err(Fault::error("invented_answer_lost_after_fence"))
    }
}
fn bound_input(
    store: &Store,
    source: &mut Source,
    clock: &Clock,
    req: &str,
    fields: &[(&str, &str)],
) -> Result<Js> {
    let policy = Policy::new(true);
    let mut context = context();
    context.set(
        "scope",
        source.result.get("observation").get("scope").clone(),
    );
    context.set("currentRequestRef", req.into());
    context.set(
        "selectedFields",
        Js::Arr(fields.iter().map(|(f, _)| (*f).into()).collect()),
    );
    context.set(
        "facts",
        Js::Arr(fields.iter().map(|(f, v)| fact(f, v)).collect()),
    );
    let mut proposals = Vec::new();
    for (field, value) in fields {
        let mut p = proposal(field, value);
        p.set("decision", js(json!({"kind":"replace","requestRef":req})));
        proposals.push(p);
    }
    let mut svc = service::Service {
        store,
        source,
        policy: &policy,
        backend: &mut Backend,
        runtime: clock,
    };
    let intake = svc.intake(&obj(&[("context", context.clone())]))?;
    let mut input = obj(&[
        ("context", context),
        ("observationHandle", intake.get("observationHandle").clone()),
        ("proposals", Js::Arr(proposals)),
    ]);
    let reviewed = svc.review(&input, true)?;
    assert!(
        reviewed.get("status").is("prepared"),
        "{}",
        stringify(&reviewed)?
    );
    input.set("reviewDigest", reviewed.get("reviewDigest").clone());
    input.set(
        "request",
        js(json!({"requestRef":req,"instruction":{"text":"Invented explicit save instruction"}})),
    );
    Ok(input)
}
fn bind_field(source: &mut Source, field: &str, target: Js, key: &str) {
    let mut o = source.result.get("observation").clone();
    let mut fields = o.get("fields").clone();
    let mut value = fields.get(field).clone();
    value.set("effectTarget", target);
    value.set("effectRecordKey", key.into());
    fields.set(field, value);
    o.set("fields", fields);
    source.result.set("observation", o);
}
#[test]
fn reviewed_effect_target_reaches_backend_and_fence() {
    use edu_academy_core::workflow::{canonical, state};
    let root = root("bound-target");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let policy = Policy::new(true);
    let mut source = Source::observed();
    for field in ["progress", "previousHomeworkAssessment"] {
        let target = source
            .result
            .get("observation")
            .get(if field == "progress" {
                "target"
            } else {
                "previousTarget"
            })
            .clone();
        bind_field(
            &mut source,
            field,
            canonical(&target).unwrap().into(),
            if field == "progress" {
                "current-record"
            } else {
                "previous-record"
            },
        );
    }
    let input = bound_input(
        &store,
        &mut source,
        &clock,
        "bound-request",
        &[
            ("progress", "- Updated progress"),
            ("previousHomeworkAssessment", "1"),
        ],
    )
    .unwrap();
    let review = store
        .get(Kind::Review, &input.get("reviewDigest").code_units())
        .unwrap();
    assert_eq!(review.get("effectBindingVersion"), &Js::Num(2.0));
    let mut backend = BoundBackend(vec![]);
    let result = service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut backend,
        runtime: &clock,
    }
    .save(&input)
    .unwrap();
    assert!(result.get("status").is("held"));
    assert_eq!(backend.0.len(), 2);
    let executions = state::student_executions(&store, input.get("context").get("scope")).unwrap();
    assert_eq!(executions.len(), 2);
    for action in &backend.0 {
        let p = review
            .get("patch")
            .array()
            .iter()
            .find(|p| p.get("field") == action.get("field"))
            .unwrap();
        assert_eq!(p.get("effectTarget"), action.get("effectTarget"));
        assert_eq!(p.get("effectRecordKey"), action.get("effectRecordKey"));
        let e = executions
            .iter()
            .find(|e| e.record.get("field") == action.get("field"))
            .unwrap();
        assert_eq!(e.record.get("effectTarget"), action.get("effectTarget"));
        assert!(e.state.needs_reconciliation());
        assert_eq!(
            e.record.get("actionDigest"),
            &sha(&canonical(action).unwrap()).into()
        );
        assert_eq!(
            codec::parse(e.key.as_bytes()).unwrap().get("effectTarget"),
            action.get("effectTarget")
        );
    }
    assert_ne!(
        backend.0[0].get("effectTarget"),
        backend.0[1].get("effectTarget")
    );
    let p = &review.get("patch").array()[0];
    let mut drifted = p.clone();
    drifted.set("effectTarget", "different-course/membership".into());
    assert_eq!(
        state::checked_execution_key(
            &store,
            review.get("requestRef"),
            input.get("context").get("scope"),
            &drifted
        )
        .unwrap_err()
        .message,
        "workflow_execution_key_encoding_conflict"
    );
    let mut approval = edu_academy_core::workflow::approval::RequestApproval {
        store: &store,
        runtime: &clock,
        input: obj(&[("reviewed", review.get("patch").clone())]),
    };
    let mut action = backend.0[0].clone();
    action.set("effectTarget", "different-course/membership".into());
    assert_eq!(
        approval.consume(&action).unwrap_err().message,
        "workflow_effect_target_drift"
    );
    assert_eq!(
        state::student_executions(&store, input.get("context").get("scope"))
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn retained_fence_status_after_target_drift_requires_manual_reconciliation() {
    use edu_academy_core::workflow::state;
    let root = root("target-drift");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let policy = Policy::new(true);
    let mut source = Source::observed();
    bind_field(&mut source, "progress", "C1/M1/R".into(), "current-record");
    let input = bound_input(
        &store,
        &mut source,
        &clock,
        "first",
        &[("progress", "- Updated progress")],
    )
    .unwrap();
    let mut backend = BoundBackend(vec![]);
    // Drift between review and save, before any fence.
    bind_field(&mut source, "progress", "C2/M2/R".into(), "current-record");
    let error = service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut backend,
        runtime: &clock,
    }
    .save(&input)
    .unwrap_err();
    assert_eq!(error.message, "workflow_effect_target_drift");
    assert!(backend.0.is_empty());
    bind_field(&mut source, "progress", "C1/M1/R".into(), "current-record");
    service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut backend,
        runtime: &clock,
    }
    .save(&input)
    .unwrap();
    assert_eq!(backend.0.len(), 1);
    bind_field(&mut source, "progress", "C2/M2/R".into(), "current-record");
    // B1: a NEW request cannot narrow away the older unresolved T1/R fence.
    let next = bound_input(
        &store,
        &mut source,
        &clock,
        "second",
        &[("progress", "- Updated progress")],
    )
    .unwrap();
    let error = service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut backend,
        runtime: &clock,
    }
    .save(&next)
    .unwrap_err();
    assert_eq!(error.message, "workflow_effect_target_drift");
    assert_eq!(backend.0.len(), 1);
    let executions = state::student_executions(&store, input.get("context").get("scope")).unwrap();
    assert_eq!(executions.len(), 1);
    let e = &executions[0];
    // Before Q confirmation is pending; at Q a drift still cannot release E1.
    let pending = service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut backend,
        runtime: &clock,
    }
    .reconcile(&obj(&[("context", next.get("context").clone())]))
    .unwrap();
    assert!(pending.get("reasonCode").is("save_confirmation_pending"));
    let at_q = Clock::new("2040-01-04T12:10:00.000Z");
    let reconciled = service::Service {
        store: &store,
        source: &mut source,
        policy: &policy,
        backend: &mut backend,
        runtime: &at_q,
    }
    .reconcile(&obj(&[("context", next.get("context").clone())]))
    .unwrap();
    assert!(reconciled.get("status").is("held"));
    assert!(
        reconciled
            .get("reasonCode")
            .is("qualified_proof_or_target_repair_required")
    );
    assert_eq!(reconciled.get("fields").array().len(), 1);
    let field = &reconciled.get("fields").array()[0];
    assert!(field.get("field").is("progress"));
    assert!(field.get("status").is("uncertain"));
    assert!(field.get("reasonCode").is("workflow_effect_target_drift"));
    assert!(field.get("executionState").is(e.state.name()));
    assert!(store.get(Kind::Reconciliation, &e.key).unwrap().absent());
    let reopened = Store::open(&root.join("store"), &root).unwrap();
    let status = facts_status(&reopened, &input);
    let retained = &status.get("fields").array()[0].get("executions").array()[0];
    assert!(retained.get("reconciliation").is("manual_reconciliation"));
    assert!(retained.get("nextStep").is("ask_person"));
    assert!(retained.get("question").string());
    assert!(!reopened.get(Kind::Execution, &e.key).unwrap().absent());
    assert!(
        state::field_state(&store, &e.key)
            .unwrap()
            .needs_reconciliation()
    );
    let fresh_root = root.join("missing");
    let empty_store = Store::open(&fresh_root, &root).unwrap();
    let mut absent = Source::observed();
    let mut o = absent.result.get("observation").clone();
    let mut fields = o.get("fields").clone();
    fields.set("progress", known("- Existing progress"));
    o.set("fields", fields);
    absent.result.set("observation", o);
    assert_eq!(
        bound_input(
            &empty_store,
            &mut absent,
            &clock,
            "missing",
            &[("progress", "- Updated progress")]
        )
        .unwrap_err()
        .message,
        "workflow_effect_target_required"
    );
    assert_eq!(
        fs::read_dir(empty_store.root().join("review"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn legacy_review_and_execution_keep_their_fences() {
    use edu_academy_core::{js::text, workflow::state};
    let root = root("legacy-binding");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let policy = Policy::new(true);
    let mut source = Source::observed();
    let input = bound_input(
        &store,
        &mut source,
        &clock,
        "legacy",
        &[("progress", "- Updated progress")],
    )
    .unwrap();
    let reviewed = store
        .get(Kind::Review, &input.get("reviewDigest").code_units())
        .unwrap();
    let old = Js::Obj(
        reviewed
            .props()
            .iter()
            .filter(|(k, _)| k != &text("effectBindingVersion"))
            .cloned()
            .collect(),
    );
    let old_store = Store::open(&root.join("old-store"), &root).unwrap();
    old_store
        .publish(
            Kind::Review,
            &input.get("reviewDigest").code_units(),
            &old,
            &clock,
        )
        .unwrap();
    let mut backend = BoundBackend(vec![]);
    assert_eq!(
        service::Service {
            store: &old_store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock
        }
        .save(&input)
        .unwrap_err()
        .message,
        "workflow_legacy_review_unbound"
    );
    let scope = input.get("context").get("scope");
    let legacy = obj(&[
        ("scope", scope.clone()),
        ("requestRef", "older".into()),
        ("field", "progress".into()),
        ("effectRecordKey", "current-record".into()),
    ]);
    store
        .add_ledger(&state::lock_key(scope).unwrap(), "legacy-key", &clock)
        .unwrap();
    store
        .publish(Kind::Execution, "legacy-key", &legacy, &clock)
        .unwrap();
    assert_eq!(
        service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock
        }
        .save(&input)
        .unwrap_err()
        .message,
        "workflow_legacy_execution_unbound"
    );
    assert_eq!(
        service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock
        }
        .reconcile(&obj(&[("context", input.get("context").clone())]))
        .unwrap_err()
        .message,
        "workflow_legacy_execution_unbound"
    );
    assert_eq!(store.get(Kind::Execution, "legacy-key").unwrap(), legacy);
    assert!(
        store
            .get(Kind::Reconciliation, "legacy-key")
            .unwrap()
            .absent()
    );
    assert!(backend.0.is_empty());
}
#[test]
fn effect_target_limit() {
    use edu_academy_core::workflow::schema::MAX_EFFECT_TARGET_UNITS;
    let root = root("target-limit");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let mut source = Source::observed();
    bind_field(
        &mut source,
        "progress",
        "x".repeat(MAX_EFFECT_TARGET_UNITS).into(),
        "current-record",
    );
    let input = bound_input(
        &store,
        &mut source,
        &clock,
        "limit",
        &[("progress", "- Updated progress")],
    )
    .unwrap();
    let review = store
        .get(Kind::Review, &input.get("reviewDigest").code_units())
        .unwrap();
    assert_eq!(
        review.get("patch").array()[0]
            .get("effectTarget")
            .units()
            .len(),
        MAX_EFFECT_TARGET_UNITS
    );
    bind_field(
        &mut source,
        "progress",
        "x".repeat(MAX_EFFECT_TARGET_UNITS + 1).into(),
        "current-record",
    );
    assert_eq!(
        service::qualified_observation(
            source.result.get("observation"),
            input.get("context").get("scope")
        )
        .unwrap_err()
        .message,
        "workflow_effect_target_invalid"
    );
}

const FACT_FIELDS: [(&str, &str); 3] = [
    ("progress", "- Updated progress"),
    ("homework", "- Updated homework"),
    ("previousHomeworkAssessment", "1"),
];
fn facts_input(store: &Store, source: &mut Source, clock: &Clock, count: usize) -> Js {
    bound_input(store, source, clock, "facts", &FACT_FIELDS[..count]).unwrap()
}
fn facts_status(store: &Store, input: &Js) -> Js {
    service::status(
        store,
        &obj(&[("scope", input.get("context").get("scope").clone())]),
    )
    .unwrap()
}
// Scripted claims only: no native completion or application is qualified here.
struct FactsBackend {
    results: Vec<Js>,
    calls: usize,
}
impl service::Backend for FactsBackend {
    fn configured(&self) -> bool {
        true
    }
    fn save(
        &mut self,
        scope: &Js,
        patches: &Js,
        approval: &mut dyn Approval,
        subject: &Js,
    ) -> Result<Js> {
        let mut boundary = BoundBackend(vec![]);
        assert_eq!(
            boundary
                .save(scope, patches, approval, subject)
                .unwrap_err()
                .message,
            "invented_answer_lost_after_fence"
        );
        self.calls += boundary.0.len();
        Ok(Js::Arr(self.results.clone()))
    }
}
fn facts_result(field: &str, status: &str, facts: Option<Js>) -> Js {
    let mut r = obj(&[
        ("field", field.into()),
        ("status", status.into()),
        ("dispatched", true.into()),
        ("transport", "returned".into()),
    ]);
    if let Some(facts) = facts {
        r.set("completionFacts", facts);
    }
    r
}
fn facts_save(
    store: &Store,
    source: &mut Source,
    backend: &mut FactsBackend,
    clock: &Clock,
    input: &Js,
) -> Result<Js> {
    service::Service {
        store,
        source,
        backend,
        policy: &Policy::new(true),
        runtime: clock,
    }
    .save(input)
}
fn facts_reconcile(store: &Store, source: &mut Source, clock: &Clock, input: &Js) -> Js {
    service::Service {
        store,
        source,
        backend: &mut Backend,
        policy: &Policy::new(true),
        runtime: clock,
    }
    .reconcile(&obj(&[("context", input.get("context").clone())]))
    .unwrap()
}
fn assert_facts(fields: &Js, expected: &[Js], status: bool) {
    for e in expected {
        let field = fields
            .array()
            .iter()
            .find(|r| r.get("field") == e.get("field"))
            .unwrap();
        let value = if status {
            &field.get("executions").array()[0]
        } else {
            field
        };
        assert_eq!(value.has("completionFacts"), e.has("completionFacts"));
        assert_eq!(value.get("completionFacts"), e.get("completionFacts"));
    }
}
#[test]
fn opaque_completion_facts_survive_save_reopen_and_repeat() {
    use edu_academy_core::workflow::state;
    for rejected in [false, true] {
        let root = root("facts-roundtrip");
        let store = Store::open(&root.join("store"), &root).unwrap();
        let clock = Clock::new(NOW);
        let mut source = Source::observed();
        let input = facts_input(&store, &mut source, &clock, 2);
        let results = vec![
            facts_result(
                "progress",
                "persisted",
                Some(js(
                    json!({"unqualified":"claim", "target":"opaque-A", "unknown":{"λ":[true,3,null,"😀"]}}),
                )),
            ),
            facts_result(
                "homework",
                if rejected {
                    "not_persisted"
                } else {
                    "persisted"
                },
                Some(js(
                    json!({"target":"opaque-B", "different":["retained",false]}),
                )),
            ),
        ];
        let mut backend = FactsBackend {
            results: results.clone(),
            calls: 0,
        };
        let saved = facts_save(&store, &mut source, &mut backend, &clock, &input).unwrap();
        assert_facts(saved.get("fields"), &results, false);
        assert_eq!(backend.calls, 2);
        for e in state::student_executions(&store, input.get("context").get("scope")).unwrap() {
            let original = store.get(Kind::Outcome, &e.key).unwrap();
            let expected = results
                .iter()
                .find(|r| r.get("field") == e.record.get("field"))
                .unwrap();
            assert_eq!(
                original.get("completionFacts"),
                expected.get("completionFacts")
            );
            let mut replacement = original.clone();
            replacement.set("completionFacts", Js::Null);
            let (created, surviving) = store
                .publish(Kind::Outcome, &e.key, &replacement, &clock)
                .unwrap();
            assert!(!created);
            assert_eq!(surviving, original);
        }
        drop(store);
        let store = Store::open(&root.join("store"), &root).unwrap();
        let repeated = facts_save(&store, &mut source, &mut backend, &clock, &input).unwrap();
        assert_facts(repeated.get("fields"), &results, false);
        assert!(repeated.get("reasonCode").is("save_confirmation_pending"));
        assert_eq!(backend.calls, 2);
        assert_facts(
            facts_reconcile(&store, &mut source, &clock, &input).get("fields"),
            &results,
            false,
        );
        let status = facts_status(&store, &input);
        assert_facts(status.get("fields"), &results, true);
    }
}
#[test]
fn missing_legacy_completion_facts_are_not_invented() {
    use edu_academy_core::workflow::state;
    let root = root("facts-absent");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let mut source = Source::observed();
    let input = facts_input(&store, &mut source, &clock, 2);
    let results = vec![
        facts_result("progress", "not_persisted", Some(Js::Null)),
        facts_result("homework", "not_persisted", None),
    ];
    let mut backend = FactsBackend {
        results: results.clone(),
        calls: 0,
    };
    let saved = facts_save(&store, &mut source, &mut backend, &clock, &input).unwrap();
    assert_facts(saved.get("fields"), &results, false);
    assert_facts(
        facts_reconcile(&store, &mut source, &clock, &input).get("fields"),
        &results,
        false,
    );
    // Outcomes have no global version change: the absent member is the legacy
    // shape. Reopen preserves it, while explicit null stays present.
    drop(store);
    let store = Store::open(&root.join("store"), &root).unwrap();
    assert_facts(facts_status(&store, &input).get("fields"), &results, true);
    for e in state::student_executions(&store, input.get("context").get("scope")).unwrap() {
        let outcome = store.get(Kind::Outcome, &e.key).unwrap();
        assert_eq!(
            outcome.has("completionFacts"),
            e.record.get("field").is("progress")
        );
    }
}
#[test]
fn invalid_completion_facts_keep_execution_fenced() {
    use edu_academy_core::workflow::state;
    for bad in [
        "invalid".into(),
        Js::Arr(vec![]),
        obj(&[("lost", Js::Undefined)]),
        obj(&[("number", Js::Num(f64::NAN))]),
        obj(&[("number", Js::BigInt)]),
    ] {
        let root = root("facts-invalid");
        let store = Store::open(&root.join("store"), &root).unwrap();
        let clock = Clock::new(NOW);
        let mut source = Source::observed();
        let input = facts_input(&store, &mut source, &clock, 3);
        // Bad FIRST of three returned results: validate all before any Outcome,
        // retain later siblings' facts, and give all returned fields an outcome.
        let results = vec![
            facts_result("progress", "persisted", Some(bad)),
            facts_result(
                "homework",
                "persisted",
                Some(obj(&[("original", true.into())])),
            ),
            facts_result(
                "previousHomeworkAssessment",
                "not_persisted",
                Some(Js::Null),
            ),
        ];
        let mut backend = FactsBackend {
            results: results.clone(),
            calls: 0,
        };
        assert_eq!(
            facts_save(&store, &mut source, &mut backend, &clock, &input)
                .unwrap_err()
                .message,
            "workflow_completion_facts_invalid"
        );
        assert_eq!(backend.calls, 3);
        for e in state::student_executions(&store, input.get("context").get("scope")).unwrap() {
            assert!(e.state.needs_reconciliation());
            let outcome = store.get(Kind::Outcome, &e.key).unwrap();
            assert!(outcome.get("status").is("unresolved"));
            if e.record.get("field").is("progress") {
                assert!(!outcome.has("completionFacts"));
            } else {
                let r = results
                    .iter()
                    .find(|r| r.get("field") == e.record.get("field"))
                    .unwrap();
                assert_eq!(outcome.get("completionFacts"), r.get("completionFacts"));
            }
        }
        let again = facts_save(&store, &mut source, &mut backend, &clock, &input).unwrap();
        assert!(again.get("reasonCode").is("save_confirmation_pending"));
        assert_eq!(backend.calls, 3);
    }
}
#[test]
fn completion_facts_limit() {
    use edu_academy_core::workflow::{schema::MAX_COMPLETION_FACTS_BYTES, state};
    let root = root("facts-limit");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let mut source = Source::observed();
    let input = facts_input(&store, &mut source, &clock, 1);
    let empty = obj(&[("p", "".into())]);
    let mut facts = obj(&[(
        "p",
        "x".repeat(MAX_COMPLETION_FACTS_BYTES - codec::bytes(&empty).unwrap().len())
            .into(),
    )]);
    assert_eq!(
        codec::bytes(&facts).unwrap().len(),
        MAX_COMPLETION_FACTS_BYTES
    );
    let mut backend = FactsBackend {
        results: vec![facts_result("progress", "persisted", Some(facts.clone()))],
        calls: 0,
    };
    let saved = facts_save(&store, &mut source, &mut backend, &clock, &input).unwrap();
    assert_eq!(
        saved.get("fields").array()[0].get("completionFacts"),
        &facts
    );
    let next_store = Store::open(&root.join("next"), &root).unwrap();
    let input = facts_input(&next_store, &mut source, &clock, 1);
    facts.set("p", format!("{}x", facts.get("p").code_units()).into());
    backend.results = vec![facts_result("progress", "persisted", Some(facts))];
    assert_eq!(
        facts_save(&next_store, &mut source, &mut backend, &clock, &input)
            .unwrap_err()
            .message,
        "workflow_completion_facts_too_large"
    );
    assert!(
        state::student_executions(&next_store, input.get("context").get("scope")).unwrap()[0]
            .state
            .needs_reconciliation()
    );
}

// S1/S2 use real Service admission/fences with explicitly unqualified scripted
// answers. Before Q, readback and teacher words cannot settle the attempt.
fn unqualified_settlement_sequence(transport: &str, desired: bool, assertion: bool) {
    use edu_academy_core::workflow::state;
    let root = root("settlement-held");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let mut source = Source::observed();
    let input = facts_input(&store, &mut source, &clock, 1);
    let mut result = facts_result("progress", "unresolved", None);
    result.set("transport", transport.into());
    let mut backend = FactsBackend {
        results: vec![result],
        calls: 0,
    };
    let saved = facts_save(&store, &mut source, &mut backend, &clock, &input).unwrap();
    assert!(saved.get("status").is("held"));
    assert!(
        saved.get("fields").array()[0]
            .get("status")
            .is("unresolved")
    );
    assert_eq!(backend.calls, 1);
    drop(store);
    let store = Store::open(&root.join("store"), &root).unwrap();
    let scope = input.get("context").get("scope");
    let original = state::student_executions(&store, scope).unwrap().remove(0);
    let outcome = store.get(Kind::Outcome, &original.key).unwrap();
    if desired {
        let mut observation = source.result.get("observation").clone();
        let mut fields = observation.get("fields").clone();
        let mut progress = fields.get("progress").clone();
        progress.set("value", "- Updated progress".into());
        fields.set("progress", progress);
        observation.set("fields", fields);
        source.result.set("observation", observation);
    }
    let mut reconcile = obj(&[("context", input.get("context").clone())]);
    if assertion {
        reconcile.set(
            "resolution",
            js(json!({
                "request":{"requestRef":"facts","instruction":{"text":"It failed. Save again."}},
                "executions":[{"requestRef":"facts","field":"progress"}]
            })),
        );
    }
    let held = service::Service {
        store: &store,
        source: &mut source,
        backend: &mut backend,
        policy: &Policy::new(true),
        runtime: &clock,
    }
    .reconcile(&reconcile)
    .unwrap();
    assert!(held.get("status").is("held"));
    assert!(
        held.get("fields").array()[0]
            .get("nextStep")
            .is("ask_person")
    );
    assert!(
        store
            .get(Kind::Reconciliation, &original.key)
            .unwrap()
            .absent()
    );
    assert_eq!(store.get(Kind::Outcome, &original.key).unwrap(), outcome);
    let next = bound_input(
        &store,
        &mut source,
        &clock,
        "new-approved-intent",
        &[("progress", "- Next progress")],
    )
    .unwrap();
    let blocked = facts_save(&store, &mut source, &mut backend, &clock, &next).unwrap();
    assert!(blocked.get("reasonCode").is("save_confirmation_pending"));
    // A second reopen proves both durable consumption and zero second dispatch.
    let reopened = Store::open(&root.join("store"), &root).unwrap();
    let executions = state::student_executions(&reopened, scope).unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].key, original.key);
    assert!(executions[0].state.needs_reconciliation());
    assert_eq!(backend.calls, 1);
}
#[test]
fn s1_loss_and_timeout_equality_remain_held_after_restart() {
    for transport in ["lost", "returned_timeout"] {
        unqualified_settlement_sequence(transport, true, false);
    }
}
#[test]
fn s2_returned_ack_and_teacher_assertion_cannot_release_attempt() {
    for transport in ["returned", "lost", "returned_timeout"] {
        unqualified_settlement_sequence(transport, false, true);
    }
}

#[test]
fn outcome_without_execution_fence_never_authorizes_dispatch() {
    use edu_academy_core::workflow::state;
    for indexed in [false, true] {
        let root = root("missing-fence");
        let store = Store::open(&root.join("store"), &root).unwrap();
        let clock = Clock::new(NOW);
        let mut source = Source::observed();
        let input = facts_input(&store, &mut source, &clock, 1);
        let review = store
            .get(Kind::Review, &input.get("reviewDigest").code_units())
            .unwrap();
        let scope = input.get("context").get("scope");
        let patch = &review.get("patch").array()[0];
        let key = state::checked_execution_key(&store, &"facts".into(), scope, patch).unwrap();
        store
            .publish(
                Kind::Outcome,
                &key,
                &obj(&[("status", "not_persisted".into())]),
                &clock,
            )
            .unwrap();
        if indexed {
            store
                .add_ledger(&state::lock_key(scope).unwrap(), &key, &clock)
                .unwrap();
        }
        let reopened = Store::open(&root.join("store"), &root).unwrap();
        assert!(
            state::field_state(&reopened, &key)
                .unwrap()
                .needs_reconciliation()
        );
        let mut backend = FactsBackend {
            results: vec![],
            calls: 0,
        };
        let result = facts_save(&reopened, &mut source, &mut backend, &clock, &input);
        if indexed {
            assert_eq!(
                result.unwrap_err().message,
                "workflow_execution_missing_with_evidence"
            );
        } else {
            assert!(result.unwrap().get("status").is("held"));
        }
        assert_eq!(backend.calls, 0);
        assert!(reopened.get(Kind::Execution, &key).unwrap().absent());
    }
}

#[test]
fn empty_backend_result_keeps_fence_held_with_question() {
    use edu_academy_core::workflow::state;
    let root = root("empty-result");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let mut source = Source::observed();
    let input = facts_input(&store, &mut source, &clock, 1);
    let mut backend = FactsBackend {
        results: vec![],
        calls: 0,
    };
    let held = facts_save(&store, &mut source, &mut backend, &clock, &input).unwrap();
    assert!(held.get("status").is("held"));
    assert!(held.get("nextStep").is("ask_person"));
    assert!(held.get("question").string());
    let reopened = Store::open(&root.join("store"), &root).unwrap();
    assert!(
        state::student_executions(&reopened, input.get("context").get("scope")).unwrap()[0]
            .state
            .needs_reconciliation()
    );
    assert_eq!(backend.calls, 1);
}

// S3 synthetic-only shape: the key shifts independently of target.recordId.
// Product-shaped target/key drift is covered separately below.
fn occurrence_alias_sequence(field: &str, next_date: bool, missing_review: bool) {
    use edu_academy_core::workflow::state;
    let root = root("occurrence-alias");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let mut source = Source::observed();
    let first = bound_input(
        &store,
        &mut source,
        &clock,
        "alias-first",
        &[(
            field,
            if field == "progress" {
                "- First update"
            } else {
                "3"
            },
        )],
    )
    .unwrap();
    let mut backend = FactsBackend {
        results: vec![facts_result(field, "unresolved", None)],
        calls: 0,
    };
    assert!(
        facts_save(&store, &mut source, &mut backend, &clock, &first)
            .unwrap()
            .get("status")
            .is("held")
    );
    drop(store);
    let store = Store::open(&root.join("store"), &root).unwrap();
    let mut observation = source.result.get("observation").clone();
    if next_date {
        let mut scope = observation.get("scope").clone();
        scope.set("lessonDate", "2040-01-05".into());
        observation.set("scope", scope);
        let mut current = observation.get("target").clone();
        current.set("lessonDate", "2040-01-05".into());
        current.set("recordId", "invented-next-current".into());
        observation.set("target", current);
        // previousTarget remains the same actual previous occurrence.
    }
    source.result.set("observation", observation);
    let effect = source
        .result
        .get("observation")
        .get("fields")
        .get(field)
        .get("effectTarget")
        .clone();
    bind_field(&mut source, field, effect, "invented-alias-second");
    let second = bound_input(
        &store,
        &mut source,
        &clock,
        "alias-second",
        &[(
            field,
            if field == "progress" {
                "- Second update"
            } else {
                "2"
            },
        )],
    )
    .unwrap();
    if missing_review {
        let digest = first.get("reviewDigest").code_units();
        fs::remove_file(
            store
                .root()
                .join("review")
                .join(format!("{}.json", sha(&format!("review\n{digest}")))),
        )
        .unwrap();
    }
    let result = facts_save(&store, &mut source, &mut backend, &clock, &second);
    if missing_review {
        assert_eq!(
            result.unwrap_err().message,
            "workflow_execution_review_target_unavailable"
        );
    } else {
        let held = result.unwrap();
        assert!(held.get("status").is("held"));
        assert!(held.get("reasonCode").is("save_confirmation_pending"));
    }
    let reopened = Store::open(&root.join("store"), &root).unwrap();
    let executions =
        state::student_executions(&reopened, second.get("context").get("scope")).unwrap();
    assert_eq!(executions.len(), 1);
    assert!(executions[0].record.get("requestRef").is("alias-first"));
    assert!(executions[0].state.needs_reconciliation());
    assert_eq!(backend.calls, 1);
}
#[test]
fn s3_record_key_alias_is_held_after_restart() {
    occurrence_alias_sequence("progress", false, false);
}
#[test]
fn s3_previous_occurrence_alias_across_current_dates_is_held() {
    occurrence_alias_sequence("previousHomeworkAssessment", true, false);
}
#[test]
fn s3_missing_immutable_review_refuses_before_second_dispatch() {
    occurrence_alias_sequence("progress", false, true);
}

// Mirror product sources: key = target.recordId; effectTarget = canonical(target).
fn product_occurrence_source(source: &mut Source, change: &str) {
    let mut observation = source.result.get("observation").clone();
    for name in ["target", "previousTarget"] {
        let mut target = observation.get(name).clone();
        target.set("recordId", format!("invented-{name}-{change}").into());
        if change == "membership" {
            target.set("membershipId", "invented-other-membership".into());
        }
        if change == "course" {
            target.set("courseId", "invented-other-course".into());
        }
        if change == "date" || (change == "previous" && name == "target") {
            target.set(
                "lessonDate",
                if name == "target" {
                    "2040-01-05"
                } else {
                    "2040-01-04"
                }
                .into(),
            );
        }
        observation.set(name, target);
    }
    let mut scope = observation.get("scope").clone();
    scope.set(
        "lessonDate",
        observation.get("target").get("lessonDate").clone(),
    );
    observation.set("scope", scope);
    let mut fields = observation.get("fields").clone();
    for field in review::FIELDS {
        let target = observation.get(if field == "previousHomeworkAssessment" {
            "previousTarget"
        } else {
            "target"
        });
        let mut value = fields.get(field).clone();
        value.set("effectRecordKey", target.get("recordId").clone());
        value.set(
            "effectTarget",
            edu_academy_core::workflow::canonical(target)
                .unwrap()
                .into(),
        );
        fields.set(field, value);
    }
    observation.set("fields", fields);
    source.result.set("observation", observation);
}
fn product_occurrence_sequence(field: &str, change: &str, blocked: bool) {
    use edu_academy_core::workflow::state;
    let root = root("product-occurrence");
    let store = Store::open(&root.join("store"), &root).unwrap();
    let clock = Clock::new(NOW);
    let mut source = Source::observed();
    product_occurrence_source(&mut source, "first");
    let values = &[(
        field,
        if field == "progress" {
            "- Product-shaped update"
        } else {
            "3"
        },
    )];
    let first = bound_input(&store, &mut source, &clock, "product-first", values).unwrap();
    let mut backend = FactsBackend {
        results: vec![facts_result(field, "unresolved", None)],
        calls: 0,
    };
    facts_save(&store, &mut source, &mut backend, &clock, &first).unwrap();
    drop(store);
    let store = Store::open(&root.join("store"), &root).unwrap();
    product_occurrence_source(&mut source, change);
    let second = bound_input(&store, &mut source, &clock, "product-second", values).unwrap();
    let result = facts_save(&store, &mut source, &mut backend, &clock, &second);
    if blocked {
        // Existing Service drift refusal holds the alias before another fence.
        assert_eq!(result.unwrap_err().message, "workflow_effect_target_drift");
    } else {
        assert!(result.unwrap().get("status").is("held")); // dispatched, unqualified outcome
    }
    let reopened = Store::open(&root.join("store"), &root).unwrap();
    let executions =
        state::student_executions(&reopened, second.get("context").get("scope")).unwrap();
    let expected = if blocked { 1 } else { 2 };
    assert_eq!(executions.len(), expected);
    assert_eq!(backend.calls, expected);
    assert!(executions.iter().all(|e| e.state.needs_reconciliation()));
    assert_eq!(
        executions
            .iter()
            .any(|e| e.record.get("requestRef").is("product-second")),
        !blocked
    );
    for e in executions {
        let target = codec::parse(e.record.get("effectTarget").code_units().as_bytes()).unwrap();
        assert_eq!(e.record.get("effectRecordKey"), target.get("recordId"));
    }
}
#[test]
fn s3_product_record_id_shift_is_held_after_restart() {
    product_occurrence_sequence("progress", "record", true);
}
#[test]
fn s3_product_membership_shift_is_held_after_restart() {
    product_occurrence_sequence("progress", "membership", true);
}
#[test]
fn s3_product_different_course_is_admitted_after_restart() {
    product_occurrence_sequence("progress", "course", false);
}
#[test]
fn s3_product_different_date_is_admitted_after_restart() {
    product_occurrence_sequence("progress", "date", false);
}
#[test]
fn s3_product_previous_occurrence_record_id_shift_is_held() {
    product_occurrence_sequence("previousHomeworkAssessment", "previous", true);
}
