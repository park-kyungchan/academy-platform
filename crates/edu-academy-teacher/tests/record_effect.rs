//! Invented recorded answers and disposable shared state only; no native calls.
use edu_academy_core::{
    js::{Js, Result, obj},
    workflow::{
        Runtime,
        approval::RequestApproval,
        codec, same,
        service::{Backend, Source},
        state,
        store::Store,
    },
};
use edu_academy_teacher::{
    record_backend::*, record_data::*, record_effect::*, record_fixture::RecordFixture,
    record_store::DraftStore,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const FIXTURE: &str = include_str!("fixtures/record-answers.json");
fn fixture() -> RecordFixture {
    RecordFixture::from_json(FIXTURE).unwrap()
}
fn setup(fixture: RecordFixture) -> (SyntheticCoreSource, PathBuf) {
    let root = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap()).join(format!(
        "record-effect-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let state = SyntheticRecordState::open(&root, fixture.seed, 10000).unwrap();
    (
        SyntheticCoreSource {
            state: Arc::new(Mutex::new(state)),
            records: fixture.records,
        },
        root,
    )
}
fn scope(source: &SyntheticCoreSource, index: usize) -> Js {
    let record = &source.records[index];
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "9001".into()),
        (
            "studentId",
            record.snapshot.identity.student_key.clone().into(),
        ),
        (
            "lessonDate",
            record.snapshot.identity.lesson_date.clone().into(),
        ),
    ])
}
fn read(source: &mut SyntheticCoreSource, index: usize) -> Js {
    use edu_academy_core::workflow::service::Source;
    source
        .read(&scope(source, index), &"invented-request".into(), &Js::Null)
        .unwrap()
}
#[test]
fn exact_per_field_targets() {
    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let homework =
        &mut value["answers"][0]["capture"]["answer"]["structuredContent"]["fields"][1]["target"];
    homework["courseId"] = "invented-other-course".into();
    homework["membershipId"] = "invented-other-membership".into();
    let fixture = RecordFixture::from_json(&value.to_string()).unwrap();
    let expected = fixture.records[0].clone();
    let (mut source, _) = setup(fixture);
    let result = read(&mut source, 0);
    let observation = result.get("observation");
    assert!(
        same(
            observation.get("target"),
            expected.field_evidence.progress.get("target")
        )
        .unwrap()
    );
    assert!(
        same(
            observation.get("previousTarget"),
            expected
                .field_evidence
                .previous_homework_assessment
                .get("target")
        )
        .unwrap()
    );
    for field in RecordField::ALL {
        let target = expected.field_evidence[field].get("target");
        let actual = observation.get("fields").get(field.wire_name());
        assert_eq!(actual.get("effectRecordKey"), target.get("recordId"));
        // Independent serde JSON object ordering supplies the expected full string.
        let expected_json: serde_json::Value =
            serde_json::from_slice(&codec::bytes(target).unwrap()).unwrap();
        assert!(actual.get("effectTarget").is(&expected_json.to_string()));
        assert_eq!(
            actual.get("value"),
            expected.field_evidence[field].get("before").get("value")
        );
        assert_eq!(
            actual.get("evidenceRefs"),
            expected.field_evidence[field]
                .get("before")
                .get("evidenceRefs")
        );
    }
    let fields = observation.get("fields");
    assert_eq!(
        fields.get("progress").get("effectRecordKey"),
        fields.get("homework").get("effectRecordKey")
    );
    assert_ne!(
        fields.get("progress").get("effectTarget"),
        fields.get("homework").get("effectTarget")
    );
    assert!(
        observation.get("previousTarget").get("lessonDate").units()
            < scope(&source, 0).get("lessonDate").units()
    );
    let encoded = canonical_effect_target(expected.field_evidence.progress.get("target")).unwrap();
    assert_ne!(
        encoded,
        expected
            .field_evidence
            .progress
            .get("target")
            .get("recordId")
            .code_units()
    );
    assert_eq!(source.state.lock().unwrap().read_count(), 1);
}
#[test]
fn failed_recorded_answer_is_not_absence() {
    for index in 4..7 {
        let mut fixture = fixture();
        let failed = fixture.records[index].clone();
        assert!(
            !fixture
                .seed
                .records
                .iter()
                .any(|r| r.identity == failed.snapshot.identity)
        );
        fixture.records = vec![failed.clone()];
        let (mut source, _) = setup(fixture);
        let output = read(&mut source, 0);
        assert_eq!(output, failed.answer);
        assert!(output.get("status").is("unavailable"));
        assert!(!output.has("observation"));
        let state = source.state.lock().unwrap();
        assert_eq!(state.read_count(), 0);
        assert_eq!(state.dispatch_count(), 0);
        assert_eq!(state.source_operations(), 0);
    }
}
#[test]
fn synthetic_core_source_reloads_values() {
    let fixture = fixture();
    let seed = fixture.seed.clone();
    let owner = seed.owner.clone();
    let target = seed.writable_targets[0].synthetic_target.clone();
    let (mut source, root) = setup(fixture);
    let first = read(&mut source, 0);
    let original = source.records[0].clone();
    let store = DraftStore::open(&root, owner).unwrap();
    {
        let account = store.lock_account().unwrap();
        let mut patch = RecordFields::from_fn(|_| None);
        patch.progress = Some(FieldPatch {
            effect_target: target,
            operation: PatchOperation::Set,
            value: "- Invented shared-state update".into(),
        });
        source
            .state
            .lock()
            .unwrap()
            .dispatch_with_account(
                &account,
                &original.snapshot.identity,
                &patch,
                &original.snapshot.provenance.read_at,
            )
            .unwrap();
    }
    let second = read(&mut source, 0);
    assert!(
        second
            .get("observation")
            .get("fields")
            .get("progress")
            .get("value")
            .is("- Invented shared-state update")
    );
    assert_ne!(
        first
            .get("observation")
            .get("fields")
            .get("progress")
            .get("value"),
        second
            .get("observation")
            .get("fields")
            .get("progress")
            .get("value")
    );
    for key in ["sourceRevision", "wireDigest", "observedAt"] {
        assert_eq!(
            first.get("observation").get(key),
            second.get("observation").get(key)
        );
    }
    assert_eq!(source.records[0].snapshot, original.snapshot);
    let state = SyntheticRecordState::open(&root, seed, 10000).unwrap();
    let mut reopened = SyntheticCoreSource {
        state: Arc::new(Mutex::new(state)),
        records: source.records.clone(),
    };
    assert_eq!(
        read(&mut reopened, 0).get("observation"),
        second.get("observation")
    );
    assert_eq!(source.state.lock().unwrap().read_count(), 2);
    assert_eq!(source.state.lock().unwrap().dispatch_count(), 1);
}
#[test]
fn source_unknown_preserves_binding_and_refuses_scope_drift() {
    use edu_academy_core::workflow::service::Source;
    let mut fixture = fixture();
    let RecordObservation::Observed { fields, .. } = &mut fixture.seed.records[0].observation
    else {
        panic!("observed fixture")
    };
    fields.homework = RecordValue::Unknown;
    fixture.seed.records[0].provenance.field_reasons.homework = Some("invented_unknown".into());
    let (mut source, _) = setup(fixture);
    let value = read(&mut source, 0);
    let homework = value.get("observation").get("fields").get("homework");
    assert!(homework.get("state").is("unknown"));
    assert!(homework.get("reasonCode").is("invented_unknown"));
    assert!(homework.get("effectTarget").string());
    let mut wrong = scope(&source, 0);
    wrong.set("teacherId", "different-teacher".into());
    assert_eq!(
        source
            .read(&wrong, &Js::Null, &Js::Null)
            .unwrap_err()
            .message,
        "source_scope_conflict"
    );
    assert_eq!(source.state.lock().unwrap().read_count(), 1);
    source.records[0].snapshot.identity.owner.account = "different-owner".into();
    assert!(
        source
            .read(&scope(&source, 0), &Js::Null, &Js::Null)
            .is_err()
    );
    assert_eq!(source.state.lock().unwrap().read_count(), 1);
}
#[test]
fn synthetic_core_source_does_not_retry() {
    use edu_academy_core::workflow::service::Source;
    for outcome in [ReadOutcome::Timeout, ReadOutcome::Unavailable] {
        let mut fixture = fixture();
        let identity = fixture.records[0].snapshot.identity.clone();
        fixture.seed.read_scripts = vec![
            ReadScript {
                identity: identity.clone(),
                outcome,
            },
            ReadScript {
                identity,
                outcome: ReadOutcome::Observed { elapsed_ms: 0 },
            },
        ];
        let (mut source, _) = setup(fixture);
        assert!(
            source
                .read(&scope(&source, 0), &Js::Null, &Js::Null)
                .is_err()
        );
        assert_eq!(source.state.lock().unwrap().read_count(), 1);
        // A separate explicit read may succeed; the failed invocation never retries.
        assert!(read(&mut source, 0).get("status").is("observed"));
        assert_eq!(source.state.lock().unwrap().read_count(), 2);
    }
}

