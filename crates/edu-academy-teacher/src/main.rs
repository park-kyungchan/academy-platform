//! Stdio MCP server (JSON-RPC 2.0, one message per line) for slice 1.
//! The synthetic backend is the only backend in this slice.
use chrono::NaiveDate;
use edu_academy_teacher::{
    backend::SourceBackend,
    envelope::{Envelope, Status},
    facts::SeededFacts,
    lesson::PrepareRequest,
    load_scenario,
    log::{CallLog, CallLogEntry},
    synthetic::{SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort},
    tools::{self, Handler, tools_list_minified},
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

// Variant B: only display is model-visible; the envelope is client metadata.
const ENVELOPE_META_KEY: &str = "academy/envelope";
const INSTRUCTIONS: &str = "Each tool result of this server is finished Korean text for the teacher. Show that text to the teacher exactly as it is and in full, with nothing added before or after it: no summary, no rewording, no advice, no extra question. When the text asks the teacher something or lists choices, stop there and wait for the teacher's answer, then call the tool again with that answer. When the teacher later asks about a result already shown and no tool is needed, answer briefly in your own words from that result only. Call a tool again only when the teacher asks for something else or answers a question.";

const MAX_MESSAGE_BYTES: usize = 1_048_576;

enum Message {
    Bytes(Vec<u8>),
    TooLong,
}

// Bound allocation while consuming the entire line, including an oversized
// tail. EOF may terminate the last message without a newline.
fn read_message(input: &mut impl BufRead) -> std::io::Result<Option<Message>> {
    let mut bytes = Vec::new();
    let mut too_long = false;
    let mut seen = false;
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return Ok(if seen {
                Some(if too_long {
                    Message::TooLong
                } else {
                    Message::Bytes(bytes)
                })
            } else {
                None
            });
        }
        seen = true;
        let newline = chunk.iter().position(|b| *b == b'\n');
        let length = newline.unwrap_or(chunk.len());
        if !too_long {
            if length > MAX_MESSAGE_BYTES - bytes.len() {
                too_long = true;
                bytes.clear();
            } else {
                bytes.extend_from_slice(&chunk[..length]);
            }
        }
        input.consume(length + usize::from(newline.is_some()));
        if newline.is_some() {
            return Ok(Some(if too_long {
                Message::TooLong
            } else {
                Message::Bytes(bytes)
            }));
        }
    }
}

const SUPPORTED_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];

type Backend = SourceBackend<SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort>;

