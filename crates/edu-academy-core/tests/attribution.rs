//! Synthetic-only contract qualification; no TS, docs, catalogue or runtime logs.
use edu_academy_core::{
    attribution::{self, Approval, Envelope, Instruction, Rejection, Verdict},
    js::{Fault, Js, Result, obj, strings, text},
    templates::from_json,
    workflow::{
        approval::{
            AttributionOwner, AttributionStore, AttributionTransaction, ConsumeOwner,
            DurableConsume,
        },
        codec, schema,
    },
};
use std::collections::HashSet;

fn action() -> Js {
    from_json(serde_json::json!({
        "after": "Synthetic value", "field": "progress", "studentId": "synthetic:student:1"
    }))
}
fn source() -> String {
    "a".repeat(64)
}
fn instruction() -> Instruction {
    Instruction {
        runtime: "codex".into(),
        runtime_session_id: "synthetic:session:1".into(),
        request_ref: "synthetic:request:1".into(),
        current_user_event_ref: "synthetic:event:1".into(),
        text_blocks: vec![" alpha ".into(), "β".into(), "".into()],
        superseded: false,
        source_sha256: Some(source()),
        action_digests: vec![attribution::action_digest(&action(), &source()).unwrap()],
    }
}
fn envelope() -> Js {
    let i = instruction();
    obj(&[
        ("schemaVersion", attribution::SCHEMA_VERSION.into()),
        ("runtime", i.runtime.into()),
        ("runtimeSessionId", i.runtime_session_id.into()),
        ("requestRef", i.request_ref.into()),
        ("currentUserEventRef", i.current_user_event_ref.into()),
        (
            "requestTextSha256",
            attribution::normalized_request_text_hash(&i.text_blocks).into(),
        ),
        ("issuedAt", "1970-01-01T00:00:00Z".into()),
        ("expiresAt", "1970-01-01T00:01:00Z".into()),
        (
            "actionDigests",
            Js::Arr(vec![
                attribution::action_digest(&action(), &source())
                    .unwrap()
                    .into(),
            ]),
        ),
    ])
}
#[derive(Clone)]
struct Current {
    value: Option<Instruction>,
    second: Option<Option<Instruction>>,
    reads: usize,
    now: i64,
    fail: bool,
}
impl Default for Current {
    fn default() -> Self {
        Self {
            value: Some(instruction()),
            second: None,
            reads: 0,
            now: 1,
            fail: false,
        }
    }
}
impl Current {
    fn lookup(&mut self, request_ref: &str) -> Result<Option<Instruction>> {
        assert_eq!(request_ref, "synthetic:request:1");
        self.reads += 1;
        if self.fail {
            return Err(Fault::error("synthetic_lookup_failure"));
        }
        if let Some(value) = &self.second {
            return Ok(value.clone());
        }
        Ok(self.value.clone())
    }
    fn now_ms(&self) -> i64 {
        self.now
    }
}
#[derive(Default)]
struct Consume {
    keys: HashSet<String>,
    records: Vec<Js>,
    fail_after_write: bool,
    refuse: bool,
}
impl ConsumeOwner for Consume {
    fn consume_once(&mut self, key: &str, record: &Js) -> Result<bool> {
        self.records.push(record.clone());
        if self.refuse {
            return Ok(false);
        }
        let created = self.keys.insert(key.into());
        if self.fail_after_write {
            Err(Fault::error("synthetic_uncertain_consume"))
        } else {
            Ok(created)
        }
    }
}
struct LocalStore<'a> {
    current: &'a mut Current,
    owner: &'a mut dyn ConsumeOwner,
}
struct LocalTransaction<'a> {
    current: &'a mut Current,
    owner: &'a mut dyn ConsumeOwner,
}
impl AttributionStore for LocalStore<'_> {
    type Transaction<'a>
        = LocalTransaction<'a>
    where
        Self: 'a;
    fn transaction(&mut self) -> Result<Self::Transaction<'_>> {
        Ok(LocalTransaction {
            current: self.current,
            owner: self.owner,
        })
    }
}
impl AttributionTransaction for LocalTransaction<'_> {
    fn lookup(&mut self, request_ref: &str) -> Result<Option<Instruction>> {
        self.current.lookup(request_ref)
    }
    fn now_ms(&self) -> i64 {
        self.current.now_ms()
    }
}
impl ConsumeOwner for LocalTransaction<'_> {
    fn consume_once(&mut self, key: &str, record: &Js) -> Result<bool> {
        self.owner.consume_once(key, record)
    }
}
fn consume(
    approval: &mut Approval,
    current: &mut Current,
    store: &mut dyn ConsumeOwner,
) -> Result<Verdict> {
    approval.consume(
        &action(),
        &source(),
        1,
        &mut AttributionOwner::new(&mut LocalStore {
            current,
            owner: store,
        }),
    )
}

