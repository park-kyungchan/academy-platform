//! `import <directory> [--dry-run]`: the memory a teacher's sessions wrote as
//! Markdown becomes store objects, through the same writes the other
//! subcommands make. The directory is only read. See README.md.
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::engine::{Engine, Got};
use crate::{
    Binding, Domain, Refusal, Registered, Reminder, Write, bind_write, check_slug, check_utc,
    close_one, close_write, is_date, note_write, open_write, student_write, surface_write,
    unbind_write, utc_now,
};

/// The most a note's text holds, in bytes.
const NOTE_MAX: usize = 8000;
const NOTE_CUT: &str = "\n(cut)";
/// The most a student's surface holds, in bytes.
const SURFACE_MAX: usize = 2000;
/// A cell that says "none" in a table of ids.
const ABSENT: [&str; 5] = ["", "-", "\u{2014}", "\u{2013}", "n/a"];

// ---------------------------------------------------------------- the plan

/// One entry of the report's `parse`: a row or a section that was not read.
struct Parse {
    path: String,
    line: usize,
    reason: String,
}

/// One step of the plan, applied in order.
enum Item {
    /// One write on the object of `ty`.
    Single { write: Write, ty: &'static str },
    /// Two writes that end in `status`: the object is read first, and when it
    /// already holds that state with the same values, neither is written.
    Pair {
        first: Write,
        second: Write,
        ty: &'static str,
        status: &'static str,
    },
    /// `close_lesson_day` of the student with the date.
    DayClose { slug: String, date: String },
}

impl Item {
    fn actions(&self) -> Vec<&'static str> {
        match self {
            Item::Single { write, .. } => vec![write.action],
            Item::Pair { first, second, .. } => vec![first.action, second.action],
            Item::DayClose { .. } => vec!["close_lesson_day"],
        }
    }
}

struct StudentPlan {
    slug: String,
    items: Vec<Item>,
    staging_lines: usize,
}

struct Plan {
    class: Vec<Item>,
    students: Vec<StudentPlan>,
    parse: Vec<Parse>,
}

// ---------------------------------------------------------------- the command

pub fn run(engine: &Engine, domain: &Domain, args: &[String]) -> Result<i32, Refusal> {
    let mut dry = false;
    let mut directory = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--dry-run" if !dry => dry = true,
            "--dry-run" => return Err(Refusal::usage("--dry-run", "--dry-run is given twice")),
            other if other.starts_with("--") => {
                return Err(Refusal::usage(other, &format!("unknown option {other}")));
            }
            other => directory.push(other),
        }
    }
    let [directory] = directory.as_slice() else {
        return Err(Refusal::usage("", "import <directory> [--dry-run]"));
    };
    let plan = build(engine, domain, Path::new(directory))?;
    let parse: Vec<Value> = plan
        .parse
        .iter()
        .map(|p| json!({"path": p.path, "line": p.line, "reason": p.reason}))
        .collect();
    let report = if dry {
        dry_report(&plan, parse)
    } else {
        real_report(engine, &plan, parse)?
    };
    println!("{}", report.0);
    Ok(report.1)
}

fn planned(items: &[Item]) -> Value {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for item in items {
        for action in item.actions() {
            *counts.entry(action).or_default() += 1;
        }
    }
    json!(counts)
}

fn dry_report(plan: &Plan, parse: Vec<Value>) -> (Value, i32) {
    let mut students = Map::new();
    for s in &plan.students {
        students.insert(
            s.slug.clone(),
            json!({"planned": planned(&s.items), "staging_lines": s.staging_lines}),
        );
    }
    let code = i32::from(!parse.is_empty());
    (
        json!({
            "students": students,
            "class_reminders": {"planned": planned(&plan.class)},
            "parse": parse,
        }),
        code,
    )
}

#[derive(Default)]
struct Counts {
    created: u64,
    already_applied: u64,
    refused: u64,
}

impl Counts {
    fn json(&self) -> Map<String, Value> {
        let Value::Object(map) = json!({
            "created": self.created,
            "already_applied": self.already_applied,
            "refused": self.refused,
        }) else {
            unreachable!()
        };
        map
    }
}

