//! `edu lesson` against an invented LMS, an invented secret file, identity and roster,
//! a standing owner root made by the preparation's standing form and a disposable store.
//! Every identity, text, cookie and password here is invented; no network.
use edu_academy_core::workflow::Runtime;
use edu_academy_ontology::{ACADEMY_READ_SCHEMA, ACADEMY_READ_SPOOL_ROOT};
use edu_academy_sources::{
    data::{Result, unbase64},
    http::{self, Request, Response, Transport},
    prepare_owner,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PAGE: &str = include_str!("fixtures/day.html");
const PASSWORD: &str = "PASS-MARK-secret";
const COOKIE: &str = "COOKIE-MARK-session";
const NAMES: [&str; 2] = ["김한별", "Invented Newcomer"];

struct Clock;
impl Runtime for Clock {
    fn epoch_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
    fn now(&self) -> String {
        String::new()
    }
    fn uuid(&self) -> edu_academy_core::js::Result<String> {
        Ok("invented".into())
    }
}
fn day(offset: i64) -> String {
    edu::seoul_date(Clock.epoch_ms(), offset)
}

/// What the invented LMS saw and how it answers: the request paths in order and the page
/// it serves (the fixture with its date replaced by the asked date, then `edit`).
#[derive(Clone, Default)]
struct Lms {
    seen: Rc<RefCell<Vec<String>>>,
    edit: Rc<RefCell<Option<(String, String)>>>,
    connects: Rc<Cell<usize>>,
}
impl Lms {
    fn count(&self, path: &str) -> usize {
        self.seen
            .borrow()
            .iter()
            .filter(|p| p.as_str() == path)
            .count()
    }
}
struct Wire {
    lms: Lms,
    credentials_ok: bool,
}
fn field(r: &Request, name: &str) -> String {
    let value = &r.body.iter().find(|(k, _)| k == name).unwrap().1;
    String::from_utf8(unbase64(value).unwrap()).unwrap()
}
impl Transport for Wire {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        self.lms.seen.borrow_mut().push(r.path.clone());
        let mut headers = vec![];
        let body = if r.path == "/c_login.jsp" {
            headers.push((
                "Set-Cookie".into(),
                format!("JSESSIONID={COOKIE}; Path=/; HttpOnly"),
            ));
            let fields = [
                "p_process",
                "p_prino",
                "p_groupno",
                "userpwd",
                "userid",
                "cf_pc",
                "cf_user",
                "autoLoginCheck",
                "autoLoginKey",
            ]
            .iter()
            .map(|n| format!("<input name={n}>"))
            .collect::<String>();
            format!(
                r#"<form id="loginForm" method="post">{fields}</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>"#
            )
        } else if r.path == "/servlet/controller.login.LoginServlet" {
            // The secret file's password and the identity's user name reach the wire.
            self.credentials_ok = field(r, "userpwd").starts_with(&format!("{PASSWORD}  "))
                && field(r, "userid").starts_with("invented-user  ");
            String::new()
        } else if r.path == http::MAIN {
            let date = &r.body.iter().find(|(k, _)| k == "std_ymd").unwrap().1;
            let mut page = PAGE.replace(
                "2031-03-03",
                &format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]),
            );
            if let Some((from, to)) = self.lms.edit.borrow().as_ref() {
                page = page.replace(from, to);
            }
            page
        } else if self.credentials_ok {
            "<form name=userForm><input name=pri_no value=99001></form><form name=form1><input name=g_pri_no value=99001><input name=g_clg_no value=88001><input name=g_mem_type value=invented></form><a>로그아웃</a>".into()
        } else {
            "<form><input type=password name=again></form>".into()
        };
        Ok(Response {
            status: 200,
            headers,
            bytes: body.into_bytes(),
            url: format!("{}{}", http::ORIGIN, r.path),
            stalled: false,
        })
    }
}

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Env {
    root: PathBuf,
    lms: Lms,
}
struct Run {
    line: Value,
    rest: Vec<String>,
    code: i32,
    stdout: String,
}
fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
fn private_file(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
impl Env {
    fn new() -> Self {
        Self::with(30, ACADEMY_READ_SCHEMA)
    }
    /// `schema`: the text installed into the store (the embedded one, or a changed one).
    fn with(wait: u64, schema: &str) -> Self {
        let root =
            PathBuf::from(std::env::var("S10_TEST_ROOT").expect("synthetic root")).join(format!(
                "edu-lesson-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
        private_dir(&root);
        let shared = root.join("shared.lock");
        fs::write(&shared, b"").unwrap();
        private_file(
            &root.join("identity.json"),
            &json!({"teacherSourceKey":"99001","teacherLabel":"Invented Teacher","collegeSourceKey":"88001","sourceUsername":"invented-user"}).to_string(),
        );
        private_file(
            &root.join("secret.env"),
            &format!("OTHER=x\nLESSON_PASSWORD=\"{PASSWORD}\"\n"),
        );
        private_file(
            &root.join("students.json"),
            &json!([{"student_id":"77001","student_name":"Invented roster one"},{"student_id":"77003","student_name":"Invented roster two"}]).to_string(),
        );
        let args = [
            "--identity",
            &root.join("identity.json").display().to_string(),
            "--lock",
            &shared.display().to_string(),
            "--students",
            &root.join("students.json").display().to_string(),
            "--allowed-from",
            &day(-400),
            "--allowed-through",
            &day(400),
            "--owner-root",
            &root.join("owner").display().to_string(),
            "--families",
            "dailyrecord",
        ]
        .map(String::from);
        let (inputs, families) = prepare_owner::parse_standing_args(args.into_iter()).unwrap();
        let prepared = prepare_owner::prepare_standing(&inputs, &families, &Clock).unwrap();
        assert!(!root.join("owner/credential.json").exists());
        let pin = |name: &str| {
            prepared["documents"][name]["sha256"]
                .as_str()
                .unwrap()
                .to_string()
        };
        let spool = root.join("spool");
        private_dir(&root.join("store"));
        let store = root.join("store/store.sqlite");
        let text = schema.replace(ACADEMY_READ_SPOOL_ROOT, &spool.display().to_string());
        fs::write(root.join("schema.json"), text).unwrap();
        let installed = ont::run(
            &[
                "--store".into(),
                store.display().to_string(),
                "--now".into(),
                "1".into(),
                "schema".into(),
                "install".into(),
                root.join("schema.json").display().to_string(),
            ],
            &b""[..],
            0,
            0,
        );
        assert_eq!(installed.code, 0, "{}", installed.stdout);
        private_dir(&root.join("raw"));
        let config = json!({
            "schemaVersion":"edu.lesson-config/1",
            "ownerRoot":root.join("owner"),
            "admissionSha256":pin("transport-admission.json"),
            "rosterSha256":pin("roster.json"),
            "spool":spool,
            "store":store,
            "secretFile":root.join("secret.env"),
            "secretKey":"LESSON_PASSWORD",
            "identityFile":root.join("identity.json"),
            "rawDir":root.join("raw"),
            "lockWaitSeconds":wait,
        });
        private_file(&root.join("lesson.json"), &config.to_string());
        Self {
            root,
            lms: Lms::default(),
        }
    }
    /// The engine answers the ids 0 and 0 only; the other engine tests pass them too.
    fn edu(&self, args: &[&str]) -> Run {
        self.edu_as((0, 0), args)
    }
    fn edu_as(&self, ids: (u32, u32), args: &[&str]) -> Run {
        self.lms.seen.borrow_mut().clear();
        let mut all = vec![
            "--config".to_string(),
            self.root.join("lesson.json").display().to_string(),
            "lesson".into(),
        ];
        all.extend(args.iter().map(|s| s.to_string()));
        let lms = self.lms.clone();
        let out = edu::run(&all, &Clock, ids, &mut |_: &http::Cancellation| {
            lms.connects.set(lms.connects.get() + 1);
            Ok(Wire {
                lms: lms.clone(),
                credentials_ok: false,
            })
        });
        let mut lines = out.stdout.lines();
        let line = serde_json::from_str(lines.next().expect("line 1")).expect("line 1 is JSON");
        let rest = lines.map(String::from).collect();
        // Every refusal is exactly one line and exit code 2.
        let run = Run {
            line,
            rest,
            code: out.code,
            stdout: out.stdout,
        };
        if run.line["status"] != "ok" {
            assert_eq!(run.code, 2, "{}", run.stdout);
            assert!(run.rest.is_empty(), "{}", run.stdout);
        } else {
            assert_eq!(run.code, 0, "{}", run.stdout);
        }
        // No cookie or password anywhere; no name or record text in line 1.
        for marker in [COOKIE, PASSWORD] {
            assert!(!run.stdout.contains(marker), "{}", run.stdout);
        }
        let first = run.line.to_string();
        for marker in ["TXT-MARK", NAMES[0], NAMES[1], "invented-user"] {
            assert!(!first.contains(marker), "{first}");
        }
        run
    }
    fn log(&self) -> Vec<Value> {
        let path = self.root.join("store/lesson-runs.jsonl");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let text = fs::read_to_string(path).unwrap();
        for marker in [
            COOKIE, PASSWORD, "TXT-MARK", NAMES[0], NAMES[1], "77001", "77003",
        ] {
            assert!(!text.contains(marker), "{text}");
        }
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    fn raw_files_clean(&self) {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
        let mut files = vec![];
        walk(&self.root.join("raw"), &mut files);
        assert!(!files.is_empty());
        for file in files {
            let bytes = String::from_utf8_lossy(&fs::read(file).unwrap()).into_owned();
            assert!(!bytes.contains(COOKIE) && !bytes.contains(PASSWORD));
        }
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn table_rows(run: &Run, prefix: &str) -> usize {
    run.rest.iter().filter(|l| l.starts_with(prefix)).count()
}

#[test]
fn a3_day_twice_then_record_by_a_korean_name_then_history() {
    // Checks specs/edu/lesson.md#R1, specs/edu/lesson.md#R4, specs/edu/lesson.md#R5, specs/edu/lesson.md#R6, specs/edu/lesson.md#R17, specs/edu/lesson.md#R24, specs/edu/lesson.md#R25, specs/edu/lesson.md#R26, specs/edu/lesson.md#R27, specs/edu/lesson.md#R30, specs/edu/lesson.md#R31, specs/edu/lesson.md#R36, specs/edu/lesson.md#R37, specs/edu/lesson.md#R38, specs/edu/lesson.md#R42.
    let env = Env::new();
    let date = day(-3);
    let first = env.edu(&["day", &date]);
    let line = &first.line;
    assert_eq!(line["status"], "ok");
    assert_eq!(line["date"], date.as_str());
    assert_eq!(line["login"], "new");
    assert_eq!(
        line["requests"].as_u64().unwrap() as usize,
        env.lms.seen.borrow().len()
    );
    assert_eq!(env.lms.count("/servlet/controller.login.LoginServlet"), 1);
    assert_eq!(env.lms.count(http::MAIN), 1);
    assert_eq!(
        (
            &line["rows"],
            &line["stored"],
            &line["outside_roster"],
            &line["cut_fields"]
        ),
        (&json!(2), &json!(2), &json!(0), &json!(0))
    );
    assert_eq!(line["lock_wait_seconds"], 0);
    let captured = line["captured_at"].as_u64().unwrap();
    assert!(Clock.epoch_ms() / 1000 - captured < 60);
    assert!(line["age_seconds"].as_u64().unwrap() < 60);
    assert_eq!(table_rows(&first, "| 77001 | 김한별 |"), 1);
    assert_eq!(table_rows(&first, "| 77003 | Invented Newcomer |"), 1);

    let second = env.edu(&["day", &date]);
    assert_eq!(second.line["login"], "none");
    assert_eq!(second.line["requests"], 0);
    assert!(env.lms.seen.borrow().is_empty());
    assert_eq!(second.line["captured_at"], captured);
    assert_eq!(second.rest, first.rest);

    let record = env.edu(&["record", &date, "김한별"]);
    assert_eq!(record.line["login"], "none");
    assert_eq!(record.line["student_id"], "77001");
    assert_eq!(record.line["captured_at"], captured);
    assert_eq!(table_rows(&record, "| 77001 | 김한별 |"), 1);
    assert!(record.stdout.contains("TXT-MARK-progress"));
    assert_eq!(table_rows(&record, "| 77003 |"), 0);

    let history = env.edu(&["history", "77001"]);
    assert_eq!(history.line["login"], "none");
    assert_eq!(history.line["requests"], 0);
    assert_eq!(history.line["since"], day(-56).as_str());
    assert_eq!(history.line["rows"], 1);
    assert_eq!(history.line["captured_at"], captured);
    assert_eq!(table_rows(&history, &format!("| {date} |")), 1);
    assert!(env.lms.seen.borrow().is_empty());
    assert_eq!(env.lms.connects.get(), 1);

    let log = env.log();
    assert_eq!(log.len(), 4);
    let verbs: Vec<_> = log.iter().map(|l| l["verb"].clone()).collect();
    assert_eq!(verbs, ["day", "day", "record", "history"]);
    let logins: Vec<_> = log.iter().map(|l| l["login"].clone()).collect();
    assert_eq!(logins, ["new", "none", "none", "none"]);
    assert!(log.iter().all(|l| l["status"] == "ok" && l["dates"] == 1));
    assert_eq!(log[0]["requests"], first.line["requests"]);
    assert!(
        !fs::read_to_string(env.root.join("store/lesson-runs.jsonl"))
            .unwrap()
            .contains(&date)
    );
    env.raw_files_clean();
}

#[test]
fn i0_a_malformed_or_future_date_is_refused_before_any_request_or_engine_call() {
    // Checks specs/edu/lesson.md#R2, specs/edu/lesson.md#R8, specs/edu/lesson.md#R9, specs/edu/lesson.md#R11, specs/edu/lesson.md#R21.
    // The store holds another schema: an engine call would end with its code instead.
    let env = Env::with(
        30,
        &ACADEMY_READ_SCHEMA.replace("\"version\": 2", "\"version\": 3"),
    );
    for bad in ["2040-6-1", "20400102", "2040-13-45", ""] {
        for args in [
            vec!["day", bad],
            vec!["day", bad, "--refresh"],
            vec!["record", bad, "77001"],
            vec!["refresh", bad],
            vec!["refresh", "2026-01-01", bad],
            vec!["history", "77001", "--since", bad],
        ] {
            let run = env.edu(&args);
            assert_eq!(
                run.line,
                json!({"status":"lesson_date_invalid"}),
                "{args:?}"
            );
        }
    }
    let tomorrow = day(1);
    for args in [
        vec!["day", tomorrow.as_str()],
        vec!["record", tomorrow.as_str(), "77001"],
        vec!["refresh", tomorrow.as_str()],
    ] {
        let run = env.edu(&args);
        assert_eq!(
            run.line,
            json!({"status":"lesson_date_in_future"}),
            "{args:?}"
        );
    }
    let schema = env.edu(&["history", "77001"]);
    assert_eq!(schema.line, json!({"status":"lesson_schema_mismatch"}));
    assert_eq!(env.lms.connects.get(), 0);
    assert!(env.lms.seen.borrow().is_empty());
}

#[test]
fn i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date() {
    // Checks specs/edu/lesson.md#R1, specs/edu/lesson.md#R4, specs/edu/lesson.md#R17, specs/edu/lesson.md#R24, specs/edu/lesson.md#R26, specs/edu/lesson.md#R27, specs/edu/lesson.md#R32, specs/edu/lesson.md#R37, specs/edu/lesson.md#R43.
    let env = Env::new();
    let dates = [day(-5), day(-4), day(-3)];
    let run = env.edu(&["refresh", &dates[0], &dates[1], &dates[2]]);
    assert_eq!(run.line["login"], "new");
    assert_eq!(run.line["dates"], 3);
    assert_eq!(env.lms.count("/servlet/controller.login.LoginServlet"), 1);
    assert_eq!(env.lms.count("/c_login.jsp"), 1);
    assert_eq!(env.lms.count(http::MAIN), 3);
    assert_eq!(run.rest.len(), 3);
    for (line, date) in run.rest.iter().zip(&dates) {
        let coverage: Value = serde_json::from_str(line).unwrap();
        assert_eq!(coverage["date"], date.as_str());
        assert_eq!(coverage["rows"], 2);
        assert!(coverage["captured_at"].as_u64().is_some());
    }
    // A captured date is read without a request; `--refresh` captures it again, one login.
    let read = env.edu(&["day", &dates[1]]);
    assert_eq!(
        (&read.line["login"], &read.line["requests"]),
        (&json!("none"), &json!(0))
    );
    let again = env.edu(&["day", &dates[1], "--refresh"]);
    assert_eq!(again.line["login"], "new");
    assert_eq!(env.lms.count("/servlet/controller.login.LoginServlet"), 1);
    assert_eq!(env.lms.count(http::MAIN), 1);
    assert!(again.line["captured_at"].as_u64() >= read.line["captured_at"].as_u64());
    // The family's own writer and recorder wrote the spool and the raw answers.
    let year = fs::read_to_string(
        env.root
            .join(format!("spool/lesson_day/{}.jsonl", &dates[0][..4])),
    )
    .unwrap();
    let line: Value =
        serde_json::from_str(year.lines().find(|l| l.contains(&dates[1])).unwrap()).unwrap();
    assert_eq!(
        line.as_object().unwrap().keys().collect::<Vec<_>>(),
        [
            "cut_fields",
            "lesson_date",
            "observed_at",
            "outside_roster_count",
            "row_count",
            "source_namespace",
            "stored_count",
            "type"
        ]
    );
    assert!(env.root.join("spool/.lesson-record.lock").exists());
    let index = fs::read_to_string(env.root.join("raw/77001/index.jsonl")).unwrap();
    assert_eq!(index.lines().count(), 4);
    env.raw_files_clean();
    let log = env.log();
    assert_eq!(log[0]["dates"], 3);
    assert_eq!(log.len(), 3);
}

#[test]
fn i3_refusals_carry_no_cookie_password_name_or_text() {
    // Checks specs/edu/lesson.md#R2, specs/edu/lesson.md#R4, specs/edu/lesson.md#R5, specs/edu/lesson.md#R7, specs/edu/lesson.md#R12, specs/edu/lesson.md#R13, specs/edu/lesson.md#R17, specs/edu/lesson.md#R28, specs/edu/lesson.md#R31, specs/edu/lesson.md#R39.
    let env = Env::new();
    let date = day(-2);
    // A page of another date: the family refuses it, the run prints its code only.
    *env.lms.edit.borrow_mut() = Some((
        format!("name=\"std_date\" value=\"{date}\""),
        "name=\"std_date\" value=\"2001-01-01\"".into(),
    ));
    let run = env.edu(&["day", &date]);
    assert_eq!(run.line, json!({"status":"capture_day_date_conflict"}));
    *env.lms.edit.borrow_mut() = None;
    // A wrong password in the secret file: the login fails with the session's fixed code.
    private_file(
        &env.root.join("secret.env"),
        "LESSON_PASSWORD=\"PASS-MARK-other\"\n",
    );
    let run = env.edu(&["day", &date]);
    assert_eq!(
        run.line,
        json!({"status":"native_owner_authentication_failed"})
    );
    assert!(!run.stdout.contains("PASS-MARK-other"));
    private_file(
        &env.root.join("secret.env"),
        &format!("LESSON_PASSWORD={PASSWORD}\n"),
    );
    let ok = env.edu(&["day", &date]);
    assert_eq!(ok.line["status"], "ok");
    let missing = env.edu(&["record", &date, "없는이름"]);
    assert_eq!(missing.line, json!({"status":"lesson_student_not_found"}));
    for bad in ["1234567890123", "a\u{7}b"] {
        let run = env.edu(&["record", &date, bad]);
        assert_eq!(run.line, json!({"status":"lesson_student_invalid"}));
    }
    let run = env.edu(&["history", "김한별"]);
    assert_eq!(run.line, json!({"status":"lesson_student_invalid"}));
    for args in [
        vec!["day"],
        vec!["dance"],
        vec!["day", &date, "--exact"],
        vec!["history", "77001", "--refresh"],
    ] {
        assert_eq!(
            env.edu(&args).line,
            json!({"status":"lesson_usage_invalid"}),
            "{args:?}"
        );
    }
    let statuses: Vec<_> = env.log().iter().map(|l| l["status"].clone()).collect();
    assert_eq!(
        statuses[..3],
        [
            "capture_day_date_conflict",
            "native_owner_authentication_failed",
            "ok"
        ]
    );
    env.raw_files_clean();
}

#[test]
fn a_name_on_two_students_is_refused_with_their_count_and_exact_prints_members() {
    // Checks specs/edu/lesson.md#R40, specs/edu/lesson.md#R41.
    let env = Env::new();
    let date = day(-1);
    *env.lms.edit.borrow_mut() = Some(("Invented Newcomer".into(), "김한별".into()));
    env.edu(&["day", &date]);
    let run = env.edu(&["record", &date, "김한별"]);
    assert_eq!(
        run.line,
        json!({"status":"lesson_student_ambiguous","count":2})
    );
    let exact = env.edu(&["record", &date, "77001", "--exact"]);
    assert_eq!(exact.line["student_id"], "77001");
    assert_eq!(exact.rest.len(), 1);
    let row: Value = serde_json::from_str(&exact.rest[0]).unwrap();
    assert_eq!(row["lesson_memo"], "TXT-MARK-memo line one\nline two");
    assert_eq!(row["previous_record_key"], "40");
    assert_eq!(row["student_name"], "김한별");
    assert!(row["observed_at"].as_u64().is_some());
    let first = env.edu(&["record", &date, "77003", "--exact"]);
    let row: Value = serde_json::from_str(&first.rest[0]).unwrap();
    assert!(row.get("previous_record_key").is_none());
    assert_eq!(row["previous_homework_assessment"], "");
}

#[test]
fn a_caller_that_is_not_root_is_refused_by_the_engine_rule() {
    // Checks specs/edu/lesson.md#R17, specs/edu/lesson.md#R19.
    let env = Env::new();
    // A read of the secret file would end with `native_owner_credential_refused`.
    fs::remove_file(env.root.join("secret.env")).unwrap();
    for ids in [(65534, 65534), (0, 65534), (65534, 0)] {
        for args in [
            vec!["day", "2026-01-02", "--refresh"],
            vec!["history", "77001"],
        ] {
            let run = env.edu_as(ids, &args);
            assert_eq!(
                run.line,
                json!({"status":"principal_not_trusted"}),
                "{ids:?} {args:?}"
            );
        }
    }
    assert_eq!(env.lms.connects.get(), 0);
    assert!(env.lms.seen.borrow().is_empty());
    assert!(!env.root.join("secret.env").exists());
    // Before the engine: nothing of the spool was made.
    assert!(!env.root.join("spool").exists());
    assert!(
        env.log()
            .iter()
            .all(|l| l["status"] == "principal_not_trusted" && l["requests"] == 0)
    );
}

#[test]
fn i5_a_store_with_another_schema_is_refused_before_any_request() {
    // Checks specs/edu/lesson.md#R21.
    let env = Env::with(
        30,
        &ACADEMY_READ_SCHEMA.replacen("\"description\": \"", "\"description\": \"Changed. ", 1),
    );
    for args in [
        vec!["day", "2026-01-02", "--refresh"],
        vec!["refresh", "2026-01-02"],
    ] {
        let run = env.edu(&args);
        assert_eq!(run.line, json!({"status":"lesson_schema_mismatch"}));
    }
    assert_eq!(env.lms.connects.get(), 0);
    assert!(env.lms.seen.borrow().is_empty());
}

#[test]
fn a_busy_account_lock_is_retried_for_the_configured_seconds() {
    // Checks specs/edu/lesson.md#R17, specs/edu/lesson.md#R29, specs/edu/lesson.md#R30.
    let env = Env::with(1, ACADEMY_READ_SCHEMA);
    let lock = fs::File::open(env.root.join("shared.lock")).unwrap();
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive).unwrap();
    let start = Instant::now();
    let run = env.edu(&["day", &day(-1)]);
    assert!(start.elapsed() >= Duration::from_secs(1));
    assert_eq!(run.line, json!({"status":"owner_account_busy"}));
    assert!(env.lms.seen.borrow().is_empty());
    let log = env.log();
    assert_eq!(log[0]["status"], "owner_account_busy");
    assert!(log[0]["lock_wait_seconds"].as_u64().unwrap() >= 1);
    // Released while the run waits: the run proceeds and names the seconds it waited.
    let env = Env::with(10, ACADEMY_READ_SCHEMA);
    let path = env.root.join("shared.lock");
    let (held, wait) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let lock = fs::File::open(path).unwrap();
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive).unwrap();
        held.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(1500));
    });
    wait.recv().unwrap();
    let run = env.edu(&["day", &day(-1)]);
    holder.join().unwrap();
    assert_eq!(run.line["status"], "ok");
    assert!(run.line["lock_wait_seconds"].as_u64().unwrap() >= 1);
    assert_eq!(
        env.log()[0]["lock_wait_seconds"],
        run.line["lock_wait_seconds"]
    );
}

#[test]
fn the_configuration_file_is_private_and_exact_and_a_failed_index_shows_no_table() {
    // Checks specs/edu/lesson.md#R2, specs/edu/lesson.md#R3, specs/edu/lesson.md#R15, specs/edu/lesson.md#R16, specs/edu/lesson.md#R23.
    let env = Env::new();
    let config = env.root.join("lesson.json");
    fs::set_permissions(&config, fs::Permissions::from_mode(0o620)).unwrap();
    assert_eq!(
        env.edu(&["history", "77001"]).line,
        json!({"status":"lesson_config_refused"})
    );
    fs::set_permissions(&config, fs::Permissions::from_mode(0o644)).unwrap();
    let mut value: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    value["unknown"] = json!(1);
    private_file(&config, &value.to_string());
    assert_eq!(
        env.edu(&["history", "77001"]).line,
        json!({"status":"lesson_config_refused"})
    );
    value.as_object_mut().unwrap().remove("unknown");
    value.as_object_mut().unwrap().remove("rawDir");
    value.as_object_mut().unwrap().remove("lockWaitSeconds");
    private_file(&config, &value.to_string());
    let date = day(-1);
    assert_eq!(env.edu(&["day", &date]).line["status"], "ok");
    private_file(
        &env.root
            .join(format!("spool/lesson_record/{}.jsonl", &date[..7])),
        "not a line\n",
    );
    for args in [vec!["history", "77001"], vec!["day", date.as_str()]] {
        assert_eq!(env.edu(&args).line, json!({"status":"lesson_index_failed"}));
    }
    // No configuration at all: refused, nothing on standard error.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_edu"))
        .args([
            "--config",
            &env.root.join("absent.json").display().to_string(),
            "lesson",
            "history",
            "77001",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "{\"status\":\"lesson_config_refused\"}\n"
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn seoul_dates_follow_the_calendar() {
    // Checks specs/edu/lesson.md#R10.
    // 2024-02-28T15:00:00Z is 2024-02-29 00:00 in Seoul.
    assert_eq!(edu::seoul_date(1_709_132_400_000, 0), "2024-02-29");
    assert_eq!(edu::seoul_date(1_709_132_399_999, 0), "2024-02-28");
    assert_eq!(edu::seoul_date(1_709_132_400_000, 1), "2024-03-01");
    assert_eq!(edu::seoul_date(1_709_132_400_000, -60), "2023-12-31");
    assert_eq!(edu::seoul_date(0, 0), "1970-01-01");
}
