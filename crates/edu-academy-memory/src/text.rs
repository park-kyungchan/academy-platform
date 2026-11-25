//! Bounded text: sections with a byte budget, entries with a byte bound.
//!
//! A section's budget covers its leading separator, its heading, every entry
//! line and its last line. An entry's bound covers its whole line, newline
//! included. A text over its bound is cut at a character boundary and ends
//! "… (+<n> bytes)", n being the bytes left out of the text. An entry that does
//! not fit what is left of its section is left out with every entry after it,
//! and the section's last line is "(+<k> more)".

/// One line of a section: a fixed prefix and a text that may be cut.
pub struct Entry {
    pub prefix: String,
    pub text: String,
}

impl Entry {
    pub fn new(prefix: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            prefix: flat(&prefix.into()),
            text: flat(&text.into()),
        }
    }
}

pub struct Section<'a> {
    pub heading: &'a str,
    pub budget: usize,
    /// The bound of each entry's line; `None` bounds it by what is left only.
    pub bound: Option<usize>,
    /// Whether a blank line separates the section from the one before it.
    pub separated: bool,
}

/// Control characters become spaces, so every entry is one line and the byte
/// count of a text stays that of the stored text.
pub fn flat(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn floor_boundary(text: &str, mut at: usize) -> usize {
    if at >= text.len() {
        return text.len();
    }
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn marker(left_out: usize) -> String {
    format!("… (+{left_out} bytes)")
}

fn more(count: usize) -> String {
    format!("(+{count} more)\n")
}

/// The entry's line within `limit` bytes, newline included, or `None` when
/// not even its prefix and the cut marker fit.
pub fn line(entry: &Entry, limit: usize) -> Option<String> {
    let whole = entry.prefix.len() + entry.text.len() + 1;
    if whole <= limit {
        return Some(format!("{}{}\n", entry.prefix, entry.text));
    }
    // The marker for the whole text is the longest the cut can need.
    let room = limit.checked_sub(entry.prefix.len() + marker(entry.text.len()).len() + 1)?;
    let cut = floor_boundary(&entry.text, room);
    let line = format!(
        "{}{}{}\n",
        entry.prefix,
        &entry.text[..cut],
        marker(entry.text.len() - cut)
    );
    debug_assert!(line.len() <= limit);
    Some(line)
}

/// The section within its budget, whatever its entries hold.
pub fn render(section: &Section, entries: &[Entry]) -> String {
    let mut out = String::new();
    if section.separated {
        out.push('\n');
    }
    out.push_str(section.heading);
    out.push('\n');
    if entries.is_empty() {
        out.push_str("none\n");
        return bounded(out, section.budget);
    }
    for (index, entry) in entries.iter().enumerate() {
        let after = entries.len() - index - 1;
        let reserve = if after > 0 { more(after).len() } else { 0 };
        let left = section.budget.saturating_sub(out.len() + reserve);
        let limit = section.bound.map_or(left, |bound| bound.min(left));
        let fitted = match section.bound {
            // An entry is cut by its own bound; what is left only decides
            // whether its line goes in.
            Some(bound) => line(entry, bound).filter(|l| l.len() <= left),
            None => line(entry, limit),
        };
        match fitted {
            Some(text) => out.push_str(&text),
            None => {
                out.push_str(&more(entries.len() - index));
                break;
            }
        }
    }
    bounded(out, section.budget)
}

/// A last guard: the rules above keep a section within its budget already.
fn bounded(mut text: String, budget: usize) -> String {
    if text.len() > budget {
        let cut = floor_boundary(&text, budget);
        text.truncate(cut);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_line_stays_whole() {
        let entry = Entry::new("- a: ", "short");
        assert_eq!(line(&entry, 100).unwrap(), "- a: short\n");
    }

    #[test]
    fn a_long_text_is_cut_at_a_character_boundary_with_its_count() {
        let text = "가".repeat(100); // 300 bytes
        let entry = Entry::new("- a: ", text.clone());
        let got = line(&entry, 60).unwrap();
        assert!(got.len() <= 60, "{got}");
        let body = got.strip_prefix("- a: ").unwrap();
        let (kept, rest) = body.split_once("… (+").unwrap();
        let count: usize = rest.trim_end_matches(" bytes)\n").parse().unwrap();
        assert_eq!(kept.len() + count, text.len());
        assert!(text.starts_with(kept));
    }

    #[test]
    fn control_characters_become_spaces() {
        let entry = Entry::new("- ", "a\nb\tc");
        assert_eq!(entry.text, "a b c");
    }

    #[test]
    fn entries_past_the_budget_are_counted_on_the_last_line() {
        let entries: Vec<Entry> = (0..30)
            .map(|i| Entry::new(format!("- {i}: "), "x".repeat(500)))
            .collect();
        let section = Section {
            heading: "## test",
            budget: 400,
            bound: Some(110),
            separated: true,
        };
        let out = render(&section, &entries);
        assert!(out.len() <= 400, "{}", out.len());
        let last = out.lines().last().unwrap();
        let shown = out.lines().filter(|l| l.starts_with("- ")).count();
        assert_eq!(last, format!("(+{} more)", 30 - shown));
        assert!(
            out.lines()
                .filter(|l| l.starts_with("- "))
                .all(|l| l.len() < 110)
        );
    }

    #[test]
    fn an_empty_section_says_none() {
        let section = Section {
            heading: "## test",
            budget: 300,
            bound: Some(120),
            separated: false,
        };
        assert_eq!(render(&section, &[]), "## test\nnone\n");
    }

    #[test]
    fn an_unbounded_entry_is_cut_to_what_is_left() {
        let section = Section {
            heading: "## test",
            budget: 200,
            bound: None,
            separated: false,
        };
        let entries = [
            Entry::new("slug: ", "a"),
            Entry::new("surface: ", "y".repeat(2000)),
        ];
        let out = render(&section, &entries);
        assert!(out.len() <= 200);
        assert!(out.contains("surface: yyy"));
        assert!(out.trim_end().ends_with("bytes)"));
    }
}
