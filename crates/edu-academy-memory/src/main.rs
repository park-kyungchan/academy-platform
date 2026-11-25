//! edu-academy-memory: a student's memory in the domain `academy_memory`, read
//! and written through the host's engine run as a process. See README.md.
mod engine;
mod text;

use engine::{Engine, Found, Got, value};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};
use text::{Entry, Section};

/// The schema the command writes through; its namespace keys every Id.
const SCHEMA: &str = include_str!("../../../ontology/academy-memory.schema.json");
/// The prefix of every request id the command makes.
const REQUEST_PREFIX: &str = "eam-";
/// The engine's limit on a request id.
const REQUEST_MAX: usize = 64;
const BRIEF_MAX: usize = 6000;
const SLUG_MAX: usize = 32;
/// The live reads a lesson day needs, in this order.
const DUE_KINDS: [&str; 5] = [
    "dailyrecord",
    "prestudy",
    "section_results",
    "dailytest",
    "clinic",
];
const USAGE: &str = "edu-academy-memory [--store <path>] (brief <slug> | show <type> <key> | roster | due <slug> <lesson_date> | student | surface | bind | unbind | observed | note | remind open|close | counsel prepare|result|close | day-close <lesson_date> <slug>...); see README.md";

/// A refusal: one line {"error":{code,field,detail}} on standard error.
pub struct Refusal {
    pub code: String,
    field: String,
    detail: String,
    exit: i32,
}

impl Refusal {
    fn new(code: &str, field: &str, detail: &str) -> Self {
        Self {
            code: code.to_owned(),
            field: field.to_owned(),
            detail: detail.to_owned(),
            exit: 2,
        }
    }
    fn usage(field: &str, detail: &str) -> Self {
        Self::new("usage", field, detail)
    }
    fn invalid(field: &str, detail: &str) -> Self {
        Self::new("invalid_input", field, detail)
    }
    fn student_missing(slug: &str) -> Self {
        Self::new(
            "student_missing",
            "slug",
            &format!("no student {slug} in the store"),
        )
    }
    pub fn engine_missing(detail: &str) -> Self {
        Self {
            exit: 3,
            ..Self::new("engine_missing", "", detail)
        }
    }
    /// The engine's refusal passed on with its own code.
    pub fn from_engine(answer: &Value) -> Self {
        let text = |name: &str| answer.get(name).and_then(Value::as_str).unwrap_or("");
        let failed: Vec<&str> = answer
            .get("failed")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let detail = if !failed.is_empty() {
            format!("{}: {}", failed.join(", "), text("message"))
        } else if !text("rule").is_empty() {
            text("rule").to_owned()
        } else {
            text("message").to_owned()
        };
        Self::new(
            answer
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("refused"),
            text("field"),
            &detail,
        )
    }
    fn line(&self) -> String {
        json!({"error": {"code": self.code, "field": self.field, "detail": self.detail}})
            .to_string()
    }
}

struct Domain {
    namespace: Value,
    /// Every object type with its properties, in the schema's order of keys.
    types: BTreeMap<String, Vec<String>>,
    kinds: Vec<String>,
}

fn domain() -> Domain {
    // The schema is part of the build; a malformed one fails the tests.
    let schema: Value = serde_json::from_str(SCHEMA).unwrap_or(Value::Null);
    let types = schema
        .get("object_types")
        .and_then(Value::as_object)
        .map(|types| {
            types
                .iter()
                .map(|(name, ty)| {
                    let properties = ty
                        .get("properties")
                        .and_then(Value::as_object)
                        .map(|p| p.keys().cloned().collect())
                        .unwrap_or_default();
                    (name.clone(), properties)
                })
                .collect()
        })
        .unwrap_or_default();
    let kinds = schema
        .pointer("/value_types/observation_kind/enum")
        .and_then(Value::as_array)
        .map(|k| {
            k.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Domain {
        namespace: schema.get("namespace").cloned().unwrap_or(Value::Null),
        types,
        kinds,
    }
}

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(refusal) => {
            eprintln!("{}", refusal.line());
            refusal.exit
        }
    };
    std::process::exit(code);
}

