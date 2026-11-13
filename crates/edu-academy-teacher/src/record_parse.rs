//! New teacher lines only; never reconstruct an edition from historical prose.
use crate::record_data::*;
use edu_academy_core::js::{Fault, Js, Result};
use regex::Regex;
use std::num::NonZeroU64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedLines {
    Ready(Vec<RecordEntry>),
    Ask(Vec<PendingQuestion>),
}

fn text(value: &Js) -> Result<String> {
    if !value.string() || value.units().is_empty() {
        return Err(Fault::error("record_catalog_text_required"));
    }
    String::from_utf16(value.units()).map_err(|_| Fault::error("record_catalog_text_invalid"))
}
fn rest<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    input
        .strip_prefix(prefix)
        .filter(|tail| tail.is_empty() || tail.starts_with(|c: char| c.is_whitespace() || c == ','))
}
fn trim(input: &str) -> &str {
    input.trim_start_matches(|c: char| c.is_whitespace() || c == ',')
}
fn book_names(book: &Js) -> Result<Vec<String>> {
    let label = text(book.get("label"))?;
    let mut names = vec![label.clone()];
    if !book.get("edition").absent() {
        names.push(format!("{label} {}", text(book.get("edition"))?));
    }
    Ok(names)
}
fn question(
    field: AnswerField,
    label: &str,
    reason: &str,
    words: &str,
    choices: Vec<QuestionChoice>,
) -> PendingQuestion {
    PendingQuestion {
        field,
        reason_code: reason.into(),
        text: format!(
            "{label}의 “{words}”는 어떤 뜻으로 기록할까요? 해당 항목의 문장을 그대로 답해 주세요."
        ),
        choices,
        // No draft is accepted by this lexical API. The draft owner must bind
        // these unbound questions to its current revision before presenting them.
        revision: NonZeroU64::MIN,
    }
}
fn choice(lines: &[String], index: usize, replacement: String) -> QuestionChoice {
    let mut answer = lines.to_vec();
    answer[index] = replacement.clone();
    QuestionChoice {
        label: replacement,
        answer: AnswerValue::Lines(answer),
    }
}

