//! V3 native exec replacement: no Bun/shell fallback, no hidden child exit code.
use edu_academy_mcp::{Error, Result, identity::verify_executable, release::ReleaseGraph};
use std::{os::unix::process::CommandExt, path::PathBuf, process::Command};

fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 || args[0] != "--install-root" {
        return Err(Error::new("explicit_install_root_required"));
    }
    let root = PathBuf::from(&args[1]);
    let graph = ReleaseGraph::open(&root)?;
    verify_executable(
        &graph.pointer.launcher,
        &std::env::current_exe()
            .map_err(|_| Error::new("loaded_executable_identity_unavailable"))?,
    )?;
    graph.verify_unchanged()?;
    // No source credential environment is inherited. Business launch configuration
    // and safe supervised children require the later owner/transport grant.
    let _exec_error = Command::new(&graph.pointer.bridge.path)
        .args([
            "--install-root",
            root.to_str()
                .ok_or_else(|| Error::new("release_argv_invalid"))?,
        ])
        .env_clear()
        .env("LANG", "C.UTF-8")
        .env("TZ", "UTC")
        .exec();
    Err(Error::new("launcher_exec_failed"))
}
fn main() {
    if let Err(error) = run() {
        eprintln!(
            "{}@crates/edu-academy-mcp/src/bin/edu-academy-launcher.rs:1",
            error.code
        );
        std::process::exit(1);
    }
}
