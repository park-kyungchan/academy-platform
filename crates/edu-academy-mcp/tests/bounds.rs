use edu_academy_mcp::{
    Js,
    audit::{Effect, tool_result, with_tool_meta},
    encode, obj, process,
};

#[test]
fn large_valid_result_omits_only_added_metadata() {
    let raw = obj(&[
        ("status", "observed".into()),
        ("fictional", "x".repeat(600000).into()),
    ]);
    let result = tool_result(raw).unwrap();
    let metadata = obj(&[(
        "academy/bridgeAudit",
        obj(&[("occurrence", "fictional-marker".into())]),
    )]);
    let audited = with_tool_meta(&result, &metadata).unwrap();
    assert_eq!(audited, result);
    assert!(!audited.has("_meta"));
    assert!(encode(&audited).unwrap().len() > 1024 * 1024);
}
#[test]
fn small_result_preserves_existing_meta_and_adds_bridge_audit() {
    let mut result = tool_result(obj(&[("status", "observed".into())])).unwrap();
    result.set(
        "_meta",
        obj(&[("academy/audit", obj(&[("occurrence", "upstream".into())]))]),
    );
    let audited = with_tool_meta(
        &result,
        &obj(&[(
            "academy/bridgeAudit",
            obj(&[("occurrence", "bridge".into())]),
        )]),
    )
    .unwrap();
    assert!(audited.get("_meta").has("academy/audit"));
    assert!(audited.get("_meta").has("academy/bridgeAudit"));
    assert!(!result.get("_meta").has("academy/bridgeAudit"));
}
#[test]
fn returned_transport_is_not_persisted_or_zero_effect() {
    for effect in [Effect::Possible, Effect::Returned] {
        let evidence = effect.failure_evidence();
        assert_eq!(evidence.get("academyWrites"), &Js::Null);
        assert!(evidence.get("effectStatus").is("unknown"));
    }
    assert_eq!(
        Effect::NotDispatched
            .failure_evidence()
            .get("academyWrites"),
        &Js::Num(0.0)
    );
    let confirmed = Effect::ReadbackConfirmed { academy_writes: 1 }.failure_evidence();
    assert_eq!(confirmed.get("academyWrites"), &Js::Num(1.0));
    assert!(confirmed.get("effectStatus").is("readback_confirmed"));
    // A green-looking fake endpoint cannot be started while cancellation is absent.
    assert_eq!(
        process::require_supervised_execution().unwrap_err().code,
        process::CANCELLATION_GAP
    );
}