fn real_report(engine: &Engine, plan: &Plan, parse: Vec<Value>) -> Result<(Value, i32), Refusal> {
    let now = utc_now();
    let mut refused = 0;
    let mut class = Counts::default();
    for item in &plan.class {
        apply(engine, item, &now, "class", &mut class)?;
    }
    refused += class.refused;
    let mut students = Map::new();
    for s in &plan.students {
        let mut counts = Counts::default();
        for item in &s.items {
            apply(engine, item, &now, &s.slug, &mut counts)?;
        }
        refused += counts.refused;
        let mut entry = counts.json();
        entry.insert("staging_lines".to_owned(), json!(s.staging_lines));
        students.insert(s.slug.clone(), Value::Object(entry));
    }
    let code = i32::from(!parse.is_empty() || refused > 0);
    Ok((
        json!({
            "students": students,
            "class_reminders": class.json(),
            "parse": parse,
        }),
        code,
    ))
}

/// Whether a write is said to have changed the store.
fn changed(outcome: &Value) -> bool {
    outcome.get("outcome").and_then(Value::as_str) != Some("already_applied")
}

fn apply(
    engine: &Engine,
    item: &Item,
    now: &str,
    owner: &str,
    counts: &mut Counts,
) -> Result<(), Refusal> {
    match settle(engine, item, now) {
        Ok(true) => counts.created += 1,
        Ok(false) => counts.already_applied += 1,
        // Without the engine nothing more can be written.
        Err(refusal) if refusal.exit == 3 => return Err(refusal),
        Err(refusal) => {
            counts.refused += 1;
            // The refusal's detail may hold the text it refused: only its code.
            eprintln!(
                "{}",
                json!({"refused": {"owner": owner, "action": item.actions(), "code": refusal.code, "field": refusal.field}})
            );
        }
    }
    Ok(())
}

/// Apply one item; `true` when it changed the store.
fn settle(engine: &Engine, item: &Item, now: &str) -> Result<bool, Refusal> {
    match item {
        Item::Single { write, ty } => {
            let current = engine.get(ty, &write.key)?;
            Ok(changed(&write.commit(engine, current.as_ref())?))
        }
        Item::Pair {
            first,
            second,
            ty,
            status,
        } => {
            let current: Option<Got> = engine.get(ty, &first.key)?;
            if current
                .as_ref()
                .is_some_and(|o| first.holds_with_status(o, status))
            {
                return Ok(false);
            }
            let one = changed(&first.commit(engine, current.as_ref())?);
            let after = engine.get(ty, &second.key)?;
            let two = changed(&second.commit(engine, after.as_ref())?);
            Ok(one || two)
        }
        Item::DayClose { slug, date } => Ok(changed(&close_one(engine, date, slug, now)?)),
    }
}

// ---------------------------------------------------------------- reading

/// The text of a file: `None` when it does not exist.
fn read(path: &Path) -> Result<Option<String>, String> {
    match fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| "the file is not UTF-8".to_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read the file: {}", e.kind())),
    }
}

fn is_dir(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_dir())
}

struct Run<'a> {
    engine: &'a Engine,
    domain: &'a Domain,
    parse: Vec<Parse>,
    /// Every reminder id of the run, so that one id is imported once.
    reminders: BTreeSet<String>,
}

impl Run<'_> {
    fn parse(&mut self, path: &str, line: usize, reason: impl Into<String>) {
        self.parse.push(Parse {
            path: path.to_owned(),
            line,
            reason: reason.into(),
        });
    }
}

