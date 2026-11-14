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
pub fn store(
    root: &std::path::Path,
    directory: &std::path::Path,
    runtime: &dyn edu_academy_core::workflow::Runtime,
) -> Result<Store> {
    use std::os::unix::fs::PermissionsExt;
    if directory.ancestors().any(|p| p.join(".git").exists()) {
        return Err(Fault::error("store_inside_git_refused"));
    }
    if let Ok(metadata) = std::fs::symlink_metadata(directory)
        && metadata.is_dir()
        && metadata.permissions().mode() & 0o077 != 0
    {
        return Err(Fault::error("store_mode_refused"));
    }
    if std::fs::symlink_metadata(directory).is_ok() {
        let mut pending = vec![directory.to_path_buf()];
        while let Some(path) = pending.pop() {
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|_| Fault::error("store_tree_refused"))?;
            if metadata.is_symlink()
                || (!metadata.is_dir() && !metadata.is_file())
                || (metadata.is_dir() && metadata.permissions().mode() & 0o077 != 0)
            {
                return Err(Fault::error("store_tree_refused"));
            }
            if metadata.is_dir() {
                for entry in
                    std::fs::read_dir(path).map_err(|_| Fault::error("store_tree_refused"))?
                {
                    pending.push(
                        entry
                            .map_err(|_| Fault::error("store_tree_refused"))?
                            .path(),
                    );
                }
            }
        }
    }
    edu_academy_ontology::open_store(directory, root, runtime)
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

struct ReconcileSource<'a>(
    &'a mut dyn edu_academy_core::workflow::service::Source,
    [usize; 2],
);
impl edu_academy_core::workflow::service::Source for ReconcileSource<'_> {
    fn read(&mut self, scope: &Js, request: &Js, subject: &Js) -> Result<Js> {
        self.1[0] += 1;
        let result = self.0.read(scope, request, subject);
        self.1[1] += usize::from(
            !result
                .as_ref()
                .is_ok_and(|v| v.get("status").is("observed")),
        );
        result
    }
}

