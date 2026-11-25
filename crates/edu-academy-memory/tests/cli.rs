//! The command against a stand-in engine of this test's own, so no engine is
//! needed. The stand-in is a script written into the test's temporary
//! directory that runs this test executable in stand-in mode; where that
//! directory does not allow executing, this executable is run directly. It
//! records each call's arguments and standard input and answers the next
//! answer the test queued. Invented students and texts only.
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
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
        (
            "import_dry_run_plans_every_write_and_calls_nothing",
            import_dry_run_plans_every_write_and_calls_nothing,
        ),
        (
            "import_applies_every_form_and_a_second_run_changes_nothing",
            import_applies_every_form_and_a_second_run_changes_nothing,
        ),
        (
            "import_then_roster_and_brief_show_what_was_read",
            import_then_roster_and_brief_show_what_was_read,
        ),
        (
            "import_counts_a_refusal_and_goes_on",
            import_counts_a_refusal_and_goes_on,
        ),
        (
            "import_cuts_a_long_note_and_a_long_surface",
            import_cuts_a_long_note_and_a_long_surface,
        ),
        (
            "import_leaves_the_directory_as_it_was",
            import_leaves_the_directory_as_it_was,
        ),
        (
            "import_without_an_engine_stops_with_engine_missing",
            import_without_an_engine_stops_with_engine_missing,
        ),
        (
            "import_refuses_bad_arguments_and_a_directory_without_a_registry",
            import_refuses_bad_arguments_and_a_directory_without_a_registry,
        ),
        (
            "import_of_a_clean_directory_exits_zero_twice",
            import_of_a_clean_directory_exits_zero_twice,
        ),
        (
            "import_writes_only_the_close_for_a_reminder_closed_later",
            import_writes_only_the_close_for_a_reminder_closed_later,
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
    fs::write(dir.join(format!("call{n}.stdin")), &input).unwrap();
    if dir.join("fake.json").exists() {
        fake(dir, &args, &input);
    }
    let answer = fs::read(dir.join(format!("answer{n}.json")))
        .unwrap_or_else(|_| br#"{"status":"refused","code":"stand_in_exhausted"}"#.to_vec());
    std::io::stdout().write_all(&answer).unwrap();
    let refused = serde_json::from_slice::<Value>(&answer)
        .map(|a| a["status"] == "refused")
        .unwrap_or(true);
    std::process::exit(if refused { 2 } else { 0 });
}

// ---------------------------------------------------------------- fake store

/// A small in-memory engine kept in `fake.json`: `get`, `search` (the filters
/// `eq`, `in` and `all`) and the actions an import applies, with the bounds
/// of the schema that matter to it. A stage made by `Stage::fake` answers
/// from it instead of from the queue.
fn fake(dir: &Path, args: &[String], input: &[u8]) -> ! {
    let path = dir.join("fake.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let request: Value = serde_json::from_slice(input).unwrap_or(Value::Null);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let answer = match args.as_slice() {
        ["get"] => fake_get(&state, &request),
        ["search"] => fake_search(&state, &request),
        ["action", "apply"] => fake_apply(&mut state, &request),
        _ => json!({"status": "refused", "code": "stand_in_unknown"}),
    };
    fs::write(&path, state.to_string()).unwrap();
    std::io::stdout()
        .write_all(answer.to_string().as_bytes())
        .unwrap();
    std::process::exit(if answer["status"] == "refused" { 2 } else { 0 });
}

fn fake_slot(ty: &str, key: &str) -> String {
    format!("{ty}\t{key}")
}

fn fake_get(state: &Value, request: &Value) -> Value {
    let ty = request["id"]["object_type"].as_str().unwrap();
    let key = request["id"]["native_key"].as_str().unwrap();
    match state["objects"].get(fake_slot(ty, key)) {
        Some(o) => object(ty, key, o["head"].as_str().unwrap(), o["fields"].clone()),
        None => unavailable(),
    }
}

fn fake_matches(filter: &Value, plain: &Value) -> bool {
    if let Some(all) = filter.get("all") {
        return all
            .as_array()
            .unwrap()
            .iter()
            .all(|f| fake_matches(f, plain));
    }
    if let Some(eq) = filter.get("eq") {
        return plain.get(eq[0].as_str().unwrap()) == Some(&eq[1]);
    }
    if let Some(any) = filter.get("in") {
        let wanted = any[1].as_array().unwrap();
        return plain
            .get(any[0].as_str().unwrap())
            .is_some_and(|v| wanted.contains(v));
    }
    true
}

fn fake_search(state: &Value, request: &Value) -> Value {
    let ty = request["object_type"].as_str().unwrap();
    let prefix = fake_slot(ty, "");
    let objects: Vec<(String, Value)> = state["objects"]
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(slot, o)| {
            let key = slot.strip_prefix(&prefix)?;
            fake_matches(&request["where"], &o["fields"])
                .then(|| (key.to_owned(), o["fields"].clone()))
        })
        .collect();
    found(ty, &objects, None)
}

