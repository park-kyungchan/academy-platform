//! JavaScript JSON.parse data semantics, preserving object order and UTF-16 escapes.
//! Parsing containers uses an explicit stack, including old deeply nested records.
use crate::js::{Fault, Js, Result, Text};
fn invalid() -> Fault {
    Fault::error("workflow_json_invalid")
}
struct Reader<'a> {
    bytes: &'a [u8],
    text: &'a str,
    at: usize,
}
enum Frame {
    Array(Vec<Js>, u8),
    Object(Vec<(Text, Js)>, Option<Text>, u8),
}
impl Reader<'_> {
    fn ws(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\r' | b'\n' | b'\t'))
        {
            self.at += 1;
        }
    }
    fn consume(&mut self, b: u8) -> bool {
        if self.bytes.get(self.at) == Some(&b) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Result<Text> {
        if !self.consume(b'"') {
            return Err(invalid());
        }
        let mut out = vec![];
        loop {
            let b = *self.bytes.get(self.at).ok_or_else(invalid)?;
            self.at += 1;
            match b {
                b'"' => return Ok(out),
                0..=31 => return Err(invalid()),
                b'\\' => {
                    let b = *self.bytes.get(self.at).ok_or_else(invalid)?;
                    self.at += 1;
                    out.push(match b {
                        b'"' => 34,
                        b'\\' => 92,
                        b'/' => 47,
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'u' => {
                            let mut n = 0;
                            for _ in 0..4 {
                                let b = *self.bytes.get(self.at).ok_or_else(invalid)?;
                                self.at += 1;
                                let d = match b {
                                    b'0'..=b'9' => b - b'0',
                                    b'a'..=b'f' => b - b'a' + 10,
                                    b'A'..=b'F' => b - b'A' + 10,
                                    _ => return Err(invalid()),
                                };
                                n = n * 16 + u16::from(d);
                            }
                            n
                        }
                        _ => return Err(invalid()),
                    });
                }
                32..=127 => out.push(u16::from(b)),
                _ => {
                    self.at -= 1;
                    // parse validated the complete UTF-8 input once. at is
                    // always a character boundary; decoding advances by len_utf8.
                    let tail = &self.text[self.at..];
                    let ch = tail.chars().next().ok_or_else(invalid)?;
                    let mut buf = [0; 2];
                    out.extend_from_slice(ch.encode_utf16(&mut buf));
                    self.at += ch.len_utf8();
                }
            }
        }
    }
    fn value(&mut self, stack: &mut Vec<Frame>) -> Result<Option<Js>> {
        self.ws();
        let b = *self.bytes.get(self.at).ok_or_else(invalid)?;
        match b {
            b'[' => {
                self.at += 1;
                stack.push(Frame::Array(vec![], 0));
                Ok(None)
            }
            b'{' => {
                self.at += 1;
                stack.push(Frame::Object(vec![], None, 0));
                Ok(None)
            }
            b'"' => Ok(Some(Js::Str(self.string()?))),
            b'n' | b't' | b'f' => {
                let (s, v) = match b {
                    b'n' => ("null", Js::Null),
                    b't' => ("true", Js::Bool(true)),
                    _ => ("false", Js::Bool(false)),
                };
                if !self.bytes[self.at..].starts_with(s.as_bytes()) {
                    return Err(invalid());
                }
                self.at += s.len();
                Ok(Some(v))
            }
            b'-' | b'0'..=b'9' => {
                let start = self.at;
                self.consume(b'-');
                if !self.consume(b'0') {
                    let b = *self.bytes.get(self.at).ok_or_else(invalid)?;
                    if !matches!(b, b'1'..=b'9') {
                        return Err(invalid());
                    }
                    self.at += 1;
                    while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                        self.at += 1;
                    }
                }
                if self.consume(b'.') {
                    let start = self.at;
                    while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                        self.at += 1;
                    }
                    if start == self.at {
                        return Err(invalid());
                    }
                }
                if self.consume(b'e') || self.consume(b'E') {
                    if !self.consume(b'+') {
                        self.consume(b'-');
                    }
                    let start = self.at;
                    while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                        self.at += 1;
                    }
                    if start == self.at {
                        return Err(invalid());
                    }
                }
                let s = &self.text[start..self.at];
                Ok(Some(Js::Num(s.parse().map_err(|_| invalid())?)))
            }
            _ => Err(invalid()),
        }
    }
}
pub fn parse(bytes: &[u8]) -> Result<Js> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let mut r = Reader { bytes, text, at: 0 };
    let mut stack = vec![];
    let mut pending = r.value(&mut stack)?;
    loop {
        if let Some(value) = pending.take() {
            match stack.last_mut() {
                Some(Frame::Array(a, state)) if *state == 0 || *state == 1 => {
                    a.push(value);
                    *state = 2;
                }
                Some(Frame::Object(a, key, state)) if *state == 3 => {
                    let k = key.take().ok_or_else(invalid)?;
                    if let Some((_, old)) = a.iter_mut().find(|(old, _)| old == &k) {
                        *old = value;
                    } else {
                        a.push((k, value));
                    }
                    *state = 4;
                }
                None => {
                    r.ws();
                    return if r.at == bytes.len() {
                        Ok(value)
                    } else {
                        Err(invalid())
                    };
                }
                _ => return Err(invalid()),
            }
        }
        r.ws();
        match stack.last_mut() {
            Some(Frame::Array(_, state)) if *state == 0 => {
                if r.consume(b']') {
                    let Some(Frame::Array(a, _)) = stack.pop() else {
                        return Err(invalid());
                    };
                    pending = Some(Js::Arr(a));
                } else {
                    pending = r.value(&mut stack)?;
                }
            }
            Some(Frame::Array(_, state)) if *state == 1 => {
                pending = r.value(&mut stack)?;
            }
            Some(Frame::Array(_, state)) => {
                if r.consume(b',') {
                    *state = 1;
                } else if r.consume(b']') {
                    let Some(Frame::Array(a, _)) = stack.pop() else {
                        return Err(invalid());
                    };
                    pending = Some(Js::Arr(a));
                } else {
                    return Err(invalid());
                }
            }
            Some(Frame::Object(_, key, state)) if *state == 0 || *state == 1 => {
                if *state == 0 && r.consume(b'}') {
                    let Some(Frame::Object(a, _, _)) = stack.pop() else {
                        return Err(invalid());
                    };
                    pending = Some(Js::Obj(a));
                } else {
                    *key = Some(r.string()?);
                    *state = 2;
                }
            }
            Some(Frame::Object(_, _, state)) if *state == 2 => {
                if !r.consume(b':') {
                    return Err(invalid());
                }
                *state = 3;
            }
            Some(Frame::Object(_, _, state)) if *state == 3 => {
                pending = r.value(&mut stack)?;
            }
            Some(Frame::Object(_, _, state)) => {
                if r.consume(b',') {
                    *state = 1;
                } else if r.consume(b'}') {
                    let Some(Frame::Object(a, _, _)) = stack.pop() else {
                        return Err(invalid());
                    };
                    pending = Some(Js::Obj(a));
                } else {
                    return Err(invalid());
                }
            }
            None => return Err(invalid()),
        }
    }
}
pub fn bytes(v: &Js) -> Result<Vec<u8>> {
    Ok((crate::js::stringify(v)? + "\n").into_bytes())
}
