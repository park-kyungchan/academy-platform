//! Only this file may be assigned to the future Terra leaf implementer.
use crate::protocol::{LeafError, WeekPeriod};

pub fn journal_wire_width(_units: &[u16]) -> Result<u64, LeafError> {
    Err(LeafError::Unimplemented)
}

pub fn cism_week(_date: &str) -> Result<WeekPeriod, LeafError> {
    Err(LeafError::Unimplemented)
}

pub fn cism_month_dates(_month: &str) -> Result<Vec<String>, LeafError> {
    Err(LeafError::Unimplemented)
}
