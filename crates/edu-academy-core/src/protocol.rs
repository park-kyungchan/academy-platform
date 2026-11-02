use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Width { units: Vec<u16> },
    Week { value: String },
    Month { value: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekPeriod {
    pub period_start: String,
    pub period_end: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LeafError {
    CismDateRejected,
    CismMonthRejected,
    Unimplemented,
}

impl LeafError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::CismDateRejected => "cism_date_rejected",
            Self::CismMonthRejected => "cism_month_rejected",
            Self::Unimplemented => "s08_leaf_unimplemented",
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Return { value: Value },
    Throw { error: SourceError },
}

#[derive(Serialize)]
pub struct SourceError {
    pub name: &'static str,
    pub message: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Effects {
    pub denied_transport_attempts: u32,
    pub forbidden_fake_dispatches: u32,
}

#[derive(Serialize)]
pub struct Frame {
    pub outcome: Outcome,
    pub events: Vec<Value>,
    pub effects: Effects,
}

pub fn execute(request: Request) -> Result<Frame, LeafError> {
    let result = match request {
        Request::Width { units } => crate::leaf::journal_wire_width(&units).map(Value::from),
        Request::Week { value } => crate::leaf::cism_week(&value)
            .map(|period| serde_json::to_value(period).expect("fixed string fields serialize")),
        Request::Month { value } => crate::leaf::cism_month_dates(&value)
            .map(|dates| serde_json::to_value(dates).expect("string array serializes")),
    };
    let outcome = match result {
        Ok(value) => Outcome::Return { value },
        Err(LeafError::Unimplemented) => return Err(LeafError::Unimplemented),
        Err(error) => Outcome::Throw {
            error: SourceError {
                name: "Error",
                message: error.message(),
            },
        },
    };
    Ok(Frame {
        outcome,
        events: Vec::new(),
        effects: Effects {
            denied_transport_attempts: 0,
            forbidden_fake_dispatches: 0,
        },
    })
}
