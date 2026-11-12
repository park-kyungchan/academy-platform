//! Owned recorded-answer metadata with current synthetic values; no native transport.
use crate::{record_backend::SyntheticRecordState, record_data::*, record_fixture::AnswerRecord};
use edu_academy_core::{
    js::{Fault, Js, Result, obj},
    workflow::{
        canonical, same,
        service::{Source, qualified_observation},
    },
};
use std::sync::{Arc, Mutex};

/// Canonical full owner target. A physical record id alone is never the binding.
pub fn canonical_effect_target(target: &Js) -> Result<String> {
    canonical(target)
}

pub struct SyntheticCoreSource {
    pub state: Arc<Mutex<SyntheticRecordState>>,
    pub records: Vec<AnswerRecord>,
}
impl Source for SyntheticCoreSource {
    fn read(&mut self, scope: &Js, _: &Js, subject: &Js) -> Result<Js> {
        let mut matching = self.records.iter().filter(|r| {
            scope.get("studentId").is(&r.snapshot.identity.student_key)
                && scope.get("lessonDate").is(&r.snapshot.identity.lesson_date)
        });
        let record = matching
            .next()
            .ok_or_else(|| Fault::error("record_source_record_required"))?;
        if matching.next().is_some() {
            return Err(Fault::error("record_source_record_ambiguous"));
        }
        let unavailable = || Fault::error("record_source_unavailable");
        // Release the mutex before acquiring account; the existing account ->
        // state order is also used by callers holding the same owner account.
        let store = self
            .state
            .lock()
            .map_err(|_| unavailable())?
            .source_store(&record.snapshot.identity)?;
        if matches!(
            record.snapshot.observation,
            RecordObservation::Failed { .. }
        ) {
            return Ok(record.answer.clone());
        }
        if !same(record.answer.get("scope"), scope)? {
            return Err(Fault::error("source_scope_conflict"));
        }
        let account = store.lock_account()?;
        let current = self
            .state
            .lock()
            .map_err(|_| unavailable())?
            .read_once_with_account(
                &account,
                &record.snapshot.identity,
                &record.snapshot.provenance.read_at,
            )?;
        let RecordObservation::Observed { fields: values, .. } = current.observation else {
            return Err(Fault::error("record_source_not_observed"));
        };
        let mut fields = obj(&[]);
        for field in RecordField::ALL {
            let evidence = &record.field_evidence[field];
            let target = evidence.get("target");
            let mut value = match &values[field] {
                RecordValue::Known(value) => obj(&[
                    ("state", "known".into()),
                    ("value", value.clone().into()),
                    (
                        "evidenceRefs",
                        evidence.get("before").get("evidenceRefs").clone(),
                    ),
                ]),
                RecordValue::Unknown => obj(&[
                    ("state", "unknown".into()),
                    (
                        "reasonCode",
                        current.provenance.field_reasons[field]
                            .as_deref()
                            .unwrap_or("record_source_field_unknown")
                            .into(),
                    ),
                ]),
            };
            value.set("effectTarget", canonical_effect_target(target)?.into());
            value.set("effectRecordKey", target.get("recordId").clone());
            fields.set(field.wire_name(), value);
        }
        let source = record.answer.get("source");
        let observation = obj(&[
            ("scope", scope.clone()),
            (
                "target",
                record.field_evidence.progress.get("target").clone(),
            ),
            (
                "previousTarget",
                record
                    .field_evidence
                    .previous_homework_assessment
                    .get("target")
                    .clone(),
            ),
            (
                "observedAt",
                record.snapshot.provenance.read_at.clone().into(),
            ),
            ("sourceRevision", source.get("sourceRevision").clone()),
            ("wireDigest", source.get("wireDigest").clone()),
            ("fields", fields),
            ("toneSamples", Js::Arr(vec![])),
            ("transmissionState", source.get("transmissionState").clone()),
        ]);
        Ok(obj(&[
            ("status", "observed".into()),
            ("observation", qualified_observation(&observation, scope)?),
            ("subject", subject.clone()),
        ]))
    }
}

