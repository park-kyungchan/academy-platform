//! Attempt identity, state and fencing supplied by the ledger owner.
use super::{Runtime, state::FieldState};
use crate::js::Result;
#[derive(Clone, Debug)]
pub struct AttemptTarget {
    pub group: String,
    pub source_namespace: String,
    pub teacher_id: String,
    pub scope_student_id: String,
    pub request_lesson_date: String,
    pub field: String,
    pub record_key: String,
    pub request_ref: String,
    pub occurrence: [String; 4],
}
#[derive(Clone, Debug)]
pub struct Attempt {
    pub key: String,
    pub state: FieldState,
    pub execution: String,
    pub outcome: Option<String>,
    pub reconciliation: Option<String>,
}
/// Each mutation atomically records its digest and state. `true` means newly
/// recorded; duplicate keys/records return `false`. Only open attempts change.
/// Begin fences the same group and field by record key OR occurrence.
pub trait AttemptLedger {
    fn begin(
        &self,
        key: &str,
        target: &AttemptTarget,
        execution_sha256: &str,
        clock: &dyn Runtime,
    ) -> Result<bool>;
    fn record_outcome(
        &self,
        key: &str,
        outcome_sha256: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool>;
    fn record_reconciliation(
        &self,
        key: &str,
        reconciliation_sha256: &str,
        state: FieldState,
        clock: &dyn Runtime,
    ) -> Result<bool>;
    fn attempt(&self, key: &str) -> Result<Option<Attempt>>;
    fn attempts(&self, group: &str, limit: usize) -> Result<Vec<Attempt>>;
    fn open_on(&self, target: &AttemptTarget) -> Result<Vec<Attempt>>;
}
