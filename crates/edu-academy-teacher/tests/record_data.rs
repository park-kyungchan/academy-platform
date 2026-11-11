use edu_academy_teacher::record_data::*;
use serde_json::{Value, json};
use std::num::NonZeroU64;

fn identity() -> RecordIdentity {
    RecordIdentity {
        owner: OwnerScope {
            owner: "synthetic-owner".into(),
            account: "synthetic-account".into(),
        },
        student_key: "99011".into(),
        lesson_date: "2026-11-11".into(),
        kind: RecordKind::DailyRecord,
    }
}
fn values() -> RecordFields<RecordValue> {
    RecordFields::from_fn(|_| RecordValue::Unknown)
}
fn patch() -> RecordPatch {
    let mut p = RecordFields::from_fn(|_| None);
    p.homework = Some(FieldPatch {
        effect_target: "synthetic:99011:2026-11-11".into(),
        operation: PatchOperation::Replace,
        value: "- 가우스 5-1-2, 2-2 가상주제F 유형다지기".into(),
    });
    p
}
fn binding(p: &RecordPatch, v: &RecordFields<RecordValue>) -> ContentBinding {
    ContentBinding::new(identity(), "synthetic-generation".into(), p, v).unwrap()
}
fn draft() -> Draft {
    let p = patch();
    let v = values();
    let b = binding(&p, &v);
    Draft {
        identity: identity(),
        generation: b.generation.clone(),
        revision: NonZeroU64::MIN,
        before: RecordSnapshot {
            identity: identity(),
            requested_date: "2026-11-11".into(),
            provenance: RecordProvenance {
                read_at: "2026-11-11T18:10:00+09:00".into(),
                evidence_refs: vec!["S-053".into()],
                source_revision: None,
                wire_digest: None,
                field_reasons: RecordFields::from_fn(|_| Some("not_observed".into())),
            },
            observation: RecordObservation::NotObserved,
        },
        binding: Some(b.clone()),
        next_attempt_ordinal: NonZeroU64::MIN,
        pending_attempt: Some(PendingAttempt {
            request_ref: "p8s3:synthetic-request".into(),
            ordinal: NonZeroU64::MIN,
            instruction_hash: "synthetic-instruction".into(),
            binding: b.clone(),
        }),
        failed_attempts: Some(FailedAttemptSummary {
            binding: b,
            definite_no_effect_batches: 1,
        }),
        editable: RecordFields::from_fn(|f| f == RecordField::Homework),
        dispositions: RecordFields::from_fn(|_| FieldDisposition::Unresolved),
        teacher_inputs: TeacherInputs {
            homework: Some(vec!["유형다지기로 바꿔".into()]),
            ..Default::default()
        },
        entries: vec![],
        patch: p,
        rendered_preview: "가상학생A 미리보기".into(),
        preview_hash: content_hash(&"가상학생A 미리보기").unwrap(),
        pending_questions: vec![PendingQuestion {
            field: AnswerField::Replace,
            reason_code: "replace_decision_required".into(),
            text: "교체할까요?".into(),
            choices: vec![QuestionChoice {
                label: "교체".into(),
                answer: AnswerValue::Boolean(true),
            }],
            revision: NonZeroU64::MIN,
        }],
        created: "2026-11-11T18:10:00+09:00".into(),
        last_used: "2026-11-11T18:10:00+09:00".into(),
        closed_at: None,
        state: DraftState::Open,
    }
}

#[test]
fn six_field_bound_and_source_names_are_exact() {
    assert_eq!(
        RECORD_FIELD_COUNT,
        edu_academy_sources::native::FIELDS.len()
    );
    assert_eq!(RECORD_FIELD_COUNT, edu_academy_core::decision::FIELDS.len());
    let mut fields = values();
    for (index, field) in RecordField::ALL.into_iter().enumerate() {
        assert_eq!(
            field.wire_name(),
            edu_academy_sources::native::FIELDS[index]
        );
        assert_eq!(serde_json::to_value(field).unwrap(), field.wire_name());
        fields[field] = RecordValue::Known(field.wire_name().into());
    }
    let encoded = serde_json::to_value(&fields).unwrap();
    assert_eq!(encoded.as_object().unwrap().len(), RECORD_FIELD_COUNT);
    for field in RecordField::ALL {
        assert_eq!(encoded[field.wire_name()]["value"], field.wire_name());
        let mut missing = encoded.clone();
        missing.as_object_mut().unwrap().remove(field.wire_name());
        assert!(serde_json::from_value::<RecordFields<RecordValue>>(missing).is_err());
    }
    let mut extra = encoded;
    extra["seventh"] = json!({"state":"unknown"});
    assert!(serde_json::from_value::<RecordFields<RecordValue>>(extra).is_err());
}

#[test]
fn known_blank_unknown_and_literal_source_text_stay_distinct() {
    let mut v = values();
    let unknown = content_hash(&v).unwrap();
    v.attendance = RecordValue::Known(String::new());
    assert_ne!(unknown, content_hash(&v).unwrap());
    v.lesson_memo = RecordValue::Known(" legacy | value\n<!-- raw --> 😀 ".into());
    let round: RecordFields<RecordValue> =
        serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
    assert_eq!(v, round);
    assert_eq!(round.homework, RecordValue::Unknown);
    v.attendance = RecordValue::Known(" ".into());
    assert_ne!(content_hash(&v).unwrap(), content_hash(&round).unwrap());
}

