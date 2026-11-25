//! The schema `academy_memory` checked, installed and applied by the pinned
//! engine on disposable stores: invented students and texts only.
use edu_academy_ontology::ACADEMY_MEMORY_SCHEMA;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const NOW: u64 = 2_000_000_000;

fn namespace() -> Value {
    json!({"instance":"vps://palantirkc","account":"root"})
}
fn id(ty: &str, key: &str) -> Value {
    json!({"object_type":ty,"namespace":namespace(),"native_key":key})
}
fn student(slug: &str) -> Value {
    id("student", slug)
}
fn utc(minute: u32) -> String {
    format!("2040-01-01T{:02}:{:02}:00Z", minute / 60, minute % 60)
}

struct Env {
    root: PathBuf,
    rid: Cell<u64>,
}
impl Env {
    fn new() -> Self {
        let parent = PathBuf::from(
            std::env::var("S10_TEST_ROOT").expect("explicit synthetic root required"),
        );
        let root = parent.join(format!(
            "academy-memory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .unwrap();
        let env = Self {
            root,
            rid: Cell::new(0),
        };
        let file = env.root.join("academy-memory.schema.json");
        fs::write(&file, ACADEMY_MEMORY_SCHEMA).unwrap();
        let file = file.display().to_string();
        let (code, ans) = env.run(&["schema", "check", &file], b"");
        assert_eq!(code, 0, "{ans}");
        assert_eq!(ans["clean"], json!(true), "{ans}");
        assert_eq!(ans["violations"], json!([]), "{ans}");
        let (code, ans) = env.run(&["schema", "install", &file], b"");
        assert_eq!(code, 0, "{ans}");
        assert_eq!(
            ans["result"]["pin"],
            json!({"domain":"academy_memory","version":1,"sha256":format!("{:x}", Sha256::digest(ACADEMY_MEMORY_SCHEMA.as_bytes()))}),
            "{ans}"
        );
        env
    }
    fn run(&self, args: &[&str], stdin: &[u8]) -> (i32, Value) {
        let mut all: Vec<String> = vec![
            "--store".into(),
            self.root.join("store.sqlite").display().to_string(),
            "--now".into(),
            NOW.to_string(),
        ];
        all.extend(args.iter().map(|s| (*s).to_string()));
        let out = ont::run(&all, stdin, 0, 0);
        assert_eq!(out.stderr, "");
        (out.code, serde_json::from_str(out.stdout.trim()).unwrap())
    }
    fn apply(&self, action: &str, parameters: Value) -> Value {
        let n = self.rid.get() + 1;
        self.rid.set(n);
        let body = json!({"action_type":action,"parameters":parameters,"request_id":format!("memory-{n}")});
        self.run(
            &["action", "apply", "--client", "edu-academy"],
            body.to_string().as_bytes(),
        )
        .1
    }
    fn applied(&self, action: &str, parameters: Value) {
        let ans = self.apply(action, parameters);
        assert_eq!(ans["result"]["outcome"], "applied", "{action}: {ans}");
    }
    fn refused(&self, action: &str, parameters: Value, code: &str, failed: &[&str]) {
        let ans = self.apply(action, parameters);
        assert_eq!(ans["status"], "refused", "{action}: {ans}");
        assert_eq!(ans["code"], code, "{action}: {ans}");
        if !failed.is_empty() {
            assert_eq!(ans["failed"], json!(failed), "{action}: {ans}");
        }
    }
    /// The rows of a view after its two header lines.
    fn rows(&self, view: &str, args: Value) -> Vec<String> {
        let req = json!({"op":"render","view":view,"args":args});
        let (code, ans) = self.run(&["render"], req.to_string().as_bytes());
        assert_eq!(code, 0, "{ans}");
        let result = if ans["result"].is_object() {
            &ans["result"]
        } else {
            &ans
        };
        let md = result["markdown"].as_str().unwrap();
        let rows: Vec<String> = md
            .lines()
            .filter(|l| l.starts_with('|'))
            .skip(2)
            .map(str::to_string)
            .collect();
        assert_eq!(
            result["rows"].as_u64().unwrap() as usize,
            rows.len(),
            "{md}"
        );
        rows
    }
    fn register(&self, slug: &str, status: &str) {
        self.applied(
            "register_student",
            json!({"student":student(slug),"slug":slug,"name":format!("Invented {slug}"),"status":status,"student_id":"77001","class_slot":"Mon 16","updated_at":utc(1)}),
        );
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn file_declares_the_interface() {
    let s: Value = serde_json::from_str(ACADEMY_MEMORY_SCHEMA).unwrap();
    assert_eq!(s["format"], "ontology-schema/v1");
    assert_eq!(s["domain"], "academy_memory");
    assert_eq!(s["version"], 1);
    assert_eq!(s["namespace"], namespace());
    assert_eq!(s["scope_keys"], json!([]));
    assert_eq!(s["sources"], json!({}));
    let read: Value = serde_json::from_str(edu_academy_ontology::ACADEMY_READ_SCHEMA).unwrap();
    for shared in ["iso_date", "key_digits"] {
        assert_eq!(s["value_types"][shared], read["value_types"][shared]);
    }
    let names = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    let mut types = vec![
        "student",
        "textbook_binding",
        "observation",
        "lesson_note",
        "reminder",
        "counseling",
    ];
    types.sort();
    assert_eq!(names(&s["object_types"]), types);
    let mut actions = vec![
        "register_student",
        "set_surface",
        "bind_textbook",
        "retire_binding",
        "record_observation",
        "write_lesson_note",
        "open_reminder",
        "close_reminder",
        "prepare_counseling",
        "record_counseling",
        "close_counseling",
        "close_lesson_day",
    ];
    actions.sort();
    assert_eq!(names(&s["action_types"]), actions);
    let mut views = vec![
        "student_card",
        "student_bindings",
        "student_observations",
        "student_notes",
        "student_reminders",
        "student_counselings",
        "roster",
        "open_reminders",
    ];
    views.sort();
    assert_eq!(names(&s["views"]), views);
    let mut links = vec![
        "binding_student",
        "observation_student",
        "note_student",
        "reminder_student",
        "counseling_student",
    ];
    links.sort();
    assert_eq!(names(&s["link_types"]), links);
    let mut sorted_types: Vec<Value> = types.iter().map(|t| json!(t)).collect();
    sorted_types.sort_by_key(|v| v.to_string());
    let grants = s["grants"].as_array().unwrap();
    assert_eq!(grants.len(), 1);
    let mut grant = grants[0].clone();
    for list in ["read", "derive", "actions"] {
        let mut v = grant[list].as_array().unwrap().clone();
        v.sort_by_key(|x| x.to_string());
        grant[list] = json!(v);
    }
    assert_eq!(
        grant,
        json!({"principal":"agent","scope":{},"read":sorted_types,"derive":sorted_types,"actions":actions})
    );
    for (name, ty) in s["object_types"].as_object().unwrap() {
        for (prop, def) in ty["properties"].as_object().unwrap() {
            let bounded = def["max_bytes"].is_u64()
                || def["type"].get("ref").is_some()
                || def["type"].get("value_type").is_some();
            assert!(bounded, "{name}.{prop} has no bound");
        }
    }
    // No action calls anything outside the store: no function, no source.
    assert_eq!(s["functions"], json!({}));
    for (name, action) in s["action_types"].as_object().unwrap() {
        for rule in action["rules"].as_array().unwrap() {
            let kinds: Vec<&String> = rule.as_object().unwrap().keys().collect();
            assert!(
                kinds.iter().all(|k| [
                    "create_object",
                    "modify_object",
                    "create_or_modify_object",
                    "set"
                ]
                .contains(&k.as_str())),
                "{name}: {kinds:?}"
            );
        }
    }
}

#[test]
fn refusals_name_their_criterion() {
    let env = Env::new();
    let ghost = student("invented-ghost");
    let binding = id("textbook_binding", "invented-ghost|12");
    let counseling = id("counseling", "invented-ghost|2040-01-05");
    let missing = "submission_criteria";
    for (action, parameters) in [
        ("set_surface", json!({"student":ghost,"surface":"s"})),
        (
            "bind_textbook",
            json!({"binding":binding,"student":ghost,"book_no":"12","status":"active"}),
        ),
        (
            "record_observation",
            json!({"observation":id("observation","o-1"),"student":ghost,"kind":"dailyrecord","lesson_date":"2040-01-02","observed_at":utc(2),"summary":"s"}),
        ),
        (
            "write_lesson_note",
            json!({"note":id("lesson_note","invented-ghost|2040-01-02"),"student":ghost,"lesson_date":"2040-01-02","text":"t","provenance":"local_state"}),
        ),
        (
            "open_reminder",
            json!({"reminder":id("reminder","r-1"),"student":ghost,"text":"t"}),
        ),
        (
            "prepare_counseling",
            json!({"counseling":counseling,"student":ghost,"date":"2040-01-05"}),
        ),
        (
            "close_lesson_day",
            json!({"student":ghost,"lesson_date":"2040-01-02","updated_at":utc(3)}),
        ),
    ] {
        env.refused(action, parameters, missing, &["student_missing"]);
    }
    env.refused(
        "retire_binding",
        json!({"student":ghost,"binding":binding}),
        missing,
        &["student_missing", "binding_missing"],
    );
    env.register("invented-a", "active");
    let a = student("invented-a");
    env.refused(
        "retire_binding",
        json!({"student":a,"binding":id("textbook_binding","invented-a|12")}),
        missing,
        &["binding_missing"],
    );
    env.refused(
        "close_reminder",
        json!({"reminder":id("reminder","r-none")}),
        missing,
        &["reminder_not_open"],
    );
    env.applied(
        "open_reminder",
        json!({"reminder":id("reminder","r-class"),"text":"Bring the class sheets","due":"2040-01-09"}),
    );
    env.applied(
        "close_reminder",
        json!({"reminder":id("reminder","r-class"),"updated_at":utc(4)}),
    );
    env.refused(
        "close_reminder",
        json!({"reminder":id("reminder","r-class")}),
        missing,
        &["reminder_not_open"],
    );
    for action in ["record_counseling", "close_counseling"] {
        let mut parameters = json!({"counseling":id("counseling","invented-a|2040-01-05")});
        if action == "record_counseling" {
            parameters["result"] = json!("r");
        }
        env.refused(action, parameters, missing, &["counseling_missing"]);
    }
    let prepared = json!({"counseling":id("counseling","invented-a|2040-01-05"),"student":a,"date":"2040-01-05","purpose":"p"});
    env.applied("prepare_counseling", prepared.clone());
    let ans = env.apply("prepare_counseling", prepared);
    assert_eq!(ans["status"], "refused", "{ans}");
    env.refused(
        "record_observation",
        json!({"observation":id("observation","o-2"),"student":a,"kind":"dailyrecord","lesson_date":"2040-01-02","observed_at":utc(5),"summary":"x".repeat(1501)}),
        "invalid_parameter",
        &[],
    );
    env.refused(
        "register_student",
        json!({"student":student("invented-b"),"slug":"invented-b","status":"graduated"}),
        "invalid_parameter",
        &[],
    );
}

#[test]
fn views_read_the_memory_newest_first() {
    let env = Env::new();
    env.register("invented-a", "active");
    env.register("invented-b", "onboarding");
    env.register("invented-c", "paused");
    let a = student("invented-a");
    env.applied(
        "set_surface",
        json!({"student":a,"surface":"Starts from the second unit."}),
    );
    env.applied(
        "close_lesson_day",
        json!({"student":a,"lesson_date":"2040-01-20","updated_at":utc(30)}),
    );
    let card = env.rows("student_card", json!({"student":"invented-a"}));
    assert_eq!(
        card,
        [
            "| invented-a | Invented invented-a | active | Mon 16 | 77001 | (missing) | (missing) | 2040-01-20 | 2040-01-01T00:30:00Z | Starts from the second unit. |"
        ]
    );
    assert_eq!(
        env.rows("roster", json!({})),
        [
            "| invented-a | Invented invented-a | Mon 16 | 2040-01-20 |",
            "| invented-b | Invented invented-b | Mon 16 | (missing) |"
        ]
    );

    for (no, status) in [("12", "active"), ("13", "candidate"), ("14", "active")] {
        env.applied(
            "bind_textbook",
            json!({"binding":id("textbook_binding",&format!("invented-a|{no}")),"student":a,"book_no":no,"book_label":format!("Invented book {no}"),"status":status,"provenance":"server_observed","observed_at":utc(6)}),
        );
    }
    env.applied(
        "retire_binding",
        json!({"student":a,"binding":id("textbook_binding","invented-a|14")}),
    );
    let bindings = env.rows("student_bindings", json!({"student":"invented-a"}));
    assert_eq!(bindings.len(), 1, "{bindings:?}");
    assert!(
        bindings[0].starts_with("| invented-a\\|12 | 12 |"),
        "{bindings:?}"
    );

    for n in 0..13 {
        env.applied(
            "record_observation",
            json!({"observation":id("observation",&format!("o-{n}")),"student":a,"kind":"dailyrecord","lesson_date":"2040-01-02","observed_at":utc(100 + n),"tool":"invented_tool","record_id":format!("R{n}"),"summary":format!("Record {n}")}),
        );
    }
    let observations = env.rows("student_observations", json!({"student":"invented-a"}));
    assert_eq!(observations.len(), 12);
    assert!(
        observations[0].contains("| R12 | Record 12 |"),
        "{observations:?}"
    );
    assert!(
        observations[11].contains("| R1 | Record 1 |"),
        "{observations:?}"
    );
    assert!(
        env.rows("student_observations", json!({"student":"invented-b"}))
            .is_empty()
    );

    for day in ["2040-01-02", "2040-01-09", "2040-01-16", "2040-01-23"] {
        env.applied(
            "write_lesson_note",
            json!({"note":id("lesson_note",&format!("invented-a|{day}")),"student":a,"lesson_date":day,"text":format!("Note of {day}"),"provenance":"teacher_confirmed","written_at":utc(40)}),
        );
    }
    let notes = env.rows("student_notes", json!({"student":"invented-a"}));
    assert_eq!(notes.len(), 3);
    assert!(notes[0].starts_with("| 2040-01-23 |"), "{notes:?}");
    assert!(notes[2].starts_with("| 2040-01-09 |"), "{notes:?}");

    for (key, who) in [("r-1", Some(&a)), ("r-2", Some(&a)), ("r-class", None)] {
        let mut parameters =
            json!({"reminder":id("reminder",key),"text":format!("Remember {key}")});
        if let Some(who) = who {
            parameters["student"] = who.clone();
        }
        env.applied("open_reminder", parameters);
    }
    env.applied(
        "close_reminder",
        json!({"reminder":id("reminder","r-2"),"updated_at":utc(50)}),
    );
    let reminders = env.rows("student_reminders", json!({"student":"invented-a"}));
    assert_eq!(reminders.len(), 1);
    assert!(
        reminders[0].starts_with("| r-1 | Remember r-1 |"),
        "{reminders:?}"
    );
    let open = env.rows("open_reminders", json!({}));
    assert_eq!(open.len(), 2, "{open:?}");
    assert!(open[0].starts_with("| r-1 | invented-a |"), "{open:?}");
    assert!(open[1].starts_with("| r-class | (missing) |"), "{open:?}");

    for date in ["2040-01-05", "2040-02-05", "2040-03-05"] {
        env.applied(
            "prepare_counseling",
            json!({"counseling":id("counseling",&format!("invented-a|{date}")),"student":a,"date":date,"purpose":"Invented purpose","form":"Invented form"}),
        );
    }
    let latest = id("counseling", "invented-a|2040-03-05");
    env.applied(
        "record_counseling",
        json!({"counseling":latest,"result":"Invented result"}),
    );
    let counselings = env.rows("student_counselings", json!({"student":"invented-a"}));
    assert_eq!(counselings.len(), 2);
    assert!(
        counselings[0].starts_with("| 2040-03-05 | held | Invented purpose |"),
        "{counselings:?}"
    );
    assert!(
        counselings[0].ends_with("| Invented result |"),
        "{counselings:?}"
    );
    assert!(
        counselings[1].starts_with("| 2040-02-05 | prepared |"),
        "{counselings:?}"
    );
    env.applied("close_counseling", json!({"counseling":latest}));
    let counselings = env.rows("student_counselings", json!({"student":"invented-a"}));
    assert!(
        counselings[0].starts_with("| 2040-03-05 | closed |"),
        "{counselings:?}"
    );
}
