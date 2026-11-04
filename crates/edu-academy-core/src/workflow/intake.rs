//! Workflow-configured intake: qualified observations use the existing owner store.
use super::{
    merge, nullable, observe_attribution,
    review::{self, FIELDS},
    service::{Service, base},
    state::{attribution_summary, request_key},
    store::Kind,
};
use crate::js::{Fault, Js, Result, obj, trim};

impl Service<'_> {
    pub fn intake(&mut self, input: &Js) -> Result<Js> {
        let input = crate::schema::validate("dailyRecordIntakeSchema", input)?;
        let context = input.get("context");
        let supplied_values = input.get("suppliedValues").array();
        for (i, proposal) in supplied_values.iter().enumerate() {
            if supplied_values[..i]
                .iter()
                .any(|p| p.get("field") == proposal.get("field"))
            {
                return Err(Fault::error("duplicate_proposal_field"));
            }
        }
        let scope = context.get("scope");
        let request = context.get("currentRequestRef");
        let read = self.read_source(scope, request, &Js::Undefined);
        let unavailable_source = Js::Null;
        let source = if read.get("status").is("observed") {
            read.get("observation")
        } else {
            &unavailable_source
        };
        let mut blockers = if read.get("status").is("unavailable") {
            vec![read.get("reasonCode").clone()]
        } else if review::stale(source.get("observedAt"), review::now_epoch(self.runtime)?) {
            vec!["stale_source".into()]
        } else {
            vec![]
        };
        let mut interviews = vec![];
        let mut fields = vec![];
        for field in FIELDS {
            let current = if source.absent() {
                obj(&[
                    ("state", "unknown".into()),
                    ("reasonCode", "source_not_read".into()),
                ])
            } else {
                source.get("fields").get(field).clone()
            };
            let supplied = supplied_values.iter().any(|p| {
                p.get("field").is(field) && review::proposal_blockers(p, context).is_empty()
            });
            let existing = current.get("state").is("known")
                && !review::unknown_assessment(field, current.get("value"))
                && (!matches!(field, "attendance" | "progress" | "homework")
                    || !trim(current.get("value").units()).is_empty());
            let facts = context
                .get("facts")
                .array()
                .iter()
                .any(|f| f.get("field").is(field));
            let learning_field = matches!(field, "progress" | "homework");
            let interview = if field == "lessonMemo" {
                context.get("memoAnswer").get("kind").is("not_supplied")
            } else {
                !existing && !supplied && (!learning_field || !facts)
            };
            if interview {
                interviews.push(obj(&[
                    ("field", field.into()),
                    (
                        "reasonCode",
                        if field == "lessonMemo" {
                            "today_student_specific_memo_required"
                        } else {
                            "explicit_value_or_qualified_existing_value_required"
                        }
                        .into(),
                    ),
                ]));
            }
            fields.push(obj(&[
                ("field", field.into()),
                ("before", current),
                (
                    "selected",
                    context
                        .get("selectedFields")
                        .array()
                        .iter()
                        .any(|f| f.is(field))
                        .into(),
                ),
                (
                    "disposition",
                    if existing {
                        "preserve_existing"
                    } else if supplied {
                        "explicit_user_value"
                    } else if facts && learning_field {
                        "user_facts_available"
                    } else {
                        "missing"
                    }
                    .into(),
                ),
                (
                    "target",
                    nullable(source.get(if field == "previousHomeworkAssessment" {
                        "previousTarget"
                    } else {
                        "target"
                    })),
                ),
            ]));
        }
        for p in supplied_values {
            blockers.extend(review::proposal_blockers(p, context).iter().map(|b| {
                let mut value = p.get("field").units().to_vec();
                value.push(u16::from(b':'));
                value.extend_from_slice(b.units());
                Js::Str(value)
            }));
        }
        let attribution = if input.get("attribution").absent() {
            observe_attribution(self.runtime, input.get("actor"))
        } else {
            input.get("attribution").clone()
        };
        let by = attribution_summary(&attribution);
        let mut handle = Js::Null;
        if !source.absent() && blockers.is_empty() {
            handle = format!("dailyrecord-observation:{}", self.runtime.uuid()?).into();
            let subject = if read.get("subject").truthy() {
                obj(&[(
                    "displayName",
                    read.get("subject").get("displayName").clone(),
                )])
            } else {
                Js::Null
            };
            self.store.publish(
                Kind::Observation,
                &handle.code_units(),
                &obj(&[
                    ("observation", source.clone()),
                    ("scope", scope.clone()),
                    ("requestRef", request.clone()),
                    ("subject", subject),
                    ("createdBy", attribution),
                    ("createdAt", self.runtime.now().into()),
                ]),
                self.runtime,
            )?;
            // Publication and audit are separate durable operations. Propagate an
            // audit failure without removing or retrying the published observation.
            self.store.append_audit(
                &request_key(scope, request.clone())?,
                &obj(&[
                    ("event", "observation_recorded".into()),
                    ("observationHandle", handle.clone()),
                    ("by", by.clone()),
                    ("sourceRevision", source.get("sourceRevision").clone()),
                ]),
                self.runtime,
            )?;
        }
        let tone_samples = if !source.absent() && !self.policy.parent_pins().absent() {
            source
                .get("toneSamples")
                .array()
                .iter()
                .filter(|s| self.policy.check_lines(s.get("text")))
                .cloned()
                .collect()
        } else {
            vec![]
        };
        Ok(merge(
            base(context),
            &obj(&[
                (
                    "status",
                    if !blockers.is_empty() {
                        "held"
                    } else if !interviews.is_empty() {
                        "interview_required"
                    } else {
                        "ready_for_review"
                    }
                    .into(),
                ),
                ("observationHandle", handle.clone()),
                (
                    "observationPersistence",
                    if handle.absent() {
                        "process_local"
                    } else {
                        "owner_durable"
                    }
                    .into(),
                ),
                ("attribution", by),
                ("fields", Js::Arr(fields)),
                ("interviews", Js::Arr(interviews)),
                ("blockers", Js::Arr(blockers)),
                (
                    "source",
                    if source.absent() {
                        Js::Null
                    } else {
                        obj(&[
                            ("observedAt", source.get("observedAt").clone()),
                            ("sourceRevision", source.get("sourceRevision").clone()),
                            ("wireDigest", source.get("wireDigest").clone()),
                            ("transmissionState", source.get("transmissionState").clone()),
                        ])
                    },
                ),
                ("toneSamples", Js::Arr(tone_samples)),
                (
                    "toneSelection",
                    "hyphen_lines_only_other_formats_ignored".into(),
                ),
                ("sourceAvailability", read.get("status").clone()),
                ("failure", nullable(read.get("failure"))),
                ("execution", "not_supported".into()),
            ]),
        ))
    }
}
