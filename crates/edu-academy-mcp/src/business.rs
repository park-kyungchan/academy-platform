//! Borrowed, explicit DailyRecord business connection. No native factory or defaults.
use crate::{Error, Js, Result, audit::Effect, obj, release::ReleaseGraph};
use edu_academy_core::{
    js::{Fault, Result as CoreResult, trim},
    workflow::{
        read_failure,
        service::{self, Service},
    },
};
use edu_academy_sources::{
    http::Transport,
    native::{self, StageError},
    owner_workflow::{CredentialProvider, Owner, SourceView},
};

pub const TOOLS: [&str; 7] = [
    "academy_dailyrecord_intake",
    "academy_dailyrecord_review",
    "academy_dailyrecord_save",
    "academy_dailyrecord_reconcile",
    "academy_dailyrecord_execution_status",
    "academy_student_resolve",
    "academy_runtime_read_admission",
];

/// Caller builds the actual Service from explicitly owned Store/Source/Policy/
/// Backend/Runtime, and lends it alongside the accepted Owner for routing/search.
/// In production its source and writer MUST be distinct views of this same Owner,
/// sharing its Client/cache/runtime/admissions. Synthetic Source ports can test
/// Service algorithms but cannot qualify native ownership or the public HTTP wire.
pub struct DailyRecord<'a, 'client, T: Transport, C: CredentialProvider> {
    service: Service<'a>,
    resolver: Option<SourceView<'client, T, C>>,
}
impl<'a, 'client, T: Transport, C: CredentialProvider> DailyRecord<'a, 'client, T, C> {
    pub fn from_borrowed_service(service: Service<'a>, owner: &Owner<'client, T, C>) -> Self {
        Self {
            service,
            resolver: Some(owner.source()),
        }
    }
    /// Explicit absence, not a default owner factory. Only a graph pinned to
    /// JSON null admission can bind this driver. No source request is inferred.
    pub fn from_borrowed_service_without_read_owner(service: Service<'a>) -> Self {
        Self {
            service,
            resolver: None,
        }
    }
    fn routing_admission(&self) -> CoreResult<Js> {
        self.resolver
            .as_ref()
            .map_or(Ok(Js::Null), SourceView::read_routing_admission)
    }
}

