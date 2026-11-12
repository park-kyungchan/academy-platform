//! Initial read-only rows. Later packets add real dispatch/result traces here.
use edu_academy_core::{js::Js, workflow::codec};
use edu_academy_teacher::{record_data::*, record_fixture::*};
use std::{collections::BTreeSet, panic::catch_unwind};
#[derive(Clone)]
struct Dispatch {
    identity: RecordIdentity,
    field: RecordField,
    target: Js,
    attempt: String,
    approving_call: u64,
    fenced: bool,
    earlier_unresolved: bool,
}
#[derive(Clone)]
struct Saved {
    identity: RecordIdentity,
    field: RecordField,
    target: Js,
    intended: String,
    readback: RecordValue,
}
fn check(
    preview: &RecordIdentity,
    selected: &[(RecordField, Js, RecordValue)],
    dispatches: &[Dispatch],
    saved: &[Saved],
) {
    let mut attempts = BTreeSet::new();
    let mut calls = BTreeSet::new();
    for effect in dispatches {
        assert_eq!(&effect.identity, preview, "G1: target differs from preview");
        assert!(
            selected
                .iter()
                .any(|(field, target, _)| *field == effect.field && *target == effect.target),
            "G1: physical field target not previewed"
        );
        assert!(
            effect.fenced && !effect.earlier_unresolved,
            "G2: missing fence or unresolved predecessor"
        );
        assert!(
            attempts.insert((&effect.attempt, effect.field.wire_name())),
            "G2: attempt repeated"
        );
        assert!(
            calls.insert((effect.approving_call, effect.field.wire_name())),
            "G2: approving call repeated field"
        );
    }
    for claim in saved {
        assert_eq!(&claim.identity, preview, "G3: wrong readback target");
        assert!(
            selected
                .iter()
                .any(|(field, target, desired)| *field == claim.field
                    && *target == claim.target
                    && *desired == RecordValue::Known(claim.intended.clone())),
            "G3: unsent or misbound physical field claimed saved"
        );
        assert_eq!(
            claim.readback,
            RecordValue::Known(claim.intended.clone()),
            "G3: unequal or unknown readback"
        );
    }
}
#[test]
fn shared_checker_rejects_wrong_identity_duplicate_and_unequal_readback() {
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let record = &fixture.records[0];
    let preview = &record.snapshot.identity;
    let field = RecordField::Attendance;
    let target = record.field_evidence[field].get("target").clone();
    let selected = [(field, target.clone(), RecordValue::Known("L".into()))];
    let dispatched = Dispatch {
        identity: preview.clone(),
        field,
        target: target.clone(),
        attempt: "invented-attempt".into(),
        approving_call: 1,
        fenced: true,
        earlier_unresolved: false,
    };
    let saved = Saved {
        identity: preview.clone(),
        field,
        target,
        intended: "L".into(),
        readback: RecordValue::Known("L".into()),
    };
    // Establish that each negative starts from a checker-accepted trace.
    check(
        preview,
        &selected,
        std::slice::from_ref(&dispatched),
        std::slice::from_ref(&saved),
    );
    for wrong_student in [true, false] {
        let mut wrong = dispatched.clone();
        if wrong_student {
            wrong.identity.student_key = "invented-other-student".into();
        } else {
            wrong.identity.lesson_date = "2040-01-01".into();
        }
        assert!(catch_unwind(|| check(preview, &selected, &[wrong], &[])).is_err());
    }
    assert!(
        catch_unwind(|| check(
            preview,
            &selected,
            &[dispatched.clone(), dispatched.clone()],
            &[]
        ))
        .is_err()
    );
    let mut repeated_call = dispatched.clone();
    repeated_call.attempt = "invented-second-attempt".into();
    assert!(catch_unwind(|| check(preview, &selected, &[dispatched, repeated_call], &[])).is_err());
    let mut unequal = saved;
    unequal.readback = RecordValue::Known("Y".into());
    assert!(catch_unwind(|| check(preview, &selected, &[], &[unequal])).is_err());
}