#[test]
fn exact_schema_version_unknown_fields_and_required_types() {
    assert!(Envelope::parse(&envelope()).is_ok());
    assert_eq!(
        schema::validate("attributionEnvelopeSchema", &envelope()).unwrap(),
        envelope()
    );
    for key in [
        "schemaVersion",
        "runtime",
        "runtimeSessionId",
        "requestRef",
        "requestTextSha256",
        "currentUserEventRef",
        "issuedAt",
        "expiresAt",
        "actionDigests",
    ] {
        let mut missing = envelope();
        if let Js::Obj(props) = &mut missing {
            props.retain(|(k, _)| k != &text(key));
        }
        assert!(Envelope::parse(&missing).is_err(), "missing {key}");
        for bad in [
            Js::Null,
            Js::Undefined,
            Js::Bool(true),
            Js::Num(1.0),
            obj(&[]),
        ] {
            let mut e = envelope();
            e.set(key, bad);
            assert!(Envelope::parse(&e).is_err(), "bad type for {key}");
        }
    }
    for (key, value) in [
        ("schemaVersion", "academy.dailyrecord-attribution/1"),
        ("schemaVersion", "academy.attribution-envelope/2"),
        ("runtime", "claude"),
        ("runtime", "CODEX"),
        ("requestTextSha256", &"A".repeat(64)),
    ] {
        let mut e = envelope();
        e.set(key, value.into());
        assert!(Envelope::parse(&e).is_err());
    }
    let mut extra = envelope();
    extra.set("adapterClaim", true.into());
    assert!(Envelope::parse(&extra).is_err());
    // Duplicate object members are refused even if a caller constructs Js directly.
    let mut duplicate = envelope();
    if let Js::Obj(props) = &mut duplicate {
        props[1].0 = text("schemaVersion");
    }
    assert!(Envelope::parse(&duplicate).is_err());
    for v in [
        Js::Null,
        Js::Arr(vec![]),
        Js::Inherited(envelope().props().to_vec()),
    ] {
        assert!(Envelope::parse(&v).is_err());
    }
}

#[test]
fn grant_uniqueness_is_stricter_than_retained_zod_and_bounds_are_exact() {
    for n in [1, 32] {
        let mut e = envelope();
        e.set(
            "actionDigests",
            Js::Arr((0..n).map(|i| format!("{i:064x}").into()).collect()),
        );
        assert!(Envelope::parse(&e).is_ok());
    }
    for grants in [
        Js::Arr(vec![]),
        Js::Arr(vec!["a".repeat(64).into(); 2]),
        Js::Arr((0..33).map(|i| format!("{i:064x}").into()).collect()),
        strings(&["not-a-digest"]),
        Js::Arr(vec!["A".repeat(64).into()]),
        Js::Arr(vec!["a".repeat(63).into()]),
        Js::Arr(vec!["a".repeat(65).into()]),
        Js::Arr(vec![Js::Num(0.0)]),
    ] {
        let mut e = envelope();
        e.set("actionDigests", grants);
        assert!(Envelope::parse(&e).is_err());
    }
}

#[test]
fn references_use_the_schema_zod_length_intersection_without_trimming() {
    for key in ["runtimeSessionId", "requestRef", "currentUserEventRef"] {
        for (value, valid) in [
            (" ".into(), true),
            ("x".repeat(256), true),
            ("x".repeat(257), false),
            ("😀".repeat(128), true),
            ("😀".repeat(129), false),
            ("😀".repeat(256), false),
            (String::new(), false),
        ] {
            let mut e = envelope();
            e.set(key, value.clone().into());
            assert_eq!(
                Envelope::parse(&e).is_ok(),
                valid,
                "{key} scalar {} UTF16 {}",
                value.chars().count(),
                value.encode_utf16().count()
            );
        }
        let mut e = envelope();
        e.set(key, Js::Str(vec![0xd800]));
        assert!(
            Envelope::parse(&e).is_err(),
            "unpaired surrogate has no scalar value"
        );
    }
}

