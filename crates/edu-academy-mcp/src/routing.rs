//! Owner-pinned paper routing; aliases never widen a caller's source scope.
use crate::{
    Error, Js, Result,
    registry::{Entry, ToolRegistry},
    string,
};
use std::collections::BTreeSet;

pub fn validate_configuration(routing: &Js) -> Result<()> {
    if !matches!(routing, Js::Obj(_))
        || routing.props().iter().any(|(key, _)| {
            !["admissionPath", "admissionSha256", "paperScopes"]
                .iter()
                .any(|name| key == &crate::text(name))
        })
    {
        return Err(Error::new("release_routing_unqualified"));
    }
    let path = string(routing.get("admissionPath"))?;
    if !crate::roots::canonical_absolute(std::path::Path::new(&path))
        || !crate::identity::valid_hash(&string(routing.get("admissionSha256"))?)
    {
        return Err(Error::new("release_routing_unqualified"));
    }
    if !routing.has("paperScopes") {
        return Ok(());
    }
    let scopes = routing.get("paperScopes");
    if !matches!(scopes, Js::Arr(_)) || !(1..=32).contains(&scopes.array().len()) {
        return Err(Error::new("paper_scope_unqualified"));
    }
    let mut endpoints = BTreeSet::new();
    for scope in scopes.array() {
        let endpoint = string(scope.get("endpoint"))?;
        let students = scope.get("studentIds");
        if !matches!(scope, Js::Obj(_))
            || scope.props().len() != 2
            || !["paper1", "paper2"].contains(&endpoint.as_str())
            || !endpoints.insert(endpoint)
            || !matches!(students, Js::Arr(_))
            || !(1..=1000).contains(&students.array().len())
        {
            return Err(Error::new("paper_scope_unqualified"));
        }
        let mut ids = BTreeSet::new();
        for student in students.array() {
            let student = string(student)?;
            if !native_key(&student) || !ids.insert(student) {
                return Err(Error::new("paper_scope_duplicate"));
            }
        }
    }
    Ok(())
}
fn native_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 16
        && s.as_bytes()[0] != b'0'
        && s.bytes().all(|b| b.is_ascii_digit())
}
pub fn select<'a>(
    routing: &Js,
    registry: &'a ToolRegistry,
    entry: &'a Entry,
    args: &Js,
) -> Result<&'a Entry> {
    let routable = [
        "academy_paper_headers",
        "academy_paper_definition_lookup",
        "academy_printed_paper_lookup",
        "academy_daily_test_inventory",
        "academy_source_read_contracts",
        "academy_paper_read_contracts",
    ];
    if !routing.has("paperScopes")
        || !["paper1", "paper2"].contains(&entry.endpoint.as_str())
        || !routable.contains(&entry.upstream_name.as_str())
    {
        return Ok(entry);
    }
    let student = string(args.get("studentId"))?;
    if !native_key(&student) {
        return Err(Error::new("tool_input_invalid"));
    }
    let matches = routing
        .get("paperScopes")
        .array()
        .iter()
        .filter(|scope| {
            scope
                .get("studentIds")
                .array()
                .iter()
                .any(|id| id.is(&student))
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(Error::new("paper_scope_unresolved"));
    }
    let endpoint = string(matches[0].get("endpoint"))?;
    if entry.endpoint == "paper2" && endpoint != "paper2" {
        return Err(Error::new("paper_scope_unresolved"));
    }
    registry
        .entries()
        .iter()
        .find(|candidate| {
            candidate.endpoint == endpoint && candidate.upstream_name == entry.upstream_name
        })
        .ok_or_else(|| Error::new("paper_scope_unresolved"))
}
