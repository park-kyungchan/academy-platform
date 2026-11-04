//! A fictional save queued with its cancellation must not reach a child.
#[path = "fixtures/stdio_peer.rs"]
mod peer;
use edu_academy_mcp::{Js, encode, obj};
use peer::setup::*;
use std::io::Write;

#[test]
fn queued_save_cancelled_in_one_input_batch_never_dispatches() {
    let scene = scene("normal", "academy_dailyrecord_save");
    let mut bridge = Bridge::start(&scene);
    let messages = [
        call("academy_dailyrecord_save"),
        obj(&[
            ("jsonrpc", "2.0".into()),
            ("method", "notifications/cancelled".into()),
            ("params", obj(&[("requestId", Js::Num(1.0))])),
        ]),
        request("after-cancel".into(), "ping", obj(&[])),
    ];
    let mut batch = vec![];
    for message in messages {
        batch.extend(encode(&message).unwrap());
        batch.push(b'\n');
    }
    bridge
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&batch)
        .unwrap();
    bridge.child.stdin.as_mut().unwrap().flush().unwrap();
    let response = bridge.receive();
    assert!(response.get("id").is("after-cancel"), "{response:?}");
    assert!(!scene.state.join("called.json").exists());
    assert!(receipts(&scene, "-dispatch.json").is_empty());
    bridge.close();
}