fn run() -> Result<i32, Refusal> {
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        args.push(
            arg.into_string()
                .map_err(|_| Refusal::usage("", "an argument is not UTF-8"))?,
        );
    }
    let mut store = None;
    if args.first().map(String::as_str) == Some("--store") {
        if args.len() < 2 {
            return Err(Refusal::usage("--store", "--store takes a path"));
        }
        store = Some(args[1].clone());
        args.drain(..2);
    }
    let Some(command) = args.first().cloned() else {
        return Err(Refusal::usage("", USAGE));
    };
    let rest = &args[1..];
    let domain = domain();
    let engine = Engine::new(store, domain.namespace.clone());
    match command.as_str() {
        "brief" => brief(&engine, rest).map(|text| {
            print!("{text}");
            0
        }),
        "show" => show(&engine, &domain, rest).map(|text| {
            print!("{text}");
            0
        }),
        "roster" => roster(&engine, rest).map(|text| {
            print!("{text}");
            0
        }),
        "due" => due(&engine, rest).map(|answer| {
            println!("{answer}");
            0
        }),
        "day-close" => {
            day_close(&engine, rest).map_err(|mut refusal| {
                // For day-close alone, exit 1 stands in the place of exit 2.
                if refusal.exit == 2 {
                    refusal.exit = 1;
                }
                refusal
            })
        }
        _ => write_command(&engine, &domain, &command, rest).map(|answer| {
            println!("{answer}");
            0
        }),
    }
}

// ---------------------------------------------------------------- arguments

struct Args {
    positional: Vec<String>,
    options: BTreeMap<String, String>,
}

impl Args {
    fn parse(args: &[String], allowed: &[&str], positional: &[&str]) -> Result<Self, Refusal> {
        let mut parsed = Self {
            positional: Vec::new(),
            options: BTreeMap::new(),
        };
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if let Some(name) = arg.strip_prefix("--") {
                if !allowed.contains(&name) {
                    return Err(Refusal::usage(arg, &format!("unknown option {arg}")));
                }
                let value = iter
                    .next()
                    .ok_or_else(|| Refusal::usage(arg, &format!("{arg} takes a value")))?;
                if parsed
                    .options
                    .insert(name.to_owned(), value.clone())
                    .is_some()
                {
                    return Err(Refusal::usage(arg, &format!("{arg} is given twice")));
                }
            } else {
                parsed.positional.push(arg.clone());
            }
        }
        if parsed.positional.len() != positional.len() {
            return Err(Refusal::usage(
                "",
                &format!("expected the arguments <{}>", positional.join("> <")),
            ));
        }
        Ok(parsed)
    }
    fn opt(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }
    fn required(&self, name: &str) -> Result<&str, Refusal> {
        self.opt(name)
            .ok_or_else(|| Refusal::usage(&format!("--{name}"), &format!("--{name} is required")))
    }
}

fn check_slug(slug: &str, field: &str) -> Result<(), Refusal> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    if slug.is_empty() || slug.len() > SLUG_MAX || !slug.chars().all(allowed) {
        return Err(Refusal::invalid(
            field,
            "1 to 32 bytes of ASCII letters, digits, '.', '_' and '-'",
        ));
    }
    Ok(())
}

fn digits(text: &str, at: std::ops::Range<usize>) -> bool {
    text.get(at)
        .is_some_and(|d| d.bytes().all(|b| b.is_ascii_digit()))
}

fn number(text: &str, at: std::ops::Range<usize>) -> u32 {
    text.get(at)
        .and_then(|d| d.parse().ok())
        .unwrap_or(u32::MAX)
}

fn is_date(text: &str) -> bool {
    text.len() == 10
        && digits(text, 0..4)
        && &text[4..5] == "-"
        && digits(text, 5..7)
        && &text[7..8] == "-"
        && digits(text, 8..10)
        && (1..=12).contains(&number(text, 5..7))
        && (1..=31).contains(&number(text, 8..10))
}

fn check_date(text: &str, field: &str) -> Result<(), Refusal> {
    if is_date(text) {
        Ok(())
    } else {
        Err(Refusal::invalid(field, "a date YYYY-MM-DD"))
    }
}