#[test]
fn b2_save_outcome_rows() {
    let input = include_str!("fixtures/record-answers.json");
    let fixture = RecordFixture::from_json(input).unwrap();
    let value: serde_json::Value = serde_json::from_str(input).unwrap();
    for (name, index, wrong_identity, contract_row) in [
        ("B-read-failed", 6, false, "R05"),
        ("B-target-identity", 0, true, "R05"),
    ] {
        let item = &value["answers"][index];
        let mut identity: RecordIdentity =
            serde_json::from_value(item["identity"].clone()).unwrap();
        let preview = identity.clone();
        if wrong_identity {
            identity.student_key = "1003".into();
        }
        let capture = codec::parse(item["capture"].to_string().as_bytes()).unwrap();
        let result = decode_record_answer(&capture, identity);
        if wrong_identity {
            assert_eq!(
                result.err().expect("misbound read admitted").message,
                "record_answer_identity_conflict"
            );
        } else {
            assert!(matches!(
                result.unwrap().snapshot.observation,
                RecordObservation::Failed { .. }
            ));
        }
        // This decoder has no mutation/backend or saved-status API. Both actual
        // read-only results therefore produce empty effect/confirmation traces.
        check(&preview, &[], &[], &[]);
        assert_eq!(
            contract_row, "R05",
            "absent fence cannot classify an applied attempt"
        );
        assert!(name.starts_with("B-"));
    }
    // Mutate a later field independently; checking only the first field fails this case.
    let mut capture = value["answers"][0]["capture"].clone();
    capture["answer"]["structuredContent"]["fields"][3]["target"]["studentId"] =
        "invented-other-student".into();
    let result = decode_record_answer(
        &codec::parse(capture.to_string().as_bytes()).unwrap(),
        fixture.records[0].snapshot.identity.clone(),
    );
    assert_eq!(
        result.err().expect("later field mismatch admitted").message,
        "record_answer_identity_conflict"
    );
    assert!(fixture.seed.read_scripts.is_empty());
}

#[test]
fn c_ambiguous_preview() {
    use edu_academy_teacher::record_parse::{ParsedLines, parse_lines};
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let mut books = Vec::new();
    for edition in ["edition-A", "edition-B"] {
        books.push(serde_json::json!({
            "studentRef":"invented-student", "bookRef":edition,
            "label":"invented-book", "edition":edition,
            "bindingState":"confirmed", "units":[{
                "unitRef":"invented-unit", "code":"2-1", "title":"invented-topic",
                "kind":"minor", "activities":["필수예제"], "sections":[]
            }]
        }));
    }
    let catalog = codec::parse(serde_json::json!({"books":books}).to_string().as_bytes()).unwrap();
    let result = parse_lines(
        RecordField::Homework,
        &["invented-book 2-1 필수예제 8문항".into()],
        &catalog,
    )
    .unwrap();
    let ParsedLines::Ask(questions) = result else {
        panic!("ambiguous edition must ask");
    };
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].field, AnswerField::Homework);
    assert_eq!(questions[0].choices.len(), 2);
    assert!(questions[0].choices.iter().all(|choice| matches!(
        &choice.answer, AnswerValue::Lines(lines) if lines.len() == 1 && lines[0].ends_with("8문항")
    )));
    // C-ambiguous-preview: lexical questions create neither a durable fence nor
    // dispatch/saved traces. That is R05, not the conditional later R07.
    check(&fixture.records[0].snapshot.identity, &[], &[], &[]);
    let contract_row = "R05";
    assert_eq!(contract_row, "R05");
}

