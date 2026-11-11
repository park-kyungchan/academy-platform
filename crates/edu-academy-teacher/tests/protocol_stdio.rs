//! Stdio protocol test: drive the built binary one line at a time.
use edu_academy_teacher::tools::DISPLAY_DIRECTIVE;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

const DISPLAY_Q01: &str = include_str!("fixtures/display-q01.txt");
const DISPLAY_Q02: &str = include_str!("fixtures/display-q02.txt");
const INSTRUCTIONS: &str = "Each tool result of this server is finished Korean text for the teacher. Show that text to the teacher exactly as it is and in full, with nothing added before or after it: no summary, no rewording, no advice, no extra question. When the text asks the teacher something or lists choices, stop there and wait for the teacher's answer, then call the tool again with that answer and the other arguments of the previous call unchanged. When the teacher later asks about a result already shown and no tool is needed, answer briefly in your own words from that result only. Call a tool again only when the teacher asks for something else or answers a question.";
const TOOLS_FIXTURE: &[u8] = include_bytes!("fixtures/tools-list.json");

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Receiver<String>,
    run_log: PathBuf,
    dir: PathBuf,
}

impl Server {
    fn start() -> Self {
        Self::start_with_log(None)
    }
    fn start_with_log(kind: Option<&str>) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "edu-academy-teacher-protocol-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run_log = dir.join("run-log.jsonl");
        if kind == Some("existing") {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&run_log, "").unwrap();
            std::fs::set_permissions(&run_log, std::fs::Permissions::from_mode(0o644)).unwrap();
        } else if kind == Some("symlink") {
            std::fs::write(dir.join("target"), "retained").unwrap();
            std::os::unix::fs::symlink(dir.join("target"), &run_log).unwrap();
        }
        let scenario = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(if kind == Some("unit") {
            "tests/fixtures/scenario-v3.json"
        } else {
            "tests/fixtures/scenario-v2.json"
        });
        let scenario = if kind == Some("empty_roster") {
            let mut value: Value =
                serde_json::from_str(&std::fs::read_to_string(&scenario).unwrap()).unwrap();
            value["classes"][2]["roster"] = json!([]);
            let path = dir.join("scenario.json");
            std::fs::write(&path, value.to_string()).unwrap();
            path
        } else {
            scenario
        };
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
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let (tx, stdout) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin: Some(stdin),
            stdout,
            run_log,
            dir,
        }
    }
    fn send(&mut self, raw: &str) -> String {
        writeln!(self.stdin.as_mut().unwrap(), "{raw}").unwrap();
        self.stdin.as_mut().unwrap().flush().unwrap();
        let line = self
            .stdout
            .recv_timeout(Duration::from_secs(10))
            .expect("reply within 10 seconds");
        assert!(!line.is_empty(), "server closed without a reply");
        line
    }
    fn call(&mut self, value: Value) -> Value {
        let reply: Value = serde_json::from_str(&self.send(&value.to_string())).unwrap();
        if let Some(content) = reply["result"].get("content") {
            assert_eq!(content.as_array().unwrap().len(), 2);
            assert_eq!(content[0], json!({"type":"text","text":DISPLAY_DIRECTIVE}));
            assert_eq!(
                content[1],
                json!({"type":"text","text":reply["result"]["_meta"]["academy/envelope"]["display"]})
            );
        }
        reply
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
    assert_eq!(init["result"]["instructions"], INSTRUCTIONS);
    assert_eq!(init["result"]["serverInfo"]["name"], "academy");
    assert_eq!(
        init["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );

    // A notification gets no reply; the next request still works (checked
    // by every later call in this test).

    writeln!(
        server.stdin.as_mut().unwrap(),
        "{}",
        json!({"jsonrpc":"2.0", "method":"notifications/initialized"})
    )
    .unwrap();
    server.stdin.as_mut().unwrap().flush().unwrap();
    assert!(matches!(
        server.stdout.recv_timeout(Duration::from_millis(200)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));

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
    assert_eq!(
        q01["result"]["content"],
        json!([{"type":"text","text":DISPLAY_DIRECTIVE},{"type":"text","text":DISPLAY_Q01}])
    );
    assert!(q01["result"].get("structuredContent").is_none());
    assert_eq!(q01["result"]["content"][1]["text"], DISPLAY_Q01);
    assert_eq!(q01["result"]["_meta"]["academy/envelope"]["status"], "ok");
    // checks.json Q01 data assertion, at the protocol level.
    assert_eq!(
        q01["result"]["_meta"]["academy/envelope"]["data"]["students"][0]["previous_lesson"]["date"],
        "2026-11-09"
    );

    // Q02 class form.
    let q02 = server.call(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "academy_lesson_prepare", "arguments": {"class_name": "가상반A"}}
    }));
    assert_eq!(q02["result"]["content"][1]["text"], DISPLAY_Q02);
    assert_eq!(q02["result"]["_meta"]["academy/envelope"]["status"], "ok");

    assert_eq!(
        q02["result"]["content"],
        json!([{"type":"text","text":DISPLAY_DIRECTIVE},{"type":"text","text":DISPLAY_Q02}])
    );
    for result in [&q01["result"], &q02["result"]] {
        assert_eq!(result.as_object().unwrap().len(), 3);
        assert_eq!(result["_meta"].as_object().unwrap().len(), 1);
        assert_eq!(
            result["_meta"]["academy/envelope"]["display"],
            result["content"][1]["text"]
        );
        assert!(result.get("structuredContent").is_none());
    }

    // One of the other 13 tools returns tool_not_in_slice_1.
    let other = server.call(json!({
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": {"name": "academy_help", "arguments": {"topic": "x"}}
    }));
    assert_eq!(other["result"]["isError"], true);
    assert_eq!(
        other["result"]["_meta"]["academy/envelope"]["stop"]["code"],
        "tool_not_in_slice_1"
    );

    // An unknown tool name returns the catalogue's refusal, not a crash.
    let unknown = server.call(json!({
        "jsonrpc": "2.0", "id": 6, "method": "tools/call",
        "params": {"name": "가상학생UnknownTool", "arguments": {}}
    }));
    assert_eq!(unknown["error"]["code"], -32602);
    assert_eq!(unknown["id"], 6);
    assert!(unknown.get("result").is_none());

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
    let fourth: Value = serde_json::from_str(lines[3]).unwrap();
    assert_eq!(fourth["tool"], "unknown");
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

fn assert_invalid_recovers(raw: &[u8], code: i32) {
    let mut server = Server::start();
    server.stdin.as_mut().unwrap().write_all(raw).unwrap();
    server.stdin.as_mut().unwrap().flush().unwrap();
    let line = server
        .stdout
        .recv_timeout(Duration::from_secs(10))
        .expect("error within 10 seconds");
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["error"]["code"], code);
    if code == -32700 {
        assert!(reply["id"].is_null());
    }
    let ping = server.call(json!({"id":10,"method":"ping"}));
    assert_eq!(ping["id"], 10);
    assert_eq!(ping["result"], json!({}));
}