/// A mode runs once. The caller retains the owner lease across this entire call.
pub fn execute(input: &Js, service: &mut Service<'_>) -> Result<Js> {
    supported(input)?;
    match input.get("mode").code_units().as_str() {
        "plan" => Ok(prepare(input, service)?.1),
        "save" => {
            let (request, mut review) = prepare(input, service)?;
            if !review.get("status").is("prepared") {
                review.set("code", refusal_code(&review));
                return Ok(review);
            }
            if review.get("planDigest") != input.get("approval") {
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
            output.set("wireGate", review.get("wireGate").clone());
            Ok(output)
        }
        "status" => {
            let mut reconciled = Js::Null;
            let mut counts = [Js::Null, Js::Null];
            if input.get("reconcile") == &Js::Bool(true) {
                let wire: serde_json::Value = serde_json::from_str(&canonical(input)?)
                    .map_err(|_| Fault::error("driver_input_rejected"))?;
                let context = j(json!({"schemaVersion":"academy.dailyrecord-user-context/1",
                    "purpose":"Read settlement status","currentRequestRef":wire["requestRef"],"scope":wire["scope"],
                    "selectedFields":[wire["field"]],"facts":[],"memoAnswer":{"kind":"no_addition","requestRef":wire["requestRef"]}}));
                let mut source = ReconcileSource(service.source, [0, 0]);
                let result = Service {
                    store: service.store,
                    source: &mut source,
                    backend: service.backend,
                    policy: service.policy,
                    runtime: service.runtime,
                }
                .reconcile(&obj(&[("context", context)]));
                counts = source.1.map(|n| Js::Num(n as f64));
                reconciled = result.unwrap_or_else(|error| {
                    obj(&[
                        ("status", "failed".into()),
                        ("reasonCode", code(&error.message).into()),
                    ])
                });
            }
            let mut output = stored(input, service.store)?;
            output.set("reconcileStatus", code_value(reconciled.get("status")));
            output.set("reconcileCode", code_value(reconciled.get("reasonCode")));
            output.set("reconcileReads", counts[0].clone());
            output.set("reconcileReadsFailed", counts[1].clone());
            Ok(output)
        }
        _ => Err(Fault::error("driver_mode_rejected")),
    }
}

/// Code-only public boundary shared by the binary and the synthetic core tests.
fn code(s: &str) -> &str {
    if !s.is_empty()
        && s.len() <= 120
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        s
    } else {
        "driver_failed"
    }
}
fn code_value(value: &Js) -> Js {
    if value.absent() {
        Js::Null
    } else {
        code(&value.code_units()).into()
    }
}
fn refusal_code(value: &Js) -> Js {
    if !value.get("code").absent() {
        return code_value(value.get("code"));
    }
    if !value.get("wireGate").absent() && !value.get("wireGate").is("passed") {
        return code_value(value.get("wireGate"));
    }
    value
        .get("blockers")
        .array()
        .first()
        .map_or_else(|| "review_held".into(), code_value)
}
pub fn response(
    input: &Js,
    result: Result<Js>,
    stored: Js,
    evidence: &Js,
    called: bool,
) -> (Js, i32) {
    let failed = |v: &Js| {
        ["held", "refused", "failed", "unknown", "pending"]
            .iter()
            .any(|s| v.is(s))
    };
    let status_state = result.as_ref().unwrap_or(&stored);
    let conflict = input.get("mode").is("status")
        && status_state
            .get("executions")
            .array()
            .iter()
            .any(|execution| execution.get("executionState").is("reconciled_conflict"));
    let reason = match &result {
        _ if conflict => "reconciled_conflict".into(),
        Err(error) => code(&error.message).into(),
        Ok(value) if failed(value.get("status")) => refusal_code(value),
        Ok(value)
            if !(value.get("reconcileStatus").absent()
                || value.get("reconcileStatus").is("settled")
                || value.get("reconcileStatus").is("held")
                    && value.get("reconcileCode").is("save_confirmation_pending")) =>
        {
            code_value(value.get("reconcileCode"))
        }
        _ => Js::Null,
    };
    if reason.absent() {
        return (result.unwrap_or(Js::Null), 0);
    }
    let value = result.as_ref().ok();
    let mut output = obj(&[
        ("status", if called { "failed" } else { "refused" }.into()),
        ("code", reason),
        ("stored", stored),
    ]);
    if input.get("mode").is("plan") || input.get("mode").is("save") {
        output.set(
            "wireGate",
            value.map_or(Js::Null, |v| code_value(v.get("wireGate"))),
        );
    }
    if input.get("mode").is("save") {
        for key in ["httpStatus", "bodySha256", "comparison"] {
            output.set(key, edu_academy_core::workflow::nullable(evidence.get(key)));
        }
    }
    if input.get("mode").is("status") {
        for key in ["reconcileStatus", "reconcileCode"] {
            output.set(key, value.map_or(Js::Null, |v| code_value(v.get(key))));
        }
        for key in ["reconcileReads", "reconcileReadsFailed"] {
            output.set(key, value.map_or(Js::Null, |v| v.get(key).clone()));
        }
    }
    (output, if called { 3 } else { 2 })
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

// ---- mode `get`: read one lesson record and write its values to a private file ----

/// What `get` needs from the owner: a name search and one record read. Reads only.
pub trait RecordSource {
    /// Pairs `{studentKey, studentName}` of the exact name search.
    fn search_name(&mut self, name: &Js) -> Result<Js>;
    fn read_record(&mut self, scope: &Js) -> Result<crate::owner_workflow::RecordRead>;
}
impl<T: crate::http::Transport, C: crate::owner_workflow::CredentialProvider> RecordSource
    for crate::owner_workflow::SourceView<'_, T, C>
{
    fn search_name(&mut self, name: &Js) -> Result<Js> {
        self.search_students_by_name(name, None, None)
            .map_err(crate::native::StageError::into_fault)
    }
    fn read_record(&mut self, scope: &Js) -> Result<crate::owner_workflow::RecordRead> {
        crate::owner_workflow::SourceView::read_record(self, scope)
            .map_err(crate::native::StageError::into_fault)
    }
}

/// The launch student a name stands for, decided from the launch admission alone,
/// before any request. None or several launch students of that name is a refusal.
pub fn launch_student_of_name(launch: &Js, name: &Js) -> Result<Js> {
    let mut found = launch
        .get("students")
        .array()
        .iter()
        .filter(|s| s.get("displayName") == name)
        .map(|s| s.get("studentId").clone());
    match (found.next(), found.next()) {
        (Some(id), None) => Ok(id),
        (None, _) => Err(Fault::error("student_name_not_admitted")),
        _ => Err(Fault::error("student_name_ambiguous")),
    }
}

/// Checks of `out` that need no request: absolute, absent, in a directory of mode
/// 0700 owned by the caller. Returns the directory.
pub fn check_out(out: &std::path::Path) -> Result<std::path::PathBuf> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty());
    let (Some(parent), Some(_)) = (parent, out.file_name()) else {
        return Err(Fault::error("invalid_input"));
    };
    if !out.is_absolute()
        || out
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(Fault::error("invalid_input"));
    }
    if std::fs::symlink_metadata(out).is_ok() {
        return Err(Fault::error("get_out_exists"));
    }
    let metadata =
        std::fs::symlink_metadata(parent).map_err(|_| Fault::error("get_out_directory_refused"))?;
    if !metadata.is_dir()
        || metadata.permissions().mode() & 0o777 != 0o700
        || metadata.uid() != rustix::process::geteuid().as_raw()
    {
        return Err(Fault::error("get_out_directory_refused"));
    }
    Ok(parent.to_path_buf())
}

