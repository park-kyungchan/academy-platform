//! First increment admits launch metadata only; it cannot dispatch a source operation.
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
