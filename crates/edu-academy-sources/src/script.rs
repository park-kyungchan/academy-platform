//! Static ECMAScript syntax for source observations, with no evaluator or host objects.
//! Unsupported syntax refuses parsing and is reported by the presentation observer.
use crate::data::*;
#[derive(Clone, Debug)]
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub literal: Option<Js>,
}
#[derive(Clone, Debug)]
pub struct Node {
    pub kind: &'static str,
    pub start: usize,
    pub end: usize,
    pub name: String,
    pub literal: Option<Js>,
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
        match self.kind {
            "Identifier" => Some(self.name.clone()),
            "MemberExpression" => {
                let a = self.children.first()?.reference()?;
                let b = self.children.get(1)?;
                let key = b
                    .reference()
                    .or_else(|| b.literal.as_ref().filter(|v| v.string()).map(string))?;
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
    pub fn scalar(&self) -> Option<Js> {
        if self.kind == "Literal" {
            self.literal.clone()
        } else {
            None
        }
    }
}
fn syntax() -> Error {
    Error::new("SyntaxError", "static_script_syntax_unqualified")
}
fn js_string(raw: &str) -> Result<Js> {
    let mut out = vec![];
    let mut chars = raw[1..raw.len() - 1].chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.extend(c.to_string().encode_utf16());
            continue;
        }
        let c = chars.next().ok_or_else(syntax)?;
        match c {
            'n' => out.push(10),
            'r' => out.push(13),
            't' => out.push(9),
            'b' => out.push(8),
            'f' => out.push(12),
            'v' => out.push(11),
            '0' => out.push(0),
            '\n' => {}
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            'u' | 'x' => {
                let count = if c == 'u' { 4 } else { 2 };
                let s = (0..count)
                    .map(|_| chars.next().ok_or_else(syntax))
                    .collect::<Result<String>>()?;
                out.push(u16::from_str_radix(&s, 16).map_err(|_| syntax())?);
            }
            _ => out.extend(c.to_string().encode_utf16()),
        }
    }
    Ok(Js::Str(out))
}
pub fn lex(source: &str) -> Result<Vec<Token>> {
    let b = source.as_bytes();
    let mut i = 0;
    let mut t = vec![];
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b.get(i..i + 2) == Some(b"//") {
            i = source[i..].find('\n').map_or(b.len(), |p| i + p);
            continue;
        }
        if b.get(i..i + 2) == Some(b"/*") {
            i = source[i + 2..]
                .find("*/")
                .map(|p| i + 2 + p + 2)
                .ok_or_else(syntax)?;
            continue;
        }
        let start = i;
        let mut literal = None;
        if matches!(c, b'\'' | b'"') {
            i += 1;
            let mut escaped = false;
            loop {
                let x = *b.get(i).ok_or_else(syntax)?;
                i += 1;
                if escaped {
                    escaped = false
                } else if x == b'\\' {
                    escaped = true
                } else if x == c {
                    break;
                } else if matches!(x, b'\r' | b'\n') {
                    return Err(syntax());
                }
            }
            literal = Some(js_string(&source[start..i])?);
        } else if c.is_ascii_alphabetic() || matches!(c, b'_' | b'$') {
            i += 1;
            while b
                .get(i)
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$'))
            {
                i += 1
            }
            literal = match &source[start..i] {
                "null" => Some(Js::Null),
                "true" => Some(true.into()),
                "false" => Some(false.into()),
                _ => None,
            };
        } else if c.is_ascii_digit() {
            i += 1;
            while b
                .get(i)
                .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E'))
            {
                i += 1
            }
            if matches!(b.get(i - 1), Some(b'e' | b'E')) && matches!(b.get(i), Some(b'+' | b'-')) {
                i += 1;
                while b.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1
                }
            }
            literal = Some(Js::Num(source[start..i].parse().map_err(|_| syntax())?));
        } else {
            let op = [
                "===", "!==", ">>>", "**=", "==", "!=", "<=", ">=", "&&", "||", "??", "=>", "++",
                "--", "+=", "-=", "*=", "/=", "**", "<<", ">>", "?.",
            ]
            .iter()
            .find(|s| source[i..].starts_with(**s));
            if let Some(op) = op {
                i += op.len()
            } else if b"{}[]().,:;?+-*/%<>=!&|^~".contains(&c) {
                i += 1
            } else {
                return Err(syntax());
            }
        }
        t.push(Token {
            text: source[start..i].into(),
            start,
            end: i,
            literal,
        });
        if t.len() > 100000 {
            return Err(Error::error("test_pool_presentation_ast_limit"));
        }
    }
    Ok(t)
}
pub struct Parser {
    pub tokens: Vec<Token>,
    pub i: usize,
    depth: usize,
}
impl Parser {
    pub fn new(s: &str) -> Result<Self> {
        Ok(Self {
            tokens: lex(s)?,
            i: 0,
            depth: 0,
        })
    }
    fn is(&self, s: &str) -> bool {
        self.tokens.get(self.i).is_some_and(|t| t.text == s)
    }
    fn take(&mut self, s: &str) -> bool {
        if self.is(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn next(&mut self) -> Result<Token> {
        let t = self.tokens.get(self.i).cloned().ok_or_else(syntax)?;
        self.i += 1;
        Ok(t)
    }
    fn expect(&mut self, s: &str) -> Result<Token> {
        if !self.is(s) {
            return Err(syntax());
        }
        self.next()
    }
    fn last_end(&self) -> usize {
        self.tokens
            .get(self.i.saturating_sub(1))
            .map_or(0, |t| t.end)
    }
    fn identifier(&mut self) -> Result<Node> {
        let t = self.next()?;
        if t.literal.is_some()
            || !t
                .text
                .bytes()
                .next()
                .is_some_and(|b| b.is_ascii_alphabetic() || matches!(b, b'_' | b'$'))
        {
            return Err(syntax());
        }
        let mut n = Node::new("Identifier", t.start, t.end, vec![]);
        n.name = t.text;
        Ok(n)
    }
    pub fn program(&mut self) -> Result<Node> {
        let mut nodes = vec![];
        while self.i < self.tokens.len() {
            nodes.push(self.statement()?)
        }
        Ok(Node::new("Program", 0, self.last_end(), nodes))
    }
    fn statement(&mut self) -> Result<Node> {
        self.depth += 1;
        if self.depth > 256 {
            return Err(syntax());
        }
        let start = self.tokens.get(self.i).map_or(0, |t| t.start);
        let n = if self.take(";") {
            Node::new("EmptyStatement", start, self.last_end(), vec![])
        } else if self.take("{") {
            let mut nodes = vec![];
            while !self.is("}") {
                nodes.push(self.statement()?)
            }
            self.expect("}")?;
            Node::new("BlockStatement", start, self.last_end(), nodes)
        } else if self.is("function") {
            self.function(true)?
        } else if self.is("var") || self.is("let") || self.is("const") {
            self.i += 1;
            let mut nodes = vec![];
            loop {
                let id = self.identifier()?;
                let begin = id.start;
                let mut children = vec![id];
                if self.take("=") {
                    children.push(self.expr(1)?)
                }
                nodes.push(Node::new(
                    "VariableDeclarator",
                    begin,
                    self.last_end(),
                    children,
                ));
                if !self.take(",") {
                    break;
                }
            }
            self.take(";");
            Node::new("VariableDeclaration", start, self.last_end(), nodes)
        } else if self.take("return") || self.take("throw") {
            let mut nodes = vec![];
            if !self.is(";") && !self.is("}") {
                nodes.push(self.expr(0)?)
            }
            self.take(";");
            Node::new("ReturnStatement", start, self.last_end(), nodes)
        } else if self.take("if") {
            self.expect("(")?;
            let test = self.expr(0)?;
            self.expect(")")?;
            let yes = self.statement()?;
            let mut nodes = vec![test, yes];
            if self.take("else") {
                nodes.push(self.statement()?)
            }
            Node::new("IfStatement", start, self.last_end(), nodes)
        } else {
            let e = self.expr(0)?;
            self.take(";");
            Node::new("ExpressionStatement", start, self.last_end(), vec![e])
        };
        self.depth -= 1;
        Ok(n)
    }
    fn function(&mut self, declaration: bool) -> Result<Node> {
        let start = self.expect("function")?.start;
        let mut children = vec![];
        if !self.is("(") {
            children.push(self.identifier()?)
        } else if declaration {
            return Err(syntax());
        }
        self.expect("(")?;
        while !self.is(")") {
            children.push(self.identifier()?);
            if !self.take(",") {
                break;
            }
        }
        self.expect(")")?;
        if !self.is("{") {
            return Err(syntax());
        }
        children.push(self.statement()?);
        Ok(Node::new(
            if declaration {
                "FunctionDeclaration"
            } else {
                "FunctionExpression"
            },
            start,
            self.last_end(),
            children,
        ))
    }
    pub fn expr(&mut self, min: u8) -> Result<Node> {
        self.depth += 1;
        if self.depth > 256 {
            return Err(syntax());
        }
        let start = self.tokens.get(self.i).ok_or_else(syntax)?.start;
        let mut left = if self.is("function") {
            self.function(false)?
        } else if self.take("(") {
            let n = self.expr(0)?;
            self.expect(")")?;
            n
        } else if self.take("[") {
            let mut items = vec![];
            while !self.is("]") {
                if self.take(",") {
                    items.push(Node::new("Hole", self.last_end(), self.last_end(), vec![]));
                    continue;
                }
                items.push(self.expr(1)?);
                if !self.take(",") {
                    break;
                }
            }
            self.expect("]")?;
            Node::new("ArrayExpression", start, self.last_end(), items)
        } else if self.take("{") {
            let mut properties = vec![];
            while !self.is("}") {
                let k = if self.tokens.get(self.i).is_some_and(|t| t.literal.is_some()) {
                    let t = self.next()?;
                    let mut n = Node::new("Literal", t.start, t.end, vec![]);
                    n.literal = t.literal;
                    n
                } else {
                    self.identifier()?
                };
                let name = k
                    .reference()
                    .or_else(|| k.scalar().filter(Js::string).map(|v| string(&v)))
                    .ok_or_else(syntax)?;
                let begin = k.start;
                let value = if self.take(":") {
                    self.expr(1)?
                } else {
                    k.clone()
                };
                let mut property = Node::new("Property", begin, value.end, vec![k, value]);
                property.name = name;
                properties.push(property);
                if !self.take(",") {
                    break;
                }
            }
            self.expect("}")?;
            Node::new("ObjectExpression", start, self.last_end(), properties)
        } else if ["!", "~", "-", "+", "typeof", "void", "delete"]
            .iter()
            .any(|op| self.is(op))
        {
            self.i += 1;
            let child = self.expr(14)?;
            Node::new("UnaryExpression", start, child.end, vec![child])
        } else {
            let t = self.tokens.get(self.i).ok_or_else(syntax)?;
            if t.literal.is_some() {
                let t = self.next()?;
                let mut n = Node::new("Literal", t.start, t.end, vec![]);
                n.literal = t.literal;
                n
            } else {
                self.identifier()?
            }
        };
        loop {
            if self.take(".") {
                let p = self.identifier()?;
                left = Node::new("MemberExpression", left.start, p.end, vec![left, p]);
                continue;
            }
            if self.take("[") {
                let p = self.expr(0)?;
                self.expect("]")?;
                left = Node::new(
                    "MemberExpression",
                    left.start,
                    self.last_end(),
                    vec![left, p],
                );
                continue;
            }
            if self.take("(") {
                let mut children = vec![left];
                while !self.is(")") {
                    children.push(self.expr(1)?);
                    if !self.take(",") {
                        break;
                    }
                }
                self.expect(")")?;
                left = Node::new(
                    "CallExpression",
                    children[0].start,
                    self.last_end(),
                    children,
                );
                continue;
            }
            if self.is("?") && min <= 2 {
                self.i += 1;
                let yes = self.expr(1)?;
                self.expect(":")?;
                let no = self.expr(1)?;
                left = Node::new(
                    "ConditionalExpression",
                    left.start,
                    no.end,
                    vec![left, yes, no],
                );
                continue;
            }
            let op = self
                .tokens
                .get(self.i)
                .map(|t| t.text.as_str())
                .unwrap_or("");
            let binding = match op {
                "," => 0,
                "=" | "+=" | "-=" => 1,
                "||" | "??" => 3,
                "&&" => 4,
                "|" => 5,
                "^" => 6,
                "&" => 7,
                "==" | "!=" | "===" | "!==" => 8,
                "<" | ">" | "<=" | ">=" | "in" | "instanceof" => 9,
                "<<" | ">>" | ">>>" => 10,
                "+" | "-" => 11,
                "*" | "/" | "%" => 12,
                "**" => 13,
                _ => break,
            };
            if binding < min {
                break;
            }
            let assign = binding == 1;
            self.i += 1;
            let right = self.expr(if assign { binding } else { binding + 1 })?;
            left = Node::new(
                if assign {
                    "AssignmentExpression"
                } else {
                    "BinaryExpression"
                },
                left.start,
                right.end,
                vec![left, right],
            );
        }
        self.depth -= 1;
        Ok(left)
    }
}
pub fn walk(root: &Node) -> Vec<&Node> {
    let mut pending = vec![root];
    let mut out = vec![];
    while let Some(n) = pending.pop() {
        out.push(n);
        for c in &n.children {
            pending.push(c)
        }
    }
    out
}