#[test]
fn timestamps_validate_real_dates_offsets_and_the_strict_grammar() {
    for date in [
        "2000-02-29T01:02:03Z",
        "1970-01-01T09:00:00+09:00",
        "1970-01-01T00:00:00.123456Z",
        "0000-01-01T00:00:00Z",
    ] {
        let mut e = envelope();
        e.set("issuedAt", date.into());
        assert!(Envelope::parse(&e).is_ok(), "{date}");
    }
    for date in [
        "2100-02-29T00:00:00Z",
        "1970-01-00T00:00:00Z",
        "1970-13-01T00:00:00Z",
        "1970-01-01",
        "1970-01-01T00:00:00",
        "1970-01-01T00:00Z",
        "1970-01-01T00:00:00+0900",
        "1970-01-01T24:00:00Z",
        "1970-01-01T00:00:60Z",
        "1970-01-01T00:00:00+24:00",
        "1970-01-01t00:00:00z",
    ] {
        let mut e = envelope();
        e.set("issuedAt", date.into());
        assert!(Envelope::parse(&e).is_err(), "{date}");
    }
}

#[test]
fn text_hash_preserves_order_whitespace_empty_blocks_unicode_and_newlines() {
    let blocks = instruction().text_blocks;
    assert_eq!(
        attribution::normalized_request_text_hash(&blocks),
        "26dc93ca5085fbe79a2cf14b2ebde4019e37731675a4c34a4b2c09508666a2d5"
    );
    for changed in [
        vec!["alpha".into(), "β".into(), "".into()],
        vec!["β".into(), " alpha ".into(), "".into()],
        vec![" alpha \r".into(), "β".into(), "".into()],
    ] {
        assert_ne!(
            attribution::normalized_request_text_hash(&changed),
            attribution::normalized_request_text_hash(&blocks)
        );
    }
    assert_ne!(
        attribution::normalized_request_text_hash(&["é".into()]),
        attribution::normalized_request_text_hash(&["e\u{301}".into()])
    );
    assert_eq!(
        attribution::normalized_request_text_hash(&[]),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn action_digest_binds_full_action_and_source_using_existing_canonical_encoder() {
    assert_eq!(
        attribution::action_digest(&action(), &source()).unwrap(),
        "bc82e4424d66c96df88a987267282220bd21045ccd84102f6262366edb0ce1f4"
    );
    let mut changed = action();
    changed.set("before", "previous value".into());
    assert_ne!(
        attribution::action_digest(&changed, &source()).unwrap(),
        attribution::action_digest(&action(), &source()).unwrap()
    );
    assert_ne!(
        attribution::action_digest(&action(), &"b".repeat(64)).unwrap(),
        attribution::action_digest(&action(), &source()).unwrap()
    );
    for source in ["", "absent", &"A".repeat(64)] {
        assert_eq!(
            attribution::action_digest(&action(), source)
                .unwrap_err()
                .message,
            "attribution_missing_provenance"
        );
    }
}

#[test]
fn all_three_runtimes_require_actual_owner_identity() {
    for runtime in ["muse", "codex", "hermes"] {
        let mut e = envelope();
        e.set("runtime", runtime.into());
        let mut c = Current::default();
        c.value.as_mut().unwrap().runtime = runtime.into();
        let mut store = Consume::default();
        let result = consume(&mut Approval::new(&e).unwrap(), &mut c, &mut store).unwrap();
        assert!(matches!(result, Verdict::Verified { .. }));
        assert_eq!(c.reads, 1);
        c.value.as_mut().unwrap().runtime = "fictional-other-runtime".into();
        c.reads = 0;
        assert_eq!(
            consume(&mut Approval::new(&e).unwrap(), &mut c, &mut store).unwrap(),
            Verdict::Rejected(Rejection::OwnerMismatch)
        );
    }
}

#[test]
fn every_owner_binding_is_checked_and_missing_provenance_is_refused() {
    let cases: Vec<(Instruction, Rejection)> = vec![
        (
            Instruction {
                runtime_session_id: "other-session".into(),
                ..instruction()
            },
            Rejection::OwnerMismatch,
        ),
        (
            Instruction {
                request_ref: "other-request".into(),
                ..instruction()
            },
            Rejection::OwnerMismatch,
        ),
        (
            Instruction {
                current_user_event_ref: "new-event".into(),
                ..instruction()
            },
            Rejection::CurrentEventMismatch,
        ),
        (
            Instruction {
                text_blocks: vec!["other instruction".into()],
                ..instruction()
            },
            Rejection::HashMismatch,
        ),
        (
            Instruction {
                superseded: true,
                ..instruction()
            },
            Rejection::Superseded,
        ),
        (
            Instruction {
                source_sha256: None,
                ..instruction()
            },
            Rejection::MissingProvenance,
        ),
        (
            Instruction {
                source_sha256: Some("bad".into()),
                ..instruction()
            },
            Rejection::MissingProvenance,
        ),
        (
            Instruction {
                source_sha256: Some("b".repeat(64)),
                ..instruction()
            },
            Rejection::SourceMismatch,
        ),
    ];
    for (value, reason) in cases {
        let mut c = Current {
            value: Some(value),
            ..Current::default()
        };
        let mut store = Consume::default();
        assert_eq!(
            consume(&mut Approval::new(&envelope()).unwrap(), &mut c, &mut store).unwrap(),
            Verdict::Rejected(reason)
        );
        assert!(store.records.is_empty());
    }
    let mut c = Current {
        value: None,
        ..Current::default()
    };
    assert_eq!(
        consume(
            &mut Approval::new(&envelope()).unwrap(),
            &mut c,
            &mut Consume::default()
        )
        .unwrap(),
        Verdict::Rejected(Rejection::RequestNotFound)
    );
}

#[test]
fn freshness_is_inclusive_at_issue_exclusive_at_expiry_and_checked_at_consume() {
    for (now, verified) in [(-1, false), (0, true), (59999, true), (60000, false)] {
        let mut c = Current {
            now,
            ..Current::default()
        };
        let mut store = Consume::default();
        let result = Approval::new(&envelope())
            .unwrap()
            .consume(
                &action(),
                &source(),
                now,
                &mut AttributionOwner::new(&mut LocalStore {
                    current: &mut c,
                    owner: &mut store,
                }),
            )
            .unwrap();
        assert_eq!(matches!(result, Verdict::Verified { .. }), verified);
        if !verified {
            assert_eq!(c.reads, 0);
        }
    }
    // Expired or future-dated while preflight was fresh: no owner write.
    for now in [-1, 60000] {
        let mut c = Current {
            now,
            ..Current::default()
        };
        let mut store = Consume::default();
        assert_eq!(
            consume(&mut Approval::new(&envelope()).unwrap(), &mut c, &mut store).unwrap(),
            Verdict::Rejected(Rejection::Expired)
        );
        assert!(store.records.is_empty());
    }
    let mut reversed = envelope();
    reversed.set("expiresAt", "1969-12-31T23:59:59Z".into());
    assert_eq!(
        consume(
            &mut Approval::new(&reversed).unwrap(),
            &mut Current::default(),
            &mut Consume::default()
        )
        .unwrap(),
        Verdict::Rejected(Rejection::Expired)
    );
    let mut offset = envelope();
    offset.set("issuedAt", "1970-01-01T09:00:00+09:00".into());
    assert!(matches!(
        consume(
            &mut Approval::new(&offset).unwrap(),
            &mut Current::default(),
            &mut Consume::default()
        )
        .unwrap(),
        Verdict::Verified { .. }
    ));
}

#[test]
fn changed_action_or_source_cannot_use_an_existing_grant() {
    let mut a = Approval::new(&envelope()).unwrap();
    let mut c = Current::default();
    let mut store = Consume::default();
    let mut changed = action();
    changed.set("after", "Unapproved value".into());
    assert_eq!(
        a.consume(
            &changed,
            &source(),
            1,
            &mut AttributionOwner::new(&mut LocalStore {
                current: &mut c,
                owner: &mut store
            })
        )
        .unwrap(),
        Verdict::Rejected(Rejection::ActionNotBound)
    );
    assert_eq!(
        a.consume(
            &action(),
            &"b".repeat(64),
            1,
            &mut AttributionOwner::new(&mut LocalStore {
                current: &mut c,
                owner: &mut store
            })
        )
        .unwrap(),
        Verdict::Rejected(Rejection::ActionNotBound)
    );
    assert_eq!(c.reads, 0);
    assert!(store.records.is_empty());
}

#[test]
fn fresh_transaction_rejects_owner_change_before_consumption() {
    let mut variants = vec![(None, Rejection::RequestNotFound)];
    for i in 0..7 {
        let mut changed = instruction();
        let reason = match i {
            0 => {
                changed.runtime = "hermes".into();
                Rejection::OwnerMismatch
            }
            1 => {
                changed.runtime_session_id = "other-session".into();
                Rejection::OwnerMismatch
            }
            2 => {
                changed.request_ref = "other-request".into();
                Rejection::OwnerMismatch
            }
            3 => {
                changed.current_user_event_ref = "other-event".into();
                Rejection::CurrentEventMismatch
            }
            4 => {
                changed.text_blocks.push("cancel".into());
                Rejection::HashMismatch
            }
            5 => {
                changed.superseded = true;
                Rejection::Superseded
            }
            _ => {
                changed.source_sha256 = Some("b".repeat(64));
                Rejection::SourceMismatch
            }
        };
        variants.push((Some(changed), reason));
    }
    for (second, reason) in variants {
        let mut c = Current {
            second: Some(second),
            ..Current::default()
        };
        let mut store = Consume::default();
        assert_eq!(
            consume(&mut Approval::new(&envelope()).unwrap(), &mut c, &mut store).unwrap(),
            Verdict::Rejected(reason)
        );
        assert_eq!(c.reads, 1);
        assert!(store.records.is_empty());
    }
}

#[test]
fn single_use_survives_new_approval_and_envelope_reissue() {
    let mut c = Current::default();
    let mut store = Consume::default();
    let mut a = Approval::new(&envelope()).unwrap();
    assert!(matches!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Verified { .. }
    ));
    assert_eq!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
    assert_eq!(c.reads, 1);
    let mut reissued = envelope();
    reissued.set("expiresAt", "1970-01-01T00:02:00Z".into());
    assert_eq!(
        consume(&mut Approval::new(&reissued).unwrap(), &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
    assert_eq!(store.keys.len(), 1);
    assert_eq!(store.records[0].get("request"), &envelope());
    assert!(store.records[0].get("sourceSha256").is(&source()));
}

#[test]
fn distinct_action_grants_are_consumed_independently() {
    let mut other = action();
    other.set("field", "homework".into());
    let mut e = envelope();
    e.set(
        "actionDigests",
        Js::Arr(vec![
            attribution::action_digest(&action(), &source())
                .unwrap()
                .into(),
            attribution::action_digest(&other, &source())
                .unwrap()
                .into(),
        ]),
    );
    let mut a = Approval::new(&e).unwrap();
    let mut c = Current::default();
    c.value
        .as_mut()
        .unwrap()
        .action_digests
        .push(attribution::action_digest(&other, &source()).unwrap());
    let mut store = Consume::default();
    assert!(matches!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Verified { .. }
    ));
    assert!(matches!(
        a.consume(
            &other,
            &source(),
            1,
            &mut AttributionOwner::new(&mut LocalStore {
                current: &mut c,
                owner: &mut store
            })
        )
        .unwrap(),
        Verdict::Verified { .. }
    ));
    assert_eq!(store.keys.len(), 2);
}

#[test]
fn adapter_cannot_invent_an_action_grant_from_a_real_user_event() {
    let mut c = Current::default();
    c.value.as_mut().unwrap().action_digests.clear();
    let mut store = Consume::default();
    assert_eq!(
        consume(&mut Approval::new(&envelope()).unwrap(), &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::ActionNotBound)
    );
    assert!(store.records.is_empty());
    let mut changed = instruction();
    changed.action_digests.clear();
    c = Current {
        second: Some(Some(changed)),
        ..Current::default()
    };
    assert_eq!(
        consume(&mut Approval::new(&envelope()).unwrap(), &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::ActionNotBound)
    );
    assert!(store.records.is_empty());
}

#[test]
fn malformed_input_never_reaches_an_owner_and_envelope_is_detached() {
    let mut e = envelope();
    let mut a = Approval::new(&e).unwrap();
    e.set("runtime", "hermes".into());
    e.set("actionDigests", Js::Arr(vec![]));
    assert!(Approval::new(&e).is_err());
    let mut c = Current::default();
    let mut store = Consume::default();
    assert!(matches!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Verified { .. }
    ));
}

#[test]
fn owner_errors_propagate_and_ambiguous_consume_is_locally_fenced() {
    let mut a = Approval::new(&envelope()).unwrap();
    let mut c = Current {
        fail: true,
        ..Current::default()
    };
    let mut store = Consume::default();
    assert_eq!(
        consume(&mut a, &mut c, &mut store).unwrap_err().message,
        "synthetic_lookup_failure"
    );
    assert!(store.records.is_empty());
    c.fail = false;
    assert_eq!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
    let mut a = Approval::new(&envelope()).unwrap();
    store.fail_after_write = true;
    assert_eq!(
        consume(&mut a, &mut c, &mut store).unwrap_err().message,
        "synthetic_uncertain_consume"
    );
    let reads = c.reads;
    assert_eq!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
    assert_eq!(c.reads, reads);
    assert_eq!(store.records.len(), 1);
    // New process/session still depends on the owner's existing durable fence.
    store.fail_after_write = false;
    assert_eq!(
        consume(&mut Approval::new(&envelope()).unwrap(), &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
}

#[test]
fn owner_false_is_a_refusal_and_not_retried() {
    let mut a = Approval::new(&envelope()).unwrap();
    let mut c = Current::default();
    let mut store = Consume {
        refuse: true,
        ..Consume::default()
    };
    assert_eq!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
    store.refuse = false;
    assert_eq!(
        consume(&mut a, &mut c, &mut store).unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
    assert!(store.keys.is_empty());
    assert_eq!(store.records.len(), 1);
}

#[test]
fn existing_durable_consume_store_refuses_replay_after_reopen_and_concurrently() {
    let parent = std::env::var_os("S10_TEST_ROOT").expect("explicit synthetic fixture root");
    let root = std::path::PathBuf::from(parent).join(format!("e06-consume-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let root = root.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = DurableConsume { root: &root };
                barrier.wait();
                consume(
                    &mut Approval::new(&envelope()).unwrap(),
                    &mut Current::default(),
                    &mut store,
                )
                .unwrap()
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Verdict::Verified { .. }))
            .count(),
        1
    );
    assert_eq!(
        consume(
            &mut Approval::new(&envelope()).unwrap(),
            &mut Current::default(),
            &mut DurableConsume { root: &root }
        )
        .unwrap(),
        Verdict::Rejected(Rejection::Replay)
    );
    let files: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let record = codec::parse(&std::fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!(record.get("request"), &envelope());
    assert!(
        record
            .get("actionDigest")
            .is(&attribution::action_digest(&action(), &source()).unwrap())
    );
    std::fs::remove_file(&files[0]).unwrap();
    std::fs::remove_dir(&root).unwrap();
}

// A synthetic actual owner: all instruction/grant mutations and consumption
// use the same state mutex. The transaction owns its guard until after commit.
struct Gate {
    reached: std::sync::mpsc::SyncSender<()>,
    resume: std::sync::mpsc::Receiver<()>,
}
impl Gate {
    fn pause(self) {
        self.reached.send(()).unwrap();
        self.resume
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }
}
struct SharedState {
    current: Instruction,
    store: Consume,
    before_consume: Option<Gate>,
    events: Vec<&'static str>,
}
impl Default for SharedState {
    fn default() -> Self {
        Self {
            current: instruction(),
            store: Consume::default(),
            before_consume: None,
            events: vec![],
        }
    }
}
struct SharedStore {
    state: std::sync::Arc<std::sync::Mutex<SharedState>>,
    before_transaction: Option<Gate>,
}
struct SharedTransaction<'a>(std::sync::MutexGuard<'a, SharedState>);
impl AttributionStore for SharedStore {
    type Transaction<'a> = SharedTransaction<'a>;
    fn transaction(&mut self) -> Result<Self::Transaction<'_>> {
        if let Some(gate) = self.before_transaction.take() {
            // Advisory old-style state observation: reproduce the old valid
            // comparison -> mutation -> consume order, then acquire the actual
            // owner transaction. Core must freshly check inside this handle.
            assert_eq!(self.state.lock().unwrap().current, instruction());
            gate.pause();
        }
        Ok(SharedTransaction(self.state.lock().unwrap()))
    }
}
impl AttributionTransaction for SharedTransaction<'_> {
    fn lookup(&mut self, request_ref: &str) -> Result<Option<Instruction>> {
        assert_eq!(request_ref, "synthetic:request:1");
        Ok(Some(self.0.current.clone()))
    }
    fn now_ms(&self) -> i64 {
        1
    }
}
impl ConsumeOwner for SharedTransaction<'_> {
    fn consume_once(&mut self, key: &str, record: &Js) -> Result<bool> {
        // This hook is exactly after core's final claim.check and immediately
        // before the existing consume-once sink; the owner guard is still held.
        if let Some(gate) = self.0.before_consume.take() {
            gate.pause();
        }
        let consumed = self.0.store.consume_once(key, record)?;
        if consumed {
            self.0.events.push("consume");
        }
        Ok(consumed)
    }
}

