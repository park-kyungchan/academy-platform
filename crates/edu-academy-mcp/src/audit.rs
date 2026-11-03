//! Original product CallToolResult wrapping and metadata omission, not an agent ledger.
use crate::{Js, Result, encode, obj, stringify};

pub const METADATA_SOFT_BYTES: usize = 512 * 1024;

pub fn tool_result(data: Js) -> Result<Js> {
    let text =
        stringify(&data).map_err(|_| crate::Error::new("mcp_result_serialization_failed"))?;
    Ok(obj(&[
        (
            "content",
            Js::Arr(vec![obj(&[("type", "text".into()), ("text", text.into())])]),
        ),
        ("structuredContent", data),
    ]))
}
/// Return the original unmodified result if added metadata exceeds the soft cap.
pub fn with_tool_meta(result: &Js, metadata: &Js) -> Result<Js> {
    let mut audited = result.clone();
    let mut merged = if matches!(result.get("_meta"), Js::Obj(_)) {
        result.get("_meta").clone()
    } else {
        obj(&[])
    };
    for (key, value) in metadata.props() {
        let key =
            String::from_utf16(key).map_err(|_| crate::Error::new("scalar_metadata_required"))?;
        merged.set(&key, value.clone());
    }
    audited.set("_meta", merged);
    Ok(if encode(&audited)?.len() <= METADATA_SOFT_BYTES {
        audited
    } else {
        result.clone()
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    NotDispatched,
    Possible,
    Returned,
    ReadbackConfirmed { academy_writes: u32 },
}
impl Effect {
    /// Transport return does not establish persisted state or zero writes.
    pub fn failure_evidence(self) -> Js {
        match self {
            Self::NotDispatched => obj(&[("academyWrites", Js::Num(0.0))]),
            Self::Possible | Self::Returned => obj(&[
                ("academyWrites", Js::Null),
                ("effectStatus", "unknown".into()),
            ]),
            Self::ReadbackConfirmed { academy_writes } => obj(&[
                ("academyWrites", Js::Num(f64::from(academy_writes))),
                ("effectStatus", "readback_confirmed".into()),
            ]),
        }
    }
}
pub fn qualification_gap(code: &'static str, effect: Effect) -> Result<Js> {
    let mut data = obj(&[
        ("status", "blocked".into()),
        ("code", code.into()),
        ("qualification", "incomplete_rust_facade".into()),
        ("automaticRetryAllowed", false.into()),
        ("parentSends", Js::Num(0.0)),
    ]);
    for (key, value) in effect.failure_evidence().props() {
        data.set(&String::from_utf16_lossy(key), value.clone());
    }
    let mut result = tool_result(data)?;
    result.set("isError", true.into());
    Ok(result)
}
