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

/// Fresh product occurrence, not an authentication credential or agent ledger.
pub fn occurrence() -> Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| crate::Error::new("occurrence_entropy_unavailable"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(bytes
        .iter()
        .enumerate()
        .map(|(i, b)| {
            format!(
                "{}{b:02x}",
                if [4, 6, 8, 10].contains(&i) { "-" } else { "" }
            )
        })
        .collect())
}
pub fn wall_time() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    // UTC civil date from Unix days, independent of locale/TZ or an external tool.
    let z = (millis / 86_400_000) as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let second = millis / 1000;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        second / 3600 % 24,
        second / 60 % 60,
        second % 60,
        millis % 1000
    )
}
pub fn observed_attribution(args: &Js, client: Option<&Js>, bridge_sha256: &str) -> Js {
    struct Clock;
    impl edu_academy_core::workflow::Runtime for Clock {
        fn now(&self) -> String {
            wall_time()
        }
        fn epoch_ms(&self) -> u64 {
            0
        }
        fn uuid(&self) -> edu_academy_core::js::Result<String> {
            occurrence().map_err(|e| edu_academy_core::js::Fault::error(e.code))
        }
    }
    let mut value = edu_academy_core::workflow::observe_attribution(&Clock, args.get("actor"));
    value.set("observedBy", "neutral-bridge".into());
    value.set(
        "client",
        client.map_or(Js::Null, |c| {
            crate::obj(&[
                ("name", c.get("name").clone()),
                ("version", c.get("version").clone()),
            ])
        }),
    );
    let mut process = value.get("process").clone();
    process.set("bridgeSha256", bridge_sha256.into());
    value.set("process", process);
    value
}
