//! The 1MiB bound belongs only to the original bridge/server input adapter.
//! No domain/output cap is inferred from this input framing policy.
use crate::{Error, Js, Result, encode, obj, parse};
use std::io::{BufRead, Write};

pub const BRIDGE_INPUT_LINE_BYTES: usize = 1024 * 1024;
/// Pinned SDK2.3.1 ReadBuffer default, for a separately admitted child stream.
pub const SDK_CHILD_RECEIVE_BUFFER_BYTES: usize = 10 * 1024 * 1024;

pub struct BridgeInput<R> {
    reader: R,
    diagnostics: u8,
    closed: bool,
}
impl<R: BufRead> BridgeInput<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            diagnostics: 0,
            closed: false,
        }
    }
    fn report(&mut self, diagnostics: &mut impl Write) -> Result<()> {
        if self.diagnostics < 16 {
            self.diagnostics += 1;
            write_message(
                diagnostics,
                &obj(&[
                    ("code", "mcp_stdio_input_or_transport_invalid".into()),
                    ("category", "protocol".into()),
                    (
                        "evidenceLocator",
                        "src/stdio-diagnostics.ts:createAcademyStdioTransport".into(),
                    ),
                    ("automaticRetryAllowed", false.into()),
                ]),
            )?;
        }
        Ok(())
    }
    pub fn next(&mut self, diagnostics: &mut impl Write) -> Result<Option<Js>> {
        if self.closed {
            return Ok(None);
        }
        loop {
            let mut frame = Vec::new();
            loop {
                let available = self
                    .reader
                    .fill_buf()
                    .map_err(|_| Error::new("mcp_stdio_read_failed"))?;
                if available.is_empty() {
                    self.closed = true;
                    if !frame.is_empty() {
                        self.report(diagnostics)?;
                    }
                    return Ok(None);
                }
                let newline = available.iter().position(|b| *b == b'\n');
                let count = newline.map_or(available.len(), |n| n + 1);
                if count > BRIDGE_INPUT_LINE_BYTES.saturating_sub(frame.len()) {
                    self.closed = true;
                    self.report(diagnostics)?;
                    return Err(Error::new("mcp_stdio_frame_too_large"));
                }
                frame.extend_from_slice(&available[..count]);
                self.reader.consume(count);
                if newline.is_some() {
                    break;
                }
            }
            // JSON.parse's UTF-8 conversion replaces malformed byte sequences.
            let decoded = String::from_utf8_lossy(&frame);
            match parse(decoded.as_bytes()) {
                Ok(value) => return Ok(Some(value)),
                Err(_) => self.report(diagnostics)?,
            }
        }
    }
}

/// Output serialization is not subject to the bridge's *input* limit.
pub fn write_message(writer: &mut impl Write, value: &Js) -> Result<()> {
    let bytes = encode(value)?;
    writer
        .write_all(&bytes)
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|_| Error::new("mcp_stdio_write_failed"))
}