#[test]
fn invalid_utf8_recovers() {
    assert_invalid_recovers(b"\xff\n", -32700);
}
#[test]
fn invalid_json_recovers() {
    assert_invalid_recovers(b"{broken\n", -32700);
}
#[test]
fn oversized_line_recovers() {
    let mut raw = vec![b'x'; 1_048_577];
    raw.extend_from_slice(b"{\"id\":99,\"method\":\"ping\"}\n");
    assert_invalid_recovers(&raw, -32700);
}
#[test]
fn batch_recovers() {
    assert_invalid_recovers(b"[]\n", -32600);
}
#[test]
fn missing_method_recovers() {
    assert_invalid_recovers(b"{\"id\":9}\n", -32600);
}
#[test]
fn nonstring_method_recovers() {
    assert_invalid_recovers(b"{\"id\":9,\"method\":3}\n", -32600);
}
#[test]
fn exact_message_limit_is_accepted() {
    let mut server = Server::start();
    let mut raw = b"{\"id\":1,\"method\":\"ping\"}".to_vec();
    raw.resize(1_048_576, b' ');
    raw.push(b'\n');
    server.stdin.as_mut().unwrap().write_all(&raw).unwrap();
    server.stdin.as_mut().unwrap().flush().unwrap();
    let line = server
        .stdout
        .recv_timeout(Duration::from_secs(10))
        .expect("reply within 10 seconds");
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap()["result"],
        json!({})
    );
}

