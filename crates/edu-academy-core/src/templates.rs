//! Direct port of lessonJournalTemplates.ts. Labels/revisions remain source-owned.
use crate::{js::*, knowledge};
pub fn from_json(v: serde_json::Value) -> Js {
    match v {
        serde_json::Value::Null => Js::Null,
        serde_json::Value::Bool(b) => b.into(),
        serde_json::Value::Number(n) => Js::Num(n.as_f64().expect("finite number")),
        serde_json::Value::String(s) => s.into(),
        serde_json::Value::Array(a) => Js::Arr(a.into_iter().map(from_json).collect()),
        serde_json::Value::Object(p) => Js::Obj(
            p.into_iter()
                .map(|(k, v)| (text(&k), from_json(v)))
                .collect(),
        ),
    }
}
pub fn constants() -> Js {
    from_json(serde_json::from_str(include_str!("constants.json")).expect("source constants"))
}
fn refs(v: &Js) -> Result<()> {
    if !matches!(v, Js::Arr(_))
        || v.array().is_empty()
        || v.array()
            .iter()
            .any(|x| !x.string() || trim(x.units()).is_empty())
        || unique(v.array()).len() != v.array().len()
    {
        Err(Fault::error("journal_evidence_required"))
    } else {
        Ok(())
    }
}
fn safe_positive(v: &Js) -> bool {
    matches!(v,Js::Num(n) if n.is_finite()&&n.fract()==0.0&&*n>=1.0&&*n<=9007199254740991.0)
}
fn content(v: &Js) -> Result<()> {
    exact(
        v,
        &["bookLabel", "unitCode", "unitName", "basis", "evidenceRefs"],
    )?;
    refs(v.get("evidenceRefs"))?;
    require_text(v.get("unitName"), "unit name")?;
    if !v.get("bookLabel").string()
        || !v.get("unitCode").string()
        || !["source_edition", "teacher_report", "teacher_report_topic"]
            .iter()
            .any(|b| v.get("basis").is(b))
    {
        return Err(Fault::error("journal_mapping_rejected"));
    }
    if !v.get("basis").is("teacher_report_topic") {
        require_text(v.get("bookLabel"), "book")?;
        require_text(v.get("unitCode"), "literal unit code")?;
    }
    Ok(())
}
fn nonempty_join(parts: &[Text], sep: &str) -> Text {
    join(
        &parts
            .iter()
            .filter(|p| !p.is_empty())
            .cloned()
            .collect::<Vec<_>>(),
        sep,
    )
}
fn mapped(v: &Js) -> Result<Text> {
    if !matches!(v, Js::Arr(_)) || v.array().is_empty() || v.array().len() > 16 {
        return Err(Fault::error("journal_content_mapping_required"));
    }
    let mut books: Vec<(Text, Vec<Text>)> = vec![];
    for m in v.array() {
        content(m)?;
        let book = m.get("bookLabel").units().to_vec();
        let unit = nonempty_join(
            &[
                m.get("unitCode").units().to_vec(),
                m.get("unitName").units().to_vec(),
            ],
            " ",
        );
        if let Some((_, units)) = books.iter_mut().find(|(b, _)| b == &book) {
            if !units.contains(&unit) {
                units.push(unit);
            }
        } else {
            books.push((book, vec![unit]));
        }
    }
    Ok(join(
        &books
            .into_iter()
            .map(|(book, units)| nonempty_join(&[book, join(&units, "·")], " "))
            .collect::<Vec<_>>(),
        " / ",
    ))
}
fn bullet(entries: &[Text]) -> Text {
    join(
        &entries
            .iter()
            .map(|s| {
                let mut u = trim(s);
                while u.len() > 1 && u[0] == 45 && (u[1] == 32 || u[1] == 9) {
                    let mut i = 1;
                    while i < u.len() && (u[i] == 32 || u[i] == 9) {
                        i += 1;
                    }
                    u = &u[i..];
                }
                let mut b = text("- ");
                b.extend(u);
                b
            })
            .collect::<Vec<_>>(),
        "\n",
    )
}
fn collect_refs(entry: &Js, out: &mut Vec<Js>) {
    out.extend(entry.get("evidenceRefs").array().iter().cloned());
    for m in entry.get("content").array() {
        out.extend(m.get("evidenceRefs").array().iter().cloned());
    }
}
pub fn parent_internal(v: &Js) -> bool {
    let folded = Js::Str(
        v.units()
            .iter()
            .map(|u| if (65..=90).contains(u) { u + 32 } else { *u })
            .collect(),
    );
    test(
        r"(?:강사\s*확인|교사\s*확인|교사\s*검토|교사\s*(?:최종\s*)?완료\s*판정|채점|미실시|미완료|시간\s*부족|확인\s*전|검사[^\r\n\u{2028}\u{2029}]{0,8}미완료|풀이와\s*채점|teacher\s*(?:inspection|review)|ungraded|not\s*completed|insufficient\s*time)",
        &folded,
    )
}
pub fn render(input: &Js) -> Result<Js> {
    exact(
        input,
        &["lessonDate", "part", "progress", "homework", "memo"],
    )?;
    knowledge::calendar_date(input.get("lessonDate"))?;
    if !safe_positive(input.get("part"))
        || !matches!(input.get("progress"), Js::Arr(_))
        || input.get("progress").array().len() > 32
        || !matches!(input.get("memo"), Js::Arr(_))
        || input.get("memo").array().len() > 16
    {
        return Err(Fault::error("journal_template_scope_rejected"));
    }
    let labels = constants();
    let mut progress = vec![];
    for entry in input.get("progress").array() {
        exact(
            entry,
            &[
                "content",
                "activity",
                "outcome",
                "detail",
                "evidenceRefs",
                "internalCode",
            ],
        )?;
        refs(entry.get("evidenceRefs"))?;
        let activity = entry.get("activity");
        let outcome = entry.get("outcome");
        if !activity.string()
            || !outcome.string()
            || !labels.get("activities").has(&activity.code_units())
            || !labels.get("outcomes").has(&outcome.code_units())
            || !entry.get("detail").string()
            || !entry.get("internalCode").string()
        {
            return Err(Fault::error("journal_activity_rejected"));
        }
        let mut line = mapped(entry.get("content"))?;
        line.extend(text(" "));
        line.extend(labels.get("activities").get(&activity.code_units()).units());
        line.extend(text(" "));
        line.extend(labels.get("outcomes").get(&outcome.code_units()).units());
        if entry.get("detail").truthy() {
            line.extend(text("("));
            line.extend(entry.get("detail").units());
            line.extend(text(")"));
        }
        line.extend(text("."));
        progress.push(line);
    }
    let hw = input.get("homework");
    exact(hw, &["dueDate", "tasks", "qualification"])?;
    knowledge::calendar_date(hw.get("dueDate"))?;
    if hw.get("dueDate").units() < input.get("lessonDate").units()
        || !matches!(hw.get("tasks"), Js::Arr(_))
        || hw.get("tasks").array().len() > 16
        || !["verified", "unresolved_header"]
            .iter()
            .any(|s| hw.get("qualification").is(s))
    {
        return Err(Fault::error("journal_homework_rejected"));
    }
    let date = hw.get("dueDate").code_units();
    let weekdays = ["일", "월", "화", "수", "목", "금", "토"];
    let due = format!(
        "{}/{}({})까지",
        &date[5..7],
        &date[8..],
        weekdays[knowledge::weekday(&date)]
    );
    let mut tasks = vec![];
    for (i, task) in hw.get("tasks").array().iter().enumerate() {
        exact(task, &["content", "action", "selection", "evidenceRefs"])?;
        refs(task.get("evidenceRefs"))?;
        let action = task.get("action");
        if !action.string()
            || !labels.get("homeworkActions").has(&action.code_units())
            || !task.get("selection").string()
        {
            return Err(Fault::error("journal_homework_action_rejected"));
        }
        let mut line = if i == 0 {
            text(&(due.clone() + " "))
        } else {
            vec![]
        };
        line.extend(mapped(task.get("content"))?);
        if task.get("selection").truthy() {
            line.extend(text(" "));
            line.extend(task.get("selection").units());
        }
        line.extend(text(" "));
        line.extend(
            labels
                .get("homeworkActions")
                .get(&action.code_units())
                .units(),
        );
        line.extend(text("."));
        tasks.push(line);
    }
    let mut notices = vec![];
    for notice in input.get("memo").array() {
        exact(notice, &["text", "parts", "evidenceRefs"])?;
        refs(notice.get("evidenceRefs"))?;
        require_text(notice.get("text"), "memo")?;
        let parts = notice.get("parts");
        if !matches!(parts, Js::Arr(_))
            || parts.array().is_empty()
            || parts.array().iter().any(|p| !safe_positive(p))
        {
            return Err(Fault::error("journal_memo_scope_rejected"));
        }
        if parts.array().contains(input.get("part")) {
            notices.push(notice.clone());
        }
    }
    let progress = Js::Str(bullet(&progress));
    let homework = Js::Str(bullet(&tasks));
    let memo = Js::Str(bullet(
        &notices
            .iter()
            .map(|n| n.get("text").units().to_vec())
            .collect::<Vec<_>>(),
    ));
    if [&progress, &homework, &memo]
        .iter()
        .any(|v| test("P[123]", v))
    {
        return Err(Fault::error("internal_code_in_parent_text"));
    }
    let fields = [
        ("progress", &progress),
        ("homework", &homework),
        ("memo", &memo),
    ];
    let mut blockers = vec![];
    for (k, v) in fields {
        if parent_internal(v) {
            blockers.push(format!("{k}:parent_internal_language_excluded").into());
        }
    }
    if hw.get("qualification").is("unresolved_header") {
        blockers.push("homework:unresolved_original_header".into());
    }
    for (k, v) in fields {
        if crate::leaf::journal_wire_width(v.units()).expect("bounded string") > 400 {
            blockers.push(format!("{k}:source_width_exceeded").into());
        }
    }
    let mut evidence = vec![];
    for e in input.get("progress").array() {
        collect_refs(e, &mut evidence);
    }
    for e in hw.get("tasks").array() {
        collect_refs(e, &mut evidence);
    }
    for n in notices {
        evidence.extend(n.get("evidenceRefs").array().iter().cloned());
    }
    Ok(obj(&[
        (
            "templateRevision",
            labels.get("templates").get("revision").clone(),
        ),
        ("progress", progress),
        ("homework", homework),
        ("memo", memo),
        ("blockers", Js::Arr(blockers)),
        ("evidenceRefs", Js::Arr(unique(&evidence))),
        ("approval", "not_granted_by_rendering".into()),
    ]))
}