#[test]
fn per_field_writable_targets_survive_reopen() {
    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    for (i, row) in value["answers"][0]["capture"]["answer"]["structuredContent"]["fields"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        row["target"]["recordId"] = format!("invented-record-{i}").into();
    }
    let mut fixture = RecordFixture::from_json(&value.to_string()).unwrap();
    fixture.seed.effect_scripts.clear();
    let seed = fixture.seed.clone();
    let record = fixture.records[0].clone();
    let id = &record.snapshot.identity;
    let targets = RecordFields::from_fn(|field| {
        canonical_effect_target(record.field_evidence[field].get("target")).unwrap()
    });
    assert_eq!(
        RecordField::ALL
            .iter()
            .map(|f| &targets[*f])
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        RECORD_FIELD_COUNT
    );
    let (source, root) = setup(fixture);
    let store = DraftStore::open(&root, seed.owner.clone()).unwrap();
    let mut expected = RecordFields::from_fn(|_| RecordValue::Unknown);
    {
        let account = store.lock_account().unwrap();
        let mut state = source.state.lock().unwrap();
        let mut patch = RecordFields::from_fn(|_| None);
        patch.progress = Some(FieldPatch {
            effect_target: targets.homework.clone(),
            operation: PatchOperation::Set,
            value: "- Invented progress".into(),
        });
        assert_eq!(
            state
                .dispatch_with_account(&account, id, &patch, &record.snapshot.provenance.read_at)
                .unwrap_err()
                .message,
            "record_source_invalid"
        );
        patch.progress.as_mut().unwrap().effect_target = targets.progress.clone();
        assert_eq!(
            state
                .dispatch_with_account(
                    &account,
                    &seed.writable_targets[6].identity,
                    &patch,
                    &record.snapshot.provenance.read_at
                )
                .unwrap_err()
                .message,
            "record_source_invalid"
        );
        assert_eq!(state.dispatch_count(), 0);
        for field in RecordField::ALL {
            let value = match field {
                RecordField::Attendance => "L",
                RecordField::DailyTestAssessment => "3",
                RecordField::PreviousHomeworkAssessment => "2",
                _ => "- Invented field update",
            };
            let mut patch = RecordFields::from_fn(|_| None);
            patch[field] = Some(FieldPatch {
                effect_target: targets[field].clone(),
                operation: PatchOperation::Set,
                value: value.into(),
            });
            let result = state
                .dispatch_with_account(&account, id, &patch, &record.snapshot.provenance.read_at)
                .unwrap();
            assert_eq!(
                result,
                vec![EffectResult {
                    field,
                    outcome: EffectOutcome::Success,
                    transport: TransportOutcome::Returned
                }]
            );
            expected[field] = RecordValue::Known(value.into());
        }
        assert_eq!(state.dispatch_count(), RECORD_FIELD_COUNT as u64);
    }
    let persisted: RecordSeed =
        serde_json::from_slice(&std::fs::read(root.join(SOURCE_SNAPSHOT)).unwrap()).unwrap();
    assert_eq!(persisted.writable_targets, seed.writable_targets);
    let mut reopened = SyntheticRecordState::open(&root, seed, 10000).unwrap();
    let read = reopened
        .read(id, &record.snapshot.provenance.read_at)
        .unwrap();
    let RecordObservation::Observed { fields, .. } = read.observation else {
        panic!("observed required")
    };
    assert_eq!(fields, expected);
}