fn build(engine: &Engine, domain: &Domain, directory: &Path) -> Result<Plan, Refusal> {
    let students_dir = directory.join("students");
    let registry_text = match read(&students_dir.join("REGISTRY.md")) {
        Ok(Some(text)) => text,
        Ok(None) => {
            return Err(Refusal::invalid(
                "directory",
                "no students/REGISTRY.md in the directory",
            ));
        }
        Err(reason) => return Err(Refusal::invalid("directory", &reason)),
    };
    let mut run = Run {
        engine,
        domain,
        parse: Vec::new(),
        reminders: BTreeSet::new(),
    };
    let registry_path = "students/REGISTRY.md";
    let Some(registry) = registry(&registry_text) else {
        return Err(Refusal::invalid(
            "directory",
            "students/REGISTRY.md has no table of students",
        ));
    };
    let mut plan = Plan {
        class: Vec::new(),
        students: Vec::new(),
        parse: Vec::new(),
    };
    for found in registry.class {
        match found {
            ClassLine::Reminder {
                line,
                id,
                closed,
                text,
            } => {
                if let Some(item) =
                    run.reminder(registry_path, line, &id, closed, &text, None, "local_state")
                {
                    plan.class.push(item);
                }
            }
            ClassLine::Empty { line, id } => {
                run.parse(
                    registry_path,
                    line,
                    format!("the reminder {id} has no text"),
                );
            }
        }
    }

    let mut rows_seen = BTreeSet::new();
    let mut slugs_with_row = BTreeSet::new();
    for row in &registry.rows {
        slugs_with_row.insert(row.slug.clone());
        if let Err(reason) = run.registry_row(row, &mut rows_seen) {
            run.parse(registry_path, row.line, reason);
            continue;
        }
        let status = row.status.to_lowercase();
        let mut items = vec![Item::Single {
            write: student_write(
                engine,
                &row.slug,
                &Registered {
                    name: &row.name,
                    student_id: row.student_id.as_deref(),
                    course_id: row.course_id.as_deref(),
                    membership_id: row.membership_id.as_deref(),
                    status: &status,
                    class_slot: row.class_slot.as_deref(),
                },
            ),
            ty: "student",
        }];
        let dir = students_dir.join(&row.slug);
        let mut staging_lines = 0;
        if is_dir(&dir) {
            staging_lines = run.student(&row.slug, &dir, &mut items);
        } else {
            run.parse(
                registry_path,
                row.line,
                format!("the directory students/{} is missing", row.slug),
            );
        }
        plan.students.push(StudentPlan {
            slug: row.slug.clone(),
            items,
            staging_lines,
        });
    }

    // A directory nobody registered is not read.
    let mut names: Vec<String> = fs::read_dir(&students_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| is_dir(&e.path()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for name in names {
        if name != "_template" && !slugs_with_row.contains(&name) {
            run.parse(
                &format!("students/{name}"),
                0,
                "the directory has no row in students/REGISTRY.md",
            );
        }
    }
    plan.parse = run.parse;
    Ok(plan)
}

impl Run<'_> {
    /// Why a registry row cannot be imported, if it cannot.
    fn registry_row(&self, row: &Row, seen: &mut BTreeSet<String>) -> Result<(), String> {
        if check_slug(&row.slug, "slug").is_err() || row.slug.starts_with('.') {
            return Err(
                "the slug is not 1 to 32 bytes of ASCII letters, digits, '_' and '-'".to_owned(),
            );
        }
        if !seen.insert(row.slug.clone()) {
            return Err(format!("the slug {} is in the table twice", row.slug));
        }
        if row.name.is_empty() {
            return Err("the row has no student name".to_owned());
        }
        for (name, id) in [
            ("studentId", &row.student_id),
            ("courseId", &row.course_id),
            ("membershipId", &row.membership_id),
        ] {
            let bad = id.as_ref().is_some_and(|i| {
                !(1..=32).contains(&i.len()) || !i.bytes().all(|b| b.is_ascii_digit())
            });
            if bad {
                return Err(format!("{name} is not 1 to 32 digits"));
            }
        }
        let status = row.status.to_lowercase();
        if !self.domain.student_statuses.contains(&status) {
            return Err(format!(
                "the status {} is not one of {}",
                row.status,
                self.domain.student_statuses.join(", ")
            ));
        }
        Ok(())
    }

    /// Read what one student's directory holds; the staging queue's lines.
    fn student(&mut self, slug: &str, dir: &Path, items: &mut Vec<Item>) -> usize {
        let rel = format!("students/{slug}");
        if let Some(text) = self.file(&rel, dir, "CONTEXT.md")
            && let Some(surface) = surface(&text)
        {
            items.push(Item::Single {
                write: surface_write(self.engine, slug, &surface),
                ty: "student",
            });
        }
        if let Some(text) = self.file(&rel, dir, "textbook-ledger.md") {
            self.ledger(&format!("{rel}/textbook-ledger.md"), slug, &text, items);
        }
        if let Some(text) = self.file(&rel, dir, "profile.md") {
            self.profile(&format!("{rel}/profile.md"), slug, &text, items);
        }
        if let Some(text) = self.file(&rel, dir, "progress-log.md") {
            self.progress(&format!("{rel}/progress-log.md"), slug, &text, items);
        }
        fs::read(dir.join("hindsight-staging.jsonl")).map_or(0, |bytes| {
            let ends = bytes.iter().filter(|b| **b == b'\n').count();
            ends + usize::from(bytes.last().is_some_and(|b| *b != b'\n'))
        })
    }

    fn file(&mut self, rel: &str, dir: &Path, name: &str) -> Option<String> {
        match read(&dir.join(name)) {
            Ok(text) => text,
            Err(reason) => {
                self.parse(&format!("{rel}/{name}"), 0, reason);
                None
            }
        }
    }

    /// A reminder as one item, or a parse entry when its id was seen or is
    /// not one of the two forms.
    #[allow(clippy::too_many_arguments)]
    fn reminder(
        &mut self,
        path: &str,
        line: usize,
        id: &str,
        closed: bool,
        text: &str,
        student: Option<&str>,
        provenance: &str,
    ) -> Option<Item> {
        if !reminder_id(id) {
            self.parse(
                path,
                line,
                format!(
                    "the id {id} is not R-YYYYMMDD-<KEY> or R-YYYYMMDD-NNN of at most 32 bytes"
                ),
            );
            return None;
        }
        if !self.reminders.insert(id.to_owned()) {
            self.parse(path, line, format!("the reminder id {id} is used twice"));
            return None;
        }
        let write = open_write(
            self.engine,
            id,
            &Reminder {
                text,
                student,
                due: None,
                provenance,
            },
        );
        Some(if closed {
            Item::Pair {
                first: write,
                second: close_write(self.engine, id),
                ty: "reminder",
                status: "closed",
            }
        } else {
            Item::Single {
                write,
                ty: "reminder",
            }
        })
    }

    fn profile(&mut self, path: &str, slug: &str, text: &str, items: &mut Vec<Item>) {
        let lines = scan(text);
        for table in tables(&lines) {
            let (Some(c_id), Some(c_status), Some(c_summary), Some(c_prov)) = (
                table.column("id"),
                table.column("status"),
                table.column("summary"),
                table.column("provenance"),
            ) else {
                continue;
            };
            for row in &table.rows {
                let at = row.line;
                let id = clean(row.cell(c_id));
                let closed = match clean(row.cell(c_status)).to_uppercase().as_str() {
                    "OPEN" => false,
                    "CLOSED" => true,
                    other => {
                        self.parse(
                            path,
                            at,
                            format!("the status {other} is not OPEN or CLOSED"),
                        );
                        continue;
                    }
                };
                let summary = row.cell(c_summary).trim();
                if summary.is_empty() {
                    self.parse(path, at, format!("the reminder {id} has no summary"));
                    continue;
                }
                let Some(provenance) = self.tag(row.cell(c_prov)) else {
                    self.parse(
                        path,
                        at,
                        format!("the reminder {id} has no known provenance"),
                    );
                    continue;
                };
                if let Some(item) =
                    self.reminder(path, at, &id, closed, summary, Some(slug), &provenance)
                {
                    items.push(item);
                }
            }
        }
    }

    /// The first tag of a provenance cell, in lower case, when the schema has it.
    fn tag(&self, cell: &str) -> Option<String> {
        let tag = cell
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .find(|t| !t.is_empty())?
            .to_lowercase();
        self.domain.provenances.contains(&tag).then_some(tag)
    }

    fn ledger(&mut self, path: &str, slug: &str, text: &str, items: &mut Vec<Item>) {
        let lines = scan(text);
        let mut books: Vec<String> = Vec::new();
        let mut rows: BTreeMap<String, Vec<LedgerRow>> = BTreeMap::new();
        for table in tables(&lines) {
            let Some(book) = table.column("bookno") else {
                continue;
            };
            let verbatim = table.column("verbatim booklabel");
            let name = table.column("book name");
            let field = |row: &TableRow, column: Option<usize>| {
                column
                    .map(|c| row.cell(c).trim().to_owned())
                    .unwrap_or_default()
            };
            for row in &table.rows {
                let number = clean(row.cell(book));
                if number.is_empty() {
                    self.parse(path, row.line, "the row has no bookNo");
                    continue;
                }
                if !(1..=32).contains(&number.len()) || !number.bytes().all(|b| b.is_ascii_digit())
                {
                    self.parse(
                        path,
                        row.line,
                        format!("the bookNo {number} is not 1 to 32 digits"),
                    );
                    continue;
                }
                let entry = LedgerRow {
                    line: row.line,
                    verbatim: verbatim.is_some(),
                    label: field(row, verbatim.or(name)),
                    course_key: field(row, table.column("coursekey")),
                    role: field(row, table.column("role / track")),
                    provenance: field(row, table.column("provenance")),
                    status: clean(&field(row, table.column("status"))),
                    at: clean(&field(row, table.column("observed / confirmed at"))),
                };
                if !rows.contains_key(&number) {
                    books.push(number.clone());
                }
                rows.entry(number).or_default().push(entry);
            }
        }
        for number in books {
            let rows = &rows[&number];
            let first = &rows[0];
            let label_row = rows.iter().find(|r| r.verbatim && !r.label.is_empty());
            let label = label_row.map_or(first.label.as_str(), |r| r.label.as_str());
            let course_key = label_row
                .map(|r| r.course_key.as_str())
                .filter(|c| !c.is_empty())
                .unwrap_or(first.course_key.as_str());
            // The rest comes from the first table, and from a later row only
            // where that table has no value for it.
            let rest = |value: fn(&LedgerRow) -> &str| {
                rows.iter().map(value).find(|v| !v.is_empty()).unwrap_or("")
            };
            let (role, status, at) = (rest(|r| &r.role), rest(|r| &r.status), rest(|r| &r.at));
            let provenance = rows.iter().find_map(|r| self.tag(&r.provenance));
            let label = if label.is_empty() {
                rows.iter()
                    .map(|r| r.label.as_str())
                    .find(|l| !l.is_empty())
                    .unwrap_or("")
            } else {
                label
            };
            if label.is_empty() {
                self.parse(path, first.line, format!("the book {number} has no label"));
                continue;
            }
            let Some(provenance) = provenance else {
                self.parse(
                    path,
                    first.line,
                    format!("the book {number} has no provenance"),
                );
                continue;
            };
            if status.is_empty() {
                self.parse(path, first.line, format!("the book {number} has no status"));
                continue;
            }
            let upper = status.to_uppercase();
            let (kept, retired) = if upper == "ACTIVE" {
                ("active", false)
            } else if upper.contains("CANDIDATE") {
                ("candidate", false)
            } else {
                ("active", true)
            };
            let at = (check_utc(at, "at").is_ok()).then_some(at);
            let write = bind_write(
                self.engine,
                slug,
                &number,
                &Binding {
                    label,
                    course_key: Some(course_key).filter(|c| !c.is_empty()),
                    role: Some(role).filter(|r| !r.is_empty()),
                    status: kept,
                    provenance: &provenance,
                    at,
                },
            );
            items.push(if retired {
                Item::Pair {
                    first: write,
                    second: unbind_write(self.engine, slug, &number),
                    ty: "textbook_binding",
                    status: "retired",
                }
            } else {
                Item::Single {
                    write,
                    ty: "textbook_binding",
                }
            });
        }
    }

    fn progress(&mut self, path: &str, slug: &str, text: &str, items: &mut Vec<Item>) {
        let lines = scan(text);
        let mut notes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        // The date and the lines of the section being read.
        let mut open: Option<(String, Vec<&str>)> = None;
        let close = |open: &mut Option<(String, Vec<&str>)>,
                     notes: &mut BTreeMap<String, Vec<String>>| {
            if let Some((date, body)) = open.take() {
                notes
                    .entry(date)
                    .or_default()
                    .push(body.join("\n").trim_end().to_owned());
            }
        };
        for l in &lines {
            if !l.fenced && (l.text.starts_with("# ") || l.text.starts_with("## ")) {
                close(&mut open, &mut notes);
                if let Some(rest) = l.text.strip_prefix("## ") {
                    match heading_date(rest) {
                        Heading::Date(date) => open = Some((date, vec![l.text])),
                        Heading::Bad => self.parse(
                            path,
                            l.no,
                            "the heading starts like a date but is not a date YYYY-MM-DD",
                        ),
                        Heading::Other => {}
                    }
                }
            } else if let Some((_, body)) = open.as_mut() {
                body.push(l.text);
            }
        }
        close(&mut open, &mut notes);
        let newest = notes.keys().next_back().cloned();
        for (date, sections) in notes {
            items.push(Item::Single {
                write: note_write(
                    self.engine,
                    slug,
                    &date,
                    &cut_note(&sections.join("\n\n")),
                    "local_state",
                ),
                ty: "lesson_note",
            });
        }
        if let Some(date) = newest {
            items.push(Item::DayClose {
                slug: slug.to_owned(),
                date,
            });
        }
    }
}

struct LedgerRow {
    line: usize,
    /// The row's table has a column "Verbatim bookLabel".
    verbatim: bool,
    label: String,
    course_key: String,
    role: String,
    provenance: String,
    status: String,
    at: String,
}

// ---------------------------------------------------------------- the registry

struct Row {
    line: usize,
    slug: String,
    name: String,
    student_id: Option<String>,
    course_id: Option<String>,
    membership_id: Option<String>,
    status: String,
    class_slot: Option<String>,
}

enum ClassLine {
    Reminder {
        line: usize,
        id: String,
        closed: bool,
        text: String,
    },
    Empty {
        line: usize,
        id: String,
    },
}

struct Registry {
    class: Vec<ClassLine>,
    rows: Vec<Row>,
}

/// The table of students and the reminders of the whole class before it.
fn registry(text: &str) -> Option<Registry> {
    let lines = scan(text);
    let tables = tables(&lines);
    let table = tables.iter().find(|t| {
        t.column("slug").is_some()
            && t.column("student name").is_some()
            && t.column("status").is_some()
    })?;
    let (slug, name, status) = (
        table.column("slug")?,
        table.column("student name")?,
        table.column("status")?,
    );
    let optional = |row: &TableRow, name: &str| {
        let cell = table.column(name).map(|c| clean(row.cell(c)))?;
        (!ABSENT.contains(&cell.to_lowercase().as_str())).then_some(cell)
    };
    let rows = table
        .rows
        .iter()
        .map(|row| Row {
            line: row.line,
            slug: clean(row.cell(slug)),
            name: row.cell(name).trim().to_owned(),
            student_id: optional(row, "studentid"),
            course_id: optional(row, "courseid"),
            membership_id: optional(row, "membershipid"),
            status: clean(row.cell(status)),
            class_slot: optional(row, "class slot"),
        })
        .collect();
    let class = lines
        .iter()
        .filter(|l| !l.fenced && l.no < table.line)
        .filter_map(|l| class_reminder(l.no, l.text))
        .collect();
    Some(Registry { class, rows })
}

/// A line that holds a reminder id and the word OPEN or CLOSED.
fn class_reminder(line: usize, text: &str) -> Option<ClassLine> {
    let (start, end) = runs(text, |c| c.is_ascii_alphanumeric() || c == '-')
        .into_iter()
        .map(|(s, e)| (s, s + text[s..e].trim_end_matches('-').len()))
        .find(|(s, e)| id_shape(&text[*s..*e]))?;
    let id = text[start..end].to_owned();
    let blanked = format!("{} {}", &text[..start], &text[end..]);
    let (ws, we) = runs(&blanked, |c| c.is_ascii_alphanumeric())
        .into_iter()
        .find(|(s, e)| matches!(&blanked[*s..*e], "OPEN" | "CLOSED"))?;
    let closed = &blanked[ws..we] == "CLOSED";
    let mut rest = format!("{} {}", &blanked[..ws], &blanked[we..]);
    // What enclosed the id or the word and is now empty goes with them.
    for pair in ["( )", "()", "[ ]", "[]", "` `", "``", "** **", "****"] {
        rest = rest.replace(pair, " ");
    }
    let rest = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = rest
        .trim_matches(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '-' | '*' | '+' | '>' | ':' | '|' | ',' | '\u{2014}' | '\u{2013}'
                )
        })
        .to_owned();
    Some(if text.is_empty() {
        ClassLine::Empty { line, id }
    } else {
        ClassLine::Reminder {
            line,
            id,
            closed,
            text,
        }
    })
}

