//! Inert SWC ECMAScript AST projection for the original presentation observer.
use crate::data::*;
#[path = "script_swc.rs"]
mod script_swc;
pub use script_swc::Parser;
#[derive(Clone, Debug)]
pub struct Node {
    pub kind: &'static str,
    pub start: usize,
    pub end: usize,
    pub name: String,
    pub literal: Option<Js>,
    pub computed: bool,
    pub children: Vec<Node>,
}
impl Node {
    fn new(kind: &'static str, start: usize, end: usize, children: Vec<Node>) -> Self {
        Self {
            kind,
            start,
            end,
            name: String::new(),
            literal: None,
            computed: false,
            children,
        }
    }
    pub fn field(&self, key: &str) -> Option<&Node> {
        self.children
            .iter()
            .find(|p| p.kind == "Property" && p.name == key)
            .and_then(|p| p.children.last())
    }
    pub fn reference(&self) -> Option<String> {
        self.reference_at(0)
    }
    fn reference_at(&self, depth: usize) -> Option<String> {
        if depth > 16 {
            return None;
        }
        match self.kind {
            "Identifier" => Some(self.name.clone()),
            "MemberExpression" => {
                let a = self.children.first()?.reference_at(depth + 1)?;
                let b = self.children.get(1)?;
                let key = if self.computed {
                    b.scalar().filter(Js::string).map(|v| string(&v))
                } else {
                    b.reference_at(depth + 1)
                }?;
                if key.bytes().enumerate().all(|(i, c)| {
                    c.is_ascii_alphabetic()
                        || matches!(c, b'_' | b'$')
                        || (i > 0 && c.is_ascii_digit())
                }) {
                    Some(format!("{a}.{key}"))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    pub fn checked_reference(&self) -> Result<Option<String>> {
        self.checked_reference_at(0)
    }
    fn checked_reference_at(&self, depth: usize) -> Result<Option<String>> {
        if depth > 16 {
            return Ok(None);
        }
        let bounded = |s: String, max: usize| -> Result<String> {
            let units = text(&s);
            if units.len() > max || units.iter().any(|c| matches!(c,0..=8|11|12|14..=31)) {
                return Err(Error::error("test_pool_presentation_text_limit"));
            }
            Ok(s)
        };
        match self.kind {
            "Identifier" => Ok(Some(bounded(self.name.clone(), 256)?)),
            "MemberExpression" => {
                let Some(a) = self.children.first() else {
                    return Ok(None);
                };
                let Some(a) = a.checked_reference_at(depth + 1)? else {
                    return Ok(None);
                };
                let Some(b) = self.children.get(1) else {
                    return Ok(None);
                };
                let key = if self.computed {
                    b.checked_scalar()?.filter(Js::string).map(|v| string(&v))
                } else {
                    b.checked_reference_at(depth + 1)?
                };
                let Some(key) = key else {
                    return Ok(None);
                };
                if key.is_empty()
                    || !key.bytes().enumerate().all(|(i, c)| {
                        c.is_ascii_alphabetic()
                            || matches!(c, b'_' | b'$')
                            || (i > 0 && c.is_ascii_digit())
                    })
                {
                    return Ok(None);
                }
                Ok(Some(bounded(format!("{a}.{key}"), 512)?))
            }
            _ => Ok(None),
        }
    }
    pub fn checked_scalar(&self) -> Result<Option<Js>> {
        let value = self.scalar();
        if let Some(Js::Str(units)) = &value
            && (units.len() > 4096 || units.iter().any(|c| matches!(c,0..=8|11|12|14..=31)))
        {
            return Err(Error::error("test_pool_presentation_text_limit"));
        }
        Ok(value)
    }
    pub fn scalar(&self) -> Option<Js> {
        if self.kind == "Literal" {
            self.literal.clone()
        } else {
            None
        }
    }
}
pub fn walk(root: &Node) -> Result<Vec<&Node>> {
    let mut pending = vec![root];
    let mut out = vec![];
    while let Some(n) = pending.pop() {
        if n.kind == "Hole" {
            continue;
        }
        if out.len() >= 100_000 {
            return Err(Error::error("test_pool_presentation_ast_limit"));
        }
        out.push(n);
        for c in &n.children {
            pending.push(c);
        }
    }
    Ok(out)
}
