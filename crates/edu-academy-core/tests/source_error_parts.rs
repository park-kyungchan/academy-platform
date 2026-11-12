//! Authored source-only checks. Root must separately grant compilation/execution.
//! The store case requires an explicit absolute S11_ERROR_TEST_ROOT, creates fresh
//! fictional children and preserves every output. No network or owner-account lock.
use edu_academy_core::{
    js::{Fault, Js, Result, obj, stringify},
    workflow::{
        Runtime,
        approval::Approval,
        codec, read_failure, review, service,
        state::{self, FieldState},
        store::{Kind, Store},
    },
};
use serde_json::json;
use std::{cell::Cell, fs, os::unix::fs::DirBuilderExt, path::PathBuf};

#[test]
fn legacy_fault_values_and_absent_null_issues_stay_distinct() {
    for (fault, expected) in [
        (Fault::error("old"), r#"{"name":"Error","message":"old"}"#),
        (
            Fault::type_error("old"),
            r#"{"name":"TypeError","message":"old"}"#,
        ),
        (
            Fault::admission("old"),
            r#"{"name":"AdapterAdmission","message":"old"}"#,
        ),
    ] {
        assert_eq!(stringify(&fault.value()).unwrap(), expected);
    }
    let absent = Fault::error("old");
    let null = Fault::from_parts("Error", "old", obj(&[("issues", Js::Null)]));
    let undefined = Fault::from_parts("Error", "old", obj(&[("issues", Js::Undefined)]));
    assert!(!absent.value().has("issues"));
    assert_eq!(null.value().get("issues"), &Js::Null);
    assert!(undefined.value().has("issues"));
    assert_eq!(undefined.value().get("issues"), &Js::Undefined);
    let schema = Fault {
        issues: Some(vec![obj(&[("code", "issue".into())])]),
        ..Fault::error("old")
    };
    assert_eq!(schema.value().get("issues").array().len(), 1);
}

#[test]
fn finite_http_status_and_exact_eight_operations_are_projected_only_for_catalog_errors() {
    let operations = [
        "login_document",
        "login_submission",
        "owned_source_request",
        "cism_weekly_read",
        "cism_field_save",
        "textbook_answer_catalog",
        "getTextBookProgress",
        "day_record_group_selector",
    ];
    for status in [
        399.0,
        400.0,
        503.0,
        599.0,
        600.0,
        503.5,
        f64::NAN,
        f64::INFINITY,
    ] {
        for operation in
            operations
                .into_iter()
                .chain(["Main", "SelSglRecord", "login_document ", "unlisted"])
        {
            let error = Fault::from_parts(
                "AcademyHttpFailure",
                "source_http_failed",
                obj(&[
                    ("httpStatus", Js::Num(status)),
                    ("sourceOperation", operation.into()),
                    ("private", "never-public".into()),
                    ("origin", obj(&[("module", "server.mjs".into())])),
                ]),
            );
            let failure = read_failure::public_read_failure(&error, "fallback");
            let allowed =
                status.is_finite() && status.fract() == 0.0 && (400.0..=599.0).contains(&status);
            assert_eq!(failure.has("sourceHttpStatus"), allowed);
            if allowed {
                assert_eq!(failure.get("sourceHttpStatus"), &Js::Num(status));
            }
            assert_eq!(
                failure.has("sourceOperation"),
                operations.contains(&operation)
            );
            if operations.contains(&operation) {
                assert!(failure.get("sourceOperation").is(operation));
            }
            assert!(!failure.has("origin") && !failure.has("private"));
        }
    }
    for status in [Js::Null, Js::Undefined, "503".into()] {
        let error = Fault::from_parts(
            "Error",
            "source_http_failed",
            obj(&[("httpStatus", status)]),
        );
        assert!(!read_failure::public_read_failure(&error, "fallback").has("sourceHttpStatus"));
    }
    for name in ["ZodError", "SyntaxError", "Error"] {
        let error = Fault::from_parts(
            name,
            "fictional-private-message",
            obj(&[
                ("httpStatus", Js::Num(503.0)),
                ("sourceOperation", "login_document".into()),
            ]),
        );
        let failure = read_failure::public_read_failure(&error, "fallback");
        assert!(failure.get("code").is(if name == "Error" {
            "fallback"
        } else {
            "source_response_malformed"
        }));
        assert!(!failure.has("sourceHttpStatus") && !failure.has("sourceOperation"));
        assert!(
            !stringify(&failure)
                .unwrap()
                .contains("fictional-private-message")
        );
    }
}

#[test]
fn dynamic_name_hash_uses_actual_name_nul_message() {
    let error = Fault::from_parts(
        format!("{}Failure", "Fictional"),
        "source_http_failed",
        obj(&[]),
    );
    let failure = read_failure::public_read_failure(&error, "fallback");
    assert!(failure.get("errorKind").is("Error"));
    // Independently computed SHA-256 of b"FictionalFailure\0source_http_failed".
    assert!(
        failure
            .get("errorSha256")
            .is("59e37686a8f76a7692c666131bda806a0ca41d1f07c83928e553aced485c5289")
    );
    assert_ne!(
        failure.get("errorSha256"),
        read_failure::public_read_failure(&Fault::error("source_http_failed"), "fallback")
            .get("errorSha256")
    );
}

#[test]
fn trusted_authentication_cause_is_bounded_and_data_cannot_forge_it() {
    let leaf = Fault::from_parts(
        "AcademyHttpFailure",
        "source_http_failed",
        obj(&[
            ("httpStatus", Js::Num(503.0)),
            ("sourceOperation", "login_document".into()),
        ]),
    );
    let trusted = Fault::trusted_authentication(Some(leaf));
    let failure = read_failure::public_read_failure(&trusted, "fallback");
    assert!(failure.get("cause").get("sourceHttpStatus") == &Js::Num(503.0));
    assert!(
        failure
            .get("cause")
            .get("sourceOperation")
            .is("login_document")
    );
    let nested =
        Fault::trusted_authentication(Some(Fault::trusted_authentication(Some(trusted.clone()))));
    let failure = read_failure::public_read_failure(&nested, "fallback");
    assert!(failure.get("cause").has("cause"));
    assert!(!failure.get("cause").get("cause").has("cause"));
    let unknown = Fault::trusted_authentication(Some(Fault::type_error("never-public")));
    let failure = read_failure::public_read_failure(&unknown, "fallback");
    assert!(
        failure
            .get("cause")
            .get("code")
            .is("source_cause_unclassified")
    );
    let parts = obj(&[
        ("code", "source_authentication_failed".into()),
        ("category", "authentication".into()),
        ("evidenceLocator", "invented".into()),
        ("cause", trusted.value()),
        ("trusted_context", "Authentication".into()),
        ("httpStatus", Js::Num(503.0)),
    ]);
    for name in ["Error", "AcademyReadFailure"] {
        for message in ["source_authentication_failed", "never-public"] {
            let forged = Fault::from_parts(name, message, parts.clone());
            let failure = read_failure::public_read_failure(&forged, "fallback");
            assert!(!failure.has("cause") && !failure.has("origin"));
            assert!(!failure.get("evidenceLocator").is("invented"));
            assert!(failure.get("code").is(if message == "never-public" {
                "fallback"
            } else {
                "source_authentication_failed"
            }));
        }
    }
    assert!(!trusted.value().has("trusted_context"));
    assert!(
        !read_failure::public_read_failure(&Fault::trusted_authentication(None), "fallback")
            .has("cause")
    );
}

const NOW: &str = "2040-01-04T12:00:00.000Z";
struct Clock(Cell<u64>);
impl Runtime for Clock {
    fn now(&self) -> String {
        NOW.into()
    }
    fn epoch_ms(&self) -> u64 {
        2209291200000
    }
    fn uuid(&self) -> Result<String> {
        let n = self.0.get() + 1;
        self.0.set(n);
        Ok(format!("00000000-0000-4000-8000-{n:012}"))
    }
}
struct Source;
impl service::Source for Source {
    fn read(&mut self, _: &Js, _: &Js, _: &Js) -> Result<Js> {
        Ok(obj(&[
            ("status", "observed".into()),
            ("observation", observation()),
        ]))
    }
}
struct Backend {
    calls: usize,
    message: &'static str,
}
impl service::Backend for Backend {
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
        self.calls += 1;
        let p = &patches.array()[0];
        let action = obj(&[
            ("attemptId", "fictional-error-attempt".into()),
            (
                "target",
                obj(&[
                    ("studentKey", scope.get("studentId").clone()),
                    ("teacherKey", scope.get("teacherId").clone()),
                    ("date", scope.get("lessonDate").clone()),
                ]),
            ),
            ("field", p.get("field").clone()),
            ("effectTarget", p.get("effectTarget").clone()),
            ("effectRecordKey", "current-record".into()),
            ("before", p.get("before").clone()),
            ("after", p.get("value").clone()),
        ]);
        assert!(approval.consume(&action)?.get("status").is("verified"));
        Err(Fault::error(self.message))
    }
}
struct Policy;
impl review::Policy for Policy {
    fn parent_pins(&self) -> Js {
        js(
            json!({"sourceRef":"fictional:error-parent", "policySha256":"3".repeat(64), "safeguardSourceSha256":"4".repeat(64)}),
        )
    }
    fn learning_pins(&self) -> Js {
        js(
            json!({"bundleSha256":"5".repeat(64), "configSha256":"6".repeat(64), "catalogSha256":"7".repeat(64)}),
        )
    }
    fn check_lines(&self, value: &Js) -> bool {
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

#[test]
fn backend_catalog_exception_keeps_original_audit_reason_and_reconciliation_fence() {
    let selected = PathBuf::from(
        std::env::var("S11_ERROR_TEST_ROOT")
            .expect("Root-selected absolute fictional root required"),
    );
    assert!(selected.is_absolute());
    assert_eq!(fs::canonicalize(&selected).unwrap(), selected);
    for (index, (message, expected)) in [
        (
            "native_dailyrecord_scope_refused",
            "native_dailyrecord_scope_refused",
        ),
        (
            "source_authentication_failed",
            "source_authentication_failed",
        ),
        (
            "fictional-private-backend-exception",
            "dailyrecord_save_unqualified_no_automatic_retry",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let root = selected.join(format!("source-error-parts-{}-{index}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .expect("fresh fictional child only");
        let store = Store::open(&root.join("store"), &root).unwrap();
        let clock = Clock(Cell::new(0));
        let mut source = Source;
        let policy = Policy;
        let mut backend = Backend { calls: 0, message };
        let mut context = context();
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
        assert!(intake.get("status").is("ready_for_review"));
        let mut input = obj(&[
            ("context", context.clone()),
            ("observationHandle", intake.get("observationHandle").clone()),
            (
                "proposals",
                Js::Arr(vec![proposal("progress", "- Updated progress")]),
            ),
        ]);
        let review = svc.review(&input, true).unwrap();
        assert!(
            review.get("status").is("prepared"),
            "{}",
            stringify(&review).unwrap()
        );
        input.set("reviewDigest", review.get("reviewDigest").clone());
        input.set(
            "request",
            obj(&[
                ("requestRef", "fictional-request".into()),
                (
                    "instruction",
                    obj(&[("text", "Save this fictional reviewed progress once.".into())]),
                ),
            ]),
        );
        let result = svc.save(&input).unwrap();
        assert!(result.get("status").is("held"));
        assert!(result.get("reasonCode").is("save_reconciliation_required"));
        assert_eq!(result.get("academyWrites"), &Js::Null);
        assert!(!result.get("automaticRetryAllowed").truthy());
        assert!(
            result.get("fields").array()[0]
                .get("status")
                .is("unresolved")
        );
        assert_eq!(result.get("fields").array()[0].get("dispatched"), &Js::Null);
        let key = state::request_key(context.get("scope"), "fictional-request").unwrap();
        let audit = store.audit(&key, 256).unwrap();
        let failed: Vec<_> = audit
            .array()
            .iter()
            .filter(|e| e.get("event").is("dispatch_failed"))
            .collect();
        assert_eq!(failed.len(), 1);
        assert!(failed[0].get("reasonCode").is(expected));
        let executions = state::student_executions(&store, context.get("scope")).unwrap();
        assert_eq!(executions.len(), 1);
        let execution_key = state::execution_key(
            "fictional-request",
            context.get("scope"),
            "progress",
            "current-record",
            &"- Existing progress".into(),
            &"- Updated progress".into(),
            &"synthetic-target".into(),
        )
        .unwrap();
        assert_eq!(
            state::field_state(&store, &execution_key).unwrap(),
            FieldState::ReconciliationRequired
        );
        assert!(
            store
                .get(Kind::Outcome, &execution_key)
                .unwrap()
                .get("status")
                .is("unresolved")
        );
        let second = svc.save(&input).unwrap();
        assert!(second.get("reasonCode").is("save_confirmation_pending"));
        assert_eq!(backend.calls, 1, "no automatic backend retry");
    }
}