fn check_utc(text: &str, field: &str) -> Result<(), Refusal> {
    let ok = text.len() == 20
        && text.is_ascii()
        && is_date(&text[..10])
        && &text[10..11] == "T"
        && digits(text, 11..13)
        && &text[13..14] == ":"
        && digits(text, 14..16)
        && &text[16..17] == ":"
        && digits(text, 17..19)
        && &text[19..20] == "Z"
        && number(text, 11..13) < 24
        && number(text, 14..16) < 60
        && number(text, 17..19) < 60;
    if ok {
        Ok(())
    } else {
        Err(Refusal::invalid(field, "a UTC time YYYY-MM-DDTHH:MM:SSZ"))
    }
}

fn check_digits(text: &str, field: &str) -> Result<(), Refusal> {
    if (1..=32).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(Refusal::invalid(field, "1 to 32 digits"))
    }
}

/// A text given as is, or as @file by the file's contents.
fn content(given: &str, field: &str) -> Result<String, Refusal> {
    match given.strip_prefix('@') {
        Some(path) => read_file(path, field),
        None => Ok(given.to_owned()),
    }
}

/// A text that must be given as @file.
fn file_content(given: &str, field: &str) -> Result<String, Refusal> {
    match given.strip_prefix('@') {
        Some(path) => read_file(path, field),
        None => Err(Refusal::usage(field, &format!("{field} takes @<file>"))),
    }
}

fn read_file(path: &str, field: &str) -> Result<String, Refusal> {
    let bytes = std::fs::read(path)
        .map_err(|e| Refusal::invalid(field, &format!("cannot read the file: {}", e.kind())))?;
    String::from_utf8(bytes).map_err(|_| Refusal::invalid(field, "the file is not UTF-8"))
}

/// The clock's UTC time YYYY-MM-DDTHH:MM:SSZ.
fn utc_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let of_day = seconds % 86_400;
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    )
}

// ---------------------------------------------------------------- reads

fn field(fields: &Map<String, Value>, name: &str) -> String {
    engine::text(fields, name).unwrap_or_default()
}

fn field_or_none(fields: &Map<String, Value>, name: &str) -> String {
    engine::text(fields, name).unwrap_or_else(|| "none".to_owned())
}

fn student_of(engine: &Engine, slug: &str) -> Result<Got, Refusal> {
    check_slug(slug, "slug")?;
    engine
        .get("student", slug)?
        .ok_or_else(|| Refusal::student_missing(slug))
}

/// A book number in numeric order.
fn book_order(found: &Found) -> (usize, String, String) {
    let number = field(&found.fields, "book_no");
    (number.len(), number, found.key.clone())
}

fn active_bindings(engine: &Engine, slug: &str) -> Result<Vec<Found>, Refusal> {
    let student = engine.id("student", slug);
    let mut bindings = engine.search(
        "textbook_binding",
        json!({"all": [{"eq": ["student", student]}, {"eq": ["status", "active"]}]}),
    )?;
    bindings.sort_by_key(book_order);
    Ok(bindings)
}

/// Newest first by the property, then by key.
fn newest(mut found: Vec<Found>, by: &str, count: usize) -> Vec<Found> {
    found.sort_by(|a, b| (field(&b.fields, by), &b.key).cmp(&(field(&a.fields, by), &a.key)));
    found.truncate(count);
    found
}

