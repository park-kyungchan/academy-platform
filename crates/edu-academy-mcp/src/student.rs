//! Student metadata MCP adapters over the E09a source layer.
//! No re-parsing here: input shape is the registry schema, validated by the
//! protocol layer, and every source read is `StudentSource::metadata`.
use crate::{Error, Js, Result, obj, release::ReleaseGraph};
use edu_academy_core::{js::Fault, workflow::read_failure};
use edu_academy_sources::{
    data::fault_from_error, student_course::ReadPort, student_metadata::StudentSource,
};

pub const TOOLS: [&str; 2] = ["academy_student_metadata_get", "academy_student_metadata"];

pub fn is_student_tool(name: &str) -> bool {
    TOOLS.contains(&name)
}

/// Borrowed student driver. Only a fictional owner graph qualifies, exactly
/// as the DailyRecord and History loans; no native factory is provided.
pub struct StudentDriver<P: ReadPort> {
    source: StudentSource<P>,
    routing_admission: Js,
}

impl<P: ReadPort> StudentDriver<P> {
    pub fn new(source: StudentSource<P>, routing_admission: Js) -> Self {
        Self {
            source,
            routing_admission,
        }
    }
    pub fn source_requests(&self) -> usize {
        self.source.requests()
    }
    pub fn check_binding(&self, graph: &ReleaseGraph) -> Result<()> {
        if graph.owner.kind != "fictional" {
            return Err(Error::new("business_owner_binding_unqualified"));
        }
        let admission = graph.admission()?;
        if crate::encode(&admission)? != crate::encode(&self.routing_admission)?
            || !graph
                .routing
                .get("admissionSha256")
                .is(&crate::digest(&crate::encode(&self.routing_admission)?))
        {
            return Err(Error::new("release_upstream_admission_mismatch"));
        }
        Ok(())
    }
    /// One metadata call. Source refusals keep their exact code; the public
    /// envelope mirrors `History::history_call` (unavailable + failedSource).
    pub fn call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js> {
        if !is_student_tool(tool) {
            return Err(Fault::error("unknown_tool"));
        }
        let observed_at = crate::audit::wall_time();
        let result = self
            .source
            .metadata(tool, input, &observed_at)
            .map_err(fault_from_error)
            .and_then(|value| {
                let bytes = crate::audit::tool_result(value.clone())
                    .and_then(|v| crate::encode(&v))
                    .map_err(|e| Fault::error(e.code))?;
                if bytes.len() > 512 * 1024 {
                    return Err(Fault::error("metadata_result_too_large"));
                }
                Ok(value)
            });
        match result {
            Ok(value) => Ok(value),
            Err(error) => Ok(failure_envelope(&error, self.source.requests())),
        }
    }
}

fn failure_envelope(error: &Fault, source_requests: usize) -> Js {
    // As in the History handler (business.rs), the public code comes from
    // the catalogued failure projection, so failure.code == code always and
    // an uncatalogued upstream string never reaches the envelope.
    let failure =
        read_failure::public_read_failure(error, "student_metadata_unqualified_no_automatic_retry");
    let code = failure.get("code").clone();
    let mut out = obj(&[
        ("status", "unavailable".into()),
        ("reasonCode", code.clone()),
        ("code", code),
        ("failure", failure),
        ("failedSource", "metadata".into()),
        ("sourceRequests", source_requests.into()),
        ("automaticRetryAllowed", false.into()),
    ]);
    out.set("academyWrites", Js::Num(0.0));
    out.set("parentSends", Js::Num(0.0));
    out
}

/// Protocol-facing loan, so `RpcSession` never owns a source port directly.
pub trait StudentCall {
    fn check_student_binding(&self, graph: &ReleaseGraph) -> Result<()>;
    fn student_call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js>;
    fn student_source_requests(&self) -> usize;
}

impl<P: ReadPort> StudentCall for StudentDriver<P> {
    fn check_student_binding(&self, graph: &ReleaseGraph) -> Result<()> {
        self.check_binding(graph)
    }
    fn student_call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js> {
        self.call(tool, input)
    }
    fn student_source_requests(&self) -> usize {
        self.source_requests()
    }
}