fn seoul_today() -> NaiveDate {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + 9 * 3600;
    let days = secs.div_euclid(86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as i32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { y + 1 } else { y } as i32;
    NaiveDate::from_ymd_opt(year, month, day as u32).unwrap_or_default()
}

fn status_str(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Ask => "ask",
        Status::Saved => "saved",
        Status::Held => "held",
        Status::Unavailable => "unavailable",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut scenario_path: Option<PathBuf> = None;
    let mut today: Option<NaiveDate> = None;
    let mut run_log: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        let (target, label): (&mut Option<PathBuf>, &str) = match args[index].as_str() {
            "--scenario" => (&mut scenario_path, "--scenario"),
            "--today" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --today");
                    std::process::exit(2);
                };
                match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
                    Ok(date) => today = Some(date),
                    Err(_) => {
                        eprintln!("invalid --today date: {value}");
                        std::process::exit(2);
                    }
                }
                index += 2;
                continue;
            }
            "--run-log" => (&mut run_log, "--run-log"),
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        };
        let Some(value) = args.get(index + 1) else {
            eprintln!("missing value for {label}");
            std::process::exit(2);
        };
        *target = Some(PathBuf::from(value));
        index += 2;
    }
    let Some(scenario_path) = scenario_path else {
        eprintln!("missing argument: --scenario");
        std::process::exit(2);
    };
    let today = today.unwrap_or_else(seoul_today);
    let text = match std::fs::read_to_string(&scenario_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("scenario_read_failed: {error}");
            std::process::exit(2);
        }
    };
    let scenario = match load_scenario(&text) {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let facts = SeededFacts::from_scenario(&scenario);
    let read_at = format!("{}T09:00:00+09:00", today.format("%Y-%m-%d"));
    let mut backend: Backend = SourceBackend::synthetic(scenario, &read_at);

    let mut run_log = run_log.and_then(|path| CallLog::open(&path));
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut stdout = std::io::stdout().lock();
    while let Ok(Some(message)) = read_message(&mut input) {
        let parsed = match message {
            Message::Bytes(bytes) => {
                if bytes.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                serde_json::from_slice::<Value>(&bytes).ok()
            }
            Message::TooLong => None,
        };
        let request: Value = match parsed {
            Some(value) => value,
            None => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {"code": -32700, "message": "Parse error"}
                });
                let _ = writeln!(stdout, "{response}");
                let _ = stdout.flush();
                continue;
            }
        };
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            if request.is_array() || request.get("id").is_some() {
                let response = json!({"jsonrpc":"2.0", "id":request.get("id").cloned().unwrap_or(Value::Null),
                    "error":{"code":-32600,"message":"Invalid Request"}});
                let _ = writeln!(stdout, "{response}");
                let _ = stdout.flush();
            }
            continue;
        };
        let id = request.get("id").cloned();
        let Some(id) = id else {
            // Notification (including notifications/initialized): no reply.
            continue;
        };
        let response_line = match method {
            "initialize" => {
                let requested = request
                    .get("params")
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = if SUPPORTED_VERSIONS.contains(&requested) {
                    requested
                } else {
                    SUPPORTED_VERSIONS[0]
                };
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": version,
                        "instructions": INSTRUCTIONS,
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "academy", "version": env!("CARGO_PKG_VERSION")}
                    }
                });
                response.to_string()
            }
            "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}).to_string(),
            "tools/list" => format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}",
                serde_json::to_string(&id).unwrap_or_else(|_| "null".into()),
                tools_list_minified()
            ),
            "tools/call" => {
                let started = Instant::now();
                let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let args_text = serde_json::to_string(&arguments).unwrap_or_default();
                let before_reads = backend.source_reads();
                let envelope: Envelope = match tools::dispatch(&name) {
                    Some(Handler::LessonPrepare) => {
                        let text_arg = |key: &str| {
                            arguments
                                .get(key)
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        };
                        let req = PrepareRequest {
                            student: text_arg("student"),
                            class_name: text_arg("class_name"),
                            date: text_arg("date"),
                            book: text_arg("book"),
                        };
                        edu_academy_teacher::prepare(&mut backend, &facts, &req, today)
                    }
                    Some(Handler::NotInSlice1) => Handler::NotInSlice1
                        .unavailable_envelope()
                        .unwrap_or_else(Envelope::unavailable_tool),
                    None => Envelope::unknown_tool(),
                };
                let source_reads = backend.source_reads().saturating_sub(before_reads);
                if let Some(log) = &mut run_log {
                    let code = if let Some(stop) = &envelope.stop {
                        Some(stop.code.as_str().to_string())
                    } else {
                        envelope
                            .ask
                            .as_ref()
                            .map(|ask| format!("ask:{}", ask.field.as_str()))
                    };
                    // A log write failure never fails the call.
                    log.append(&CallLogEntry {
                        tool: if tools::dispatch(&name).is_some() {
                            name.clone()
                        } else {
                            "unknown".into()
                        },
                        args_bytes: args_text.len(),
                        status: status_str(envelope.status).to_string(),
                        code,
                        display_bytes: envelope.display.len(),
                        duration_ms: started.elapsed().as_millis(),
                        source_reads,
                    });
                }
                let structured = serde_json::to_value(&envelope).unwrap_or_else(|_| json!({}));
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{"type": "text", "text": envelope.display}],
                        "_meta": {(ENVELOPE_META_KEY): structured},
                        "isError": false
                    }
                });
                response.to_string()
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": "Method not found"}
            })
            .to_string(),
        };
        let _ = writeln!(stdout, "{response_line}");
        let _ = stdout.flush();
    }
}