/// `catalog` is an owned, single-student projection: books contain studentRef,
/// bookRef, label, edition (nullable), bindingState and units. Units contain
/// unitRef, code, title, kind, activities (literal strings) and sections
/// (sectionRef/label). Activities come from the owning edition tree, not prose.
/// This function borrows only that data and grants no approval/effect authority.
pub fn parse_lines(field: RecordField, lines: &[String], catalog: &Js) -> Result<ParsedLines> {
    let (answer_field, label) = match field {
        RecordField::Progress => (AnswerField::Progress, "진도"),
        RecordField::Homework => (AnswerField::Homework, "숙제"),
        _ => return Err(Fault::error("record_parse_narrative_field_required")),
    };
    let Js::Arr(books) = catalog.get("books") else {
        return Err(Fault::error("record_catalog_books_required"));
    };
    let mut student = None;
    for book in books {
        let scope = text(book.get("studentRef"))?;
        if student.as_ref().is_some_and(|previous| previous != &scope) {
            return Err(Fault::error("record_catalog_student_scope_mismatch"));
        }
        student = Some(scope);
        text(book.get("bookRef"))?;
        book_names(book)?;
    }
    // Lexical spans only: no count conversion, numeric bound, date inference or
    // semantic validation. Core Service::review remains the semantic boundary.
    let qualifier = Regex::new(concat!(
        r"^(?:(?:PART\s+[^\s,]+|[^\s,]+)\s+제외|",
        r"(?:[0-9]+\s*문항)|(?:[0-9]+\s*유형)|(?:유형\s*[0-9]+)|",
        r"(?:solving|풀이|풀었고)|",
        r"(?P<date>(?:[0-9]+[-./][0-9]+[-./][0-9]+|[0-9]+\s*월\s*[0-9]+\s*일))까지)"
    ))
    .map_err(|_| Fault::error("record_parse_lexical_rule_invalid"))?;
    let mut entries = Vec::new();
    let mut questions = Vec::new();
    for (index, raw) in lines.iter().enumerate() {
        let input = raw.trim().strip_prefix("- ").unwrap_or(raw.trim());
        let mut candidates = Vec::new();
        for book in books {
            for name in book_names(book)? {
                if let Some(tail) = rest(input, &name) {
                    candidates.push((book, name, trim(tail)));
                }
            }
        }
        let longest = candidates.iter().map(|(_, name, _)| name.len()).max();
        candidates.retain(|(_, name, _)| Some(name.len()) == longest);
        if candidates.len() != 1 || !candidates[0].0.get("bindingState").is("confirmed") {
            let mut choices = Vec::new();
            for (book, _, tail) in &candidates {
                let name = book_names(book)?.pop().expect("label exists");
                // Only offer replacements that uniquely name a confirmed book.
                let unique = books
                    .iter()
                    .filter(|b| book_names(b).is_ok_and(|names| names.contains(&name)))
                    .count()
                    == 1;
                if unique && book.get("bindingState").is("confirmed") {
                    choices.push(choice(lines, index, format!("{name}, {tail}")));
                }
            }
            questions.push(question(
                answer_field,
                label,
                "record_book_mapping_required",
                raw,
                choices,
            ));
            continue;
        }
        let (book, book_name, tail) = &candidates[0];
        let Js::Arr(units) = book.get("units") else {
            return Err(Fault::error("record_catalog_units_required"));
        };
        let mut matches = Vec::new();
        for unit in units {
            let code = text(unit.get("code"))?;
            let title = text(unit.get("title"))?;
            let full = format!("{code} {title}");
            for name in [full, code, title] {
                if let Some(after) = rest(tail, &name) {
                    matches.push((unit, name, trim(after)));
                }
            }
        }
        let longest = matches.iter().map(|(_, name, _)| name.len()).max();
        matches.retain(|(_, name, _)| Some(name.len()) == longest);
        if matches.len() != 1 || matches[0].0.get("kind").is("unknown") {
            let choices = matches
                .iter()
                .map(|(unit, _, after)| {
                    Ok(choice(
                        lines,
                        index,
                        format!(
                            "{book_name}, {} {} {after}",
                            text(unit.get("code"))?,
                            text(unit.get("title"))?
                        ),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            questions.push(question(
                answer_field,
                label,
                "record_unit_mapping_required",
                raw,
                choices,
            ));
            continue;
        }
        let (unit, _, mut remaining) = matches[0].clone();
        let Js::Arr(activities) = unit.get("activities") else {
            return Err(Fault::error("record_catalog_activities_required"));
        };
        let mut activity_matches = Vec::new();
        for value in activities {
            let activity = text(value)?;
            if let Some(after) = rest(remaining, &activity) {
                activity_matches.push((activity, after, false));
            } else if activity == "유형다지기"
                && let Some(after) = rest(remaining, "유형다지기로 바꿔")
            {
                activity_matches.push((activity, after, true));
            }
        }
        let longest = activity_matches.iter().map(|(a, _, _)| a.len()).max();
        activity_matches.retain(|(a, _, _)| Some(a.len()) == longest);
        if activity_matches.len() != 1 {
            questions.push(question(
                answer_field,
                label,
                "record_activity_mapping_required",
                remaining,
                vec![],
            ));
            continue;
        }
        let (activity, after, replace) = activity_matches.remove(0);
        remaining = trim(after);
        let mut qualifiers = if replace {
            vec!["교체".into()]
        } else {
            Vec::new()
        };
        let mut section_refs = Vec::new();
        let mut deadline = None;
        while !remaining.is_empty() {
            if let Some(capture) = qualifier.captures(remaining) {
                let span = capture.get(0).expect("complete match").as_str();
                if let Some(after) = rest(remaining, span) {
                    if let Some(date) = capture.name("date") {
                        if deadline
                            .as_deref()
                            .is_some_and(|prior| prior != date.as_str())
                        {
                            break;
                        }
                        deadline = Some(date.as_str().to_owned());
                    }
                    qualifiers.push(match span {
                        "solving" | "풀었고" => "풀이".into(),
                        _ => span.into(),
                    });
                    remaining = trim(after);
                    continue;
                }
            }
            let mut sections = Vec::new();
            for section in unit.get("sections").array() {
                let name = text(section.get("label"))?;
                if let Some(after) = rest(remaining, &name) {
                    sections.push((name, text(section.get("sectionRef"))?, after));
                }
            }
            let longest = sections.iter().map(|(name, _, _)| name.len()).max();
            sections.retain(|(name, _, _)| Some(name.len()) == longest);
            if sections.len() != 1 {
                break;
            }
            let (name, reference, after) = sections.remove(0);
            qualifiers.push(name);
            section_refs.push(reference);
            remaining = trim(after);
        }
        if !remaining.is_empty() {
            questions.push(question(
                answer_field,
                label,
                "record_teacher_words_unmapped",
                remaining,
                vec![],
            ));
            continue;
        }
        entries.push(RecordEntry {
            field,
            raw_line: raw.clone(),
            book_ref: text(book.get("bookRef"))?,
            unit_ref: text(unit.get("unitRef"))?,
            unit_code: text(unit.get("code"))?,
            minor_title: text(unit.get("title"))?,
            activity,
            qualifiers,
            scope: if section_refs.is_empty() {
                EntryScope::Unit
            } else {
                EntryScope::Sections
            },
            section_refs,
            deadline,
        });
    }
    Ok(if questions.is_empty() {
        ParsedLines::Ready(entries)
    } else {
        ParsedLines::Ask(questions)
    })
}
