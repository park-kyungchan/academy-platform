use edu_academy_core::protocol::{Request, execute};
use std::io::{Read, Write};

fn main() {
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 1_048_576
    {
        fail("s08_protocol_input_rejected", 64);
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
