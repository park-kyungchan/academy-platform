//! Explicit recorded-answer decoding; no source call, retry or mutation.
use crate::{
    record_backend::*,
    record_data::*,
    synthetic::{Scenario, load_scenario},
};
use edu_academy_core::{
    js::{Fault, Js, Result},
    workflow::codec,
};

#[derive(Clone)]
pub struct AnswerRecord {
    pub snapshot: RecordSnapshot,
    pub field_evidence: RecordFields<Js>,
    /// Retains whole-read coverage/failure metadata as well as field provenance.
    pub answer: Js,
}
fn invalid() -> Fault {
    Fault::error("record_answer_invalid")
}
fn string(v: &Js) -> Result<String> {
    if !v.string() {
        return Err(invalid());
    }
    String::from_utf16(v.units()).map_err(|_| invalid())
}
fn optional(v: &Js) -> Result<Option<String>> {
    if v.absent() {
        Ok(None)
    } else {
        string(v).map(Some)
    }
}
fn date(v: &str) -> Result<()> {
    chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").map_err(|_| invalid())?;
    Ok(())
}
pub fn decode_record_answer(capture: &Js, identity: RecordIdentity) -> Result<AnswerRecord> {
    date(&identity.lesson_date)?;
    let answer = capture.get("answer").get("structuredContent");
    let failed = answer.get("status").is("unavailable")
        || answer.get("sourceAvailability").is("unavailable");
    if !(capture.get("tool").is("academy_dailyrecord_get")
        || failed && capture.get("tool").is("academy_student_resolve"))
    {
        return Err(Fault::error("record_answer_not_record"));
    }
    if answer.get("lessonDate").string() && !answer.get("lessonDate").is(&identity.lesson_date) {
        return Err(Fault::error("record_answer_identity_conflict"));
    }
    let source = answer.get("source");
    let at = if failed {
        capture.get("clock").get("completed_at")
    } else {
        source.get("observedAt")
    };
    let read_at = string(at)?;
    chrono::DateTime::parse_from_rfc3339(&read_at).map_err(|_| invalid())?;
    let mut provenance = RecordProvenance {
        read_at,
        evidence_refs: vec![],
        source_revision: optional(source.get("sourceRevision"))?,
        wire_digest: optional(source.get("wireDigest"))?,
        field_reasons: RecordFields::from_fn(|_| None),
    };
    let mut field_evidence = RecordFields::from_fn(|_| Js::Null);
    let observation = if failed {
        let reason_code = string(answer.get("reasonCode"))?;
        provenance.field_reasons = RecordFields::from_fn(|_| Some(reason_code.clone()));
        if let Some(reference) = optional(answer.get("failure").get("evidenceLocator"))? {
            provenance.evidence_refs.push(reference);
        }
        RecordObservation::Failed { reason_code }
    } else {
        if !answer.get("status").is("observed") {
            return Err(invalid());
        }
        let scope = answer.get("scope");
        if !scope.get("studentId").is(&identity.student_key)
            || !scope.get("lessonDate").is(&identity.lesson_date)
        {
            return Err(Fault::error("record_answer_identity_conflict"));
        }
        let rows = answer.get("fields").array();
        if rows.len() != RECORD_FIELD_COUNT {
            return Err(invalid());
        }
        let mut fields = RecordFields::from_fn(|_| RecordValue::Unknown);
        for field in RecordField::ALL {
            let matching: Vec<_> = rows
                .iter()
                .filter(|r| r.get("field").is(field.wire_name()))
                .collect();
            if matching.len() != 1 {
                return Err(invalid());
            }
            let row = matching[0];
            let target = row.get("target");
            for key in ["sourceNamespace", "teacherId", "studentId"] {
                if string(target.get(key))?.is_empty() || target.get(key) != scope.get(key) {
                    return Err(Fault::error("record_answer_identity_conflict"));
                }
            }
            let target_date = string(target.get("lessonDate"))?;
            date(&target_date)?;
            if (field == RecordField::PreviousHomeworkAssessment
                && target_date >= identity.lesson_date)
                || (field != RecordField::PreviousHomeworkAssessment
                    && target_date != identity.lesson_date)
            {
                return Err(Fault::error("record_answer_identity_conflict"));
            }
            for key in ["courseId", "membershipId", "recordId"] {
                if string(target.get(key))?.is_empty() {
                    return Err(invalid());
                }
            }
            let before = row.get("before");
            fields[field] = if before.get("state").is("known") {
                RecordValue::Known(string(before.get("value"))?)
            } else if before.get("state").is("unknown") {
                provenance.field_reasons[field] = optional(before.get("reasonCode"))?;
                RecordValue::Unknown
            } else {
                return Err(invalid());
            };
            for reference in before.get("evidenceRefs").array() {
                provenance.evidence_refs.push(string(reference)?);
            }
            field_evidence[field] = row.clone();
        }
        let occurrence = string(field_evidence.progress.get("target").get("recordId"))?;
        RecordObservation::Observed { occurrence, fields }
    };
    Ok(AnswerRecord {
        snapshot: RecordSnapshot {
            requested_date: identity.lesson_date.clone(),
            identity,
            provenance,
            observation,
        },
        field_evidence,
        answer: answer.clone(),
    })
}

pub struct RecordFixture {
    pub scenario: Scenario,
    pub seed: RecordSeed,
    pub records: Vec<AnswerRecord>,
    pub assumptions: Vec<String>,
}
impl RecordFixture {
    pub fn from_json(text: &str) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|_| invalid())?;
        let scenario = load_scenario(&value["scenario"].to_string()).map_err(|_| invalid())?;
        let owner: OwnerScope =
            serde_json::from_value(value["owner"].clone()).map_err(|_| invalid())?;
        let assumptions: Vec<String> =
            serde_json::from_value(value["assumptions"].clone()).map_err(|_| invalid())?;
        if assumptions.is_empty() {
            return Err(invalid());
        }
        let mut records = vec![];
        for item in value["answers"].as_array().ok_or_else(invalid)? {
            let identity: RecordIdentity =
                serde_json::from_value(item["identity"].clone()).map_err(|_| invalid())?;
            if identity.owner != owner {
                return Err(Fault::error("record_answer_identity_conflict"));
            }
            records.push(decode_record_answer(
                &codec::parse(item["capture"].to_string().as_bytes())?,
                identity,
            )?);
        }
        let seed = RecordSeed {
            format: RECORD_SOURCE_FORMAT.into(),
            owner,
            records: records
                .iter()
                .filter(|r| matches!(r.snapshot.observation, RecordObservation::Observed { .. }))
                .map(|r| r.snapshot.clone())
                .collect(),
            writable_targets: serde_json::from_value(value["writable_targets"].clone())
                .map_err(|_| invalid())?,
            read_scripts: vec![],
            effect_scripts: serde_json::from_value(value["effect_scripts"].clone())
                .map_err(|_| invalid())?,
        };
        Ok(Self {
            scenario,
            seed,
            records,
            assumptions,
        })
    }
}