fn brief(engine: &Engine, args: &[String]) -> Result<String, Refusal> {
    let args = Args::parse(args, &[], &["slug"])?;
    let slug = args.positional[0].as_str();
    let student = student_of(engine, slug)?;
    let id = engine.id("student", slug);
    let bindings = active_bindings(engine, slug)?;
    let observations = newest(
        engine.search("observation", json!({"eq": ["student", id]}))?,
        "observed_at",
        12,
    );
    let notes = newest(
        engine.search("lesson_note", json!({"eq": ["student", id]}))?,
        "lesson_date",
        3,
    );
    let mut reminders: Vec<Found> = engine
        .search("reminder", json!({"eq": ["status", "open"]}))?
        .into_iter()
        .filter(|r| match engine::text(&r.fields, "student") {
            None => true,
            Some(owner) => owner == slug,
        })
        .collect();
    reminders.sort_by(|a, b| a.key.cmp(&b.key));
    let counselings = newest(
        engine.search("counseling", json!({"eq": ["student", id]}))?,
        "date",
        2,
    );

    let fields = &student.fields;
    let mut card = vec![Entry::new("slug: ", slug)];
    for name in [
        "name",
        "status",
        "class_slot",
        "student_id",
        "course_id",
        "membership_id",
    ] {
        if let Some(v) = engine::text(fields, name) {
            card.push(Entry::new(format!("{name}: "), v));
        }
    }
    card.push(Entry::new(
        "memory_as_of: ",
        field_or_none(fields, "memory_as_of"),
    ));
    for name in ["updated_at", "surface"] {
        if let Some(v) = engine::text(fields, name) {
            card.push(Entry::new(format!("{name}: "), v));
        }
    }
    let bindings: Vec<Entry> = bindings
        .iter()
        .map(|b| {
            Entry::new(
                format!(
                    "- book_no={} course_key={} label=",
                    field(&b.fields, "book_no"),
                    field_or_none(&b.fields, "course_key")
                ),
                field_or_none(&b.fields, "book_label"),
            )
        })
        .collect();
    let observations: Vec<Entry> = observations
        .iter()
        .map(|o| {
            Entry::new(
                format!(
                    "- {} {} {}: ",
                    field(&o.fields, "kind"),
                    field(&o.fields, "lesson_date"),
                    field(&o.fields, "observed_at")
                ),
                field(&o.fields, "summary"),
            )
        })
        .collect();
    let notes: Vec<Entry> = notes
        .iter()
        .map(|n| {
            Entry::new(
                format!(
                    "- {} {}: ",
                    field(&n.fields, "lesson_date"),
                    field(&n.fields, "provenance")
                ),
                field(&n.fields, "text"),
            )
        })
        .collect();
    let reminders: Vec<Entry> = reminders
        .iter()
        .map(|r| {
            let whose = if value(&r.fields, "student").is_some() {
                "student"
            } else {
                "class"
            };
            let due = engine::text(&r.fields, "due")
                .map(|d| format!(" due {d}"))
                .unwrap_or_default();
            Entry::new(
                format!("- {} ({whose}){due}: ", r.key),
                field(&r.fields, "text"),
            )
        })
        .collect();
    let counselings: Vec<Entry> = counselings
        .iter()
        .map(|c| {
            Entry::new(
                format!(
                    "- {} {}: ",
                    field(&c.fields, "date"),
                    field(&c.fields, "status")
                ),
                field_or_none(&c.fields, "purpose"),
            )
        })
        .collect();

    let sections: [(&str, usize, Option<usize>, &[Entry]); 6] = [
        ("## student", 1000, None, &card),
        ("## bindings (active)", 700, Some(160), &bindings),
        (
            "## observations (12 newest)",
            2400,
            Some(180),
            &observations,
        ),
        ("## notes (3 newest)", 1200, Some(370), &notes),
        ("## reminders (open)", 400, Some(110), &reminders),
        ("## counselings (2 newest)", 300, Some(120), &counselings),
    ];
    let mut out = String::new();
    for (index, (heading, budget, bound, entries)) in sections.into_iter().enumerate() {
        out.push_str(&text::render(
            &Section {
                heading,
                budget,
                bound,
                separated: index > 0,
            },
            entries,
        ));
    }
    debug_assert!(out.len() <= BRIEF_MAX);
    out.truncate(BRIEF_MAX.min(out.len()));
    Ok(out)
}

fn show(engine: &Engine, domain: &Domain, args: &[String]) -> Result<String, Refusal> {
    let args = Args::parse(args, &[], &["type", "key"])?;
    let (object_type, key) = (args.positional[0].as_str(), args.positional[1].as_str());
    let Some(properties) = domain.types.get(object_type) else {
        let types: Vec<&str> = domain.types.keys().map(String::as_str).collect();
        return Err(Refusal::invalid(
            "type",
            &format!("one of {}", types.join(", ")),
        ));
    };
    let got = if object_type == "student" {
        student_of(engine, key)?
    } else {
        if key.is_empty() {
            return Err(Refusal::invalid("key", "a key is not empty"));
        }
        engine.get(object_type, key)?.ok_or_else(|| {
            Refusal::new(
                "unavailable",
                "key",
                &format!("no {object_type} {key} in the store"),
            )
        })?
    };
    let mut out = format!("{object_type} {key}\nhead: {}\n", got.head);
    for name in properties {
        if let Some(v) = engine::text(&got.fields, name) {
            out.push_str(&format!("{name}: {v}\n"));
        }
    }
    Ok(out)
}

