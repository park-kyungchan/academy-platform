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
