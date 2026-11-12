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
