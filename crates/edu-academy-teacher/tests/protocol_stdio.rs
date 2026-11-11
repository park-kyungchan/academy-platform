//! Stdio protocol test: drive the built binary one line at a time.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

const DISPLAY_Q01: &str = include_str!("fixtures/display-q01.txt");
const DISPLAY_Q02: &str = include_str!("fixtures/display-q02.txt");
const TOOLS_FIXTURE: &[u8] = include_bytes!("fixtures/tools-list.json");

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    run_log: PathBuf,
    dir: PathBuf,
}

impl Server {
    fn start() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "edu-academy-teacher-protocol-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run_log = dir.join("run-log.jsonl");
        let scenario =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scenario-v1.json");
        let mut child = Command::new(env!("CARGO_BIN_EXE_edu-academy-teacher"))
            .args([
                "--scenario",
                scenario.to_str().unwrap(),
                "--today",
                "2026-11-11",
                "--run-log",
                run_log.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            run_log,
            dir,
        }
    }
    fn send(&mut self, raw: &str) -> String {
        writeln!(self.stdin, "{raw}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "server closed without a reply");
        line
    }
    fn call(&mut self, value: Value) -> Value {
        serde_json::from_str(&self.send(&value.to_string())).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn stdio_protocol_end_to_end() {
    let mut server = Server::start();

    // initialize answers the client's protocol version.
    let init = server.call(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "protocol-test", "version": "0"}}
    }));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "academy");
    assert_eq!(
        init["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );

    // A notification gets no reply; the next request still works (checked
    // by every later call in this test).

    // tools/list equals the fixture bytes.
    let raw = server.send(
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}).to_string(),
    );
    let fixture = String::from_utf8(TOOLS_FIXTURE.to_vec()).unwrap();
    assert!(raw.contains(fixture.trim()), "{raw}");
    let listed: Value = serde_json::from_str(&raw).unwrap();
    let expected: Value = serde_json::from_slice(TOOLS_FIXTURE).unwrap();
    assert_eq!(listed["result"], expected);

    // Q01 single student.
    let q01 = server.call(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "academy_lesson_prepare", "arguments": {"student": "가상학생A"}}
    }));
    assert_eq!(q01["result"]["isError"], false);
    assert_eq!(q01["result"]["content"][0]["type"], "text");
    assert_eq!(q01["result"]["content"][0]["text"], DISPLAY_Q01);
    assert_eq!(q01["result"]["structuredContent"]["status"], "ok");
    // checks.json Q01 data assertion, at the protocol level.
    assert_eq!(
        q01["result"]["structuredContent"]["data"]["students"][0]["previous_lesson"]["date"],
        "2026-11-09"
    );

    // Q02 class form.
    let q02 = server.call(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "academy_lesson_prepare", "arguments": {"class_name": "가상반A"}}
    }));
    assert_eq!(q02["result"]["content"][0]["text"], DISPLAY_Q02);
    assert_eq!(q02["result"]["structuredContent"]["status"], "ok");

    // One of the other 13 tools returns tool_not_in_slice_1.
    let other = server.call(json!({
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": {"name": "academy_help", "arguments": {"topic": "x"}}
    }));
    assert_eq!(other["result"]["isError"], false);
    assert_eq!(
        other["result"]["structuredContent"]["stop"]["code"],
        "tool_not_in_slice_1"
    );

    // An unknown tool name returns the catalogue's refusal, not a crash.
    let unknown = server.call(json!({
        "jsonrpc": "2.0", "id": 6, "method": "tools/call",
        "params": {"name": "academy_no_such_tool", "arguments": {}}
    }));
    assert_eq!(unknown["result"]["isError"], false);
    assert_eq!(
        unknown["result"]["structuredContent"]["stop"]["code"],
        "tool_not_in_slice_1"
    );

    // A malformed line gets -32700 and the next valid request still works.
    let bad = server.send("{not json");
    let bad_value: Value = serde_json::from_str(&bad).unwrap();
    assert_eq!(bad_value["error"]["code"], -32700);
    let ping = server.call(json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}));
    assert_eq!(ping["result"], json!({}));

    // Unknown method is -32601.
    let no_method = server.call(json!({"jsonrpc": "2.0", "id": 8, "method": "no/such"}));
    assert_eq!(no_method["error"]["code"], -32601);

    // Run log: one line per tools/call (4 calls), exact keys, no Korean
    // character and no argument value.
    let log_text = std::fs::read_to_string(&server.run_log).unwrap();
    let lines: Vec<&str> = log_text.lines().collect();
    assert_eq!(lines.len(), 4, "{log_text}");
    assert!(
        !log_text
            .chars()
            .any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)),
        "{log_text}"
    );
    assert!(!log_text.contains("가상학생A"), "{log_text}");
    let expected_keys = [
        "args_bytes",
        "args_sha256",
        "code",
        "display_bytes",
        "duration_ms",
        "source_reads",
        "status",
        "tool",
        "ts",
    ];
    for line in &lines {
        let row: Value = serde_json::from_str(line).unwrap();
        let mut keys: Vec<&str> = row
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, expected_keys, "{line}");
    }
    let first: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["tool"], "academy_lesson_prepare");
    assert_eq!(first["status"], "ok");
    assert_eq!(first["code"], Value::Null);
    assert_eq!(first["source_reads"], 4);
    let third: Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(third["code"], "tool_not_in_slice_1");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&server.run_log)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn missing_scenario_argument_exits_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_edu-academy-teacher"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains("--scenario"), "{stderr}");
}