/// Writes `bytes` to a temporary name beside `out` (mode 0600, never overwriting)
/// and renames it. Returns sha256 and byte count. Nothing is left on failure.
pub fn write_private(out: &std::path::Path, bytes: &[u8]) -> Result<(String, usize)> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let directory = check_out(out)?;
    let temporary = directory.join(format!(
        ".get-{}.tmp",
        std::fs::read_to_string("/proc/sys/kernel/random/uuid")
            .map_err(|_| Fault::error("occurrence_entropy_unavailable"))?
            .trim()
    ));
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if std::fs::symlink_metadata(out).is_ok() {
            return Err(std::io::Error::other("exists"));
        }
        std::fs::rename(&temporary, out)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(Fault::error("get_out_write_failed"));
    }
    Ok((sha(&String::from_utf8_lossy(bytes)), bytes.len()))
}

fn projected(field: &Js) -> Js {
    if field.get("state").is("known") {
        obj(&[
            ("state", "known".into()),
            ("value", field.get("value").clone()),
        ])
    } else {
        obj(&[
            ("state", "unknown".into()),
            ("reasonCode", field.get("reasonCode").clone()),
        ])
    }
}
/// Same typing as the source layer: a string value is known, anything else unknown.
fn typed(value: &Js) -> Js {
    if value.string() {
        obj(&[("state", "known".into()), ("value", value.clone())])
    } else {
        obj(&[
            ("state", "unknown".into()),
            ("reasonCode", "native_field_not_observed".into()),
        ])
    }
}
/// The file's document. The current record's six fields are the projection of the
/// observation the workflow modes use. The previous record's four row fields come
/// from its own row; its two assessments are not read by the source layer.
/// `transmissionState` and `toneSamples` are the observation's, unchanged.
pub fn record_document(read: &crate::owner_workflow::RecordRead) -> Result<Js> {
    let observation = &read.observation;
    let current_fields = observation.get("fields");
    let previous_fields = read.previous.get("fields");
    let mut record = obj(&[]);
    let mut previous = obj(&[]);
    for field in crate::native::FIELDS {
        record.set(field, projected(current_fields.get(field)));
        previous.set(
            field,
            if matches!(field, "dailyTestAssessment" | "previousHomeworkAssessment") {
                typed(&Js::Null)
            } else {
                typed(previous_fields.get(field))
            },
        );
    }
    let target = observation.get("target");
    let before = observation.get("previousTarget");
    Ok(obj(&[
        ("schemaVersion", "academy.dailyrecord-get/1".into()),
        ("studentId", target.get("studentId").clone()),
        ("lessonDate", target.get("lessonDate").clone()),
        ("observed_at", observation.get("observedAt").clone()),
        // Both exactly as the source layer carries them in the observation.
        (
            "transmissionState",
            observation.get("transmissionState").clone(),
        ),
        ("toneSamples", observation.get("toneSamples").clone()),
        (
            "record",
            obj(&[
                ("identity", target.clone()),
                ("date", target.get("lessonDate").clone()),
                ("fields", record),
            ]),
        ),
        (
            "previous",
            obj(&[
                ("identity", before.clone()),
                ("date", before.get("lessonDate").clone()),
                ("fields", previous),
            ]),
        ),
    ]))
}