#[test]
fn absence_failure_and_successful_unattributed_values_are_distinct() {
    let mut s = draft().before;
    let absent = serde_json::to_value(&s).unwrap();
    s.observation = RecordObservation::Failed {
        reason_code: "synthetic_timeout".into(),
    };
    assert_ne!(absent, serde_json::to_value(&s).unwrap());
    s.observation = RecordObservation::Observed {
        occurrence: "S-019".into(),
        fields: values(),
    };
    s.provenance.field_reasons.homework = Some("student_ambiguous".into());
    let round: RecordSnapshot = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
    assert_eq!(round, s);
    assert_eq!(round.requested_date, "2026-11-11");
    assert_eq!(
        round.provenance.field_reasons.homework.as_deref(),
        Some("student_ambiguous")
    );
}

#[test]
fn identity_key_binds_owner_account_student_date_and_kind() {
    let id = identity();
    let key = id.key().unwrap();
    let mut variants = [id.clone(), id.clone(), id.clone(), id.clone()];
    variants[0].owner.owner.push('x');
    variants[1].owner.account.push('x');
    variants[2].student_key.push('x');
    variants[3].lesson_date = "2026-11-09".into();
    for other in variants {
        assert_ne!(key, other.key().unwrap());
    }
    assert_eq!(serde_json::to_value(id.kind).unwrap(), "daily_record");
    assert_eq!(key, content_hash(&id).unwrap());
}

#[test]
fn content_binding_covers_every_mask_target_operation_and_raw_value() {
    let p = patch();
    let v = values();
    let original = binding(&p, &v);
    for field in RecordField::ALL {
        let mut changed = p.clone();
        changed[field] = Some(FieldPatch {
            effect_target: "other-target".into(),
            operation: PatchOperation::Set,
            value: "".into(),
        });
        assert_ne!(original.patch_hash, binding(&changed, &v).patch_hash);
        let mut changed_source = v.clone();
        changed_source[field] = RecordValue::Known("".into());
        assert_ne!(
            original.source_hash,
            binding(&p, &changed_source).source_hash
        );
    }
    let mut changed = p.clone();
    changed.homework.as_mut().unwrap().effect_target.push('x');
    assert_ne!(original.patch_hash, binding(&changed, &v).patch_hash);
    changed = p.clone();
    changed.homework.as_mut().unwrap().operation = PatchOperation::Set;
    assert_ne!(original.patch_hash, binding(&changed, &v).patch_hash);
    changed = p.clone();
    changed.homework.as_mut().unwrap().value.push(' ');
    assert_ne!(original.patch_hash, binding(&changed, &v).patch_hash);
    assert_ne!(
        original,
        ContentBinding::new(identity(), "replacement".into(), &p, &v).unwrap()
    );
}

#[test]
fn hashes_use_complete_raw_values_and_the_existing_canonical_algorithm() {
    let mut v = values();
    v.lesson_memo = RecordValue::Known("가".repeat(300));
    let before = binding(&patch(), &v);
    if let RecordValue::Known(raw) = &mut v.lesson_memo {
        raw.push('끝');
    }
    assert_ne!(before.source_hash, binding(&patch(), &v).source_hash);
    let json = serde_json::to_vec(&v).unwrap();
    let js = edu_academy_core::workflow::codec::parse(&json).unwrap();
    assert_eq!(
        content_hash(&v).unwrap(),
        edu_academy_core::js::sha(&edu_academy_core::workflow::canonical(&js).unwrap())
    );
}

#[test]
fn durable_draft_roundtrip_preserves_intent_without_restoring_presentation() {
    let d = draft();
    let mut encoded = serde_json::to_value(&d).unwrap();
    assert_eq!(serde_json::from_value::<Draft>(encoded.clone()).unwrap(), d);
    for name in ["pending_approval", "process_id", "displayed_at"] {
        encoded[name] = json!("injected");
        assert!(serde_json::from_value::<Draft>(encoded.clone()).is_err());
        encoded.as_object_mut().unwrap().remove(name);
    }
    for name in ["revision", "next_attempt_ordinal"] {
        let mut zero = encoded.clone();
        zero[name] = json!(0);
        assert!(serde_json::from_value::<Draft>(zero).is_err());
    }
    encoded["pending_attempt"]["ordinal"] = json!(0);
    assert!(serde_json::from_value::<Draft>(encoded).is_err());
}

#[test]
fn disposition_memo_and_attempt_metadata_do_not_grant_content_approval() {
    let mut d = draft();
    let approved = d.binding.clone();
    d.revision = NonZeroU64::new(2).unwrap();
    d.next_attempt_ordinal = NonZeroU64::new(2).unwrap();
    d.pending_attempt.as_mut().unwrap().request_ref = "another-request".into();
    d.before.provenance.read_at = "2026-11-11T18:11:00+09:00".into();
    assert_eq!(approved, Some(binding(&d.patch, &values())));
    d.dispositions.daily_test_assessment = FieldDisposition::Deferred;
    assert_eq!(d.state, DraftState::Open);
    assert!(d.teacher_inputs.memo.is_none());
    d.teacher_inputs.memo = Some(String::new());
    let explicit_empty: Value = serde_json::to_value(&d.teacher_inputs).unwrap();
    assert_eq!(explicit_empty["memo"], "");
    assert_eq!(
        serde_json::to_value(TeacherInputs::default()).unwrap()["memo"],
        Value::Null
    );
}