// Crate-private: applications can lend only the concrete actual-owner driver,
// never an arbitrary callback claiming admission, success or native ownership.
pub(crate) trait Business {
    fn check_binding(&self, graph: &ReleaseGraph) -> Result<()>;
    fn validate(&self, tool: &str, input: &Js) -> CoreResult<Js>;
    fn call(&mut self, tool: &str, input: &Js) -> CoreResult<Js>;
}
impl<T: Transport, C: CredentialProvider> Business for DailyRecord<'_, '_, T, C> {
    fn check_binding(&self, graph: &ReleaseGraph) -> Result<()> {
        if graph.owner.kind != "fictional" || !graph.endpoints.contains_key("dailyrecord") {
            return Err(Error::new("business_owner_binding_unqualified"));
        }
        // Store::open already checks symlinks and canonical owner root. Recheck
        // its actual location on every call, including reentry into a new session.
        let root = self.service.store.root();
        if !root.starts_with(&graph.owner.state_root)
            || !crate::roots::canonical_absolute(root)
            || std::fs::canonicalize(root).ok().as_deref() != Some(root)
        {
            return Err(Error::new("business_store_graph_mismatch"));
        }
        let admission = self
            .routing_admission()
            .map_err(|_| Error::new("business_admission_unavailable"))?;
        // JSON.stringify ordering/UTF16 semantics, not canonical sorted JSON.
        // The graph's pinned bytes must be the exact routing-admission JSON.
        let digest = crate::digest(&crate::encode(&admission)?);
        if !graph.routing.get("admissionSha256").is(&digest)
            || crate::encode(&graph.admission()?)? != crate::encode(&admission)?
        {
            return Err(Error::new("release_upstream_admission_mismatch"));
        }
        Ok(())
    }
    fn validate(&self, tool: &str, input: &Js) -> CoreResult<Js> {
        let schema = match tool {
            "academy_dailyrecord_intake" => "dailyRecordIntakeSchema",
            "academy_dailyrecord_review" => "dailyRecordReviewSchema",
            "academy_dailyrecord_save" => "dailyRecordSaveSchema",
            "academy_dailyrecord_reconcile" => "dailyRecordReconcileSchema",
            "academy_dailyrecord_execution_status" => "dailyRecordExecutionStatusSchema",
            "academy_student_resolve" => return resolve_input(input),
            "academy_runtime_read_admission" => {
                return if matches!(input, Js::Obj(_)) && input.props().is_empty() {
                    Ok(obj(&[]))
                } else {
                    Err(Fault::error("tool_input_invalid"))
                };
            }
            _ => return Err(Fault::error("unknown_tool")),
        };
        edu_academy_core::workflow::schema::validate(schema, input)
    }
    fn call(&mut self, tool: &str, input: &Js) -> CoreResult<Js> {
        match tool {
            "academy_runtime_read_admission" => {
                let admission = self.routing_admission()?;
                if admission.absent() {
                    return Ok(obj(&[
                        ("status", "unavailable".into()),
                        ("admissionSha256", Js::Null),
                    ]));
                }
                let digest = edu_academy_core::js::stringify(&admission)
                    .map(|s| crate::digest(s.as_bytes()))?;
                Ok(obj(&[
                    ("status", "configured".into()),
                    ("admissionSha256", digest.into()),
                ]))
            }
            "academy_student_resolve" => {
                let mut base = obj(&[
                    ("purpose", input.get("purpose").clone()),
                    ("currentRequestRef", input.get("currentRequestRef").clone()),
                    ("displayName", input.get("displayName").clone()),
                    ("academyWrites", Js::Num(0.0)),
                    ("parentSends", Js::Num(0.0)),
                    ("automaticRetryAllowed", false.into()),
                    ("sourceContentIsUntrustedData", true.into()),
                    ("execution", "read_only".into()),
                ]);
                // Let the existing resolver decide trim/UTF16 validity before
                // touching the actual source. Never stringify a lone surrogate.
                let Some(resolver) = self.resolver.as_mut() else {
                    base.set("status", "unavailable".into());
                    base.set(
                        "reasonCode",
                        "student_resolve_backend_not_configured".into(),
                    );
                    return Ok(base);
                };
                let probe = native::resolve_name(input.get("displayName"), &Js::Arr(vec![]));
                let resolved = if probe.get("calls") == &Js::Num(0.0) {
                    probe.get("value").clone()
                } else {
                    let exact = Js::Str(trim(input.get("displayName").units()).to_vec());
                    match resolver.search_students_by_name(
                        &exact,
                        Some(input.get("currentRequestRef")),
                        input.has("lessonDate").then(|| input.get("lessonDate")),
                    ) {
                        Ok(rows) => native::resolve_name(&exact, &rows).get("value").clone(),
                        Err(StageError::StudentNameNotAdmitted) => obj(&[
                            ("status", "unavailable".into()),
                            ("reasonCode", "student_name_not_admitted".into()),
                        ]),
                        Err(error) => {
                            let failure = read_failure::public_read_failure(
                                &error.into_fault(),
                                "student_resolve_search_unqualified_no_automatic_retry",
                            );
                            obj(&[
                                ("status", "unavailable".into()),
                                ("reasonCode", failure.get("code").clone()),
                                ("failure", failure),
                            ])
                        }
                    }
                };
                for (key, value) in resolved.props() {
                    // Keys originate only in the existing resolver/failure projection.
                    base.set(&String::from_utf16_lossy(key), value.clone());
                }
                Ok(base)
            }
            "academy_dailyrecord_execution_status" => service::status(self.service.store, input),
            "academy_dailyrecord_intake" => self.service.intake(input),
            "academy_dailyrecord_review" => self.service.review(input, true),
            "academy_dailyrecord_save" => self.service.save(input),
            "academy_dailyrecord_reconcile" => self.service.reconcile(input),
            _ => Err(Fault::error("unknown_tool")),
        }
    }
}

// Original studentResolveInputSchema: these strings are not trimmed by schema.
// Matching/trim/strict-row decisions remain exclusively native::resolve_name.
fn resolve_input(input: &Js) -> CoreResult<Js> {
    let reject = || Fault::error("tool_input_invalid");
    if !matches!(input, Js::Obj(_))
        || input.props().iter().any(|(k, _)| {
            !["purpose", "currentRequestRef", "displayName", "lessonDate"]
                .iter()
                .any(|s| k == &crate::text(s))
        })
    {
        return Err(reject());
    }
    for (key, max) in [
        ("purpose", 256),
        ("currentRequestRef", 256),
        ("displayName", 200),
    ] {
        let value = input.get(key);
        if !value.string() || value.units().is_empty() || value.units().len() > max {
            return Err(reject());
        }
    }
    if input.has("lessonDate") {
        edu_academy_core::schema::validate("dailyRecordDateSchema", input.get("lessonDate"))?;
    }
    Ok(input.clone())
}