fn former_interval_change_must_refuse(changed: Instruction, reason: Rejection) {
    use std::sync::{Arc, Mutex, mpsc};
    let state = Arc::new(Mutex::new(SharedState::default()));
    let (compared_tx, compared_rx) = mpsc::sync_channel(0);
    let (changed_tx, changed_rx) = mpsc::sync_channel(0);
    let mutation_state = state.clone();
    let mutation = std::thread::spawn(move || {
        compared_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        {
            let mut transaction = mutation_state.lock().unwrap();
            transaction.current = changed;
            transaction.events.push("mutation");
        }
        changed_tx.send(()).unwrap();
    });
    let mut store = SharedStore {
        state: state.clone(),
        before_transaction: Some(Gate {
            reached: compared_tx,
            resume: changed_rx,
        }),
    };
    let verdict = Approval::new(&envelope())
        .unwrap()
        .consume(
            &action(),
            &source(),
            1,
            &mut AttributionOwner::new(&mut store),
        )
        .unwrap();
    mutation.join().unwrap();
    assert_eq!(verdict, Verdict::Rejected(reason));
    let state = state.lock().unwrap();
    assert!(state.store.keys.is_empty());
    assert!(state.store.records.is_empty());
    assert_eq!(state.events, ["mutation"]);
}

