//! Inactive first-increment native metadata bridge. No source child/HTTP dispatch.
use edu_academy_mcp::{
    Error, Result,
    framing::{BridgeInput, write_message},
    identity::verify_executable,
    protocol::RpcSession,
    release::ReleaseGraph,
};
use std::{
    io::{self, BufReader, Write},
    path::PathBuf,
};

fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 && args.len() != 4
        || args.first().map(String::as_str) != Some("--install-root")
    {
        return Err(Error::new("explicit_install_root_required"));
    }
    let root = PathBuf::from(&args[1]);
    let graph = ReleaseGraph::open(&root)?;
    let current = std::env::current_exe()
        .map_err(|_| Error::new("loaded_executable_identity_unavailable"))?;
    if args.len() == 4 {
        if args[2] != "--endpoint" {
            return Err(Error::new("release_argv_invalid"));
        }
        let endpoint = graph
            .endpoints
            .get(&args[3])
            .ok_or_else(|| Error::new("release_endpoint_absent"))?;
        if endpoint.args != args {
            return Err(Error::new("release_endpoint_identity_mismatch"));
        }
        verify_executable(&endpoint.executable, &current)?;
        // A declared endpoint cannot turn metadata admission into domain implementation.
        return Err(Error::new("configured_rust_endpoint_unimplemented"));
    }
    verify_executable(&graph.pointer.bridge, &current)?;
    let mut session = RpcSession::new(graph);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut input = BridgeInput::new(BufReader::new(stdin.lock()));
    let mut output = stdout.lock();
    let mut diagnostics = stderr.lock();
    let mut count = 0;
    while let Some(request) = input.next(&mut diagnostics)? {
        match session.handle(&request) {
            Ok(Some(response)) => write_message(&mut output, &response)?,
            Ok(None) => (),
            Err(_) if count < 16 => {
                count += 1;
                diagnostics.write_all(b"{\"code\":\"mcp_stdio_input_or_transport_invalid\",\"category\":\"protocol\",\"evidenceLocator\":\"src/stdio-diagnostics.ts:createAcademyStdioTransport\",\"automaticRetryAllowed\":false}\n")
                    .map_err(|_| Error::new("mcp_stdio_write_failed"))?;
            }
            Err(_) => (),
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!(
            "{}@crates/edu-academy-mcp/src/bin/edu-academy-mcp.rs:1",
            error.code
        );
        std::process::exit(1);
    }
}
