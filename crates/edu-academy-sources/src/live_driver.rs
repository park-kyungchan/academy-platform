//! Opt-in one-field qualification. Output contains only codes, metadata and hashes.
use edu_academy_core::{
    js::{Fault, Js, Result, obj, sha},
    templates::from_json as j,
    workflow::{canonical, review::Policy, service::Service, store::Store},
};
use serde_json::json;

pub struct DisconnectedPolicy;
impl Policy for DisconnectedPolicy {
    fn parent_pins(&self) -> Js {
        Js::Null
    }
    fn learning_pins(&self) -> Js {
        Js::Null
    }
    fn check_lines(&self, _: &Js) -> bool {
        false
    }
    fn excluded_parent_language(&self, _: &Js) -> bool {
        true
    }
    fn scope_qualified(&self, _: &Js) -> bool {
        false
    }
    fn learning_review(&self, _: &Js, _: &Js, _: &Js) -> Vec<Js> {
        vec!["field_not_supported".into()]
    }
}
pub fn supported(input: &Js) -> Result<()> {
    if ![
        "attendance",
        "dailyTestAssessment",
        "previousHomeworkAssessment",
    ]
    .iter()
    .any(|f| input.get("field").is(f))
    {
        return Err(Fault::error("field_not_supported"));
    }
    Ok(())
}
pub fn store(root: &std::path::Path, directory: &std::path::Path) -> Result<Store> {
    use std::os::unix::fs::PermissionsExt;
    if directory.ancestors().any(|p| p.join(".git").exists()) {
        return Err(Fault::error("store_inside_git_refused"));
    }
    if let Ok(metadata) = std::fs::symlink_metadata(directory)
        && metadata.permissions().mode() & 0o077 != 0
    {
        return Err(Fault::error("store_mode_refused"));
    }
    Store::open(directory, root)
}
fn hash(v: &Js) -> Js {
    sha(&String::from_utf16_lossy(v.units())).into()
}
pub fn prepare(input: &Js, service: &mut Service<'_>) -> Result<(Js, Js)> {
    supported(input)?;
    let wire: serde_json::Value = serde_json::from_str(&canonical(input)?)
        .map_err(|_| Fault::error("driver_input_rejected"))?;
    let f = input.get("field");
    let context = j(json!({"schemaVersion":"academy.dailyrecord-user-context/1",
        "purpose":"One reviewed field", "currentRequestRef":wire["requestRef"], "scope":wire["scope"],
        "selectedFields":[wire["field"]], "memoAnswer":{"kind":"no_addition","requestRef":wire["requestRef"]},
        "facts":[{"ref":"explicit","field":wire["field"],"text":"Explicit field value","explicitValue":wire["value"],
        "requiredFragments":[],"questionCounts":[],"deadlines":[],"stageClaims":[]}]}));
    let proposals = j(
        json!([{"field":wire["field"],"value":wire["value"],"factRefs":["explicit"],
        "decision":{"kind":"replace","requestRef":wire["requestRef"]}}]),
    );
    let mut wire_code = Js::Null;
    let intake = {
        let mut source = GateSource(service.source, &mut wire_code);
        Service {
            store: service.store,
            source: &mut source,
            backend: service.backend,
            policy: service.policy,
            runtime: service.runtime,
        }
        .intake(&obj(&[
            ("context", context.clone()),
            ("suppliedValues", proposals.clone()),
        ]))?
    };
    let review_input = obj(&[
        ("context", context),
        ("proposals", proposals),
        ("observationHandle", intake.get("observationHandle").clone()),
    ]);
    if !intake.get("blockers").array().is_empty() {
        let gate = if wire_code.absent() {
            intake.get("blockers").array()[0].clone()
        } else {
            wire_code
        };
        return Ok(held(review_input, gate, intake.get("blockers").clone()));
    }
    let review = service.review(&review_input, true)?;
    if !review.get("status").is("prepared") {
        let blockers = review
            .get("rows")
            .array()
            .iter()
            .flat_map(|r| r.get("blockers").array().to_vec())
            .collect();
        return Ok(held(review_input, "passed".into(), Js::Arr(blockers)));
    }
    let p = review
        .get("patch")
        .array()
        .first()
        .ok_or_else(|| Fault::error("prepared_patch_required"))?;
    let mut binding = obj(&[
        ("scope", review.get("scope").clone()),
        ("field", f.clone()),
        ("effectTarget", p.get("effectTarget").clone()),
        ("effectRecordKey", p.get("effectRecordKey").clone()),
        ("beforeSha256", hash(p.get("before"))),
        ("afterSha256", hash(p.get("after"))),
    ]);
    let digest = sha(&canonical(&binding)?);
    // Identity and value-bearing binding parts never leave the private core store.
    binding = obj(&[
        ("planDigest", digest.into()),
        ("field", f.clone()),
        ("beforeSha256", binding.get("beforeSha256").clone()),
        ("afterSha256", binding.get("afterSha256").clone()),
        ("wireGate", "passed".into()),
        ("status", "prepared".into()),
    ]);
    let observation = service.store.get(
        edu_academy_core::workflow::store::Kind::Observation,
        &intake.get("observationHandle").code_units(),
    )?;
    let observation = observation.get("observation");
    let fields = observation.get("fields");
    let others = edu_academy_core::workflow::review::FIELDS
        .iter()
        .filter(|k| !f.is(k))
        .map(|k| (*k, hash(fields.get(k).get("value"))))
        .collect::<Vec<_>>();
    binding.set("otherFieldsSha256", obj(&others));
    binding.set(
        "identitySha256",
        obj(&["sourceNamespace", "studentId", "lessonDate", "courseId"]
            .map(|k| (k, hash(observation.get("target").get(k))))),
    );
    let mut saved_input = review_input;
    saved_input.set("reviewDigest", review.get("reviewDigest").clone());
    saved_input.set("request",j(json!({"requestRef":wire["requestRef"],"instruction":{"text":"Save the explicitly supplied field value"}})));
    Ok((saved_input, binding))
}

