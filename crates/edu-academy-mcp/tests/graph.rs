mod common;
use edu_academy_mcp::{
    Js, parse,
    protocol::RpcSession,
    release::{ConnectionGate, ReleaseGraph},
};

#[test]
fn pointer_change_latches_refusal_after_exact_restoration() {
    let fixture = common::Fixture::new();
    let mut session = RpcSession::new(fixture.graph());
    let ping = parse(br#"{"jsonrpc":"2.0","id":"same-id","method":"ping"}"#).unwrap();
    assert!(session.handle(&ping).unwrap().unwrap().has("result"));
    let mut changed = serde_json::from_slice::<serde_json::Value>(&fixture.pointer).unwrap();
    changed["generation"] = "fictional-v2".into();
    fixture.replace_pointer(&serde_json::to_vec(&changed).unwrap());
    let refused = session.handle(&ping).unwrap().unwrap();
    assert!(
        refused
            .get("error")
            .get("message")
            .is("release_pointer_changed")
    );
    fixture.replace_pointer(&fixture.pointer);
    assert_eq!(session.handle(&ping).unwrap().unwrap(), refused);
    assert!(
        RpcSession::new(fixture.graph())
            .handle(&ping)
            .unwrap()
            .unwrap()
            .has("result")
    );
}
#[test]
fn changed_pinned_source_refuses_even_when_pointer_is_unchanged() {
    let fixture = common::Fixture::new();
    let graph = fixture.graph();
    std::fs::write(&fixture.source, b"changed source bytes").unwrap();
    assert_eq!(
        graph.verify_unchanged().unwrap_err().code,
        "release_artifact_mismatch"
    );
}
#[test]
fn legacy_graph_is_refused_without_conversion() {
    let fixture = common::Fixture::new();
    let mut pointer = serde_json::from_slice::<serde_json::Value>(&fixture.pointer).unwrap();
    pointer["schemaVersion"] = "academy.mcp-release/1".into();
    fixture.replace_pointer(&serde_json::to_vec(&pointer).unwrap());
    assert!(
        matches!(ReleaseGraph::open(&fixture.install), Err(e) if e.code=="legacy_graph_refused")
    );
}
#[test]
fn initialize_retains_selected_version_and_configured_route_cannot_fake_success() {
    let fixture = common::Fixture::new();
    let mut session = RpcSession::new(fixture.graph());
    let init = parse(br#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"fictional","version":"1"}}}"#).unwrap();
    let response = session.handle(&init).unwrap().unwrap();
    assert!(
        response
            .get("result")
            .get("protocolVersion")
            .is("2025-03-26")
    );
    let call = parse(br#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"academy_source_status"}}"#).unwrap();
    let response = session.handle(&call).unwrap().unwrap();
    assert_eq!(response.get("result").get("isError"), &Js::Bool(true));
    assert!(
        response
            .get("result")
            .get("structuredContent")
            .get("code")
            .is("configured_rust_route_unimplemented")
    );
}
#[test]
fn gate_does_not_accept_success_after_an_observed_failure() {
    let mut gate = ConnectionGate::default();
    let failure = edu_academy_mcp::Error::new("release_pointer_changed");
    gate.check(Ok(())).unwrap();
    assert_eq!(gate.check(Err(failure)), Err(failure));
    assert_eq!(gate.check(Ok(())), Err(failure));
    assert!(gate.refused());
}
