use edu_academy_mcp::{
    Js, encode,
    framing::{BRIDGE_INPUT_LINE_BYTES, BridgeInput, write_message},
    parse,
    registry::ToolRegistry,
    validation,
};
use std::io::{BufReader, Cursor};

#[test]
fn fragmented_crlf_malformed_and_eof_fragment_keep_valid_typed_ids() {
    let bytes = b"{broken}\n{\"jsonrpc\":\"2.0\",\"id\":\"8\",\"method\":\"ping\"}\r\n{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"ping\"}\n{";
    let mut input = BridgeInput::new(BufReader::with_capacity(3, Cursor::new(bytes)));
    let mut diagnostics = Vec::new();
    let a = input.next(&mut diagnostics).unwrap().unwrap();
    let b = input.next(&mut diagnostics).unwrap().unwrap();
    assert!(a.get("id").is("8"));
    assert!(matches!(b.get("id"), Js::Num(n) if *n == 8.0));
    assert!(input.next(&mut diagnostics).unwrap().is_none());
    assert_eq!(diagnostics.iter().filter(|b| **b == b'\n').count(), 2);
    assert!(!String::from_utf8(diagnostics).unwrap().contains("broken"));
}
#[test]
fn input_limit_is_lf_inclusive_without_changing_output_limit() {
    let mut bytes = b"{\"value\":\"".to_vec();
    bytes.resize(BRIDGE_INPUT_LINE_BYTES - 3, b'x');
    bytes.extend_from_slice(b"\"}\n");
    let mut reader = BridgeInput::new(Cursor::new(bytes.clone()));
    assert!(reader.next(&mut Vec::new()).unwrap().is_some());
    bytes.insert(bytes.len() - 3, b'x');
    let mut reader = BridgeInput::new(Cursor::new(bytes));
    assert_eq!(
        reader.next(&mut Vec::new()).unwrap_err().code,
        "mcp_stdio_frame_too_large"
    );
}
#[test]
fn public_schema_rejects_changed_name_extra_scope_and_33_labels() {
    let raw = include_bytes!(
        "../../../migration/mcp-runtime-reference-v1/frozen/inventory/registry.json"
    );
    let registry = ToolRegistry::parse(raw).unwrap();
    assert_eq!(registry.entries().len(), 41);
    assert_eq!(registry.list().unwrap().len(), 42);
    assert!(registry.get("academy_item_answer_lookup_renamed").is_err());
    let item = registry.get("academy_item_answer_lookup").unwrap();
    let mut input =
        parse(br#"{"labels":["12345"],"purpose":"fictional","currentRequestRef":"request"}"#)
            .unwrap();
    validation::validate(item.tool.get("inputSchema"), &input).unwrap();
    input.set(
        "labels",
        Js::Arr((0..33).map(|i| format!("{}", 12345 + i).into()).collect()),
    );
    assert!(validation::validate(item.tool.get("inputSchema"), &input).is_err());
    let get = registry.get("academy_dailyrecord_get").unwrap();
    let extra = parse(br#"{"displayName":"Invented Student","purpose":"fictional","currentRequestRef":"request","teacherId":"99002"}"#).unwrap();
    assert!(validation::validate(get.tool.get("inputSchema"), &extra).is_err());
    // All listed tools retain source-provided annotations and exact schema members.
    assert_eq!(
        get.tool.get("annotations").get("idempotentHint"),
        &Js::Bool(false)
    );
}
#[test]
fn omissions_null_unicode_and_defaults_are_separate_boundaries() {
    let schema = parse(br#"{"type":"object","properties":{"value":{"type":"string","maxLength":1},"limit":{"type":"integer","default":2}},"required":["value"],"additionalProperties":false}"#).unwrap();
    let value = parse(br#"{"value":"\ud83d\ude00"}"#).unwrap();
    validation::validate(&schema, &value).unwrap(); // JSON Schema counts code points.
    assert!(!value.has("limit")); // Endpoint defaults are not inserted at the bridge.
    assert!(validation::validate(&schema, &parse(br#"{"value":null}"#).unwrap()).is_err());
    let lone = parse(br#"{"value":"\ud800"}"#).unwrap();
    assert!(
        String::from_utf8(encode(&lone).unwrap())
            .unwrap()
            .contains("\\ud800")
    );
}
#[test]
fn output_writer_does_not_apply_bridge_input_cap() {
    let value: Js = "x".repeat(BRIDGE_INPUT_LINE_BYTES + 10).into();
    let mut output = Vec::new();
    write_message(&mut output, &value).unwrap();
    assert!(output.len() > BRIDGE_INPUT_LINE_BYTES);
    assert_eq!(parse(&output).unwrap(), value);
}