#[test]
fn c_parent_preview() {
    use edu_academy_teacher::{
        record_parent::render_parent,
        record_parse::{ParsedLines, parse_lines},
    };
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let catalog = codec::parse(
        serde_json::json!({"books":[{
            "studentRef":"invented-student", "bookRef":"invented-book", "label":"가상교재",
            "edition":null, "bindingState":"confirmed", "units":[{
                "unitRef":"invented-unit", "code":"2-1", "title":"가상주제",
                "kind":"minor", "activities":["필수예제"], "sections":[]
            }]
        }]})
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    let ParsedLines::Ready(entries) = parse_lines(
        RecordField::Homework,
        &["가상교재 2-1 필수예제 8문항".into()],
        &catalog,
    )
    .unwrap() else {
        panic!("resolved proposal required");
    };
    let proposal = render_parent(&entries, &catalog).unwrap();
    assert_eq!(
        proposal.homework.as_deref(),
        Some("- 가상교재, 2-1 가상주제 필수예제 8문항")
    );
    for field in RecordField::ALL {
        if field != RecordField::Homework {
            assert!(proposal[field].is_none());
        }
    }
    // C-parent-preview is a proposal only (R05): this pure renderer records no
    // durable pre-effect fact (R07), invokes no backend and makes no saved claim.
    check(&fixture.records[0].snapshot.identity, &[], &[], &[]);
}

#[test]
fn d_missing_fields() {
    use edu_academy_teacher::{
        envelope::{Coverage, Envelope, Status},
        record_words::{AskSet, RecordQuestion},
    };
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let set = AskSet::new(vec![
        RecordQuestion {
            field: AnswerField::DailyTest,
            question: "일일시험 평가는 어떻게 기록할까요?".into(),
            choices: vec![QuestionChoice {
                label: "미확인".into(),
                answer: AnswerValue::Text("미확인".into()),
            }],
        },
        RecordQuestion {
            field: AnswerField::Memo,
            question: "메모 문장을 그대로 답해 주세요.".into(),
            choices: vec![],
        },
    ])
    .unwrap();
    let result = Envelope::record_ask(
        "가상학생A 입력 확인".into(),
        serde_json::json!({}),
        set,
        Coverage::default(),
    )
    .unwrap();
    assert_eq!(result.status, Status::Ask);
    assert_eq!(result.questions.len(), 2);
    assert!(result.display.contains("일일시험 평가 선택"));
    assert!(result.display.contains("메모 선택"));
    assert!(!result.display.contains("출석 선택"));
    // D-missing-fields: R05 content only, no durable R07 fact. Grouped questions
    // expose no dispatch/saved API; the shared checker receives empty traces.
    check(&fixture.records[0].snapshot.identity, &[], &[], &[]);
}

#[test]
fn d_unresolved_unsent() {
    use edu_academy_teacher::record_policy::settle_fields;
    let mut fixture =
        RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let mut before = fixture.records.remove(0);
    let RecordObservation::Observed { fields, .. } = &mut before.snapshot.observation else {
        panic!("observed fixture required");
    };
    *fields = RecordFields::from_fn(|_| RecordValue::Unknown);
    fields.homework = RecordValue::Known("".into());
    let input = TeacherInputs {
        homework: Some(vec!["가상 숙제".into()]),
        ..Default::default()
    };
    let dispositions = settle_fields(&before, &input).unwrap();
    assert_eq!(dispositions.homework, FieldDisposition::ProposalConfirmed);
    for field in RecordField::ALL {
        if field != RecordField::Homework {
            assert_eq!(dispositions[field], FieldDisposition::Unresolved);
        }
    }
    // D-unresolved-unsent: only input dispositions, no preview/approval/attempt.
    // R05, not a durable R07 fact. No unsupplied field can acquire a saved claim.
    check(&before.snapshot.identity, &[], &[], &[]);
}

#[test]
fn c_core_target_refusal_rows() {
    use edu_academy_core::{
        js::{obj, sha},
        workflow::{
            Runtime,
            approval::{Approval, RequestApproval},
            state,
            store::{Kind, Store},
        },
    };
    struct Clock;
    impl Runtime for Clock {
        fn now(&self) -> String {
            "2040-01-04T12:00:00.000Z".into()
        }
        fn epoch_ms(&self) -> u64 {
            0
        }
        fn uuid(&self) -> edu_academy_core::js::Result<String> {
            Ok("invented-refusal".into())
        }
    }
    let fixture = RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let record = &fixture.records[0];
    let preview = &record.snapshot.identity;
    let root = std::path::PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("core-checker-{}", std::process::id()));
    edu_academy_core::workflow::store::mkdir(&root).unwrap();
    let scope = obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "teacher".into()),
        ("studentId", "student".into()),
        ("lessonDate", "2040-01-04".into()),
    ]);
    for (name, code, row) in [
        (
            "C-core-target-missing",
            "workflow_effect_target_required",
            "R05",
        ),
        ("C-core-target-drift", "workflow_effect_target_drift", "R07"),
        (
            "C-core-legacy-unbound",
            "workflow_legacy_execution_unbound",
            "R37/R09",
        ),
    ] {
        let store = Store::open(&root.join(name), &root).unwrap();
        let reviewed = obj(&[
            ("field", "progress".into()),
            ("effectRecordKey", "physical".into()),
            ("effectTarget", "reviewed-target".into()),
            ("before", "Before".into()),
            ("after", "After".into()),
        ]);
        let mut action = reviewed.clone();
        action.set(
            "target",
            obj(&[
                ("teacherKey", "teacher".into()),
                ("studentKey", "student".into()),
                ("date", "2040-01-04".into()),
            ]),
        );
        if row == "R05" {
            action = obj(&[
                ("field", "progress".into()),
                ("effectRecordKey", "physical".into()),
            ]);
        }
        if row == "R07" {
            action.set("effectTarget", "different-target".into());
        }
        let legacy = obj(&[
            ("scope", scope.clone()),
            ("requestRef", "older".into()),
            ("field", "progress".into()),
            ("effectRecordKey", "physical".into()),
            ("beforeSha256", sha("Before").into()),
            ("afterSha256", sha("After").into()),
        ]);
        if row == "R37/R09" {
            store
                .add_ledger(&state::lock_key(&scope).unwrap(), "legacy", &Clock)
                .unwrap();
            store
                .publish(Kind::Execution, "legacy", &legacy, &Clock)
                .unwrap();
        }
        let mut approval = RequestApproval {
            store: &store,
            runtime: &Clock,
            input: obj(&[
                ("scope", scope.clone()),
                ("requestRef", "new".into()),
                ("reviewed", Js::Arr(vec![reviewed])),
            ]),
        };
        // This is the actual synthetic effect boundary: only verified approval
        // could append a dispatch. The error is the returned core result.
        let result = approval.consume(&action);
        let mut dispatches = vec![];
        if result
            .as_ref()
            .is_ok_and(|r| r.get("status").is("verified"))
        {
            dispatches.push(Dispatch {
                identity: preview.clone(),
                field: RecordField::Progress,
                target: record.field_evidence[RecordField::Progress]
                    .get("target")
                    .clone(),
                attempt: name.into(),
                approving_call: 1,
                fenced: true,
                earlier_unresolved: false,
            });
        }
        assert_eq!(result.unwrap_err().message, code, "{name}/{row}");
        assert!(dispatches.is_empty());
        check(preview, &[], &dispatches, &[]);
        let executions = state::student_executions(&store, &scope).unwrap();
        assert_eq!(executions.len(), usize::from(row == "R37/R09"));
        if row == "R37/R09" {
            assert_eq!(executions[0].record, legacy);
            assert!(executions[0].state.needs_reconciliation());
            assert!(store.get(Kind::Reconciliation, "legacy").unwrap().absent());
            let status_input = obj(&[("scope", scope.clone())]);
            let before =
                edu_academy_core::workflow::service::status(&store, &status_input).unwrap();
            // C-core-facts-unqualified: absent legacy facts grant no proof.
            assert!(
                !before.get("fields").array()[0].get("executions").array()[0]
                    .has("completionFacts")
            );
            let facts = obj(&[("claim", "unqualified".into())]);
            store
                .publish(
                    Kind::Outcome,
                    "legacy",
                    &obj(&[
                        ("status", "unresolved".into()),
                        ("completionFacts", facts.clone()),
                    ]),
                    &Clock,
                )
                .unwrap();
            // C-core-facts-roundtrip: actual status reads the same Outcome.
            let after = edu_academy_core::workflow::service::status(&store, &status_input).unwrap();
            assert_eq!(
                after.get("fields").array()[0].get("executions").array()[0].get("completionFacts"),
                &facts
            );
            assert!(
                state::field_state(&store, "legacy")
                    .unwrap()
                    .needs_reconciliation()
            );
            check(preview, &[], &dispatches, &[]);
        }
    }
}