fn roster(engine: &Engine, args: &[String]) -> Result<String, Refusal> {
    Args::parse(args, &[], &[])?;
    let mut students = engine.search(
        "student",
        json!({"in": ["status", ["active", "onboarding"]]}),
    )?;
    students.sort_by(|a, b| a.key.cmp(&b.key));
    if students.is_empty() {
        return Ok("none\n".to_owned());
    }
    Ok(students
        .iter()
        .map(|s| {
            format!(
                "{}\tclass_slot={}\tmemory_as_of={}\n",
                text::flat(&s.key),
                text::flat(&field_or_none(&s.fields, "class_slot")),
                field_or_none(&s.fields, "memory_as_of")
            )
        })
        .collect())
}

/// Every observation of the student and date, by a search with both equalities.
fn observations_of(engine: &Engine, slug: &str, date: &str) -> Result<Vec<Found>, Refusal> {
    engine.search(
        "observation",
        json!({"all": [{"eq": ["student", engine.id("student", slug)]}, {"eq": ["lesson_date", date]}]}),
    )
}

fn due(engine: &Engine, args: &[String]) -> Result<Value, Refusal> {
    let args = Args::parse(args, &[], &["slug", "lesson_date"])?;
    let (slug, date) = (args.positional[0].as_str(), args.positional[1].as_str());
    check_slug(slug, "slug")?;
    check_date(date, "lesson_date")?;
    let student = student_of(engine, slug)?;
    let mut have: Vec<(String, String)> = observations_of(engine, slug, date)?
        .iter()
        .map(|o| (field(&o.fields, "observed_at"), field(&o.fields, "kind")))
        .collect();
    have.sort();
    let kinds: BTreeSet<&str> = have.iter().map(|(_, kind)| kind.as_str()).collect();
    let due: Vec<&str> = DUE_KINDS
        .into_iter()
        .filter(|k| !kinds.contains(k))
        .collect();
    let bindings: Vec<Value> = active_bindings(engine, slug)?
        .iter()
        .map(|b| {
            json!({
                "book_no": field(&b.fields, "book_no"),
                "course_key": value(&b.fields, "course_key"),
                "book_label": value(&b.fields, "book_label"),
            })
        })
        .collect();
    Ok(json!({
        "student": slug,
        "lesson_date": date,
        "memory_as_of": value(&student.fields, "memory_as_of"),
        "have": have.iter().map(|(at, kind)| json!({"kind": kind, "observed_at": at})).collect::<Vec<_>>(),
        "due": due,
        "bindings": bindings,
    }))
}

// ---------------------------------------------------------------- writes

/// One action of the schema on one object.
struct Write {
    action: &'static str,
    key: String,
    parameters: Map<String, Value>,
    /// The values the call would set, compared with the stored object.
    sets: Vec<(&'static str, Value)>,
    /// Parameters taken from the clock: out of the comparison and the hash.
    clock: &'static [&'static str],
}

impl Write {
    fn new(action: &'static str, key: String) -> Self {
        Self {
            action,
            key,
            parameters: Map::new(),
            sets: Vec::new(),
            clock: &[],
        }
    }
    /// A parameter that is also a property the call sets.
    fn set(&mut self, name: &'static str, v: Value) {
        self.parameters.insert(name.to_owned(), v.clone());
        self.sets.push((name, v));
    }
    fn set_opt(&mut self, name: &'static str, v: Option<&str>) {
        if let Some(v) = v {
            self.set(name, json!(v));
        }
    }
    /// A parameter that names an object and sets no property.
    fn param(&mut self, name: &str, v: Value) {
        self.parameters.insert(name.to_owned(), v);
    }
    /// A property the action's rule sets to a fixed value.
    fn fixed(&mut self, name: &'static str, v: &str) {
        self.sets.push((name, json!(v)));
    }