/// SDK2.3.1 handler exceptions are text-only tool errors, without a fabricated
/// structuredContent/failure class. Source catches project typed errors earlier.
pub(crate) fn tool_error(message: &str) -> Js {
    obj(&[
        (
            "content",
            Js::Arr(vec![obj(&[
                ("type", "text".into()),
                ("text", message.into()),
            ])]),
        ),
        ("isError", true.into()),
    ])
}
/// These calls may change durable local workflow state even without Academy I/O.
/// Once save was entered, no output/graph failure establishes zero Academy writes.
pub(crate) fn effect(tool: &str) -> Effect {
    if tool == "academy_dailyrecord_save" {
        Effect::Possible
    } else {
        Effect::ReadbackConfirmed { academy_writes: 0 }
    }
}

/// Separate history-only source loan; never substitutes the seven-route driver.
/// Only fictional owner graphs qualify in this source slice. No native factory.
pub struct History<P: edu_academy_sources::history::Port> {
    source: edu_academy_sources::history::History<P>,
}
impl<P: edu_academy_sources::history::Port> History<P> {
    pub fn from_source(source: edu_academy_sources::history::History<P>) -> Self {
        Self { source }
    }
    pub(crate) fn check_history_binding(&self, graph: &ReleaseGraph, endpoint: &str) -> Result<()> {
        if graph.owner.kind != "fictional" {
            return Err(Error::new("business_owner_binding_unqualified"));
        }
        let admission = self.source.routing_admission();
        if crate::encode(&graph.admission()?)? != crate::encode(admission)?
            || !graph
                .routing
                .get("admissionSha256")
                .is(&crate::digest(&crate::encode(admission)?))
        {
            return Err(Error::new("release_upstream_admission_mismatch"));
        }
        if ["paper1", "paper2"].contains(&endpoint) {
            let declared = graph
                .routing
                .get("paperScopes")
                .array()
                .iter()
                .find(|s| s.get("endpoint").is(endpoint));
            if !declared
                .is_some_and(|s| s.get("studentIds").array() == self.source.paper_students())
            {
                return Err(Error::new("paper_scope_unresolved"));
            }
        }
        Ok(())
    }
    pub(crate) fn history_call(&mut self, tool: &str, input: &Js) -> CoreResult<Js> {
        if tool == "academy_runtime_read_admission" {
            return Ok(obj(&[
                ("status", "configured".into()),
                (
                    "admissionSha256",
                    crate::digest(
                        &crate::encode(self.source.routing_admission())
                            .map_err(|e| Fault::error(e.code))?,
                    )
                    .into(),
                ),
            ]));
        }
        let metadata = tool == "academy_student_metadata_get";
        let result = if metadata {
            self.source.metadata(input)
        } else {
            self.source.headers(input)
        };
        // Cap/serialization failures belong to the same source envelope as a
        // failed read, so the parent retains source identity and measured calls.
        let result = result
            .map_err(edu_academy_sources::data::fault_from_error)
            .and_then(|value| {
                let bytes = crate::audit::tool_result(value.clone())
                    .and_then(|v| crate::encode(&v))
                    .map_err(|e| Fault::error(e.code))?;
                if bytes.len() > 512 * 1024 {
                    return Err(Fault::error(if metadata {
                        "metadata_result_too_large"
                    } else {
                        "source_headers_read_failed"
                    }));
                }
                Ok(value)
            });
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                let failure = read_failure::public_read_failure(
                    &error,
                    if metadata {
                        "student_metadata_unqualified_no_automatic_retry"
                    } else {
                        "source_headers_read_failed"
                    },
                );
                let mut out = obj(&[
                    (
                        "status",
                        if metadata { "unavailable" } else { "blocked" }.into(),
                    ),
                    (
                        if metadata { "reasonCode" } else { "code" },
                        failure.get("code").clone(),
                    ),
                    ("failure", failure),
                    ("sourceRequests", self.source.source_requests().into()),
                    ("automaticRetryAllowed", false.into()),
                ]);
                out.set("academyWrites", Js::Num(0.0));
                out.set("parentSends", Js::Num(0.0));
                Ok(out)
            }
        }
    }
}
