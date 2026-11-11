//! Append-only call log: one JSON line per `tools/call`, metadata only.
//! Never an argument value, a name or a display text.
use std::{
    fs::File,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct CallLogEntry {
    pub tool: String,
    pub args_bytes: usize,
    pub status: String,
    pub code: Option<String>,
    pub display_bytes: usize,
    pub duration_ms: u128,
    pub source_reads: usize,
}

/// Current time as an ISO-8601 UTC string, from the host clock.
pub fn now_ts() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { y + 1 } else { y }, month, day)
}

/// A single opened descriptor per run; failures disable logging, never calls.
pub struct CallLog {
    file: Option<File>,
}

impl CallLog {
    pub fn open(path: &Path) -> Option<Self> {
        use rustix::fs::{self, FileType, Mode, OFlags};
        let open = || -> std::io::Result<File> {
            let fd = fs::open(
                path,
                OFlags::WRONLY
                    | OFlags::CREATE
                    | OFlags::APPEND
                    | OFlags::NOFOLLOW
                    | OFlags::CLOEXEC
                    | OFlags::NONBLOCK,
                Mode::from_raw_mode(0o600),
            )?;
            let stat = fs::fstat(&fd)?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
                return Err(std::io::Error::other("log is not a regular file"));
            }
            fs::fchmod(&fd, Mode::from_raw_mode(0o600))?;
            Ok(File::from(fd))
        };
        match open() {
            Ok(file) => Some(Self { file: Some(file) }),
            Err(_) => {
                eprintln!("call_log_disabled: cannot securely open log");
                None
            }
        }
    }

    pub fn append(&mut self, entry: &CallLogEntry) {
        let line = format!(
            "{}\n",
            serde_json::json!({
                "ts": now_ts(), "tool": entry.tool, "args_bytes": entry.args_bytes,
                "status": entry.status, "code": entry.code,
                "display_bytes": entry.display_bytes, "duration_ms": entry.duration_ms,
                "source_reads": entry.source_reads,
            })
        );
        if let Some(file) = &mut self.file
            && file.write_all(line.as_bytes()).is_err()
        {
            self.file = None;
            eprintln!("call_log_disabled: cannot append log");
        }
    }
}
