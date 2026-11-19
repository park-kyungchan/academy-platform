//! Invented identities and pages, native owner workflow, in-memory transport; no sockets.
use super::*;
use crate::{
    capture::{Family, Recorder, RecordingPort, RecordingTransport, capture_dailytest, contracts},
    http::{self, Cancellation, Request, Response, Transport},
    owner_admission::TransportAdmission,
    owner_root::OwnerRoot,
    owner_workflow::{Credentials, NativeSession},
    student_course::{COURSE_NS, DAILY_PATH},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        "2040-01-04T09:00:00.000Z".into()
    }
    fn epoch_ms(&self) -> u64 {
        1000
    }
    fn uuid(&self) -> edu_academy_core::js::Result<String> {
        Ok("synthetic-dailytest".into())
    }
}
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = PathBuf::from(
            std::env::var("S09_NATIVE_FIXTURE_ROOT").expect("synthetic root outside Git"),
        )
        .join(format!(
            "dailytest-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        fs::write(p.join("shared.lock"), b"synthetic lock").unwrap();
        fs::write(
            p.join("account-lock.json"),
            serde_json::json!({"schemaVersion":"academy.account-lock/1","path":p.join("shared.lock")})
                .to_string(),
        )
        .unwrap();
        fs::set_permissions(
            p.join("account-lock.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        Self(p)
    }
    fn options(&self, max: usize) -> Options {
        Options {
            date: "2040-01-04".into(),
            max_requests: max,
            no_detail: false,
            raw_dir: None,
            spool: self.0.join("spool"),
        }
    }
    fn root(&self) -> Rc<OwnerRoot> {
        Rc::new(OwnerRoot::open(&self.0).unwrap())
    }
    fn raw(&self) -> PathBuf {
        let raw = self.0.join("raw");
        if !raw.exists() {
            fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
        }
        raw
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn admission(families: &[Family]) -> TransportAdmission {
    TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "fictional-owner".into(),
        teacher_id: "99001".into(),
        teacher_label: "Fictional Teacher".into(),
        college_id: "88001".into(),
        allowed_from: "2040-01-01".into(),
        allowed_through: "2040-01-31".into(),
        student_ids: vec!["77001".into()],
        source_namespaces: vec![COURSE_NS.into(), METADATA_NS.into()],
        operations: crate::capture::contracts_for(families),
    }
}
fn students() -> Vec<Student> {
    vec![Student {
        student_id: "77001".into(),
        student_name: "Invented Learner".into(),
    }]
}
fn row(paper: u64, attempt: u64, id: Option<&str>, score: Option<&str>) -> String {
    let mut s = format!(
        r#"{{"priNo":77001,"userName":"Invented Learner","pNo":{paper},"testingNo":{attempt},"testingName":"Fictional test {attempt}""#
    );
    if let Some(id) = id {
        s += &format!(r#","_id":"{id}""#);
    }
    if let Some(score) = score {
        s += &format!(r#","score":{score}"#);
    }
    s + "}"
}
/// `noise` puts other inline script around the literal: many tokens, a regular
/// expression literal, a template literal and divisions.
fn page(options: &str, rows: &[String], noise: bool) -> String {
    let literal = format!("testResultList: JSON.parse('[{}]')", rows.join(","));
    let (before, after) = if noise {
        let tokens = "var filler = 1 + 2 / 3; ".repeat(1500);
        (
            format!(
                "<script>{tokens} const rx = /a+'\\/(b)/g; const t = `template ${{1/2}} \\` still`; const quotient = 4 / 2 / 1;</script>"
            ),
            "<script>const after = 8 / 2; function onSearch(){ return /x/.test(`${1/1}`); }</script>"
                .to_string(),
        )
    } else {
        (String::new(), String::new())
    };
    format!(
        "{before}<form name=\"search\"><select name=\"sort_date\">{options}</select></form><script>const results = {{{literal}}};</script>{after}"
    )
}
const OPTIONS: &str =
    "<option value=\"1\">Recent week</option><option value=\"11\">Recent month</option>";
fn period_one() -> Vec<String> {
    vec![
        row(81001, 82001, Some("r1"), Some("90")),
        row(81002, 82002, Some("r2"), None),
        row(81003, 82003, Some("r3"), Some("87.5")),
    ]
}
fn period_eleven() -> Vec<String> {
    vec![
        row(81004, 82004, Some("bad id!"), Some("100")),
        row(81005, 82005, None, Some("0")),
    ]
}
fn pages(noise: bool) -> HashMap<String, String> {
    HashMap::from([
        ("1".into(), page(OPTIONS, &period_one(), noise)),
        ("11".into(), page(OPTIONS, &period_eleven(), noise)),
    ])
}
const OWNER: &str = "<form name=userForm><input name=pri_no value=99001></form><form name=form1><input name=g_pri_no value=99001><input name=g_clg_no value=88001><input name=g_mem_type value=fictional></form>";
struct Synthetic {
    seen: Rc<RefCell<Vec<String>>>,
    pages: HashMap<String, String>,
    fail_on: Option<String>,
}
impl Transport for Synthetic {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        self.seen.borrow_mut().push(r.path.clone());
        let mut headers = vec![];
        let body = if r.path == "/c_login.jsp" {
            headers.push((
                "Set-Cookie".into(),
                "JSESSIONID=syntheticdailytest; Path=/; HttpOnly".into(),
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
            "accepted".into()
        } else if r.path == DAILY_PATH {
            let selector = r
                .body
                .iter()
                .find(|(k, _)| k == "sort_date")
                .map(|(_, v)| v.clone())
                .unwrap();
            if self.fail_on.as_deref() == Some(selector.as_str()) {
                return Err(Error::error("synthetic_failure"));
            }
            self.pages.get(&selector).cloned().unwrap_or_default()
        } else {
            format!("{OWNER}<a>로그아웃</a>")
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
type Port = RecordingPort<'static, NativeSession<'static, RecordingTransport<'static, Synthetic>>>;
fn session(
    f: &Fixture,
    families: &[Family],
    pages: HashMap<String, String>,
    fail_on: Option<&str>,
    raw: Option<PathBuf>,
    seen: Rc<RefCell<Vec<String>>>,
) -> Port {
    let recorder = Recorder::new(raw, &Clock);
    let session = NativeSession::open(
        f.root(),
        admission(families),
        recorder.transport(Synthetic {
            seen,
            pages,
            fail_on: fail_on.map(Into::into),
        }),
        || {
            Ok(Credentials::new(
                "fictional-user".into(),
                "fictional-password".into(),
            ))
        },
        Cancellation::default(),
        &Clock,
    )
    .unwrap();
    recorder.port(session)
}
fn line(
    key: &str,
    attempt: &str,
    paper: &str,
    period: (&str, usize),
    row: usize,
    id: &str,
    score: &str,
) -> String {
    format!(
        "{{\"attempt_key\":\"{attempt}\",\"key\":\"{key}\",\"observed_at\":1,\"paper_key\":\"{paper}\",\"period\":\"{}\",\"period_ordinal\":{},\"row_ordinal\":{row},\"source_row_id\":{id},\"source_score\":{score},\"student\":\"77001\",\"student_id\":\"77001\",\"type\":\"paper_attempt\"}}\n",
        period.0, period.1
    )
}
fn expected() -> String {
    [
        line(
            "77001|82001",
            "82001",
            "81001",
            ("1", 1),
            1,
            "\"r1\"",
            "\"90\"",
        ),
        line(
            "77001|82002",
            "82002",
            "81002",
            ("1", 1),
            2,
            "\"r2\"",
            "null",
        ),
        line(
            "77001|82003",
            "82003",
            "81003",
            ("1", 1),
            3,
            "\"r3\"",
            "\"87.5\"",
        ),
        line(
            "77001|82004",
            "82004",
            "81004",
            ("11", 2),
            1,
            "null",
            "\"100\"",
        ),
        line(
            "77001|82005",
            "82005",
            "81005",
            ("11", 2),
            2,
            "null",
            "\"0\"",
        ),
    ]
    .concat()
}
fn partition(spool: &Path) -> PathBuf {
    spool.join("paper_attempt/77001.jsonl")
}

#[test]
fn two_periods_five_rows_exact_bytes_one_login_two_requests() {
    let f = Fixture::new();
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session(
        &f,
        &[Family::DailyTest],
        pages(false),
        None,
        None,
        seen.clone(),
    );
    let options = f.options(4);
    let report = capture_dailytest(&mut s, &students(), &options, &Clock);
    assert_eq!(report[0].status, "refreshed", "{:?}", report[0].cause);
    assert_eq!(report[0].requests, 2);
    assert_eq!(report[0].paper_attempts, Some(5));
    assert_eq!(
        serde_json::to_string(&report[0]).unwrap(),
        "{\"roster_ordinal\":1,\"status\":\"refreshed\",\"requests\":2,\"registered_books\":0,\"section_attempts\":0,\"paper_attempts\":5}"
    );
    assert_eq!(
        seen.borrow()
            .iter()
            .filter(|p| p.as_str() == "/c_login.jsp")
            .count(),
        1
    );
    assert_eq!(
        fs::read_to_string(partition(&options.spool)).unwrap(),
        expected()
    );
    assert_eq!(
        fs::metadata(partition(&options.spool))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    // Only the family's partition exists: nothing of the textbook records.
    assert!(!options.spool.join("registered_book").exists());
    assert!(!options.spool.join("section_attempt").exists());
}

#[test]
fn failure_in_second_period_keeps_old_partition_and_reports_cause() {
    let f = Fixture::new();
    let options = f.options(4);
    {
        let mut s = session(
            &f,
            &[Family::DailyTest],
            pages(false),
            None,
            None,
            Rc::default(),
        );
        assert_eq!(
            capture_dailytest(&mut s, &students(), &options, &Clock)[0].status,
            "refreshed"
        );
    }
    let old = fs::read(partition(&options.spool)).unwrap();
    let mut s = session(
        &f,
        &[Family::DailyTest],
        pages(false),
        Some("11"),
        None,
        Rc::default(),
    );
    let r = capture_dailytest(&mut s, &students(), &options, &Clock);
    assert_eq!(r[0].status, "capture_request_failed");
    assert!(!r[0].cause.is_empty());
    assert_eq!(r[0].requests, 2);
    assert_eq!(r[0].paper_attempts, None);
    assert_eq!(fs::read(partition(&options.spool)).unwrap(), old);
}

#[test]
fn budget_stops_before_the_excess_request_and_keeps_the_old_partition() {
    let f = Fixture::new();
    let options = f.options(4);
    {
        let mut s = session(
            &f,
            &[Family::DailyTest],
            pages(false),
            None,
            None,
            Rc::default(),
        );
        capture_dailytest(&mut s, &students(), &options, &Clock);
    }
    let old = fs::read(partition(&options.spool)).unwrap();
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session(
        &f,
        &[Family::DailyTest],
        pages(false),
        None,
        None,
        seen.clone(),
    );
    let r = capture_dailytest(&mut s, &students(), &f.options(1), &Clock);
    assert_eq!(r[0].status, "capture_request_limit");
    assert_eq!(r[0].requests, 1);
    assert_eq!(
        seen.borrow()
            .iter()
            .filter(|p| p.as_str() == DAILY_PATH)
            .count(),
        1
    );
    assert_eq!(fs::read(partition(&options.spool)).unwrap(), old);
}

#[test]
fn outside_student_and_admission_without_the_family_dispatch_nothing() {
    let f = Fixture::new();
    // A student outside the admission.
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session(
        &f,
        &[Family::DailyTest],
        pages(false),
        None,
        None,
        seen.clone(),
    );
    let mut outside = students();
    outside[0].student_id = "77002".into();
    let r = capture_dailytest(&mut s, &outside, &f.options(4), &Clock);
    assert_eq!(r[0].status, "capture_admission_refused");
    assert_eq!(r[0].requests, 0);
    assert!(seen.borrow().is_empty());
    drop(s);
    // An admission that lists the textbook operations only.
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session(
        &f,
        &[Family::Textbook],
        pages(false),
        None,
        None,
        seen.clone(),
    );
    let r = capture_dailytest(&mut s, &students(), &f.options(4), &Clock);
    assert_eq!(r[0].status, "capture_admission_refused");
    assert!(
        r[0].cause
            .contains(&"student_read_contract_not_admitted".to_string())
    );
    assert_eq!(r[0].requests, 0);
    assert!(seen.borrow().is_empty());
    assert!(!f.options(4).spool.exists());
}

#[test]
fn repeated_attempt_key_is_made_unique_by_paper_and_period() {
    let f = Fixture::new();
    let mut p = pages(false);
    // The same attempt in both periods, and a shared attempt key of another paper.
    p.insert(
        "11".into(),
        page(
            OPTIONS,
            &[
                row(81001, 82001, Some("r1"), Some("90")),
                row(81009, 82002, Some("r9"), None),
            ],
            false,
        ),
    );
    let mut s = session(&f, &[Family::DailyTest], p, None, None, Rc::default());
    let options = f.options(4);
    assert_eq!(
        capture_dailytest(&mut s, &students(), &options, &Clock)[0].status,
        "refreshed"
    );
    let text = fs::read_to_string(partition(&options.spool)).unwrap();
    let keys: Vec<String> = text
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["key"]
                .as_str()
                .unwrap()
                .into()
        })
        .collect();
    assert_eq!(
        keys,
        [
            "77001|82001|81001|1",
            "77001|82002|81002|1",
            "77001|82003",
            "77001|82001|81001|11",
            "77001|82002|81009|11"
        ]
    );
    assert_eq!(keys.iter().collect::<HashSet<_>>().len(), keys.len());
}

#[test]
fn source_parity_ignores_other_script_in_every_parser_reached() {
    // metadata_rows_by_expression <- TypeScript inspectAssessmentSearchMetadata, and the
    // sort_date options <- readDailyTestHistory: a page with 10,000 other script tokens,
    // a regular-expression literal, a template literal and divisions around the value
    // yields the rows of the plain fixture.
    let noisy = pages(true);
    assert!(noisy["1"].len() > 30_000);
    let f = Fixture::new();
    let mut s = session(&f, &[Family::DailyTest], noisy, None, None, Rc::default());
    let options = f.options(4);
    assert_eq!(
        capture_dailytest(&mut s, &students(), &options, &Clock)[0].status,
        "refreshed"
    );
    assert_eq!(
        fs::read_to_string(partition(&options.spool)).unwrap(),
        expected()
    );
    // The syntax-tree reader of `StudentSource::daily` refuses the same page: the defect
    // this family does not reach.
    let student = SourceStudent {
        student_key: "77001".into(),
        student_name: "Invented Learner".into(),
    };
    assert!(crate::student_metadata::metadata_rows(&pages(true)["1"], &student).is_err());
}

#[test]
fn page_refusals_name_their_codes_and_publish_nothing() {
    let f = Fixture::new();
    let cases: [(&str, String, &str); 4] = [
        ("no_selector", "<script>const r={testResultList:JSON.parse('[]')};</script>".into(), "daily_history_selector_drift"),
        ("input_selector", "<input name=sort_date value=1><script>const r={testResultList:JSON.parse('[]')};</script>".into(), "daily_history_selector_drift"),
        ("current_not_offered", page("<option value=\"11\">Month</option>", &[], false), "daily_history_selector_drift"),
        ("two_lists", format!("{}<script>const q={{testResultList:JSON.parse('[]')}};</script>", page(OPTIONS, &[], false)), "result_list_missing_or_ambiguous"),
    ];
    for (label, html, code) in cases {
        let mut p = pages(false);
        p.insert("1".into(), html);
        let mut s = session(&f, &[Family::DailyTest], p, None, None, Rc::default());
        let options = f.options(4);
        let r = capture_dailytest(&mut s, &students(), &options, &Clock);
        assert!(
            r[0].status.starts_with("capture_daily_"),
            "{label}: {}",
            r[0].status
        );
        assert!(
            r[0].cause.contains(&code.to_string()),
            "{label}: {:?}",
            r[0].cause
        );
        assert!(!partition(&options.spool).exists(), "{label}");
    }
}

#[test]
fn prepare_owner_names_the_families_and_the_default_document_is_unchanged() {
    let f = Fixture::new();
    let put = |name: &str, value: &[u8]| {
        let path = f.0.join(name);
        fs::write(&path, value).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    };
    let identity = put("identity.json", br#"{"teacherSourceKey":"99001","teacherLabel":"Fictional Teacher","collegeSourceKey":"88001","sourceUsername":"OwnerUsername_CANARY"}"#);
    let secret = put("secret.env", b"SYNTHETIC_KEY=\"OwnerPassword_CANARY\"\n");
    let students_file = put("students.json", &serde_json::to_vec(&students()).unwrap());
    // The structural pin of the textbook mode, as the existing fixtures build it.
    let fields: Vec<Value> = ["clg_no","cls_no","p_pageno","check_da_test","check_fa_test","check_na_test","stu_name","chioce_cls","sort_date1","sort_date2"].iter().map(|name| serde_json::json!({"name":name,"tag":"input","type":"hidden","value":null,"checked":null,"options":[]})).collect();
    let form = put("form.json", &serde_json::to_vec(&serde_json::json!({"schemaVersion":"academy.paper-search-form/1","action":"/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&ass_no=1001","method":"post","fields":fields})).unwrap());
    let binary = academy_capture_binary();
    let prepare = |label: &str, families: Option<&str>| -> (Value, Vec<u8>) {
        let root = f.0.join(label);
        let mut command = std::process::Command::new(&binary);
        command
            .args(["prepare-owner", "--identity"])
            .arg(&identity)
            .arg("--secret-env")
            .arg(&secret)
            .args(["--secret-key", "SYNTHETIC_KEY", "--lock"])
            .arg(f.0.join("shared.lock"))
            .arg("--students")
            .arg(&students_file)
            .arg("--paper-form")
            .arg(&form)
            .args([
                "--allowed-from",
                "2040-01-01",
                "--allowed-through",
                "2040-01-31",
                "--expires-at",
                "2040-01-31T23:00:00Z",
                "--owner-root",
            ])
            .arg(&root);
        if let Some(families) = families {
            command.args(["--families", families]);
        }
        let out = command.output().unwrap();
        assert!(out.stderr.is_empty());
        let output: Value = serde_json::from_slice(&out.stdout).unwrap();
        if !out.status.success() {
            return (output, vec![]);
        }
        let bytes = Rc::new(OwnerRoot::open(&root).unwrap())
            .read(
                "transport-admission.json",
                output["documents"]["transport-admission.json"]["sha256"]
                    .as_str()
                    .unwrap(),
                1_048_576,
            )
            .unwrap()
            .bytes;
        (output, bytes)
    };
    // The document the mode `textbook` wrote before this unit: the textbook operations only.
    let base = serde_json::to_vec(
        &serde_json::to_value(&TransportAdmission {
            schema_version: "academy.transport-admission/1".into(),
            owner_id: "academy-capture".into(),
            teacher_id: "99001".into(),
            teacher_label: "Fictional Teacher".into(),
            college_id: "88001".into(),
            allowed_from: "2040-01-01".into(),
            allowed_through: "2040-01-31".into(),
            student_ids: vec!["77001".into()],
            source_namespaces: vec![COURSE_NS.into(), METADATA_NS.into()],
            operations: contracts(),
        })
        .unwrap(),
    )
    .unwrap();
    let ids = |bytes: &[u8]| -> Vec<String> {
        TransportAdmission::from_bytes(bytes)
            .unwrap()
            .operations
            .iter()
            .map(|o| o.id.clone())
            .collect()
    };
    let (_, default) = prepare("default", None);
    assert!(default == base, "default document differs");
    assert_eq!(
        ids(&default),
        [
            "student.books",
            "textbook.wrongnote",
            "textbook.detail",
            "metadata.paper.initial",
            "metadata.paper.search.7"
        ]
    );
    let (_, explicit) = prepare("explicit", Some("textbook"));
    assert!(explicit == base, "explicit textbook document differs");
    let (both_out, both) = prepare("both", Some("textbook,dailytest"));
    assert_eq!(both_out["operationCount"], 6);
    assert_eq!(
        ids(&both),
        [
            "student.books",
            "textbook.wrongnote",
            "textbook.detail",
            "metadata.daily",
            "metadata.paper.initial",
            "metadata.paper.search.7"
        ]
    );
    let (_, only) = prepare("only", Some("dailytest"));
    assert_eq!(ids(&only), ["metadata.daily"]);
    // Unknown, repeated and empty family lists refuse before any file exists.
    for bad in ["paper", "dailytest,dailytest", ""] {
        let (out, _) = prepare("bad", Some(bad));
        assert_eq!(out["status"], "prepare_input_invalid");
        assert!(!f.0.join("bad").exists());
    }
}

fn binary() -> PathBuf {
    academy_capture_binary()
}

#[test]
fn dailytest_binary_refuses_an_admission_without_the_family_before_any_login() {
    let f = Fixture::new();
    let a = admission(&[Family::Textbook]);
    let bytes = serde_json::to_vec(&a).unwrap();
    let roster = serde_json::json!({"students":students(),"paper_form":null});
    for (name, body) in [
        ("admission.json", bytes.clone()),
        ("roster.json", roster.to_string().into_bytes()),
    ] {
        fs::write(f.0.join(name), body).unwrap();
        fs::set_permissions(f.0.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    let out = std::process::Command::new(binary())
        .args(["dailytest", "--owner-root"])
        .arg(&f.0)
        .args([
            "--admission",
            "admission.json",
            "--admission-sha256",
            &crate::data::hash(&bytes),
            "--credential",
            "absent.json",
            "--credential-sha256",
            &"0".repeat(64),
            "--roster",
        ])
        .arg(f.0.join("roster.json"))
        .arg("--spool")
        .arg(f.0.join("spool"))
        .args(["--date", "2040-01-04", "--max-requests", "8"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stderr.is_empty());
    let response: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        response["status"], "capture_admission_refused",
        "{response}"
    );
    assert_eq!(response["students"][0]["requests"], 0);
    assert!(
        response["students"][0]["cause"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "student_read_contract_not_admitted")
    );
    assert!(!f.0.join("spool").exists());
    // `--no-detail` belongs to the textbook mode only.
    let out = std::process::Command::new(binary())
        .args(["dailytest", "--no-detail"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

fn replay(f: &Fixture, raw: &Path, spool: &Path, max: usize, extra: &[&str]) -> (Value, String) {
    fs::write(
        f.0.join("replay-students.json"),
        serde_json::to_vec(&students()).unwrap(),
    )
    .unwrap();
    fs::write(f.0.join("replay-identity.json"), br#"{"teacherSourceKey":"99001","teacherLabel":"Fictional Teacher","collegeSourceKey":"88001","sourceUsername":"Username_CANARY"}"#).unwrap();
    let out = std::process::Command::new(binary())
        .args(["replay", "--family", "dailytest", "--raw-dir"])
        .arg(raw)
        .arg("--spool")
        .arg(spool)
        .arg("--students")
        .arg(f.0.join("replay-students.json"))
        .arg("--identity")
        .arg(f.0.join("replay-identity.json"))
        .args(["--max-requests", &max.to_string()])
        .args(extra)
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    for payload in [&stdout, &stderr] {
        for canary in [
            "Username_CANARY",
            "Invented Learner",
            "Fictional Teacher",
            "77001",
            "99001",
            "88001",
        ] {
            assert!(!payload.contains(canary), "private output");
        }
    }
    (serde_json::from_str(&stdout).unwrap(), stderr)
}

#[test]
fn replay_binary_equals_the_capture_and_a_missing_answer_names_the_operation() {
    let f = Fixture::new();
    let raw = f.raw();
    let mut options = f.options(4);
    options.raw_dir = Some(raw.clone());
    {
        let mut s = session(
            &f,
            &[Family::DailyTest],
            pages(true),
            None,
            Some(raw.clone()),
            Rc::default(),
        );
        let r = capture_dailytest(&mut s, &students(), &options, &Clock);
        assert_eq!(r[0].status, "refreshed");
    }
    // The answers are on disk before any parser saw them, and nothing but the two bodies.
    let index = fs::read_to_string(raw.join("77001/index.jsonl")).unwrap();
    assert_eq!(index.lines().count(), 2);
    assert!(
        index
            .lines()
            .all(|l| serde_json::from_str::<Value>(l).unwrap()["operation"] == "metadata.daily")
    );
    let (out, stderr) = replay(&f, &raw, &f.0.join("replayed"), 4, &[]);
    assert!(stderr.is_empty());
    assert_eq!(out["students"][0]["status"], "refreshed", "{out}");
    assert_eq!(out["students"][0]["requests"], 2);
    assert!(out["students"][0].get("cause").is_none());
    // Replay's record time is the binary's clock: equal apart from `observed_at`.
    let normalize = |path: PathBuf| -> String {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|l| {
                let mut v: Value = serde_json::from_str(l).unwrap();
                v["observed_at"] = serde_json::json!(1);
                v.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        normalize(partition(&options.spool)),
        normalize(partition(&f.0.join("replayed")))
    );
    assert_eq!(
        normalize(partition(&f.0.join("replayed"))),
        expected()
            .trim_end()
            .lines()
            .collect::<Vec<_>>()
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    // No paper form, no detail flag for this family.
    let (refused, _) = replay(&f, &raw, &f.0.join("never"), 4, &["--paper-form", "x.json"]);
    assert_eq!(refused["status"], "replay_input_invalid");
    assert!(!f.0.join("never").exists());
    // A missing answer: drop the period `11` entry of the index.
    let kept: Vec<&str> = index.lines().take(1).collect();
    fs::write(raw.join("77001/index.jsonl"), kept.join("\n") + "\n").unwrap();
    let (out, _) = replay(&f, &raw, &f.0.join("missing"), 4, &[]);
    assert_eq!(out["students"][0]["status"], "replay_body_missing");
    assert_eq!(out["students"][0]["operation"], "metadata.daily");
    assert_eq!(out["students"][0]["requests"], 1);
    assert!(!partition(&f.0.join("missing")).exists());
}

/// The period `11` page is replaced by `html`: refused, old partition untouched.
fn later_page_refused(label: &str, html: impl Fn(&str, &str) -> String) {
    let f = Fixture::new();
    let options = f.options(4);
    {
        let mut s = session(
            &f,
            &[Family::DailyTest],
            pages(false),
            None,
            None,
            Rc::default(),
        );
        assert_eq!(
            capture_dailytest(&mut s, &students(), &options, &Clock)[0].status,
            "refreshed"
        );
    }
    let old = fs::read(partition(&options.spool)).unwrap();
    let literal = format!(
        "<script>const r = {{testResultList: JSON.parse('[{}]')}};</script>",
        period_eleven().join(",")
    );
    let mut p = pages(false);
    p.insert("11".into(), html(OPTIONS, &literal));
    let mut s = session(&f, &[Family::DailyTest], p, None, None, Rc::default());
    let r = capture_dailytest(&mut s, &students(), &options, &Clock);
    assert_eq!(
        r[0].status, "capture_daily_selector_failed",
        "{label}: {:?}",
        r[0].cause
    );
    assert!(
        r[0].cause
            .contains(&"daily_history_selector_drift".to_string()),
        "{label}: {:?}",
        r[0].cause
    );
    assert_eq!(r[0].requests, 2, "{label}");
    assert!(
        fs::read(partition(&options.spool)).unwrap() == old,
        "{label}: partition changed"
    );
}
#[test]
fn later_page_without_sort_date_control_is_refused() {
    later_page_refused("missing", |_, literal| literal.to_string());
}
#[test]
fn later_page_with_duplicated_sort_date_control_is_refused() {
    later_page_refused("duplicated", |options, literal| {
        format!(
            "<form><select name=\"sort_date\">{options}</select></form><select name=\"sort_date\">{options}</select>{literal}"
        )
    });
}
#[test]
fn later_page_with_form_owned_sort_date_control_is_refused() {
    later_page_refused("form_owned", |options, literal| {
        format!(
            "<form id=\"other\"></form><select name=\"sort_date\" form=\"other\">{options}</select>{literal}"
        )
    });
}
#[test]
fn later_page_without_the_requested_selector_among_enabled_options_is_refused() {
    later_page_refused("requested_absent", |_, literal| {
        format!(
            "<select name=\"sort_date\"><option value=\"1\">Recent week</option><option value=\"12\">Previous month</option><option value=\"11\" disabled>Recent month</option></select>{literal}"
        )
    });
}

/// The `academy-capture` binary of this build. A unit test gets no `CARGO_BIN_EXE_<name>` (Cargo sets
/// it for integration tests only), and its own path differs by layout (`<profile>/deps/` before Cargo
/// 1.100, `<profile>/build/<pkg>/<hash>/out/` with build-dir layout v2), so look in its ancestors for
/// the final artifact, which stays in `<target-dir>/<profile>/`.
fn academy_capture_binary() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.ancestors()
        .map(|d| d.join("academy-capture"))
        .find(|p| p.is_file())
        .unwrap()
}
