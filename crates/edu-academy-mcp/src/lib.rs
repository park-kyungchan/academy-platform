//! Immutable MCP sessions and owned native transport. Domain ports remain unqualified.
#![forbid(unsafe_code)]

pub mod audit;
pub mod business;
pub mod dailyrecord_read;
pub mod dispatch;
pub mod endpoint;
pub mod framing;
pub mod identity;
pub mod process;
pub mod protocol;
pub mod registry;
pub mod release;
pub mod roots;
pub mod routing;
pub mod upstream;
pub mod validation;

pub use edu_academy_core::js::{Js, obj, stringify, text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
}
impl Error {
    pub const fn new(code: &'static str) -> Self {
        Self { code }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub fn string(value: &Js) -> Result<String> {
    if !value.string() {
        return Err(Error::new("tool_input_invalid"));
    }
    String::from_utf16(value.units()).map_err(|_| Error::new("scalar_metadata_required"))
}
pub fn encode(value: &Js) -> Result<Vec<u8>> {
    stringify(value)
        .map(String::into_bytes)
        .map_err(|_| Error::new("mcp_result_serialization_failed"))
}
pub fn parse(bytes: &[u8]) -> Result<Js> {
    edu_academy_core::workflow::codec::parse(bytes)
        .map_err(|_| Error::new("mcp_stdio_input_or_transport_invalid"))
}
pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