struct GateSource<'a>(
    &'a mut dyn edu_academy_core::workflow::service::Source,
    &'a mut Js,
);
impl edu_academy_core::workflow::service::Source for GateSource<'_> {
    fn read(&mut self, scope: &Js, request: &Js, subject: &Js) -> Result<Js> {
        let result = self.0.read(scope, request, subject);
        if let Err(error) = &result
            && error.message == "journal_mutation_wire_not_qualified"
        {
            *self.1 = error.message.as_str().into();
        }
        result
    }
}

/// A mode runs once. The caller retains the owner lease across this entire call.
pub fn execute(input: &Js, service: &mut Service<'_>) -> Result<Js> {
    supported(input)?;
    match input.get("mode").code_units().as_str() {
        "plan" => Ok(prepare(input, service)?.1),
        "save" => {
            let (request, review) = prepare(input, service)?;
            if !review.get("status").is("prepared")
                || review.get("planDigest") != input.get("approval")
            {
                let mut result = stored(input, service.store)?;
                result.set("status", "refused".into());
                result.set("code", "approval_digest_mismatch".into());
                result.set("wireGate", review.get("wireGate").clone());
                return Ok(result);
            }
            let result = service.save(&request)?;
            let mut output = stored(input, service.store)?;
            output.set("status", result.get("status").clone());
            output.set("code", result.get("reasonCode").clone());
            Ok(output)
        }
        "status" => {
            if input.get("reconcile") == &Js::Bool(true) {
                let wire: serde_json::Value = serde_json::from_str(&canonical(input)?)
                    .map_err(|_| Fault::error("driver_input_rejected"))?;
                let context = j(json!({"schemaVersion":"academy.dailyrecord-user-context/1",
                    "purpose":"Read settlement status","currentRequestRef":wire["requestRef"],"scope":wire["scope"],
                    "selectedFields":[wire["field"]],"facts":[],"memoAnswer":{"kind":"no_addition","requestRef":wire["requestRef"]}}));
                service.reconcile(&obj(&[("context", context)]))?;
            }
            stored(input, service.store)
        }
        _ => Err(Fault::error("driver_mode_rejected")),
    }
}

pub fn stored(input: &Js, store: &Store) -> Result<Js> {
    use edu_academy_core::workflow::{state, store::Kind};
    let mut records = vec![];
    for execution in state::student_executions(store, input.get("scope"))? {
        let record = &execution.record;
        if record.get("field") != input.get("field")
            || record.get("scope").get("lessonDate") != input.get("scope").get("lessonDate")
        {
            continue;
        }
        let outcome = store.get(Kind::Outcome, &execution.key)?;
        let due = state::settlement_at(record, &outcome);
        records.push(j(json!({"executionState":execution.state.name(),
            "actionDigest":record.get("actionDigest").code_units(),"earliestSettleAtEpochMs":due})));
    }
    Ok(obj(&[
        ("field", input.get("field").clone()),
        ("executions", Js::Arr(records)),
    ]))
}

fn held(input: Js, wire: Js, blockers: Js) -> (Js, Js) {
    (
        input,
        obj(&[
            ("status", "held".into()),
            ("wireGate", wire),
            ("blockers", blockers),
        ]),
    )
}

/// Evidence from the exact immediate read returned by core http::apply.
/// A missing read is unknown; it is never labelled equal.
pub fn http_evidence(action: &Js, result: &Js) -> Js {
    let before = result.get("before").get("fields");
    let after = result.get("after").get("fields");
    let comparison = edu_academy_core::workflow::review::FIELDS.map(|field| {
        let expected = if action.get("field").is(field) {
            action.get("after")
        } else {
            before.get(field)
        };
        let equal = if after.get(field).absent() {
            Js::Null
        } else if after.get(field) == expected {
            "equal".into()
        } else {
            "not_equal".into()
        };
        (field, equal)
    });
    obj(&[
        (
            "httpStatus",
            result.get("receipt").get("httpStatus").clone(),
        ),
        (
            "bodySha256",
            result.get("receipt").get("bodySha256").clone(),
        ),
        ("comparison", obj(&comparison)),
    ])
}

/// Retains only public evidence while NativeBackend consumes the real port result.
pub struct EvidencePort<'a> {
    pub inner: &'a mut dyn edu_academy_core::workflow::native::SourcePort,
    pub evidence: Js,
}
impl edu_academy_core::workflow::native::SourcePort for EvidencePort<'_> {
    fn authenticate(&mut self) -> Result<()> {
        self.inner.authenticate()
    }
    fn discover_groups(&mut self, input: &Js) -> Result<Js> {
        self.inner.discover_groups(input)
    }
    fn execute(&mut self, input: &Js) -> Result<Js> {
        self.inner.execute(input)
    }
    fn journal_state(&mut self, target: &Js) -> Result<Js> {
        self.inner.journal_state(target)
    }
    fn apply(
        &mut self,
        action: &Js,
        approval: &mut dyn edu_academy_core::workflow::approval::Approval,
    ) -> Result<Js> {
        let result = self.inner.apply(action, approval)?;
        self.evidence = http_evidence(action, &result);
        Ok(result)
    }
}