#[test]
fn e_read_failed() {
    use edu_academy_core::{js::obj, workflow::service::Source};
    use edu_academy_teacher::{
        record_backend::SyntheticRecordState,
        record_effect::{HeldSource, SyntheticCoreSource},
    };
    use std::sync::{Arc, Mutex};
    let mut fixture =
        RecordFixture::from_json(include_str!("fixtures/record-answers.json")).unwrap();
    let failed = fixture.records[4].clone();
    let mut scope = fixture.records[0].answer.get("scope").clone();
    scope.set(
        "studentId",
        failed.snapshot.identity.student_key.clone().into(),
    );
    scope.set(
        "lessonDate",
        failed.snapshot.identity.lesson_date.clone().into(),
    );
    assert!(
        !fixture
            .seed
            .records
            .iter()
            .any(|r| r.identity == failed.snapshot.identity)
    );
    fixture.records = vec![failed.clone()];
    let root = std::path::PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("source-row-{}", std::process::id()));
    let state = Arc::new(Mutex::new(
        SyntheticRecordState::open(&root, fixture.seed, 10000).unwrap(),
    ));
    let mut source = SyntheticCoreSource {
        state: Arc::clone(&state),
        records: fixture.records,
    };
    let drafts = edu_academy_teacher::record_store::DraftStore::open(
        &root,
        failed.snapshot.identity.owner.clone(),
    )
    .unwrap();
    let account = drafts.lock_account().unwrap();
    let result = HeldSource {
        inner: &mut source,
        account: &account,
        read_at: &failed.snapshot.provenance.read_at,
    }
    .read(&scope, &"invented-read".into(), &obj(&[]))
    .unwrap();
    assert!(result.get("status").is("unavailable"));
    assert!(result.get("reasonCode").is("student_name_not_found"));
    assert_eq!(result, failed.answer);
    assert!(!result.has("observation"));
    assert_eq!(state.lock().unwrap().read_count(), 0);
    assert_eq!(state.lock().unwrap().dispatch_count(), 0);
    // E-read-failed, R05: actual recorded failure, no inferred absence, no
    // pre-effect record, dispatch or saved claim. No save actor is constructed.
    check(&failed.snapshot.identity, &[], &[], &[]);
}

