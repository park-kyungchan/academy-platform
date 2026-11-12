use edu_academy_core::{js::Js, workflow::codec};
use edu_academy_teacher::{record_data::*, record_fixture::*};
use std::{collections::BTreeSet, path::Path};
const FIXTURE: &str = include_str!("fixtures/record-answers.json");
fn js(value: &serde_json::Value) -> Js {
    codec::parse(value.to_string().as_bytes()).unwrap()
}
fn text(value: &Js) -> String {
    String::from_utf16(value.units()).unwrap()
}
fn owner() -> OwnerScope {
    OwnerScope {
        owner: "invented-teacher".into(),
        account: "invented-account".into(),
    }
}

#[test]
fn recorded_answer_shapes() {
    let fixture = RecordFixture::from_json(FIXTURE).unwrap();
    assert_eq!(fixture.scenario.students[0].name, "가상학생A");
    assert_eq!(fixture.seed.records.len(), 4);
    assert!(fixture.seed.read_scripts.is_empty());
    assert_eq!(fixture.seed.effect_scripts.len(), 6);
    assert!(!fixture.assumptions.is_empty());
    let blank = &fixture.records[1];
    let RecordObservation::Observed { fields, .. } = &blank.snapshot.observation else {
        panic!("observed required")
    };
    assert_eq!(fields.progress, RecordValue::Known("".into()));
    assert_eq!(fields.attendance, RecordValue::Known(" ".into()));
    assert_eq!(
        fields.daily_test_assessment,
        RecordValue::Known("-1".into())
    );
    assert_eq!(
        fields.previous_homework_assessment,
        RecordValue::Known("-1".into())
    );
    assert!(
        blank
            .field_evidence
            .previous_homework_assessment
            .get("target")
            .get("lessonDate")
            .is("2026-11-09")
    );
    let RecordObservation::Observed { fields, .. } = &fixture.records[2].snapshot.observation
    else {
        panic!("legacy required")
    };
    assert_eq!(
        fields.progress,
        RecordValue::Known("가상 진도 [복습]\n가상 두 번째 줄".into())
    );
    assert_eq!(
        fixture.records[0].snapshot.provenance.source_revision,
        blank.snapshot.provenance.source_revision
    );
    assert_eq!(
        fixture.records[0].snapshot.provenance.wire_digest,
        blank.snapshot.provenance.wire_digest
    );
    for answer in &fixture.records[4..] {
        assert!(matches!(
            answer.snapshot.observation,
            RecordObservation::Failed { .. }
        ));
        assert!(!answer.answer.get("failure").absent());
    }
    let value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let first = &value["answers"][0];
    let id: RecordIdentity = serde_json::from_value(first["identity"].clone()).unwrap();
    for field in ["studentId", "teacherId", "lessonDate"] {
        let mut capture = first["capture"].clone();
        capture["answer"]["structuredContent"]["fields"][0]["target"][field] = "other".into();
        assert!(decode_record_answer(&js(&capture), id.clone()).is_err());
    }
    let mut capture = first["capture"].clone();
    capture["answer"]["structuredContent"]["fields"][2]["before"] =
        serde_json::json!({"state":"unknown","reasonCode":"invented_unread"});
    let unknown = decode_record_answer(&js(&capture), id).unwrap();
    assert!(
        matches!(unknown.snapshot.observation,RecordObservation::Observed {fields,..} if fields.lesson_memo==RecordValue::Unknown)
    );
}

