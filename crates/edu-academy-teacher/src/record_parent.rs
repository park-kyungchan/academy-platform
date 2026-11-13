//! Parent narratives for resolved new authoring entries, never source rewrites.
use crate::record_data::*;
use edu_academy_core::js::{Fault, Js, Result};

fn invalid() -> Fault {
    Fault::error("record_parent_mapping_unverified")
}
fn line_text(value: &str) -> Result<&str> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(value)
}
fn text(value: &Js) -> Result<String> {
    if !value.string() {
        return Err(invalid());
    }
    let value = String::from_utf16(value.units()).map_err(|_| invalid())?;
    line_text(&value)?;
    Ok(value)
}
fn lookup<'a>(values: &'a Js, key: &str, reference: &str) -> Result<&'a Js> {
    let mut matches = values
        .array()
        .iter()
        .filter(|value| value.get(key).is(reference));
    let value = matches.next().ok_or_else(invalid)?;
    if matches.next().is_some() {
        return Err(invalid());
    }
    Ok(value)
}
struct Line<'a> {
    entry: &'a RecordEntry,
    book: String,
    sections: Vec<String>,
    qualifiers: Vec<String>,
}
impl Line<'_> {
    fn compatible(&self, other: &Self) -> bool {
        // Counts and exclusions stay on their original entry. In particular,
        // two identical cardinal counts must never disappear through grouping.
        self.entry.scope == EntryScope::Sections
            && other.entry.scope == EntryScope::Sections
            && self.entry.field == other.entry.field
            && self.entry.book_ref == other.entry.book_ref
            && self.entry.unit_ref == other.entry.unit_ref
            && self.entry.activity == other.entry.activity
            && self.entry.deadline == other.entry.deadline
            && self.qualifiers == other.qualifiers
            && self.qualifiers.iter().all(|q| q == "풀이")
            && other
                .sections
                .iter()
                .all(|section| !self.sections.contains(section))
    }
    fn render(&self) -> String {
        let mut value = format!(
            "- {}, {} {} {}",
            self.book, self.entry.unit_code, self.entry.minor_title, self.entry.activity
        );
        for word in self.sections.iter().chain(&self.qualifiers) {
            value.push(' ');
            value.push_str(word);
        }
        value
    }
}

/// Uses the same owned book/unit/activity/section projection as `parse_lines`.
/// A catalog label is its qualified parent book text; it is never inferred from
/// a historical record, brand shorthand or raw teacher prose. None means that
/// this renderer proposes no new value for that field, including known blanks.
pub fn render_parent(
    entries: &[RecordEntry],
    catalog: &Js,
) -> Result<RecordFields<Option<String>>> {
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut student = None;
    for entry in entries {
        if !matches!(entry.field, RecordField::Progress | RecordField::Homework) {
            return Err(Fault::error("record_parent_narrative_field_required"));
        }
        let book = lookup(catalog.get("books"), "bookRef", &entry.book_ref)?;
        if !book.get("bindingState").is("confirmed") {
            return Err(invalid());
        }
        let scope = text(book.get("studentRef"))?;
        if student.as_ref().is_some_and(|prior| prior != &scope) {
            return Err(invalid());
        }
        student = Some(scope);
        let label = text(book.get("label"))?;
        if lines
            .iter()
            .any(|line| line.book == label && line.entry.book_ref != entry.book_ref)
        {
            return Err(Fault::error("record_parent_book_label_ambiguous"));
        }
        let unit = lookup(book.get("units"), "unitRef", &entry.unit_ref)?;
        if !unit.get("code").is(&entry.unit_code)
            || !unit.get("title").is(&entry.minor_title)
            || !(unit.get("kind").is("minor") || unit.get("kind").is("chapter_assessment"))
            || !unit
                .get("activities")
                .array()
                .iter()
                .any(|a| a.is(&entry.activity))
        {
            return Err(invalid());
        }
        for word in [&entry.unit_code, &entry.minor_title, &entry.activity] {
            line_text(word)?;
        }
        if (entry.scope == EntryScope::Sections) == entry.section_refs.is_empty() {
            return Err(invalid());
        }
        let mut sections = Vec::new();
        for reference in &entry.section_refs {
            let section = lookup(unit.get("sections"), "sectionRef", reference)?;
            sections.push(text(section.get("label"))?);
        }
        let deadline_word = entry.deadline.as_ref().map(|date| format!("{date}까지"));
        if let Some(date) = &entry.deadline {
            line_text(date)?;
        }
        let mut qualifiers = Vec::new();
        for word in &entry.qualifiers {
            line_text(word)?;
            // The deadline is shown once above its exact group. 교체 is the
            // supplied field operation shown in the preview disposition, not
            // an assertion about the student's learning in a parent narrative.
            if deadline_word.as_ref() != Some(word) && word != "교체" && !sections.contains(word)
            {
                qualifiers.push(word.clone());
            }
        }
        let line = Line {
            entry,
            book: label,
            sections,
            qualifiers,
        };
        if let Some(group) = lines.iter_mut().find(|group| group.compatible(&line)) {
            group.sections.extend(line.sections);
        } else {
            lines.push(line);
        }
    }
    let mut values = RecordFields::from_fn(|_| None);
    for field in [RecordField::Progress, RecordField::Homework] {
        let selected: Vec<_> = lines
            .iter()
            .filter(|line| line.entry.field == field)
            .collect();
        let mut dates = vec![None];
        for line in &selected {
            let date = line.entry.deadline.as_deref();
            if !dates.contains(&date) {
                dates.push(date);
            }
        }
        let mut output = Vec::new();
        for date in dates {
            let group: Vec<_> = selected
                .iter()
                .filter(|line| line.entry.deadline.as_deref() == date)
                .collect();
            if group.is_empty() {
                continue;
            }
            if let Some(date) = date {
                output.push(format!("- {date}까지"));
            }
            output.extend(group.iter().map(|line| line.render()));
        }
        if !output.is_empty() {
            values[field] = Some(output.join("\n"));
        }
    }
    Ok(values)
}
