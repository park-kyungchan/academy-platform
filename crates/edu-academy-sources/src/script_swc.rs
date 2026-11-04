//! Full native parser; this projection has no evaluator, host objects or source lookup.
use super::Node;
use crate::data::*;
use swc_common::{BytePos, Span, Spanned};
use swc_ecma_ast::*;
use swc_ecma_parser::{EsSyntax, Parser as SwcParser, StringInput, Syntax, lexer::Lexer};
use swc_ecma_regexp::{LiteralParser, Options as RegexpOptions};
use swc_ecma_regexp_visit::{Visit as RegexpVisit, VisitWith as RegexpVisitWith};
use swc_ecma_visit::{Visit, VisitWith};
#[derive(Default)]
struct Regexp2022 {
    names: std::collections::HashSet<String>,
    invalid: bool,
}
impl RegexpVisit for Regexp2022 {
    fn visit_capturing_group(&mut self, group: &swc_ecma_regexp::ast::CapturingGroup) {
        if let Some(name) = &group.name {
            match decoded_capture_name(name) {
                Some(name) => {
                    if !self.names.insert(name) {
                        self.invalid = true;
                    }
                }
                None => self.invalid = true,
            }
        }
        group.visit_children_with(self);
    }
    fn visit_ignore_group(&mut self, group: &swc_ecma_regexp::ast::IgnoreGroup) {
        if group.modifiers.is_some() {
            self.invalid = true;
        }
        group.visit_children_with(self);
    }
    fn visit_unicode_property_escape(
        &mut self,
        property: &swc_ecma_regexp::ast::UnicodePropertyEscape,
    ) {
        // The pinned Acorn ecma13ScriptValues table excludes these aliases;
        // its ecma14 table adds them. SWC's Unicode tables are newer than that
        // ES2022 vocabulary. This is a grammar version restriction on the
        // parsed property, independent of any observer input or expected value.
        const LATER_SCRIPT_ALIASES: &[&str] = &[
            "Berf",
            "Beria_Erfe",
            "Gara",
            "Garay",
            "Gukh",
            "Gurung_Khema",
            "Hrkt",
            "Katakana_Or_Hiragana",
            "Kawi",
            "Kirat_Rai",
            "Krai",
            "Nag_Mundari",
            "Nagm",
            "Ol_Onal",
            "Onao",
            "Sidetic",
            "Sidt",
            "Sunu",
            "Sunuwar",
            "Tai_Yo",
            "Tayo",
            "Todhri",
            "Todr",
            "Tolong_Siki",
            "Tols",
            "Tulu_Tigalari",
            "Tutg",
            "Unknown",
            "Zzzz",
        ];
        if matches!(
            property.name.as_ref(),
            "Script" | "sc" | "Script_Extensions" | "scx"
        ) && property
            .value
            .as_ref()
            .is_some_and(|v| LATER_SCRIPT_ALIASES.contains(&v.as_ref()))
        {
            self.invalid = true;
        }
    }
}
// Regexp AST capture names preserve escape spelling. Parse an IdentifierName
// property with the same native ECMAScript parser to obtain the cooked name,
// including keyword names; no source text is evaluated.
fn decoded_capture_name(raw: &str) -> Option<String> {
    let source = format!("({{{raw}:0}})");
    let mut parser = SwcParser::new_from(Lexer::new(
        Syntax::Es(Default::default()),
        EsVersion::Es2022,
        StringInput::new(&source, BytePos(1), BytePos(source.len() as u32 + 1)),
        None,
    ));
    let parsed = parser.parse_expr().ok()?;
    if !parser.take_errors().is_empty() {
        return None;
    }
    let Expr::Paren(paren) = *parsed else {
        return None;
    };
    let Expr::Object(object) = *paren.expr else {
        return None;
    };
    let [PropOrSpread::Prop(property)] = object.props.as_slice() else {
        return None;
    };
    let Prop::KeyValue(property) = &**property else {
        return None;
    };
    let PropName::Ident(name) = &property.key else {
        return None;
    };
    Some(name.sym.to_string())
}
fn syntax() -> Error {
    Error::new("SyntaxError", "static_script_syntax_unqualified")
}
pub struct Parser<'a> {
    source: &'a str,
    handler: bool,
}
impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Result<Self> {
        Ok(Self {
            source,
            handler: false,
        })
    }
    pub fn handler(source: &'a str) -> Result<Self> {
        Ok(Self {
            source,
            handler: true,
        })
    }
    fn parser(&self) -> SwcParser<Lexer<'a>> {
        SwcParser::new_from(Lexer::new(
            Syntax::Es(EsSyntax {
                allow_return_outside_function: self.handler,
                ..Default::default()
            }),
            EsVersion::Es2022,
            StringInput::new(
                self.source,
                BytePos(1),
                BytePos(self.source.len() as u32 + 1),
            ),
            None,
        ))
    }
    pub fn program(&mut self) -> Result<Node> {
        let mut parser = self.parser();
        let tree = parser.parse_script().map_err(|_| syntax())?;
        if !parser.take_errors().is_empty() || tree.shebang.is_some() {
            return Err(syntax());
        }
        let mut projection = Projection {
            source: self.source,
            ..Default::default()
        };
        tree.visit_with(&mut projection);
        projection.finish(
            "Program",
            Span::new(BytePos(1), BytePos(self.source.len() as u32 + 1)),
        )
    }
    pub fn expr(&mut self, _min: u8) -> Result<Node> {
        let mut parser = self.parser();
        let tree = parser.parse_expr().map_err(|_| syntax())?;
        if !parser.take_errors().is_empty() {
            return Err(syntax());
        }
        let mut projection = Projection {
            source: self.source,
            ..Default::default()
        };
        tree.visit_with(&mut projection);
        if projection.invalid {
            return Err(syntax());
        }
        projection.nodes.pop().ok_or_else(syntax)
    }
}
#[derive(Default)]
struct Projection<'a> {
    source: &'a str,
    nodes: Vec<Node>,
    invalid: bool,
}
impl Projection<'_> {
    fn node(kind: &'static str, span: Span, children: Vec<Node>) -> Node {
        Node::new(
            kind,
            span.lo.0.saturating_sub(1) as usize,
            span.hi.0.saturating_sub(1) as usize,
            children,
        )
    }
    fn finish(self, kind: &'static str, span: Span) -> Result<Node> {
        if self.invalid {
            return Err(syntax());
        }
        Ok(Self::node(kind, span, self.nodes))
    }
    fn group(&mut self, kind: &'static str, span: Span, visit: impl FnOnce(&mut Self)) {
        let parent = std::mem::take(&mut self.nodes);
        visit(self);
        let children = std::mem::replace(&mut self.nodes, parent);
        self.nodes.push(Self::node(kind, span, children));
    }
    fn ident(&mut self, name: &str, span: Span) {
        let mut n = Self::node("Identifier", span, vec![]);
        n.name = name.into();
        self.nodes.push(n);
    }
    fn literal(&mut self, l: &Lit) {
        let mut n = Self::node("Literal", l.span(), vec![]);
        n.literal = match l {
            Lit::Str(s) => Some(Js::Str(s.value.to_ill_formed_utf16().collect())),
            Lit::Bool(b) => Some(b.value.into()),
            Lit::Null(_) => Some(Js::Null),
            Lit::Num(n) if n.value.is_finite() => Some(Js::Num(n.value)),
            Lit::Regex(r) => {
                let mut seen = vec![];
                for f in r.flags.chars() {
                    if !"dgimsuy".contains(f) || seen.contains(&f) {
                        self.invalid = true;
                    }
                    seen.push(f);
                }
                match LiteralParser::new(&r.exp, Some(&r.flags), RegexpOptions::default()).parse() {
                    Ok(pattern) => {
                        let mut version = Regexp2022::default();
                        pattern.visit_with(&mut version);
                        self.invalid |= version.invalid;
                    }
                    Err(_) => self.invalid = true,
                }
                None
            }
            _ => None,
        };
        self.nodes.push(n);
    }
    fn method_function(&mut self, key: &PropName, function: &Function) {
        self.method_function_at(key.span(), function);
    }
    fn method_function_at(&mut self, key_span: Span, function: &Function) {
        // SWC includes the property key in a method Function span; Acorn starts
        // its value at the parameter-list token. Recover that native token span
        // directly after the fully parsed key, retaining comments and UTF16.
        let start = key_span.hi.0.saturating_sub(1) as usize;
        let end = function.span.hi.0.saturating_sub(1) as usize;
        let mut lexer = Lexer::new(
            Syntax::Es(Default::default()),
            EsVersion::Es2022,
            StringInput::new(
                &self.source[start..end],
                BytePos(start as u32 + 1),
                BytePos(end as u32 + 1),
            ),
            None,
        );
        let begin = lexer.next().map_or(function.span.lo, |t| t.span.lo);
        self.group(
            "FunctionExpression",
            Span::new(begin, function.span.hi),
            |p| function.visit_children_with(p),
        );
    }
    fn chain_child(&mut self, e: &Expr) {
        if let Expr::OptChain(o) = e {
            self.chain_base(&o.base);
        } else {
            e.visit_with(self);
        }
    }
    fn chain_base(&mut self, base: &OptChainBase) {
        match base {
            OptChainBase::Member(m) => {
                self.group("MemberExpression", m.span, |p| {
                    p.chain_child(&m.obj);
                    match &m.prop {
                        MemberProp::Ident(i) => p.ident(&i.sym, i.span),
                        MemberProp::Computed(c) => c.expr.visit_with(p),
                        MemberProp::PrivateName(_) => {
                            p.nodes
                                .push(Self::node("PrivateIdentifier", m.prop.span(), vec![]))
                        }
                    }
                });
                if let Some(n) = self.nodes.last_mut() {
                    n.computed = matches!(m.prop, MemberProp::Computed(_));
                }
            }
            OptChainBase::Call(c) => self.group("CallExpression", c.span, |p| {
                p.chain_child(&c.callee);
                c.args.visit_with(p);
            }),
        }
    }
    fn member(&mut self, m: &MemberExpr) {
        self.group("MemberExpression", m.span, |p| {
            m.obj.visit_with(p);
            match &m.prop {
                MemberProp::Ident(i) => p.ident(&i.sym, i.span),
                MemberProp::Computed(c) => c.expr.visit_with(p),
                MemberProp::PrivateName(_) => {
                    p.nodes
                        .push(Self::node("PrivateIdentifier", m.prop.span(), vec![]))
                }
            }
        });
        if let Some(n) = self.nodes.last_mut() {
            n.computed = matches!(m.prop, MemberProp::Computed(_));
        }
    }
}
impl Visit for Projection<'_> {
    fn visit_ident(&mut self, i: &Ident) {
        self.ident(&i.sym, i.span);
    }
    fn visit_ident_name(&mut self, i: &IdentName) {
        self.ident(&i.sym, i.span);
    }
    fn visit_lit(&mut self, l: &Lit) {
        self.literal(l);
    }
    fn visit_str(&mut self, s: &Str) {
        self.literal(&Lit::Str(s.clone()));
    }
    fn visit_number(&mut self, n: &Number) {
        self.literal(&Lit::Num(n.clone()));
    }
    fn visit_big_int(&mut self, n: &BigInt) {
        self.literal(&Lit::BigInt(n.clone()));
    }
    fn visit_stmt(&mut self, s: &Stmt) {
        if let Stmt::Block(block) = s {
            block.visit_with(self);
            return;
        }
        if let Stmt::Decl(Decl::Var(declaration)) = s {
            declaration.visit_with(self);
            return;
        }
        let kind = if matches!(s, Stmt::Decl(Decl::Fn(_))) {
            "FunctionDeclaration"
        } else {
            "Statement"
        };
        self.group(kind, s.span(), |p| s.visit_children_with(p));
    }
    fn visit_block_stmt(&mut self, block: &BlockStmt) {
        self.group("BlockStatement", block.span, |p| block.stmts.visit_with(p));
    }
    fn visit_var_decl(&mut self, declaration: &VarDecl) {
        self.group("VariableDeclaration", declaration.span, |p| {
            declaration.decls.visit_with(p)
        });
    }
    fn visit_class(&mut self, class: &Class) {
        // ESTree visits the superclass before ClassBody. Keep the wrappers
        // Acorn counts even where SWC represents a body as a plain Vec.
        class.super_class.visit_with(self);
        self.group("ClassBody", class.span, |p| class.body.visit_with(p));
    }
    fn visit_class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Empty(_) => {} // Acorn omits class semicolons.
            ClassMember::Method(method) => self.group("MethodDefinition", method.span, |p| {
                method.key.visit_with(p);
                p.method_function(&method.key, &method.function);
            }),
            ClassMember::PrivateMethod(method) => {
                self.group("MethodDefinition", method.span, |p| {
                    method.key.visit_with(p);
                    p.method_function_at(method.key.span, &method.function);
                })
            }
            ClassMember::Constructor(constructor) => {
                self.group("MethodDefinition", constructor.span, |p| {
                    constructor.key.visit_with(p);
                    p.group("FunctionExpression", constructor.span, |p| {
                        constructor.params.visit_with(p);
                        constructor.body.visit_with(p);
                    });
                })
            }
            ClassMember::ClassProp(property) => {
                self.group("PropertyDefinition", property.span, |p| {
                    property.key.visit_with(p);
                    property.value.visit_with(p);
                })
            }
            ClassMember::PrivateProp(property) => {
                self.group("PropertyDefinition", property.span, |p| {
                    property.key.visit_with(p);
                    property.value.visit_with(p);
                })
            }
            ClassMember::StaticBlock(block) => self.group("StaticBlock", block.span, |p| {
                block.body.stmts.visit_with(p)
            }),
            _ => {
                self.invalid = true;
            }
        }
    }
    fn visit_private_name(&mut self, name: &PrivateName) {
        self.nodes
            .push(Self::node("PrivateIdentifier", name.span, vec![]));
    }
    fn visit_super(&mut self, value: &Super) {
        self.nodes.push(Self::node("Super", value.span, vec![]));
    }
    fn visit_simple_assign_target(&mut self, target: &SimpleAssignTarget) {
        match target {
            SimpleAssignTarget::Member(member) => self.member(member),
            SimpleAssignTarget::SuperProp(member) => {
                self.group("MemberExpression", member.span, |p| {
                    member.visit_children_with(p)
                });
            }
            SimpleAssignTarget::Paren(paren) => paren.expr.visit_with(self),
            _ => target.visit_children_with(self),
        }
    }
    fn visit_assign_target_pat(&mut self, target: &AssignTargetPat) {
        self.group("Pattern", target.span(), |p| target.visit_children_with(p));
    }
    fn visit_prop_or_spread(&mut self, property: &PropOrSpread) {
        match property {
            PropOrSpread::Prop(property) => property.visit_with(self),
            PropOrSpread::Spread(spread) => self.group("SpreadElement", spread.span(), |p| {
                spread.expr.visit_with(p)
            }),
        }
    }
    fn visit_tpl(&mut self, template: &Tpl) {
        self.group("TemplateLiteral", template.span, |p| {
            template.visit_children_with(p)
        });
    }
    fn visit_function_body(&mut self, body: &FunctionBody) {
        self.group("BlockStatement", body.span, |p| body.visit_children_with(p));
    }
    fn visit_switch_case(&mut self, case: &SwitchCase) {
        self.group("SwitchCase", case.span, |p| case.visit_children_with(p));
    }
    fn visit_catch_clause(&mut self, clause: &CatchClause) {
        self.group("CatchClause", clause.span, |p| {
            clause.visit_children_with(p)
        });
    }
    fn visit_tpl_element(&mut self, element: &TplElement) {
        self.nodes
            .push(Self::node("TemplateElement", element.span, vec![]));
    }
    fn visit_var_declarator(&mut self, v: &VarDeclarator) {
        self.group("VariableDeclarator", v.span, |p| v.visit_children_with(p));
    }
    fn visit_pat(&mut self, pat: &Pat) {
        if let Pat::Ident(i) = pat {
            self.ident(&i.id.sym, i.id.span);
        } else if let Pat::Expr(expression) = pat {
            expression.visit_with(self);
        } else {
            self.group("Pattern", pat.span(), |p| pat.visit_children_with(p));
        }
    }
    fn visit_object_pat_prop(&mut self, property: &ObjectPatProp) {
        match property {
            ObjectPatProp::KeyValue(property) => self.group("Property", property.span(), |p| {
                property.visit_children_with(p)
            }),
            ObjectPatProp::Assign(property) => self.group("Property", property.span, |p| {
                p.ident(&property.key.id.sym, property.key.id.span);
                if let Some(value) = &property.value {
                    p.group("AssignmentPattern", property.span, |p| {
                        p.ident(&property.key.id.sym, property.key.id.span);
                        value.visit_with(p);
                    });
                } else {
                    p.ident(&property.key.id.sym, property.key.id.span);
                }
            }),
            ObjectPatProp::Rest(rest) => {
                self.group("RestElement", rest.span, |p| rest.arg.visit_with(p))
            }
        }
    }
    fn visit_expr_or_spread(&mut self, e: &ExprOrSpread) {
        if e.spread.is_some() {
            self.group("SpreadElement", e.span(), |p| e.expr.visit_with(p));
        } else {
            e.expr.visit_with(self);
        }
    }
    fn visit_prop(&mut self, prop: &Prop) {
        // Acorn properties have a key and value even for shorthand/getters/methods.
        self.group("Property", prop.span(), |p| match prop {
            Prop::Shorthand(i) => {
                p.ident(&i.sym, i.span);
                p.ident(&i.sym, i.span);
            }
            Prop::KeyValue(v) => {
                v.key.visit_with(p);
                v.value.visit_with(p);
            }
            Prop::Getter(v) => {
                v.key.visit_with(p);
                p.method_function(&v.key, &v.function);
            }
            Prop::Setter(v) => {
                v.key.visit_with(p);
                p.method_function(&v.key, &v.function);
            }
            Prop::Method(v) => {
                v.key.visit_with(p);
                p.method_function(&v.key, &v.function);
            }
            _ => prop.visit_children_with(p),
        });
        let computed = match prop {
            Prop::KeyValue(v) => matches!(v.key, PropName::Computed(_)),
            Prop::Getter(v) => matches!(v.key, PropName::Computed(_)),
            Prop::Setter(v) => matches!(v.key, PropName::Computed(_)),
            Prop::Method(v) => matches!(v.key, PropName::Computed(_)),
            _ => false,
        };
        if let Some(n) = self.nodes.last_mut() {
            n.computed = computed;
            n.name = n
                .children
                .first()
                .and_then(|key| {
                    if computed {
                        key.scalar().filter(Js::string).map(|v| string(&v))
                    } else {
                        key.reference().or_else(|| key.scalar().map(|v| string(&v)))
                    }
                })
                .unwrap_or_default();
        }
    }
    fn visit_expr(&mut self, e: &Expr) {
        match e {
            Expr::Paren(p) => p.expr.visit_with(self),
            Expr::Tpl(template) => template.visit_with(self),
            Expr::MetaProp(meta) => self.group("MetaProperty", meta.span, |p| {
                let (first, last) = match meta.kind {
                    MetaPropKind::NewTarget => ("new", "target"),
                    MetaPropKind::ImportMeta => ("import", "meta"),
                };
                p.ident(
                    first,
                    Span::new(meta.span.lo, BytePos(meta.span.lo.0 + first.len() as u32)),
                );
                p.ident(
                    last,
                    Span::new(BytePos(meta.span.hi.0 - last.len() as u32), meta.span.hi),
                );
            }),
            Expr::Ident(i) => self.ident(&i.sym, i.span),
            Expr::Lit(l) => self.literal(l),
            Expr::Member(m) => self.member(m),
            Expr::Array(a) => self.group("ArrayExpression", a.span, |p| {
                for e in &a.elems {
                    if let Some(e) = e {
                        e.visit_with(p);
                    } else {
                        p.nodes.push(Self::node("Hole", a.span, vec![]));
                    }
                }
            }),
            Expr::OptChain(o) => self.group("ChainExpression", o.span, |p| p.chain_base(&o.base)),
            Expr::Call(c) => {
                if let Callee::Import(i) = &c.callee {
                    if c.args.len() != 1 || i.phase != ImportPhase::Evaluation {
                        self.invalid = true;
                    }
                    // The full AST loses a trailing comma. Consult the native lexer at
                    // the parsed argument boundary, without skipping or parsing syntax.
                    if let Some(arg) = c.args.last() {
                        let begin = arg.span().hi.0.saturating_sub(1) as usize;
                        let end = c.span.hi.0.saturating_sub(1) as usize;
                        let mut tail = Lexer::new(
                            Syntax::Es(Default::default()),
                            EsVersion::Es2022,
                            StringInput::new(
                                &self.source[begin..end],
                                BytePos(1),
                                BytePos((end - begin + 1) as u32),
                            ),
                            None,
                        );
                        if tail.next().is_some_and(|t| t.token.to_string() == ",") {
                            self.invalid = true;
                        }
                    }
                    self.group("ImportExpression", c.span, |p| c.args.visit_with(p));
                } else {
                    self.group("CallExpression", c.span, |p| c.visit_children_with(p));
                }
            }
            _ => {
                let kind = match e {
                    Expr::Object(_) => "ObjectExpression",
                    Expr::Fn(_) => "FunctionExpression",
                    Expr::Arrow(_) => "ArrowFunctionExpression",
                    Expr::Assign(_) => "AssignmentExpression",
                    _ => "Expression",
                };
                self.group(kind, e.span(), |p| e.visit_children_with(p));
            }
        }
    }
}
