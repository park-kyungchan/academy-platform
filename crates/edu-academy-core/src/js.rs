//! Explicit JS data, including non-scalar UTF-16 and selected own-property descriptors.
//! Object order is insertion order; integer-index enumeration is applied by stringify.
use sha2::{Digest, Sha256};

pub type Text = Vec<u16>;
static UNDEFINED: Js = Js::Undefined;
#[derive(Clone, Debug, PartialEq)]
pub enum Js {
    Null,
    Undefined,
    Hole,
    Bool(bool),
    Num(f64),
    Str(Text),
    Arr(Vec<Js>),
    Obj(Vec<(Text, Js)>),
    BigInt,
    Function,
    Symbol,
    Sparse(usize),
    Decorated(Vec<Js>, Vec<(Text, Js)>),
    Date(Text),
    NullPrototype(Vec<(Text, Js)>),
    SymbolKeys(Vec<(Text, Js)>),
    Accessor(Vec<(Text, Js)>),
    Cycle,
}
#[derive(Clone, Debug)]
pub struct Fault {
    pub name: &'static str,
    pub message: String,
}
impl Fault {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            name: "Error",
            message: message.into(),
        }
    }
    pub fn type_error(message: impl Into<String>) -> Self {
        Self {
            name: "TypeError",
            message: message.into(),
        }
    }
    pub fn admission(message: impl Into<String>) -> Self {
        Self {
            name: "AdapterAdmission",
            message: message.into(),
        }
    }
    pub fn value(&self) -> Js {
        obj(&[
            ("name", self.name.into()),
            ("message", self.message.clone().into()),
        ])
    }
}
pub type Result<T> = std::result::Result<T, Fault>;
pub fn text(s: &str) -> Text {
    s.encode_utf16().collect()
}
pub fn obj(p: &[(&str, Js)]) -> Js {
    Js::Obj(p.iter().map(|(k, v)| (text(k), v.clone())).collect())
}
pub fn strings(values: &[&str]) -> Js {
    Js::Arr(values.iter().map(|s| (*s).into()).collect())
}
pub fn join(parts: &[Text], separator: &str) -> Text {
    let sep = text(separator);
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.extend(&sep);
        }
        out.extend(p);
    }
    out
}
pub fn js_space(u: u16) -> bool {
    matches!(u, 0x9..=0xd|0x20|0xa0|0x1680|0x2000..=0x200a|0x2028|0x2029|0x202f|0x205f|0x3000|0xfeff)
}
pub fn trim(s: &[u16]) -> &[u16] {
    let start = s.iter().position(|u| !js_space(*u)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|u| !js_space(*u))
        .map_or(start, |i| i + 1);
    &s[start..end]
}
pub fn includes(s: &[u16], fragment: &[u16]) -> bool {
    fragment.is_empty() || s.windows(fragment.len()).any(|v| v == fragment)
}
impl From<&str> for Js {
    fn from(v: &str) -> Self {
        Self::Str(text(v))
    }
}
impl From<String> for Js {
    fn from(v: String) -> Self {
        Self::Str(text(&v))
    }
}
impl From<bool> for Js {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<usize> for Js {
    fn from(v: usize) -> Self {
        Self::Num(v as f64)
    }
}
impl Js {
    pub fn get(&self, k: &str) -> &Js {
        self.props()
            .iter()
            .find(|(key, _)| key == &text(k))
            .map_or(&UNDEFINED, |(_, v)| v)
    }
    pub fn props(&self) -> &[(Text, Js)] {
        match self {
            Self::Obj(p) | Self::NullPrototype(p) | Self::SymbolKeys(p) | Self::Accessor(p) => p,
            _ => &[],
        }
    }
    pub fn array(&self) -> &[Js] {
        if let Self::Arr(v) = self { v } else { &[] }
    }
    pub fn units(&self) -> &[u16] {
        if let Self::Str(v) = self { v } else { &[] }
    }
    /// Only for matching ASCII identifiers or regexp predicates. Never renders text.
    pub fn code_units(&self) -> String {
        self.units()
            .iter()
            .map(|u| char::from_u32(u32::from(*u)).unwrap_or('\u{e000}'))
            .collect()
    }
    pub fn is(&self, v: &str) -> bool {
        matches!(self,Self::Str(u) if u==&text(v))
    }
    pub fn absent(&self) -> bool {
        matches!(self, Self::Null | Self::Undefined)
    }
    pub fn has(&self, k: &str) -> bool {
        self.props().iter().any(|(key, _)| key == &text(k))
    }
    pub fn string(&self) -> bool {
        matches!(self, Self::Str(_))
    }
    pub fn set(&mut self, k: &str, v: Js) {
        if let Self::Obj(p) = self {
            if let Some((_, old)) = p.iter_mut().find(|(key, _)| key == &text(k)) {
                *old = v;
            } else {
                p.push((text(k), v));
            }
        }
    }
    pub fn push(&mut self, v: Js) {
        if let Self::Arr(a) = self {
            a.push(v)
        }
    }
    pub fn truthy(&self) -> bool {
        match self {
            Self::Null | Self::Undefined | Self::Hole => false,
            Self::Bool(v) => *v,
            Self::Str(v) => !v.is_empty(),
            Self::Num(v) => *v != 0.0 && !v.is_nan(),
            _ => true,
        }
    }
}
pub fn require_text(value: &Js, label: &str) -> Result<()> {
    if !value.string() || trim(value.units()).is_empty() {
        Err(Fault::error(format!("{label} must be nonempty")))
    } else {
        Ok(())
    }
}
pub fn exact(value: &Js, keys: &[&str]) -> Result<()> {
    if value.absent() {
        return Err(Fault::admission(
            "s08_null_object_exception_not_characterized",
        ));
    }
    if value.props().len() != keys.len() || keys.iter().any(|k| !value.has(k)) {
        Err(Fault::error("unexpected or missing domain fields"))
    } else {
        Ok(())
    }
}
pub fn unique(values: &[Js]) -> Vec<Js> {
    let mut out = Vec::new();
    for v in values {
        if !out.contains(v) {
            out.push(v.clone())
        }
    }
    out
}
pub fn sha(bytes: &str) -> String {
    format!("{:x}", Sha256::digest(bytes.as_bytes()))
}
pub fn quote(units: &[u16]) -> String {
    let mut out = String::from("\"");
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        match u {
            0x22 => out.push_str("\\\""),
            0x5c => out.push_str("\\\\"),
            8 => out.push_str("\\b"),
            12 => out.push_str("\\f"),
            10 => out.push_str("\\n"),
            13 => out.push_str("\\r"),
            9 => out.push_str("\\t"),
            0..=31 => out.push_str(&format!("\\u{u:04x}")),
            0xd800..=0xdbff if i + 1 < units.len() && (0xdc00..=0xdfff).contains(&units[i + 1]) => {
                let scalar =
                    0x10000 + ((u32::from(u) - 0xd800) << 10) + (u32::from(units[i + 1]) - 0xdc00);
                out.push(char::from_u32(scalar).expect("surrogate pair"));
                i += 1;
            }
            0xd800..=0xdfff => out.push_str(&format!("\\u{u:04x}")),
            _ => out.push(char::from_u32(u32::from(u)).expect("non-surrogate")),
        }
        i += 1;
    }
    out.push('"');
    out
}
fn number(n: f64) -> String {
    if n == 0.0 {
        "0".into()
    } else {
        ryu_js::Buffer::new().format(n).to_string()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Learning,
    Admission,
    Cism,
}
fn index(k: &[u16]) -> Option<u32> {
    let s = String::from_utf16(k).ok()?;
    let n = s.parse::<u32>().ok()?;
    if n == u32::MAX || n.to_string() != s {
        None
    } else {
        Some(n)
    }
}
pub fn enumerate(props: &[(Text, Js)]) -> Vec<&(Text, Js)> {
    let mut p: Vec<_> = props.iter().collect();
    p.sort_by(|a, b| match (index(&a.0), index(&b.0)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });
    p
}
/// Three distinct source encoders. Admission has no source recursion bound; the
/// finite adapter must admit depth/cycle separately before invoking that family.
pub fn encode(value: &Js, family: Family, events: &mut Vec<Js>) -> Result<Option<String>> {
    fn run(v: &Js, f: Family, d: usize, e: &mut Vec<Js>) -> Result<Option<String>> {
        if f == Family::Learning && d > 64 {
            return Err(Fault::error("invalid_json_depth"));
        }
        if f != Family::Learning && d > 64 {
            return Err(Fault::admission("s08_recursive_encoder_admission_required"));
        }
        let invalid = || {
            Fault::error(if f == Family::Learning {
                "only finite JSON data is allowed"
            } else {
                "dailyrecord_save_action_not_json"
            })
        };
        let out = match v {
            Js::Null => "null".into(),
            Js::Bool(b) => b.to_string(),
            Js::Str(s) => quote(s),
            Js::Num(n) if n.is_finite() => number(*n),
            Js::Num(_) if f == Family::Cism => "null".into(),
            Js::Undefined | Js::Function | Js::Symbol | Js::Hole if f == Family::Cism => {
                return Ok(None);
            }
            Js::BigInt if f == Family::Cism => {
                return Err(Fault::type_error("JSON.stringify cannot serialize BigInt."));
            }
            Js::Cycle if f != Family::Learning => {
                return Err(Fault::admission("s08_recursive_encoder_admission_required"));
            }
            Js::Cycle => return Err(Fault::error("cyclic JSON is not allowed")),
            Js::Arr(a) if f == Family::Learning && a.iter().any(|c| matches!(c, Js::Hole)) => {
                return Err(Fault::error("sparse or decorated arrays are not allowed"));
            }
            Js::Sparse(_) | Js::Decorated(_, _) if f == Family::Learning => {
                return Err(Fault::error("sparse or decorated arrays are not allowed"));
            }
            Js::SymbolKeys(_) if f == Family::Learning => {
                return Err(Fault::error("symbol fields are not JSON data"));
            }
            Js::Accessor(_) if f == Family::Learning => {
                return Err(Fault::error("accessors are not JSON data"));
            }
            Js::Date(_) if f == Family::Learning => {
                return Err(Fault::error("plain JSON objects required"));
            }
            Js::Date(iso) if f == Family::Cism => quote(iso),
            Js::Date(_) => "{}".into(),
            Js::Sparse(n) => format!(
                "[{}]",
                (0..*n)
                    .map(|_| if f == Family::Cism { "null" } else { "" })
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Js::Arr(a) | Js::Decorated(a, _) => {
                let mut out = Vec::new();
                for child in a {
                    if matches!(child, Js::Hole) && f != Family::Cism {
                        out.push(String::new());
                    } else {
                        out.push(run(child, f, d + 1, e)?.unwrap_or_else(|| "null".into()));
                    }
                }
                format!("[{}]", out.join(","))
            }
            Js::Obj(p) | Js::NullPrototype(p) | Js::SymbolKeys(p) | Js::Accessor(p) => {
                let mut props = enumerate(p);
                if f == Family::Admission {
                    props.retain(|(_, value)| {
                        if matches!(v, Js::Accessor(_)) {
                            e.push("accessor_read".into());
                        }
                        !matches!(value, Js::Undefined)
                    });
                }
                if f != Family::Cism {
                    props.sort_by(|a, b| a.0.cmp(&b.0));
                }
                let mut parts = Vec::new();
                for (key, child) in props {
                    if matches!(v, Js::Accessor(_)) {
                        e.push("accessor_read".into());
                    }
                    if let Some(s) = run(child, f, d + 1, e)? {
                        parts.push(format!("{}:{s}", quote(key)));
                    }
                }
                format!("{{{}}}", parts.join(","))
            }
            _ => return Err(invalid()),
        };
        Ok(Some(out))
    }
    run(value, family, 0, events)
}
pub fn stringify(v: &Js) -> Result<String> {
    encode(v,Family::Cism,&mut vec![])?.ok_or_else(||Fault::type_error("The \"data\" argument must be of type string or an instance of Buffer, TypedArray, or DataView. Received undefined"))
}
pub fn digest(v: &Js, f: Family) -> Result<String> {
    let b=encode(v,f,&mut vec![])?.ok_or_else(||Fault::type_error("The \"data\" argument must be of type string or an instance of Buffer, TypedArray, or DataView. Received undefined"))?;
    Ok(sha(&b))
}
/// Match JS non-u regexes on code units, with the exact ECMAScript whitespace set.
pub fn regex(pattern: &str) -> regex::Regex {
    let p=pattern.replace(r"\s",r"[\t\n\x0b\x0c\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]").replace(r"\d","[0-9]");
    regex::Regex::new(&p).expect("source constant regex")
}
pub fn test(pattern: &str, value: &Js) -> bool {
    regex(pattern).is_match(&value.code_units())
}