    fn request_id(&self, base: &str) -> String {
        let mut hashed = self.parameters.clone();
        for name in self.clock {
            hashed.remove(*name);
        }
        let digest = Sha256::digest(
            json!({"action": self.action, "parameters": hashed, "base": base})
                .to_string()
                .as_bytes(),
        );
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{REQUEST_PREFIX}{}",
            &hex[..REQUEST_MAX - REQUEST_PREFIX.len()]
        )
    }

    fn commit(&self, engine: &Engine, current: Option<&Got>) -> Result<Value, Refusal> {
        if let Some(object) = current
            && self
                .sets
                .iter()
                .all(|(name, v)| value(&object.fields, name) == Some(v))
        {
            return Ok(self.answer("already_applied"));
        }
        let base = current.map_or("new", |o| o.head.as_str());
        let outcome = engine.apply(self.action, &self.parameters, &self.request_id(base))?;
        Ok(self.answer(&outcome))
    }

    fn answer(&self, outcome: &str) -> Value {
        json!({"action": self.action, "key": self.key, "outcome": outcome})
    }
}

fn write_command(
    engine: &Engine,
    domain: &Domain,
    command: &str,
    args: &[String],
) -> Result<Value, Refusal> {
    match command {
        "student" => student(engine, args),
        "surface" => {
            let a = Args::parse(args, &["text"], &["slug"])?;
            let slug = a.positional[0].as_str();
            check_slug(slug, "slug")?;
            let surface = content(a.required("text")?, "--text")?;
            let mut w = Write::new("set_surface", slug.to_owned());
            w.param("student", engine.id("student", slug));
            w.set("surface", json!(surface));
            w.commit(engine, engine.get("student", slug)?.as_ref())
        }
        "bind" => {
            let a = Args::parse(
                args,
                &["label", "course-key", "role", "status", "at", "provenance"],
                &["slug", "book_no"],
            )?;
            let (slug, book) = (a.positional[0].as_str(), a.positional[1].as_str());
            check_slug(slug, "slug")?;
            check_digits(book, "book_no")?;
            let label = a.required("label")?;
            let provenance = a.required("provenance")?;
            let status = a.opt("status").unwrap_or("active");
            if !matches!(status, "active" | "candidate") {
                return Err(Refusal::invalid("--status", "active or candidate"));
            }
            if let Some(at) = a.opt("at") {
                check_utc(at, "--at")?;
            }
            let key = format!("{slug}|{book}");
            let mut w = Write::new("bind_textbook", key.clone());
            w.param("binding", engine.id("textbook_binding", &key));
            w.set("student", engine.id("student", slug));
            w.set("book_no", json!(book));
            w.set("book_label", json!(label));
            w.set_opt("course_key", a.opt("course-key"));
            w.set_opt("role", a.opt("role"));
            w.set("status", json!(status));
            w.set("provenance", json!(provenance));
            w.set_opt("observed_at", a.opt("at"));
            w.commit(engine, engine.get("textbook_binding", &key)?.as_ref())
        }
        "unbind" => {
            let a = Args::parse(args, &[], &["slug", "book_no"])?;
            let (slug, book) = (a.positional[0].as_str(), a.positional[1].as_str());
            check_slug(slug, "slug")?;
            check_digits(book, "book_no")?;
            let key = format!("{slug}|{book}");
            let mut w = Write::new("retire_binding", key.clone());
            w.param("student", engine.id("student", slug));
            w.param("binding", engine.id("textbook_binding", &key));
            w.fixed("status", "retired");
            w.commit(engine, engine.get("textbook_binding", &key)?.as_ref())
        }
        "observed" => {
            let a = Args::parse(
                args,
                &["at", "summary", "tool", "record"],
                &["slug", "kind", "lesson_date"],
            )?;
            let (slug, kind, date) = (
                a.positional[0].as_str(),
                a.positional[1].as_str(),
                a.positional[2].as_str(),
            );
            check_slug(slug, "slug")?;
            if !domain.kinds.iter().any(|k| k == kind) {
                return Err(Refusal::invalid(
                    "kind",
                    &format!("one of {}", domain.kinds.join(", ")),
                ));
            }
            check_date(date, "lesson_date")?;
            let at = a.required("at")?;
            check_utc(at, "--at")?;
            let summary = content(a.required("summary")?, "--summary")?;
            let key = format!("{slug}|{kind}|{date}|{at}");
            let mut w = Write::new("record_observation", key.clone());
            w.param("observation", engine.id("observation", &key));
            w.set("student", engine.id("student", slug));
            w.set("kind", json!(kind));
            w.set("lesson_date", json!(date));
            w.set("observed_at", json!(at));
            w.set("summary", json!(summary));
            w.set_opt("tool", a.opt("tool"));
            w.set_opt("record_id", a.opt("record"));
            w.commit(engine, engine.get("observation", &key)?.as_ref())
        }
        "note" => {
            let a = Args::parse(args, &["provenance", "text"], &["slug", "lesson_date"])?;
            let (slug, date) = (a.positional[0].as_str(), a.positional[1].as_str());
            check_slug(slug, "slug")?;
            check_date(date, "lesson_date")?;
            let provenance = a.required("provenance")?;
            let text = content(a.required("text")?, "--text")?;
            let key = format!("{slug}|{date}");
            let mut w = Write::new("write_lesson_note", key.clone());
            w.param("note", engine.id("lesson_note", &key));
            w.set("student", engine.id("student", slug));
            w.set("lesson_date", json!(date));
            w.set("text", json!(text));
            w.set("provenance", json!(provenance));
            w.commit(engine, engine.get("lesson_note", &key)?.as_ref())
        }
        "remind" => remind(engine, args),
        "counsel" => counsel(engine, args),
        _ => Err(Refusal::usage("", USAGE)),
    }
}