struct HeldClock;
impl Runtime for HeldClock {
    fn now(&self) -> String {
        "2026-11-13T09:00:00Z".into()
    }
    fn epoch_ms(&self) -> u64 {
        0
    }
    fn uuid(&self) -> Result<String> {
        Ok(format!("held-{}", NEXT.fetch_add(1, Ordering::Relaxed)))
    }
}

#[test]
fn held_entry_points_do_not_reacquire_account() {
    let mut fixture = fixture();
    fixture.seed.effect_scripts.clear();
    let owner = fixture.seed.owner.clone();
    let (mut source, root) = setup(fixture);
    let scope = scope(&source, 0);
    let before = read(&mut source, 0);
    let field = before.get("observation").get("fields").get("progress");
    let patches = Js::Arr(vec![obj(&[
        ("field", "progress".into()),
        ("value", "- Invented held-account update".into()),
        ("before", field.get("value").clone()),
        ("effectTarget", field.get("effectTarget").clone()),
        ("effectRecordKey", field.get("effectRecordKey").clone()),
        (
            "wireDigest",
            before.get("observation").get("wireDigest").clone(),
        ),
    ])]);
    let mut reviewed = patches.array()[0].clone();
    reviewed.set("after", reviewed.get("value").clone());
    let core = Store::open(&root.join("core"), &root).unwrap();
    let mut approval = RequestApproval {
        store: &core,
        input: obj(&[
            ("scope", scope.clone()),
            ("requestRef", "invented-held-request".into()),
            ("reviewed", Js::Arr(vec![reviewed])),
            ("reviewDigest", "invented-held-review".into()),
        ]),
        runtime: &HeldClock,
    };
    let mut backend = SyntheticCoreBackend {
        state: Arc::clone(&source.state),
    };
    let drafts = DraftStore::open(&root, owner).unwrap();
    let account = drafts.lock_account().unwrap();
    // The standalone methods fail before consuming approval or invoking effects.
    assert_eq!(
        source
            .read(&scope, &Js::Null, &Js::Null)
            .unwrap_err()
            .message,
        "shared_writer_lock_busy"
    );
    assert_eq!(
        backend
            .save(&scope, &patches, &mut approval, &Js::Null)
            .unwrap_err()
            .message,
        "shared_writer_lock_busy"
    );
    assert_eq!(source.state.lock().unwrap().dispatch_count(), 0);
    assert!(state::student_executions(&core, &scope).unwrap().is_empty());
    assert_eq!(
        std::fs::read_dir(core.root().join("execution"))
            .unwrap()
            .count(),
        0
    );
    let held = source
        .read_with_account(&account, &scope, &Js::Null, &HeldClock.now())
        .unwrap();
    assert_eq!(
        held.get("observation")
            .get("fields")
            .get("progress")
            .get("value"),
        field.get("value")
    );
    let saved = backend
        .save_with_account(&account, &scope, &patches, &mut approval, &Js::Null)
        .unwrap();
    assert!(saved.array()[0].get("status").is("persisted"));
    assert_eq!(source.state.lock().unwrap().dispatch_count(), 1);
    let executions = state::student_executions(&core, &scope).unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(
        executions[0].record.get("effectTarget"),
        field.get("effectTarget")
    );
    let current = source
        .read_with_account(&account, &scope, &Js::Null, &HeldClock.now())
        .unwrap();
    assert_eq!(
        current
            .get("observation")
            .get("fields")
            .get("progress")
            .get("value"),
        patches.array()[0].get("value")
    );
}

