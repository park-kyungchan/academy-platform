//! Dedicated native bridge: safe signal receipt precedes product I/O threads.
//! Endpoint mode never fabricates a qualified business port.
use edu_academy_mcp::{
    Error, Js, Result, audit,
    framing::{self, BridgeInput, InputEvent, InputPump, OutputPump, write_message},
    identity::verify_executable,
    obj,
    process::{self, Control, Limits},
    protocol::{RpcSession, valid_envelope},
    release::ReleaseGraph,
};
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::{
    collections::VecDeque,
    io::{self, BufReader},
    path::PathBuf,
    sync::mpsc::TryRecvError,
    time::{Duration, Instant},
};
/// Cancellation belongs to request identity even while that request is queued.
/// Only active/queued IDs are retained, so unknown notifications cannot grow state.
#[derive(Default)]
struct PendingInput {
    messages: VecDeque<Js>,
    cancelled: Vec<Js>,
}
impl PendingInput {
    fn push(&mut self, message: Js, active: &Js) -> Result<()> {
        if valid_envelope(&message)
            && !message.has("id")
            && message.get("method").is("notifications/cancelled")
        {
            let id = message.get("params").get("requestId");
            let known = !id.absent()
                && ((!active.absent() && id == active)
                    || self
                        .messages
                        .iter()
                        .any(|m| m.has("id") && m.get("id") == id));
            if known && !self.is_cancelled(id) {
                self.cancelled.push(id.clone());
            }
            return Ok(());
        }
        if self.messages.len() >= 16 {
            return Err(Error::new("mcp_pending_input_capacity_exceeded"));
        }
        self.messages.push_back(message);
        Ok(())
    }
    fn is_cancelled(&self, id: &Js) -> bool {
        self.cancelled.contains(id)
    }
    fn take_cancelled(&mut self, id: &Js) -> bool {
        if let Some(at) = self.cancelled.iter().position(|v| v == id) {
            self.cancelled.remove(at);
            true
        } else {
            false
        }
    }
    fn pop_front(&mut self) -> Option<Js> {
        while let Some(message) = self.messages.pop_front() {
            if !message.has("id") || !self.take_cancelled(message.get("id")) {
                return Some(message);
            }
        }
        None
    }
}
struct ParentControl {
    signals: Signals,
    input: InputPump,
    pending: PendingInput,
    diagnostics: u8,
    eof: bool,
    failure: Option<Error>,
    signal: Option<i32>,
}
impl ParentControl {
    fn signals_only(&mut self) -> Result<()> {
        for signal in self.signals.pending() {
            self.signal.get_or_insert(signal);
        }
        if self.signal.is_some() {
            Err(Error::new("parent_signal_cancelled"))
        } else {
            Ok(())
        }
    }
    fn poll_input(
        &mut self,
        active: &Js,
        before_notification: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.signals_only()?;
        for _ in 0..32 {
            match self.input.try_next() {
                Ok(InputEvent::Message(message)) => {
                    let notification = valid_envelope(&message) && !message.has("id");
                    if notification {
                        before_notification()?;
                    }
                    self.pending.push(message, active)?;
                }
                Ok(InputEvent::Diagnostic) => self.diagnostics = self.diagnostics.saturating_add(1),
                Ok(InputEvent::Eof) => self.eof = true,
                Ok(InputEvent::Failed(e)) => self.failure = Some(e),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.eof = true;
                    break;
                }
            }
        }
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.eof {
            return Err(Error::new("mcp_client_eof"));
        }
        if self.pending.is_cancelled(active) {
            return Err(Error::new("request_cancelled"));
        }
        Ok(())
    }
}
impl Control for ParentControl {
    fn poll(
        &mut self,
        request_id: &Js,
        before_notification: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.poll_input(request_id, before_notification)
    }
    fn adopt_orphans(&self) -> bool {
        true
    }
}
struct Exit {
    result: Result<()>,
    signal: Option<i32>,
}
fn endpoint(args: &[String], root: &PathBuf) -> Result<()> {
    process::child_parent_guard()?;
    let graph = ReleaseGraph::open(root)?;
    let config = graph
        .endpoints
        .get(&args[3])
        .ok_or_else(|| Error::new("release_endpoint_absent"))?;
    if config.args != args {
        return Err(Error::new("release_endpoint_identity_mismatch"));
    }
    let executable = std::env::current_exe()
        .map_err(|_| Error::new("loaded_executable_identity_unavailable"))?;
    verify_executable(&config.executable, &executable)?;
    let mut session = RpcSession::endpoint(graph, args[3].clone())?;
    // Default native TERM remains available in a worker blocked in domain I/O.
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = BridgeInput::new(BufReader::new(stdin.lock()));
    let mut output = stdout.lock();
    let mut diagnostics = Vec::new();
    let mut protocol_diagnostics = 0;
    loop {
        let request = input.next(&mut diagnostics)?;
        if !diagnostics.is_empty() {
            use std::io::Write;
            io::stderr()
                .lock()
                .write_all(&diagnostics)
                .map_err(|_| Error::new("mcp_stdio_write_failed"))?;
            diagnostics.clear();
        }
        let Some(request) = request else {
            break;
        };
        match session.handle(&request) {
            Ok(Some(response)) => write_message(&mut output, &response)?,
            Ok(None) => (),
            Err(_) if protocol_diagnostics < 16 => {
                protocol_diagnostics += 1;
                // Stderr belongs to the child pipe and is continuously drained.
                write_message(&mut io::stderr().lock(), &framing::diagnostic())?;
            }
            Err(_) => (),
        }
    }
    Ok(())
}
fn bridge(root: &PathBuf) -> Exit {
    let startup = (|| -> Result<(RpcSession, Signals)> {
        let signals =
            Signals::new([SIGTERM]).map_err(|_| Error::new("safe_signal_receipt_unavailable"))?;
        process::install_binary_subreaper()?;
        let graph = ReleaseGraph::open(root)?;
        let executable = std::env::current_exe()
            .map_err(|_| Error::new("loaded_executable_identity_unavailable"))?;
        verify_executable(&graph.pointer.bridge, &executable)?;
        Ok((RpcSession::new(graph), signals))
    })();
    let (mut session, signals) = match startup {
        Ok(v) => v,
        Err(e) => {
            return Exit {
                result: Err(e),
                signal: None,
            };
        }
    };
    let mut control = ParentControl {
        signals,
        input: InputPump::stdin(),
        pending: PendingInput::default(),
        diagnostics: 0,
        eof: false,
        failure: None,
        signal: None,
    };
    let output = OutputPump::stdout();
    let errors = OutputPump::stderr();
    let mut protocol_diagnostics = 0;
    let result = (|| -> Result<()> {
        loop {
            let idle = control.poll_input(&Js::Undefined, &mut || session.check_graph());
            while control.diagnostics > 0 {
                control.diagnostics -= 1;
                errors.send(
                    &framing::diagnostic(),
                    Instant::now() + Duration::from_millis(100),
                    || control.signals_only(),
                )?;
            }
            idle?;
            let Some(request) = control.pending.pop_front() else {
                std::thread::sleep(process::TICK);
                continue;
            };
            let response = session.handle_supervised(&request, &mut control, Limits::default());
            if control.signal.is_some() {
                return Err(Error::new("parent_signal_cancelled"));
            }
            if let Some(error) = control.failure {
                return Err(error);
            }
            if control.eof {
                response?;
                return Err(Error::new("mcp_client_eof"));
            }
            if control.pending.take_cancelled(request.get("id")) {
                continue; // SDK does not answer an aborted request.
            }
            match response {
                Ok(Some(response)) => {
                    let refusal =
                        response.has("error") || response.get("result").get("isError").truthy();
                    output.send(&response, Instant::now() + Duration::from_secs(2), || {
                        if !refusal {
                            session.check_graph()?;
                        }
                        control.poll_input(request.get("id"), &mut || session.check_graph())
                    })?;
                }
                Ok(None) => (),
                Err(_) if protocol_diagnostics < 16 => {
                    protocol_diagnostics += 1;
                    errors.send(
                        &framing::diagnostic(),
                        Instant::now() + Duration::from_millis(100),
                        || control.poll_input(&Js::Undefined, &mut || session.check_graph()),
                    )?;
                }
                Err(_) => (),
            }
        }
    })();
    let mut signal = control.signal;
    let input_joined = control.input.finish();
    let output_joined = output.finish();
    let errors_joined = errors.finish();
    for received in control.signals.pending() {
        signal.get_or_insert(received);
    }
    let result = match result {
        // EOF may arrive while acknowledging a response already read by the
        // client. Normalize only this exact close after I/O actually joined;
        // blocked writers and unrelated errors retain their failure outcome.
        Err(error)
            if error.code == "mcp_client_eof"
                && control.eof
                && control.failure.is_none()
                && signal.is_none()
                && input_joined
                && output_joined
                && errors_joined =>
        {
            Ok(())
        }
        other => other,
    };
    // Explicit observation rather than claiming that dropping a future joined I/O.
    let io = session.io_receipt(&obj(&[
        ("completedAt", audit::wall_time().into()),
        ("inputThreadJoined", input_joined.into()),
        ("stdoutThreadJoined", output_joined.into()),
        ("stderrThreadJoined", errors_joined.into()),
        ("blockingInheritedStdioFlagsChanged", false.into()),
        (
            "failureCode",
            result.as_ref().err().map_or(Js::Null, |e| e.code.into()),
        ),
        ("signal", signal.map_or(Js::Null, |v| Js::Num(f64::from(v)))),
    ]));
    for received in control.signals.pending() {
        signal.get_or_insert(received);
    }
    Exit {
        result: result.and(io),
        signal,
    }
}
fn run() -> Exit {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if (args.len() != 2 && args.len() != 4)
        || args.first().map(String::as_str) != Some("--install-root")
        || (args.len() == 4 && args[2] != "--endpoint")
    {
        return Exit {
            result: Err(Error::new("explicit_install_root_required")),
            signal: None,
        };
    }
    let root = PathBuf::from(&args[1]);
    if args.len() == 4 {
        Exit {
            result: endpoint(&args, &root),
            signal: None,
        }
    } else {
        bridge(&root)
    }
}
fn main() {
    let exit = run();
    if let Some(signal) = exit.signal {
        // Restore/re-raise through the pinned safe API after owned child cleanup.
        // A failed emulation exits1; it is never reported as a fabricated signal.
        let _ = signal_hook::low_level::emulate_default_handler(signal);
        signal_hook::low_level::exit(1);
    }
    if let Err(error) = exit.result {
        let errors = OutputPump::stderr();
        let _ = errors.send(
            &obj(&[
                ("code", error.code.into()),
                (
                    "evidenceLocator",
                    "crates/edu-academy-mcp/src/bin/edu-academy-mcp.rs".into(),
                ),
            ]),
            Instant::now() + Duration::from_millis(100),
            || Ok(()),
        );
        signal_hook::low_level::exit(1);
    }
    // _exit avoids at-exit stdio locking when an inherited writer is blocked.
    signal_hook::low_level::exit(0);
}

