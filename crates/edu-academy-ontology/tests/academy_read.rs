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
    /// an empty spool (every partition directory) and the schema installed
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
        for dir in [
            "spool/registered_book",
            "spool/section_attempt",
            "spool/lesson_record",
            "spool/lesson_day",
        ] {
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
    /// A view with any arguments: the number of rows and the table.
    fn table(&self, now: u64, view: &str, args: Value) -> (u64, String) {
        let req = json!({"op":"render","view":view,"args":args});
        let (code, ans) = self.run_with(now, &["render"], req.to_string().as_bytes());
        assert_eq!(code, 0, "{ans}");
        let result = if ans["result"].is_object() {
            &ans["result"]
        } else {
            &ans
        };
        (
            result["rows"].as_u64().unwrap(),
            result["markdown"].as_str().unwrap().to_string(),
        )
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
        "<!-- ont:render registered_books student_id=77001 schema=academy_read@2 -->\n| Binding ordinal | Book number | Book label | Hint | Observed at |\n| --- | --- | --- | --- | --- |\n| 1 | 2517 | Gauss fraction book | Fictional section 01 | 1970-01-01T00:16:40Z |\n| 2 | 812 | Euler sets book | (missing) | 1970-01-01T00:16:40Z |\n<!-- /ont:render -->",
    ),
    (
        "section_results",
        "77001",
        "<!-- ont:render section_results student_id=77001 schema=academy_read@2 -->\n| Book number | Book label | Index ordinal | Title | Date literal | Date precision | Correct count | Item count | Wrong ordinals | Observed at |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n| 812 | Euler sets book | 1 | Invented set 01 | 11-05 | month_day | 3 | 3 |  | 1970-01-01T00:16:40Z |\n| 812 | Euler sets book | 2 | Invented set 02 | 11-06 | month_day | 2 | 4 | 1, 3 | 1970-01-01T00:16:40Z |\n| 812 | Euler sets book | 3 | Invented set 03 | 11-07 | month_day | 4 | 4 |  | 1970-01-01T00:16:40Z |\n| 2517 | Gauss fraction book | 1 | Fictional section 01 | 11-03 | month_day | 1 | 2 | 2 | 1970-01-01T00:16:40Z |\n| 2517 | Gauss fraction book | 2 | Fictional section 02 | 11-04 | month_day | (missing) | (missing) | (missing) | 1970-01-01T00:16:40Z |\n<!-- /ont:render -->",
    ),
    (
        "registered_books",
        "77002",
        "<!-- ont:render registered_books student_id=77002 schema=academy_read@2 -->\n| Binding ordinal | Book number | Book label | Hint | Observed at |\n| --- | --- | --- | --- | --- |\n| 1 | 300 | Invented logic book | Invented chapter | 1970-01-01T00:16:40Z |\n<!-- /ont:render -->",
    ),
    (
        "section_results",
        "77002",
        "<!-- ont:render section_results student_id=77002 schema=academy_read@2 -->\n| Book number | Book label | Index ordinal | Title | Date literal | Date precision | Correct count | Item count | Wrong ordinals | Observed at |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n<!-- /ont:render -->",
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

/// Records at the largest size the capture can publish, with multibyte text
/// (one invented Hangul syllable repeated; 3 UTF-8 bytes per UTF-16 unit): a
/// book label of 200 units, a title and a hint of 2000 units (200 is the graded
/// bound; 2000 is the schema's choice for the no-detail path, which the capture
/// leaves unbounded), a 16-digit key, a date literal of 4095 bytes and 800
/// ordinals of 16 digits. One such line must not fail the source of the others.
#[test]
fn a5_records_at_the_capture_maximum_with_multibyte_text_are_indexed_and_rendered() {
    let env = Env::new();
    let student = "9007199254740991";
    let book_no = "9007199254740990";
    let label = "가".repeat(200);
    let title = "나".repeat(2000);
    let date = "다".repeat(1365);
    let wrong = vec!["9007199254740991"; 800].join(", ");
    assert_eq!(label.len(), 600);
    assert_eq!(title.len(), 6000);
    let mut b = book(student, book_no, &label, 1, Some(&title), 1000);
    b["course_key"] = json!("9007199254740991");
    env.put(
        "registered_book",
        student,
        &[b, book("77001", "812", "Euler sets book", 1, None, 1000)],
    );
    let mut s = graded(
        student,
        book_no,
        &label,
        1,
        &title,
        &date,
        (1, 2),
        &wrong,
        1000,
    );
    s["source_date_precision"] = json!("unknown");
    s["key"] = json!(format!("{student}|{book_no}|9007199254740991"));
    env.put("section_attempt", student, &[s]);
    let (code, ans) = env.index_code(2000);
    assert_eq!(code, 0, "{ans}");
    assert_eq!(
        appended(source(&ans, "registered_book")),
        [2, 0, 0, 0],
        "{ans}"
    );
    assert_eq!(
        appended(source(&ans, "section_attempt")),
        [1, 0, 0, 0],
        "{ans}"
    );
    let books = env.render(2000, "registered_books", student);
    assert!(
        books.contains(&format!("| {book_no} | {label} | {title} |")),
        "hint and label"
    );
    let sections = env.render(2000, "section_results", student);
    for text in [&label, &title, &date, &wrong] {
        assert!(sections.contains(text.as_str()));
    }
}

// ---- The lesson record: the types `lesson_record` and `lesson_day`. ----
// The lines are the contract of the capture family `dailyrecord`
// (docs/academy-capture.md, "The lesson record's lines"). A month of lesson
// records is one file and a year of captured dates is one file.

/// A row with nothing saved yet. Every member is on every line except
/// `record_key` and `previous_record_key`; an empty text is `""`.
fn lesson(student: &str, name: &str, membership: &str, date: &str, ordinal: u64, t: u64) -> Value {
    json!({
        "type":"lesson_record","observed_at":t,"source_namespace":"academy.lms.dayrecord",
        "student_id":student,"student_name":name,"lesson_date":date,"row_ordinal":ordinal,
        "course_id":"3","course_label":"Algebra A","membership_key":membership,
        "attendance":"Y","attendance_label":"Present",
        "progress":"","homework":"","lesson_memo":"","daily_test_assessment":"0",
        "student_memo":"","previous_lesson_date_literal":"","previous_progress":"",
        "previous_homework":"","previous_lesson_memo":"","previous_homework_assessment":"",
        "cut_fields":0
    })
}
fn with(mut line: Value, members: Value) -> Value {
    for (k, v) in members.as_object().unwrap() {
        line[k.as_str()] = v.clone();
    }
    line
}
fn lesson_day(date: &str, rows: u64, t: u64) -> Value {
    json!({
        "type":"lesson_day","observed_at":t,"source_namespace":"academy.lms.dayrecord",
        "lesson_date":date,"row_count":rows,"stored_count":rows,
        "outside_roster_count":0,"cut_fields":0
    })
}
/// Student 77001 on two dates of one membership; student 77002 on the first
/// date with nothing saved (the page showed no record key).
fn lesson_lines() -> [Value; 3] {
    let first = with(
        lesson("77001", "Ada Invented", "50001", "2031-03-03", 0, 1000),
        json!({
            "record_key":"41","progress":"Unit 3 to p.40","homework":"p.41-44",
            "lesson_memo":"line one\nline two | pipe","daily_test_assessment":"2",
            "student_memo":"Invented memo","previous_record_key":"40",
            "previous_lesson_date_literal":"02/28","previous_progress":"Unit 2",
            "previous_homework":"p.30-33","previous_homework_assessment":"3"
        }),
    );
    let unsaved = lesson("77002", "가나다", "50002", "2031-03-03", 1, 1000);
    let second = with(
        lesson("77001", "Ada Invented", "50001", "2031-03-05", 0, 1000),
        json!({
            "record_key":"42","previous_record_key":"41",
            "previous_lesson_date_literal":"03/03","previous_progress":"Unit 3 to p.40",
            "previous_homework":"p.41-44","previous_lesson_memo":"line one\nline two | pipe",
            "previous_homework_assessment":"2"
        }),
    );
    [first, unsaved, second]
}
fn lesson_fill(env: &Env) {
    env.put("lesson_record", "2031-03", &lesson_lines());
    env.put(
        "lesson_day",
        "2031",
        &[
            lesson_day("2031-03-03", 2, 1000),
            lesson_day("2031-03-05", 1, 1000),
        ],
    );
}
const DAY_HEAD: &str = "| Student id | Student | Course | Membership | Attendance | Daily test | Previous homework rate | Previous date | Observed at |";
const DAY_ADA: &str =
    "| 77001 | Ada Invented | Algebra A | 50001 | Present | 2 | 3 | 02/28 | 1970-01-01T00:16:40Z |";
const DAY_UNSAVED: &str =
    "| 77002 | 가나다 | Algebra A | 50002 | Present | 0 |  |  | 1970-01-01T00:16:40Z |";

#[test]
fn b1_the_four_views_of_the_lesson_record() {
    let env = Env::new();
    lesson_fill(&env);
    let ans = env.index(2000);
    assert_eq!(
        appended(source(&ans, "lesson_record")),
        [3, 0, 0, 0],
        "{ans}"
    );
    assert_eq!(appended(source(&ans, "lesson_day")), [2, 0, 0, 0], "{ans}");
    // One date: one row per membership in the page's order, without the texts;
    // a row with nothing saved has empty cells.
    let (rows, day) = env.table(2000, "daily_records", json!({"lesson_date":"2031-03-03"}));
    assert_eq!(
        day,
        format!(
            "<!-- ont:render daily_records lesson_date=2031-03-03 schema=academy_read@2 -->\n{DAY_HEAD}\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n{DAY_ADA}\n{DAY_UNSAVED}\n<!-- /ont:render -->"
        )
    );
    assert_eq!(rows, 2);
    // One student on one date: every member. The only cells that can be
    // `(missing)` are the two keys the LMS does not show before a save.
    let (rows, one) = env.table(
        2000,
        "daily_record",
        json!({"lesson_date":"2031-03-03","student_id":"77002"}),
    );
    assert_eq!(rows, 1);
    assert!(
        one.contains("| 77002 | 가나다 | Algebra A | 3 | 50002 | (missing) | Y | Present |  |  |  | 0 |  | (missing) |  |  |  |  |  | 0 | 1970-01-01T00:16:40Z |"),
        "{one}"
    );
    let (_, full) = env.table(
        2000,
        "daily_record",
        json!({"lesson_date":"2031-03-03","student_id":"77001"}),
    );
    assert!(
        full.contains("| 77001 | Ada Invented | Algebra A | 3 | 50001 | 41 | Y | Present | Unit 3 to p.40 | p.41-44 | line one line two \\| pipe | 2 | Invented memo | 40 | 02/28 | Unit 2 | p.30-33 |  | 3 | 0 | 1970-01-01T00:16:40Z |"),
        "{full}"
    );
    // One student's records from a date on, the newest date first.
    let (rows, history) = env.table(
        2000,
        "record_history",
        json!({"student_id":"77001","since":"2031-03-01"}),
    );
    assert_eq!(rows, 2);
    let (rows, newest) = env.table(
        2000,
        "record_history",
        json!({"student_id":"77001","since":"2031-03-04"}),
    );
    assert_eq!(rows, 1);
    assert!(newest.contains("| 2031-03-05 |") && !newest.contains("| 2031-03-03 |"));
    assert!(
        history.contains("| 2031-03-05 | Algebra A | 50001 | 42 | Present |  |  |  | 0 | 2 | 1970-01-01T00:16:40Z |\n| 2031-03-03 | Algebra A | 50001 | 41 | Present | Unit 3 to p.40 | p.41-44 | line one line two \\| pipe | 2 | 3 | 1970-01-01T00:16:40Z |"),
        "{history}"
    );
    // The previous lesson's fields on the later date's row are the earlier
    // date's own fields: the check a replay runs inside the store.
    assert!(full.contains("| Unit 3 to p.40 | p.41-44 |"));
    let (_, later) = env.table(
        2000,
        "daily_record",
        json!({"lesson_date":"2031-03-05","student_id":"77001"}),
    );
    assert!(
        later
            .contains("| 41 | 03/03 | Unit 3 to p.40 | p.41-44 | line one line two \\| pipe | 2 |"),
        "{later}"
    );
}

#[test]
fn b2_a_date_never_captured_differs_from_a_captured_date_without_rows() {
    let env = Env::new();
    lesson_fill(&env);
    env.put(
        "lesson_day",
        "2031",
        &[
            lesson_day("2031-03-03", 2, 1000),
            lesson_day("2031-03-04", 0, 1000),
            lesson_day("2031-03-05", 1, 1000),
        ],
    );
    env.index(2000);
    for date in ["2031-03-04", "2031-03-06"] {
        let (rows, _) = env.table(2000, "daily_records", json!({"lesson_date":date}));
        assert_eq!(rows, 0, "{date}");
    }
    let (rows, held_none) = env.table(2000, "day_coverage", json!({"lesson_date":"2031-03-04"}));
    assert_eq!(rows, 1);
    assert!(
        held_none.contains("| 2031-03-04 | 0 | 0 | 0 | 0 | 1970-01-01T00:16:40Z |"),
        "{held_none}"
    );
    let (rows, _) = env.table(2000, "day_coverage", json!({"lesson_date":"2031-03-06"}));
    assert_eq!(rows, 0);
    // A date written without its dashes names no object: the views answer
    // with no row, and only a line with such a date fails its source (b5).
    let (rows, _) = env.table(2000, "day_coverage", json!({"lesson_date":"20310303"}));
    assert_eq!(rows, 0);
}

#[test]
fn b3_a_recapture_retracts_the_row_the_page_no_longer_holds_and_keeps_other_dates() {
    let env = Env::new();
    lesson_fill(&env);
    env.index(2000);
    let [first, _, second] = lesson_lines();
    // The date 2031-03-03 captured again at 1500: one row is gone and one changed.
    let again = with(
        first,
        json!({"observed_at":1500,"attendance":"L","attendance_label":"Late"}),
    );
    env.put("lesson_record", "2031-03", &[again, second]);
    env.put(
        "lesson_day",
        "2031",
        &[
            lesson_day("2031-03-03", 1, 1500),
            lesson_day("2031-03-05", 1, 1000),
        ],
    );
    let ans = env.index(2100);
    assert_eq!(
        appended(source(&ans, "lesson_record")),
        [0, 1, 0, 1],
        "{ans}"
    );
    assert_eq!(source(&ans, "lesson_record")["unchanged"], 1, "{ans}");
    let (rows, day) = env.table(2100, "daily_records", json!({"lesson_date":"2031-03-03"}));
    assert_eq!(rows, 1);
    assert!(
        day.contains("| 77001 | Ada Invented | Algebra A | 50001 | Late |"),
        "{day}"
    );
    assert!(day.contains("| 1970-01-01T00:25:00Z |"), "{day}");
    let (rows, _) = env.table(
        2100,
        "record_history",
        json!({"student_id":"77002","since":"2031-01-01"}),
    );
    assert_eq!(rows, 0);
    let (rows, later) = env.table(2100, "daily_records", json!({"lesson_date":"2031-03-05"}));
    assert_eq!(rows, 1);
    assert!(later.contains("| 1970-01-01T00:16:40Z |"), "{later}");
}

#[test]
fn b4_two_memberships_on_one_date_are_two_objects_and_a_repeated_key_fails_the_source() {
    let env = Env::new();
    let [first, unsaved, second] = lesson_lines();
    let other_course = with(
        lesson("77001", "Ada Invented", "50009", "2031-03-03", 2, 1000),
        json!({"course_id":"5","course_label":"Geometry B","record_key":"41"}),
    );
    env.put(
        "lesson_record",
        "2031-03",
        &[first.clone(), unsaved.clone(), other_course, second.clone()],
    );
    env.index(2000);
    // The same record key in two memberships: two rows that the membership tells apart.
    let (rows, both) = env.table(
        2000,
        "daily_record",
        json!({"lesson_date":"2031-03-03","student_id":"77001"}),
    );
    assert_eq!(rows, 2);
    assert!(
        both.contains("| Algebra A | 3 | 50001 | 41 |")
            && both.contains("| Geometry B | 5 | 50009 | 41 |"),
        "{both}"
    );
    let before = env.table(2000, "daily_records", json!({"lesson_date":"2031-03-03"}));
    // One key on two lines: the whole source fails and the store keeps what it had.
    let twice = with(first.clone(), json!({"row_ordinal":7}));
    env.put("lesson_record", "2031-03", &[first, twice, unsaved, second]);
    let (code, ans) = env.index_code(2100);
    let failed = source(&ans, "lesson_record");
    assert_ne!(code, 0, "{ans}");
    assert_eq!(failed["status"], "source_failed", "{ans}");
    assert!(
        failed["cause"]
            .as_str()
            .unwrap()
            .starts_with("duplicate_key@"),
        "{ans}"
    );
    assert_eq!(appended(failed), [0, 0, 0, 0]);
    assert_eq!(
        env.table(2100, "daily_records", json!({"lesson_date":"2031-03-03"})),
        before
    );
}

#[test]
fn b5_a_line_older_than_its_head_or_with_a_malformed_date_fails_the_whole_source() {
    let env = Env::new();
    lesson_fill(&env);
    env.index(2000);
    let before = env.table(2000, "daily_records", json!({"lesson_date":"2031-03-05"}));
    let [first, unsaved, second] = lesson_lines();
    // A writer that replaces a line by one with a smaller time blocks every
    // date of the source, not only its own: the capture's writer never does.
    let older = with(
        first.clone(),
        json!({"observed_at":900,"progress":"changed"}),
    );
    let newer = with(second.clone(), json!({"observed_at":1500,"progress":"new"}));
    env.put("lesson_record", "2031-03", &[older, unsaved.clone(), newer]);
    let (code, ans) = env.index_code(2100);
    let failed = source(&ans, "lesson_record");
    assert_ne!(code, 0, "{ans}");
    assert!(
        failed["cause"]
            .as_str()
            .unwrap()
            .starts_with("record_older_than_head@"),
        "{ans}"
    );
    assert_eq!(appended(failed), [0, 0, 0, 0]);
    assert_eq!(
        env.table(2100, "daily_records", json!({"lesson_date":"2031-03-05"})),
        before
    );
    let undashed = with(second, json!({"lesson_date":"20310305"}));
    env.put("lesson_record", "2031-03", &[first, unsaved, undashed]);
    let (code, ans) = env.index_code(2200);
    let failed = source(&ans, "lesson_record");
    assert_ne!(code, 0, "{ans}");
    assert_eq!(
        failed["cause"], "schema_violation:value_constraint_violation@/fields/lesson_date",
        "{ans}"
    );
    assert_eq!(
        env.table(2200, "daily_records", json!({"lesson_date":"2031-03-05"})),
        before
    );
}

/// The seven texts of a row at their bound of 6000 bytes (one invented Hangul
/// syllable, 3 bytes, 2000 times) stay in one line under the engine's limit;
/// one byte more in one text fails the source, which is why the capture cuts
/// a longer text and counts it in `cut_fields`.
#[test]
fn b6_seven_texts_at_their_bound_are_indexed_and_one_byte_more_fails_the_source() {
    let env = Env::new();
    let [first, unsaved, second] = lesson_lines();
    let big = "가".repeat(2000);
    let texts = [
        "progress",
        "homework",
        "lesson_memo",
        "student_memo",
        "previous_progress",
        "previous_homework",
        "previous_lesson_memo",
    ];
    let mut full = second.clone();
    for name in texts {
        full[name] = json!(big);
    }
    assert!(full.to_string().len() < 65_536);
    env.put(
        "lesson_record",
        "2031-03",
        &[first.clone(), unsaved.clone(), full],
    );
    let ans = env.index(2000);
    assert_eq!(
        appended(source(&ans, "lesson_record")),
        [3, 0, 0, 0],
        "{ans}"
    );
    let (rows, one) = env.table(
        2000,
        "daily_record",
        json!({"lesson_date":"2031-03-05","student_id":"77001"}),
    );
    assert_eq!(rows, 1);
    assert_eq!(one.matches(big.as_str()).count(), 7);
    for name in texts {
        let over = with(
            second.clone(),
            json!({"observed_at":1100, name: format!("{big}a")}),
        );
        env.put(
            "lesson_record",
            "2031-03",
            &[first.clone(), unsaved.clone(), over],
        );
        let (code, ans) = env.index_code(2100);
        assert_ne!(code, 0, "{name}: {ans}");
        assert_eq!(
            source(&ans, "lesson_record")["cause"],
            format!("schema_violation:value_constraint_violation@/fields/{name}"),
            "{ans}"
        );
    }
}

const LESSON_TEXTS: [&str; 7] = [
    "progress",
    "homework",
    "lesson_memo",
    "student_memo",
    "previous_progress",
    "previous_homework",
    "previous_lesson_memo",
];
/// The cause of the source `lesson_record` in an index run that must fail
/// and append nothing.
fn lesson_failure(env: &Env, now: u64) -> String {
    let (code, ans) = env.index_code(now);
    let failed = source(&ans, "lesson_record");
    assert_ne!(code, 0, "{ans}");
    assert_eq!(failed["status"], "source_failed", "{ans}");
    assert_eq!(appended(failed), [0, 0, 0, 0], "{ans}");
    failed["cause"].as_str().unwrap().to_string()
}

/// Rule 4 of the contract. Every string member is bounded, and a writer
/// measures a string in the form the line holds it (quote mark, backslash and
/// control characters escaped). A line whose strings all stand at their bounds
/// is indexed; one byte more in a bounded member fails the whole source; and
/// a text that keeps its 6000 bytes only before the escaping makes a line, or
/// an observation, that the engine refuses whatever `max_bytes` says.
#[test]
fn b8_every_bound_of_a_line_and_the_serialized_form_a_writer_measures() {
    let env = Env::new();
    let [first, unsaved, second] = lesson_lines();
    let bounds: [(&str, usize); 12] = [
        ("student_name", 200),
        ("course_label", 200),
        ("attendance_label", 64),
        ("previous_lesson_date_literal", 64),
        ("student_id", 32),
        ("membership_key", 32),
        ("course_id", 32),
        ("record_key", 32),
        ("previous_record_key", 32),
        ("attendance", 16),
        ("daily_test_assessment", 16),
        ("previous_homework_assessment", 16),
    ];
    // Every string member at its bound; the texts are quote marks, of which
    // 3000 are 6000 bytes in the line.
    let mut full = second.clone();
    for name in LESSON_TEXTS {
        full[name] = json!("\"".repeat(3000));
    }
    for (name, bytes) in bounds {
        full[name] = json!("7".repeat(bytes));
    }
    let bytes = full.to_string().len();
    assert!((43_000..44_000).contains(&bytes), "{bytes}");
    env.put(
        "lesson_record",
        "2031-03",
        &[first.clone(), unsaved.clone(), full],
    );
    let ans = env.index(2000);
    assert_eq!(
        appended(source(&ans, "lesson_record")),
        [3, 0, 0, 0],
        "{ans}"
    );
    // One byte more in a member that is no text fails every date.
    for (name, bytes) in bounds {
        let over = with(
            second.clone(),
            json!({"observed_at":1100, name: "7".repeat(bytes + 1)}),
        );
        env.put(
            "lesson_record",
            "2031-03",
            &[first.clone(), unsaved.clone(), over],
        );
        assert_eq!(
            lesson_failure(&env, 2100),
            format!("schema_violation:value_constraint_violation@/fields/{name}")
        );
    }
    // 6000 quote marks pass `max_bytes` and are 12000 bytes in the line.
    let mut escaped = with(second.clone(), json!({"observed_at":1100}));
    for name in LESSON_TEXTS {
        escaped[name] = json!("\"".repeat(6000));
    }
    assert!(escaped.to_string().len() > 65_536);
    env.put(
        "lesson_record",
        "2031-03",
        &[first.clone(), unsaved.clone(), escaped],
    );
    assert!(lesson_failure(&env, 2100).starts_with("record_line_too_long@"));
    // 4600 line breaks in each text: the line is under 65,536 bytes and the
    // observation the engine makes of it is not.
    let mut breaks = with(second, json!({"observed_at":1100}));
    for name in LESSON_TEXTS {
        breaks[name] = json!("\n".repeat(4600));
    }
    assert!(breaks.to_string().len() < 65_536);
    env.put("lesson_record", "2031-03", &[first, unsaved, breaks]);
    assert!(lesson_failure(&env, 2100).starts_with("observation_too_large@"));
}

/// The key and the types of the members. A line without one of the three key
/// members, with a key part that is empty, holds the separator, holds a
/// letter or is longer than 32 digits, or with a number written as a string,
/// fails the whole source. A line without another member is indexed and
/// renders `(missing)`, which a writer never produces.
#[test]
fn b9_a_malformed_key_or_type_fails_the_source_and_a_left_out_member_renders_missing() {
    let env = Env::new();
    lesson_fill(&env);
    env.index(2000);
    let [first, unsaved, second] = lesson_lines();
    let later = with(second, json!({"observed_at":1100}));
    let put = |line: Value| {
        env.put(
            "lesson_record",
            "2031-03",
            &[first.clone(), unsaved.clone(), line],
        );
    };
    for member in ["student_id", "membership_key", "lesson_date"] {
        let mut line = later.clone();
        line.as_object_mut().unwrap().remove(member);
        put(line);
        assert!(
            lesson_failure(&env, 2100).starts_with("key_unresolved@"),
            "{member}"
        );
    }
    for (member, value) in [
        ("student_id", String::new()),
        ("student_id", "1|2".to_string()),
        ("student_id", "7".repeat(33)),
        ("membership_key", "2|3".to_string()),
        ("membership_key", "5000a".to_string()),
    ] {
        put(with(later.clone(), json!({ member: value.clone() })));
        assert_eq!(
            lesson_failure(&env, 2100),
            format!("schema_violation:value_constraint_violation@/fields/{member}"),
            "{value}"
        );
    }
    put(with(later.clone(), json!({"row_ordinal":"0"})));
    assert_eq!(
        lesson_failure(&env, 2100),
        "schema_violation:wrong_value_type@/fields/row_ordinal"
    );
    let mut partial = later;
    partial.as_object_mut().unwrap().remove("progress");
    put(partial);
    env.index(2100);
    let (rows, one) = env.table(
        2100,
        "daily_record",
        json!({"lesson_date":"2031-03-05","student_id":"77001"}),
    );
    assert_eq!(rows, 1);
    assert_eq!(one.matches("(missing)").count(), 1, "{one}");
}

/// The files and the history's bound. A month file that is emptied or gone
/// retracts its rows with status `ok`, so a writer never removes one. A time
/// later than the index run and a line of the other type fail the source.
/// The history shows that it reached its bound, and inside a date it keeps
/// the page's order, not the order of the membership keys as text.
#[test]
fn b10_a_month_file_that_is_emptied_or_gone_retracts_and_the_history_shows_its_bound() {
    let env = Env::new();
    lesson_fill(&env);
    env.index(2000);
    env.put("lesson_record", "2031-03", &[]);
    let ans = env.index(2100);
    assert_eq!(
        appended(source(&ans, "lesson_record")),
        [0, 0, 0, 3],
        "{ans}"
    );
    let (rows, _) = env.table(2100, "daily_records", json!({"lesson_date":"2031-03-03"}));
    assert_eq!(rows, 0);
    let back: Vec<Value> = lesson_lines()
        .into_iter()
        .map(|line| with(line, json!({"observed_at":2150})))
        .collect();
    env.put("lesson_record", "2031-03", &back);
    let ans = env.index(2200);
    assert_eq!(
        appended(source(&ans, "lesson_record")),
        [3, 0, 0, 0],
        "{ans}"
    );
    fs::remove_file(env.partition("lesson_record", "2031-03")).unwrap();
    let ans = env.index(2300);
    assert_eq!(
        appended(source(&ans, "lesson_record")),
        [0, 0, 0, 3],
        "{ans}"
    );
    let [first, _, _] = lesson_lines();
    env.put(
        "lesson_record",
        "2031-03",
        &[with(first.clone(), json!({"observed_at":9000}))],
    );
    assert!(lesson_failure(&env, 2400).starts_with("record_time_in_future@"));
    env.put(
        "lesson_record",
        "2031-03",
        &[
            with(first, json!({"observed_at":2350})),
            lesson_day("2031-03-03", 1, 2350),
        ],
    );
    assert!(lesson_failure(&env, 2400).starts_with("record_type_mismatch@"));
    fs::remove_file(env.partition("lesson_record", "2031-03")).unwrap();
    // 24 dates of one membership and, on the newest, a second membership whose
    // key sorts last as text and stands first on the page: 25 rows.
    let mut rows: Vec<Value> = (1..=24)
        .map(|day| {
            lesson(
                "77003",
                "Bo Invented",
                "50003",
                &format!("2031-05-{day:02}"),
                1,
                2350,
            )
        })
        .collect();
    rows.push(lesson("77003", "Bo Invented", "9", "2031-05-24", 0, 2350));
    env.put("lesson_record", "2031-05", &rows);
    env.index(2400);
    let (count, history) = env.table(
        2400,
        "record_history",
        json!({"student_id":"77003","since":"2031-01-01"}),
    );
    assert_eq!(count, 24);
    assert!(history.contains("| (+1 more; limit 24) |"), "{history}");
    assert!(
        history.contains("| 2031-05-02 |") && !history.contains("| 2031-05-01 |"),
        "{history}"
    );
    let page_first = history.find("| 2031-05-24 | Algebra A | 9 |").unwrap();
    let page_second = history.find("| 2031-05-24 | Algebra A | 50003 |").unwrap();
    assert!(page_first < page_second, "{history}");
    let (count, _) = env.table(
        2400,
        "record_history",
        json!({"student_id":"77003","since":"2031-05-20"}),
    );
    assert_eq!(count, 6);
}

/// What a reader of the views has to know: the sentences of the document's
/// "Views" paragraph that no other test holds. `since` of `record_history`
/// is compared with `lesson_date` as TEXT, so only a zero-padded `YYYY-MM-DD`
/// means a date and another spelling answers other rows without a mark. A
/// date's table shows 200 rows and says so at 201. A `lesson_record` source
/// that fails while `lesson_day` is indexed leaves, in the store, a coverage
/// newer than its rows. The sizes the document names are upper bounds here.
/// A capture that changes nothing still renews the time the views show.
#[test]
fn b11_since_is_text_a_date_of_201_rows_says_so_and_the_sizes_the_document_names() {
    let env = Env::new();
    for date in ["2040-01-02", "2040-06-15", "2041-02-01"] {
        env.put(
            "lesson_record",
            &date[..7],
            &[lesson("77003", "Bo Invented", "50003", date, 0, 1000)],
        );
    }
    env.index(2000);
    let history = |since: &str| {
        env.table(
            2000,
            "record_history",
            json!({"student_id":"77003","since":since}),
        )
        .0
    };
    assert_eq!(
        history("2040-06-15"),
        2,
        "a row of the date itself is shown"
    );
    assert_eq!(history("2041-02-02"), 0);
    // Not a padded date: no refusal and no mark, and the rows are other rows.
    assert_eq!(history("2040-6-1"), 1, "the row of 2040-06-15 is not shown");
    assert_eq!(history("20400102"), 1);
    assert_eq!(history("2040"), 3);
    // The views of one date compare for equality: another spelling is no row.
    let (rows, _) = env.table(2000, "daily_records", json!({"lesson_date":"2040-6-15"}));
    assert_eq!(rows, 0);
    // The patterns check a shape: a date that does not exist is indexed, and
    // an id is the text the page held (`007` is not `7`).
    env.put(
        "lesson_record",
        "2040-13",
        &[
            lesson("007", "Bo Invented", "50003", "2040-13-45", 0, 1000),
            lesson("7", "Bo Invented", "50003", "2040-13-45", 1, 1000),
        ],
    );
    // The engine does not check a directory's mode (0700 is the writer's duty).
    fs::set_permissions(
        env.root.join("spool/lesson_record"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    env.index(2050);
    let (rows, _) = env.table(2050, "daily_records", json!({"lesson_date":"2040-13-45"}));
    assert_eq!(rows, 2);
    let req = json!({"op":"render","view":"record_history","args":{"student_id":"77003"}});
    let (code, ans) = env.run_with(2000, &["render"], req.to_string().as_bytes());
    assert_eq!(code, 2, "{ans}");
    assert_eq!(ans["status"], "refused", "{ans}");
    assert_eq!(ans["code"], "invalid_argument", "{ans}");

    // 200 rows of one date are the whole table; at 201 the last row says so,
    // and the row the table does not show is still read by its id.
    let day = "2040-03-04";
    let many: Vec<Value> = (0..201u64)
        .map(|i| {
            lesson(
                &(80_001 + i).to_string(),
                "Bo Invented",
                &(60_001 + i).to_string(),
                day,
                i,
                1000,
            )
        })
        .collect();
    env.put("lesson_record", "2040-03", &many[..200]);
    env.index(2100);
    let (count, table) = env.table(2100, "daily_records", json!({"lesson_date":day}));
    assert_eq!(count, 200);
    assert!(!table.contains("more; limit"), "{table}");
    env.put("lesson_record", "2040-03", &many);
    env.index(2200);
    let (count, table) = env.table(2200, "daily_records", json!({"lesson_date":day}));
    assert_eq!(count, 200);
    assert!(table.contains("| (+1 more; limit 200) |"), "{table}");
    let (count, _) = env.table(
        2200,
        "daily_record",
        json!({"lesson_date":day,"student_id":"80201"}),
    );
    assert_eq!(count, 1);

    // The sizes of the document: seven texts of 300 bytes on every row.
    let full = |student: &str, date: &str, ordinal: u64, bytes: usize| {
        let mut line = lesson(student, "Bo Invented", "50004", date, ordinal, 1000);
        for text in LESSON_TEXTS {
            line[text] = json!("x".repeat(bytes));
        }
        line
    };
    let month: Vec<Value> = (1..=24)
        .map(|d| full("77004", &format!("2040-09-{d:02}"), 0, 300))
        .collect();
    env.put("lesson_record", "2040-09", &month);
    let class: Vec<Value> = (0..20u64)
        .map(|i| full(&(81_001 + i).to_string(), "2040-10-01", i, 300))
        .collect();
    env.put("lesson_record", "2040-10", &class);
    env.put(
        "lesson_record",
        "2040-11",
        &[full("77005", "2040-11-02", 0, 6000)],
    );
    env.put("lesson_day", "2040", &[lesson_day("2040-09-10", 1, 1000)]);
    env.index(2300);
    let (_, one) = env.table(
        2300,
        "daily_record",
        json!({"lesson_date":"2040-09-01","student_id":"77004"}),
    );
    let (_, date) = env.table(2300, "daily_records", json!({"lesson_date":"2040-10-01"}));
    let (rows, past) = env.table(
        2300,
        "record_history",
        json!({"student_id":"77004","since":"2040-09-01"}),
    );
    let (_, bound) = env.table(
        2300,
        "daily_record",
        json!({"lesson_date":"2040-11-02","student_id":"77005"}),
    );
    assert!(one.len() < 3_000, "one row in full: {}", one.len());
    assert!(date.len() < 2_500, "20 rows of a date: {}", date.len());
    assert_eq!(rows, 24);
    assert!(past.len() < 25_000, "24 rows of a history: {}", past.len());
    assert!(
        bound.len() < 44_000,
        "one row at its bound: {}",
        bound.len()
    );

    // The three views of one date compare for EQUALITY: the start of a stored
    // date (2040-09-10 to 2040-09-19 are stored) answers no row.
    for (view, args) in [
        ("daily_records", json!({"lesson_date":"2040-09-1"})),
        (
            "daily_record",
            json!({"lesson_date":"2040-09-1","student_id":"77004"}),
        ),
        ("day_coverage", json!({"lesson_date":"2040-09-1"})),
    ] {
        assert_eq!(env.table(2300, view, args).0, 0, "{view}");
    }
    let (rows, _) = env.table(2300, "day_coverage", json!({"lesson_date":"2040-09-10"}));
    assert_eq!(rows, 1);
    // Every view asked without one of its arguments is refused.
    for (view, args) in [
        ("daily_records", json!({})),
        ("daily_record", json!({"lesson_date":"2040-09-10"})),
        ("day_coverage", json!({})),
    ] {
        let req = json!({"op":"render","view":view,"args":args});
        let (code, ans) = env.run_with(2300, &["render"], req.to_string().as_bytes());
        assert_eq!(code, 2, "{view}: {ans}");
        assert_eq!(ans["code"], "invalid_argument", "{view}: {ans}");
    }

    // A capture that finds a date unchanged still moves `Observed at` of its
    // rows and of its line to the time of that capture.
    let again = Env::new();
    lesson_fill(&again);
    again.index(2000);
    let later: Vec<Value> = lesson_lines()
        .into_iter()
        .map(|line| with(line, json!({"observed_at":1500})))
        .collect();
    again.put("lesson_record", "2031-03", &later);
    again.put(
        "lesson_day",
        "2031",
        &[
            lesson_day("2031-03-03", 2, 1500),
            lesson_day("2031-03-05", 1, 1500),
        ],
    );
    let ans = again.index(2100);
    assert_eq!(source(&ans, "lesson_record")["renewed"], 3, "{ans}");
    assert_eq!(source(&ans, "lesson_day")["renewed"], 2, "{ans}");
    let (_, coverage) = again.table(2100, "day_coverage", json!({"lesson_date":"2031-03-03"}));
    assert!(
        coverage.contains("| 2031-03-03 | 2 | 2 | 0 | 0 | 1970-01-01T00:25:00Z |"),
        "{coverage}"
    );
    let (_, rows) = again.table(2100, "daily_records", json!({"lesson_date":"2031-03-03"}));
    assert_eq!(
        rows.matches("| 1970-01-01T00:25:00Z |").count(),
        2,
        "{rows}"
    );

    // In the STORE a coverage can be newer than its rows: the rows' source
    // failed (here by rule 2) in a run that indexed the coverage.
    let other = Env::new();
    lesson_fill(&other);
    other.index(2000);
    let [first, unsaved, second] = lesson_lines();
    other.put(
        "lesson_record",
        "2031-03",
        &[
            with(first, json!({"observed_at":900,"progress":"changed"})),
            unsaved,
            second,
        ],
    );
    other.put(
        "lesson_day",
        "2031",
        &[
            lesson_day("2031-03-03", 5, 2050),
            lesson_day("2031-03-05", 1, 1000),
        ],
    );
    let (code, ans) = other.index_code(2100);
    assert_eq!(code, 2, "{ans}");
    let failed = source(&ans, "lesson_record");
    assert_eq!(failed["status"], "source_failed", "{ans}");
    assert!(
        failed["cause"]
            .as_str()
            .unwrap()
            .starts_with("record_older_than_head@"),
        "{ans}"
    );
    assert_eq!(source(&ans, "lesson_day")["appended"]["update"], 1, "{ans}");
    let (_, coverage) = other.table(2100, "day_coverage", json!({"lesson_date":"2031-03-03"}));
    assert!(coverage.contains("| 2031-03-03 | 5 | 5 |"), "{coverage}");
    let (rows, _) = other.table(2100, "daily_records", json!({"lesson_date":"2031-03-03"}));
    assert_eq!(rows, 2);
}

/// The two lines of the contract, byte for byte as the capture family
/// `dailyrecord` writes them (members sorted, no space). The capture's own test
/// pins the same two lines from an invented page, as `a4` shows for the
/// textbook lines.
const LESSON_RECORD_LINE: &str = "{\"attendance\":\"Y\",\"attendance_label\":\"Present\",\"course_id\":\"3\",\"course_label\":\"Algebra A\",\"cut_fields\":0,\"daily_test_assessment\":\"2\",\"homework\":\"p.41-44\",\"lesson_date\":\"2031-03-03\",\"lesson_memo\":\"line one\\nline two | pipe\",\"membership_key\":\"50001\",\"observed_at\":1,\"previous_homework\":\"p.30-33\",\"previous_homework_assessment\":\"3\",\"previous_lesson_date_literal\":\"02/28\",\"previous_lesson_memo\":\"\",\"previous_progress\":\"Unit 2\",\"previous_record_key\":\"40\",\"progress\":\"Unit 3 to p.40\",\"record_key\":\"41\",\"row_ordinal\":0,\"source_namespace\":\"academy.lms.dayrecord\",\"student_id\":\"77001\",\"student_memo\":\"Invented memo\",\"student_name\":\"Ada Invented\",\"type\":\"lesson_record\"}";
const LESSON_DAY_LINE: &str = "{\"cut_fields\":0,\"lesson_date\":\"2031-03-03\",\"observed_at\":1,\"outside_roster_count\":0,\"row_count\":1,\"source_namespace\":\"academy.lms.dayrecord\",\"stored_count\":1,\"type\":\"lesson_day\"}";

#[test]
fn b7_the_contract_lines_are_indexed_and_no_cell_is_missing() {
    let capture_tests = include_str!("../../edu-academy-sources/src/dailyrecord_tests.rs");
    for line in [LESSON_RECORD_LINE, LESSON_DAY_LINE] {
        assert!(
            capture_tests.contains(&line.replace('\\', "\\\\").replace('"', "\\\"")),
            "capture test must pin the contract line"
        );
    }
    let env = Env::new();
    fs::write(
        env.partition("lesson_record", "2031-03"),
        format!("{LESSON_RECORD_LINE}\n"),
    )
    .unwrap();
    fs::write(
        env.partition("lesson_day", "2031"),
        format!("{LESSON_DAY_LINE}\n"),
    )
    .unwrap();
    let ans = env.index(2);
    assert_eq!(source(&ans, "lesson_record")["status"], "ok", "{ans}");
    assert_eq!(source(&ans, "lesson_day")["status"], "ok", "{ans}");
    for (view, args) in [
        ("daily_records", json!({"lesson_date":"2031-03-03"})),
        (
            "daily_record",
            json!({"lesson_date":"2031-03-03","student_id":"77001"}),
        ),
        (
            "record_history",
            json!({"student_id":"77001","since":"2031-03-01"}),
        ),
        ("day_coverage", json!({"lesson_date":"2031-03-03"})),
    ] {
        let (rows, md) = env.table(2, view, args);
        assert_eq!(rows, 1, "{md}");
        assert!(
            !md.contains("(missing)") && !md.contains("(null)") && !md.contains("(blank)"),
            "{md}"
        );
    }
    // A day and a half without an index run marks every cell; the age of a
    // capture is read from `Observed at`, never from this mark.
    let (_, stale) = env.table(
        2 + 129_600 + 1,
        "day_coverage",
        json!({"lesson_date":"2031-03-03"}),
    );
    assert!(
        stale.contains("| 2031-03-03 (stale) | 1 (stale) |"),
        "{stale}"
    );
    assert!(stale.contains("| 1970-01-01T00:00:01Z |"), "{stale}");
    // One index run that finds the line unchanged clears the mark, and the
    // time of the capture stays.
    env.index(2 + 129_600 + 2);
    let (_, fresh) = env.table(
        2 + 129_600 + 3,
        "day_coverage",
        json!({"lesson_date":"2031-03-03"}),
    );
    assert!(!fresh.contains("(stale)"), "{fresh}");
    assert!(fresh.contains("| 1970-01-01T00:00:01Z |"), "{fresh}");
}
