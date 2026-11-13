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
/// `begin` returns `Ok(false)` for an existing key before checking the fence.
/// Its existence check, fence check and insertion are one atomic step across processes.
/// The fence and `open_on` match the same group and field and either the same
/// record key or all four occurrence values.
/// `request_ref`, the scope members and `request_lesson_date` are stored but never matched.
/// Each mutation stores exactly the supplied digest and state and computes neither.
/// `record_outcome` receives `ReconciliationRequired` (keeps the attempt open)
/// or `SettledPersisted` (closes it).
/// `record_reconciliation` receives one of the three `Reconciled*` states,
/// requires an outcome and closes the attempt.
/// A mutation of a closed attempt returns `Ok(false)`.
/// A mutation of an unknown key returns `Err(workflow_ledger_unavailable)`.
/// `attempts(group, limit)` returns every attempt of that group when there are
/// at most `limit`, otherwise any `limit` attempts, in no promised order.
/// The ledger is shared between threads and processes, so every implementor is `Send + Sync`.
pub trait AttemptLedger: Send + Sync {
    /// Maximum UTF-8 bytes accepted by `begin`, or no byte limit.
    /// Store checks this pure policy before publishing an execution body.
    fn execution_key_limit(&self) -> Option<usize> {
        None
    }

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