fn student(engine: &Engine, args: &[String]) -> Result<Value, Refusal> {
    let a = Args::parse(
        args,
        &[
            "name",
            "student-id",
            "course-id",
            "membership-id",
            "status",
            "class-slot",
        ],
        &["slug"],
    )?;
    let slug = a.positional[0].as_str();
    check_slug(slug, "slug")?;
    let name = a.required("name")?;
    let current = engine.get("student", slug)?;
    let status = match (a.opt("status"), &current) {
        (Some(status), _) => status.to_owned(),
        (None, Some(stored)) => field(&stored.fields, "status"),
        (None, None) => {
            return Err(Refusal::usage(
                "--status",
                "--status is required when the student is new",
            ));
        }
    };
    let mut w = Write::new("register_student", slug.to_owned());
    w.param("student", engine.id("student", slug));
    w.set("slug", json!(slug));
    w.set("name", json!(name));
    w.set_opt("student_id", a.opt("student-id"));
    w.set_opt("course_id", a.opt("course-id"));
    w.set_opt("membership_id", a.opt("membership-id"));
    w.set("status", json!(status));
    w.set_opt("class_slot", a.opt("class-slot"));
    w.commit(engine, current.as_ref())
}

fn check_id(id: &str) -> Result<(), Refusal> {
    // A reminder's id is written in a slug's characters, so a brief line can
    // always hold it.
    check_slug(id, "id")
}

fn remind(engine: &Engine, args: &[String]) -> Result<Value, Refusal> {
    match args.first().map(String::as_str) {
        Some("open") => {
            let a = Args::parse(
                &args[1..],
                &["text", "student", "due", "provenance"],
                &["id"],
            )?;
            let id = a.positional[0].as_str();
            check_id(id)?;
            let text = a.required("text")?;
            let provenance = a.required("provenance")?;
            if let Some(slug) = a.opt("student") {
                check_slug(slug, "--student")?;
            }
            if let Some(due) = a.opt("due") {
                check_date(due, "--due")?;
            }
            let mut w = Write::new("open_reminder", id.to_owned());
            w.param("reminder", engine.id("reminder", id));
            w.set("text", json!(text));
            if let Some(slug) = a.opt("student") {
                w.set("student", engine.id("student", slug));
            }
            w.set_opt("due", a.opt("due"));
            w.set("provenance", json!(provenance));
            w.fixed("status", "open");
            w.commit(engine, engine.get("reminder", id)?.as_ref())
        }
        Some("close") => {
            let a = Args::parse(&args[1..], &[], &["id"])?;
            let id = a.positional[0].as_str();
            check_id(id)?;
            let mut w = Write::new("close_reminder", id.to_owned());
            w.param("reminder", engine.id("reminder", id));
            w.fixed("status", "closed");
            w.commit(engine, engine.get("reminder", id)?.as_ref())
        }
        _ => Err(Refusal::usage(
            "",
            "remind open <id> ... | remind close <id>",
        )),
    }
}

