//! Inert HTML5 tree plus raw-token source locations, always measured in UTF-16.
//! The HTML5 tree owns repair/selection/text; raw bytes own spans, never serialization.
use crate::data::*;
use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}
impl Range {
    pub fn value(&self) -> Js {
        obj(&[("start", self.start.into()), ("end", self.end.into())])
    }
}
#[derive(Clone)]
struct RawElement {
    tag: String,
    attributes: Vec<(String, String)>,
    start: usize,
    open_end: usize,
    end: usize,
    content_end: usize,
}
pub struct Document {
    pub source: String,
    pub tree: Html,
    raw: Vec<RawElement>,
    locations: HashMap<String, usize>,
}
fn selector(s: &str) -> Selector {
    Selector::parse(s).expect("internal selector")
}
fn ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':')
}
fn void(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}
fn id(e: ElementRef<'_>) -> String {
    format!("{:?}", e.id())
}
/// Raw token scanner recognizes quoted attributes, comments and HTML raw-text states.
/// Optional element ends are resolved from following tokens, not a repaired tree dump.
fn scan(source: &str) -> Vec<RawElement> {
    let bytes = source.as_bytes();
    let lower = source.to_ascii_lowercase();
    let mut result: Vec<RawElement> = vec![];
    let mut stack: Vec<usize> = vec![];
    let mut i = 0;
    while i < bytes.len() {
        if let Some(&top) = stack.last() {
            let tag = &result[top].tag;
            if matches!(
                tag.as_str(),
                "script"
                    | "style"
                    | "textarea"
                    | "title"
                    | "xmp"
                    | "iframe"
                    | "noembed"
                    | "noframes"
            ) {
                if let Some(end) = lower[i..].find(&format!("</{tag}")) {
                    i += end
                } else {
                    break;
                }
            }
        }
        if bytes.get(i) != Some(&b'<') {
            i += 1;
            continue;
        }
        if source[i..].starts_with("<!--") {
            i = source[i + 4..]
                .find("-->")
                .map_or(bytes.len(), |p| i + 4 + p + 3);
            continue;
        }
        if matches!(bytes.get(i + 1), Some(b'!' | b'?')) {
            i = source[i..].find('>').map_or(bytes.len(), |p| i + p + 1);
            continue;
        }
        let start = i;
        let closing = bytes.get(i + 1) == Some(&b'/');
        i += if closing { 2 } else { 1 };
        let ns = i;
        while bytes.get(i).is_some_and(|b| ident(*b)) {
            i += 1
        }
        if ns == i {
            continue;
        }
        let name = lower[ns..i].to_string();
        let mut quote = 0;
        let mut end = i;
        while end < bytes.len() {
            let c = bytes[end];
            if quote != 0 {
                if c == quote {
                    quote = 0
                }
            } else if c == b'\'' || c == b'"' {
                quote = c
            } else if c == b'>' {
                break;
            }
            end += 1
        }
        if end == bytes.len() {
            break;
        }
        let token_end = end + 1;
        if closing {
            if let Some(pos) = stack.iter().rposition(|n| result[*n].tag == name) {
                for &n in stack[pos + 1..].iter().rev() {
                    result[n].end = start;
                    result[n].content_end = start
                }
                let n = stack[pos];
                result[n].end = token_end;
                result[n].content_end = start;
                stack.truncate(pos)
            }
            i = token_end;
            continue;
        }
        let implied = |open: &str, next: &str| match open {
            "td" | "th" => matches!(next, "td" | "th" | "tr"),
            "tr" => next == "tr",
            "option" => matches!(next, "option" | "optgroup"),
            "li" => next == "li",
            "p" => matches!(next, "p" | "div" | "table" | "section" | "h1" | "h2" | "h3"),
            _ => false,
        };
        if let Some(pos) = stack.iter().rposition(|n| implied(&result[*n].tag, &name)) {
            for &n in &stack[pos..] {
                result[n].end = start;
                result[n].content_end = start
            }
            stack.truncate(pos)
        }
        let attrs = &source[i..end];
        let fragment = Html::parse_fragment(&format!("<s09-raw {attrs}></s09-raw>"));
        let attributes = fragment
            .select(&selector("s09-raw"))
            .next()
            .map(|e| {
                e.value()
                    .attrs()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect()
            })
            .unwrap_or_default();
        let n = result.len();
        result.push(RawElement {
            tag: name.clone(),
            attributes,
            start,
            open_end: token_end,
            end: token_end,
            content_end: token_end,
        });
        if !void(&name) {
            stack.push(n)
        }
        i = token_end;
    }
    for n in stack {
        result[n].end = source.len();
        result[n].content_end = source.len()
    }
    result
}
impl Document {
    pub fn parse(source: &str) -> Self {
        let tree = Html::parse_document(source);
        let raw = scan(source);
        let mut locations = HashMap::new();
        let mut used = HashSet::new();
        for e in tree.select(&selector("*")) {
            if let Some((n, _)) = raw.iter().enumerate().find(|(n, r)| {
                !used.contains(n)
                    && r.tag == e.value().name()
                    && r.attributes
                        .iter()
                        .all(|(k, v)| e.value().attr(k) == Some(v.as_str()))
                    && e.value()
                        .attrs()
                        .all(|(k, v)| r.attributes.iter().any(|(a, b)| a == k && b == v))
            }) {
                locations.insert(id(e), n);
                used.insert(n);
            }
        }
        Self {
            source: source.into(),
            tree,
            raw,
            locations,
        }
    }
    pub fn select(&self, s: &str) -> Vec<ElementRef<'_>> {
        self.tree.select(&selector(s)).collect()
    }
    pub fn range(&self, e: ElementRef<'_>) -> Option<Range> {
        let n = *self.locations.get(&id(e))?;
        let r = &self.raw[n];
        Some(Range {
            start: self.source[..r.start].encode_utf16().count(),
            end: self.source[..r.end].encode_utf16().count(),
        })
    }
    pub fn script(&self, e: ElementRef<'_>) -> Option<(&str, usize)> {
        let r = &self.raw[*self.locations.get(&id(e))?];
        if r.content_end < r.open_end {
            return None;
        }
        Some((
            &self.source[r.open_end..r.content_end],
            self.source[..r.open_end].encode_utf16().count(),
        ))
    }
    pub fn raw(&self, r: &Range) -> Js {
        let u = text(&self.source);
        Js::Str(u[r.start..r.end].to_vec())
    }
    pub fn span_observation(&self) -> Js {
        Js::Arr(
            self.select("#repair,td,select,option,input,textarea,script#handler")
                .iter()
                .map(|&e| {
                    let r = self.range(e);
                    obj(&[
                        ("tag", e.value().name().to_uppercase().into()),
                        ("id", e.value().attr("id").unwrap_or("").into()),
                        (
                            "value",
                            if matches!(
                                e.value().name(),
                                "input" | "select" | "textarea" | "option"
                            ) {
                                value(e).into()
                            } else {
                                Js::Null
                            },
                        ),
                        ("text", content(e).into()),
                        ("sourceRange", r.as_ref().map_or(Js::Null, Range::value)),
                        ("raw", r.as_ref().map_or(Js::Null, |r| self.raw(r))),
                    ])
                })
                .collect(),
        )
    }
}
pub fn content(e: ElementRef<'_>) -> String {
    e.text().collect::<String>()
}
pub fn select<'a>(e: ElementRef<'a>, s: &str) -> Vec<ElementRef<'a>> {
    e.select(&selector(s)).collect()
}
pub fn closest<'a>(e: ElementRef<'a>, tag: &str) -> Option<ElementRef<'a>> {
    e.ancestors()
        .filter_map(ElementRef::wrap)
        .find(|e| e.value().name() == tag)
}
pub fn value(e: ElementRef<'_>) -> String {
    match e.value().name() {
        "textarea" => content(e),
        "option" => e.value().attr("value").map_or_else(
            || {
                content(e)
                    .split_ascii_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            },
            str::to_string,
        ),
        "select" => {
            let options = select(e, "option");
            let selected = options
                .iter()
                .enumerate()
                .filter(|(_, e)| e.value().attr("selected").is_some())
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let multiple = e.value().attr("multiple").is_some();
            let idx = if multiple {
                selected.first().copied()
            } else {
                selected.last().copied().or_else(|| {
                    options.iter().position(|e| {
                        e.value().attr("disabled").is_none()
                            && closest(*e, "optgroup")
                                .is_none_or(|g| g.value().attr("disabled").is_none())
                    })
                })
            };
            idx.map_or(String::new(), |i| value(options[i]))
        }
        "input" => {
            let v = e.value().attr("value").unwrap_or(
                if matches!(e.value().attr("type"), Some("radio" | "checkbox")) {
                    "on"
                } else {
                    ""
                },
            );
            if matches!(
                e.value().attr("type"),
                Some("text" | "search" | "tel" | "url" | "email" | "password") | None
            ) {
                v.replace(['\n', '\r'], "")
            } else {
                v.into()
            }
        }
        _ => String::new(),
    }
}
pub fn source_function(source: &str, name: &str) -> Result<Js> {
    use regex::Regex;
    if source.encode_utf16().count() > 1_000_000
        || !Regex::new(r"^[A-Za-z_$][A-Za-z0-9_$]{0,64}$")
            .unwrap()
            .is_match(name)
    {
        return Err(Error::error("source_function_scope_rejected"));
    }
    let re = Regex::new(&format!(
        r"function\s+{}\s*\([A-Za-z0-9_$,\s]*\)\s*\{{",
        regex::escape(name)
    ))
    .unwrap();
    let matches = re.find_iter(source).collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(Js::Null);
    }
    if matches.len() != 1 {
        return Err(Error::error("source_function_ambiguous"));
    }
    let m = matches[0];
    let mut i = m.end() - 1;
    let b = source.as_bytes();
    let mut depth = 0;
    let mut quote = 0;
    let mut escaped = false;
    let mut line = false;
    let mut block = false;
    while i < b.len() {
        if String::from_utf8_lossy(&b[m.start()..i])
            .encode_utf16()
            .count()
            > 20000
        {
            return Err(Error::error("source_function_too_large"));
        }
        let c = b[i];
        let next = b.get(i + 1).copied();
        if line {
            if c == b'\n' {
                line = false
            }
            i += 1;
            continue;
        }
        if block {
            if c == b'*' && next == Some(b'/') {
                block = false;
                i += 1
            }
            i += 1;
            continue;
        }
        if quote != 0 {
            if escaped {
                escaped = false
            } else if c == b'\\' {
                escaped = true
            } else if c == quote {
                quote = 0
            }
            i += 1;
            continue;
        }
        if c == b'/' && next == Some(b'/') {
            line = true;
            i += 2;
            continue;
        }
        if c == b'/' && next == Some(b'*') {
            block = true;
            i += 2;
            continue;
        }
        if matches!(c, b'\'' | b'"' | b'`') {
            quote = c;
            i += 1;
            continue;
        }
        if c == b'/' {
            return Err(Error::error("source_function_unqualified_slash_syntax"));
        }
        if c == b'{' {
            depth += 1
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Ok(source[m.start()..i + 1].into());
            }
        }
        i += 1;
    }
    Err(Error::error("source_function_unterminated"))
}