#[cfg(test)]
mod queued_cancellation_tests {
    use super::*;
    fn call(id: Js, tool: &str) -> Js {
        obj(&[
            ("jsonrpc", "2.0".into()),
            ("id", id),
            ("method", "tools/call".into()),
            (
                "params",
                obj(&[("name", tool.into()), ("arguments", obj(&[]))]),
            ),
        ])
    }
    fn cancel(id: Js) -> Js {
        obj(&[
            ("jsonrpc", "2.0".into()),
            ("method", "notifications/cancelled".into()),
            ("params", obj(&[("requestId", id)])),
        ])
    }
    #[test]
    fn a_save_cancelled_in_the_same_idle_batch_never_reaches_dispatch() {
        let mut pending = PendingInput::default();
        // Deterministic same-poll order: call, cancellation, then a live call.
        pending
            .push(
                call("save-😀".into(), "academy_dailyrecord_save"),
                &Js::Undefined,
            )
            .unwrap();
        pending
            .push(cancel("save-😀".into()), &Js::Undefined)
            .unwrap();
        pending
            .push(call("read".into(), "academy_source_status"), &Js::Undefined)
            .unwrap();
        let mut save_dispatches = 0;
        while let Some(request) = pending.pop_front() {
            if request
                .get("params")
                .get("name")
                .is("academy_dailyrecord_save")
            {
                save_dispatches += 1;
            }
            assert!(request.get("id").is("read"));
        }
        assert_eq!(save_dispatches, 0);
        assert!(pending.cancelled.is_empty());
    }
    #[test]
    fn queued_and_active_cancellations_preserve_exact_id_and_bounded_state() {
        let mut pending = PendingInput::default();
        pending
            .push(
                call(Js::Num(1.0), "academy_dailyrecord_save"),
                &"active".into(),
            )
            .unwrap();
        pending.push(cancel("1".into()), &"active".into()).unwrap();
        pending
            .push(cancel("active".into()), &"active".into())
            .unwrap();
        assert!(pending.is_cancelled(&"active".into()));
        assert!(pending.pop_front().unwrap().get("id") == &Js::Num(1.0));
        assert!(pending.take_cancelled(&"active".into()));
        for n in 0..100 {
            pending
                .push(cancel(Js::Num(n as f64)), &Js::Undefined)
                .unwrap();
        }
        assert!(pending.cancelled.is_empty());
    }
}