fn counsel(engine: &Engine, args: &[String]) -> Result<Value, Refusal> {
    let step = args.first().map(String::as_str).unwrap_or("");
    let allowed: &[&str] = match step {
        "prepare" => &["purpose", "form", "stored-at"],
        "result" => &["result"],
        "close" => &[],
        _ => {
            return Err(Refusal::usage(
                "",
                "counsel prepare|result|close <slug> <date> ...",
            ));
        }
    };
    let a = Args::parse(&args[1..], allowed, &["slug", "date"])?;
    let (slug, date) = (a.positional[0].as_str(), a.positional[1].as_str());
    check_slug(slug, "slug")?;
    check_date(date, "date")?;
    let key = format!("{slug}|{date}");
    let counseling = engine.id("counseling", &key);
    let w = match step {
        "prepare" => {
            let purpose = a.required("purpose")?;
            let form = file_content(a.required("form")?, "--form")?;
            let mut w = Write::new("prepare_counseling", key.clone());
            w.param("counseling", counseling);
            w.set("student", engine.id("student", slug));
            w.set("date", json!(date));
            w.set("purpose", json!(purpose));
            w.set("form", json!(form));
            w.set_opt("stored_at", a.opt("stored-at"));
            w.fixed("status", "prepared");
            w
        }
        "result" => {
            let result = file_content(a.required("result")?, "--result")?;
            let mut w = Write::new("record_counseling", key.clone());
            w.param("counseling", counseling);
            w.set("result", json!(result));
            w.fixed("status", "held");
            w
        }
        _ => {
            let mut w = Write::new("close_counseling", key.clone());
            w.param("counseling", counseling);
            w.fixed("status", "closed");
            w
        }
    };
    w.commit(engine, engine.get("counseling", &key)?.as_ref())
}

// ---------------------------------------------------------------- day-close

fn day_close(engine: &Engine, args: &[String]) -> Result<i32, Refusal> {
    let Some((date, slugs)) = args.split_first() else {
        return Err(Refusal::usage("", "day-close <lesson_date> <slug>..."));
    };
    if slugs.is_empty() || args.iter().any(|a| a.starts_with("--")) {
        return Err(Refusal::usage("", "day-close <lesson_date> <slug>..."));
    }
    check_date(date, "lesson_date")?;
    let now = utc_now();
    let mut refused = false;
    for slug in slugs {
        match close_one(engine, date, slug, &now) {
            Ok(answer) => println!("{answer}"),
            Err(refusal) if refusal.exit == 3 => return Err(refusal),
            Err(refusal) => {
                refused = true;
                println!("{}", refusal.line());
            }
        }
    }
    Ok(i32::from(refused))
}

fn close_one(engine: &Engine, date: &str, slug: &str, now: &str) -> Result<Value, Refusal> {
    let student = student_of(engine, slug)?;
    let observations = observations_of(engine, slug, date)?;
    let note = engine.get("lesson_note", &format!("{slug}|{date}"))?;
    if observations.is_empty() && note.is_none() {
        return Err(Refusal::new(
            "nothing_read",
            "slug",
            &format!("{slug}: no observation and no note of {date}"),
        ));
    }
    let mut w = Write::new("close_lesson_day", slug.to_owned());
    w.param("student", engine.id("student", slug));
    w.param("lesson_date", json!(date));
    w.param("updated_at", json!(now));
    w.fixed("memory_as_of", date);
    w.clock = &["updated_at"];
    w.commit(engine, Some(&student))
}
