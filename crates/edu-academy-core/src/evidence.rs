//! Pure byte decoder for the existing owner-file reader compatibility boundary.
//! Does not open a real path, retry an effect, or own file custody.
use crate::{js::*, schema};
pub fn convert(v: serde_json::Value) -> Js {
    match v {
        serde_json::Value::Null => Js::Null,
        serde_json::Value::Bool(v) => v.into(),
        serde_json::Value::Number(v) => Js::Num(v.as_f64().expect("JSON number")),
        serde_json::Value::String(v) => v.into(),
        serde_json::Value::Array(a) => Js::Arr(a.into_iter().map(convert).collect()),
        serde_json::Value::Object(p) => {
            Js::Obj(p.into_iter().map(|(k, v)| (text(&k), convert(v))).collect())
        }
    }
}
pub fn parse_bytes(bytes: &str) -> Result<Js> {
    let value = serde_json::from_str::<serde_json::Value>(bytes)
        .map(convert)
        .map_err(|_| Fault {
            name: "SyntaxError".into(),
            issues: None,
            message: if bytes.starts_with('\u{feff}') {
                "JSON Parse error: Unrecognized token '﻿'"
            } else {
                "JSON Parse error: Expected '}'"
            }
            .into(),
            ..Fault::error("")
        })?;
    schema::validate("dailyRecordSaveRequestEvidenceSchema", &value)
}