#[test]
fn existing_log_becomes_0600() {
    use std::os::unix::fs::PermissionsExt;
    let mut server = Server::start_with_log(Some("existing"));
    server.call(json!({"id":1,"method":"tools/call","params":{"name":"academy_help"}}));
    assert_eq!(
        std::fs::metadata(&server.run_log)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn symlink_log_is_not_followed() {
    let mut server = Server::start_with_log(Some("symlink"));
    for id in 1..=2 {
        let reply =
            server.call(json!({"id":id,"method":"tools/call","params":{"name":"academy_help"}}));
        assert_eq!(reply["result"]["isError"], true);
    }
    assert_eq!(
        std::fs::read_to_string(server.dir.join("target")).unwrap(),
        "retained"
    );
    server.child.kill().unwrap();
    server.child.wait().unwrap();
    let mut stderr = String::new();
    server
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(
        stderr.lines().collect::<Vec<_>>(),
        ["call_log_disabled: cannot securely open log"]
    );
}

#[test]
fn blank_lines_are_skipped_without_reply() {
    let mut server = Server::start();
    server
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"\n \t\r\n")
        .unwrap();
    server.stdin.as_mut().unwrap().flush().unwrap();
    assert!(matches!(
        server.stdout.recv_timeout(Duration::from_millis(200)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(server.call(json!({"id":1,"method":"ping"}))["id"], 1);
}

fn reply_at_eof(raw: &[u8]) -> Value {
    let mut server = Server::start();
    server.stdin.as_mut().unwrap().write_all(raw).unwrap();
    server.stdin.as_mut().unwrap().flush().unwrap();
    drop(server.stdin.take());
    let line = server
        .stdout
        .recv_timeout(Duration::from_secs(10))
        .expect("reply at EOF");
    serde_json::from_str(&line).unwrap()
}

#[test]
fn oversized_line_at_eof_is_parse_error() {
    let reply = reply_at_eof(&vec![b'x'; 1_048_577]);
    assert_eq!(reply["error"]["code"], -32700);
    assert!(reply["id"].is_null());
}

#[test]
fn partial_line_at_eof_is_answered() {
    assert_eq!(reply_at_eof(br#"{"id":42,"method":"ping"}"#)["id"], 42);
}

#[test]
fn seven_contract_targets_through_tools_call() {
    for (arguments, expected, empty) in [
        (json!({"student":"가상학생A"}), DISPLAY_Q01, false),
        (json!({"class_name":"가상반A"}), DISPLAY_Q02, false),
        (
            json!({"student":"가상학생F"}),
            include_str!("fixtures/display-previous-not-found.txt"),
            false,
        ),
        (
            json!({"student":"가상학생A","class_name":"가상반B"}),
            include_str!("fixtures/display-disagreement.txt"),
            false,
        ),
        (
            json!({"student":"가상학생A","date":"2026-10-20"}),
            include_str!("fixtures/display-outside-admission.txt"),
            false,
        ),
        (
            json!({"class_name":"가상반C"}),
            include_str!("fixtures/display-empty-roster.txt"),
            true,
        ),
        (
            json!({"student":"가상학생A","date":"지난 수업일"}),
            include_str!("fixtures/display-previous-lesson-date.txt"),
            false,
        ),
    ] {
        let mut server = Server::start_with_log(if empty { Some("empty_roster") } else { None });
        let response=server.call(json!({"id":1,"method":"tools/call","params":{"name":"academy_lesson_prepare","arguments":arguments}}));
        assert_eq!(
            response["result"]["content"],
            json!([{"type":"text","text":DISPLAY_DIRECTIVE},{"type":"text","text":expected}])
        );
        assert_eq!(
            response["result"]["_meta"]["academy/envelope"]["display"],
            expected
        );
        assert!(response["result"].get("structuredContent").is_none());
    }
}

fn unit_call(server: &mut Server, id: i32, args: Value, fixture: &str, status: &str) -> Value {
    let reply = server.call(json!({"jsonrpc":"2.0", "id":id,"method":"tools/call",
        "params":{"name":"academy_unit_status","arguments":args}}));
    assert_eq!(reply["id"], id);
    let result = &reply["result"];
    assert_eq!(
        result["content"],
        json!([{"type":"text","text":DISPLAY_DIRECTIVE},{"type":"text","text":fixture}])
    );
    assert_eq!(result["_meta"]["academy/envelope"]["status"], status);
    assert_eq!(result["_meta"]["academy/envelope"]["display"], fixture);
    assert_eq!(result["isError"], false);
    assert!(result.get("structuredContent").is_none());
    reply
}

#[test]
fn unit_protocol_q11_to_q16_display_and_distinct_frames() {
    let mut s = Server::start_with_log(Some("unit"));
    let list = s.send(r#"{"id":0,"method":"tools/list"}"#);
    assert_eq!(TOOLS_FIXTURE.len(), 8102);
    assert!(list.contains(std::str::from_utf8(TOOLS_FIXTURE).unwrap()));
    unit_call(
        &mut s,
        11,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
        include_str!("fixtures/display-q11.txt"),
        "ok",
    );
    let args = json!({"student":"가상학생B","unit":"4-4","topic":"DailyTest"});
    unit_call(
        &mut s,
        12,
        args.clone(),
        include_str!("fixtures/display-q12-first.txt"),
        "ok",
    );
    unit_call(
        &mut s,
        13,
        args,
        include_str!("fixtures/display-q12-later.txt"),
        "ok",
    );
    unit_call(
        &mut s,
        14,
        json!({"student":"가상학생A","topic":"클리닉"}),
        include_str!("fixtures/display-q13.txt"),
        "ok",
    );
    unit_call(
        &mut s,
        15,
        json!({"student":"가상학생B","topic":"클리닉"}),
        include_str!("fixtures/display-q14-ask.txt"),
        "ask",
    );
    unit_call(
        &mut s,
        16,
        json!({"student":"가상학생B","topic":"클리닉","book":"가우스 5-1-1"}),
        include_str!("fixtures/display-q14-result.txt"),
        "ok",
    );
    unit_call(
        &mut s,
        17,
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"섹션"}),
        include_str!("fixtures/display-q15.txt"),
        "ok",
    );
    unit_call(
        &mut s,
        18,
        json!({"student":"가상학생A","unit":"1-2","topic":"섹션"}),
        include_str!("fixtures/display-q15-bare.txt"),
        "ok",
    );
    unit_call(
        &mut s,
        19,
        json!({"student":"가상학생A","unit":"2-3","topic":"섹션"}),
        include_str!("fixtures/display-q16.txt"),
        "ok",
    );
}

#[test]
fn unit_invalid_requests_read_no_sources_and_stop_is_error() {
    let mut s = Server::start_with_log(Some("unit"));
    unit_call(
        &mut s,
        20,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
        include_str!("fixtures/display-q11.txt"),
        "ok",
    );
    for (i, args) in [
        json!({}),
        json!({"student":4}),
        json!({"student":"가상학생A","unit":null}),
        json!({"student":"가상학생A","offset":1}),
        json!([]),
    ]
    .into_iter()
    .enumerate()
    {
        let out = s.call(json!({"id":i,"method":"tools/call","params":{"name":"academy_unit_status","arguments":args}}));
        assert_eq!(out["error"]["code"], -32602);
        assert_eq!(out["id"], i);
        assert!(out.get("result").is_none());
    }
    let text = std::fs::read_to_string(&s.run_log).unwrap();
    assert_eq!(text.lines().count(), 6);
    let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert!(first["source_reads"].as_u64().unwrap() > 0);
    for line in text.lines().skip(1) {
        assert_eq!(
            serde_json::from_str::<Value>(line).unwrap()["source_reads"],
            0
        );
    }
    let stopped = s.call(json!({"id":6,"method":"tools/call","params":{"name":"academy_unit_status","arguments":{"student":"가상학생Z"}}}));
    let result = &stopped["result"];
    assert_eq!(result["isError"], true);
    assert_eq!(
        result["_meta"]["academy/envelope"]["stop"]["code"],
        "student_not_found"
    );
    assert_eq!(
        result["content"][1]["text"],
        result["_meta"]["academy/envelope"]["display"]
    );
    assert!(result.get("structuredContent").is_none());
}

#[test]
fn unit_generated_student_label_prints_the_same_frame_time() {
    let mut exact = Server::start_with_log(Some("unit"));
    let mut choice = Server::start_with_log(Some("unit"));
    for (i, time) in [(0, "09:00"), (1, "10:00")] {
        let mut replies = Vec::new();
        for (server, student) in [
            (&mut exact, "가상학생B"),
            (&mut choice, "가상학생B (가상반A)"),
        ] {
            let out = server.call(json!({"id":i,"method":"tools/call","params":{"name":"academy_unit_status","arguments":{"student":student,"unit":"4-4","topic":"DailyTest"}}}));
            let envelope = &out["result"]["_meta"]["academy/envelope"];
            assert_eq!(envelope["status"], "ok");
            let text = out["result"]["content"][1]["text"].as_str().unwrap();
            assert!(text.contains(&format!("읽은 원천 (11월 11일 {time})")));
            replies.push(envelope["data"]["read_at"].clone());
        }
        assert_eq!(replies[0], replies[1]);
        assert!(replies[0].as_str().unwrap().contains(time));
    }
}
