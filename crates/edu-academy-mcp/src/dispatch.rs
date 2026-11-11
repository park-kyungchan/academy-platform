//! Product occurrences/receipts and native dispatch; never a workflow journal.
use crate::{
    Error, Js, Result,
    audit::{self, Effect},
    endpoint, obj,
    process::{self, Context, Control, Limits},
    release::{ConnectionGate, ReleaseGraph},
};
use std::time::Instant;
pub enum Reply {
    Tool(Js),
    ReceiptFailure { occurrence: String, effect: Effect },
}
/// Names this dispatcher handles itself (identity, the parent-get and
/// daily-test compositions) instead of forwarding to an endpoint child.
/// Read-only: the match sites in `call` remain the behaviour.
pub fn dispatch_adapter_names() -> [&'static str; 3] {
    [
        "academy_runtime_identity",
        "academy_dailyrecord_get",
        "academy_dailytest_history_with_headers",
    ]
}
fn workflow(name: &str) -> bool {
    matches!(
        name,
        "academy_dailyrecord_intake"
            | "academy_dailyrecord_review"
            | "academy_dailyrecord_save"
            | "academy_dailyrecord_execution_status"
            | "academy_dailyrecord_reconcile"
            | "academy_cism_weekly_review"
            | "academy_cism_weekly_save"
    )
}
fn forwarded(args: &Js, name: &str, client: Option<&Js>, graph: &ReleaseGraph) -> Js {
    let mut payload = Js::Obj(
        args.props()
            .iter()
            .filter(|(key, _)| key != &crate::text("attribution"))
            .cloned()
            .collect(),
    );
    if workflow(name) {
        payload.set(
            "attribution",
            audit::observed_attribution(args, client, &graph.pointer.bridge.sha256),
        );
    }
    payload
}
fn failure(error: Error) -> Js {
    let read = edu_academy_core::workflow::read_failure::unavailable(
        &edu_academy_core::js::Fault::error(error.code),
    );
    if read.get("reasonCode").is(error.code) {
        return read.get("failure").clone();
    }
    obj(&[
        ("code", error.code.into()),
        ("category", "upstream_unavailable".into()),
        ("evidenceLocator", "academy-mcp:rust-native-dispatch".into()),
        ("automaticRetryAllowed", false.into()),
    ])
}
fn upstream_occurrence(result: &Js) -> Js {
    let meta = result.get("_meta").get("academy/audit").get("occurrence");
    let value = if meta.absent() {
        result.get("structuredContent").get("occurrence")
    } else {
        meta
    };
    let units = value.units();
    if value.string()
        && units.len() == 36
        && units.iter().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                *c == u16::from(b'-')
            } else {
                (u16::from(b'0')..=u16::from(b'9')).contains(c)
                    || (u16::from(b'a')..=u16::from(b'f')).contains(c)
            }
        })
    {
        value.clone()
    } else {
        Js::Null
    }
}
fn upstream_failure(result: &Js) -> Option<Js> {
    let raw = result.get("structuredContent");
    let code = if raw.get("failure").get("code").string() {
        raw.get("failure").get("code")
    } else if raw.get("code").string() {
        raw.get("code")
    } else {
        raw.get("reasonCode")
    };
    let safe = edu_academy_core::workflow::read_failure::safe_reason(code);
    if &safe == code && !safe.is("source_read_unavailable") && code.string() {
        return Some(
            edu_academy_core::workflow::read_failure::unavailable(
                &edu_academy_core::js::Fault::error(code.code_units()),
            )
            .get("failure")
            .clone(),
        );
    }
    let content = result.get("content").array();
    if content.len() == 1
        && content[0].get("type").is("text")
        && content[0]
            .get("text")
            .code_units()
            .starts_with("Input validation error:")
    {
        return Some(failure(Error::new("tool_input_invalid")));
    }
    None
}
// This boundary mirrors one RPC invocation. Keeping the independent session,
// caller attribution and cancellation ports explicit avoids hiding ownership in
// a second aggregate context alongside process::Context.
#[expect(
    clippy::too_many_arguments,
    reason = "Explicit RPC/session/ownership ports at the dispatch boundary"
)]
pub fn call(
    graph: &ReleaseGraph,
    gate: &mut ConnectionGate,
    name: &str,
    args: &Js,
    client: Option<&Js>,
    request_id: &Js,
    control: &mut dyn Control,
    limits: Limits,
) -> Result<Reply> {
    let limits = limits.validate()?;
    let occurrence = audit::occurrence()?;
    let started = Instant::now();
    let mut effect = Effect::NotDispatched;
    let tool = if name == "academy_runtime_identity" || graph.registry.get(name).is_ok() {
        name
    } else {
        "unrecognized_tool"
    };
    let mut phase = "release";
    let mut endpoint_name: Option<String> = None;
    let mut upstream_name: Option<String> = None;
    let mut failure_metadata = None;
    let mut public_upstream_failure = None;
    let mut parent_get = false;
    let mut steps = Vec::new();
    let mut intake_issued = false;
    let mut observation_handle = Js::Null;
    let dispatch = obj(&[
        ("issuedAt", audit::wall_time().into()),
        ("tool", tool.into()),
        (
            "argumentsSha256",
            crate::digest(&crate::encode(args)?).into(),
        ),
        ("automaticRetryAllowed", false.into()),
        ("academyWrites", Js::Num(0.0)),
        ("parentSends", Js::Num(0.0)),
    ]);
    if graph
        .receipt(&format!("{occurrence}-dispatch.json"), &dispatch)
        .is_err()
    {
        return Ok(Reply::ReceiptFailure { occurrence, effect });
    }
    let work = (|| -> Result<Js> {
        gate.check(graph.verify_unchanged())?;
        if name == "academy_runtime_identity" {
            if !args.props().is_empty() {
                return Err(Error::new("tool_input_invalid"));
            }
            return audit::tool_result(graph.identity());
        }
        let registered = graph.registry.get(name)?;
        phase = "input_validation";
        crate::validation::validate(registered.tool.get("inputSchema"), args)?;
        phase = "routing";
        let entry = crate::routing::select(&graph.routing, &graph.registry, registered, args)?;
        endpoint_name = Some(entry.endpoint.clone());
        upstream_name = Some(entry.upstream_name.clone());
        phase = "upstream";
        if entry.upstream_name == "academy_dailytest_history_with_headers" {
            let result = crate::dailytest_history::call(
                graph,
                &mut *gate,
                args,
                request_id,
                &mut *control,
                limits,
                started,
                &occurrence,
                &mut failure_metadata,
                &mut public_upstream_failure,
            )?;
            phase = "release_readback";
            return Ok(result);
        }
        parent_get = entry.upstream_name == "academy_dailyrecord_get";
        let prepared = if parent_get {
            let prepared = crate::dailyrecord_read::prepare(
                args,
                &crate::upstream::read_admission(&graph.admission()?),
                crate::dailyrecord_read::wall_epoch_ms(),
            )?;
            if let Some(held) = prepared.held() {
                phase = "release_readback";
                return audit::tool_result(held.clone());
            }
            Some(prepared)
        } else {
            None
        };
        let plan = endpoint::plan(graph, &entry.endpoint)?;
        let mut context = Context::new(
            graph,
            &mut *gate,
            &mut *control,
            request_id,
            started + limits.work,
        );
        let payload = forwarded(args, &entry.upstream_name, client, graph);
        let supervised = process::supervise(&plan, &mut context, limits, |child, context| {
            let mut upstream = crate::upstream::Client::default();
            if let Err(error) = upstream.connect(child, context, &entry.endpoint) {
                failure_metadata = upstream.failure_metadata;
                return Err(error);
            }
            if let Some(prepared) = prepared {
                let data = crate::dailyrecord_read::read(
                    prepared,
                    |step, input| {
                        let response = upstream
                            .call(
                                child,
                                context,
                                step,
                                forwarded(input, step, client, graph),
                                || {
                                    if step == "academy_dailyrecord_intake" {
                                        intake_issued = true;
                                    }
                                },
                            )
                            .map_err(crate::dailyrecord_read::InvokeFailure::Transport)?;
                        if response.get("isError").truthy()
                            || !response.get("structuredContent").truthy()
                        {
                            return Err(crate::dailyrecord_read::tool_failure(&response));
                        }
                        let mut data = response.get("structuredContent").clone();
                        let safe = crate::dailyrecord_read::safe_occurrence(
                            response.get("_meta").get("academy/audit").get("occurrence"),
                        );
                        let mut evidence = obj(&[
                            ("tool", step.into()),
                            (
                                "status",
                                edu_academy_core::workflow::nullable(data.get("status")),
                            ),
                            (
                                "sourceAvailability",
                                edu_academy_core::workflow::nullable(
                                    data.get("sourceAvailability"),
                                ),
                            ),
                        ]);
                        if safe.truthy() {
                            evidence.set("upstreamOccurrence", safe.clone());
                        }
                        steps.push(evidence);
                        if step == "academy_dailyrecord_intake"
                            && data.get("observationPersistence").is("owner_durable")
                        {
                            observation_handle = data.get("observationHandle").clone();
                        }
                        if data.get("status").is("unavailable")
                            && !data.get("failure").truthy()
                            && response
                                .get("_meta")
                                .get("academy/audit")
                                .get("failure")
                                .truthy()
                        {
                            data.set(
                                "failure",
                                crate::dailyrecord_read::public_failure_from_data(
                                    response.get("_meta").get("academy/audit").get("failure"),
                                ),
                            );
                        }
                        if safe.truthy() {
                            data.set("upstreamOccurrence", safe);
                        }
                        Ok(data)
                    },
                    crate::dailyrecord_read::wall_epoch_ms,
                )?;
                // Caught business/transport failures cannot hide deadline,
                // cancellation or frozen-graph failure. Cleanup still runs.
                context.verify()?;
                context.check()?;
                return audit::tool_result(data);
            }
            upstream.call(child, context, &entry.upstream_name, payload, || {
                if matches!(
                    entry.upstream_name.as_str(),
                    "academy_dailyrecord_save" | "academy_cism_weekly_save"
                ) {
                    effect = Effect::Possible;
                }
            })
        });
        graph.receipt(
            &format!("{occurrence}-cleanup.json"),
            &supervised.cleanup.evidence(),
        )?;
        let result = supervised.result?;
        if result.get("isError").truthy() {
            let code = result.get("structuredContent").get("code");
            failure_metadata = Some(obj(&[("upstreamOccurrence", upstream_occurrence(&result))]));
            public_upstream_failure = upstream_failure(&result);
            return Err(Error::new(
                if code.is("configured_rust_route_unimplemented") {
                    "configured_rust_route_unimplemented"
                } else if code.is("workflow_source_port_unqualified") {
                    "workflow_source_port_unqualified"
                } else {
                    "upstream_tool_error"
                },
            ));
        }
        phase = "release_readback";
        Ok(result)
    })();
    // Every outcome traverses final readback; retain its write stage on mismatch.
    let work = match gate.check(graph.verify_unchanged()) {
        Ok(()) => work,
        Err(e) => Err(e),
    };
    let elapsed = Js::Num(started.elapsed().as_millis() as f64);
    let endpoint = endpoint_name.map_or(Js::Null, Into::into);
    let upstream = upstream_name.map_or(Js::Null, Into::into);
    match work {
        Ok(result) => {
            let mut receipt = obj(&[
                ("occurrence", occurrence.clone().into()),
                ("phase", phase.into()),
                ("endpoint", endpoint.clone()),
                ("upstreamName", upstream.clone()),
                ("generation", graph.pointer.generation.clone().into()),
                ("pointerSha256", graph.pointer_sha256.clone().into()),
                ("elapsedMs", elapsed.clone()),
                ("completedAt", audit::wall_time().into()),
                ("tool", tool.into()),
                ("isError", result.get("isError").truthy().into()),
                (
                    "status",
                    edu_academy_core::workflow::nullable(
                        result.get("structuredContent").get("status"),
                    ),
                ),
                (
                    "sourceAvailability",
                    edu_academy_core::workflow::nullable(
                        result.get("structuredContent").get("sourceAvailability"),
                    ),
                ),
                ("upstreamOccurrence", upstream_occurrence(&result)),
                (
                    "resultSha256",
                    crate::digest(&crate::encode(&result)?).into(),
                ),
                (
                    "academyWrites",
                    if matches!(effect, Effect::NotDispatched) {
                        Js::Num(0.0)
                    } else {
                        Js::Null
                    },
                ),
                ("parentSends", Js::Num(0.0)),
                ("automaticRetryAllowed", false.into()),
            ]);
            if parent_get {
                receipt.set("steps", Js::Arr(steps.clone()));
                let data = result.get("structuredContent");
                receipt.set(
                    "upstreamOccurrence",
                    crate::dailyrecord_read::safe_occurrence(data.get("upstreamOccurrence")),
                );
                receipt.set(
                    "failure",
                    if data.get("failure").truthy() {
                        crate::dailyrecord_read::public_failure_from_data(data.get("failure"))
                    } else {
                        Js::Null
                    },
                );
                receipt.set(
                    "localWorkflowEffects",
                    local_observation(intake_issued, &observation_handle),
                );
            }
            if graph
                .receipt(&format!("{occurrence}-result.json"), &receipt)
                .is_err()
            {
                return Ok(Reply::ReceiptFailure { occurrence, effect });
            }
            if name == "academy_runtime_identity" {
                return Ok(Reply::Tool(result));
            }
            Ok(Reply::Tool(audit::with_tool_meta(
                &result,
                &obj(&[(
                    "academy/bridgeAudit",
                    obj(&[
                        ("occurrence", occurrence.into()),
                        ("endpoint", endpoint),
                        ("upstreamName", upstream),
                        ("generation", graph.pointer.generation.clone().into()),
                        ("elapsedMs", elapsed),
                    ]),
                )]),
            )?))
        }
        Err(error) => {
            let public_failure = if error.code == "upstream_tool_error" {
                public_upstream_failure.unwrap_or_else(|| failure(error))
            } else {
                failure(error)
            };
            let mut data = obj(&[
                ("status", "blocked".into()),
                ("code", public_failure.get("code").clone()),
                ("failure", public_failure),
                ("occurrence", occurrence.clone().into()),
                ("phase", phase.into()),
                ("endpoint", endpoint),
                ("upstreamName", upstream),
                ("elapsedMs", elapsed),
                ("automaticRetryAllowed", false.into()),
                ("parentSends", Js::Num(0.0)),
            ]);
            if let Some(metadata) = failure_metadata {
                for (key, value) in metadata.props() {
                    data.set(&String::from_utf16_lossy(key), value.clone());
                }
            }
            for (key, value) in effect.failure_evidence().props() {
                data.set(&String::from_utf16_lossy(key), value.clone());
            }
            if parent_get {
                data.set("steps", Js::Arr(steps));
                data.set(
                    "localWorkflowEffects",
                    local_observation(intake_issued, &observation_handle),
                );
            }
            let mut receipt = data.clone();
            receipt.set("observedAt", audit::wall_time().into());
            receipt.set("tool", tool.into());
            if graph
                .receipt(&format!("{occurrence}-failure.json"), &receipt)
                .is_err()
            {
                return Ok(Reply::ReceiptFailure { occurrence, effect });
            }
            let mut result = audit::tool_result(data)?;
            result.set("isError", true.into());
            Ok(Reply::Tool(result))
        }
    }
}

// Intake is read-only at Academy but may publish a durable local observation
// before a transport/audit/output failure. Never retry or erase that evidence.
fn local_observation(issued: bool, handle: &Js) -> Js {
    obj(&[
        ("intakeIssued", issued.into()),
        (
            "observationHandle",
            if handle.string() {
                handle.clone()
            } else {
                Js::Null
            },
        ),
        (
            "reconciliation",
            if issued {
                "inspect_owner_store_no_automatic_retry"
            } else {
                "not_issued"
            }
            .into(),
        ),
    ])
}
