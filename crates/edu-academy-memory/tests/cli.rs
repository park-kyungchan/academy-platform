//! The command against a stand-in engine of this test's own, so no engine is
//! needed. The stand-in is a script written into the test's temporary
//! directory that runs this test executable in stand-in mode; where that
//! directory does not allow executing, this executable is run directly. It
//! records each call's arguments and standard input and answers the next
//! answer the test queued. Invented students and texts only.
use serde_json::{Map, Value, json};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_edu-academy-memory");
const MODE: &str = "EDU_MEMORY_STAND_IN";
const SCHEMA: &str = include_str!("../../../ontology/academy-memory.schema.json");
static NEXT: AtomicU64 = AtomicU64::new(0);

type Test = fn();

fn main() {
    if let Some(dir) = std::env::var_os(MODE) {
        stand_in(Path::new(&dir));
    }
    let tests: &[(&str, Test)] = &[
        (
            "refuses_bad_input_before_any_call",
            refuses_bad_input_before_any_call,
        ),
        (
            "an_engine_that_cannot_run_is_engine_missing",
            an_engine_that_cannot_run_is_engine_missing,
        ),
        (
            "observed_applies_once_then_already_applied",
            observed_applies_once_then_already_applied,
        ),
        (
            "request_id_follows_head_and_file_contents",
            request_id_follows_head_and_file_contents,
        ),
        (
            "student_status_is_required_only_when_new",
            student_status_is_required_only_when_new,
        ),
        (
            "bind_passes_active_and_checks_its_time",
            bind_passes_active_and_checks_its_time,
        ),
        (
            "an_engine_refusal_is_passed_on",
            an_engine_refusal_is_passed_on,
        ),
        (
            "brief_stays_within_its_budgets",
            brief_stays_within_its_budgets,
        ),
        ("brief_of_nothing_says_none", brief_of_nothing_says_none),
        ("show_prints_one_object", show_prints_one_object),
        (
            "due_names_the_missing_kinds_and_exact_keys",
            due_names_the_missing_kinds_and_exact_keys,
        ),
        (
            "day_close_closes_each_student",
            day_close_closes_each_student,
        ),
        (
            "day_close_hash_leaves_out_the_clock",
            day_close_hash_leaves_out_the_clock,
        ),
        (
            "roster_prints_one_line_per_student",
            roster_prints_one_line_per_student,
        ),
        (
            "reminders_bindings_and_counselings",
            reminders_bindings_and_counselings,
        ),
    ];
    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        ran += 1;
        let outcome = std::panic::catch_unwind(test);
        println!(
            "test {name} ... {}",
            if outcome.is_ok() { "ok" } else { "FAILED" }
        );
        if outcome.is_err() {
            failed.push(*name);
        }
    }
    println!(
        "\ntest result: {}. {} passed; {} failed",
        if failed.is_empty() { "ok" } else { "FAILED" },
        ran - failed.len(),
        failed.len()
    );
    std::process::exit(i32::from(!failed.is_empty()));
}

// ---------------------------------------------------------------- stand-in

fn stand_in(dir: &Path) -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--probe"] {
        std::process::exit(0);
    }
    let count = dir.join("count");
    let n = fs::read_to_string(&count)
        .ok()
        .and_then(|c| c.trim().parse::<usize>().ok())
        .unwrap_or(0)
        + 1;
    fs::write(&count, n.to_string()).unwrap();
    fs::write(dir.join(format!("call{n}.args")), args.join("\n")).unwrap();
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    fs::write(dir.join(format!("call{n}.stdin")), input).unwrap();
    let answer = fs::read(dir.join(format!("answer{n}.json")))
        .unwrap_or_else(|_| br#"{"status":"refused","code":"stand_in_exhausted"}"#.to_vec());
    std::io::stdout().write_all(&answer).unwrap();
    let refused = serde_json::from_slice::<Value>(&answer)
        .map(|a| a["status"] == "refused")
        .unwrap_or(true);
    std::process::exit(if refused { 2 } else { 0 });
}