/// Reads one lesson record from an already opened owner session and returns its
/// value (the document of `record_document`). Reads only; nothing is written and no
/// session is opened or closed here, so a long-lived process calls this once per
/// read over one session. `scope` carries the id the launch admission lists; `name`
/// is given when the student was named and must then resolve to exactly that id: the
/// day is read only after that.
pub fn get_record(source: &mut dyn RecordSource, scope: &Js, name: Option<&Js>) -> Result<Js> {
    if let Some(name) = name {
        let pairs = source.search_name(name)?;
        let mut keys: Vec<&Js> = vec![];
        for pair in pairs.array() {
            if !keys.contains(&pair.get("studentKey")) {
                keys.push(pair.get("studentKey"));
            }
        }
        match keys.as_slice() {
            [] => return Err(Fault::error("student_name_not_found")),
            [key] if *key == scope.get("studentId") => {}
            [_] => return Err(Fault::error("student_name_not_admitted")),
            _ => return Err(Fault::error("student_name_ambiguous")),
        }
    }
    record_document(&source.read_record(scope)?)
}

/// Writes a record value as the private file `out` and returns codes, counts and the
/// file's hash only. Called only with a value that was read completely.
pub fn write_record_file(document: &Js, out: &std::path::Path) -> Result<Js> {
    let mut bytes = canonical(document)?.into_bytes();
    bytes.push(b'\n');
    let counts = |fields: &Js| {
        let known = crate::native::FIELDS
            .iter()
            .filter(|f| fields.get(f).get("state").is("known"))
            .count();
        (known, crate::native::FIELDS.len() - known)
    };
    let (known, unknown) = counts(document.get("record").get("fields"));
    let (previous_known, previous_unknown) = counts(document.get("previous").get("fields"));
    let (digest, size) = write_private(out, &bytes)?;
    Ok(obj(&[
        ("status", "read".into()),
        ("mode", "get".into()),
        ("recordFieldsKnown", Js::Num(known as f64)),
        ("recordFieldsUnknown", Js::Num(unknown as f64)),
        ("previousFieldsKnown", Js::Num(previous_known as f64)),
        ("previousFieldsUnknown", Js::Num(previous_unknown as f64)),
        ("fileSha256", digest.into()),
        ("fileBytes", Js::Num(size as f64)),
    ]))
}

/// `get_record` then `write_record_file`: nothing is written unless the whole read
/// succeeded.
pub fn get(
    source: &mut dyn RecordSource,
    scope: &Js,
    name: Option<&Js>,
    out: &std::path::Path,
) -> Result<Js> {
    write_record_file(&get_record(source, scope, name)?, out)
}