/// Bounded dedicated-binary I/O handoffs. Blocking inherited CLI descriptors are
/// never changed or closed by library code. A stopped binary may have unjoined
/// I/O threads; it exits through the safe low-level exit after owned cleanup.
pub enum InputEvent {
    Message(Js),
    Diagnostic,
    Eof,
    Failed(Error),
}
pub struct InputPump {
    receiver: std::sync::mpsc::Receiver<InputEvent>,
    thread: std::thread::JoinHandle<()>,
}
impl InputPump {
    pub fn stdin() -> Self {
        use std::sync::mpsc::sync_channel;
        let (sender, receiver) = sync_channel(8);
        let thread = std::thread::spawn(move || {
            struct Diagnostics(std::sync::mpsc::SyncSender<InputEvent>);
            impl Write for Diagnostics {
                fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                    if bytes.contains(&b'\n') {
                        self.0
                            .send(InputEvent::Diagnostic)
                            .map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
                    }
                    Ok(bytes.len())
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            let stdin = std::io::stdin();
            let mut input = BridgeInput::new(std::io::BufReader::new(stdin.lock()));
            let mut diagnostics = Diagnostics(sender.clone());
            loop {
                match input.next(&mut diagnostics) {
                    Ok(Some(message)) => {
                        if sender.send(InputEvent::Message(message)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = sender.send(InputEvent::Eof);
                        break;
                    }
                    Err(e) => {
                        let _ = sender.send(InputEvent::Failed(e));
                        break;
                    }
                }
            }
        });
        Self { receiver, thread }
    }
    pub fn try_next(&self) -> std::result::Result<InputEvent, std::sync::mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
    pub fn finished(&self) -> bool {
        self.thread.is_finished()
    }
    pub fn finish(self) -> bool {
        let Self { receiver, thread } = self;
        drop(receiver);
        finish_thread(thread)
    }
}
struct WriteTask {
    bytes: Vec<u8>,
    done: std::sync::mpsc::SyncSender<Result<()>>,
}
pub struct OutputPump {
    sender: std::sync::mpsc::SyncSender<WriteTask>,
    thread: std::thread::JoinHandle<()>,
}
impl OutputPump {
    pub fn stdout() -> Self {
        Self::start(false)
    }
    pub fn stderr() -> Self {
        Self::start(true)
    }
    fn start(stderr: bool) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<WriteTask>(1);
        let thread = std::thread::spawn(move || {
            while let Ok(task) = receiver.recv() {
                let result = if stderr {
                    let stream = std::io::stderr();
                    let mut writer = stream.lock();
                    writer.write_all(&task.bytes).and_then(|()| writer.flush())
                } else {
                    let stream = std::io::stdout();
                    let mut writer = stream.lock();
                    writer.write_all(&task.bytes).and_then(|()| writer.flush())
                }
                .map_err(|_| Error::new("mcp_stdio_write_failed"));
                let _ = task.done.send(result);
            }
        });
        Self { sender, thread }
    }
    pub fn send(
        &self,
        value: &Js,
        deadline: std::time::Instant,
        mut poll: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        use std::sync::mpsc::{TryRecvError, TrySendError, sync_channel};
        let mut bytes = encode(value)?;
        bytes.push(b'\n');
        let (done, receiver) = sync_channel(1);
        let mut task = WriteTask { bytes, done };
        loop {
            poll()?;
            if std::time::Instant::now() >= deadline {
                return Err(Error::new("mcp_stdio_output_deadline"));
            }
            match self.sender.try_send(task) {
                Ok(()) => break,
                Err(TrySendError::Full(back)) => {
                    task = back;
                    std::thread::sleep(crate::process::TICK);
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(Error::new("mcp_stdio_write_failed"));
                }
            }
        }
        loop {
            match receiver.try_recv() {
                Ok(result) => return result,
                Err(TryRecvError::Disconnected) => {
                    return Err(Error::new("mcp_stdio_write_failed"));
                }
                Err(TryRecvError::Empty) => (),
            }
            poll()?;
            if std::time::Instant::now() >= deadline {
                return Err(Error::new("mcp_stdio_output_deadline"));
            }
            std::thread::sleep(crate::process::TICK);
        }
    }
    pub fn finished(&self) -> bool {
        self.thread.is_finished()
    }
    pub fn finish(self) -> bool {
        let Self { sender, thread } = self;
        drop(sender);
        finish_thread(thread)
    }
}
fn finish_thread(thread: std::thread::JoinHandle<()>) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(25);
    while !thread.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(crate::process::TICK);
    }
    thread.is_finished() && thread.join().is_ok()
}
pub fn diagnostic() -> Js {
    obj(&[
        ("code", "mcp_stdio_input_or_transport_invalid".into()),
        ("category", "protocol".into()),
        (
            "evidenceLocator",
            "src/stdio-diagnostics.ts:createAcademyStdioTransport".into(),
        ),
        ("automaticRetryAllowed", false.into()),
    ])
}