fn fake_refusal(code: &str, failed: &str) -> Value {
    json!({"status": "refused", "code": code, "failed": [failed], "message": "stand-in", "field": failed})
}

fn fake_apply(state: &mut Value, request: &Value) -> Value {
    let action = request["action_type"].as_str().unwrap();
    let params = request["parameters"].as_object().unwrap();
    let native = |name: &str| {
        params
            .get(name)
            .and_then(|v| v["native_key"].as_str())
            .map(str::to_owned)
    };
    let (ty, target) = match action {
        "register_student" | "set_surface" | "close_lesson_day" => ("student", "student"),
        "bind_textbook" | "retire_binding" => ("textbook_binding", "binding"),
        "open_reminder" | "close_reminder" => ("reminder", "reminder"),
        "write_lesson_note" => ("lesson_note", "note"),
        other => panic!("the fake store does not know {other}"),
    };
    let key = native(target).unwrap();
    let exists =
        |state: &Value, ty: &str, key: &str| state["objects"].get(fake_slot(ty, key)).is_some();
    if action != "register_student"
        && let Some(student) = native("student")
        && !exists(state, "student", &student)
    {
        return fake_refusal("submission_criteria", "student_missing");
    }
    let slot = fake_slot(ty, &key);
    let mut fields = state["objects"]
        .get(&slot)
        .map_or_else(|| json!({}), |o| o["fields"].clone());
    let too_long = |name: &str, max: usize| {
        params
            .get(name)
            .and_then(Value::as_str)
            .is_some_and(|v| v.len() > max)
    };
    let merge = |fields: &mut Value, skip: &str| {
        for (name, v) in params {
            if name != skip {
                fields[name] = v.clone();
            }
        }
    };
    match action {
        "register_student" => merge(&mut fields, "student"),
        "set_surface" => {
            if too_long("surface", 2000) {
                return fake_refusal("invalid_parameter", "surface");
            }
            fields["surface"] = params["surface"].clone();
        }
        "bind_textbook" => merge(&mut fields, "binding"),
        "retire_binding" => {
            if !exists(state, ty, &key) {
                return fake_refusal("submission_criteria", "binding_missing");
            }
            fields["status"] = json!("retired");
        }
        "open_reminder" => {
            if too_long("text", 600) {
                return fake_refusal("invalid_parameter", "text");
            }
            merge(&mut fields, "reminder");
            fields["status"] = json!("open");
        }
        "close_reminder" => {
            if fields["status"] != "open" {
                return fake_refusal("submission_criteria", "reminder_not_open");
            }
            fields["status"] = json!("closed");
        }
        "write_lesson_note" => {
            if too_long("text", 8000) {
                return fake_refusal("invalid_parameter", "text");
            }
            merge(&mut fields, "note");
        }
        _ => {
            fields["memory_as_of"] = params["lesson_date"].clone();
            fields["updated_at"] = params["updated_at"].clone();
        }
    }
    let counter = state["counter"].as_u64().unwrap() + 1;
    state["counter"] = json!(counter);
    state["objects"][&slot] = json!({"head": format!("r{counter}"), "fields": fields});
    state["applied"]
        .as_array_mut()
        .unwrap()
        .push(json!({"action": action, "key": key}));
    applied("applied")
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
    /// A stage whose engine is the fake store, empty.
    fn fake() -> Self {
        let stage = Self::new();
        fs::write(
            stage.dir.join("fake.json"),
            json!({"counter": 0, "objects": {}, "applied": []}).to_string(),
        )
        .unwrap();
        stage
    }
    /// The fake store as it is now.
    fn store(&self) -> Value {
        serde_json::from_slice(&fs::read(self.dir.join("fake.json")).unwrap()).unwrap()
    }
    /// The fields of one object of the fake store.
    fn stored(&self, ty: &str, key: &str) -> Option<Value> {
        self.store()["objects"]
            .get(fake_slot(ty, key))
            .map(|o| o["fields"].clone())
    }
    /// The actions applied so far, in order.
    fn applied_actions(&self) -> Vec<String> {
        self.store()["applied"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["action"].as_str().unwrap().to_owned())
            .collect()
    }
    /// A workspace of the stage's own: the files under `ws/`; its path.
    fn workspace(&self, files: &[(&str, &str)]) -> PathBuf {
        let root = self.dir.join("ws");
        for (name, text) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        root
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
        "stu-a\tclass_slot=Slot One\tmemory_as_of=2040-01-02\nstu-b\tclass_slot=none\tmemory_as_of=-\n"
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

// ---------------------------------------------------------------- import

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .display()
        .to_string()
}

/// Every file under a directory with its bytes, by relative path.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let name = path.strip_prefix(root).unwrap().display().to_string();
                out.insert(name, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn parse_entries(report: &Value) -> Vec<(String, u64)> {
    report["parse"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["path"].as_str().unwrap().to_owned(),
                p["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

/// What the fixture `import-class` holds that cannot be read, in the order
/// the import finds it.
fn class_parse() -> Vec<(String, u64)> {
    [
        ("students/stu-a/textbook-ledger.md", 11),
        ("students/stu-a/textbook-ledger.md", 18),
        ("students/stu-a/profile.md", 7),
        ("students/stu-a/progress-log.md", 22),
        ("students/REGISTRY.md", 14),
        ("students/REGISTRY.md", 15),
        ("students/stu-x", 0),
    ]
    .map(|(path, line)| (path.to_owned(), line))
    .to_vec()
}

fn import_dry_run_plans_every_write_and_calls_nothing() {
    let stage = Stage::new();
    let no_engine = stage.dir.join("no-engine");
    let dir = fixture("import-class");
    let out = stage.run_with(&no_engine, &["import", "--dry-run", &dir]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let report = out.json();
    assert_eq!(
        report["students"],
        json!({
            "stu-a": {"planned": {
                "register_student": 1, "set_surface": 1, "bind_textbook": 4,
                "retire_binding": 1, "open_reminder": 2, "close_reminder": 1,
                "write_lesson_note": 2, "close_lesson_day": 1,
            }, "staging_lines": 3},
            "stu-b": {"planned": {
                "register_student": 1, "open_reminder": 1,
                "write_lesson_note": 1, "close_lesson_day": 1,
            }, "staging_lines": 0},
            "stu-c": {"planned": {
                "register_student": 1, "bind_textbook": 1,
                "write_lesson_note": 1, "close_lesson_day": 1,
            }, "staging_lines": 0},
            "stu-d": {"planned": {"register_student": 1}, "staging_lines": 0},
            "stu-f": {"planned": {"register_student": 1}, "staging_lines": 0},
        })
    );
    assert_eq!(
        report["class_reminders"],
        json!({"planned": {"open_reminder": 2, "close_reminder": 1}})
    );
    assert_eq!(parse_entries(&report), class_parse());
    let reasons: Vec<&str> = report["parse"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["reason"].as_str().unwrap())
        .collect();
    assert!(reasons[4].contains("GRADUATED"), "{reasons:?}");
    assert!(
        reasons[5].contains("students/stu-f is missing"),
        "{reasons:?}"
    );
    assert!(reasons[6].contains("no row"), "{reasons:?}");
    // The same directory with the engine's help applies nothing either.
    assert_eq!(stage.calls(), 0);
    assert!(out.stderr.is_empty());
}

fn import_applies_every_form_and_a_second_run_changes_nothing() {
    let stage = Stage::fake();
    let dir = fixture("import-class");
    let out = stage.run(&["import", &dir]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let first = out.json();
    assert_eq!(
        first["students"],
        json!({
            "stu-a": {"created": 11, "already_applied": 0, "refused": 0, "staging_lines": 3},
            "stu-b": {"created": 4, "already_applied": 0, "refused": 0, "staging_lines": 0},
            "stu-c": {"created": 4, "already_applied": 0, "refused": 0, "staging_lines": 0},
            "stu-d": {"created": 1, "already_applied": 0, "refused": 0, "staging_lines": 0},
            "stu-f": {"created": 1, "already_applied": 0, "refused": 0, "staging_lines": 0},
        })
    );
    assert_eq!(
        first["class_reminders"],
        json!({"created": 2, "already_applied": 0, "refused": 0})
    );
    assert_eq!(parse_entries(&first), class_parse());

    // The student, with the status in lower case and the class slot.
    let a = stage.stored("student", "stu-a").unwrap();
    assert_eq!(a["status"], "active");
    assert_eq!(a["name"], "Invented Alpha");
    assert_eq!(a["student_id"], "1001");
    assert_eq!(a["class_slot"], "Slot One");
    assert_eq!(a["memory_as_of"], "2040-01-05");
    assert_eq!(
        a["surface"],
        "- Works on invented unit 3.\n  - nested remark\n- Prefers short drills."
    );
    assert_eq!(
        stage.stored("student", "stu-b").unwrap()["status"],
        "onboarding"
    );
    let d = stage.stored("student", "stu-d").unwrap();
    assert!(d.get("memory_as_of").is_none() && d.get("student_id").is_none());
    assert!(stage.stored("student", "stu-e").is_none());
    assert!(stage.stored("student", "stu-f").is_some());

    // The bindings: a label table beside the bindings table, two tags in one
    // cell, a candidate, a retired one, and a time that is not a UTC time.
    let b = |book: &str| {
        stage
            .stored("textbook_binding", &format!("stu-a|{book}"))
            .unwrap()
    };
    assert_eq!(
        b("1001"),
        json!({
            "student": id("student", "stu-a"), "book_no": "1001",
            "book_label": "Invented Book 1 (exact label, 2nd ed.)", "course_key": "CK-1B",
            "role": "main", "status": "active", "provenance": "teacher_confirmed",
            "observed_at": "2040-01-02T08:00:00Z",
        })
    );
    assert_eq!(b("1002")["status"], "candidate");
    assert!(b("1002").get("observed_at").is_none());
    assert_eq!(b("1003")["status"], "retired");
    assert_eq!(b("1003")["book_label"], "Invented Book 3");
    assert_eq!(b("1004")["status"], "active");
    assert!(b("1004").get("course_key").is_none());
    assert!(stage.stored("textbook_binding", "stu-a|1005").is_none());

    // The reminders: of the class and of a student, each form of id.
    let class = stage.stored("reminder", "R-20400101-CLASSA").unwrap();
    assert_eq!(
        class,
        json!({"text": "Bring the invented seating chart to the class.", "provenance": "local_state", "status": "open"})
    );
    let closed = stage.stored("reminder", "R-20400102-001").unwrap();
    assert_eq!(closed["status"], "closed");
    assert_eq!(closed["text"], "Reminder return the invented key.");
    assert!(closed.get("student").is_none());
    let sheet = stage.stored("reminder", "R-20400103-SHEET").unwrap();
    assert_eq!(sheet["provenance"], "teacher_confirmed");
    assert_eq!(sheet["student"], id("student", "stu-a"));
    assert_eq!(sheet["status"], "open");
    let card = stage.stored("reminder", "R-20400104-007").unwrap();
    assert_eq!(
        (card["status"].as_str(), card["provenance"].as_str()),
        (Some("closed"), Some("local_state"))
    );
    assert!(stage.stored("reminder", "r-bad-id").is_none());
    for never_read in ["R-20400199-TEMPLATE", "R-20400107-GONE", "R-20400108-STRAY"] {
        assert!(
            stage.stored("reminder", never_read).is_none(),
            "{never_read}"
        );
    }

    // The notes: sections word for word, two of one date joined by a blank
    // line, nested bullets and tags untouched, the other headings left out.
    let note = stage.stored("lesson_note", "stu-a|2040-01-02").unwrap();
    assert_eq!(
        note["text"],
        "## 2040-01-02 Lesson one\n\n- Reviewed [PRESTUDY @ 2040-01-02T09:00:00Z] the invented items\n  - nested: record rec-1 [DAILYTEST @ 2040-01-02T10:00:00Z] 8 of 10\n    - deeper prose, no tag\n- plain bullet\n\n## 2040-01-02\n\n- A second section of the same date."
    );
    assert_eq!(note["provenance"], "local_state");
    assert_eq!(
        stage.stored("lesson_note", "stu-a|2040-01-05").unwrap()["text"],
        "## 2040-01-05\n\n- A later lesson."
    );
    assert!(stage.stored("lesson_note", "stu-a|2040-13-45").is_none());
    assert_eq!(
        stage.stored("lesson_note", "stu-c|2040-01-04").unwrap()["text"],
        "## 2040-01-04 only the heading"
    );

    // A second run of the unchanged directory creates nothing and changes no
    // revision.
    let before = stage.store()["objects"].clone();
    let applied = stage.applied_actions().len();
    let out = stage.run(&["import", &dir]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let second = out.json();
    assert_eq!(
        second["students"],
        json!({
            "stu-a": {"created": 0, "already_applied": 11, "refused": 0, "staging_lines": 3},
            "stu-b": {"created": 0, "already_applied": 4, "refused": 0, "staging_lines": 0},
            "stu-c": {"created": 0, "already_applied": 4, "refused": 0, "staging_lines": 0},
            "stu-d": {"created": 0, "already_applied": 1, "refused": 0, "staging_lines": 0},
            "stu-f": {"created": 0, "already_applied": 1, "refused": 0, "staging_lines": 0},
        })
    );
    assert_eq!(
        second["class_reminders"],
        json!({"created": 0, "already_applied": 2, "refused": 0})
    );
    assert_eq!(stage.store()["objects"], before);
    assert_eq!(stage.applied_actions().len(), applied);
}

fn import_then_roster_and_brief_show_what_was_read() {
    let stage = Stage::fake();
    let out = stage.run(&["import", &fixture("import-class")]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let roster = stage.run(&["roster"]);
    assert_eq!(
        roster.stdout,
        "stu-a\tclass_slot=Slot One\tmemory_as_of=2040-01-05\n\
         stu-b\tclass_slot=Slot Two\tmemory_as_of=2040-01-03\n\
         stu-c\tclass_slot=Slot One\tmemory_as_of=2040-01-04\n\
         stu-d\tclass_slot=Slot Three\tmemory_as_of=-\n\
         stu-f\tclass_slot=Slot Two\tmemory_as_of=-\n"
    );
    let brief = stage.run(&["brief", "stu-a"]).stdout;
    assert!(
        brief.contains(
            "- book_no=1001 course_key=CK-1B label=Invented Book 1 (exact label, 2nd ed.)\n"
        ),
        "{brief}"
    );
    assert!(!brief.contains("book_no=1003"), "{brief}");
    assert!(
        brief.contains("- 2040-01-05 local_state: ## 2040-01-05  - A later lesson.\n"),
        "{brief}"
    );
    assert!(brief.contains("R-20400103-SHEET (student)"), "{brief}");
    assert!(brief.contains("R-20400101-CLASSA (class)"), "{brief}");
    assert!(!brief.contains("R-20400104-007"), "{brief}");
}

fn import_counts_a_refusal_and_goes_on() {
    let stage = Stage::fake();
    let long = "x".repeat(700);
    let profile = format!(
        "| ID | Status | Summary | Provenance | Updated At |\n|---|---|---|---|---|\n| R-20400201-LONG | OPEN | {long} | PLANNED | 2040-02-01 |\n| R-20400202-001 | OPEN | A short one. | PLANNED | 2040-02-01 |\n"
    );
    let dir = stage.workspace(&[
        (
            "students/REGISTRY.md",
            "| Slug | Student Name | studentId | courseId | membershipId | Status | Class Slot | Directory |\n|---|---|---|---|---|---|---|---|\n| stu-r | Invented Rho | 1 | 2 | 3 | active | Slot | students/stu-r |\n",
        ),
        ("students/stu-r/profile.md", &profile),
        ("students/stu-r/progress-log.md", "## 2040-02-01\n- lesson\n"),
    ]);
    let out = stage.run(&["import", &dir.display().to_string()]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let report = out.json();
    assert_eq!(
        report["students"]["stu-r"],
        json!({"created": 4, "already_applied": 0, "refused": 1, "staging_lines": 0})
    );
    assert_eq!(report["parse"], json!([]));
    let line: Value = serde_json::from_str(out.stderr.trim()).unwrap();
    assert_eq!(line["refused"]["code"], "invalid_parameter");
    assert_eq!(line["refused"]["owner"], "stu-r");
    assert!(!out.stderr.contains(&long));
    assert!(stage.stored("reminder", "R-20400201-LONG").is_none());
    assert!(stage.stored("reminder", "R-20400202-001").is_some());
    assert!(stage.stored("lesson_note", "stu-r|2040-02-01").is_some());
}

fn import_cuts_a_long_note_and_a_long_surface() {
    let stage = Stage::fake();
    let long_note = format!("## 2040-03-01\n{}\n", "가".repeat(3000));
    let long_surface = format!("## Current Surface\n- {}\n", "나".repeat(1000));
    let dir = stage.workspace(&[
        (
            "students/REGISTRY.md",
            "| Slug | Student Name | studentId | courseId | membershipId | Status | Class Slot | Directory |\n|---|---|---|---|---|---|---|---|\n| stu-l | Invented Lambda | 1 | 2 | 3 | active | Slot | students/stu-l |\n",
        ),
        ("students/stu-l/progress-log.md", &long_note),
        ("students/stu-l/CONTEXT.md", &long_surface),
    ]);
    let out = stage.run(&["import", &dir.display().to_string()]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let text = stage.stored("lesson_note", "stu-l|2040-03-01").unwrap()["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(text.len() <= 8000 && text.len() > 7990, "{}", text.len());
    assert!(text.ends_with("\n(cut)"));
    assert!(text.starts_with("## 2040-03-01\n가가"));
    let surface = stage.stored("student", "stu-l").unwrap()["surface"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        surface.len() <= 2000 && surface.len() > 1990,
        "{}",
        surface.len()
    );
    assert!(surface.starts_with("- 나나"));
}

fn import_leaves_the_directory_as_it_was() {
    let stage = Stage::fake();
    for name in ["import-class", "import-clean"] {
        let dir = fixture(name);
        let before = tree(Path::new(&dir));
        assert!(!before.is_empty());
        stage.run(&["import", &dir]);
        stage.run(&["import", &dir, "--dry-run"]);
        assert_eq!(tree(Path::new(&dir)), before, "{name}");
    }
}

fn import_without_an_engine_stops_with_engine_missing() {
    let stage = Stage::new();
    let out = stage.run_with(
        &stage.dir.join("no-engine"),
        &["import", &fixture("import-clean")],
    );
    assert_eq!(out.code, 3, "{}", out.stderr);
    assert_eq!(out.error()["code"], "engine_missing");
}

fn import_refuses_bad_arguments_and_a_directory_without_a_registry() {
    let stage = Stage::fake();
    let empty = stage.workspace(&[("students/stu-a/profile.md", "x")]);
    let empty = empty.display().to_string();
    let missing = stage.dir.join("not-there").display().to_string();
    let no_table = stage.workspace(&[("other/students/REGISTRY.md", "no table\n")]);
    let no_table = no_table.join("other").display().to_string();
    for (args, code) in [
        (vec!["import"], "usage"),
        (vec!["import", "a", "b"], "usage"),
        (vec!["import", "--bogus", "a"], "usage"),
        (vec!["import", "--dry-run", "--dry-run", "a"], "usage"),
        (vec!["import", empty.as_str()], "invalid_input"),
        (vec!["import", missing.as_str()], "invalid_input"),
        (vec!["import", no_table.as_str()], "invalid_input"),
    ] {
        let out = stage.run(&args);
        assert_eq!(out.code, 2, "{args:?} {}", out.stderr);
        assert_eq!(out.error()["code"], code, "{args:?}");
        assert!(out.stdout.is_empty());
    }
    assert_eq!(stage.applied_actions().len(), 0);
}

fn import_of_a_clean_directory_exits_zero_twice() {
    let stage = Stage::fake();
    let dir = fixture("import-clean");
    for (created, already) in [(4, 0), (0, 4)] {
        let out = stage.run(&["import", &dir]);
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert_eq!(
            out.json(),
            json!({
                "students": {"stu-z": {"created": created, "already_applied": already, "refused": 0, "staging_lines": 0}},
                "class_reminders": {"created": 0, "already_applied": 0, "refused": 0},
                "parse": [],
            })
        );
    }
    let out = stage.run(&["import", &dir, "--dry-run"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(out.json()["parse"], json!([]));
}

fn import_writes_only_the_close_for_a_reminder_closed_later() {
    let stage = Stage::fake();
    let header = "| ID | Status | Summary | Provenance | Updated At |\n|---|---|---|---|---|\n";
    let registry = "| Slug | Student Name | studentId | courseId | membershipId | Status | Class Slot | Directory |\n|---|---|---|---|---|---|---|---|\n| stu-q | Invented Qi | 1 | 2 | 3 | active | Slot | students/stu-q |\n";
    let open = format!("{header}| R-20400301-QQ | OPEN | Bring it. | PLANNED | 2040-03-01 |\n");
    let dir = stage.workspace(&[
        ("students/REGISTRY.md", registry),
        ("students/stu-q/profile.md", &open),
    ]);
    let arg = dir.display().to_string();
    assert_eq!(stage.run(&["import", &arg]).code, 0);
    let closed = open.replace("| OPEN |", "| CLOSED |");
    fs::write(dir.join("students/stu-q/profile.md"), &closed).unwrap();
    let before = stage.applied_actions().len();
    let out = stage.run(&["import", &arg]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(out.json()["students"]["stu-q"]["created"], 1);
    assert_eq!(out.json()["students"]["stu-q"]["already_applied"], 1);
    assert_eq!(stage.applied_actions()[before..], ["close_reminder"]);
    assert_eq!(
        stage.stored("reminder", "R-20400301-QQ").unwrap()["status"],
        "closed"
    );
    // Closed with the same values: neither write is made again.
    let out = stage.run(&["import", &arg]);
    assert_eq!(out.json()["students"]["stu-q"]["created"], 0);
    assert_eq!(stage.applied_actions().len(), before + 1);
    // The same id with other values after it was closed is written again.
    fs::write(
        dir.join("students/stu-q/profile.md"),
        closed.replace("Bring it.", "Bring it twice."),
    )
    .unwrap();
    let out = stage.run(&["import", &arg]);
    assert_eq!(out.json()["students"]["stu-q"]["created"], 1);
    assert_eq!(
        stage.applied_actions()[before + 1..],
        ["open_reminder", "close_reminder"]
    );
}
