//! Run only with Root's explicit fresh fictional root and artifact paths.
//! Tests change no library-host signal/subreaper state and keep native receipts.
#[path = "fixtures/stdio_peer.rs"]
mod peer;
use edu_academy_mcp::{Js, obj, parse};
use peer::setup::*;
use std::{fs, time::Duration};

fn marker(scene: &Scene, name: &str) -> Js {
    let path = scene.state.join(name);
    wait_file(&path, Duration::from_secs(8));
    parse(&fs::read(path).unwrap()).unwrap()
}
fn cleanup(scene: &Scene) -> Js {
    let value = wait_receipt(scene, "-cleanup.json").remove(0);
    assert!(value.get("qualified").truthy(), "{value:?}");
    assert!(value.get("stdoutEof").truthy());
    assert!(value.get("stderrEof").truthy());
    assert_gone(&value);
    value
}
fn escaped_gone(scene: &Scene, direct: &Js, descendant: &Js) {
    let evidence = cleanup(scene);
    assert_gone(direct);
    assert_gone(descendant);
    for identity in [direct, descendant] {
        assert!(
            evidence
                .get("reaped")
                .array()
                .iter()
                .any(|v| v.get("pid") == identity.get("pid")),
            "{evidence:?}"
        );
    }
    assert_lock_released(scene);
}
#[test]
fn cancellation_reaps_ignored_term_escaped_descendant_and_releases_same_lock() {
    let scene = scene("ignore_term", "academy_source_status");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&call("academy_source_status"));
    let direct = marker(&scene, "called.json");
    let descendant = marker(&scene, "descendant.json");
    assert_ne!(direct.get("group"), descendant.get("group"));
    bridge.send(&obj(&[
        ("jsonrpc", "2.0".into()),
        ("method", "notifications/cancelled".into()),
        ("params", obj(&[("requestId", Js::Num(1.0))])),
    ]));
    let failures = wait_receipt(&scene, "-failure.json");
    assert!(
        failures[0].get("code").is("request_cancelled"),
        "{failures:?}"
    );
    escaped_gone(&scene, &direct, &descendant);
    // An aborted SDK request gets no response; subsequent typed ping is first.
    bridge.send(&request("after-cancel".into(), "ping", obj(&[])));
    assert!(bridge.receive().get("id").is("after-cancel"));
    bridge.close();
}
#[test]
fn client_eof_cleans_active_child_and_releases_same_lock() {
    let scene = scene("hold", "academy_source_status");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&call("academy_source_status"));
    let direct = marker(&scene, "called.json");
    bridge.eof();
    assert!(bridge.finish().success());
    cleanup(&scene);
    assert_gone(&direct);
    assert_lock_released(&scene);
    assert!(
        receipts(&scene, "-failure.json")[0]
            .get("code")
            .is("mcp_client_eof")
    );
}
#[test]
fn parent_term_cleans_descendants_before_preserving_native_signal_status() {
    let scene = scene("ignore_term", "academy_source_status");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&call("academy_source_status"));
    let direct = marker(&scene, "called.json");
    let descendant = marker(&scene, "descendant.json");
    bridge.term();
    assert_eq!(signal(bridge.finish()), Some(15));
    escaped_gone(&scene, &direct, &descendant);
    assert!(
        receipts(&scene, "-failure.json")[0]
            .get("code")
            .is("parent_signal_cancelled")
    );
}
#[test]
fn graph_switch_before_call_refuses_and_latches_after_pointer_restore() {
    let scene = scene("before_call", "academy_source_status");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&call("academy_source_status"));
    let direct = marker(&scene, "before-call.json");
    changed_graph(&scene);
    let response = bridge.receive();
    assert!(
        response.get("result").get("isError").truthy(),
        "{response:?}"
    );
    assert_eq!(
        response
            .get("result")
            .get("structuredContent")
            .get("academyWrites"),
        &Js::Num(0.0)
    );
    assert!(!scene.state.join("called.json").exists());
    cleanup(&scene);
    assert_gone(&direct);
    fs::write(scene.install.join("release.json"), &scene.pointer).unwrap();
    bridge.send(&request("still-latched".into(), "ping", obj(&[])));
    assert!(bridge.receive().has("error"));
    bridge.close();
}
#[test]
fn graph_switch_after_possible_write_preserves_unknown_without_retry() {
    let scene = scene("hold", "academy_dailyrecord_save");
    let mut bridge = Bridge::start(&scene);
    bridge.send(&call("academy_dailyrecord_save"));
    let direct = marker(&scene, "called.json");
    changed_graph(&scene);
    let response = bridge.receive();
    let data = response.get("result").get("structuredContent");
    assert!(
        response.get("result").get("isError").truthy(),
        "{response:?}"
    );
    assert_eq!(data.get("academyWrites"), &Js::Null);
    assert!(data.get("effectStatus").is("unknown"));
    assert_eq!(data.get("automaticRetryAllowed"), &Js::Bool(false));
    cleanup(&scene);
    assert_gone(&direct);
    assert_lock_released(&scene);
    assert_eq!(receipts(&scene, "-dispatch.json").len(), 1);
    assert_eq!(receipts(&scene, "-failure.json").len(), 1);
    bridge.close();
}
#[test]
fn child_receive_bound_and_blocked_inherited_output_have_finite_cleanup() {
    let flooded = scene("flood", "academy_source_status");
    let mut bridge = Bridge::start(&flooded);
    bridge.send(&call("academy_source_status"));
    let direct = marker(&flooded, "called.json");
    let response = bridge.receive();
    assert!(
        response
            .get("result")
            .get("structuredContent")
            .get("code")
            .is("upstream_receive_buffer_exceeded"),
        "{response:?}"
    );
    cleanup(&flooded);
    assert_gone(&direct);
    assert_lock_released(&flooded);
    bridge.close();

    let blocked = scene("large", "academy_source_status");
    let mut bridge = Bridge::start(&blocked);
    bridge.send(&call("academy_source_status"));
    // Deliberately never drain bridge stdout. Child result and cleanup happened;
    // the separately bounded inherited writer must not deadlock process exit.
    wait_receipt(&blocked, "-result.json");
    assert_eq!(bridge.finish().code(), Some(1));
    cleanup(&blocked);
    let io = wait_receipt(&blocked, "-io.json");
    assert_eq!(io[0].get("stdoutThreadJoined"), &Js::Bool(false));
    assert_eq!(
        io[0].get("blockingInheritedStdioFlagsChanged"),
        &Js::Bool(false)
    );
    assert_lock_released(&blocked);
}
