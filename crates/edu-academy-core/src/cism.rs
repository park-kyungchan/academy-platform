//! Retained CISM pure bytes and membership. Raw runtime kind is deliberately a string.
use crate::js::*;
pub const NARRATIVES: [&str; 8] = ["i_1", "i_2", "s_1", "s_2", "s_3", "s_4", "m_1", "m_2"];
pub const WIRE_SHA: &str = "401beb7c9957d4b688e00655f6f5ece6831e1b857961b131d7cf02269fd6b67b";
fn form(v: &Js) -> String {
    let s = js_string(v);
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                out.push(*b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
pub fn js_string(v: &Js) -> String {
    match v {
        Js::Str(s) => String::from_utf16_lossy(s),
        Js::Undefined => "undefined".into(),
        Js::Null => "null".into(),
        Js::Bool(b) => b.to_string(),
        Js::Num(n) => ryu_js::Buffer::new().format(*n).to_string(),
        Js::Arr(a) => a
            .iter()
            .map(|x| {
                if x.absent() {
                    String::new()
                } else {
                    js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        _ => "[object Object]".into(),
    }
}
pub fn write_parameters(report: &Js, field: &Js, after: &Js) -> Result<Js> {
    let source = stringify(field.get("source"))?;
    let mut member = false;
    for f in report.get("fields").array() {
        if f.get("fieldRef") == field.get("fieldRef")
            && f.get("kind") == field.get("kind")
            && stringify(f.get("source"))? == source
        {
            member = true;
            break;
        }
    }
    if !member {
        return Err(Fault::error("cism_fields_unqualified"));
    }
    if !after.string() {
        return Err(Fault::type_error(if after.absent() {
            format!(
                "{} is not an object (evaluating 'after.trim')",
                if matches!(after, Js::Null) {
                    "null"
                } else {
                    "undefined"
                }
            )
        } else {
            "after.trim is not a function. (In 'after.trim()', 'after.trim' is undefined)".into()
        }));
    }
    if trim(after.units()).is_empty()
        || after.units().len() > 6000
        || after
            .units()
            .iter()
            .any(|u| matches!(*u,0..=8|11|12|14..=31))
    {
        return Err(Fault::error("cism_value_rejected"));
    }
    let kind = field.get("kind");
    if kind.is("class_mode") && !test("^[123]$", after)
        || (kind.is("think_note") || kind.is("week_test"))
            && (!test(r"^\d{1,3}(?:\.\d{1,2})?$", after)
                || after.code_units().parse::<f64>().unwrap_or(f64::NAN) > 100.0)
    {
        return Err(Fault::error("cism_value_rejected"));
    }
    let mut entries = vec![
        (
            "reqCmd",
            if kind.is("narrative") {
                "SetCismContentData"
            } else {
                "SetCismChartData"
            }
            .into(),
        ),
        ("tutor_pri_no", report.get("teacherKey").clone()),
        ("the_date", report.get("date").clone()),
        ("field_name", field.get("source").get("fieldName").clone()),
        ("field_value", after.clone()),
    ];
    if !kind.is("narrative") {
        for (wire, key) in [
            ("stu_pri_no", "studentKey"),
            ("cm_seq", "membershipKey"),
            ("grp_seq", "groupKey"),
        ] {
            let v = field.get("source").get(key);
            if !v.truthy() {
                return Err(Fault::error("cism_fields_unqualified"));
            }
            entries.push((wire, v.clone()));
        }
    }
    let wire = entries
        .iter()
        .map(|(k, v)| format!("{}={}", form(&(*k).into()), form(v)))
        .collect::<Vec<_>>()
        .join("&");
    Ok(obj(&[
        (
            "entries",
            Js::Arr(
                entries
                    .iter()
                    .map(|(k, v)| Js::Arr(vec![(*k).into(), js_string(v).into()]))
                    .collect(),
            ),
        ),
        ("wire", wire.into()),
    ]))
}
use scraper::{ElementRef, Html, Selector};
fn selector(s: &str) -> Selector {
    Selector::parse(s).expect("source selector")
}
fn content(n: ElementRef<'_>) -> String {
    n.text().collect::<String>()
}
fn trimmed(s: &str) -> String {
    String::from_utf16_lossy(trim(&text(s)))
}
fn node_value(n: ElementRef<'_>) -> String {
    match n.value().name() {
        "textarea" => content(n).replace("\r\n", "\n").replace('\r', "\n"),
        "select" => {
            let options = selector("option");
            let list: Vec<_> = n.select(&options).collect();
            let option = list
                .iter()
                .find(|n| n.value().attr("selected").is_some())
                .or(list.first());
            option.map_or(String::new(), |n| {
                n.value()
                    .attr("value")
                    .map(str::to_owned)
                    .unwrap_or_else(|| trimmed(&content(*n)))
            })
        }
        _ => n.value().attr("value").unwrap_or("").into(),
    }
}
/// Narrow source-function text selection; no script evaluation.
pub fn select_source_function(source: &str, name: &str) -> Result<Option<String>> {
    if source.encode_utf16().count() > 1000000
        || !regex(r"^[A-Za-z_$][A-Za-z0-9_$]{0,64}$").is_match(name)
    {
        return Err(Fault::error("source_function_scope_rejected"));
    }
    let re = regex(&format!(r"function\s+{name}\s*\([A-Za-z0-9_$,\s]*\)\s*\{{"));
    let matches: Vec<_> = re.find_iter(source).collect();
    if matches.is_empty() {
        return Ok(None);
    }
    if matches.len() != 1 {
        return Err(Fault::error("source_function_ambiguous"));
    }
    let found = matches[0];
    let start = found.start();
    let brace = found.end() - 1;
    let bytes = source.as_bytes();
    let mut depth = 0;
    let mut quote = 0;
    let mut escaped = false;
    let mut line = false;
    let mut block = false;
    let mut i = brace;
    while i < bytes.len() {
        if String::from_utf8_lossy(&bytes[start..i])
            .encode_utf16()
            .count()
            > 20000
        {
            return Err(Fault::error("source_function_too_large"));
        }
        let c = bytes[i];
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        if line {
            if c == b'\n' {
                line = false;
            }
            i += 1;
            continue;
        }
        if block {
            if c == b'*' && next == b'/' {
                block = false;
                i += 1;
            }
            i += 1;
            continue;
        }
        if quote != 0 {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == quote {
                quote = 0;
            }
            i += 1;
            continue;
        }
        if c == b'/' && next == b'/' {
            line = true;
            i += 2;
            continue;
        }
        if c == b'/' && next == b'*' {
            block = true;
            i += 2;
            continue;
        }
        if [b'\'', b'"', b'`'].contains(&c) {
            quote = c;
            i += 1;
            continue;
        }
        if c == b'/' {
            return Err(Fault::error("source_function_unqualified_slash_syntax"));
        }
        if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Ok(Some(source[start..=i].into()));
            }
        }
        i += 1;
    }
    Err(Fault::error("source_function_unterminated"))
}
pub fn parse_report(
    html: &str,
    date: &str,
    teacher: &Js,
    now: &str,
    events: &mut Vec<Js>,
) -> Result<Js> {
    let period = crate::leaf::cism_week(date).map_err(|e| Fault::error(e.message()))?;
    if html.len() > 2000000 {
        return Err(Fault::error("cism_body_rejected"));
    }
    events.push(obj(&[
        ("event", "inert_dom_parse".into()),
        ("mime", "text/html".into()),
        ("bytes", html.len().into()),
    ]));
    let d = Html::parse_document(html);
    let first = |s: &str| d.select(&selector(s)).next();
    let val = |id: &str| first(&format!("[id=\"{id}\"]")).map(node_value);
    if first("input[type=password]").is_some()
        || first("h1").is_none_or(|n| !content(n).contains("주간 교무회의 CISM 작성"))
    {
        return Err(Fault::error("cism_document_rejected"));
    }
    if val("the_date").as_deref() != Some(date) {
        return Err(Fault::error("cism_scope_conflict"));
    }
    if teacher.absent() {
        return Err(Fault::type_error(format!(
            "{} is not an object (evaluating 'teacher.teacherKey')",
            if matches!(teacher, Js::Null) {
                "null"
            } else {
                "undefined"
            }
        )));
    }
    if val("tutor_pri_no").as_deref() != Some(&teacher.get("teacherKey").code_units())
        || val("teacher_name").as_deref() != Some(&teacher.get("teacherLabel").code_units())
        || first("#theDateForm [name=reqCmd]").and_then(|n| n.value().attr("value"))
            != Some("CismChartMain")
    {
        return Err(Fault::error("cism_scope_conflict"));
    }
    let mut wire = vec![];
    for name in ["setCismData", "onTextareaFocusOut"] {
        wire.push(select_source_function(&html.replace("\r\n", "\n"), name)?);
    }
    if wire.iter().any(Option::is_none)
        || sha(&wire.into_iter().flatten().collect::<Vec<_>>().join("\n")) != WIRE_SHA
    {
        return Err(Fault::error("cism_wire_drift"));
    }
    let week_label = first("#theDateForm label")
        .map(|n| trimmed(&content(n)))
        .unwrap_or_default();
    let re = regex(r"^(20\d{2})-(\d{1,2}) 주차 \((\d{1,2})월(\d{1,2})일~(\d{1,2})월(\d{1,2})일\)$");
    let good = re.captures(&week_label).is_some_and(|c| {
        [
            (3, &period.period_start[5..7]),
            (4, &period.period_start[8..10]),
            (5, &period.period_end[5..7]),
            (6, &period.period_end[8..10]),
        ]
        .iter()
        .all(|(i, v)| c[*i].parse::<u32>() == v.parse::<u32>())
    });
    if !good {
        return Err(Fault::error("cism_period_conflict"));
    }
    let mut fields = vec![];
    for name in NARRATIVES {
        let sel = selector(&format!("textarea#{name}"));
        let nodes: Vec<_> = d.select(&sel).collect();
        if nodes.len() != 1 {
            return Err(Fault::error("cism_fields_unqualified"));
        }
        let n = nodes[0];
        if n.value().attr("name") != Some(name)
            || n.value().attr("onfocusout")
                != Some(format!("javascript:onTextareaFocusOut(this,'{name}');").as_str())
        {
            return Err(Fault::error("cism_fields_unqualified"));
        }
        let previous = n.prev_siblings().find_map(ElementRef::wrap);
        let label = previous
            .map(|n| trimmed(&content(n)))
            .unwrap_or(name.into());
        let source = obj(&[("fieldName", name.into())]);
        fields.push(obj(&[
            (
                "fieldRef",
                digest(
                    &Js::Arr(vec!["narrative".into(), source.clone()]),
                    Family::Cism,
                )?
                .into(),
            ),
            ("kind", "narrative".into()),
            ("label", label.into()),
            ("value", node_value(n).into()),
            ("studentName", Js::Null),
            ("groupName", Js::Null),
            ("source", source),
        ]));
    }
    let control_selector = selector("select[id^=class_mode_],input[onfocusout]");
    let controls: Vec<_> = d.select(&control_selector).collect();
    let mut groups = std::collections::HashMap::new();
    for n in controls.iter().filter(|n| n.value().name() == "select") {
        if let Some(c) = n
            .value()
            .attr("onchange")
            .and_then(|h| regex(r",(\d+)\);$").captures(h))
        {
            let root = n
                .ancestors()
                .filter_map(ElementRef::wrap)
                .find(|n| n.value().attr("id") == Some("root"));
            let label = root
                .and_then(|r| r.select(&selector("[id^=grp_name_]")).next())
                .or_else(|| {
                    n.value().attr("id").and_then(|id| {
                        first(&format!(
                            "#grp_name_{}",
                            id.trim_start_matches("class_mode_")
                        ))
                    })
                });
            if let Some(label) = label {
                let label = trimmed(&content(label));
                if !label.is_empty() {
                    groups.insert(c[1].to_owned(), label);
                }
            }
        }
    }
    for n in controls {
        let handler = n
            .value()
            .attr("onchange")
            .or(n.value().attr("onfocusout"))
            .unwrap_or("");
        if !handler.contains("setCismData(") {
            continue;
        }
        let re = regex(
            r"^setCismData\('(class_mode|think_note|week_test)',(\d+),([1-9]\d{0,15}),([1-9]\d{0,15}),([1-9]\d{0,15})\);$",
        );
        let c = re
            .captures(handler)
            .ok_or_else(|| Fault::error("cism_fields_unqualified"))?;
        let kind = &c[1];
        let id = format!("{}_{}", kind, &c[2]);
        if n.value().attr("id") != Some(id.as_str())
            || d.select(&selector(&format!("{}[id=\"{id}\"]", n.value().name())))
                .count()
                != 1
        {
            return Err(Fault::error("cism_fields_unqualified"));
        }
        if kind == "class_mode"
            && n.select(&selector("option"))
                .map(|n| n.value().attr("value").unwrap_or(""))
                .collect::<Vec<_>>()
                .join(",")
                != "0,3,2,1"
        {
            return Err(Fault::error("cism_fields_unqualified"));
        }
        let source = obj(&[
            ("fieldName", kind.into()),
            ("studentKey", c[3].into()),
            ("membershipKey", c[4].into()),
            ("groupKey", c[5].into()),
        ]);
        let student = if kind == "class_mode" {
            None
        } else {
            n.ancestors()
                .filter_map(ElementRef::wrap)
                .find(|n| n.value().name() == "tr")
                .and_then(|tr| tr.select(&selector("td,th")).next())
                .map(|n| trimmed(&content(n)))
        };
        fields.push(obj(&[
            (
                "fieldRef",
                digest(&Js::Arr(vec![kind.into(), source.clone()]), Family::Cism)?.into(),
            ),
            ("kind", kind.into()),
            (
                "label",
                match kind {
                    "class_mode" => "이번주 반 수업 분위기",
                    "think_note" => "풀이노트작성",
                    _ => "주간테스트",
                }
                .into(),
            ),
            ("value", node_value(n).into()),
            ("studentName", student.map(Js::from).unwrap_or(Js::Null)),
            (
                "groupName",
                groups
                    .get(&c[5])
                    .map(|s| s.clone().into())
                    .unwrap_or(Js::Null),
            ),
            ("source", source),
        ]));
    }
    let refs: Vec<_> = fields.iter().map(|f| f.get("fieldRef").clone()).collect();
    if fields.len() < 8 || fields.len() > 200 || unique(&refs).len() != refs.len() {
        return Err(Fault::error("cism_fields_unqualified"));
    }
    let period = obj(&[
        ("periodStart", period.period_start.into()),
        ("periodEnd", period.period_end.into()),
    ]);
    let fields = Js::Arr(fields);
    let report_digest = digest(
        &Js::Arr(vec![
            period.clone(),
            teacher.get("teacherKey").clone(),
            WIRE_SHA.into(),
            fields.clone(),
        ]),
        Family::Cism,
    )?;
    Ok(obj(&[
        ("date", date.into()),
        ("periodStart", period.get("periodStart").clone()),
        ("periodEnd", period.get("periodEnd").clone()),
        ("weekLabel", week_label.into()),
        ("teacherLabel", teacher.get("teacherLabel").clone()),
        ("teacherKey", teacher.get("teacherKey").clone()),
        ("wireSha256", WIRE_SHA.into()),
        ("reportSha256", report_digest.into()),
        ("observedAt", now.into()),
        ("fields", fields),
        ("sourceMode", "live_source".into()),
        ("pdfGenerated", false.into()),
    ]))
}
