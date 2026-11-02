//! JS JSON data boundary: ordered properties and exact UTF-16, including lone surrogates.
pub use edu_academy_core::js::{Js, Text, obj, stringify, strings, text, trim};
use edu_academy_core::schema::pretty;
use sha2::{Digest, Sha256};
#[derive(Debug, Clone)]
pub struct Error {
    pub name: String,
    pub message: String,
    pub properties: Js,
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub fn new(name: &str, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            properties: obj(&[]),
        }
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self::new("Error", message)
    }
    pub fn refusal(code: &str) -> Self {
        let mut e = Self::error(code);
        e.properties.set("code", code.into());
        e
    }
    pub fn value(&self) -> Js {
        let mut v = obj(&[
            ("name", self.name.clone().into()),
            ("message", self.message.clone().into()),
        ]);
        for (k, x) in self.properties.props() {
            if let Js::Obj(p) = &mut v {
                p.push((k.clone(), x.clone()));
            }
        }
        v
    }
    pub fn zod(issues: Vec<Js>) -> Self {
        let a = Js::Arr(issues);
        Self {
            name: "ZodError".into(),
            message: pretty(&a, 0),
            properties: obj(&[("issues", a)]),
        }
    }
}
pub fn observe(result: Result<Js>) -> Js {
    match result {
        Ok(v) => obj(&[("outcome", "returned".into()), ("value", v)]),
        Err(e) => obj(&[("outcome", "threw".into()), ("error", e.value())]),
    }
}
pub fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
pub fn json(v: &Js) -> String {
    stringify(v).expect("JSON data")
}
pub fn string(v: &Js) -> String {
    String::from_utf16_lossy(v.units())
}
pub fn number(v: &Js) -> Option<f64> {
    if let Js::Num(n) = v { Some(*n) } else { None }
}
pub fn array(v: &Js) -> bool {
    matches!(v, Js::Arr(_))
}
pub fn object(v: &Js) -> bool {
    matches!(v, Js::Obj(_))
}
pub fn sha_valid(v: &Js) -> bool {
    v.units().len() == 64 && v.units().iter().all(|u| matches!(*u,48..=57|97..=102))
}
pub fn base64(bytes: &[u8], url: bool) -> String {
    let alphabet = if url {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i < c.len() + 1 {
                s.push(alphabet[((n >> (18 - i * 6)) & 63) as usize] as char)
            } else if !url {
                s.push('=')
            }
        }
    }
    s
}
pub fn unbase64(s: &str) -> Result<Vec<u8>> {
    let mut out = vec![];
    let mut bits = 0u32;
    let mut len = 0;
    for c in s.bytes() {
        let n = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return Err(Error::error("base64_invalid")),
        };
        bits = (bits << 6) | u32::from(n);
        len += 6;
        if len >= 8 {
            len -= 8;
            out.push((bits >> len) as u8);
            bits &= (1 << len) - 1;
        }
    }
    Ok(out)
}
/// Parse JSON text without serde scalar conversion. The input decoder is selected by its owner.
pub fn parse(s: &str) -> Result<Js> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self
                .s
                .get(self.i)
                .is_some_and(|c| matches!(c, b' ' | b'\r' | b'\n' | b'\t'))
            {
                self.i += 1
            }
        }
        fn err(&self) -> Error {
            Error::new(
                "SyntaxError",
                if self.s == b"{" {
                    "JSON Parse error: Expected '}'"
                } else {
                    "JSON Parse error: Unexpected identifier \"undefined\""
                },
            )
        }
        fn string(&mut self) -> Result<Text> {
            self.i += 1;
            let mut out = vec![];
            let mut begin = self.i;
            loop {
                let c = *self.s.get(self.i).ok_or_else(|| self.err())?;
                if c == b'"' || c == b'\\' {
                    let raw =
                        std::str::from_utf8(&self.s[begin..self.i]).map_err(|_| self.err())?;
                    out.extend(raw.encode_utf16());
                    self.i += 1;
                    if c == b'"' {
                        return Ok(out);
                    }
                    let e = *self.s.get(self.i).ok_or_else(|| self.err())?;
                    self.i += 1;
                    out.push(match e {
                        b'"' => 34,
                        b'\\' => 92,
                        b'/' => 47,
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'u' => {
                            let raw = self.s.get(self.i..self.i + 4).ok_or_else(|| self.err())?;
                            let n = u16::from_str_radix(
                                std::str::from_utf8(raw).map_err(|_| self.err())?,
                                16,
                            )
                            .map_err(|_| self.err())?;
                            self.i += 4;
                            n
                        }
                        _ => return Err(self.err()),
                    });
                    begin = self.i;
                } else if c < 32 {
                    return Err(self.err());
                } else {
                    self.i += 1
                }
            }
        }
        fn value(&mut self, depth: usize) -> Result<Js> {
            if depth > 4096 {
                return Err(self.err());
            }
            self.ws();
            match self.s.get(self.i).copied() {
                Some(b'"') => Ok(Js::Str(self.string()?)),
                Some(b'{') => {
                    self.i += 1;
                    self.ws();
                    let mut p: Vec<(Text, Js)> = vec![];
                    if self.s.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Js::Obj(p));
                    }
                    loop {
                        self.ws();
                        if self.s.get(self.i) != Some(&b'"') {
                            return Err(self.err());
                        }
                        let k = self.string()?;
                        self.ws();
                        if self.s.get(self.i) != Some(&b':') {
                            return Err(self.err());
                        }
                        self.i += 1;
                        let v = self.value(depth + 1)?;
                        if let Some((_, old)) = p.iter_mut().find(|(key, _)| key == &k) {
                            *old = v
                        } else {
                            p.push((k, v))
                        }
                        self.ws();
                        match self.s.get(self.i) {
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(Js::Obj(p));
                            }
                            Some(b',') => self.i += 1,
                            _ => return Err(self.err()),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    self.ws();
                    let mut a = vec![];
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Js::Arr(a));
                    }
                    loop {
                        a.push(self.value(depth + 1)?);
                        self.ws();
                        match self.s.get(self.i) {
                            Some(b']') => {
                                self.i += 1;
                                return Ok(Js::Arr(a));
                            }
                            Some(b',') => self.i += 1,
                            _ => return Err(self.err()),
                        }
                    }
                }
                Some(b'n') if self.s.get(self.i..self.i + 4) == Some(b"null") => {
                    self.i += 4;
                    Ok(Js::Null)
                }
                Some(b't') if self.s.get(self.i..self.i + 4) == Some(b"true") => {
                    self.i += 4;
                    Ok(Js::Bool(true))
                }
                Some(b'f') if self.s.get(self.i..self.i + 5) == Some(b"false") => {
                    self.i += 5;
                    Ok(Js::Bool(false))
                }
                Some(b'-' | b'0'..=b'9') => {
                    let start = self.i;
                    while self.s.get(self.i).is_some_and(|c| {
                        matches!(c, b'0'..=b'9' | b'-' | b'+' | b'e' | b'E' | b'.')
                    }) {
                        self.i += 1
                    }
                    let s = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err())?;
                    static NUMBER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
                    let grammar = NUMBER.get_or_init(|| {
                        regex::Regex::new(r"^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$")
                            .unwrap()
                    });
                    if !grammar.is_match(s) {
                        return Err(self.err());
                    }
                    Ok(Js::Num(s.parse::<f64>().map_err(|_| self.err())?))
                }
                _ => Err(self.err()),
            }
        }
    }
    let mut p = P {
        s: s.as_bytes(),
        i: 0,
    };
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err());
    }
    Ok(v)
}
pub fn valid_date(v: &Js) -> bool {
    let s = string(v);
    let a = s.split('-').collect::<Vec<_>>();
    if a.len() != 3
        || a[0].len() != 4
        || a[1].len() != 2
        || a[2].len() != 2
        || !a.iter().all(|s| s.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    let (y, m, d) = (
        a[0].parse::<u32>().unwrap_or(99999),
        a[1].parse::<u32>().unwrap_or(0),
        a[2].parse::<u32>().unwrap_or(0),
    );
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    d > 0 && d <= days
}