#[test]
fn held_read_reports_caller_read_time() {
    let fixture = fixture();
    let owner = fixture.seed.owner.clone();
    let (mut source, root) = setup(fixture);
    let scope = scope(&source, 0);
    let recorded = read(&mut source, 0);
    let read_at = HeldClock.now();
    assert!(!recorded.get("observation").get("observedAt").is(&read_at));
    let drafts = DraftStore::open(&root, owner).unwrap();
    let account = drafts.lock_account().unwrap();
    let result = source
        .read_with_account(&account, &scope, &Js::Null, &read_at)
        .unwrap();
    assert!(result.get("observation").get("observedAt").is(&read_at));
    for field in RecordField::ALL {
        let old = recorded
            .get("observation")
            .get("fields")
            .get(field.wire_name());
        let current = result
            .get("observation")
            .get("fields")
            .get(field.wire_name());
        assert_eq!(current.get("value"), old.get("value"));
        assert_eq!(current.get("effectTarget"), old.get("effectTarget"));
        if current.get("state").is("known") {
            assert_ne!(current.get("evidenceRefs"), old.get("evidenceRefs"));
            assert!(
                current
                    .get("evidenceRefs")
                    .array()
                    .iter()
                    .all(|r| r.code_units().contains(&read_at))
            );
        }
    }
    assert_eq!(source.state.lock().unwrap().read_count(), 2);
    assert_eq!(source.state.lock().unwrap().dispatch_count(), 0);
}