/// The maximal runs of characters the predicate accepts, as byte ranges.
fn runs(text: &str, accepts: impl Fn(char) -> bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (accepts(c), start) {
            (true, None) => start = Some(at),
            (false, Some(s)) => {
                out.push((s, at));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    out
}

/// R-YYYYMMDD-<capitals, digits and "-">, which holds R-YYYYMMDD-NNN.
fn id_shape(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() > 11
        && id.starts_with("R-")
        && bytes[2..10].iter().all(u8::is_ascii_digit)
        && bytes[10] == b'-'
        && bytes[11..]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'-')
}

fn reminder_id(id: &str) -> bool {
    id_shape(id) && check_slug(id, "id").is_ok()
}

// ---------------------------------------------------------------- Markdown

struct Line<'a> {
    /// 1-based.
    no: usize,
    text: &'a str,
    /// A line of a fenced block, its fences included.
    fenced: bool,
}

fn scan(text: &str) -> Vec<Line<'_>> {
    let mut fence = false;
    text.split('\n')
        .enumerate()
        .map(|(i, raw)| {
            let text = raw.strip_suffix('\r').unwrap_or(raw);
            let marker =
                text.trim_start().starts_with("```") || text.trim_start().starts_with("~~~");
            let fenced = fence || marker;
            if marker {
                fence = !fence;
            }
            Line {
                no: i + 1,
                text,
                fenced,
            }
        })
        .collect()
}