#[test]
fn former_interval_rejects_revocation_before_consumption() {
    let mut revoked = instruction();
    revoked.action_digests.clear();
    former_interval_change_must_refuse(revoked, Rejection::ActionNotBound);
}

#[test]
fn former_interval_rejects_instruction_change_before_consumption() {
    let mut changed = instruction();
    changed.current_user_event_ref = "synthetic:replacement-event".into();
    changed.text_blocks = vec!["Synthetic replacement instruction".into()];
    former_interval_change_must_refuse(changed, Rejection::CurrentEventMismatch);
}

#[test]
fn transaction_blocks_revocation_between_final_comparison_and_consume() {
    use std::sync::{Arc, Mutex, TryLockError, mpsc};
    let (compared_tx, compared_rx) = mpsc::sync_channel(0);
    let (attempted_tx, attempted_rx) = mpsc::sync_channel(0);
    let state = Arc::new(Mutex::new(SharedState {
        before_consume: Some(Gate {
            reached: compared_tx,
            resume: attempted_rx,
        }),
        ..SharedState::default()
    }));
    let mutation_state = state.clone();
    let mutation = std::thread::spawn(move || {
        compared_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        // We are in the old dangerous interval. A revoker using the owner
        // boundary cannot commit until the comparison+consume transaction ends.
        assert!(matches!(
            mutation_state.try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        attempted_tx.send(()).unwrap();
        let mut transaction = mutation_state.lock().unwrap();
        transaction.current.action_digests.clear();
        transaction.events.push("revoke");
    });
    let mut store = SharedStore {
        state: state.clone(),
        before_transaction: None,
    };
    let verdict = Approval::new(&envelope())
        .unwrap()
        .consume(
            &action(),
            &source(),
            1,
            &mut AttributionOwner::new(&mut store),
        )
        .unwrap();
    mutation.join().unwrap();
    assert!(matches!(verdict, Verdict::Verified { .. }));
    assert_eq!(state.lock().unwrap().events, ["consume", "revoke"]);
    // Once revocation commits, even a new Approval cannot report Verified.
    let verdict = Approval::new(&envelope())
        .unwrap()
        .consume(
            &action(),
            &source(),
            1,
            &mut AttributionOwner::new(&mut store),
        )
        .unwrap();
    assert_eq!(verdict, Verdict::Rejected(Rejection::ActionNotBound));
    assert_eq!(state.lock().unwrap().store.records.len(), 1);
}