struct Stage {
    dir: PathBuf,
    engine: PathBuf,
    queued: usize,
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Out {
    fn json(&self) -> Value {
        serde_json::from_str(self.stdout.trim()).unwrap_or_else(|_| panic!("{}", self.stdout))
    }
    fn error(&self) -> Value {
        let line: Value = serde_json::from_str(self.stderr.trim())
            .unwrap_or_else(|_| panic!("stderr: {}", self.stderr));
        line["error"].clone()
    }
}

impl Stage {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "edu-academy-memory-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .unwrap();
        let exe = std::env::current_exe().unwrap();
        let script = dir.join("engine");
        let quoted = exe.display().to_string().replace('\'', r"'\''");
        fs::write(&script, format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n")).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let runs = Command::new(&script)
            .arg("--probe")
            .env(MODE, &dir)
            .status()
            .is_ok_and(|s| s.success());
        let engine = if runs { script } else { exe };
        Self {
            dir,
            engine,
            queued: 0,
        }
    }
    fn answer(&mut self, answer: Value) {
        self.queued += 1;
        fs::write(
            self.dir.join(format!("answer{}.json", self.queued)),
            answer.to_string(),
        )
        .unwrap();
    }
    fn run(&self, args: &[&str]) -> Out {
        self.run_with(&self.engine, args)
    }
    fn run_with(&self, engine: &Path, args: &[&str]) -> Out {
        let out = Command::new(BIN)
            .args(args)
            .env("EDU_ONT_BIN", engine)
            .env(MODE, &self.dir)
            .output()
            .unwrap();
        Out {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8(out.stdout).unwrap(),
            stderr: String::from_utf8(out.stderr).unwrap(),
        }
    }
    fn calls(&self) -> usize {
        fs::read_to_string(self.dir.join("count"))
            .ok()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(0)
    }
    /// The arguments and the request of call n.
    fn call(&self, n: usize) -> (String, Value) {
        let args = fs::read_to_string(self.dir.join(format!("call{n}.args"))).unwrap();
        let input = fs::read(self.dir.join(format!("call{n}.stdin"))).unwrap();
        (args, serde_json::from_slice(&input).unwrap())
    }
    fn file(&self, name: &str, text: &str) -> String {
        let path = self.dir.join(name);
        fs::write(&path, text).unwrap();
        format!("@{}", path.display())
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn namespace() -> Value {
    serde_json::from_str::<Value>(SCHEMA).unwrap()["namespace"].clone()
}
fn id(ty: &str, key: &str) -> Value {
    json!({"object_type": ty, "namespace": namespace(), "native_key": key})
}
fn fields(plain: &Value) -> Value {
    let mut out = Map::new();
    for (name, v) in plain.as_object().unwrap() {
        out.insert(name.clone(), json!({"kind": "value", "value": v}));
    }
    Value::Object(out)
}
fn unavailable() -> Value {
    json!({"status": "refused", "code": "unavailable"})
}
fn object(ty: &str, key: &str, head: &str, plain: Value) -> Value {
    json!({"status": "ok", "result": {"kind": "objects", "objects": [{
        "head": head,
        "id": id(ty, key),
        "observations": [
            {"revision": "older#0", "fields": {}},
            {"revision": head, "fields": fields(&plain)}
        ]
    }]}})
}
fn found(ty: &str, objects: &[(String, Value)], continuation: Option<&str>) -> Value {
    let objects: Vec<Value> = objects
        .iter()
        .map(|(key, plain)| json!({"id": id(ty, key), "head": {"fields": fields(plain)}}))
        .collect();
    json!({"status": "ok", "result": {"kind": "object_set", "objects": objects, "continuation": continuation}})
}
fn applied(outcome: &str) -> Value {
    json!({"status": "ok", "result": {"outcome": outcome}})
}
fn student(slug: &str, extra: Value) -> Value {
    let mut plain = json!({"slug": slug, "name": "Invented Name", "status": "active"});
    for (k, v) in extra.as_object().unwrap() {
        plain[k] = v.clone();
    }
    object("student", slug, "act-rq-s#0", plain)
}

// ---------------------------------------------------------------- tests

fn refuses_bad_input_before_any_call() {
    let stage = Stage::new();
    let too_long = "s".repeat(33);
    for args in [
        vec!["brief", "Bad Slug"],
        vec!["brief", too_long.as_str()],
        vec!["brief", "stu-ä"],
        vec!["due", "stu-a", "2040-1-2"],
        vec![
            "observed",
            "stu-a",
            "dailytest",
            "2040-01-02",
            "--at",
            "2040-01-02 03:04:05",
            "--summary",
            "x",
        ],
        vec![
            "observed",
            "stu-a",
            "dailytest",
            "2040-01-02",
            "--at",
            "2040-01-02T03:04:05+09:00",
            "--summary",
            "x",
        ],
        vec![
            "observed",
            "stu-a",
            "nonsense",
            "2040-01-02",
            "--at",
            "2040-01-02T03:04:05Z",
            "--summary",
            "x",
        ],
        vec![
            "bind",
            "stu-a",
            "12",
            "--label",
            "L",
            "--provenance",
            "planned",
            "--at",
            "2040-01-02",
        ],
        vec!["show", "lesson", "k"],
        vec![
            "remind",
            "open",
            "has space",
            "--text",
            "t",
            "--provenance",
            "planned",
        ],
    ] {
        let out = stage.run(&args);
        assert_eq!(out.code, 2, "{args:?} {}", out.stderr);
        assert_eq!(out.error()["code"], "invalid_input", "{args:?}");
        assert!(out.stdout.is_empty());
    }
    for args in [
        vec!["brief"],
        vec!["nonsense"],
        vec!["note", "stu-a", "2040-01-02", "--text", "t"],
        vec![
            "counsel",
            "result",
            "stu-a",
            "2040-01-02",
            "--result",
            "not a file",
        ],
        vec!["surface", "stu-a", "--text", "a", "--text", "b"],
    ] {
        let out = stage.run(&args);
        assert_eq!(out.code, 2, "{args:?}");
        assert_eq!(out.error()["code"], "usage", "{args:?}");
    }
    assert_eq!(stage.calls(), 0);
}

fn an_engine_that_cannot_run_is_engine_missing() {
    let stage = Stage::new();
    let out = stage.run_with(&stage.dir.join("no-engine"), &["roster"]);
    assert_eq!(out.code, 3);
    assert_eq!(out.error()["code"], "engine_missing");
    let out = stage.run_with(
        &stage.dir.join("no-engine"),
        &["day-close", "2040-01-02", "stu-a"],
    );
    assert_eq!(out.code, 3);
}

const AT: &str = "2040-01-02T03:04:05Z";

fn observed_args(summary: &str) -> Vec<String> {
    [
        "observed",
        "stu-a",
        "dailytest",
        "2040-01-02",
        "--at",
        AT,
        "--summary",
        summary,
        "--tool",
        "tool-x",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn observed_applies_once_then_already_applied() {
    let mut stage = Stage::new();
    let key = format!("stu-a|dailytest|2040-01-02|{AT}");
    stage.answer(unavailable());
    stage.answer(applied("applied"));
    let args = observed_args("seven of ten");
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = stage.run(&args);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        out.json(),
        json!({"action": "record_observation", "key": key, "outcome": "applied"})
    );
    let (argv, get) = stage.call(1);
    assert_eq!(argv, "get");
    assert_eq!(get["id"], id("observation", &key));
    let (argv, apply) = stage.call(2);
    assert_eq!(argv, "action\napply");
    assert_eq!(apply["action_type"], "record_observation");
    let rid = apply["request_id"].as_str().unwrap();
    assert!(rid.starts_with("eam-") && rid.len() == 64, "{rid}");
    assert!(
        rid.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    );
    assert_eq!(
        apply["parameters"],
        json!({
            "observation": id("observation", &key),
            "student": id("student", "stu-a"),
            "kind": "dailytest",
            "lesson_date": "2040-01-02",
            "observed_at": AT,
            "summary": "seven of ten",
            "tool": "tool-x",
        })
    );
    // The same call again: the store holds every value, so nothing is applied.
    stage.answer(object(
        "observation",
        &key,
        "act-rq-o#0",
        json!({"student": id("student", "stu-a"), "kind": "dailytest", "lesson_date": "2040-01-02",
               "observed_at": AT, "summary": "seven of ten", "tool": "tool-x"}),
    ));
    let out = stage.run(&args);
    assert_eq!(out.json()["outcome"], "already_applied");
    assert_eq!(stage.calls(), 3);
}

fn note_request_id(stage: &mut Stage, current: Value, text: &str) -> String {
    stage.answer(current);
    stage.answer(applied("applied"));
    let before = stage.calls();
    let out = stage.run(&[
        "note",
        "stu-a",
        "2040-01-02",
        "--provenance",
        "teacher_confirmed",
        "--text",
        text,
    ]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let (_, apply) = stage.call(before + 2);
    apply["request_id"].as_str().unwrap().to_owned()
}

fn request_id_follows_head_and_file_contents() {
    let mut stage = Stage::new();
    let one = stage.file("one.txt", "an invented note");
    let two = stage.file("two.txt", "an invented note");
    let first = note_request_id(&mut stage, unavailable(), &one);
    let same = note_request_id(&mut stage, unavailable(), &two);
    assert_eq!(first, same, "a file enters the hash by its contents");
    let literal = note_request_id(&mut stage, unavailable(), "an invented note");
    assert_eq!(first, literal);
    let stored = object(
        "lesson_note",
        "stu-a|2040-01-02",
        "act-rq-n#0",
        json!({"student": id("student", "stu-a"), "lesson_date": "2040-01-02", "text": "older", "provenance": "teacher_confirmed"}),
    );
    let after = note_request_id(&mut stage, stored, &one);
    assert_ne!(first, after, "the head revision enters the hash");
    let (_, apply) = stage.call(2);
    assert_eq!(apply["parameters"]["text"], "an invented note");
}

fn student_status_is_required_only_when_new() {
    let mut stage = Stage::new();
    stage.answer(unavailable());
    let out = stage.run(&["student", "stu-a", "--name", "Invented Name"]);
    assert_eq!(out.code, 2);
    assert_eq!(out.error()["field"], "--status");
    assert_eq!(stage.calls(), 1);
    stage.answer(student("stu-a", json!({"status": "paused"})));
    stage.answer(applied("applied"));
    let out = stage.run(&[
        "student",
        "stu-a",
        "--name",
        "Another Name",
        "--class-slot",
        "slot-1",
    ]);
    assert_eq!(out.json()["outcome"], "applied");
    let (_, apply) = stage.call(3);
    assert_eq!(apply["action_type"], "register_student");
    assert_eq!(
        apply["parameters"],
        json!({"student": id("student", "stu-a"), "slug": "stu-a", "name": "Another Name", "status": "paused", "class_slot": "slot-1"})
    );
    stage.answer(student("stu-a", json!({"class_slot": "slot-1"})));
    let out = stage.run(&[
        "student",
        "stu-a",
        "--name",
        "Invented Name",
        "--class-slot",
        "slot-1",
    ]);
    assert_eq!(out.json()["outcome"], "already_applied");
    assert_eq!(stage.calls(), 4);
}

fn bind_passes_active_and_checks_its_time() {
    let mut stage = Stage::new();
    stage.answer(unavailable());
    stage.answer(applied("applied"));
    let out = stage.run(&[
        "bind",
        "stu-a",
        "1234",
        "--label",
        "Invented Book 1-2 (full label)",
        "--provenance",
        "server_observed",
        "--at",
        AT,
    ]);
    assert_eq!(
        out.json(),
        json!({"action": "bind_textbook", "key": "stu-a|1234", "outcome": "applied"})
    );
    let (_, apply) = stage.call(2);
    assert_eq!(apply["parameters"]["status"], "active");
    assert_eq!(apply["parameters"]["observed_at"], AT);
    assert_eq!(
        apply["parameters"]["binding"],
        id("textbook_binding", "stu-a|1234")
    );
    let out = stage.run(&[
        "bind",
        "stu-a",
        "1234",
        "--label",
        "L",
        "--provenance",
        "planned",
        "--status",
        "retired",
    ]);
    assert_eq!(out.error()["code"], "invalid_input");
    assert_eq!(stage.calls(), 2);
}

fn an_engine_refusal_is_passed_on() {
    let mut stage = Stage::new();
    stage.answer(unavailable());
    stage.answer(json!({"status": "refused", "code": "submission_criteria", "failed": ["student_missing"],
        "message": "Apply register_student for this student first.", "trust_limit": "not passed on"}));
    let out = stage.run(&["surface", "stu-z", "--text", "facts"]);
    assert_eq!(out.code, 2);
    assert!(out.stdout.is_empty());
    let error = out.error();
    assert_eq!(error["code"], "submission_criteria");
    assert!(
        error["detail"]
            .as_str()
            .unwrap()
            .contains("student_missing")
    );
    assert_eq!(out.stderr.lines().count(), 1);
    assert!(!out.stderr.contains("trust_limit"));
}

fn brief_stays_within_its_budgets() {
    let mut stage = Stage::new();
    stage.answer(student(
        "stu-a",
        json!({"memory_as_of": "2040-01-01", "surface": "가".repeat(666)}),
    ));
    let bindings: Vec<(String, Value)> = (0..10)
        .map(|i| (format!("stu-a|{}", 100 - i), json!({"book_no": format!("{}", 100 - i), "course_key": "CK-1", "book_label": "L".repeat(600), "status": "active"})))
        .collect();
    stage.answer(found("textbook_binding", &bindings, None));
    let observation = |i: usize| {
        let at = format!("2040-01-{:02}T10:00:00Z", i + 1);
        (
            format!("stu-a|dailytest|2040-01-{:02}|{at}", i + 1),
            json!({"kind": "dailytest", "lesson_date": format!("2040-01-{:02}", i + 1), "observed_at": at, "summary": format!("{i} {}", "s\n".repeat(750))}),
        )
    };
    let first: Vec<_> = (0..10).map(observation).collect();
    let second: Vec<_> = (10..20).map(observation).collect();
    stage.answer(found("observation", &first, Some("c1:next")));
    stage.answer(found("observation", &second, None));
    let notes: Vec<_> = (0..5)
        .map(|i| (format!("stu-a|2040-01-{:02}", i + 1), json!({"lesson_date": format!("2040-01-{:02}", i + 1), "provenance": "teacher_confirmed", "text": "n".repeat(8000)})))
        .collect();
    stage.answer(found("lesson_note", &notes, None));
    let mut reminders: Vec<_> = (0..30)
        .map(|i| {
            (
                format!("r-{i:02}"),
                json!({"text": "t".repeat(600), "status": "open"}),
            )
        })
        .collect();
    reminders.push((
        "r-other".to_owned(),
        json!({"student": id("student", "stu-b"), "text": "other", "status": "open"}),
    ));
    stage.answer(found("reminder", &reminders, None));
    let counselings: Vec<_> = (0..4)
        .map(|i| (format!("stu-a|2040-02-{:02}", i + 1), json!({"date": format!("2040-02-{:02}", i + 1), "status": "prepared", "purpose": "p".repeat(600)})))
        .collect();
    stage.answer(found("counseling", &counselings, None));

    let out = stage.run(&["brief", "stu-a"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let brief = out.stdout;
    assert!(brief.len() <= 6000, "{}", brief.len());
    let headings: Vec<&str> = brief.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(headings.len(), 6);
    assert!(headings.iter().all(|h| h.len() <= 40));
    let parts: Vec<&str> = brief.split("\n\n").collect();
    assert_eq!(parts.len(), 6);
    // Each section's bytes: the split took the blank line between two
    // sections, the newline ending one and the separator opening the next.
    let mut total = 0;
    for (index, (part, budget)) in parts
        .iter()
        .zip([1000, 700, 2400, 1200, 400, 300])
        .enumerate()
    {
        let size = part.len() + usize::from(index > 0) + usize::from(index < 5);
        assert!(size <= budget, "{size} {part}");
        total += size;
    }
    assert_eq!(total, brief.len());
    let entries = |part: &str, bound: usize| {
        for line in part.lines().filter(|l| l.starts_with("- ")) {
            assert!(line.len() < bound, "{line}");
        }
    };
    entries(parts[1], 160);
    entries(parts[2], 180);
    entries(parts[3], 370);
    entries(parts[4], 110);
    entries(parts[5], 120);
    // The surface is cut with its count; the whole is read by show.
    let surface = parts[0]
        .lines()
        .find(|l| l.starts_with("surface: "))
        .unwrap();
    let (kept, rest) = surface["surface: ".len()..].split_once("… (+").unwrap();
    let left_out: usize = rest.trim_end_matches(" bytes)").parse().unwrap();
    assert_eq!(kept.len() + left_out, 1998);
    assert!(parts[0].contains("memory_as_of: 2040-01-01"));
    // Bindings by book number, cut, the rest counted.
    let first_binding = parts[1].lines().nth(1).unwrap();
    assert!(
        first_binding.starts_with("- book_no=91 course_key=CK-1 label=LLL"),
        "{first_binding}"
    );
    assert!(parts[1].lines().last().unwrap().ends_with(" more)"));
    // The 12 newest observations, newest first, continuation followed.
    let (_, page) = stage.call(4);
    assert_eq!(page["continuation"], "c1:next");
    let observations: Vec<&str> = parts[2].lines().skip(1).collect();
    assert!(
        observations[0].starts_with("- dailytest 2040-01-20 2040-01-20T10:00:00Z: 19 s s"),
        "{}",
        observations[0]
    );
    assert!(observations.iter().filter(|l| l.starts_with("- ")).count() <= 12);
    let count = |l: &str| -> usize {
        l.rsplit_once("(+")
            .unwrap()
            .1
            .trim_end_matches(" bytes)")
            .parse()
            .unwrap()
    };
    let kept = observations[0]
        .split_once(": ")
        .unwrap()
        .1
        .split_once('…')
        .unwrap()
        .0;
    assert_eq!(kept.len() + count(observations[0]), "19 ".len() + 1500);
    assert!(
        parts[3]
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("- 2040-01-05 teacher_confirmed: nnn")
    );
    assert!(!parts[4].contains("r-other"));
    assert!(
        parts[4]
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("- r-00 (class): ttt")
    );
    assert!(parts[4].lines().last().unwrap().ends_with(" more)"));
    assert!(
        parts[5]
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("- 2040-02-04 prepared: ppp")
    );
    for n in 1..=stage.calls() {
        let (argv, _) = stage.call(n);
        assert!(argv == "get" || argv == "search", "{argv}");
    }
    assert_eq!(stage.calls(), 7);
}

fn brief_of_nothing_says_none() {
    let mut stage = Stage::new();
    stage.answer(unavailable());
    let out = stage.run(&["brief", "stu-a"]);
    assert_eq!(out.code, 2);
    assert_eq!(out.error()["code"], "student_missing");
    stage.answer(student("stu-a", json!({})));
    for ty in [
        "textbook_binding",
        "observation",
        "lesson_note",
        "reminder",
        "counseling",
    ] {
        stage.answer(found(ty, &[], None));
    }
    let out = stage.run(&["brief", "stu-a"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(out.stdout.lines().filter(|l| *l == "none").count(), 5);
    assert!(out.stdout.contains("memory_as_of: none"));
}

fn show_prints_one_object() {
    let mut stage = Stage::new();
    let key = format!("stu-a|clinic|2040-01-02|{AT}");
    stage.answer(object("observation", &key, "act-rq-o#1", json!({"student": id("student", "stu-a"), "kind": "clinic", "summary": "line one\nline two"})));
    let out = stage.run(&["show", "observation", &key]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout
            .starts_with(&format!("observation {key}\nhead: act-rq-o#1\n"))
    );
    assert!(out.stdout.contains("student: stu-a\n"));
    assert!(out.stdout.contains("summary: line one\nline two\n"));
    stage.answer(unavailable());
    let out = stage.run(&["show", "student", "stu-q"]);
    assert_eq!(out.error()["code"], "student_missing");
}

fn due_names_the_missing_kinds_and_exact_keys() {
    let mut stage = Stage::new();
    stage.answer(student("stu-a", json!({"memory_as_of": "2040-01-01"})));
    stage.answer(found(
        "observation",
        &[
            (
                "k1".to_owned(),
                json!({"kind": "prestudy_report", "observed_at": "2040-01-02T09:00:00Z"}),
            ),
            (
                "k2".to_owned(),
                json!({"kind": "dailytest", "observed_at": "2040-01-02T08:00:00Z"}),
            ),
        ],
        None,
    ));
    stage.answer(found("textbook_binding", &[
        ("stu-a|77".to_owned(), json!({"book_no": "77", "course_key": "CK-7", "book_label": "Invented Book, Part 2 (full)"})),
    ], None));
    let out = stage.run(&["due", "stu-a", "2040-01-02"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        out.json(),
        json!({
            "student": "stu-a",
            "lesson_date": "2040-01-02",
            "memory_as_of": "2040-01-01",
            "have": [{"kind": "dailytest", "observed_at": "2040-01-02T08:00:00Z"}, {"kind": "prestudy_report", "observed_at": "2040-01-02T09:00:00Z"}],
            "due": ["dailyrecord", "prestudy", "section_results", "clinic"],
            "bindings": [{"book_no": "77", "course_key": "CK-7", "book_label": "Invented Book, Part 2 (full)"}],
        })
    );
    let (argv, search) = stage.call(2);
    assert_eq!(argv, "search");
    assert_eq!(search["object_type"], "observation");
    assert_eq!(
        search["where"],
        json!({"all": [{"eq": ["student", id("student", "stu-a")]}, {"eq": ["lesson_date", "2040-01-02"]}]})
    );
}

fn is_utc(text: &str) -> bool {
    text.len() == 20 && text.as_bytes()[10] == b'T' && text.ends_with('Z')
}

fn day_close_closes_each_student() {
    let mut stage = Stage::new();
    // stu-a: nothing read.
    stage.answer(student("stu-a", json!({})));
    stage.answer(found("observation", &[], None));
    stage.answer(unavailable());
    // stu-b: a note only; applied.
    stage.answer(student("stu-b", json!({"memory_as_of": "2040-01-01"})));
    stage.answer(found("observation", &[], None));
    stage.answer(object(
        "lesson_note",
        "stu-b|2040-01-02",
        "act-rq-n#0",
        json!({"text": "x"}),
    ));
    stage.answer(applied("applied"));
    // stu-c: already closed.
    stage.answer(student("stu-c", json!({"memory_as_of": "2040-01-02"})));
    stage.answer(found(
        "observation",
        &[("k".to_owned(), json!({"kind": "clinic"}))],
        None,
    ));
    stage.answer(unavailable());
    // stu-d: no student.
    stage.answer(unavailable());
    let out = stage.run(&[
        "day-close",
        "2040-01-02",
        "stu-a",
        "stu-b",
        "stu-c",
        "Bad!",
        "stu-d",
    ]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let lines: Vec<Value> = out
        .stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0]["error"]["code"], "nothing_read");
    assert_eq!(
        lines[1],
        json!({"action": "close_lesson_day", "key": "stu-b", "outcome": "applied"})
    );
    assert_eq!(
        lines[2],
        json!({"action": "close_lesson_day", "key": "stu-c", "outcome": "already_applied"})
    );
    assert_eq!(lines[3]["error"]["code"], "invalid_input");
    assert_eq!(lines[4]["error"]["code"], "student_missing");
    let (_, apply) = stage.call(7);
    assert_eq!(apply["action_type"], "close_lesson_day");
    assert_eq!(apply["parameters"]["lesson_date"], "2040-01-02");
    assert!(
        is_utc(apply["parameters"]["updated_at"].as_str().unwrap()),
        "{apply}"
    );
    assert_eq!(stage.calls(), 11);
    let out = stage.run(&["day-close", "2040-13-02", "stu-a"]);
    assert_eq!(out.code, 1);
    assert_eq!(out.error()["code"], "invalid_input");
}

fn day_close_hash_leaves_out_the_clock() {
    let mut stage = Stage::new();
    let mut ids = Vec::new();
    for round in 0..2 {
        if round == 1 {
            std::thread::sleep(std::time::Duration::from_millis(1100));
        }
        stage.answer(student("stu-b", json!({"memory_as_of": "2040-01-01"})));
        stage.answer(found(
            "observation",
            &[("k".to_owned(), json!({"kind": "clinic"}))],
            None,
        ));
        stage.answer(unavailable());
        stage.answer(applied("applied"));
        let out = stage.run(&["day-close", "2040-01-02", "stu-b"]);
        assert_eq!(out.code, 0, "{}", out.stderr);
        let (_, apply) = stage.call(stage.calls());
        ids.push((
            apply["request_id"].clone(),
            apply["parameters"]["updated_at"].clone(),
        ));
    }
    assert_eq!(ids[0].0, ids[1].0);
    assert_ne!(ids[0].1, ids[1].1);
}

fn roster_prints_one_line_per_student() {
    let mut stage = Stage::new();
    stage.answer(found(
        "student",
        &[
            ("stu-b".to_owned(), json!({"status": "onboarding"})),
            (
                "stu-a".to_owned(),
                json!({"status": "active", "class_slot": "Slot One", "memory_as_of": "2040-01-02"}),
            ),
        ],
        None,
    ));
    let out = stage.run(&["roster"]);
    assert_eq!(
        out.stdout,
        "stu-a\tclass_slot=Slot One\tmemory_as_of=2040-01-02\nstu-b\tclass_slot=none\tmemory_as_of=none\n"
    );
    let (_, search) = stage.call(1);
    assert_eq!(
        search["where"],
        json!({"in": ["status", ["active", "onboarding"]]})
    );
}

fn reminders_bindings_and_counselings() {
    let mut stage = Stage::new();
    stage.answer(unavailable());
    stage.answer(applied("applied"));
    let out = stage.run(&[
        "remind",
        "open",
        "r-1",
        "--text",
        "bring the invented sheet",
        "--provenance",
        "planned",
    ]);
    assert_eq!(
        out.json(),
        json!({"action": "open_reminder", "key": "r-1", "outcome": "applied"})
    );
    let (_, apply) = stage.call(2);
    assert!(apply["parameters"].get("student").is_none());
    stage.answer(object(
        "reminder",
        "r-1",
        "act-rq-r#0",
        json!({"text": "x", "status": "closed"}),
    ));
    let out = stage.run(&["remind", "close", "r-1"]);
    assert_eq!(out.json()["outcome"], "already_applied");
    stage.answer(object(
        "textbook_binding",
        "stu-a|12",
        "act-rq-b#0",
        json!({"status": "retired"}),
    ));
    let out = stage.run(&["unbind", "stu-a", "12"]);
    assert_eq!(
        out.json(),
        json!({"action": "retire_binding", "key": "stu-a|12", "outcome": "already_applied"})
    );
    let form = stage.file("form.md", "# invented form\n");
    stage.answer(unavailable());
    stage.answer(applied("applied"));
    let out = stage.run(&[
        "counsel",
        "prepare",
        "stu-a",
        "2040-03-01",
        "--purpose",
        "invented purpose",
        "--form",
        &form,
        "--stored-at",
        "counsel/stu-a.md",
    ]);
    assert_eq!(out.json()["action"], "prepare_counseling");
    let (_, apply) = stage.call(6);
    assert_eq!(apply["parameters"]["form"], "# invented form\n");
    assert_eq!(apply["parameters"]["stored_at"], "counsel/stu-a.md");
    let result = stage.file("result.md", "invented result");
    stage.answer(object(
        "counseling",
        "stu-a|2040-03-01",
        "act-rq-c#0",
        json!({"status": "prepared"}),
    ));
    stage.answer(applied("applied"));
    let out = stage.run(&[
        "counsel",
        "result",
        "stu-a",
        "2040-03-01",
        "--result",
        &result,
    ]);
    assert_eq!(
        out.json(),
        json!({"action": "record_counseling", "key": "stu-a|2040-03-01", "outcome": "applied"})
    );
    let (_, apply) = stage.call(8);
    assert_eq!(
        apply["parameters"],
        json!({"counseling": id("counseling", "stu-a|2040-03-01"), "result": "invented result"})
    );
    stage.answer(object(
        "counseling",
        "stu-a|2040-03-01",
        "act-rq-c#1",
        json!({"status": "closed"}),
    ));
    let out = stage.run(&["counsel", "close", "stu-a", "2040-03-01"]);
    assert_eq!(out.json()["outcome"], "already_applied");
}