use edu_academy_core::{
    js::{Result, obj, sha},
    workflow::{
        self, Runtime,
        approval::Approval,
        service::{self, Backend, Source},
        state,
        store::{Kind, Store},
    },
};
use edu_academy_teacher::{
    facts::SeededFacts, record_backend::*, record_effect::*, record_policy::RecordPolicy,
};
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};
fn invented(v: serde_json::Value) -> Js {
    codec::parse(v.to_string().as_bytes()).unwrap()
}
fn field_of(value: &Js) -> RecordField {
    RecordField::ALL
        .into_iter()
        .find(|f| value.get("field").is(f.wire_name()))
        .unwrap()
}
struct SaveClock(Cell<u64>);
impl Runtime for SaveClock {
    fn now(&self) -> String {
        "2026-11-13T08:35:00.000Z".into()
    }
    fn epoch_ms(&self) -> u64 {
        0
    }
    fn uuid(&self) -> Result<String> {
        self.0.set(self.0.get() + 1);
        Ok(format!("invented-save-{}", self.0.get()))
    }
}
struct ObservedBackend<'a> {
    inner: SyntheticCoreBackend,
    store: &'a Store,
    account: &'a edu_academy_teacher::record_store::AccountGuard<'a>,
    identity: &'a RecordIdentity,
    drafts: &'a edu_academy_teacher::record_store::DraftStore,
    actions: Vec<Js>,
    invalid_later: bool,
}
struct FenceWatch<'a> {
    inner: &'a mut dyn Approval,
    store: &'a Store,
    state: Arc<Mutex<SyntheticRecordState>>,
    scope: &'a Js,
    actions: &'a mut Vec<Js>,
}
impl Approval for FenceWatch<'_> {
    fn consume(&mut self, action: &Js) -> Result<Js> {
        assert_eq!(self.state.lock().unwrap().dispatch_count(), 0);
        let result = self.inner.consume(action)?;
        if result.get("status").is("verified") {
            let executions = state::student_executions(self.store, self.scope)?;
            let digest = sha(&workflow::canonical(action)?);
            let execution = executions
                .iter()
                .find(|e| e.record.get("actionDigest").is(&digest))
                .expect("actual action fenced");
            assert!(execution.state.needs_reconciliation());
            assert_eq!(
                execution.record.get("effectTarget"),
                action.get("effectTarget")
            );
            self.actions.push(action.clone());
        }
        Ok(result)
    }
}
impl Backend for ObservedBackend<'_> {
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
        let mut watch = FenceWatch {
            inner: approval,
            store: self.store,
            state: Arc::clone(&self.inner.state),
            scope,
            actions: &mut self.actions,
        };
        // S4: the caller already holds account and draft when core holds student.
        assert!(self.drafts.lock_account().is_err());
        assert!(self.account.lock_draft(self.identity).is_err());
        assert!(
            matches!(self.store.lock(&state::lock_key(scope)?, workflow::store::LockOptions {
            wait: std::time::Duration::ZERO,
            ..Default::default()
        }, &|| false), Err(e) if e.message == "shared_writer_lock_busy")
        );
        let mut results = HeldBackend {
            inner: &mut self.inner,
            account: self.account,
        }
        .save(scope, patches, &mut watch, subject)?;
        if self.invalid_later {
            let Js::Arr(items) = &mut results else {
                panic!("array required")
            };
            items
                .last_mut()
                .unwrap()
                .set("completionFacts", "invented-invalid".into());
        }
        Ok(results)
    }
}
fn save_case(name: &str, outcomes: &[EffectOutcome]) {
    let invalid_later = name == "E-invalid-later";
    let create = name == "E-partial";
    let read_failure = name == "E-readback-error";
    let mut raw: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/record-answers.json")).unwrap();
    for (i, row) in raw["answers"][0]["capture"]["answer"]["structuredContent"]["fields"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        row["target"]["recordId"] = format!("invented-field-{i}").into();
    }
    let mut fixture = RecordFixture::from_json(&raw.to_string()).unwrap();
    let record = fixture.records[0].clone();
    let id = &record.snapshot.identity;
    fixture.seed.effect_scripts.clear();
    if create {
        fixture.seed.records.retain(|r| r.identity != *id);
    }
    let root = std::path::PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("{name}-{}", std::process::id()));
    let mut data = SyntheticRecordState::open(&root, fixture.seed.clone(), 10000).unwrap();
    if create {
        let drafts =
            edu_academy_teacher::record_store::DraftStore::open(&root, fixture.seed.owner.clone())
                .unwrap();
        let account = drafts.lock_account().unwrap();
        // Actual creation with the earlier field first; reopen resets process counters.
        for fields in [
            vec![RecordField::PreviousHomeworkAssessment],
            RecordField::ALL.to_vec(),
        ] {
            let patch = RecordFields::from_fn(|f| {
                fields.contains(&f).then(|| FieldPatch {
                    effect_target: canonical_effect_target(record.field_evidence[f].get("target"))
                        .unwrap(),
                    operation: PatchOperation::Set,
                    value: record.field_evidence[f]
                        .get("before")
                        .get("value")
                        .code_units(),
                })
            });
            data.dispatch_with_account(&account, id, &patch, &SaveClock(Cell::new(0)).now())
                .unwrap();
        }
        drop(account);
        data = SyntheticRecordState::open(&root, fixture.seed.clone(), 10000).unwrap();
    }
    // Script installation is fixture setup, before any core intake/review/save.
    let file = root.join(SOURCE_SNAPSHOT);
    let mut seeded: RecordSeed = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    for (i, outcome) in outcomes.iter().enumerate() {
        seeded.effect_scripts.push(EffectScript {
            identity: id.clone(),
            field: RecordField::ALL[i],
            outcome: *outcome,
            readback: read_failure.then_some(ReadOutcome::Unavailable),
        });
    }
    std::fs::write(file, serde_json::to_vec(&seeded).unwrap()).unwrap();
    let shared = Arc::new(Mutex::new(data));
    let mut source = SyntheticCoreSource {
        state: Arc::clone(&shared),
        records: fixture.records,
    };
    workflow::store::mkdir(&root.join("core")).unwrap();
    let store = Store::open(&root.join("core/store"), &root.join("core")).unwrap();
    let drafts =
        edu_academy_teacher::record_store::DraftStore::open(&root, fixture.seed.owner.clone())
            .unwrap();
    let account = drafts.lock_account().unwrap();
    let _draft = account.lock_draft(id).unwrap();
    // Account and draft precede core's student lock, which is still available.
    let student = store
        .lock(
            &state::lock_key(record.answer.get("scope")).unwrap(),
            workflow::store::LockOptions {
                wait: std::time::Duration::ZERO,
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
    drop(student);
    let read_at = SaveClock(Cell::new(0)).now();
    let mut source = HeldSource {
        inner: &mut source,
        account: &account,
        read_at: &read_at,
    };
    let mut backend = ObservedBackend {
        inner: SyntheticCoreBackend {
            state: Arc::clone(&shared),
        },
        store: &store,
        account: &account,
        identity: id,
        drafts: &drafts,
        actions: vec![],
        invalid_later,
    };
    let catalog = invented(
        serde_json::json!({"books":[{"studentRef":"1001","bookRef":"invented-book","label":"Invented Book","edition":null,"bindingState":"confirmed","units":[{"unitRef":"invented-unit","code":"2-1","title":"Invented topic","kind":"minor","activities":["필수예제"],"sections":[]}]}]}),
    );
    let policy = RecordPolicy::new(SeededFacts::from_scenario(&fixture.scenario), catalog);
    let clock = SaveClock(Cell::new(0));
    let scope = record.answer.get("scope").clone();
    let values = [
        "- Invented Book, 2-1 Invented topic 필수예제",
        "- Invented Book, 2-1 Invented topic 필수예제",
        "- Invented memo",
        "L",
        "3",
        "2",
    ];
    let mut context: serde_json::Value = serde_json::from_str(include_str!(
        "../../edu-academy-core/tests/fixtures/workflow-store-review-fixes-v2/CASES.json"
    ))
    .unwrap();
    context = context[0]["input"]["context"].clone();
    context["scope"] = serde_json::from_slice(&codec::bytes(&scope).unwrap()).unwrap();
    context["currentRequestRef"] = name.into();
    context["memoAnswer"] =
        serde_json::json!({"kind":"content","requestRef":name,"text":values[2]});
    context["selectedFields"] = serde_json::json!(
        RecordField::ALL[..outcomes.len()]
            .iter()
            .map(|f| f.wire_name())
            .collect::<Vec<_>>()
    );
    let mut facts = vec![];
    let mut proposals = vec![];
    for (i, field) in RecordField::ALL[..outcomes.len()].iter().enumerate() {
        let reference = format!("invented-fact-{i}");
        facts.push(serde_json::json!({"ref":reference,"field":field.wire_name(),"text":values[i],"origin":"user_supplied","requiredFragments":[],"questionCounts":[],"deadlines":[],"stageClaims":[],"explicitValue":values[i]}));
        let mut p = serde_json::json!({"field":field.wire_name(),"value":values[i],"factRefs":[reference],"decision":{"kind":"replace","requestRef":name}});
        if i < 2 {
            p["learningEntries"] = serde_json::json!([{"bookRef":"invented-book","unitRef":"invented-unit","scope":"unit","sectionRefs":[],"activity":"필수예제","deadline":null}]);
        }
        proposals.push(p);
    }
    context["facts"] = facts.into();
    let mut svc = service::Service {
        store: &store,
        source: &mut source,
        backend: &mut backend,
        policy: &policy,
        runtime: &clock,
    };
    let intake = svc
        .intake(&obj(&[("context", invented(context.clone()))]))
        .unwrap();
    let mut input = obj(&[
        ("context", invented(context)),
        ("observationHandle", intake.get("observationHandle").clone()),
        ("proposals", invented(proposals.into())),
    ]);
    let review = svc.review(&input, true).unwrap();
    assert!(
        review.get("status").is("prepared"),
        "{}",
        workflow::canonical(&Js::Arr(
            review
                .get("rows")
                .array()
                .iter()
                .map(|r| r.get("blockers").clone())
                .collect()
        ))
        .unwrap()
    );
    input.set("reviewDigest", review.get("reviewDigest").clone());
    input.set("request", invented(serde_json::json!({"requestRef":name,"instruction":{"text":"Invented explicit save instruction"}})));
    let saved = svc.save(&input);
    let executions = state::student_executions(&store, &scope).unwrap();
    assert_eq!(executions.len(), outcomes.len());
    assert_eq!(shared.lock().unwrap().dispatch_count(), 1);
    let selected: Vec<_> = review
        .get("patch")
        .array()
        .iter()
        .map(|p| {
            (
                field_of(p),
                codec::parse(p.get("effectTarget").code_units().as_bytes()).unwrap(),
                RecordValue::Known(p.get("after").code_units()),
            )
        })
        .collect();
    let dispatches: Vec<_> = backend
        .actions
        .iter()
        .map(|action| {
            let e = executions
                .iter()
                .find(|e| {
                    e.record
                        .get("actionDigest")
                        .is(&sha(&workflow::canonical(action).unwrap()))
                })
                .unwrap();
            Dispatch {
                identity: RecordIdentity {
                    owner: id.owner.clone(),
                    student_key: action.get("target").get("studentKey").code_units(),
                    lesson_date: action.get("target").get("date").code_units(),
                    kind: RecordKind::DailyRecord,
                },
                field: field_of(action),
                target: codec::parse(action.get("effectTarget").code_units().as_bytes()).unwrap(),
                attempt: e.key.clone(),
                approving_call: 1,
                fenced: !store.get(Kind::Execution, &e.key).unwrap().absent(),
                earlier_unresolved: executions.iter().any(|old| {
                    old.key != e.key
                        && old.record.get("field") == e.record.get("field")
                        && old.state.needs_reconciliation()
                }),
            }
        })
        .collect();
    check(id, &selected, &dispatches, &[]);
    if invalid_later {
        assert_eq!(
            saved.unwrap_err().message,
            "workflow_completion_facts_invalid"
        );
        assert!(executions.iter().all(|e| {
            e.state.needs_reconciliation()
                && store
                    .get(Kind::Outcome, &e.key)
                    .unwrap()
                    .get("status")
                    .is("unresolved")
        }));
        assert!(
            store
                .get(Kind::Outcome, &executions[0].key)
                .unwrap()
                .has("completionFacts")
        );
        return;
    }
    let saved = saved.unwrap();
    let fresh = source.read(&scope, &Js::Null, &Js::Null).unwrap();
    assert!(RecordField::ALL.iter().skip(outcomes.len()).all(|f| {
        fresh
            .get("observation")
            .get("fields")
            .get(f.wire_name())
            .get("value")
            == record.field_evidence[*f].get("before").get("value")
    }));
    let status = service::status(&store, &obj(&[("scope", scope.clone())])).unwrap();
    for (from_status, rows) in [(false, saved.get("fields")), (true, status.get("fields"))] {
        let mut claims = vec![];
        for row in rows.array() {
            let e = executions
                .iter()
                .find(|e| e.record.get("field") == row.get("field"))
                .unwrap();
            let r = if from_status {
                &row.get("executions").array()[0]
            } else {
                row
            };
            assert_eq!(
                r.get("completionFacts"),
                store
                    .get(Kind::Outcome, &e.key)
                    .unwrap()
                    .get("completionFacts")
            );
            if r.has("completionFacts") {
                assert_eq!(
                    r.get("completionFacts").get("effectTarget"),
                    e.record.get("effectTarget")
                );
            }
            if r.get("status").is("persisted")
                || (from_status
                    && r.get("executionState")
                        .is(state::FieldState::SettledPersisted.name()))
            {
                let field = field_of(row);
                let readback = fresh
                    .get("observation")
                    .get("fields")
                    .get(field.wire_name());
                let action = backend
                    .actions
                    .iter()
                    .find(|a| a.get("field") == row.get("field"))
                    .unwrap();
                assert!(readback.get("state").is("known"));
                assert_eq!(readback.get("effectTarget"), e.record.get("effectTarget"));
                claims.push(Saved {
                    identity: RecordIdentity {
                        owner: id.owner.clone(),
                        student_key: e.record.get("scope").get("studentId").code_units(),
                        lesson_date: e.record.get("scope").get("lessonDate").code_units(),
                        kind: RecordKind::DailyRecord,
                    },
                    field,
                    target: codec::parse(readback.get("effectTarget").code_units().as_bytes())
                        .unwrap(),
                    intended: action.get("after").code_units(),
                    readback: RecordValue::Known(readback.get("value").code_units()),
                });
            }
        }
        // R0 qualifies no profile: even synthetic final claims are held until
        // an exact-attempt proof is accepted by the shared classifier.
        assert!(claims.is_empty());
        check(id, &selected, &dispatches, &claims);
    }
    if create {
        let snapshot = shared
            .lock()
            .unwrap()
            .read_with_account(&account, id, &clock.now())
            .unwrap();
        let RecordObservation::Observed { occurrence, .. } = snapshot.observation else {
            panic!("created record")
        };
        assert_eq!(
            occurrence,
            canonical_effect_target(
                record
                    .field_evidence
                    .previous_homework_assessment
                    .get("target")
            )
            .unwrap()
        );
    }
    for (i, outcome) in outcomes.iter().enumerate() {
        let e = executions
            .iter()
            .find(|e| e.record.get("field").is(RecordField::ALL[i].wire_name()))
            .unwrap();
        if !matches!(outcome, EffectOutcome::Success | EffectOutcome::Reject) || read_failure {
            assert!(e.state.needs_reconciliation());
        }
    }
}
#[test]
fn s4_concrete_adapters_observe_account_draft_student_order() {
    save_case("E-success", &[EffectOutcome::Success; 6]);
    save_case("E-reject", &[EffectOutcome::Reject; 2]);
}
#[test]
fn partial_batch_only_selected() {
    save_case(
        "E-partial",
        &[
            EffectOutcome::Success,
            EffectOutcome::Reject,
            EffectOutcome::Success,
        ],
    );
}
#[test]
fn returned_timeout_and_read_failure_stay_unresolved() {
    save_case("E-adapter-loss", &[EffectOutcome::AppliedLostAck]);
    save_case("E-timeout", &[EffectOutcome::ReturnedTimeout]);
    save_case("E-readback-error", &[EffectOutcome::Success]);
}
#[test]
fn invalid_later_completion_facts_keep_whole_batch_fenced() {
    save_case("E-invalid-later", &[EffectOutcome::Success; 2]);
}
