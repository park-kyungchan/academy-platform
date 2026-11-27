//! `edu`: the platform's one command. Standard output only; nothing on standard error.
use edu_academy_core::workflow::Runtime;
use edu_academy_sources::http::{self, Cancellation, NativeTransport, TransportLimits};
use std::{io::Write, time::UNIX_EPOCH};

struct Clock;
impl Runtime for Clock {
    fn epoch_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
    fn now(&self) -> String {
        String::new()
    }
    fn uuid(&self) -> edu_academy_core::js::Result<String> {
        Err(edu_academy_core::js::Fault::error("unsupported"))
    }
}
fn main() {
    // A word that is not UTF-8 becomes empty, which every verb refuses.
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.into_string().unwrap_or_default())
        .collect();
    let ids = (
        rustix::process::getuid().as_raw(),
        rustix::process::geteuid().as_raw(),
    );
    let out = edu::run(&args, &Clock, ids, &mut |cancellation: &Cancellation| {
        NativeTransport::new(
            TransportLimits {
                request_bytes: http::MAX_REQUEST_BYTES,
                response_bytes: 6_000_001,
                timeout: std::time::Duration::from_secs(20),
            },
            cancellation.clone(),
        )
    });
    let mut stdout = std::io::stdout();
    if stdout.write_all(out.stdout.as_bytes()).is_err() || stdout.flush().is_err() {
        std::process::exit(2);
    }
    std::process::exit(out.code);
}
