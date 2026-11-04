//! Static presentation evidence: inert DOM, syntax trees, literal values and raw spans.
use crate::{
    data::*,
    dom::{self, Document, Range},
    script::{self, Node, Parser},
};
use scraper::ElementRef;
use std::collections::{BTreeSet, HashMap};
const FILTERS: [&str; 3] = [
    "g_examtype_filter",
    "searchConditionDifficulty",
    "searchConditionBehAreaNo",
];
const PREVIEW: [&str; 3] = ["previewDBExam", "getExamPath", "getExamCardImageFileName"];
const SELECTOR_PIN: &str = "dbe508a27dc8ea46dbdf5de697733e4c4c56afc5c766bacfa202997070e84992";
const HELPERS_PIN: &str = "a42379a32fa0183f4a9f97f00d088cd09f4eb38e89f8d43a7432f3fb74483c50";
struct Script {
    source: &'static str,
    text: String,
    offset: usize,
    tree: Option<Node>,
}
fn bounded(v: &str, max: usize) -> Result<Js> {
    let u = text(v);
    if u.len() > max || u.iter().any(|c| matches!(c,0..=8|11|12|14..=31)) {
        return Err(Error::error("test_pool_presentation_text_limit"));
    }
    Ok(Js::Str(u))
}
fn attribute(e: ElementRef<'_>, name: &str) -> Result<Js> {
    e.value()
        .attr(name)
        .map_or(Ok(Js::Null), |v| bounded(v, 4096))
}
fn range(n: &Node, s: &Script) -> Range {
    Range {
        start: s.offset + s.text[..n.start].encode_utf16().count(),
        end: s.offset + s.text[..n.end].encode_utf16().count(),
    }
}
fn scalar(n: Option<&Node>) -> Result<Option<Js>> {
    n.map(Node::checked_scalar).transpose().map(Option::flatten)
}
fn reference(n: Option<&Node>) -> Result<Option<String>> {
    n.map(Node::checked_reference)
        .transpose()
        .map(Option::flatten)
}
fn source_string(n: Option<&Node>) -> Result<Js> {
    Ok(scalar(n)?.filter(Js::string).unwrap_or(Js::Null))
}
fn property<'a>(n: &'a Node, name: &str) -> Result<Option<&'a Node>> {
    let mut matches = vec![];
    for p in n.children.iter().filter(|p| p.kind == "Property") {
        let key = p.children.first();
        let value = if p.computed {
            scalar(key)?
        } else {
            match reference(key)? {
                Some(r) => Some(r.into()),
                None => scalar(key)?,
            }
        };
        if value.as_ref().is_some_and(|v| v.is(name)) {
            matches.push(p);
        }
    }
    if matches.len() > 1 {
        return Err(Error::error("test_pool_presentation_property_ambiguous"));
    }
    Ok(matches.first().and_then(|p| p.children.last()))
}
fn add_function(functions: &mut Vec<Js>, name: &str, n: &Node, s: &Script) -> Result<()> {
    if functions.len() >= 64 {
        return Err(Error::error("test_pool_presentation_function_limit"));
    }
    let raw = &s.text[n.start..n.end];
    if raw.len() > 32000 {
        return Err(Error::error("test_pool_presentation_function_size_limit"));
    }
    let mut literals = vec![];
    for c in script::walk(n)? {
        if let Some(value) = c.checked_scalar()? {
            if literals.len() >= 128 {
                return Err(Error::error("test_pool_presentation_literal_limit"));
            }
            if value.string() {
                bounded(&string(&value), 4096)?;
            }
            literals.push((
                c.start,
                obj(&[("valueRaw", value), ("sourceRange", range(c, s).value())]),
            ));
        }
    }
    literals.sort_by_key(|(i, _)| *i);
    functions.push(obj(&[
        ("name", bounded(name, 256)?),
        ("source", s.source.into()),
        ("sourceRange", range(n, s).value()),
        ("sourceSha256", hash(json(&Js::from(raw)).as_bytes()).into()),
        (
            "literals",
            Js::Arr(literals.into_iter().map(|(_, v)| v).collect()),
        ),
    ]));
    Ok(())
}
pub fn inspect(html: &str, helpers: &str) -> Result<Js> {
    inspect_with(html, helpers, |doc, e| doc.range(e))
}
pub fn inspect_with(
    html: &str,
    helpers: &str,
    location: impl Fn(&Document, ElementRef<'_>) -> Option<Range>,
) -> Result<Js> {
    if html.len() > 2_000_000 || helpers.len() > 2_000_000 {
        return Err(Error::error("test_pool_presentation_source_limit"));
    }
    let doc = Document::parse(html);
    if doc.select("*").len() > 20000 {
        return Err(Error::error("test_pool_presentation_document_limit"));
    }
    if !doc.select("input[type=password]").is_empty()
        || html.contains("normal_session_error.jsp")
        || html.contains("세션이 만료")
    {
        return Err(Error::error("test_pool_authentication_required"));
    }
    let node_range = |e| -> Result<Range> {
        let r = location(&doc, e)
            .ok_or_else(|| Error::error("test_pool_presentation_source_range_unqualified"))?;
        if r.end < r.start || r.end > html.encode_utf16().count() {
            return Err(Error::error(
                "test_pool_presentation_source_range_unqualified",
            ));
        }
        Ok(r)
    };
    let mut unknowns = BTreeSet::<String>::new();
    let mut scripts = vec![];
    for e in doc.select("script:not([src])") {
        if scripts.len() >= 64 {
            return Err(Error::error("test_pool_presentation_script_limit"));
        }
        let t = e.value().attr("type").unwrap_or("").to_lowercase();
        if !t.is_empty() && !matches!(t.as_str(), "text/javascript" | "application/javascript") {
            unknowns.insert("nonclassic_selector_script_not_inspected".into());
            continue;
        }
        if let Some((s, offset)) = doc.script(e)
            && !s.is_empty()
        {
            node_range(e)?;
            let tree = Parser::new(s).and_then(|mut p| p.program()).ok();
            if tree.is_none() {
                unknowns.insert("selector_script_parse_unqualified".into());
            }
            scripts.push(Script {
                source: "selector",
                text: s.into(),
                offset,
                tree,
            });
        }
    }
    let tree = Parser::new(helpers).and_then(|mut p| p.program()).ok();
    if tree.is_none() {
        unknowns.insert("helpers_script_parse_unqualified".into());
    }
    scripts.push(Script {
        source: "helpers",
        text: helpers.into(),
        offset: 0,
        tree,
    });
    let mut labels = HashMap::<String, Vec<String>>::new();
    for label in doc.select("label[for]") {
        let text = dom::content(label);
        bounded(&text, 4096)?;
        labels
            .entry(label.value().attr("for").unwrap().into())
            .or_default()
            .push(text);
    }
    let relevant=regex::Regex::new(r"(?i)difficulty|exam[_-]?type|question[_-]?type|sel[_-]?type|response|(?:obj|sbj|des)[_-]?option|난이도|서술형|주관식|객관식").unwrap();
    let mut controls = vec![];
    for c in doc.select("select,input[type=radio],input[type=checkbox],button,a") {
        let id = c.value().attr("id");
        let name = c.value().attr("name");
        let mut handlers = ["onchange", "onclick", "oninput"]
            .iter()
            .filter_map(|k| {
                c.value()
                    .attr(k)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        if let Some(href) = c.value().attr("href")
            && href.to_ascii_lowercase().starts_with("javascript:")
        {
            handlers.push(href[11..].into());
        }
        let mut associated = id
            .and_then(|id| labels.get(id))
            .cloned()
            .unwrap_or_default();
        if associated.is_empty()
            && let Some(w) = dom::closest(c, "label")
        {
            let t = dom::content(w);
            bounded(&t, 4096)?;
            associated.push(t);
        }
        let own = if matches!(c.value().name(), "button" | "a") {
            Some(dom::content(c))
        } else {
            None
        };
        let label = if associated.len() == 1 {
            Some(associated[0].clone())
        } else {
            own
        };
        let search = [
            id.unwrap_or(""),
            name.unwrap_or(""),
            label.as_deref().unwrap_or(""),
            &handlers.join(" "),
        ]
        .join(" ");
        if !relevant.is_match(&search) {
            continue;
        }
        if controls.len() >= 64 {
            return Err(Error::error("test_pool_presentation_control_limit"));
        }
        if associated.len() > 1 {
            unknowns.insert("control_multiple_associated_labels".into());
        }
        let mut references = BTreeSet::new();
        for h in handlers {
            bounded(&h, 4096)?;
            match Parser::handler(&h).and_then(|mut p| p.program()) {
                Ok(tree) => {
                    for n in script::walk(&tree)? {
                        if n.kind == "CallExpression"
                            && let Some(name) = reference(n.children.first())?
                        {
                            references.insert(name);
                        }
                    }
                }
                Err(_) => {
                    unknowns.insert("control_handler_parse_unqualified".into());
                }
            }
        }
        let options = dom::select(c, "option");
        if options.len() > 100 {
            return Err(Error::error("test_pool_presentation_option_limit"));
        }
        let options = options
            .into_iter()
            .map(|o| {
                Ok(obj(&[
                    ("valueRaw", attribute(o, "value")?),
                    ("labelExact", bounded(&dom::content(o), 4096)?),
                    ("selected", o.value().attr("selected").is_some().into()),
                    ("sourceRange", node_range(o)?.value()),
                ]))
            })
            .collect::<Result<Vec<_>>>()?;
        controls.push(obj(&[
            ("tag", c.value().name().into()),
            ("id", attribute(c, "id")?),
            ("name", attribute(c, "name")?),
            ("type", attribute(c, "type")?),
            (
                "labelExact",
                label.map_or(Ok(Js::Null), |s| bounded(&s, 4096))?,
            ),
            ("valueRaw", attribute(c, "value")?),
            ("sourceRange", node_range(c)?.value()),
            ("options", Js::Arr(options)),
            (
                "handlerReferences",
                Js::Arr(references.into_iter().map(Js::from).collect()),
            ),
        ]));
    }
    let mut filters = vec![];
    let mut columns = vec![];
    let mut referenced = BTreeSet::<String>::new();
    let mut inline = vec![];
    let mut grid_count = 0;
    let grid_fallback =
        regex::Regex::new(r##"(?:\$|jQuery)\(["']#dbExamListGrid["']\)\.jqGrid\s*\("##).unwrap();
    for (si, s) in scripts.iter().enumerate() {
        if s.source != "selector" {
            continue;
        }
        let mut fallback = vec![];
        if s.tree.is_none() {
            for m in grid_fallback.find_iter(&s.text) {
                match Parser::new(&s.text[m.start()..]).and_then(|mut p| p.expr(0)) {
                    Ok(mut n) => {
                        fn shift(n: &mut Node, b: usize) {
                            n.start += b;
                            n.end += b;
                            for c in &mut n.children {
                                shift(c, b)
                            }
                        }
                        shift(&mut n, m.start());
                        fallback.push(n)
                    }
                    Err(_) => {
                        unknowns.insert("question_grid_parse_unqualified".into());
                    }
                }
            }
        }
        let trees = s
            .tree
            .as_ref()
            .map_or_else(|| fallback.iter().collect::<Vec<_>>(), |t| vec![t]);
        for tree in trees {
            for n in script::walk(tree)? {
                if n.kind == "VariableDeclarator"
                    && let Some(name) = reference(n.children.first())?
                    && FILTERS.contains(&name.as_str())
                {
                    if let Some(value) = scalar(n.children.get(1))? {
                        filters.push(obj(&[
                            ("name", name.into()),
                            ("valueRaw", value),
                            ("sourceRange", range(n, s).value()),
                        ]));
                    } else {
                        unknowns.insert(format!("filter_declaration_not_literal:{name}"));
                    }
                }
                if n.kind != "CallExpression" {
                    continue;
                }
                let Some(callee) = n.children.first() else {
                    continue;
                };
                if callee.kind != "MemberExpression" {
                    continue;
                }
                let Some(method) = callee.children.get(1) else {
                    continue;
                };
                let method_name = match method.checked_reference()? {
                    Some(r) => Some(r),
                    None => method
                        .checked_scalar()?
                        .filter(Js::string)
                        .map(|v| string(&v)),
                };
                if method_name.as_deref() != Some("jqGrid") {
                    continue;
                }
                let Some(object) = callee.children.first() else {
                    continue;
                };
                if object.kind != "CallExpression"
                    || !matches!(
                        reference(object.children.first())?.as_deref(),
                        Some("$" | "jQuery")
                    )
                    || !source_string(object.children.get(1))?.is("#dbExamListGrid")
                {
                    continue;
                }
                let Some(config) = n.children.get(1).filter(|n| n.kind == "ObjectExpression")
                else {
                    unknowns.insert("question_grid_configuration_dynamic".into());
                    continue;
                };
                grid_count += 1;
                let Some(model) =
                    property(config, "colModel")?.filter(|n| n.kind == "ArrayExpression")
                else {
                    unknowns.insert("question_grid_columns_dynamic".into());
                    continue;
                };
                if model.children.len() > 80 {
                    return Err(Error::error("test_pool_presentation_column_limit"));
                }
                let names = property(config, "colNames")?.filter(|n| n.kind == "ArrayExpression");
                for (ordinal, column) in model.children.iter().enumerate() {
                    if columns.len() >= 80 {
                        return Err(Error::error("test_pool_presentation_column_limit"));
                    }
                    if column.kind != "ObjectExpression" {
                        unknowns.insert("question_grid_column_dynamic".into());
                        continue;
                    }
                    let name = source_string(property(column, "name")?)?;
                    let index = source_string(property(column, "index")?)?;
                    let mut label = source_string(property(column, "label")?)?;
                    if matches!(label, Js::Null) {
                        label = source_string(names.and_then(|n| n.children.get(ordinal)))?
                    }
                    let formatter = property(column, "formatter")?;
                    let reference = match reference(formatter)? {
                        Some(r) => Some(r),
                        None => scalar(formatter)?.filter(Js::string).map(|v| string(&v)),
                    };
                    let kind = if formatter.is_none() {
                        "absent"
                    } else if reference.as_ref().is_some_and(|s| !s.is_empty()) {
                        "reference"
                    } else if formatter.is_some_and(|n| {
                        matches!(n.kind, "FunctionExpression" | "ArrowFunctionExpression")
                    }) {
                        "inline"
                    } else {
                        "dynamic"
                    };
                    if let Some(r) = &reference {
                        referenced.insert(r.clone());
                    }
                    if kind == "inline" {
                        inline.push((
                            format!(
                                "{}.formatter",
                                if name.string() {
                                    string(&name)
                                } else {
                                    ordinal.to_string()
                                }
                            ),
                            formatter.unwrap().clone(),
                            si,
                        ));
                    }
                    if kind == "dynamic" {
                        unknowns.insert("question_grid_formatter_dynamic".into());
                    }
                    columns.push(obj(&[
                        ("name", name),
                        ("index", index),
                        ("labelExact", label),
                        ("formatterReference", reference.map_or(Js::Null, Js::from)),
                        ("formatterKind", kind.into()),
                        ("sourceRange", range(column, s).value()),
                    ]));
                }
            }
        }
    }
    if grid_count == 0 {
        unknowns.insert("question_grid_not_observed".into());
    }
    if grid_count > 1 {
        unknowns.insert("question_grid_initializer_ambiguous".into());
    }
    for f in FILTERS {
        if filters.iter().filter(|r| r.get("name").is(f)).count() != 1 {
            unknowns.insert(format!("filter_declaration_missing_or_ambiguous:{f}"));
        }
    }
    if controls.is_empty() {
        unknowns.insert("relevant_literal_controls_not_observed".into());
    }
    let mut functions = vec![];
    let formatter_name =
        regex::Regex::new(r"(?i)^dbFormatter.*(?:Difficulty|Types|Exam|Option|Answer)").unwrap();
    for s in &scripts {
        if let Some(tree) = &s.tree {
            for n in script::walk(tree)? {
                let name = if n.kind == "FunctionDeclaration"
                    || (n.kind == "VariableDeclarator"
                        && n.children.get(1).is_some_and(|n| {
                            matches!(n.kind, "FunctionExpression" | "ArrowFunctionExpression")
                        })) {
                    reference(n.children.first())?
                } else {
                    None
                };
                if let Some(name) = name
                    && (PREVIEW.contains(&name.as_str())
                        || referenced.contains(&name)
                        || formatter_name.is_match(&name))
                {
                    add_function(&mut functions, &name, n, s)?;
                }
            }
        }
    }
    for (name, n, si) in inline {
        add_function(&mut functions, &name, &n, &scripts[si])?;
    }
    for name in &referenced {
        if !functions.iter().any(|r| r.get("name").is(name)) {
            unknowns.insert(format!("formatter_definition_not_observed:{name}"));
        }
    }
    for name in PREVIEW {
        if !functions.iter().any(|r| r.get("name").is(name)) {
            unknowns.insert(format!("preview_function_not_observed:{name}"));
        }
    }
    for f in &functions {
        let name = string(f.get("name"));
        if functions.iter().filter(|r| r.get("name").is(&name)).count() > 1 {
            unknowns.insert(format!("function_definition_ambiguous:{name}"));
        }
    }
    let selector_sha = hash(json(&Js::from(html)).as_bytes());
    let helpers_sha = hash(json(&Js::from(helpers)).as_bytes());
    let sm = selector_sha == SELECTOR_PIN;
    let hm = helpers_sha == HELPERS_PIN;
    if !sm || !hm {
        unknowns.insert("source_revisions_differ_from_preview_qualification_pins".into());
    }
    unknowns.insert("literal_values_and_labels_do_not_establish_code_semantics".into());
    unknowns.insert("question_and_solution_content_roles_not_normalized".into());
    let result = obj(&[
        ("schemaVersion", Js::Num(1.)),
        ("sourceOperation", "test_pool_presentation_protocol".into()),
        ("selectorSha256", selector_sha.into()),
        ("helpersSha256", helpers_sha.into()),
        (
            "sourceBytes",
            obj(&[
                ("selector", html.len().into()),
                ("helpers", helpers.len().into()),
            ]),
        ),
        ("rangeUnit", "utf16_code_units".into()),
        (
            "sourcePins",
            obj(&[
                ("expectedSelectorSha256", SELECTOR_PIN.into()),
                ("expectedHelpersSha256", HELPERS_PIN.into()),
                ("selectorMatches", sm.into()),
                ("helpersMatch", hm.into()),
                (
                    "qualification",
                    if sm && hm {
                        "qualified_original_source"
                    } else {
                        "source_changed_unqualified"
                    }
                    .into(),
                ),
            ]),
        ),
        ("controls", Js::Arr(controls)),
        ("gridColumns", Js::Arr(columns)),
        ("functionEvidence", Js::Arr(functions)),
        ("filterDeclarations", Js::Arr(filters)),
        (
            "unknowns",
            Js::Arr(unknowns.into_iter().map(Js::from).collect()),
        ),
        ("difficultyMapping", "not_normalized".into()),
        ("responseFormatMapping", "not_normalized".into()),
        ("sourceHandlersExecuted", false.into()),
        ("academyWrites", Js::Num(0.)),
    ]);
    if json(&result).len() > 524288 {
        return Err(Error::error("test_pool_presentation_result_limit"));
    }
    Ok(result)
}
