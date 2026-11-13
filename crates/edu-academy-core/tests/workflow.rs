//! Changed-boundary tests operate only in a caller-supplied private evidence root.
use edu_academy_core::{
    js::{Js, obj, stringify},
    workflow::{
        Runtime,
        cism::CismStore,
        codec,
        state::{self, FieldState},
        store::{Kind, LockOptions, Store, mkdir},
    },
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        "2040-01-04T12:00:00.000Z".into()
    }
    fn epoch_ms(&self) -> u64 {
        2209291200000
    }
    fn uuid(&self) -> edu_academy_core::js::Result<String> {
        Ok(format!(
            "00000000-0000-4000-8000-{:012}",
            NEXT.fetch_add(1, Ordering::SeqCst)
        ))
    }
}
fn root(name: &str) -> PathBuf {
    let p = PathBuf::from(
        std::env::var("S10_TEST_ROOT").expect("explicit private evidence root required"),
    )
    .join(format!(
        "{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    mkdir(&p).unwrap();
    p
}
#[test]
fn publication_is_immutable_and_identity_checked_after_readback() {
    let p = root("publication");
    let s = Store::open(&p.join("store"), &p).unwrap();
    let first = obj(&[("b", "한글".into()), ("a", "second".into())]);
    assert!(
        s.publish(Kind::Observation, "old-handle", &first, &Clock)
            .unwrap()
            .0
    );
    let path = fs::read_dir(s.root().join("observation"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let before = fs::read(&path).unwrap();
    let inode = fs::metadata(&path).unwrap().ino();
    let (created, read) = s
        .publish(
            Kind::Observation,
            "old-handle",
            &obj(&[("changed", true.into())]),
            &Clock,
        )
        .unwrap();
    assert!(!created);
    assert_eq!(read, first);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    fs::write(
        &path,
        b"{\"kind\":\"review\",\"id\":\"old-handle\",\"record\":null}",
    )
    .unwrap();
    assert_eq!(
        s.get(Kind::Observation, "old-handle").unwrap_err().message,
        "workflow_store_record_identity_conflict"
    );
}
#[test]
fn index_only_is_available_and_physical_fence_ignores_request_and_date() {
    let p = root("fence");
    let s = Store::open(&p.join("store"), &p).unwrap();
    let scope = obj(&[
        ("sourceNamespace", "namespace".into()),
        ("teacherId", "teacher".into()),
        ("studentId", "student".into()),
        ("lessonDate", "2040-01-04".into()),
    ]);
    s.add_ledger(&state::lock_key(&scope).unwrap(), "index-only", &Clock)
        .unwrap();
    assert!(state::student_executions(&s, &scope).unwrap().is_empty());
    assert_eq!(
        state::field_state(&s, "index-only").unwrap(),
        FieldState::Available
    );
    let mut target = scope.clone();
    target.set("courseId", "invented-course".into());
    let r = obj(&[
        ("scope", scope.clone()),
        ("field", "progress".into()),
        ("effectRecordKey", "physical".into()),
        ("effectTarget", "synthetic-target".into()),
        ("reviewDigest", "physical-review".into()),
    ]);
    s.publish(
        Kind::Review,
        "physical-review",
        &obj(&[(
            "patch",
            Js::Arr(vec![obj(&[
                ("field", "progress".into()),
                ("target", target.clone()),
            ])]),
        )]),
        &Clock,
    )
    .unwrap();
    s.publish(Kind::Execution, "index-only", &r, &Clock)
        .unwrap();
    let mut other = scope.clone();
    other.set("lessonDate", "2040-01-05".into());
    assert_eq!(
        state::unresolved_on_field(&s, &other, "progress", "physical", &other)
            .unwrap()
            .len(),
        1
    );
    assert!(
        state::unresolved_on_field(&s, &other, "homework", "physical", &other)
            .unwrap()
            .is_empty()
    );
    assert!(
        state::unresolved_on_field(&s, &other, "progress", "other", &other)
            .unwrap()
            .is_empty()
    );
    s.publish(
        Kind::Outcome,
        "index-only",
        &obj(&[("status", "persisted".into())]),
        &Clock,
    )
    .unwrap();
    assert_eq!(
        state::field_state(&s, "index-only").unwrap(),
        FieldState::ReconciliationRequired
    );
    s.publish(
        Kind::Reconciliation,
        "index-only",
        &obj(&[("classification", "conflict".into())]),
        &Clock,
    )
    .unwrap();
    assert_eq!(
        state::field_state(&s, "index-only").unwrap(),
        FieldState::ReconciliationRequired
    );
}
#[test]
fn cancelled_waiter_does_not_replace_or_release_holder_object() {
    let p = root("lock");
    let s = Store::open(&p.join("store"), &p).unwrap();
    let opts = LockOptions {
        wait: Duration::from_millis(15),
        poll: Duration::from_millis(1),
    };
    let held = s.lock("same", opts, &|| false).unwrap();
    let before = held.file().metadata().unwrap();
    assert!(s.lock("same", opts, &|| true).is_err());
    assert!(s.lock("same", opts, &|| false).is_err());
    drop(held);
    let new = s.lock("same", opts, &|| false).unwrap();
    let after = new.file().metadata().unwrap();
    assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
}
#[test]
fn cism_rejects_linked_public_symlink_and_oversized_receipts() {
    let p = root("cism-guards");
    let c = CismStore::open(&p).unwrap();
    assert!(
        c.exclusive("original.json", &obj(&[("fieldRef", "f".into())]))
            .unwrap()
    );
    fs::hard_link(p.join("original.json"), p.join("linked.json")).unwrap();
    assert!(c.read("original.json").is_err());
    assert!(c.read("linked.json").is_err());
    c.exclusive("public.json", &Js::Null).unwrap();
    fs::set_permissions(p.join("public.json"), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(c.read("public.json").is_err());
    symlink(p.join("public.json"), p.join("symlink.json")).unwrap();
    assert!(c.read("symlink.json").is_err());
    c.exclusive("large.json", &Js::Null).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(p.join("large.json"))
        .unwrap()
        .set_len(1_000_001)
        .unwrap();
    assert!(c.read("large.json").is_err());
}
#[test]
fn old_json_preserves_utf16_key_order_duplicate_keys_and_numeric_values() {
    let v = codec::parse(br#"{"z":"\ud800","a":"\udc00","z":"\ud801","2":-0,"1":1e400}"#).unwrap();
    assert_eq!(v.get("z").units(), &[0xd801]);
    assert_eq!(v.get("a").units(), &[0xdc00]);
    assert_eq!(
        stringify(&v).unwrap(),
        "{\"1\":null,\"2\":0,\"z\":\"\\ud801\",\"a\":\"\\udc00\"}"
    );
    for invalid in ["[1,]", "{\"a\":1,}", "01", "[true false]", "\"\\uXYZW\""] {
        assert!(codec::parse(invalid.as_bytes()).is_err());
    }
    let deep = "[".repeat(2000) + "null" + &"]".repeat(2000);
    assert_eq!(
        stringify(&codec::parse(deep.as_bytes()).unwrap()).unwrap(),
        deep
    );
}

#[test]
fn new_target_keys_preserve_exact_unicode_identity() {
    let cases = codec::parse(include_bytes!("fixtures/workflow-unicode-keys.json")).unwrap();
    let bound = codec::parse(include_bytes!("fixtures/workflow-target-keys-v2.json")).unwrap();
    for (index, case) in cases.array().iter().enumerate() {
        let expected = &bound.array()[index];
        let legacy = codec::parse(case.get("executionKey").code_units().as_bytes()).unwrap();
        assert!(legacy.get("effectTarget").absent());
        let p = root(&format!("unicode-key-{index}"));
        let s = Store::open(&p.join("store"), &p).unwrap();
        let scope = case.get("scope");
        let key = state::execution_key(
            case.get("requestRef").clone(),
            scope,
            "progress",
            case.get("effectRecordKey").clone(),
            case.get("before"),
            case.get("after"),
            &"synthetic-target".into(),
        )
        .unwrap();
        assert!(expected.get("executionKey").is(&key));
        assert_ne!(case.get("executionKey"), expected.get("executionKey"));
        assert!(
            case.get("requestKey")
                .is(&state::request_key(scope, case.get("requestRef").clone()).unwrap())
        );
        let mut target = scope.clone();
        target.set("courseId", "invented-course".into());
        let record = obj(&[
            ("field", "progress".into()),
            ("effectTarget", expected.get("effectTarget").clone()),
            ("effectRecordKey", case.get("effectRecordKey").clone()),
            ("reviewDigest", "unicode-review".into()),
        ]);
        s.publish(
            Kind::Review,
            "unicode-review",
            &obj(&[(
                "patch",
                Js::Arr(vec![obj(&[
                    ("field", "progress".into()),
                    ("target", target.clone()),
                ])]),
            )]),
            &Clock,
        )
        .unwrap();
        s.add_ledger(&state::lock_key(scope).unwrap(), &key, &Clock)
            .unwrap();
        s.publish(Kind::Execution, &key, &record, &Clock).unwrap();
        let reopened = Store::open(&p.join("store"), &p).unwrap();
        assert_eq!(
            reopened.ledger(&state::lock_key(scope).unwrap()).unwrap(),
            vec![key.clone()]
        );
        assert!(
            s.root()
                .join("execution")
                .join(format!(
                    "{}.json",
                    String::from_utf16(expected.get("executionFileHash").units()).unwrap()
                ))
                .exists()
        );
        assert_eq!(
            state::unresolved_on_field(
                &reopened,
                scope,
                "progress",
                case.get("effectRecordKey").clone(),
                scope,
            )
            .unwrap()
            .len(),
            1
        );
    }
}

#[test]
fn malformed_ledger_key_type_and_unpaired_utf16_fail_closed() {
    for (label, value) in [("number", "42"), ("surrogate", "\"\\ud800\"")] {
        let p = root(label);
        let s = Store::open(&p.join("store"), &p).unwrap();
        s.add_ledger("group", "original-key", &Clock).unwrap();
        let ledger = fs::read_dir(
            s.root()
                .join("ledger")
                .join(edu_academy_core::js::sha("group")),
        )
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
        fs::write(ledger, format!("{{\"group\":\"group\",\"key\":{value}}}")).unwrap();
        assert_eq!(
            s.ledger("group").unwrap_err().message,
            "workflow_store_ledger_key_invalid"
        );
    }
}

#[test]
fn legacy_lossy_settled_key_cannot_be_replayed_as_a_new_exact_key() {
    use edu_academy_core::{
        js::sha,
        workflow::approval::{Approval, RequestApproval},
    };
    let p = root("legacy-key");
    let store = Store::open(&p.join("store"), &p).unwrap();
    let scope = obj(&[
        ("sourceNamespace", "namespace".into()),
        ("teacherId", "teacher".into()),
        ("studentId", "student".into()),
        ("lessonDate", "2040-01-04".into()),
    ]);
    let request: Js = "req-😀".into();
    let before: Js = "before".into();
    let after: Js = "after".into();
    let legacy = state::execution_key(
        request.code_units().as_str(),
        &scope,
        "progress",
        "physical",
        &before,
        &after,
        &"synthetic-target".into(),
    )
    .unwrap();
    let exact = state::execution_key(
        request.clone(),
        &scope,
        "progress",
        "physical",
        &before,
        &after,
        &"synthetic-target".into(),
    )
    .unwrap();
    assert_ne!(legacy, exact);
    let record = obj(&[
        ("requestRef", request.clone()),
        ("scope", scope.clone()),
        ("field", "progress".into()),
        ("effectRecordKey", "physical".into()),
        ("effectTarget", "synthetic-target".into()),
        ("beforeSha256", sha("before").into()),
        ("afterSha256", sha("after").into()),
    ]);
    store
        .add_ledger(&state::lock_key(&scope).unwrap(), &legacy, &Clock)
        .unwrap();
    store
        .publish(Kind::Execution, &legacy, &record, &Clock)
        .unwrap();
    store
        .publish(
            Kind::Outcome,
            &legacy,
            &obj(&[("status", "persisted".into())]),
            &Clock,
        )
        .unwrap();
    let reviewed = obj(&[
        ("field", "progress".into()),
        ("effectRecordKey", "physical".into()),
        ("effectTarget", "synthetic-target".into()),
        ("before", before.clone()),
        ("after", after.clone()),
    ]);
    let mut approval = RequestApproval {
        store: &store,
        runtime: &Clock,
        input: obj(&[
            ("requestRef", request),
            ("scope", scope),
            ("reviewed", Js::Arr(vec![reviewed])),
        ]),
    };
    let action = obj(&[
        (
            "target",
            obj(&[
                ("studentKey", "student".into()),
                ("teacherKey", "teacher".into()),
                ("date", "2040-01-04".into()),
            ]),
        ),
        ("field", "progress".into()),
        ("effectRecordKey", "physical".into()),
        ("effectTarget", "synthetic-target".into()),
        ("before", before),
        ("after", after),
    ]);
    assert_eq!(
        approval.consume(&action).unwrap_err().message,
        "workflow_execution_key_encoding_conflict"
    );
    assert_eq!(
        store
            .ledger(&state::lock_key(approval.input.get("scope")).unwrap())
            .unwrap(),
        vec![legacy]
    );
    assert!(store.get(Kind::Execution, &exact).unwrap().absent());
}

#[test]
fn large_korean_json_parses_with_a_linear_utf8_scan() {
    let text = "가나다라마바사아".repeat(10_000);
    assert_eq!(text.len(), 240_000);
    let payload = format!(r#"{{"text":"{text}"}}"#);
    let started = std::time::Instant::now();
    let parsed = codec::parse(payload.as_bytes()).unwrap();
    let elapsed = started.elapsed();
    assert!(parsed.get("text").is(&text));
    eprintln!("korean_json_bytes={} elapsed={elapsed:?}", payload.len());
    assert!(
        elapsed < Duration::from_millis(500),
        "UTF-8 rescanning: {elapsed:?}"
    );
}

#[test]
fn legacy_settlement_labels_remain_held_after_reopen() {
    for label in ["persisted", "noop", "not_persisted", "unresolved"] {
        for basis in [
            "readback_new_value",
            "readback_after_returned_transport",
            "user_instruction",
        ] {
            let p = root("legacy-settlement");
            let store = Store::open(&p.join("store"), &p).unwrap();
            let execution = obj(&[("field", "progress".into())]);
            let outcome = obj(&[("status", label.into())]);
            let reconciliation = obj(&[
                ("classification", "not_persisted".into()),
                ("basis", basis.into()),
            ]);
            for (kind, value) in [
                (Kind::Execution, &execution),
                (Kind::Outcome, &outcome),
                (Kind::Reconciliation, &reconciliation),
            ] {
                store.publish(kind, "old-attempt", value, &Clock).unwrap();
            }
            drop(store);
            let reopened = Store::open(&p.join("store"), &p).unwrap();
            assert!(
                state::field_state(&reopened, "old-attempt")
                    .unwrap()
                    .needs_reconciliation()
            );
            assert_eq!(reopened.get(Kind::Outcome, "old-attempt").unwrap(), outcome);
            assert_eq!(
                reopened.get(Kind::Reconciliation, "old-attempt").unwrap(),
                reconciliation
            );
        }
    }
}

#[test]
fn execution_without_review_metadata_refuses_even_a_different_record_key() {
    let p = root("missing-review-metadata");
    let store = Store::open(&p.join("store"), &p).unwrap();
    let scope = obj(&[
        ("sourceNamespace", "invented".into()),
        ("teacherId", "teacher".into()),
        ("studentId", "student".into()),
    ]);
    store
        .add_ledger(&state::lock_key(&scope).unwrap(), "unbound-review", &Clock)
        .unwrap();
    store
        .publish(
            Kind::Execution,
            "unbound-review",
            &obj(&[
                ("field", "progress".into()),
                ("effectRecordKey", "first".into()),
            ]),
            &Clock,
        )
        .unwrap();
    let reopened = Store::open(&p.join("store"), &p).unwrap();
    for key in ["first", "second"] {
        assert_eq!(
            state::unresolved_on_field(&reopened, &scope, "progress", key, &scope)
                .err()
                .unwrap()
                .message,
            "workflow_execution_review_target_unavailable"
        );
    }
    assert_eq!(
        state::student_executions(&reopened, &scope).unwrap().len(),
        1
    );
}

#[test]
fn incomplete_stored_occurrence_target_refuses_physical_match() {
    for missing in ["sourceNamespace", "studentId", "lessonDate", "courseId"] {
        let p = root("incomplete-occurrence");
        let store = Store::open(&p.join("store"), &p).unwrap();
        let scope = obj(&[
            ("sourceNamespace", "invented".into()),
            ("teacherId", "teacher".into()),
            ("studentId", "student".into()),
            ("lessonDate", "2040-01-04".into()),
        ]);
        let mut target = scope.clone();
        target.set("courseId", "invented-course".into());
        let candidate = target.clone();
        target.set(missing, Js::Undefined);
        store
            .publish(
                Kind::Review,
                "incomplete-review",
                &obj(&[(
                    "patch",
                    Js::Arr(vec![obj(&[
                        ("field", "progress".into()),
                        ("target", target),
                    ])]),
                )]),
                &Clock,
            )
            .unwrap();
        store
            .add_ledger(&state::lock_key(&scope).unwrap(), "attempt", &Clock)
            .unwrap();
        store
            .publish(
                Kind::Execution,
                "attempt",
                &obj(&[
                    ("field", "progress".into()),
                    ("effectRecordKey", "physical".into()),
                    ("reviewDigest", "incomplete-review".into()),
                ]),
                &Clock,
            )
            .unwrap();
        let reopened = Store::open(&p.join("store"), &p).unwrap();
        assert_eq!(
            state::unresolved_on_field(&reopened, &scope, "progress", "physical", &candidate)
                .err()
                .unwrap()
                .message,
            "workflow_execution_review_target_unavailable"
        );
    }
}