struct TableRow {
    line: usize,
    cells: Vec<String>,
}

impl TableRow {
    fn cell(&self, column: usize) -> &str {
        self.cells.get(column).map_or("", String::as_str)
    }
}

struct Table {
    /// The line of the header.
    line: usize,
    header: Vec<String>,
    rows: Vec<TableRow>,
}

impl Table {
    fn column(&self, name: &str) -> Option<usize> {
        self.header.iter().position(|h| h == name)
    }
}

/// A column's name as compared: lower case, one space between words, no
/// emphasis or code marks.
fn norm(name: &str) -> String {
    name.replace(['*', '`'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// A cell without emphasis or code marks.
fn clean(cell: &str) -> String {
    cell.replace(['*', '`'], "").trim().to_owned()
}

fn cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cell)),
            c => cell.push(c),
        }
    }
    if !cell.trim().is_empty() {
        out.push(cell);
    }
    out.into_iter().map(|c| c.trim().to_owned()).collect()
}

fn is_separator(line: &str) -> bool {
    let cells = cells(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            !c.is_empty() && c.contains('-') && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))
        })
}

fn tables<'a>(lines: &[Line<'a>]) -> Vec<Table> {
    let table_line = |l: &Line| !l.fenced && l.text.trim_start().starts_with('|');
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        if table_line(&lines[i]) && table_line(&lines[i + 1]) && is_separator(lines[i + 1].text) {
            let mut table = Table {
                line: lines[i].no,
                header: cells(lines[i].text).iter().map(|h| norm(h)).collect(),
                rows: Vec::new(),
            };
            i += 2;
            while i < lines.len() && table_line(&lines[i]) {
                table.rows.push(TableRow {
                    line: lines[i].no,
                    cells: cells(lines[i].text),
                });
                i += 1;
            }
            out.push(table);
        } else {
            i += 1;
        }
    }
    out
}