fn effect_report(field: Js, status: &str, transport: &str, dispatched: bool) -> Js {
    obj(&[
        ("field", field),
        ("status", status.into()),
        ("transport", transport.into()),
        ("dispatched", dispatched.into()),
    ])
}
/// Concrete invented effects; core owns approval, fences and durable outcomes.
pub struct SyntheticCoreBackend {
    pub state: Arc<Mutex<SyntheticRecordState>>,
}
impl edu_academy_core::workflow::service::Backend for SyntheticCoreBackend {
    fn configured(&self) -> bool {
        true
    }
    fn save(
        &mut self,
        scope: &Js,
        patches: &Js,
        approval: &mut dyn edu_academy_core::workflow::approval::Approval,
        _: &Js,
    ) -> Result<Js> {
        use crate::record_backend::{EffectOutcome, TransportOutcome};
        let invalid = || Fault::error("record_source_invalid");
        let (id, store) = {
            let state = self.state.lock().map_err(|_| invalid())?;
            let id = state.adapter_identity(
                &scope.get("studentId").code_units(),
                &scope.get("lessonDate").code_units(),
            )?;
            let store = state.source_store(&id)?;
            (id, store)
        };
        let mut patch = RecordFields::from_fn(|_| None);
        for p in patches.array() {
            let field = RecordField::ALL
                .into_iter()
                .find(|f| p.get("field").is(f.wire_name()))
                .ok_or_else(invalid)?;
            let target = edu_academy_core::workflow::codec::parse(
                p.get("effectTarget").code_units().as_bytes(),
            )?;
            if patch[field].is_some()
                || !p.get("value").string()
                || !p.get("effectTarget").is(&canonical_effect_target(&target)?)
                || ["sourceNamespace", "teacherId", "studentId"]
                    .iter()
                    .any(|k| target.get(k) != scope.get(k))
                || target.get("recordId") != p.get("effectRecordKey")
            {
                return Err(invalid());
            }
            patch[field] = Some(FieldPatch {
                effect_target: p.get("effectTarget").code_units(),
                operation: PatchOperation::Set,
                value: p.get("value").code_units(),
            });
        }
        let account = store.lock_account()?;
        let before = self
            .state
            .lock()
            .map_err(|_| invalid())?
            .admitted_read(&account, &id, &patch)?;
        let RecordObservation::Observed { fields, .. } = &before.observation else {
            return Err(invalid());
        };
        if patches.array().iter().any(|p| {
            let field = RecordField::ALL
                .into_iter()
                .find(|f| p.get("field").is(f.wire_name()))
                .expect("checked field");
            !matches!(&fields[field], RecordValue::Known(v) if p.get("before").is(v))
        }) {
            return Ok(Js::Arr(
                patches
                    .array()
                    .iter()
                    .map(|p| {
                        effect_report(p.get("field").clone(), "before_drift", "not_invoked", false)
                    })
                    .collect(),
            ));
        }
        // Do not hold the state mutex during the callback. Every selected item is
        // durably fenced before this batch can mutate any academic value.
        for p in patches.array() {
            let action = obj(&[
                (
                    "attemptId",
                    format!("invented-{}", p.get("field").code_units()).into(),
                ),
                (
                    "target",
                    obj(&[
                        ("teacherKey", scope.get("teacherId").clone()),
                        ("studentKey", scope.get("studentId").clone()),
                        ("date", scope.get("lessonDate").clone()),
                    ]),
                ),
                ("field", p.get("field").clone()),
                ("effectTarget", p.get("effectTarget").clone()),
                ("effectRecordKey", p.get("effectRecordKey").clone()),
                ("before", p.get("before").clone()),
                ("after", p.get("value").clone()),
                ("wireDigest", p.get("wireDigest").clone()),
            ]);
            if !approval.consume(&action)?.get("status").is("verified") {
                return Err(Fault::error("record_core_approval_rejected"));
            }
        }
        let mut state = self.state.lock().map_err(|_| invalid())?;
        let effects =
            state.dispatch_with_account(&account, &id, &patch, &before.provenance.read_at)?;
        let read = state.read_once_with_account(&account, &id, &before.provenance.read_at);
        Ok(Js::Arr(
            effects
                .into_iter()
                .map(|effect| {
                    let p = patches
                        .array()
                        .iter()
                        .find(|p| p.get("field").is(effect.field.wire_name()))
                        .expect("selected field");
                    let value = match &read {
                        Ok(snapshot) => match &snapshot.observation {
                            RecordObservation::Observed { fields, .. } => {
                                fields[effect.field].clone()
                            }
                            _ => RecordValue::Unknown,
                        },
                        Err(_) => RecordValue::Unknown,
                    };
                    let known = matches!(value, RecordValue::Known(_));
                    let equal = matches!(&value, RecordValue::Known(v) if p.get("value").is(v));
                    let status = match (effect.outcome, known, equal) {
                        (EffectOutcome::Success, true, true) => "persisted",
                        (EffectOutcome::Reject, true, _) => "not_persisted",
                        _ => "unresolved",
                    };
                    let transport = match effect.transport {
                        TransportOutcome::Returned => "returned",
                        TransportOutcome::Lost => "lost",
                        TransportOutcome::ReturnedTimeout => "returned_timeout",
                    };
                    let mut result =
                        effect_report(effect.field.wire_name().into(), status, transport, true);
                    result.set(
                        "readback",
                        edu_academy_core::templates::from_json(
                            serde_json::to_value(value).expect("typed record value"),
                        ),
                    );
                    result.set("effectTarget", p.get("effectTarget").clone());
                    if matches!(
                        effect.outcome,
                        EffectOutcome::Success | EffectOutcome::Reject
                    ) {
                        result.set(
                            "completionFacts",
                            obj(&[
                                ("profile", "invented.synthetic-record".into()),
                                ("version", Js::Num(1.)),
                                ("field", effect.field.wire_name().into()),
                                ("effectTarget", p.get("effectTarget").clone()),
                                ("completion", "final_return".into()),
                                (
                                    "outcome",
                                    if effect.outcome == EffectOutcome::Success {
                                        "applied"
                                    } else {
                                        "rejected"
                                    }
                                    .into(),
                                ),
                                ("noPartialEffect", true.into()),
                                ("noLateEffect", true.into()),
                            ]),
                        );
                    }
                    result
                })
                .collect(),
        ))
    }
}
