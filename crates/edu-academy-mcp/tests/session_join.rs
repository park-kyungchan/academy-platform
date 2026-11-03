#[path = "fixtures/stdio_peer.rs"]
mod peer;
use edu_academy_mcp::{Js, obj, parse, protocol::RpcSession, release::ReleaseGraph};
use peer::setup::*;
use std::fs;

#[test]
fn native_session_preserves_preinitialize_reinitialize_and_observed_attribution() {
    let scene = scene("echo", "academy_dailyrecord_intake");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&request("pre".into(), "tools/list", obj(&[])));
    assert!(!bridge.receive().has("error"));
    for (name, version) in [("first", "2024-11-05"), ("latest", "2025-11-25")] {
        bridge.send(&request(
            name.into(),
            "initialize",
            obj(&[
                ("protocolVersion", version.into()),
                ("capabilities", obj(&[])),
                (
                    "clientInfo",
                    obj(&[
                        ("name", name.into()),
                        ("version", "7".into()),
                        ("discardUnknown", true.into()),
                    ]),
                ),
            ]),
        ));
        let response = bridge.receive();
        assert!(response.get("result").get("protocolVersion").is(version));
    }
    bridge.send(&request(
        Js::Num(1.0),
        "tools/call",
        obj(&[
            ("name", "academy_dailyrecord_intake".into()),
            (
                "arguments",
                obj(&[
                    ("actor", obj(&[("label", "declared-only".into())])),
                    (
                        "attribution",
                        obj(&[
                            ("observedBy", "forged".into()),
                            ("client", obj(&[("name", "forged".into())])),
                        ]),
                    ),
                ]),
            ),
        ]),
    ));
    let response = bridge.receive();
    let result = response.get("result");
    assert!(!result.get("isError").truthy(), "{response:?}");
    let observed = result.get("structuredContent").get("observedAttribution");
    assert!(observed.get("observedBy").is("neutral-bridge"));
    assert!(observed.get("client").get("name").is("latest"));
    assert!(observed.get("client").get("version").is("7"));
    assert!(result.get("_meta").get("fixture/preserved").truthy());
    assert!(
        result
            .get("_meta")
            .get("academy/bridgeAudit")
            .get("occurrence")
            .string()
    );
    bridge.close();
    assert_eq!(receipts(&scene, "-result.json").len(), 1);
}
#[test]
fn malformed_envelope_drops_with_diagnostic_and_notification_latches_graph() {
    let scene = scene("echo", "academy_source_status");
    let mut bridge = Bridge::start(&scene);
    let mut malformed = request("never-returned".into(), "ping", obj(&[]));
    malformed.set("unexpectedEnvelopeKey", "private-marker-not-logged".into());
    bridge.send(&malformed);
    bridge.send(&request("next".into(), "ping", obj(&[])));
    assert!(bridge.receive().get("id").is("next"));
    bridge.close();
    let diagnostic = fs::read_to_string(scene.evidence.join("bridge.stderr")).unwrap();
    assert!(diagnostic.contains("mcp_stdio_input_or_transport_invalid"));
    assert!(!diagnostic.contains("private-marker-not-logged"));
    let mut session = RpcSession::new(ReleaseGraph::open(&scene.install).unwrap());
    changed_graph(&scene);
    assert!(
        session
            .handle(&obj(&[
                ("jsonrpc", "2.0".into()),
                ("method", "notifications/initialized".into())
            ]))
            .unwrap()
            .is_none()
    );
    assert!(!session.initialized);
    fs::write(scene.install.join("release.json"), &scene.pointer).unwrap();
    assert!(
        session
            .handle(&request("latched".into(), "ping", obj(&[])))
            .unwrap()
            .unwrap()
            .has("error")
    );
}
#[test]
fn actual_first_inventory_refusal_and_finite_upstream_failure() {
    let scene = scene("missing_inventory", "academy_source_status");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&call("academy_source_status"));
    let response = bridge.receive();
    let data = response.get("result").get("structuredContent");
    assert!(
        response.get("result").get("isError").truthy(),
        "{response:?}"
    );
    assert!(
        data.get("code").is("upstream_tool_inventory_mismatch"),
        "{data:?}"
    );
    assert_eq!(
        data.get("inventory").get("missingTools").array(),
        &[Js::from("academy_source_status")]
    );
    assert!(!scene.state.join("called.json").exists());
    assert!(
        wait_receipt(&scene, "-cleanup.json")[0]
            .get("qualified")
            .truthy()
    );
    bridge.close();
    let failed = peer::setup::scene("upstream_error", "academy_source_status");
    let mut bridge = Bridge::start(&failed);
    bridge.send(&call("academy_source_status"));
    let response = bridge.receive();
    let data = response.get("result").get("structuredContent");
    assert!(data.get("code").is("query_outside_declared_scope"));
    assert!(data.get("failure").get("category").is("policy_denial"));
    assert_eq!(data.get("upstreamOccurrence"), &Js::Null);
    let encoded = edu_academy_mcp::stringify(&response).unwrap();
    assert!(!encoded.contains("private-upstream-message"));
    assert!(!encoded.contains("private-invalid-occurrence"));
    bridge.close();
}
#[test]
fn valid_large_result_survives_soft_metadata_omission() {
    let scene = scene("large", "academy_source_status");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&call("academy_source_status"));
    let response = bridge.receive();
    let result = response.get("result");
    assert_eq!(
        result.get("structuredContent").get("payload").units().len(),
        600000
    );
    let text_data = parse(
        result.get("content").array()[0]
            .get("text")
            .code_units()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(text_data.get("payload").units().len(), 600000);
    assert!(result.get("_meta").get("fixture/preserved").truthy());
    assert!(!result.get("_meta").has("academy/bridgeAudit"));
    assert_eq!(wait_receipt(&scene, "-result.json").len(), 1);
    bridge.close();
}
