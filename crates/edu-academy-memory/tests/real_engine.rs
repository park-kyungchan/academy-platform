//! Every subcommand against the engine `EDU_ONT_BIN` names, on a scratch store
//! in the test's own temporary directory, with `ontology/academy-memory.schema.json`
//! installed by the engine's file form. Ignored by default:
//! `EDU_ONT_BIN=$(command -v ont) cargo test -p edu-academy-memory --test real_engine -- --ignored`.
//! Refusals are asserted by the code the engine passes on. Invented values only.
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_edu-academy-memory");
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    dir: PathBuf,
    store: PathBuf,
    engine: String,
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Out {
    fn json(&self) -> Value {
        assert_eq!(self.code, 0, "{}", self.stderr);
        serde_json::from_str(self.stdout.trim()).unwrap_or_else(|_| panic!("{}", self.stdout))
    }
    fn outcome(&self) -> String {
        self.json()["outcome"].as_str().unwrap().to_owned()
    }
    fn code_of_error(&self) -> String {
        assert_eq!(self.code, 2, "{} {}", self.stdout, self.stderr);
        let line: Value = serde_json::from_str(self.stderr.trim()).unwrap();
        line["error"]["code"].as_str().unwrap().to_owned()
    }
}

impl Scratch {
    fn new() -> Self {
        let engine = std::env::var("EDU_ONT_BIN").expect("EDU_ONT_BIN names the engine");
        let dir = std::env::temp_dir().join(format!(
            "edu-academy-memory-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .unwrap();
        let store = dir.join("store.db");
        let schema =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ontology/academy-memory.schema.json");
        let out = Command::new(&engine)
            .arg("--store")
            .arg(&store)
            .args(["schema", "install"])
            .arg(&schema)
            .output()
            .unwrap();
        let answer: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(answer["status"], "ok", "{answer}");
        Self { dir, store, engine }
    }
    fn run(&self, args: &[&str]) -> Out {
        let out = Command::new(BIN)
            .arg("--store")
            .arg(&self.store)
            .args(args)
            .env("EDU_ONT_BIN", &self.engine)
            .output()
            .unwrap();
        Out {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8(out.stdout).unwrap(),
            stderr: String::from_utf8(out.stderr).unwrap(),
        }
    }
    fn file(&self, name: &str, text: &str) -> String {
        let path = self.dir.join(name);
        fs::write(&path, text).unwrap();
        format!("@{}", path.display())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
#[ignore = "needs the engine EDU_ONT_BIN names"]
fn a_lesson_day_through_every_subcommand() {
    let s = Scratch::new();
    let out = s.run(&["student", "stu-a", "--name", "Invented Student"]);
    assert_eq!(out.code_of_error(), "usage");
    let register = [
        "student",
        "stu-a",
        "--name",
        "Invented Student",
        "--status",
        "active",
        "--class-slot",
        "slot-1",
        "--student-id",
        "1001",
    ];
    assert_eq!(s.run(&register).outcome(), "applied");
    assert_eq!(s.run(&register).outcome(), "already_applied");
    assert_eq!(
        s.run(&[
            "student",
            "stu-a",
            "--name",
            "Invented Student",
            "--class-slot",
            "slot-2"
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&[
            "student",
            "stu-b",
            "--name",
            "Second Student",
            "--status",
            "onboarding"
        ])
        .outcome(),
        "applied"
    );
    let surface = s.file(
        "surface.md",
        "- works in book 1234\n- likes invented puzzles\n",
    );
    assert_eq!(
        s.run(&["surface", "stu-a", "--text", &surface]).outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&["surface", "stu-a", "--text", &surface]).outcome(),
        "already_applied"
    );

    let bind = [
        "bind",
        "stu-a",
        "1234",
        "--label",
        "Invented Book 2-1 (full label)",
        "--course-key",
        "CK-12",
        "--role",
        "main",
        "--at",
        "2040-01-02T08:00:00Z",
        "--provenance",
        "server_observed",
    ];
    assert_eq!(
        s.run(&bind).json(),
        json!({"action": "bind_textbook", "key": "stu-a|1234", "outcome": "applied"})
    );
    assert_eq!(s.run(&bind).outcome(), "already_applied");
    assert_eq!(
        s.run(&[
            "bind",
            "stu-a",
            "99",
            "--label",
            "Old Invented Book",
            "--provenance",
            "planned",
            "--status",
            "candidate"
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(s.run(&["unbind", "stu-a", "99"]).outcome(), "applied");
    assert_eq!(
        s.run(&["unbind", "stu-a", "99"]).outcome(),
        "already_applied"
    );

    let observed = [
        "observed",
        "stu-a",
        "dailytest",
        "2040-01-02",
        "--at",
        "2040-01-02T09:00:00Z",
        "--summary",
        "8 of 10",
        "--tool",
        "invented_tool",
        "--record",
        "rec-1",
    ];
    assert_eq!(
        s.run(&observed).json(),
        json!({"action": "record_observation", "key": "stu-a|dailytest|2040-01-02|2040-01-02T09:00:00Z", "outcome": "applied"})
    );
    assert_eq!(s.run(&observed).outcome(), "already_applied");
    let summary = s.file("summary.txt", "read pages 3 to 5");
    assert_eq!(
        s.run(&[
            "observed",
            "stu-a",
            "prestudy",
            "2040-01-02",
            "--at",
            "2040-01-02T09:30:00Z",
            "--summary",
            &summary
        ])
        .outcome(),
        "applied"
    );
    let note = s.file("note.md", "An invented note of the day.\nSecond line.");
    assert_eq!(
        s.run(&[
            "note",
            "stu-a",
            "2040-01-02",
            "--provenance",
            "teacher_confirmed",
            "--text",
            &note
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&[
            "note",
            "stu-a",
            "2040-01-02",
            "--provenance",
            "teacher_confirmed",
            "--text",
            &note
        ])
        .outcome(),
        "already_applied"
    );

    assert_eq!(
        s.run(&[
            "remind",
            "open",
            "r-class",
            "--text",
            "invented class reminder",
            "--provenance",
            "planned"
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&[
            "remind",
            "open",
            "r-a",
            "--text",
            "invented own reminder",
            "--student",
            "stu-a",
            "--due",
            "2040-01-09",
            "--provenance",
            "teacher_confirmed"
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&[
            "remind",
            "open",
            "r-b",
            "--text",
            "someone else's",
            "--student",
            "stu-b",
            "--provenance",
            "planned"
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&[
            "remind",
            "open",
            "r-done",
            "--text",
            "done soon",
            "--provenance",
            "planned"
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(s.run(&["remind", "close", "r-done"]).outcome(), "applied");
    assert_eq!(
        s.run(&["remind", "close", "r-done"]).outcome(),
        "already_applied"
    );

    let form = s.file("form.md", "# Invented counseling form\n");
    let prepare = [
        "counsel",
        "prepare",
        "stu-a",
        "2040-01-05",
        "--purpose",
        "invented purpose",
        "--form",
        &form,
        "--stored-at",
        "counsel/stu-a-2040-01-05.md",
    ];
    assert_eq!(s.run(&prepare).outcome(), "applied");
    assert_eq!(s.run(&prepare).outcome(), "already_applied");
    let result = s.file("result.md", "Invented result.");
    assert_eq!(
        s.run(&[
            "counsel",
            "result",
            "stu-a",
            "2040-01-05",
            "--result",
            &result
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&[
            "counsel",
            "result",
            "stu-a",
            "2040-01-05",
            "--result",
            &result
        ])
        .outcome(),
        "already_applied"
    );
    assert_eq!(
        s.run(&["counsel", "close", "stu-a", "2040-01-05"])
            .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&["counsel", "close", "stu-a", "2040-01-05"])
            .outcome(),
        "already_applied"
    );

    let out = s.run(&["show", "counseling", "stu-a|2040-01-05"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout
            .contains("stored_at: counsel/stu-a-2040-01-05.md\n"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("status: closed\n"));
    let out = s.run(&["show", "student", "stu-a"]);
    assert!(out.stdout.contains("status: active\n"), "{}", out.stdout);
    assert!(out.stdout.contains("class_slot: slot-2\n"));
    assert!(
        out.stdout
            .contains("surface: - works in book 1234\n- likes invented puzzles\n")
    );
    let out = s.run(&[
        "show",
        "observation",
        "stu-a|dailytest|2040-01-02|2040-01-02T09:00:00Z",
    ]);
    assert!(out.stdout.contains("summary: 8 of 10\n"), "{}", out.stdout);
    assert!(out.stdout.contains("record_id: rec-1\n"));

    let out = s.run(&["brief", "stu-a"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let brief = out.stdout;
    assert!(brief.len() <= 6000);
    assert!(brief.contains("class_slot: slot-2\n"), "{brief}");
    assert!(
        brief.contains("- book_no=1234 course_key=CK-12 label=Invented Book 2-1 (full label)\n"),
        "{brief}"
    );
    assert!(!brief.contains("book_no=99"));
    assert!(brief.contains("- prestudy 2040-01-02 2040-01-02T09:30:00Z: read pages 3 to 5\n"));
    assert!(
        brief.contains(
            "- 2040-01-02 teacher_confirmed: An invented note of the day. Second line.\n"
        )
    );
    assert!(brief.contains("- r-a (student) due 2040-01-09: invented own reminder\n"));
    assert!(brief.contains("- r-class (class): invented class reminder\n"));
    assert!(!brief.contains("r-b") && !brief.contains("r-done"));
    assert!(brief.contains("- 2040-01-05 closed: invented purpose\n"));

    let out = s.run(&["roster"]);
    assert_eq!(
        out.stdout,
        "stu-a\tclass_slot=slot-2\tmemory_as_of=none\nstu-b\tclass_slot=none\tmemory_as_of=none\n"
    );

    let due = s.run(&["due", "stu-a", "2040-01-02"]).json();
    assert_eq!(
        due,
        json!({
            "student": "stu-a",
            "lesson_date": "2040-01-02",
            "memory_as_of": null,
            "have": [
                {"kind": "dailytest", "observed_at": "2040-01-02T09:00:00Z"},
                {"kind": "prestudy", "observed_at": "2040-01-02T09:30:00Z"}
            ],
            "due": ["dailyrecord", "section_results", "clinic"],
            "bindings": [{"book_no": "1234", "course_key": "CK-12", "book_label": "Invented Book 2-1 (full label)"}],
        })
    );

    let out = s.run(&["day-close", "2040-01-02", "stu-a", "stu-b", "stu-none"]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let lines: Vec<Value> = out
        .stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines[0],
        json!({"action": "close_lesson_day", "key": "stu-a", "outcome": "applied"})
    );
    assert_eq!(lines[1]["error"]["code"], "nothing_read");
    assert_eq!(lines[2]["error"]["code"], "student_missing");
    let out = s.run(&["day-close", "2040-01-02", "stu-a"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(out.json()["outcome"], "already_applied");
    assert_eq!(
        s.run(&["due", "stu-a", "2040-01-02"]).json()["memory_as_of"],
        "2040-01-02"
    );
    assert!(
        s.run(&["brief", "stu-a"])
            .stdout
            .contains("memory_as_of: 2040-01-02\n")
    );
}

#[test]
#[ignore = "needs the engine EDU_ONT_BIN names"]
fn refusals_carry_the_engines_code() {
    let s = Scratch::new();
    let out = s.run(&[
        "observed",
        "stu-x",
        "clinic",
        "2040-01-02",
        "--at",
        "2040-01-02T09:00:00Z",
        "--summary",
        "x",
    ]);
    assert_eq!(out.code_of_error(), "submission_criteria");
    assert!(out.stderr.contains("student_missing"));
    let out = s.run(&[
        "student",
        "stu-x",
        "--name",
        "Invented",
        "--status",
        "active",
        "--student-id",
        "abc",
    ]);
    assert_eq!(out.code_of_error(), "invalid_parameter");
    assert_eq!(
        s.run(&[
            "student", "stu-x", "--name", "Invented", "--status", "unknown"
        ])
        .code_of_error(),
        "invalid_parameter"
    );
    assert_eq!(
        s.run(&[
            "student", "stu-x", "--name", "Invented", "--status", "active"
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&["remind", "close", "r-none"]).code_of_error(),
        "submission_criteria"
    );
    assert_eq!(
        s.run(&["unbind", "stu-x", "5"]).code_of_error(),
        "submission_criteria"
    );
    assert_eq!(
        s.run(&[
            "counsel",
            "result",
            "stu-x",
            "2040-01-05",
            "--result",
            &s.file("r.md", "r")
        ])
        .code_of_error(),
        "submission_criteria"
    );
    let form = s.file("form.md", "form");
    assert_eq!(
        s.run(&[
            "counsel",
            "prepare",
            "stu-x",
            "2040-01-05",
            "--purpose",
            "one",
            "--form",
            &form
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&[
            "counsel",
            "prepare",
            "stu-x",
            "2040-01-05",
            "--purpose",
            "two",
            "--form",
            &form
        ])
        .code_of_error(),
        "revision_conflict"
    );
    assert_eq!(
        s.run(&[
            "note",
            "stu-x",
            "2040-01-02",
            "--provenance",
            "nonsense",
            "--text",
            "t"
        ])
        .code_of_error(),
        "invalid_parameter"
    );
    assert_eq!(
        s.run(&["brief", "stu-none"]).code_of_error(),
        "student_missing"
    );
    assert_eq!(
        s.run(&["due", "stu-none", "2040-01-02"]).code_of_error(),
        "student_missing"
    );
    assert_eq!(
        s.run(&["show", "reminder", "r-none"]).code_of_error(),
        "unavailable"
    );
    assert_eq!(
        s.run(&["show", "lesson", "x"]).code_of_error(),
        "invalid_input"
    );
}

#[test]
#[ignore = "needs the engine EDU_ONT_BIN names"]
fn brief_stays_within_6000_bytes_on_a_full_store() {
    let s = Scratch::new();
    assert_eq!(
        s.run(&[
            "student",
            "stu-f",
            "--name",
            "Invented Full",
            "--status",
            "active",
            "--class-slot",
            &"c".repeat(100)
        ])
        .outcome(),
        "applied"
    );
    assert_eq!(
        s.run(&["surface", "stu-f", "--text", &"가".repeat(666)])
            .outcome(),
        "applied"
    );
    for book in 1..=10 {
        let book = book.to_string();
        assert_eq!(
            s.run(&[
                "bind",
                "stu-f",
                &book,
                "--label",
                &"L".repeat(600),
                "--course-key",
                &"K".repeat(40),
                "--provenance",
                "server_observed"
            ])
            .outcome(),
            "applied"
        );
    }
    let long = s.file("long.txt", &"o".repeat(1500));
    for day in 1..=20 {
        let date = format!("2040-01-{day:02}");
        let at = format!("{date}T10:00:00Z");
        assert_eq!(
            s.run(&[
                "observed",
                "stu-f",
                "dailyrecord",
                &date,
                "--at",
                &at,
                "--summary",
                &long
            ])
            .outcome(),
            "applied"
        );
    }
    let note = s.file("note.txt", &"n".repeat(8000));
    for day in 1..=5 {
        let date = format!("2040-01-{day:02}");
        assert_eq!(
            s.run(&[
                "note",
                "stu-f",
                &date,
                "--provenance",
                "local_state",
                "--text",
                &note
            ])
            .outcome(),
            "applied"
        );
    }
    for r in 0..30 {
        let id = format!("r-{r:02}");
        assert_eq!(
            s.run(&[
                "remind",
                "open",
                &id,
                "--text",
                &"t".repeat(600),
                "--student",
                "stu-f",
                "--due",
                "2040-02-01",
                "--provenance",
                "planned"
            ])
            .outcome(),
            "applied"
        );
    }
    let form = s.file("form.md", "form");
    for day in 1..=4 {
        let date = format!("2040-03-{day:02}");
        assert_eq!(
            s.run(&[
                "counsel",
                "prepare",
                "stu-f",
                &date,
                "--purpose",
                &"p".repeat(600),
                "--form",
                &form
            ])
            .outcome(),
            "applied"
        );
    }
    let out = s.run(&["brief", "stu-f"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let brief = out.stdout;
    assert!(brief.len() <= 6000, "{}", brief.len());
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
    assert!(
        parts[2]
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("- dailyrecord 2040-01-20 2040-01-20T10:00:00Z: ooo")
    );
    assert!(
        parts[3]
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("- 2040-01-05 local_state: nnn")
    );
    assert!(parts[4].trim_end().ends_with("more)"));
}

#[test]
#[ignore = "needs the engine EDU_ONT_BIN names"]
fn import_twice_creates_nothing_the_second_time() {
    let s = Scratch::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import-class");
    let fixture = fixture.to_str().unwrap();
    let report = |out: &Out| -> Value {
        // The fixture holds rows that cannot be read: exit 1, a report.
        assert_eq!(out.code, 1, "{}", out.stderr);
        serde_json::from_str(out.stdout.trim()).unwrap_or_else(|_| panic!("{}", out.stdout))
    };
    let first = report(&s.run(&["import", fixture]));
    for (slug, student) in first["students"].as_object().unwrap() {
        assert_eq!(student["refused"], 0, "{slug}: {first}");
    }
    assert_eq!(first["class_reminders"]["refused"], 0, "{first}");
    assert_eq!(first["students"]["stu-a"]["created"], 11, "{first}");
    assert_eq!(first["parse"].as_array().unwrap().len(), 7, "{first}");

    let before = s.run(&["show", "student", "stu-a"]).stdout;
    let second = report(&s.run(&["import", fixture]));
    let students = second["students"].as_object().unwrap();
    assert_eq!(students.len(), 5, "{second}");
    for (slug, student) in students {
        assert_eq!(student["created"], 0, "{slug}: {second}");
        assert_eq!(student["refused"], 0, "{slug}: {second}");
        assert!(student["already_applied"].as_u64().unwrap() > 0, "{slug}");
    }
    assert_eq!(
        second["class_reminders"],
        json!({"created": 0, "already_applied": 2, "refused": 0})
    );
    // No object's revision moved.
    assert_eq!(s.run(&["show", "student", "stu-a"]).stdout, before);
    let roster = s.run(&["roster"]).stdout;
    assert!(
        roster.contains("stu-a\tclass_slot=Slot One\tmemory_as_of=2040-01-05\n"),
        "{roster}"
    );
    assert!(
        roster.contains("stu-d\tclass_slot=Slot Three\tmemory_as_of=none\n"),
        "{roster}"
    );
    let brief = s.run(&["brief", "stu-a"]).stdout;
    assert!(brief.contains("book_no=1001 course_key=CK-1B"), "{brief}");
}
