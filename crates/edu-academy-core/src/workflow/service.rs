//! Existing request-bound save, metadata status and readback reconciliation.
use super::{
    Runtime,
    approval::RequestApproval,
    canonical, merge, nullable,
    review::{self, Policy},
    same,
    state::*,
    store::{Kind, LockOptions, Store},
};
use crate::js::{Fault, Js, Result, obj, sha};
/// Source result is qualified again by this service. No network implementation here.
pub trait Source {
    fn read(&mut self, scope: &Js, request: &Js, subject: &Js) -> Result<Js>;
}
pub trait Backend {
    fn configured(&self) -> bool;
    fn save(
        &mut self,
        scope: &Js,
        patches: &Js,
        approval: &mut dyn super::approval::Approval,
        subject: &Js,
    ) -> Result<Js>;
}
pub struct Service<'a> {
    pub store: &'a Store,
    pub source: &'a mut dyn Source,
    pub policy: &'a dyn Policy,
    pub backend: &'a mut dyn Backend,
    pub runtime: &'a dyn Runtime,
}
pub fn base(context: &Js) -> Js {
    let w = crate::templates::from_json(
        serde_json::from_str(include_str!("../workflow-context.json")).expect("source constant"),
    );
    obj(&[
        ("workflowContext", w),
        ("userContext", context.clone()),
        ("academyWrites", Js::Num(0.0)),
        ("parentSends", Js::Num(0.0)),
        ("automaticRetryAllowed", false.into()),
        ("sourceContentIsUntrustedData", true.into()),
    ])
}
pub fn qualified_observation(value: &Js, scope: &Js) -> Result<Js> {
    let observation = crate::schema::validate("dailyRecordObservationSchema", value)?;
    let t = observation.get("target");
    let p = observation.get("previousTarget");
    let target_scope =
        obj(&["sourceNamespace", "teacherId", "studentId", "lessonDate"]
            .map(|k| (k, t.get(k).clone())));
    if !same(observation.get("scope"), scope)? || !same(&target_scope, scope)? {
        return Err(Fault::error("source_scope_conflict"));
    }
    if !p.absent()
        && ([
            "sourceNamespace",
            "teacherId",
            "studentId",
            "courseId",
            "membershipId",
        ]
        .iter()
        .any(|k| p.get(k) != t.get(k))
            || p.get("recordId") == t.get("recordId")
            || p.get("lessonDate").units() >= t.get("lessonDate").units())
    {
        return Err(Fault::error("previous_record_scope_conflict"));
    }
    if p.absent()
        && observation
            .get("fields")
            .get("previousHomeworkAssessment")
            .get("state")
            .is("known")
    {
        return Err(Fault::error("previous_record_required"));
    }
    for f in review::FIELDS {
        let v = observation.get("fields").get(f);
        if v.get("state").is("known") && !crate::decision::valid_value(f, v.get("value")) {
            return Err(Fault::error("source_field_value_unqualified"));
        }
    }
    for sample in observation.get("toneSamples").array() {
        let st = sample.get("target");
        if ["sourceNamespace", "teacherId", "studentId"]
            .iter()
            .any(|k| st.get(k) != t.get(k))
            || st.get("lessonDate").units() >= t.get("lessonDate").units()
        {
            return Err(Fault::error("tone_sample_scope_conflict"));
        }
    }
    Ok(observation)
}
impl Service<'_> {
    pub(super) fn read_source(&mut self, scope: &Js, request: &Js, subject: &Js) -> Js {
        // Original #read projects an unavailable reason, or qualifies an observed
        // shape, then catches every typed source/qualification failure once.
        let read = (|| -> Result<Js> {
            let read = self.source.read(scope, request, subject)?;
            if read.get("status").is("unavailable") {
                return Ok(obj(&[
                    ("status", "unavailable".into()),
                    (
                        "reasonCode",
                        super::read_failure::safe_reason(read.get("reasonCode")),
                    ),
                ]));
            }
            Ok(obj(&[
                ("status", "observed".into()),
                (
                    "observation",
                    qualified_observation(read.get("observation"), scope)?,
                ),
                ("subject", read.get("subject").clone()),
            ]))
        })();
        read.unwrap_or_else(|error| super::read_failure::unavailable(&error))
    }

    fn pins(&self) -> Js {
        let parent = self.policy.parent_pins();
        obj(&[
            ("learningPolicy", self.policy.learning_pins()),
            (
                "parentSafeguard",
                if parent.absent() {
                    Js::Null
                } else {
                    obj(&[
                        ("policySha256", parent.get("policySha256").clone()),
                        (
                            "safeguardSourceSha256",
                            parent.get("safeguardSourceSha256").clone(),
                        ),
                    ])
                },
            ),
        ])
    }
    fn observation(&self, handle: &Js, scope: &Js, request: &Js) -> Result<Js> {
        let stored = self.store.get(Kind::Observation, &handle.code_units())?;
        if stored.absent()
            || !same(stored.get("scope"), scope)?
            || stored.get("requestRef") != request
        {
            return Err(Fault::error("exact_backend_observation_handle_required"));
        }
        super::schema::validate("dailyRecordObservationSchema", stored.get("observation"))?;
        Ok(stored)
    }
    pub fn review(&mut self, input: &Js, persist: bool) -> Result<Js> {
        let input = super::schema::validate("dailyRecordReviewSchema", input)?;
        let c = input.get("context");
        let scope = c.get("scope");
        let request = c.get("currentRequestRef");
        let held = if input.get("observationHandle").absent() {
            Js::Null
        } else {
            self.observation(input.get("observationHandle"), scope, request)?
        };
        // Original #review reads only after every global precondition passes.
        let fresh = if review::needs_fresh_read(&input, held.get("observation"), self.runtime)? {
            self.read_source(scope, request, held.get("subject"))
        } else {
            Js::Null
        };
        let mut result = review::review_read(
            &input,
            held.get("observation"),
            &fresh,
            self.policy,
            self.runtime,
        )?;
        if persist && result.get("status").is("prepared") {
            let fallback = super::observe_attribution(self.runtime, input.get("actor"));
            let attribution = if input.get("attribution").absent() {
                &fallback
            } else {
                input.get("attribution")
            };
            let by = attribution_summary(attribution);
            let patches = Js::Arr(
                result
                    .get("patch")
                    .array()
                    .iter()
                    .map(|p| {
                        obj(&[
                            ("field", p.get("field").clone()),
                            ("before", p.get("before").clone()),
                            ("after", p.get("after").clone()),
                            ("effectRecordKey", p.get("target").get("recordId").clone()),
                        ])
                    })
                    .collect(),
            );
            let record = obj(&[
                ("reviewDigest", result.get("reviewDigest").clone()),
                ("status", "prepared".into()),
                ("scope", scope.clone()),
                ("requestRef", request.clone()),
                ("observationHandle", input.get("observationHandle").clone()),
                ("patch", patches.clone()),
                ("reviewPolicy", self.pins()),
                ("reviewedBy", attribution.clone()),
                ("reviewedAt", self.runtime.now().into()),
                ("inputDigest", result.get("inputDigest").clone()),
            ]);
            self.store.publish(
                Kind::Review,
                &result.get("reviewDigest").code_units(),
                &record,
                self.runtime,
            )?;
            self.store.append_audit(
                &request_key(scope, request.clone())?,
                &obj(&[
                    ("event", "review_recorded".into()),
                    ("reviewDigest", result.get("reviewDigest").clone()),
                    ("by", by.clone()),
                    (
                        "fields",
                        Js::Arr(
                            patches
                                .array()
                                .iter()
                                .map(|p| p.get("field").clone())
                                .collect(),
                        ),
                    ),
                ]),
                self.runtime,
            )?;
            result.set(
                "reviewRecord",
                obj(&[("persisted", true.into()), ("reviewedBy", by)]),
            );
        }
        Ok(result)
    }
    fn hold(
        &self,
        base: &Js,
        key: &str,
        input: &Js,
        by: &Js,
        reason: &str,
        extra: Js,
    ) -> Result<Js> {
        self.store.append_audit(
            key,
            &obj(&[
                ("event", "save_held".into()),
                ("reasonCode", reason.into()),
                ("reviewDigest", input.get("reviewDigest").clone()),
                ("by", by.clone()),
            ]),
            self.runtime,
        )?;
        Ok(merge(
            merge(
                base.clone(),
                &obj(&[
                    ("status", "held".into()),
                    ("reasonCode", reason.into()),
                    ("academyWrites", Js::Num(0.0)),
                    ("execution", "not_supported".into()),
                    ("fields", Js::Arr(vec![])),
                ]),
            ),
            &extra,
        ))
    }
    pub fn save(&mut self, input: &Js) -> Result<Js> {
        let input = super::schema::validate("dailyRecordSaveSchema", input)?;
        let c = input.get("context");
        let s = c.get("scope");
        let request = c.get("currentRequestRef");
        let key = request_key(s, request.clone())?;
        let fallback = super::observe_attribution(self.runtime, input.get("actor"));
        let attribution = if input.get("attribution").absent() {
            &fallback
        } else {
            input.get("attribution")
        };
        let by = attribution_summary(attribution);
        let b = merge(
            base(c),
            &obj(&[
                ("observationHandle", input.get("observationHandle").clone()),
                ("reviewDigest", input.get("reviewDigest").clone()),
                ("attribution", by.clone()),
                (
                    "parentTransmission",
                    "separate_explicit_instruction_required".into(),
                ),
                ("workflowStoreScope", Store::SCOPE.into()),
            ]),
        );
        if !self.backend.configured() {
            return Ok(merge(
                b,
                &obj(&[
                    ("status", "unavailable".into()),
                    (
                        "reasonCode",
                        "dailyrecord_request_bound_save_backend_not_configured".into(),
                    ),
                    ("academyWrites", Js::Num(0.0)),
                    ("execution", "not_supported".into()),
                    ("fields", Js::Arr(vec![])),
                ]),
            ));
        }
        let hold = |reason, extra| self.hold(&b, &key, &input, &by, reason, extra);
        if input.get("request").absent() {
            return hold("user_instruction_provenance_required", obj(&[]));
        }
        if input.get("request").get("requestRef") != request {
            return hold("user_request_ref_mismatch", obj(&[]));
        }
        let instruction_sha = sha(&String::from_utf16_lossy(
            input.get("request").get("instruction").get("text").units(),
        ));
        let (created, record) = self.store.publish(
            Kind::Request,
            &key,
            &obj(&[
                ("request", input.get("request").clone()),
                ("instructionSha256", instruction_sha.clone().into()),
                ("recordedBy", attribution.clone()),
                ("recordedAt", self.runtime.now().into()),
            ]),
            self.runtime,
        )?;
        if !created && !record.get("instructionSha256").is(&instruction_sha) {
            return hold("request_instruction_conflict", obj(&[]));
        }
        if created {
            self.store.append_audit(
                &key,
                &obj(&[
                    ("event", "request_recorded".into()),
                    ("instructionSha256", instruction_sha.into()),
                    (
                        "sessionRef",
                        nullable(input.get("request").get("instruction").get("sessionRef")),
                    ),
                    ("by", by.clone()),
                ]),
                self.runtime,
            )?;
        }
        let prior = self
            .store
            .get(Kind::Review, &input.get("reviewDigest").code_units())?;
        if prior.absent() {
            return hold("prior_review_record_required", obj(&[]));
        }
        let digest = sha(&canonical(&obj(&[
            ("context", c.clone()),
            ("proposals", input.get("proposals").clone()),
            (
                "cismObservationHandle",
                nullable(input.get("cismObservationHandle")),
            ),
        ]))?);
        if !prior.get("status").is("prepared")
            || !same(prior.get("scope"), s)?
            || prior.get("requestRef") != request
            || prior.get("observationHandle") != input.get("observationHandle")
            || !same(prior.get("reviewPolicy"), &self.pins())?
            || !prior.get("inputDigest").is(&digest)
        {
            return hold("prior_review_record_mismatch", obj(&[]));
        }
        let _guard = match self
            .store
            .lock(&lock_key(s)?, LockOptions::default(), &|| false)
        {
            Ok(g) => g,
            Err(e) if e.message == "shared_writer_lock_busy" => {
                return hold("shared_writer_lock_busy", obj(&[]));
            }
            Err(e) => return Err(e),
        };
        let pkey = |p: &Js| checked_execution_key(self.store, request, s, p);
        let mut unresolved = vec![];
        for p in prior.get("patch").array() {
            let count = unresolved_on_field(
                self.store,
                s,
                &p.get("field").code_units(),
                p.get("effectRecordKey").clone(),
            )?
            .len();
            if count > 0 {
                unresolved.push(obj(&[
                    ("field", p.get("field").clone()),
                    ("unresolvedExecutions", count.into()),
                ]));
            }
        }
        if !unresolved.is_empty() {
            return hold(
                "uncertain_dispatch_reconcile_required",
                obj(&[("fields", Js::Arr(unresolved))]),
            );
        }
        let report = |p: &Js, extra: Js| -> Result<Js> {
            Ok(merge(
                obj(&[
                    ("field", p.get("field").clone()),
                    (
                        "executionState",
                        field_state(self.store, &pkey(p)?)?.name().into(),
                    ),
                ]),
                &extra,
            ))
        };
        let states = prior
            .get("patch")
            .array()
            .iter()
            .map(|p| field_state(self.store, &pkey(p)?))
            .collect::<Result<Vec<_>>>()?;
        if states.iter().any(|s| s.consumed_without_effect()) {
            return hold(
                "transition_already_attempted_new_request_required",
                obj(&[(
                    "fields",
                    Js::Arr(
                        prior
                            .get("patch")
                            .array()
                            .iter()
                            .map(|p| report(p, obj(&[])))
                            .collect::<Result<Vec<_>>>()?,
                    ),
                )]),
            );
        }
        if !states.is_empty() && states.iter().all(|s| s.settled_effect()) {
            return Ok(merge(
                b,
                &obj(&[
                    ("status", "saved".into()),
                    ("reasonCode", "duplicate_execution_already_settled".into()),
                    ("academyWrites", Js::Num(0.0)),
                    ("execution", "owner_request_bound".into()),
                    (
                        "fields",
                        Js::Arr(
                            prior
                                .get("patch")
                                .array()
                                .iter()
                                .map(|p| {
                                    report(
                                        p,
                                        obj(&[
                                            ("status", "noop".into()),
                                            ("dispatched", false.into()),
                                        ]),
                                    )
                                })
                                .collect::<Result<Vec<_>>>()?,
                        ),
                    ),
                ]),
            ));
        }
        // Reborrow ends before the fresh source/policy review.
        let review_input = obj(&[
            ("context", c.clone()),
            ("observationHandle", input.get("observationHandle").clone()),
            (
                "cismObservationHandle",
                input.get("cismObservationHandle").clone(),
            ),
            ("proposals", input.get("proposals").clone()),
        ]);
        let fresh = self.review(&review_input, false)?;
        if !fresh.get("status").is("prepared")
            || fresh.get("reviewDigest") != input.get("reviewDigest")
        {
            let fields = Js::Arr(
                fresh
                    .get("rows")
                    .array()
                    .iter()
                    .map(|r| {
                        obj(&[
                            ("field", r.get("field").clone()),
                            ("status", "not_attempted".into()),
                            ("dispatched", false.into()),
                            ("blockers", r.get("blockers").clone()),
                            ("holdActions", r.get("holdActions").clone()),
                        ])
                    })
                    .collect(),
            );
            return self.hold(
                &b,
                &key,
                &input,
                &by,
                "fresh_review_drift_new_review_required",
                obj(&[("fields", fields)]),
            );
        }
        if prior.get("patch").array().is_empty() {
            return Ok(merge(
                b,
                &obj(&[
                    ("status", "saved".into()),
                    ("reasonCode", "reviewed_patch_empty".into()),
                    ("academyWrites", Js::Num(0.0)),
                    ("execution", "owner_request_bound".into()),
                    (
                        "fields",
                        Js::Arr(
                            input
                                .get("proposals")
                                .array()
                                .iter()
                                .map(|p| {
                                    obj(&[
                                        ("field", p.get("field").clone()),
                                        ("status", "noop".into()),
                                        ("dispatched", false.into()),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]),
            ));
        }
        let patches = Js::Arr(
            fresh
                .get("patch")
                .array()
                .iter()
                .map(|p| {
                    obj(&[
                        ("field", p.get("field").clone()),
                        ("value", p.get("after").clone()),
                        ("before", p.get("before").clone()),
                        ("wireDigest", fresh.get("wireDigest").clone()),
                    ])
                })
                .collect(),
        );
        let mut approval = RequestApproval {
            store: self.store,
            input: obj(&[
                ("requestRef", request.clone()),
                ("attribution", attribution.clone()),
                ("reviewDigest", input.get("reviewDigest").clone()),
                ("scope", s.clone()),
                ("reviewed", prior.get("patch").clone()),
            ]),
            runtime: self.runtime,
        };
        let subject = self
            .store
            .get(
                Kind::Observation,
                &input.get("observationHandle").code_units(),
            )?
            .get("subject")
            .clone();
        self.store.append_audit(
            &key,
            &obj(&[
                ("event", "dispatch_started".into()),
                ("by", by.clone()),
                (
                    "fields",
                    Js::Arr(
                        patches
                            .array()
                            .iter()
                            .map(|p| p.get("field").clone())
                            .collect(),
                    ),
                ),
            ]),
            self.runtime,
        )?;
        let outcome = |p: &Js, r: Js| -> Result<()> {
            let k = pkey(p)?;
            if !self.store.get(Kind::Execution, &k)?.absent() {
                self.store.publish(
                    Kind::Outcome,
                    &k,
                    &merge(
                        r,
                        &obj(&[
                            ("recordedBy", by.clone()),
                            ("recordedAt", self.runtime.now().into()),
                        ]),
                    ),
                    self.runtime,
                )?;
            }
            Ok(())
        };
        let results = match self.backend.save(s, &patches, &mut approval, &subject) {
            Ok(r) => r,
            Err(error) => {
                for p in prior.get("patch").array() {
                    outcome(
                        p,
                        obj(&[("status", "unresolved".into()), ("dispatched", Js::Null)]),
                    )?;
                }
                self.store.append_audit(
                    &key,
                    &obj(&[
                        ("event", "dispatch_failed".into()),
                        ("by", by.clone()),
                        (
                            "reasonCode",
                            super::read_failure::public_read_failure(
                                &error,
                                "dailyrecord_save_unqualified_no_automatic_retry",
                            )
                            .get("code")
                            .clone(),
                        ),
                    ]),
                    self.runtime,
                )?;
                return Ok(merge(
                    b,
                    &obj(&[
                        ("status", "held".into()),
                        ("reasonCode", "save_reconciliation_required".into()),
                        ("academyWrites", Js::Null),
                        ("execution", "owner_request_bound".into()),
                        (
                            "fields",
                            Js::Arr(
                                prior
                                    .get("patch")
                                    .array()
                                    .iter()
                                    .map(|p| {
                                        report(
                                            p,
                                            obj(&[
                                                ("status", "unresolved".into()),
                                                ("dispatched", Js::Null),
                                            ]),
                                        )
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            ),
                        ),
                    ]),
                ));
            }
        };
        let recorded = |r: &Js| {
            if r.get("status").is("not_persisted") && !r.get("transport").is("returned") {
                "unresolved".into()
            } else {
                r.get("status").clone()
            }
        };
        for r in results.array() {
            if let Some(p) = prior
                .get("patch")
                .array()
                .iter()
                .find(|p| p.get("field") == r.get("field"))
            {
                outcome(
                    p,
                    obj(&[
                        ("status", recorded(r)),
                        ("readbackStatus", r.get("status").clone()),
                        ("dispatched", r.get("dispatched").clone()),
                        ("transport", nullable(r.get("transport"))),
                    ]),
                )?;
            }
        }
        let writes = results
            .array()
            .iter()
            .filter(|r| r.get("dispatched").truthy())
            .count();
        let settled = results
            .array()
            .iter()
            .all(|r| r.get("status").is("persisted") || r.get("status").is("noop"));
        let reason = if settled {
            Js::Null
        } else if results.array().iter().any(|r| {
            r.get("status").is("unresolved")
                || r.get("status").is("conflict")
                || recorded(r).is("unresolved")
        }) {
            "save_reconciliation_required".into()
        } else if results
            .array()
            .iter()
            .any(|r| r.get("status").is("before_drift"))
        {
            "fresh_before_drift_new_review_required".into()
        } else if results
            .array()
            .iter()
            .any(|r| r.get("status").is("not_persisted"))
        {
            "readback_not_persisted_new_request_required".into()
        } else {
            "partial_save_review_remaining_fields".into()
        };
        self.store.append_audit(
            &key,
            &obj(&[
                ("event", "dispatch_completed".into()),
                ("by", by.clone()),
                ("academyWrites", writes.into()),
                (
                    "results",
                    Js::Arr(
                        results
                            .array()
                            .iter()
                            .map(|r| {
                                obj(&[
                                    ("field", r.get("field").clone()),
                                    ("status", r.get("status").clone()),
                                    ("dispatched", r.get("dispatched").clone()),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ]),
            self.runtime,
        )?;
        let fields = results
            .array()
            .iter()
            .map(|r| {
                let p = prior
                    .get("patch")
                    .array()
                    .iter()
                    .find(|p| p.get("field") == r.get("field"));
                Ok(obj(&[
                    ("field", r.get("field").clone()),
                    ("status", recorded(r)),
                    ("dispatched", r.get("dispatched").clone()),
                    ("transport", nullable(r.get("transport"))),
                    (
                        "executionState",
                        if let Some(p) = p {
                            field_state(self.store, &pkey(p)?)?.name()
                        } else {
                            "available"
                        }
                        .into(),
                    ),
                ]))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(merge(
            b,
            &obj(&[
                ("status", if settled { "saved" } else { "held" }.into()),
                ("reasonCode", reason),
                ("academyWrites", writes.into()),
                ("execution", "owner_request_bound".into()),
                ("fields", Js::Arr(fields)),
            ]),
        ))
    }
    pub fn reconcile(&mut self, input: &Js) -> Result<Js> {
        let input = super::schema::validate("dailyRecordReconcileSchema", input)?;
        let c = input.get("context");
        let scope = c.get("scope");
        let b = merge(
            base(c),
            &obj(&[
                ("execution", "read_only".into()),
                (
                    "parentTransmission",
                    "separate_explicit_instruction_required".into(),
                ),
            ]),
        );
        let fallback = super::observe_attribution(self.runtime, input.get("actor"));
        let by = attribution_summary(if input.get("attribution").absent() {
            &fallback
        } else {
            input.get("attribution")
        });
        let resolution = input.get("resolution").get("request");
        if !resolution.absent() && resolution.get("requestRef") != c.get("currentRequestRef") {
            return Ok(merge(
                b,
                &obj(&[
                    ("status", "held".into()),
                    ("reasonCode", "user_request_ref_mismatch".into()),
                    ("by", by),
                    ("fields", Js::Arr(vec![])),
                ]),
            ));
        }
        let resolved_by = if resolution.absent() {
            Js::Null
        } else {
            obj(&[
                ("requestRef", resolution.get("requestRef").clone()),
                ("instruction", resolution.get("instruction").clone()),
                (
                    "instructionSha256",
                    sha(&String::from_utf16_lossy(
                        resolution.get("instruction").get("text").units(),
                    ))
                    .into(),
                ),
            ])
        };
        let _guard = self
            .store
            .lock(&lock_key(scope)?, LockOptions::default(), &|| false)?;
        let fields = || -> Result<Js> {
            Ok(Js::Arr(
                student_executions(self.store, scope)?
                    .iter()
                    .map(|e| {
                        obj(&[
                            ("field", e.record.get("field").clone()),
                            (
                                "lessonDate",
                                e.record.get("scope").get("lessonDate").clone(),
                            ),
                            ("requestRef", e.record.get("requestRef").clone()),
                            ("executionState", e.state.name().into()),
                        ])
                    })
                    .collect(),
            ))
        };
        let pending = student_executions(self.store, scope)?
            .into_iter()
            .filter(|e| e.state.needs_reconciliation())
            .collect::<Vec<_>>();
        let mut reads: Vec<(String, Js)> = vec![];
        let mut not_observed = false;
        let mut uncertain = false;
        for e in pending {
            let r = &e.record;
            let scope_key = crate::js::stringify(r.get("scope"))?;
            if !reads.iter().any(|(k, _)| k == &scope_key) {
                let review = self
                    .store
                    .get(Kind::Review, &r.get("reviewDigest").code_units())?;
                let obs = self.store.get(
                    Kind::Observation,
                    &review.get("observationHandle").code_units(),
                )?;
                let read = self.read_source(
                    r.get("scope"),
                    c.get("currentRequestRef"),
                    obs.get("subject"),
                );
                reads.push((scope_key.clone(), read));
            }
            let read = &reads
                .iter()
                .find(|(k, _)| k == &scope_key)
                .expect("inserted source read")
                .1;
            let req_key = request_key(r.get("scope"), r.get("requestRef").clone())?;
            if !read.get("status").is("observed") {
                self.store.append_audit(
                    &req_key,
                    &obj(&[
                        ("event", "reconcile_readback_unavailable".into()),
                        ("by", by.clone()),
                        ("reasonCode", read.get("reasonCode").clone()),
                    ]),
                    self.runtime,
                )?;
                continue;
            }
            let observed =
                super::schema::validate("dailyRecordObservationSchema", read.get("observation"))?;
            let field = r.get("field").code_units();
            let target = if field == "previousHomeworkAssessment" {
                observed.get("previousTarget")
            } else {
                observed.get("target")
            };
            if r.get("effectRecordKey") != target.get("recordId") {
                not_observed = true;
                self.store.append_audit(
                    &req_key,
                    &obj(&[
                        ("event", "reconcile_effect_record_not_observed".into()),
                        ("field", r.get("field").clone()),
                        ("by", by.clone()),
                    ]),
                    self.runtime,
                )?;
                continue;
            }
            let current = observed.get("fields").get(&field);
            if !current.get("state").is("known") {
                continue;
            }
            let value = sha(&String::from_utf16_lossy(current.get("value").units()));
            let classification = if r.get("afterSha256").is(&value) {
                "persisted"
            } else if r.get("beforeSha256").is(&value) {
                "not_persisted"
            } else {
                "conflict"
            };
            let returned = self
                .store
                .get(Kind::Outcome, &e.key)?
                .get("transport")
                .is("returned");
            let resolves = !resolved_by.absent()
                && same(r.get("scope"), scope)?
                && input
                    .get("resolution")
                    .get("executions")
                    .array()
                    .iter()
                    .any(|x| {
                        x.get("requestRef") == r.get("requestRef")
                            && x.get("field") == r.get("field")
                    });
            if classification != "persisted" && !returned && !resolves {
                uncertain = true;
                continue;
            }
            let basis = if classification == "persisted" {
                "readback_new_value"
            } else if returned {
                "readback_after_returned_transport"
            } else {
                "user_instruction"
            };
            self.store.publish(
                Kind::Reconciliation,
                &e.key,
                &obj(&[
                    ("classification", classification.into()),
                    ("basis", basis.into()),
                    (
                        "resolvedBy",
                        if basis == "user_instruction" {
                            resolved_by.clone()
                        } else {
                            Js::Null
                        },
                    ),
                    ("sourceRevision", observed.get("sourceRevision").clone()),
                    ("observedAt", observed.get("observedAt").clone()),
                    ("by", by.clone()),
                    ("reconciledAt", self.runtime.now().into()),
                ]),
                self.runtime,
            )?;
            self.store.append_audit(
                &req_key,
                &obj(&[
                    ("event", "reconciled".into()),
                    ("field", r.get("field").clone()),
                    ("classification", classification.into()),
                    ("basis", basis.into()),
                    (
                        "instructionSha256",
                        if basis == "user_instruction" {
                            resolved_by.get("instructionSha256").clone()
                        } else {
                            Js::Null
                        },
                    ),
                    ("by", by.clone()),
                ]),
                self.runtime,
            )?;
        }
        let after = fields()?;
        let open = student_executions(self.store, scope)?
            .iter()
            .any(|e| e.state.needs_reconciliation());
        let unavailable = reads.iter().any(|(_, r)| !r.get("status").is("observed"));
        let reason = if !open {
            Js::Null
        } else {
            if unavailable {
                "readback_unavailable_reconcile_later"
            } else if not_observed {
                "affected_record_not_observed_manual_reconcile_required"
            } else if uncertain {
                "uncertain_dispatch_not_landed_user_resolution_required"
            } else {
                "readback_field_unknown_reconcile_later"
            }
            .into()
        };
        Ok(merge(
            b,
            &obj(&[
                ("status", if open { "held" } else { "settled" }.into()),
                ("reasonCode", reason),
                ("by", by),
                ("fields", after),
            ]),
        ))
    }
}
pub fn status(store: &Store, input: &Js) -> Result<Js> {
    let input = super::schema::validate("dailyRecordExecutionStatusSchema", input)?;
    let scope = input.get("scope");
    let b = obj(&[
        ("scope", scope.clone()),
        ("requestRef", nullable(input.get("requestRef"))),
        ("academyWrites", Js::Num(0.0)),
        ("parentSends", Js::Num(0.0)),
        ("automaticRetryAllowed", false.into()),
        ("execution", "read_only".into()),
    ]);
    let executions = student_executions(store, scope)?
        .into_iter()
        .filter_map(|e| match same(e.record.get("scope"), scope) {
            Ok(true) => Some(Ok(e)),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>>>()?;
    let mut fields = vec![];
    for field in review::FIELDS {
        let values = executions
            .iter()
            .filter(|e| e.record.get("field").is(field))
            .map(|e| {
                let r = &e.record;
                obj(&[
                    ("requestRef", r.get("requestRef").clone()),
                    ("effectRecordKey", r.get("effectRecordKey").clone()),
                    ("beforeSha256", r.get("beforeSha256").clone()),
                    ("afterSha256", r.get("afterSha256").clone()),
                    ("executionState", e.state.name().into()),
                    ("by", attribution_summary(r.get("attribution"))),
                    ("recordedAt", r.get("recordedAt").clone()),
                ])
            })
            .collect::<Vec<_>>();
        if !values.is_empty() {
            fields.push(obj(&[
                ("field", field.into()),
                ("executions", Js::Arr(values)),
            ]));
        }
    }
    let request_key = if input.get("requestRef").truthy() {
        Some(request_key(scope, input.get("requestRef").clone())?)
    } else {
        None
    };
    let request = if let Some(k) = &request_key {
        store.get(Kind::Request, k)?
    } else {
        Js::Null
    };
    let request = if request.absent() {
        Js::Null
    } else {
        obj(&[
            (
                "instructionSha256",
                request.get("instructionSha256").clone(),
            ),
            (
                "sessionRef",
                nullable(request.get("request").get("instruction").get("sessionRef")),
            ),
            (
                "receivedAt",
                nullable(request.get("request").get("instruction").get("receivedAt")),
            ),
            ("recordedBy", attribution_summary(request.get("recordedBy"))),
            ("recordedAt", request.get("recordedAt").clone()),
        ])
    };
    Ok(merge(
        b,
        &obj(&[
            ("status", "recorded".into()),
            ("storeScope", Store::SCOPE.into()),
            ("fields", Js::Arr(fields)),
            ("request", request),
            (
                "audit",
                if let Some(k) = &request_key {
                    store.audit(k, 256)?
                } else {
                    Js::Arr(vec![])
                },
            ),
        ]),
    ))
}