#[test]
fn invented_effect_scripts_use_existing_state() {
    use edu_academy_teacher::{record_backend::SyntheticRecordState, record_store::DraftStore};
    let fixture = RecordFixture::from_json(FIXTURE).unwrap();
    let root = std::env::temp_dir().join(format!(
        "record-fixture-b2-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut state = SyntheticRecordState::open(&root, fixture.seed.clone(), 10000).unwrap();
    let store = DraftStore::open(&root, fixture.seed.owner.clone()).unwrap();
    let id = &fixture.seed.records[0].identity;
    let at = &fixture.seed.records[0].provenance.read_at;
    {
        let held = store.lock_account().unwrap();
        let mut patch = RecordFields::from_fn(|_| None);
        patch.progress = Some(FieldPatch {
            effect_target: fixture.seed.writable_targets[0].synthetic_target.clone(),
            operation: PatchOperation::Set,
            value: "가상 수정 진도".into(),
        });
        state.dispatch_with_account(&held, id, &patch, at).unwrap();
        let snapshot = state.read_with_account(&held, id, at).unwrap();
        assert!(
            matches!(snapshot.observation,RecordObservation::Observed{fields,..} if fields.progress==RecordValue::Known("가상 수정 진도".into()))
        );
        assert_eq!(state.dispatch_count(), 1);
        assert_eq!(state.notification_count(), 0);
    }
    drop(state);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

// Every failure message is static: private captures must never reach test output.
#[test]
fn host_recorded_answers_replay() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../academy-context-20261111/native-answers-20261112");
    if !directory.exists() {
        eprintln!("host replay skipped: capture directory absent");
        return;
    }
    let mut entries: Vec<_> = std::fs::read_dir(&directory)
        .expect("host directory unreadable")
        .map(|e| e.expect("host entry unreadable").path())
        .collect();
    entries.sort();
    assert!(entries.len() == 31, "host capture inventory changed");
    let mut observed = 0;
    let mut failed = 0;
    let mut memberships = BTreeSet::new();
    let mut record_courses = BTreeSet::new();
    let mut catalog_course = None;
    let mut groups = BTreeSet::new();
    fn group_values(value: &Js, groups: &mut BTreeSet<String>) {
        for (key, v) in value.props() {
            if key == &edu_academy_core::js::text("groupName") && v.string() {
                groups.insert(text(v));
            }
            group_values(v, groups);
        }
        for v in value.array() {
            group_values(v, groups);
        }
    }
    let mut login_failed = false;
    let mut later_resolved = false;
    for path in entries {
        let bytes = std::fs::read(path).expect("host capture unreadable");
        let parsed = codec::parse(&bytes);
        assert!(parsed.is_ok(), "host JSON invalid");
        let capture = parsed.unwrap();
        let answer = capture.get("answer").get("structuredContent");
        let id = RecordIdentity {
            owner: owner(),
            student_key: if answer.get("scope").get("studentId").string() {
                text(answer.get("scope").get("studentId"))
            } else {
                "1001".into()
            },
            lesson_date: if answer.get("lessonDate").string() {
                text(answer.get("lessonDate"))
            } else {
                "2026-11-12".into()
            },
            kind: RecordKind::DailyRecord,
        };
        if capture.get("tool").is("academy_dailyrecord_get")
            || (capture.get("tool").is("academy_student_resolve")
                && answer.get("status").is("unavailable"))
        {
            let result = decode_record_answer(&capture, id.clone());
            assert!(result.is_ok(), "host answer decode failed");
            let record = result.unwrap();
            assert!(record.answer == *answer, "host metadata changed");
            match &record.snapshot.observation {
                RecordObservation::Observed { fields, .. } => {
                    observed += 1;
                    assert!(
                        record.snapshot.provenance.read_at
                            == text(answer.get("source").get("observedAt")),
                        "host observation time changed"
                    );
                    assert!(
                        record.snapshot.provenance.source_revision
                            == Some(text(answer.get("source").get("sourceRevision"))),
                        "host revision changed"
                    );
                    assert!(
                        record.snapshot.provenance.wire_digest
                            == Some(text(answer.get("source").get("wireDigest"))),
                        "host digest changed"
                    );
                    let mut evidence = vec![];
                    for field in RecordField::ALL {
                        let row = answer
                            .get("fields")
                            .array()
                            .iter()
                            .find(|r| r.get("field").is(field.wire_name()))
                            .expect("host field missing");
                        assert!(
                            fields[field]
                                == RecordValue::Known(text(row.get("before").get("value"))),
                            "host raw value changed"
                        );
                        assert!(
                            record.field_evidence[field] == *row,
                            "host target/provenance changed"
                        );
                        evidence.extend(
                            row.get("before")
                                .get("evidenceRefs")
                                .array()
                                .iter()
                                .map(text),
                        );
                        memberships.insert(text(row.get("target").get("membershipId")));
                        record_courses.insert(text(row.get("target").get("courseId")));
                    }
                    assert!(
                        record.snapshot.provenance.evidence_refs == evidence,
                        "host evidence changed"
                    );
                    let mut wrong = id;
                    wrong.student_key.push('x');
                    assert!(
                        decode_record_answer(&capture, wrong).is_err(),
                        "host identity mismatch admitted"
                    );
                }
                RecordObservation::Failed { reason_code } => {
                    failed += 1;
                    assert!(
                        answer.get("reasonCode").is(reason_code),
                        "host reason changed"
                    );
                    login_failed |= answer.get("failure").get("category").is("authentication");
                }
                RecordObservation::NotObserved => panic!("failed host read became absence"),
            }
            assert!(
                answer.get("academyWrites") == &Js::Num(0.)
                    && answer.get("parentSends") == &Js::Num(0.),
                "read side-effect metadata changed"
            );
            assert!(
                answer.get("automaticRetryAllowed") == &Js::Bool(false),
                "read retry policy changed"
            );
        } else {
            assert!(
                decode_record_answer(&capture, id).is_err(),
                "non-record answer treated as record"
            );
            group_values(answer, &mut groups);
            if capture
                .get("tool")
                .is("academy_textbook_section_results_get")
                && answer.get("courseKey").string()
            {
                catalog_course = Some(text(answer.get("courseKey")));
            }
            if capture.get("tool").is("academy_student_resolve")
                && answer.get("studentKey").string()
                && login_failed
            {
                later_resolved = true;
            }
        }
    }
    assert!(
        observed == 12 && failed == 6,
        "host read classification changed"
    );
    assert!(
        memberships.len() > 1 && groups.len() > 1,
        "host identity separation lost"
    );
    assert!(
        catalog_course.is_some_and(|c| !record_courses.contains(&c)),
        "record/catalog course conflated"
    );
    assert!(
        login_failed && later_resolved,
        "separate later login observation missing"
    );
}

#[test]
fn recorded_values_survive_core_source_projection() {
    use edu_academy_core::workflow::service::Source;
    use edu_academy_teacher::{
        record_backend::SyntheticRecordState, record_effect::SyntheticCoreSource,
    };
    use std::sync::{Arc, Mutex};
    let fixture = RecordFixture::from_json(FIXTURE).unwrap();
    let root = std::path::PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("fixture-source-{}", std::process::id()));
    let mut source = SyntheticCoreSource {
        state: Arc::new(Mutex::new(
            SyntheticRecordState::open(&root, fixture.seed, 10000).unwrap(),
        )),
        records: fixture.records,
    };
    for index in 0..4 {
        let record = source.records[index].clone();
        let result = source
            .read(record.answer.get("scope"), &Js::Null, &Js::Null)
            .unwrap();
        for field in RecordField::ALL {
            let actual = result
                .get("observation")
                .get("fields")
                .get(field.wire_name());
            let before = record.field_evidence[field].get("before");
            assert_eq!(actual.get("state"), before.get("state"));
            assert_eq!(actual.get("value"), before.get("value"));
        }
    }
    assert_eq!(source.state.lock().unwrap().read_count(), 4);
}

#[test]
fn native_shaped_metadata_is_shared_across_distinct_values() {
    let fixture = RecordFixture::from_json(FIXTURE).unwrap();
    for record in &fixture.records {
        let source = record.answer.get("source");
        if source.absent() {
            assert!(matches!(
                record.snapshot.observation,
                RecordObservation::Failed { .. }
            ));
            continue;
        }
        for key in ["sourceRevision", "wireDigest"] {
            let value = text(source.get(key));
            assert_eq!(value.len(), 64);
            assert!(
                value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
        }
    }
    let first = &fixture.records[0];
    let RecordObservation::Observed {
        fields: first_values,
        ..
    } = &first.snapshot.observation
    else {
        panic!("observed fixture required")
    };
    assert!(fixture.records.iter().any(|record| {
        record.snapshot.identity != first.snapshot.identity
            && matches!(&record.snapshot.observation, RecordObservation::Observed {fields, ..} if fields != first_values)
            && record.answer.get("source").get("sourceRevision") == first.answer.get("source").get("sourceRevision")
            && record.answer.get("source").get("wireDigest") == first.answer.get("source").get("wireDigest")
    }));
}

#[test]
fn writable_bindings_preserve_original_identity_admission() {
    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    value["writable_targets"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let fixture = RecordFixture::from_json(&value.to_string()).unwrap();
    assert_eq!(fixture.seed.writable_targets.len(), RECORD_FIELD_COUNT);
    for target in &fixture.seed.writable_targets {
        assert_eq!(target.identity, fixture.records[0].snapshot.identity);
        assert_eq!(
            target.synthetic_target,
            edu_academy_teacher::record_effect::canonical_effect_target(
                fixture.records[0].field_evidence[target.field].get("target")
            )
            .unwrap()
        );
    }
    let original = value.clone();
    let duplicate = value["writable_targets"][0].clone();
    value["writable_targets"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert!(RecordFixture::from_json(&value.to_string()).is_err());
    // Missing and ambiguous failed records cannot introduce writable absence.
    for date in ["2026-11-10", "2026-11-12"] {
        let mut value = original.clone();
        value["writable_targets"][0]["identity"]["lesson_date"] = date.into();
        assert_eq!(
            RecordFixture::from_json(&value.to_string())
                .err()
                .expect("binding refusal required")
                .message,
            "record_answer_invalid"
        );
    }
}
