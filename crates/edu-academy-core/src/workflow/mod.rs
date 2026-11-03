//! Existing owner-scoped Education journals and participating filesystem locks.
//! No native default root, account selection, source transport or policy authority.
pub mod approval;
pub mod cism;
pub mod codec;
pub mod http;
pub mod native;
pub mod read_failure;
pub mod review;
pub mod schema;
pub mod service;
pub mod state;
pub mod store;

use crate::js::{Fault, Js, Result};
use std::io;
pub(crate) fn io_fault(e: io::Error) -> Fault {
    // Only OS classifications cross this boundary; paths and payloads stay private.
    Fault::error(match e.kind() {
        io::ErrorKind::NotFound => "ENOENT",
        io::ErrorKind::AlreadyExists => "EEXIST",
        io::ErrorKind::PermissionDenied => "EACCES",
        _ => "workflow_filesystem_failed",
    })
}
pub fn canonical(v: &Js) -> Result<String> {
    crate::js::encode(v, crate::js::Family::Admission, &mut vec![])?
        .ok_or_else(|| Fault::error("dailyrecord_save_action_not_json"))
}
pub fn same(a: &Js, b: &Js) -> Result<bool> {
    Ok(canonical(a)? == canonical(b)?)
}
pub fn merge(mut a: Js, b: &Js) -> Js {
    for (k, v) in b.props() {
        a.set(&String::from_utf16_lossy(k), v.clone());
    }
    a
}
pub fn nullable(v: &Js) -> Js {
    if v.absent() { Js::Null } else { v.clone() }
}
/// Clock and entropy supplied by the owner. Test clocks belong in migration tools.
pub trait Runtime {
    fn now(&self) -> String;
    fn wall_now(&self) -> String {
        self.now()
    }
    /// Notification after the ledger is durable, before the execution record.
    fn indexed(&self) -> Result<()> {
        Ok(())
    }
    /// Notification after the execution fence is durable, before approval returns.
    fn fenced(&self) -> Result<()> {
        Ok(())
    }
    fn uuid(&self) -> Result<String>;
    fn epoch_ms(&self) -> u64;
    fn pid(&self) -> u32 {
        std::process::id()
    }
}

/// Upstream process observation, matching the original fallback when the bridge
/// supplies no attribution. Attribution is audit evidence, never authentication.
pub fn observe_attribution(runtime: &dyn Runtime, declared: &Js) -> Js {
    use crate::js::{obj, text};
    use std::os::unix::fs::MetadataExt;
    let pid = std::process::id();
    let ppid = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            s.rsplit_once(')')
                .and_then(|(_, tail)| tail.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u32>().ok())
        })
        .unwrap_or(0);
    let uid = std::fs::metadata("/proc/self")
        .ok()
        .map(|m| Js::Num(f64::from(m.uid())))
        .unwrap_or(Js::Null);
    let parent = std::fs::read(format!("/proc/{ppid}/cmdline"))
        .ok()
        .and_then(|bytes| {
            let first = bytes.split(|b| *b == 0).next()?;
            let name = std::str::from_utf8(first).ok()?.rsplit('/').next()?;
            let units = text(name).into_iter().take(200).collect::<Vec<_>>();
            if units.is_empty() {
                None
            } else {
                Some(Js::Str(units))
            }
        })
        .unwrap_or(Js::Null);
    let pane = std::env::var("HERDR_PANE_ID")
        .ok()
        .map(|s| s.trim().encode_utf16().take(200).collect::<Vec<_>>())
        .filter(|s| !s.is_empty())
        .map(Js::Str)
        .unwrap_or(Js::Null);
    obj(&[
        ("schemaVersion", "academy.dailyrecord-attribution/1".into()),
        ("observedBy", "upstream".into()),
        ("observedAt", runtime.wall_now().into()),
        ("client", Js::Null),
        (
            "process",
            obj(&[
                ("pid", Js::Num(f64::from(pid))),
                ("ppid", Js::Num(f64::from(ppid))),
                ("uid", uid),
                ("parentCommand", parent),
                ("bridgeSha256", Js::Null),
            ]),
        ),
        ("herdrPaneId", pane),
        ("declared", nullable(declared)),
    ])
}
