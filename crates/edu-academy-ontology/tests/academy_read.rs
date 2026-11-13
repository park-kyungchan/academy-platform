//! The schema `academy_read` on disposable stores: invented records only.
//! The engine offers no override of a source glob, so a test redirects the
//! spool by replacing the root string the schema names (`ACADEMY_READ_SPOOL_ROOT`)
//! in the schema text before `ont schema install` into a temporary store.
use edu_academy_ontology::{ACADEMY_READ_SCHEMA, ACADEMY_READ_SPOOL_ROOT};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Env {
    root: PathBuf,
}
impl Env {
    /// A new private directory under the synthetic root `./check` supplies, with
    /// an empty spool (both partition directories) and the schema installed
    /// with the spool root redirected into it.
    fn new() -> Self {
        let parent = PathBuf::from(
            std::env::var("S10_TEST_ROOT").expect("explicit synthetic root required"),
        );
        let root = parent.join(format!(
            "academy-read-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        for dir in ["spool/registered_book", "spool/section_attempt"] {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(root.join(dir))
                .unwrap();
        }
        let env = Self { root };
        let text = ACADEMY_READ_SCHEMA.replace(
            ACADEMY_READ_SPOOL_ROOT,
            &env.root.join("spool").display().to_string(),
        );
        assert_ne!(text, ACADEMY_READ_SCHEMA, "the schema names the spool root");
        let file = env.root.join("schema.json");
        fs::write(&file, text).unwrap();
        let (code, ans) = env.run(1, &["schema", "install", &file.display().to_string()]);
        assert_eq!(code, 0, "{ans}");
        env
    }
    fn store(&self) -> String {
        self.root.join("store.sqlite").display().to_string()
    }
    fn partition(&self, kind: &str, student: &str) -> PathBuf {
        self.root
            .join("spool")
            .join(kind)
            .join(format!("{student}.jsonl"))
    }
    fn put(&self, kind: &str, student: &str, lines: &[Value]) {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        fs::write(self.partition(kind, student), text).unwrap();
    }
    fn run(&self, now: u64, args: &[&str]) -> (i32, Value) {
        self.run_with(now, args, b"")
    }
    fn run_with(&self, now: u64, args: &[&str], stdin: &[u8]) -> (i32, Value) {
        let mut all: Vec<String> = vec![
            "--store".into(),
            self.store(),
            "--now".into(),
            now.to_string(),
        ];
        all.extend(args.iter().map(|s| (*s).to_string()));
        let out = ont::run(&all, stdin, 0, 0);
        assert_eq!(out.stderr, "");
        (out.code, serde_json::from_str(out.stdout.trim()).unwrap())
    }
    fn index(&self, now: u64) -> Value {
        let (code, ans) = self.run(now, &["index"]);
        assert_eq!(code, 0, "{ans}");
        ans
    }
    /// An index run whose exit code the caller reads (a failed source exits non-zero).
    fn index_code(&self, now: u64) -> (i32, Value) {
        self.run(now, &["index"])
    }
    fn render(&self, now: u64, view: &str, student: &str) -> String {
        let req = json!({"op":"render","view":view,"args":{"student_id":student}});
        let (code, ans) = self.run_with(now, &["render"], req.to_string().as_bytes());
        assert_eq!(code, 0, "{ans}");
        ans["result"]["markdown"]
            .as_str()
            .or_else(|| ans["markdown"].as_str())
            .unwrap_or_else(|| panic!("no markdown in {ans}"))
            .to_string()
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn book(student: &str, no: &str, label: &str, ordinal: u64, hint: Option<&str>, t: u64) -> Value {
    json!({"type":"registered_book","key":format!("{student}|{no}"),"observed_at":t,
        "student_id":student,"student":student,"book_no":no,"book_label":label,
        "course_key":"693","binding_ordinal":ordinal,"hint":hint})
}
#[allow(clippy::too_many_arguments)]
fn graded(
    student: &str,
    no: &str,
    label: &str,
    n: u64,
    title: &str,
    date: &str,
    counts: (u64, u64),
    wrong: &str,
    t: u64,
) -> Value {
    json!({"type":"section_attempt","key":format!("{student}|{no}|{}", 82000 + n),"observed_at":t,
        "student_id":student,"student":student,"book":format!("{student}|{no}"),"book_no":no,
        "book_book_label":label,"index_ordinal":n,"source_title":title,
        "source_date_literal":date,"source_date_precision":"month_day",
        "correct_count":counts.0,"item_count":counts.1,
        "score":(counts.0 as f64) * 100.0 / (counts.1 as f64),"graded":true,"wrong_ordinals":wrong})
}
/// The shape `capture.rs` writes for a section that is not graded: counts and
/// score null, `wrong_ordinals` absent.
fn ungraded(
    student: &str,
    no: &str,
    label: &str,
    n: u64,
    title: &str,
    date: &str,
    t: u64,
) -> Value {
    json!({"type":"section_attempt","key":format!("{student}|{no}|{}", 82000 + n),"observed_at":t,
        "student_id":student,"student":student,"book":format!("{student}|{no}"),"book_no":no,
        "book_book_label":label,"index_ordinal":n,"source_title":title,
        "source_date_literal":date,"source_date_precision":"month_day",
        "correct_count":null,"item_count":null,"score":null,"graded":false})
}
/// Student 77001: two books (the second with a null hint, a smaller book number
/// that sorts before the first one as a number but after it as text) and five
/// sections of which one is not graded. Student 77002: one book, no section file.
fn fill(env: &Env, t: u64) {
    env.put(
        "registered_book",
        "77001",
        &[
            book(
                "77001",
                "2517",
                "Gauss fraction book",
                1,
                Some("Fictional section 01"),
                t,
            ),
            book("77001", "812", "Euler sets book", 2, None, t),
        ],
    );
    env.put(
        "section_attempt",
        "77001",
        &[
            graded(
                "77001",
                "2517",
                "Gauss fraction book",
                1,
                "Fictional section 01",
                "11-03",
                (1, 2),
                "2",
                t,
            ),
            ungraded(
                "77001",
                "2517",
                "Gauss fraction book",
                2,
                "Fictional section 02",
                "11-04",
                t,
            ),
            graded(
                "77001",
                "812",
                "Euler sets book",
                1,
                "Invented set 01",
                "11-05",
                (3, 3),
                "",
                t,
            ),
            graded(
                "77001",
                "812",
                "Euler sets book",
                2,
                "Invented set 02",
                "11-06",
                (2, 4),
                "1, 3",
                t,
            ),
            graded(
                "77001",
                "812",
                "Euler sets book",
                3,
                "Invented set 03",
                "11-07",
                (4, 4),
                "",
                t,
            ),
        ],
    );
    env.put(
        "registered_book",
        "77002",
        &[book(
            "77002",
            "300",
            "Invented logic book",
            1,
            Some("Invented chapter"),
            t,
        )],
    );
}
fn source<'a>(ans: &'a Value, name: &str) -> &'a Value {
    ans["result"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["source"] == name)
        .unwrap()
}
fn appended(entry: &Value) -> [u64; 4] {
    ["initial", "update", "refresh", "retraction"].map(|k| entry["appended"][k].as_u64().unwrap())
}

/// Observed time 1000 s since 1970 renders as `1970-01-01T00:16:40Z`. A graded section
/// with no wrong item has an empty `wrong_ordinals`; the section that is not
/// graded has no count and no wrong list; a null hint is `(missing)`.
const EXPECTED: &[(&str, &str, &str)] = &[
    (
        "registered_books",
        "77001",
        "<!-- ont:render registered_books student_id=77001 schema=academy_read@1 -->\n| Binding ordinal | Book number | Book label | Hint | Observed at |\n| --- | --- | --- | --- | --- |\n| 1 | 2517 | Gauss fraction book | Fictional section 01 | 1970-01-01T00:16:40Z |\n| 2 | 812 | Euler sets book | (missing) | 1970-01-01T00:16:40Z |\n<!-- /ont:render -->",
    ),
    (
        "section_results",
        "77001",
        "<!-- ont:render section_results student_id=77001 schema=academy_read@1 -->\n| Book number | Book label | Index ordinal | Title | Date literal | Date precision | Correct count | Item count | Wrong ordinals | Observed at |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n| 812 | Euler sets book | 1 | Invented set 01 | 11-05 | month_day | 3 | 3 |  | 1970-01-01T00:16:40Z |\n| 812 | Euler sets book | 2 | Invented set 02 | 11-06 | month_day | 2 | 4 | 1, 3 | 1970-01-01T00:16:40Z |\n| 812 | Euler sets book | 3 | Invented set 03 | 11-07 | month_day | 4 | 4 |  | 1970-01-01T00:16:40Z |\n| 2517 | Gauss fraction book | 1 | Fictional section 01 | 11-03 | month_day | 1 | 2 | 2 | 1970-01-01T00:16:40Z |\n| 2517 | Gauss fraction book | 2 | Fictional section 02 | 11-04 | month_day | (missing) | (missing) | (missing) | 1970-01-01T00:16:40Z |\n<!-- /ont:render -->",
    ),
    (
        "registered_books",
        "77002",
        "<!-- ont:render registered_books student_id=77002 schema=academy_read@1 -->\n| Binding ordinal | Book number | Book label | Hint | Observed at |\n| --- | --- | --- | --- | --- |\n| 1 | 300 | Invented logic book | Invented chapter | 1970-01-01T00:16:40Z |\n<!-- /ont:render -->",
    ),
    (
        "section_results",
        "77002",
        "<!-- ont:render section_results student_id=77002 schema=academy_read@1 -->\n| Book number | Book label | Index ordinal | Title | Date literal | Date precision | Correct count | Item count | Wrong ordinals | Observed at |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n<!-- /ont:render -->",
    ),
];

#[test]
fn a1_schema_check_passes_at_the_pinned_engine() {
    let env = Env::new();
    let file = env.root.join("check.json");
    fs::write(&file, ACADEMY_READ_SCHEMA).unwrap();
    let (code, ans) = env.run(1, &["schema", "check", &file.display().to_string()]);
    assert_eq!(code, 0, "{ans}");
}

#[test]
fn a2_both_views_for_both_students() {
    let env = Env::new();
    fill(&env, 1000);
    let ans = env.index(2000);
    assert_eq!(source(&ans, "registered_book")["status"], "ok", "{ans}");
    assert_eq!(source(&ans, "section_attempt")["status"], "ok", "{ans}");
    assert_eq!(appended(source(&ans, "registered_book")), [3, 0, 0, 0]);
    assert_eq!(appended(source(&ans, "section_attempt")), [5, 0, 0, 0]);
    for (view, student, expected) in EXPECTED {
        assert_eq!(
            &env.render(2000, view, student),
            expected,
            "{view} {student}"
        );
    }
    // I4: a day and a half without an index run shows the rows as stale.
    let stale = env.render(2000 + 129_600 + 1, "registered_books", "77002");
    assert!(stale.contains("(stale)"), "{stale}");
}

#[test]
fn a3_time_is_not_a_property_and_a_bad_line_fails_the_source() {
    let env = Env::new();
    fill(&env, 1000);
    env.index(2000);
    let before = env.render(2000, "registered_books", "77001");
    assert!(before.contains("1970-01-01T00:16:40Z"), "{before}");
    // I2: the same records captured again, only `observed_at` raised.
    fill(&env, 1500);
    let ans = env.index(2100);
    for name in ["registered_book", "section_attempt"] {
        assert_eq!(source(&ans, name)["status"], "ok", "{ans}");
        assert_eq!(appended(source(&ans, name)), [0, 0, 0, 0], "{ans}");
    }
    let after = env.render(2100, "registered_books", "77001");
    assert_eq!(
        after,
        before.replace("1970-01-01T00:16:40Z", "1970-01-01T00:25:00Z")
    );
    // I3: a line without `book_no` fails the source and keeps the first rows.
    let mut bad = book("77001", "812", "Euler sets book", 2, None, 1600);
    bad.as_object_mut().unwrap().remove("book_no");
    env.put(
        "registered_book",
        "77001",
        &[
            book(
                "77001",
                "2517",
                "Gauss fraction book",
                1,
                Some("Fictional section 01"),
                1600,
            ),
            bad,
        ],
    );
    let (code, ans) = env.index_code(2200);
    let failed = source(&ans, "registered_book");
    assert_ne!(code, 0, "{ans}");
    assert_eq!(failed["status"], "source_failed", "{ans}");
    assert!(
        failed["cause"]
            .as_str()
            .unwrap()
            .starts_with("key_unresolved@"),
        "{ans}"
    );
    assert_eq!(appended(failed), [0, 0, 0, 0]);
    assert_eq!(env.render(2200, "registered_books", "77001"), after);
    // On a section line a missing required member is not a failed source: the
    // engine marks the observation partial and the view shows `(missing)`.
    let mut partial = ungraded(
        "77003",
        "812",
        "Euler sets book",
        9,
        "Invented set 09",
        "11-09",
        1600,
    );
    partial.as_object_mut().unwrap().remove("book_no");
    env.put("section_attempt", "77003", &[partial]);
    let (_, ans) = env.index_code(2250);
    assert_eq!(source(&ans, "section_attempt")["status"], "ok", "{ans}");
    assert!(
        env.render(2250, "section_results", "77003")
            .contains("| (missing) | Euler sets book | 9 |")
    );
    // A line with an extra member is accepted.
    let mut extra = book("77001", "812", "Euler sets book", 2, None, 1600);
    extra["unknown_member"] = json!("ignored");
    env.put(
        "registered_book",
        "77001",
        &[
            book(
                "77001",
                "2517",
                "Gauss fraction book",
                1,
                Some("Fictional section 01"),
                1600,
            ),
            extra,
        ],
    );
    let ans = env.index(2300);
    assert_eq!(source(&ans, "registered_book")["status"], "ok", "{ans}");
}

/// The two lines `capture_tests.rs` (`exact_graded_bytes_and_one_login`) pins byte
/// for byte as what `capture.rs` writes for one graded section; the test below
/// fails when that pin changes, so these copies cannot drift unseen.
const CAPTURE_BOOK: &str = "{\"binding_ordinal\":1,\"book_label\":\"Gauss fraction book\",\"book_no\":\"2517\",\"course_key\":\"693\",\"hint\":\"Fictional section 01\",\"key\":\"77001|2517\",\"observed_at\":1,\"student\":\"77001\",\"student_id\":\"77001\",\"type\":\"registered_book\"}";
const CAPTURE_SECTION: &str = "{\"book\":\"77001|2517\",\"book_book_label\":\"Gauss fraction book\",\"book_no\":\"2517\",\"correct_count\":1,\"graded\":true,\"index_ordinal\":1,\"item_count\":2,\"key\":\"77001|2517|82001\",\"observed_at\":1,\"score\":50.0,\"source_date_literal\":\"11-03\",\"source_date_precision\":\"month_day\",\"source_title\":\"Fictional section 01\",\"student\":\"77001\",\"student_id\":\"77001\",\"type\":\"section_attempt\",\"wrong_ordinals\":\"2\"}";

#[test]
fn a4_the_lines_the_capture_writes_are_indexed_without_a_failed_source() {
    let capture_tests = include_str!("../../edu-academy-sources/src/capture_tests.rs");
    for line in [CAPTURE_BOOK, CAPTURE_SECTION] {
        let pinned = format!("{}\\n", line.replace('"', "\\\""));
        assert!(
            capture_tests.contains(&pinned),
            "capture_tests.rs no longer pins this line"
        );
    }
    let env = Env::new();
    fs::write(
        env.partition("registered_book", "77001"),
        format!("{CAPTURE_BOOK}\n"),
    )
    .unwrap();
    fs::write(
        env.partition("section_attempt", "77001"),
        format!("{CAPTURE_SECTION}\n"),
    )
    .unwrap();
    let ans = env.index(2);
    assert_eq!(source(&ans, "registered_book")["status"], "ok", "{ans}");
    assert_eq!(source(&ans, "section_attempt")["status"], "ok", "{ans}");
    for (view, student) in [("registered_books", "77001"), ("section_results", "77001")] {
        let md = env.render(2, view, student);
        println!("{md}");
        assert!(
            !md.contains("(missing)") && !md.contains("(null)") && !md.contains("(blank)"),
            "{md}"
        );
    }
}
