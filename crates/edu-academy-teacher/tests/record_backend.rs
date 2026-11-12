use edu_academy_teacher::{record_backend::*, record_data::*, record_store::DraftStore};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const NOW: &str = "2026-11-11T18:10:00+09:00";
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "record-backend-b1-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn root(&self) -> PathBuf {
        self.0.join("root")
    }
    fn path(&self) -> PathBuf {
        self.root().join(SOURCE_SNAPSHOT)
    }
    fn open(&self, seed: RecordSeed) -> SyntheticRecordState {
        SyntheticRecordState::open(&self.root(), seed, 10000).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn owner() -> OwnerScope {
    OwnerScope {
        owner: "synthetic-owner".into(),
        account: "synthetic-account".into(),
    }
}
fn identity() -> RecordIdentity {
    RecordIdentity {
        owner: owner(),
        student_key: "1001".into(),
        lesson_date: "2026-11-09".into(),
        kind: RecordKind::DailyRecord,
    }
}
fn seed() -> RecordSeed {
    RecordSeed {
        format: RECORD_SOURCE_FORMAT.into(),
        owner: owner(),
        writable_targets: vec![WritableTarget {
            identity: identity(),
            synthetic_target: "synthetic-1001-20261109".into(),
        }],
        read_scripts: vec![],
        effect_scripts: vec![],
        records: vec![RecordSnapshot {
            identity: identity(),
            requested_date: "2026-11-09".into(),
            provenance: RecordProvenance {
                read_at: NOW.into(),
                evidence_refs: vec!["S-019".into()],
                source_revision: Some("synthetic-revision".into()),
                wire_digest: None,
                field_reasons: RecordFields::from_fn(|f| {
                    (f == RecordField::DailyTestAssessment).then(|| "not_observed".into())
                }),
            },
            observation: RecordObservation::Observed {
                occurrence: "synthetic-1001-20261109".into(),
                fields: RecordFields {
                    progress: RecordValue::Known("가우스 5-1-2 2-1 필수예제".into()),
                    homework: RecordValue::Known("raw legacy\n| homework".into()),
                    lesson_memo: RecordValue::Known(String::new()),
                    attendance: RecordValue::Known("Y".into()),
                    daily_test_assessment: RecordValue::Unknown,
                    previous_homework_assessment: RecordValue::Known("-1".into()),
                },
            },
        }],
    }
}
fn error<T>(result: edu_academy_core::js::Result<T>) -> String {
    result.err().expect("expected failure").message
}
fn snapshot(path: &Path, value: &RecordSeed) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn changed_record() -> RecordSnapshot {
    let mut row = seed().records.remove(0);
    row.observation = RecordObservation::Observed {
        occurrence: "changed-synthetic-occurrence".into(),
        fields: RecordFields::from_fn(|f| RecordValue::Known(format!("changed {f:?}"))),
    };
    row.provenance.evidence_refs = vec!["S-021".into()];
    row.provenance.source_revision = Some("changed-revision".into());
    row.provenance.wire_digest = Some("a".repeat(64));
    row.provenance.field_reasons = RecordFields::from_fn(|_| Some("changed-reason".into()));
    row
}
fn progress_patch() -> RecordPatch {
    let mut patch = RecordFields::from_fn(|_| None);
    patch.progress = Some(FieldPatch {
        effect_target: "synthetic-1001-20261109".into(),
        operation: PatchOperation::Set,
        value: "persisted synthetic progress".into(),
    });
    patch
}
#[test]
fn remaining_effect_scripts_survive_reopen_in_order() {
    let t = Scratch::new();
    let mut input = seed();
    input.effect_scripts = [EffectOutcome::Reject, EffectOutcome::AppliedLostAck]
        .into_iter()
        .map(|outcome| EffectScript {
            identity: identity(),
            field: RecordField::Progress,
            outcome,
            readback: None,
        })
        .collect();
    let mut state = t.open(input);
    let store = DraftStore::open(&t.root(), owner()).unwrap();
    {
        let held = store.lock_account().unwrap();
        assert_eq!(
            state
                .dispatch_with_account(&held, &identity(), &progress_patch(), NOW)
                .unwrap()[0]
                .transport,
            TransportOutcome::Returned
        );
    }
    let remaining: RecordSeed = serde_json::from_slice(&fs::read(t.path()).unwrap()).unwrap();
    assert_eq!(remaining.effect_scripts.len(), 1);
    assert_eq!(
        remaining.effect_scripts[0].outcome,
        EffectOutcome::AppliedLostAck
    );
    drop(state);
    let mut state = t.open(seed());
    let held = store.lock_account().unwrap();
    assert_eq!(
        state
            .dispatch_with_account(&held, &identity(), &progress_patch(), NOW)
            .unwrap()[0]
            .transport,
        TransportOutcome::Lost
    );
    let RecordObservation::Observed { fields, .. } = state
        .read_with_account(&held, &identity(), NOW)
        .unwrap()
        .observation
    else {
        panic!("record required")
    };
    assert_eq!(
        fields.progress,
        RecordValue::Known(progress_patch().progress.unwrap().value)
    );
    let remaining: RecordSeed = serde_json::from_slice(&fs::read(t.path()).unwrap()).unwrap();
    assert!(remaining.effect_scripts.is_empty());
}
#[test]
fn interleaved_read_scripts_consume_only_the_queried_identity() {
    let t = Scratch::new();
    let mut other = identity();
    other.student_key = "1003".into();
    let unrelated = ReadScript {
        identity: other,
        outcome: ReadOutcome::Unavailable,
    };
    let mut input = seed();
    input.read_scripts = vec![
        unrelated.clone(),
        ReadScript {
            identity: identity(),
            outcome: ReadOutcome::Timeout,
        },
        unrelated.clone(),
        ReadScript {
            identity: identity(),
            outcome: ReadOutcome::Observed { elapsed_ms: 0 },
        },
    ];
    let mut state = t.open(input);
    assert_eq!(state.read(&identity(), NOW).unwrap(), seed().records[0]);
    assert_eq!(state.read_count(), 2);
    let remaining: RecordSeed = serde_json::from_slice(&fs::read(t.path()).unwrap()).unwrap();
    assert_eq!(
        remaining.read_scripts,
        vec![unrelated.clone(), unrelated.clone()]
    );
    assert!(state.read(&unrelated.identity, NOW).is_err());
    let remaining: RecordSeed = serde_json::from_slice(&fs::read(t.path()).unwrap()).unwrap();
    assert_eq!(remaining.read_scripts, vec![unrelated]);
}
#[test]
fn foreign_dispatch_guard_preserves_source_and_every_counter() {
    let t = Scratch::new();
    let mut state = t.open(seed());
    let other = Scratch::new();
    let store = DraftStore::open(&other.root(), owner()).unwrap();
    let held = store.lock_account().unwrap();
    let bytes = fs::read(t.path()).unwrap();
    let counters = (
        state.read_count(),
        state.dispatch_count(),
        state.notification_count(),
        state.source_operations(),
    );
    assert_eq!(
        error(state.dispatch_with_account(&held, &identity(), &progress_patch(), NOW)),
        "record_source_root_mismatch"
    );
    assert_eq!(fs::read(t.path()).unwrap(), bytes);
    assert_eq!(
        (
            state.read_count(),
            state.dispatch_count(),
            state.notification_count(),
            state.source_operations()
        ),
        counters
    );
}
#[test]
fn synthetic_success_failure_ack_loss_and_reopen() {
    use EffectOutcome::*;
    for (outcome, readback, transport, applied) in [
        (Success, None, TransportOutcome::Returned, true),
        (Reject, None, TransportOutcome::Returned, false),
        (AppliedLostAck, None, TransportOutcome::Lost, true),
        (UnappliedLostAck, None, TransportOutcome::Lost, false),
        (
            ReturnedTimeout,
            None,
            TransportOutcome::ReturnedTimeout,
            false,
        ),
        (
            Success,
            Some(ReadOutcome::Unavailable),
            TransportOutcome::Returned,
            true,
        ),
    ] {
        let t = Scratch::new();
        let mut input = seed();
        input.effect_scripts.push(EffectScript {
            identity: identity(),
            field: RecordField::Progress,
            outcome,
            readback: readback.clone(),
        });
        let mut state = t.open(input);
        let store = DraftStore::open(&t.root(), owner()).unwrap();
        let held = store.lock_account().unwrap();
        let result = state
            .dispatch_with_account(&held, &identity(), &progress_patch(), NOW)
            .unwrap();
        assert_eq!(
            result,
            vec![EffectResult {
                field: RecordField::Progress,
                transport
            }]
        );
        assert_eq!(state.dispatch_count(), 1);
        assert_eq!(state.notification_count(), 0);
        if readback.is_some() {
            assert_eq!(
                error(state.read_with_account(&held, &identity(), NOW)),
                "record_source_unavailable"
            );
        }
        let observed = state.read_with_account(&held, &identity(), NOW).unwrap();
        let RecordObservation::Observed { fields, .. } = observed.observation else {
            panic!("record required")
        };
        let RecordObservation::Observed { fields: before, .. } =
            seed().records.remove(0).observation
        else {
            unreachable!()
        };
        for field in RecordField::ALL {
            assert_eq!(
                fields[field],
                if applied && field == RecordField::Progress {
                    RecordValue::Known(progress_patch().progress.unwrap().value)
                } else {
                    before[field].clone()
                }
            );
        }
        assert_eq!(state.read_count(), 1 + u64::from(readback.is_some()));
        drop(held);
        drop(state);
        let mut reopened = t.open(seed());
        assert_eq!(
            reopened.read(&identity(), NOW).unwrap().observation,
            RecordObservation::Observed {
                occurrence: "synthetic-1001-20261109".into(),
                fields
            }
        );
        let stored: RecordSeed = serde_json::from_slice(&fs::read(t.path()).unwrap()).unwrap();
        assert!(stored.effect_scripts.is_empty() && stored.read_scripts.is_empty());
        assert_eq!(reopened.dispatch_count(), 0); // Process counters never replay effects.
    }
}
#[test]
fn full_record_exact_date_and_owner_timeout_and_retry_boundaries() {
    for (outcomes, expected_reads, success) in [
        (
            vec![ReadOutcome::Observed {
                elapsed_ms: SOURCE_READ_DEADLINE_MS - 1,
            }],
            1,
            true,
        ),
        (
            vec![
                ReadOutcome::Observed {
                    elapsed_ms: SOURCE_READ_DEADLINE_MS,
                },
                ReadOutcome::Timeout,
            ],
            2,
            false,
        ),
        (
            vec![
                ReadOutcome::Observed {
                    elapsed_ms: SOURCE_READ_DEADLINE_MS + 1,
                },
                ReadOutcome::Timeout,
            ],
            2,
            false,
        ),
        (vec![ReadOutcome::Timeout], 2, true),
        (vec![ReadOutcome::Unavailable], 1, false),
    ] {
        let t = Scratch::new();
        let mut input = seed();
        input.read_scripts = outcomes
            .into_iter()
            .map(|outcome| ReadScript {
                identity: identity(),
                outcome,
            })
            .collect();
        let mut state = t.open(input);
        assert_eq!(state.read(&identity(), NOW).is_ok(), success);
        assert_eq!(state.read_count(), expected_reads);
        assert!(expected_reads <= (MAX_SOURCE_READ_RETRIES + 1) as u64);
        let mut reopened = t.open(seed());
        assert!(reopened.read(&identity(), NOW).is_ok()); // Consumed failures survive restart.
    }
    let t = Scratch::new();
    let mut state = t.open(seed());
    let store = DraftStore::open(&t.root(), owner()).unwrap();
    let held = store.lock_account().unwrap();
    assert_eq!(
        state.read_with_account(&held, &identity(), NOW).unwrap(),
        seed().records[0]
    );
    let other = Scratch::new();
    let other_store = DraftStore::open(&other.root(), owner()).unwrap();
    let foreign = other_store.lock_account().unwrap();
    assert_eq!(
        error(state.read_with_account(&foreign, &identity(), NOW)),
        "record_source_root_mismatch"
    );
    for _ in 1..MAX_SOURCE_OPERATIONS {
        state.read_with_account(&held, &identity(), NOW).unwrap();
    }
    let bytes = fs::read(t.path()).unwrap();
    assert_eq!(
        error(state.dispatch_with_account(&held, &identity(), &progress_patch(), NOW)),
        "record_source_limit"
    );
    assert_eq!(state.dispatch_count(), 0);
    assert_eq!(fs::read(t.path()).unwrap(), bytes);
}
#[test]
fn version_script_and_writable_target_boundaries() {
    for member in ["format", "another version"] {
        let t = Scratch::new();
        t.open(seed());
        let mut value = serde_json::to_value(seed()).unwrap();
        if member == "format" {
            value.as_object_mut().unwrap().remove("format");
        } else {
            value["format"] = member.into();
        }
        fs::write(t.path(), serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            error(SyntheticRecordState::open(&t.root(), seed(), 10000)),
            "record_source_format_unsupported"
        );
    }
    for member in ["read_scripts", "effect_scripts", "writable_targets"] {
        for over in [0, 1] {
            let t = Scratch::new();
            let mut input = seed();
            for n in 0..MAX_SOURCE_SCRIPTS + over {
                match member {
                    "read_scripts" => input.read_scripts.push(ReadScript {
                        identity: identity(),
                        outcome: ReadOutcome::Unavailable,
                    }),
                    "effect_scripts" => input.effect_scripts.push(EffectScript {
                        identity: identity(),
                        field: RecordField::Progress,
                        outcome: EffectOutcome::Reject,
                        readback: None,
                    }),
                    _ => {
                        let mut id = identity();
                        id.lesson_date = (chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
                            + chrono::Duration::days(n as i64))
                        .to_string();
                        input.writable_targets.push(WritableTarget {
                            identity: id,
                            synthetic_target: format!("synthetic-{n}"),
                        });
                    }
                }
            }
            if member == "writable_targets" {
                input.writable_targets.remove(0);
            }
            assert_eq!(
                SyntheticRecordState::open(&t.root(), input, 10000).is_ok(),
                over == 0
            );
        }
    }
}
#[test]
fn partial_batch_creates_only_applied_fields_and_rejects_unbound_targets() {
    let t = Scratch::new();
    let mut input = seed();
    input.records.clear();
    input.effect_scripts.push(EffectScript {
        identity: identity(),
        field: RecordField::Homework,
        outcome: EffectOutcome::Reject,
        readback: None,
    });
    let mut state = t.open(input);
    let store = DraftStore::open(&t.root(), owner()).unwrap();
    let held = store.lock_account().unwrap();
    let mut patch = progress_patch();
    patch.progress.as_mut().unwrap().value.clear();
    patch.homework = Some(FieldPatch {
        value: "rejected synthetic homework".into(),
        ..patch.progress.clone().unwrap()
    });
    let result = state
        .dispatch_with_account(&held, &identity(), &patch, NOW)
        .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(state.dispatch_count(), 1);
    let observed = state.read_with_account(&held, &identity(), NOW).unwrap();
    let RecordObservation::Observed { occurrence, fields } = observed.observation else {
        panic!("created record required")
    };
    assert_eq!(occurrence, "synthetic-1001-20261109");
    assert_eq!(fields.progress, RecordValue::Known(String::new()));
    for field in RecordField::ALL
        .into_iter()
        .filter(|f| *f != RecordField::Progress)
    {
        assert_eq!(fields[field], RecordValue::Unknown);
    }
    let bytes = fs::read(t.path()).unwrap();
    patch.progress.as_mut().unwrap().effect_target = "unbound target".into();
    assert_eq!(
        error(state.dispatch_with_account(&held, &identity(), &patch, NOW)),
        "record_source_invalid"
    );
    let mut absent = identity();
    absent.lesson_date = "2026-11-11".into();
    assert_eq!(
        error(state.dispatch_with_account(&held, &absent, &progress_patch(), NOW)),
        "record_source_not_writable"
    );
    assert_eq!(state.dispatch_count(), 1);
    assert_eq!(fs::read(t.path()).unwrap(), bytes);
    let count = state.read_count();
    assert_eq!(
        state
            .read_with_account(&held, &absent, NOW)
            .unwrap()
            .observation,
        RecordObservation::NotObserved
    );
    assert_eq!(state.read_count(), count + 1); // Never retries a successful no-match.
}

#[test]
fn full_record_exact_date_and_owner_persistent_reads() {
    let t = Scratch::new();
    let expected = seed().records.remove(0);
    let mut two = seed();
    let mut second = changed_record();
    second.identity.student_key = "1003".into();
    two.records.push(second.clone());
    let mut state = t.open(two);
    assert_eq!(state.read(&identity(), NOW).unwrap(), expected);
    assert_eq!(state.read(&second.identity, NOW).unwrap(), second);
    let mut id = identity();
    for date in ["2026-11-05", "2026-11-11"] {
        id.lesson_date = date.into();
        let absent = state.read(&id, NOW).unwrap();
        assert_eq!(absent.requested_date, date);
        assert_eq!(absent.observation, RecordObservation::NotObserved);
        assert_eq!(absent.identity, id);
    }
    id = identity();
    id.student_key = "1002".into();
    assert_eq!(
        state.read(&id, NOW).unwrap().observation,
        RecordObservation::NotObserved
    );
    let count = state.read_count();
    for field in ["owner", "account"] {
        let mut wrong = identity();
        if field == "owner" {
            wrong.owner.owner.push('x');
        } else {
            wrong.owner.account.push('x');
        }
        assert_eq!(error(state.read(&wrong, NOW)), "record_source_invalid");
    }
    id.lesson_date = "2026-02-30".into();
    assert!(state.read(&id, NOW).is_err());
    assert!(state.read(&identity(), "invalid clock").is_err());
    assert_eq!(state.read_count(), count);
    let original = fs::read(t.path()).unwrap();
    let mut wrong = seed();
    wrong.owner.account.push('x');
    assert!(SyntheticRecordState::open(&t.root(), wrong, 10000).is_err());
    assert_eq!(fs::read(t.path()).unwrap(), original);
}
#[test]
fn synthetic_snapshot_reopens_without_reseeding_and_reloads_external_changes() {
    let t = Scratch::new();
    let mut first = t.open(seed());
    let mut changed = seed();
    changed.records[0] = changed_record();
    // A cooperating source writer publishes while holding the same account lock.
    let store = DraftStore::open(&t.root(), owner()).unwrap();
    {
        let _account = store.lock_account().unwrap();
        snapshot(&t.path(), &changed);
    }
    assert_eq!(first.read(&identity(), NOW).unwrap(), changed.records[0]);
    drop(first);
    let bytes = fs::read(t.path()).unwrap();
    let mut reopened = t.open(seed());
    assert_eq!(reopened.read(&identity(), NOW).unwrap(), changed.records[0]);
    assert_eq!(fs::read(t.path()).unwrap(), bytes);
    assert_eq!(
        fs::metadata(t.path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let later = "2026-11-11T18:11:00+09:00";
    let fresh = reopened.read(&identity(), later).unwrap();
    assert_eq!(fresh.provenance.read_at, later);
    assert_eq!(
        fresh.provenance.evidence_refs,
        changed.records[0].provenance.evidence_refs
    );
    assert_eq!(
        fresh.provenance.field_reasons,
        changed.records[0].provenance.field_reasons
    );
    assert_eq!(fresh.observation, changed.records[0].observation);
    assert_eq!(fs::read(t.path()).unwrap(), bytes); // Read timestamps never rewrite source.
}
#[test]
fn source_snapshot_byte_record_and_operation_boundaries() {
    let t = Scratch::new();
    let mut many = seed();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    many.records = (0..MAX_SOURCE_RECORDS)
        .map(|n| {
            let mut row = seed().records.remove(0);
            row.identity.lesson_date = (date + chrono::Duration::days(n as i64)).to_string();
            row.requested_date = row.identity.lesson_date.clone();
            row
        })
        .collect();
    let mut state = t.open(many.clone());
    for row in &many.records[..MAX_SOURCE_OPERATIONS] {
        assert_eq!(state.read(&row.identity, NOW).unwrap(), *row);
    }
    assert_eq!(state.source_operations(), MAX_SOURCE_OPERATIONS);
    assert_eq!(error(state.read(&identity(), NOW)), "record_source_limit");
    assert_eq!(state.read_count(), MAX_SOURCE_OPERATIONS as u64);
    state.begin_call();
    let last = state
        .read(&many.records[MAX_SOURCE_RECORDS - 1].identity, NOW)
        .unwrap();
    assert_eq!(last, many.records[MAX_SOURCE_RECORDS - 1]);
    assert_eq!(state.source_operations(), 1);
    assert_eq!(state.read_count(), MAX_SOURCE_OPERATIONS as u64 + 1);
    let extra = seed().records.remove(0);
    many.records.push(extra);
    let other = Scratch::new();
    assert_eq!(
        error(SyntheticRecordState::open(&other.root(), many, 10000)),
        "record_source_limit"
    );
    assert!(!other.path().exists());
    let mut bytes = serde_json::to_vec(&seed()).unwrap();
    bytes.resize(MAX_SOURCE_SNAPSHOT_BYTES, b' ');
    fs::write(t.path(), &bytes).unwrap();
    assert_eq!(state.read(&identity(), NOW).unwrap(), seed().records[0]);
    bytes.push(b' ');
    fs::write(t.path(), &bytes).unwrap();
    assert_eq!(error(state.read(&identity(), NOW)), "record_source_limit");
    assert_eq!(
        error(SyntheticRecordState::open(&t.root(), seed(), 10000)),
        "record_source_limit"
    );
    assert_eq!(
        fs::metadata(t.path()).unwrap().len(),
        (MAX_SOURCE_SNAPSHOT_BYTES + 1) as u64
    );
}
#[test]
fn corruption_absence_and_unattributed_success_remain_distinct() {
    let t = Scratch::new();
    let mut state = t.open(seed());
    for damage in [
        "json",
        "owner",
        "empty_foreign",
        "date",
        "duplicate",
        "seventh_field",
        "failed_record",
    ] {
        let mut value = serde_json::to_value(seed()).unwrap();
        match damage {
            "owner" => value["records"][0]["identity"]["owner"]["account"] = "other".into(),
            "empty_foreign" => {
                value["owner"]["account"] = "other".into();
                value["records"] = serde_json::json!([]);
            }
            "date" => value["records"][0]["requested_date"] = "2026-11-11".into(),
            "duplicate" => {
                let row = value["records"][0].clone();
                value["records"].as_array_mut().unwrap().push(row);
            }
            "seventh_field" => {
                value["records"][0]["observation"]["fields"]["invented"] =
                    serde_json::json!({"state":"unknown"})
            }
            "failed_record" => {
                value["records"][0]["observation"] =
                    serde_json::json!({"state":"failed","reason_code":"synthetic_timeout"})
            }
            _ => {}
        }
        let bytes = if damage == "json" {
            b"invalid JSON".to_vec()
        } else {
            serde_json::to_vec(&value).unwrap()
        };
        fs::write(t.path(), &bytes).unwrap();
        assert_eq!(error(state.read(&identity(), NOW)), "record_source_invalid");
        assert!(SyntheticRecordState::open(&t.root(), seed(), 10000).is_err());
        assert_eq!(fs::read(t.path()).unwrap(), bytes);
    }
    snapshot(&t.path(), &seed());
    let observed = state.read(&identity(), NOW).unwrap();
    if let RecordObservation::Observed { fields, .. } = observed.observation {
        assert_eq!(fields.lesson_memo, RecordValue::Known(String::new()));
        assert_eq!(fields.daily_test_assessment, RecordValue::Unknown);
        assert_eq!(
            observed
                .provenance
                .field_reasons
                .daily_test_assessment
                .as_deref(),
            Some("not_observed")
        );
    } else {
        panic!("successful unknowns must remain observed");
    }
    fs::remove_file(t.path()).unwrap();
    assert_eq!(
        error(state.read(&identity(), NOW)),
        "record_source_unavailable"
    );
    assert!(!t.path().exists());
}
#[test]
fn held_account_lock_times_out_then_allows_the_source_read() {
    let t = Scratch::new();
    let mut state = t.open(seed());
    let store = DraftStore::open(&t.root(), owner()).unwrap();
    let held = store.lock_account().unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        error(state.read(&identity(), NOW)),
        "shared_writer_lock_busy"
    );
    assert!(
        start.elapsed().as_millis() >= u128::from(edu_academy_teacher::record_store::LOCK_WAIT_MS)
    );
    drop(held);
    assert_eq!(state.read(&identity(), NOW).unwrap(), seed().records[0]);
}
#[test]
fn initial_capacity_exact_entries_and_root_bytes_and_stale_temp_recovery() {
    use edu_academy_teacher::record_store::MAX_ROOT_BYTES;
    for extra in [0, 1, 2] {
        let t = Scratch::new();
        let store = DraftStore::open(&t.root(), owner()).unwrap();
        let entries = store.lock_account().unwrap().usage(10000).unwrap().entries;
        let result = SyntheticRecordState::open(&t.root(), seed(), entries - 1 + extra);
        assert_eq!(result.is_ok(), extra == 2);
        assert_eq!(t.path().exists(), extra == 2);
    }
    for extra in [0, 1] {
        let t = Scratch::new();
        let store = DraftStore::open(&t.root(), owner()).unwrap();
        let used = store.lock_account().unwrap().usage(10000).unwrap().bytes;
        let filler = t.root().join("synthetic-capacity");
        let f = fs::File::create(&filler).unwrap();
        fs::set_permissions(&filler, fs::Permissions::from_mode(0o600)).unwrap();
        f.set_len(
            MAX_ROOT_BYTES - used - serde_json::to_vec(&seed()).unwrap().len() as u64 + extra,
        )
        .unwrap();
        let result = SyntheticRecordState::open(&t.root(), seed(), 10000);
        assert_eq!(result.is_ok(), extra == 0);
        assert_eq!(t.path().exists(), extra == 0);
    }
    let t = Scratch::new();
    DraftStore::open(&t.root(), owner()).unwrap();
    let temp = t
        .root()
        .join(format!(".record-source-{}.tmp", std::process::id()));
    fs::write(&temp, b"stale disposable bytes").unwrap();
    t.open(seed());
    assert!(!temp.exists());
    fs::remove_file(t.path()).unwrap();
    fs::create_dir(&temp).unwrap();
    let refused = SyntheticRecordState::open(&t.root(), seed(), 10000)
        .err()
        .unwrap();
    assert!(!refused.properties.get("writeOutcome").is("uncertain"));
    assert!(!t.path().exists());
}
#[test]
fn unsafe_snapshot_marker_root_and_initial_capacity_are_refused() {
    for damage in ["link", "hardlink", "permissions", "marker", "root"] {
        let t = Scratch::new();
        let mut state = t.open(seed());
        let outside = t.0.join("outside");
        match damage {
            "link" => {
                fs::rename(t.path(), &outside).unwrap();
                symlink(&outside, t.path()).unwrap();
            }
            "hardlink" => fs::hard_link(t.path(), &outside).unwrap(),
            "permissions" => {
                fs::set_permissions(t.path(), fs::Permissions::from_mode(0o644)).unwrap()
            }
            "marker" => fs::write(
                t.root().join(edu_academy_teacher::record_store::MARKER),
                b"invalid",
            )
            .unwrap(),
            "root" => {
                fs::rename(t.root(), &outside).unwrap();
                DraftStore::open(&t.root(), owner()).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(state.read(&identity(), NOW).is_err());
        if damage != "root" {
            assert!(SyntheticRecordState::open(&t.root(), seed(), 10000).is_err());
        }
    }
    let t = Scratch::new();
    assert_eq!(
        error(SyntheticRecordState::open(&t.root(), seed(), 0)),
        "record_store_capacity"
    );
    assert!(!t.path().exists());
    let unmarked = t.0.join("unmarked");
    fs::create_dir(&unmarked).unwrap();
    fs::write(unmarked.join(SOURCE_SNAPSHOT), b"unowned synthetic bytes").unwrap();
    assert!(SyntheticRecordState::open(&unmarked, seed(), 10000).is_err());
    assert_eq!(
        fs::read(unmarked.join(SOURCE_SNAPSHOT)).unwrap(),
        b"unowned synthetic bytes"
    );
}