enum Heading {
    Date(String),
    /// It starts the way a date does but is none.
    Bad,
    Other,
}

/// What follows "## ": a date, with or without text after it.
fn heading_date(rest: &str) -> Heading {
    let bytes = rest.as_bytes();
    let shaped = bytes.len() >= 10
        && bytes[..10].iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        });
    if !shaped || bytes.get(10).is_some_and(u8::is_ascii_alphanumeric) {
        return Heading::Other;
    }
    if is_date(&rest[..10]) {
        Heading::Date(rest[..10].to_owned())
    } else {
        Heading::Bad
    }
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

/// A note's text within 8,000 bytes: cut text ends with the line "(cut)".
fn cut_note(text: &str) -> String {
    if text.len() <= NOTE_MAX {
        return text.to_owned();
    }
    let kept = floor_boundary(text, NOTE_MAX - NOTE_CUT.len());
    format!("{}{NOTE_CUT}", &text[..kept])
}

/// The bullets under the heading that contains "Current Surface", cut at
/// 2,000 bytes; `None` when there is no such heading or no bullet under it.
fn surface(text: &str) -> Option<String> {
    let lines = scan(text);
    let level = |l: &Line| {
        let hashes = l.text.bytes().take_while(|b| *b == b'#').count();
        (!l.fenced && (1..=6).contains(&hashes) && l.text[hashes..].starts_with(' '))
            .then_some(hashes)
    };
    let at = lines
        .iter()
        .position(|l| level(l).is_some() && l.text.to_lowercase().contains("current surface"))?;
    let depth = level(&lines[at])?;
    let mut kept: Vec<&str> = Vec::new();
    let mut in_bullet = false;
    for l in &lines[at + 1..] {
        if level(l).is_some_and(|d| d <= depth) {
            break;
        }
        if l.fenced {
            continue;
        }
        let trimmed = l.text.trim_start();
        let bullet = ["- ", "* ", "+ "].iter().any(|m| trimmed.starts_with(m));
        if bullet || (in_bullet && l.text.starts_with([' ', '\t']) && !trimmed.is_empty()) {
            kept.push(l.text.trim_end());
            in_bullet = true;
        } else {
            in_bullet = false;
        }
    }
    if kept.is_empty() {
        return None;
    }
    let joined = kept.join("\n");
    Some(joined[..floor_boundary(&joined, SURFACE_MAX)].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_reads_a_class_reminder_line_in_its_forms() {
        let line = |text: &str| match class_reminder(3, text) {
            Some(ClassLine::Reminder {
                id, closed, text, ..
            }) => (id, closed, text),
            _ => panic!("{text}"),
        };
        assert_eq!(
            line("- R-20400101-CLASSA OPEN: bring the chart"),
            ("R-20400101-CLASSA".into(), false, "bring the chart".into())
        );
        assert_eq!(
            line("Reminder `R-20400102-001` (CLOSED) return the key (blue)"),
            (
                "R-20400102-001".into(),
                true,
                "Reminder return the key (blue)".into()
            )
        );
        assert!(class_reminder(1, "R-20400102-001 has no state").is_none());
        assert!(class_reminder(1, "OPEN without an id").is_none());
        assert!(matches!(
            class_reminder(1, "R-20400102-001 OPEN"),
            Some(ClassLine::Empty { .. })
        ));
    }

    #[test]
    fn import_cuts_a_note_so_that_it_ends_with_the_cut_line() {
        assert_eq!(cut_note("short"), "short");
        let text = "가".repeat(4000); // 12,000 bytes
        let cut = cut_note(&text);
        assert!(cut.len() <= NOTE_MAX, "{}", cut.len());
        assert!(cut.ends_with("\n(cut)"));
        assert!(cut.len() > NOTE_MAX - 4);
        let exact = "x".repeat(NOTE_MAX);
        assert_eq!(cut_note(&exact), exact);
        assert_eq!(cut_note(&format!("{exact}y")).len(), NOTE_MAX);
    }

    #[test]
    fn import_reads_a_table_with_escaped_pipes_and_two_tables() {
        let text = "intro\n\n| A | B |\n|---|:-:|\n| 1 | x \\| y |\n| 2 |\n\n| C |\n| - |\n| z |\n";
        let lines = scan(text);
        let found = tables(&lines);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].rows[0].cells, ["1", "x | y"]);
        assert_eq!(found[0].rows[1].cell(1), "");
        assert_eq!(found[1].rows[0].line, 10);
    }

    #[test]
    fn import_finds_the_surface_under_its_heading_only() {
        let text = "# Context\n- not this\n\n## Current Surface (now)\n- one\n  - nested\n\nprose\n- two\n### Deeper\n- three\n## Other\n- four\n";
        assert_eq!(surface(text).unwrap(), "- one\n  - nested\n- two\n- three");
        assert!(surface("# Nothing here\n- a\n").is_none());
    }

    #[test]
    fn import_tells_a_dated_heading_from_other_headings() {
        assert!(matches!(heading_date("2040-01-02"), Heading::Date(_)));
        assert!(matches!(
            heading_date("2040-01-02 - invented lesson"),
            Heading::Date(_)
        ));
        assert!(matches!(heading_date("2040-13-45"), Heading::Bad));
        assert!(matches!(heading_date("Notes"), Heading::Other));
        assert!(matches!(heading_date("2040-01-023"), Heading::Other));
    }
}
