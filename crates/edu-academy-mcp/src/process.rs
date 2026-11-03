//! Existing rustix supports sending group signals and safe PDEATHSIG/subreaper calls.
//! Parent signal receipt, cancellation, descendant/FD cleanup and native proof are not
//! implemented by those calls alone. Do not spawn until that boundary is resolved.
use crate::{Error, Result};

pub const CANCELLATION_GAP: &str = "safe_process_cancellation_unqualified";

pub fn require_supervised_execution() -> Result<()> {
    Err(Error::new(CANCELLATION_GAP))
}
