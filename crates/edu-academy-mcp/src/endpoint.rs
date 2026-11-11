//! Pinned native launch planning; domain routes retain explicit qualification gaps.
use crate::{
    Error, Js, Result,
    audit::{Effect, qualification_gap},
    identity::Pin,
    registry::Entry,
    release::ReleaseGraph,
};
use std::path::Path;

pub struct LaunchPlan {
    pub executable: Pin,
    pub arguments: Vec<String>,
}

pub fn validate_arguments(root: &Path, endpoint: &str, args: &[String]) -> Result<()> {
    let root = root
        .to_str()
        .ok_or_else(|| Error::new("release_argv_invalid"))?;
    // V3 intentionally has no shell, implicit cwd, TS runtime or user-provided command.
    if args != ["--install-root", root, "--endpoint", endpoint] {
        return Err(Error::new("release_endpoint_identity_mismatch"));
    }
    Ok(())
}
pub fn plan(graph: &ReleaseGraph, endpoint: &str) -> Result<LaunchPlan> {
    graph.verify_unchanged()?;
    let config = graph
        .endpoints
        .get(endpoint)
        .ok_or_else(|| Error::new("release_endpoint_absent"))?;
    validate_arguments(&graph.owner.install_root, endpoint, &config.args)?;
    Ok(LaunchPlan {
        executable: config.executable.clone(),
        arguments: config.args.clone(),
    })
}
pub fn call_unimplemented(_entry: &Entry, _arguments: &Js) -> Result<Js> {
    qualification_gap("configured_rust_route_unimplemented", Effect::NotDispatched)
}

pub fn call_unimplemented_for(
    graph: &ReleaseGraph,
    endpoint: &str,
    name: &str,
    arguments: &Js,
) -> Result<Js> {
    let entry = graph
        .registry
        .entries()
        .iter()
        .find(|entry| entry.endpoint == endpoint && entry.upstream_name == name);
    let Some(entry) = entry else {
        return qualification_gap("unknown_tool", Effect::NotDispatched);
    };
    if let Err(error) = crate::validation::validate(entry.tool.get("inputSchema"), arguments) {
        return qualification_gap(error.code, Effect::NotDispatched);
    }
    call_unimplemented(entry, arguments)
}

/// Exact endpoint/upstream binding for an explicit borrowed business driver.
/// The admission probe is a fixed endpoint-local tool, as in RpcSession::list;
/// it does not widen the release registry or any parent inventory.
pub(crate) fn business_entry<'a>(
    graph: &'a ReleaseGraph,
    endpoint: &str,
    name: &str,
) -> Result<Option<&'a Entry>> {
    if endpoint != "dailyrecord" {
        return Err(Error::new("business_endpoint_mismatch"));
    }
    let mut entries = graph
        .registry
        .entries()
        .iter()
        .filter(|entry| entry.endpoint == endpoint && entry.upstream_name == name);
    let first = entries.next();
    if entries.next().is_some() {
        return Err(Error::new("business_tool_binding_ambiguous"));
    }
    Ok(first)
}

/// Exact route for the E09c student/paper families. Returns the registry
/// entry plus the upstream (unprefixed) tool name. A `class2_` name only
/// binds on `paper2`; an `academy_*` paper name binds on its own endpoint.
pub(crate) fn student_paper_entry<'a>(
    graph: &'a ReleaseGraph,
    endpoint: &str,
    name: &str,
) -> Result<(&'a Entry, String)> {
    let upstream = if crate::student::is_student_tool(name) {
        name.to_string()
    } else if let Some(upstream) = crate::paper::upstream_name(name) {
        let class2 = crate::paper::is_class2_name(name);
        if class2 && endpoint != "paper2" {
            return Err(Error::new("paper_scope_unresolved"));
        }
        if !class2 && !["paper1", "paper2"].contains(&endpoint) {
            return Err(Error::new("business_endpoint_mismatch"));
        }
        upstream.to_string()
    } else {
        return Err(Error::new("business_endpoint_mismatch"));
    };
    let mut entries = graph
        .registry
        .entries()
        .iter()
        .filter(|e| e.endpoint == endpoint && e.upstream_name == upstream);
    let first = entries
        .next()
        .ok_or_else(|| Error::new("business_tool_not_found"))?;
    if entries.next().is_some() {
        return Err(Error::new("business_tool_binding_ambiguous"));
    }
    Ok((first, upstream))
}

pub fn is_student_paper_tool(endpoint: &str, name: &str) -> bool {
    (endpoint == "dailyrecord" && crate::student::is_student_tool(name))
        || (["paper1", "paper2"].contains(&endpoint) && crate::paper::is_paper_tool(name))
}

/// Additive, exact two-route binding. The original business_entry is unchanged.
pub(crate) fn history_entry<'a>(
    graph: &'a ReleaseGraph,
    endpoint: &str,
    name: &str,
) -> Result<&'a Entry> {
    if !((endpoint == "dailyrecord" && name == "academy_student_metadata_get")
        || (["paper1", "paper2"].contains(&endpoint) && name == "academy_paper_headers"))
    {
        return Err(Error::new("business_endpoint_mismatch"));
    }
    let mut entries = graph
        .registry
        .entries()
        .iter()
        .filter(|e| e.endpoint == endpoint && e.upstream_name == name);
    let first = entries
        .next()
        .ok_or_else(|| Error::new("business_tool_not_found"))?;
    if entries.next().is_some() {
        return Err(Error::new("business_tool_binding_ambiguous"));
    }
    Ok(first)
}
