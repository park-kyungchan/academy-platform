//! Paper MCP adapters over the E09b source layer, for both owner scopes.
//! Paper1 tools keep their `academy_*` names; the paper2 family is listed
//! as `class2_*` at the bridge and as `academy_*` inside its own endpoint
//! session. Both spellings resolve to the same upstream tool, and a scope
//! mismatch refuses with `paper_scope_unresolved` before any source read.
use crate::{Error, Js, Result, obj, release::ReleaseGraph};
use edu_academy_core::{js::Fault, workflow::read_failure};
use edu_academy_sources::{data::fault_from_error, paper::PaperSource, student_course::ReadPort};

pub const TOOLS: [&str; 6] = [
    "academy_daily_test_inventory",
    "academy_paper_definition_lookup",
    "academy_paper_headers",
    "academy_paper_read_contracts",
    "academy_printed_paper_lookup",
    "academy_source_read_contracts",
];

pub fn class2_name(upstream: &str) -> String {
    format!("class2_{upstream}")
}

pub fn is_paper_tool(name: &str) -> bool {
    TOOLS.contains(&name)
        || name
            .strip_prefix("class2_")
            .is_some_and(|u| TOOLS.contains(&u))
}

/// Strip the bridge `class2_` prefix; `None` for non-paper names.
pub fn upstream_name(name: &str) -> Option<&str> {
    if TOOLS.contains(&name) {
        Some(name)
    } else {
        name.strip_prefix("class2_").filter(|u| TOOLS.contains(u))
    }
}

pub fn is_class2_name(name: &str) -> bool {
    name.strip_prefix("class2_")
        .is_some_and(|u| TOOLS.contains(&u))
}

/// Borrowed paper driver for one exact owner scope. No fallthrough to the
/// other scope is possible: the wrapped `PaperSource` is single-scope and
/// this adapter refuses a mismatched family before dispatch.
pub struct PaperDriver<P: ReadPort> {
    source: PaperSource<P>,
    routing_admission: Js,
    lesson_date: String,
}

impl<P: ReadPort> PaperDriver<P> {
    pub fn new(source: PaperSource<P>, routing_admission: Js, lesson_date: String) -> Self {
        Self {
            source,
            routing_admission,
            lesson_date,
        }
    }
    pub fn endpoint(&self) -> &'static str {
        self.source.endpoint().as_str()
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
        let endpoint = self.endpoint();
        let declared = graph
            .routing
            .get("paperScopes")
            .array()
            .iter()
            .find(|s| s.get("endpoint").is(endpoint));
        let students = self.source.transport_admission().student_ids.clone();
        if !declared.is_some_and(|s| {
            s.get("studentIds").array().len() == students.len()
                && students.iter().all(|id| {
                    s.get("studentIds")
                        .array()
                        .iter()
                        .any(|x| x.is(id.as_str()))
                })
        }) {
            return Err(Error::new("paper_scope_unresolved"));
        }
        Ok(())
    }
    /// One paper call. `tool` may be either spelling; scope is enforced here
    /// and again by `PaperSource::read`, with zero requests on a mismatch.
    pub fn call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js> {
        let Some(upstream) = upstream_name(tool) else {
            return Err(Fault::error("unknown_tool"));
        };
        if is_class2_name(tool) && self.endpoint() != "paper2" {
            return Ok(scope_envelope(self.source.requests()));
        }
        let _ = upstream;
        let observed_at = crate::audit::wall_time();
        let before = self.source.requests();
        let result = self
            .source
            .read(tool, input, &self.lesson_date, &observed_at)
            .map_err(fault_from_error)
            .and_then(|value| {
                let bytes = crate::audit::tool_result(value.clone())
                    .and_then(|v| crate::encode(&v))
                    .map_err(|e| Fault::error(e.code))?;
                if bytes.len() > 512 * 1024 {
                    return Err(Fault::error("source_headers_read_failed"));
                }
                Ok(value)
            });
        match result {
            Ok(value) => Ok(value),
            Err(error) => Ok(failure_envelope(&error, self.source.requests(), before)),
        }
    }
}

fn scope_envelope(source_requests: usize) -> Js {
    let failure = obj(&[
        ("category", "policy_denial".into()),
        ("code", "paper_scope_unresolved".into()),
        ("evidenceLocator", "academy-mcp:paper-scope".into()),
        ("automaticRetryAllowed", false.into()),
    ]);
    let mut out = obj(&[
        ("status", "blocked".into()),
        ("code", "paper_scope_unresolved".into()),
        ("reasonCode", "paper_scope_unresolved".into()),
        ("failure", failure),
        ("failedSource", "paper".into()),
        ("sourceRequests", source_requests.into()),
        ("automaticRetryAllowed", false.into()),
    ]);
    out.set("academyWrites", Js::Num(0.0));
    out.set("parentSends", Js::Num(0.0));
    out
}

fn failure_envelope(error: &Fault, source_requests: usize, _before: usize) -> Js {
    let failure = read_failure::public_read_failure(error, "source_read_unavailable");
    let code = source_code(error, &failure);
    let mut out = obj(&[
        ("status", "blocked".into()),
        ("code", code.clone()),
        ("reasonCode", code),
        ("failure", failure),
        ("failedSource", "paper".into()),
        ("sourceRequests", source_requests.into()),
        ("automaticRetryAllowed", false.into()),
    ]);
    out.set("academyWrites", Js::Num(0.0));
    out.set("parentSends", Js::Num(0.0));
    out
}

fn source_code(error: &Fault, failure: &Js) -> Js {
    let raw = error.properties.get("code");
    if raw.string() && !raw.code_units().is_empty() {
        return raw.clone();
    }
    if error.name == "ZodError" || error.name == "SyntaxError" {
        return "source_response_malformed".into();
    }
    if !error.message.is_empty()
        && error
            .message
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return error.message.as_str().into();
    }
    failure.get("code").clone()
}

/// Protocol-facing loan for one paper scope.
pub trait PaperCall {
    fn paper_endpoint(&self) -> &str;
    fn check_paper_binding(&self, graph: &ReleaseGraph) -> Result<()>;
    fn paper_call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js>;
    fn paper_source_requests(&self) -> usize;
}

impl<P: ReadPort> PaperCall for PaperDriver<P> {
    fn paper_endpoint(&self) -> &str {
        self.endpoint()
    }
    fn check_paper_binding(&self, graph: &ReleaseGraph) -> Result<()> {
        self.check_binding(graph)
    }
    fn paper_call(&mut self, tool: &str, input: &Js) -> edu_academy_core::js::Result<Js> {
        self.call(tool, input)
    }
    fn paper_source_requests(&self) -> usize {
        self.source_requests()
    }
}
