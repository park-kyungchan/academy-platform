use edu_academy_core::protocol::{Request, execute};
use std::io::{Read, Write};

fn main() {
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(16_777_217)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 16_777_216
    {
        fail("s08_protocol_input_rejected", 64);
    }
    #[derive(serde::Deserialize)]
    struct Dispatch {
        op: String,
    }
    let dispatch: Dispatch =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| fail("s08_protocol_input_rejected", 64));
    if dispatch.op == "semantic_v4" {
        let request: edu_academy_core::extension::Request = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| fail("s08_protocol_input_rejected", 64));
        let frame = edu_academy_core::extension::execute(request)
            .unwrap_or_else(|message| fail(message, 64));
        if std::io::stdout().write_all(frame.as_bytes()).is_err() {
            fail("s08_protocol_output_failed", 74);
        }
        return;
    }
    if bytes.len() > 1_048_576 {
        fail("s08_protocol_input_rejected", 64);
    }
    if dispatch.op == "semantic_v3" {
        let request: edu_academy_core::semantic::Request = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| fail("s08_protocol_input_rejected", 64));
        let frame = edu_academy_core::semantic::execute(request)
            .unwrap_or_else(|message| fail(message, 64));
        if std::io::stdout().write_all(frame.as_bytes()).is_err() {
            fail("s08_protocol_output_failed", 74);
        }
        return;
    }
    let request: Request =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| fail("s08_protocol_input_rejected", 64));
    let frame = execute(request).unwrap_or_else(|error| fail(error.message(), 78));
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if serde_json::to_writer(&mut out, &frame).is_err() || out.write_all(b"\n").is_err() {
        fail("s08_protocol_output_failed", 74);
    }
}

fn fail(message: &str, status: i32) -> ! {
    eprintln!("{message}");
    std::process::exit(status)
}
