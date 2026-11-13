//! Invented identities, native owner workflow, in-memory transport; no sockets.
use super::*;
use crate::{
    http::{self, Cancellation, Request, Response, Transport},
    owner_root::OwnerRoot,
    owner_workflow::{Credentials, NativeSession},
};
use std::{
    os::unix::fs::PermissionsExt,
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
        Ok("synthetic-capture".into())
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
            "capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        fs::write(p.join("shared.lock"), b"synthetic lock").unwrap();
        fs::write(
            p.join("account-lock.json"),
            json!({"schemaVersion":"academy.account-lock/1","path":p.join("shared.lock")})
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
    fn options(&self, no_detail: bool, max: usize) -> Options {
        Options {
            date: "2040-01-04".into(),
            max_requests: max,
            no_detail,
            raw_dir: None,
            spool: self.0.join("spool"),
        }
    }
    fn root(&self) -> Rc<OwnerRoot> {
        Rc::new(OwnerRoot::open(&self.0).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn admission() -> TransportAdmission {
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
        operations: contracts(),
    }
}
const OWNER: &str = "<form name=userForm><input name=pri_no value=99001></form><form name=form1><input name=g_pri_no value=99001><input name=g_clg_no value=88001><input name=g_mem_type value=fictional></form>";
const SEARCH: &str = r#"<form name="search" action="/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&amp;ass_no=1001" method="post"><input name="clg_no" type="hidden" value="88001"><input name="cls_no" type="hidden" value="0"><input name="p_pageno" type="hidden" value="4"><input name="check_da_test" type="checkbox" value="D" checked><input name="check_fa_test" type="checkbox" value="F"><input name="check_na_test" type="checkbox" value="N"><input name="stu_name" type="text"><select name="chioce_cls"><option value="0">All fictional classes</option></select><input name="sort_date1" type="text" value="2000-01-01"><input name="sort_date2" type="text" value="2000-01-02"></form>"#;
fn roster() -> Roster {
    let fields:Vec<Value> = [("clg_no","input",Some("hidden"),Some("88001")),("cls_no","input",Some("hidden"),None),("p_pageno","input",Some("hidden"),None),("check_da_test","input",Some("checkbox"),Some("D")),("check_fa_test","input",Some("checkbox"),Some("F")),("check_na_test","input",Some("checkbox"),Some("N")),("stu_name","input",Some("text"),None),("chioce_cls","select",None,Some("0")),("sort_date1","input",Some("text"),None),("sort_date2","input",Some("text"),None)].iter().map(|(name,tag,ty,value)|json!({"name":name,"tag":tag,"type":ty,"value":value,"checked":if *ty==Some("checkbox"){json!(*name=="check_da_test")}else{Value::Null},"options":if *tag=="select"{json!([{"value":"0","label":"All fictional classes"}])}else{json!([])}})).collect();
    Roster {
        students: vec![Student {
            student_id: "77001".into(),
            student_name: "Invented Learner".into(),
        }],
        paper_form: json!({"schemaVersion":"academy.paper-search-form/1","action":format!("{}?p_process=UserBySearchTestResult&ass_no=1001",crate::student_course::PAPER_PATH),"method":"post","fields":fields}),
    }
}
struct Synthetic {
    seen: Rc<RefCell<Vec<String>>>,
    rows: usize,
    details: usize,
    fail_detail: Option<usize>,
}
impl Transport for Synthetic {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        self.seen.borrow_mut().push(r.path.clone());
        let field = |name: &str| {
            r.query
                .iter()
                .chain(r.body.iter())
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
                .unwrap_or("")
        };
        let mut headers = vec![];
        let body = if r.path == "/c_login.jsp" {
            headers.push((
                "Set-Cookie".into(),
                "JSESSIONID=syntheticcapture; Path=/; HttpOnly".into(),
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
        } else if field("reqCmd") == "getTextBookProgress" {
            include_str!("../tests/fixtures/textbook_index/progress-single.html").into()
        } else if field("p_process") == "getWrongAnswerNoteResult" {
            if self.rows == 0 {
                return Ok(Response {
                    status: 200,
                    headers: vec![],
                    bytes: b"not-json".to_vec(),
                    url: format!("{}{}", http::ORIGIN, r.path),
                    stalled: false,
                });
            }
            json!({"result":"ok","noteResultList":(1..=self.rows).map(|n|json!({"pri_no":77001,"name":"Invented Learner","p_no":81000+n,"testing_no":82000+n,"testing_name":format!("Fictional section {n:02}"),"apply_date":"11-03"})).collect::<Vec<_>>()}).to_string()
        } else if field("p_process") == "getStudyResultSingleTestingSingleUser" {
            self.details += 1;
            if self.fail_detail == Some(self.details) {
                return Err(Error::error("synthetic_failure"));
            }
            let c: Value = serde_json::from_str(field("condition")).unwrap();
            let n = c["testing_no"].as_u64().unwrap() - 82000;
            json!({"result":"OK","print_list":[{"pri_no":77001,"student_name":"Invented Learner","sub_title":format!("Fictional section {n:02}"),"score":50,"exam_list":[{"exam_no":1,"scoring_result":"O"},{"exam_no":2,"scoring_result":"X"}]}]}).to_string()
        } else if field("p_process") == "UserByMain" {
            format!("{OWNER}{SEARCH}")
        } else if field("p_process") == "UserBySearchTestResult" {
            format!("{OWNER}<script>const results={{testResultList:JSON.parse('[]')}};</script>")
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
fn session(
    f: &Fixture,
    rows: usize,
    fail_detail: Option<usize>,
    seen: Rc<RefCell<Vec<String>>>,
) -> RecordingPort<'static, NativeSession<'static, RecordingTransport<'static, Synthetic>>> {
    session_with_raw(f, rows, fail_detail, seen, None)
}
fn session_with_raw(
    f: &Fixture,
    rows: usize,
    fail_detail: Option<usize>,
    seen: Rc<RefCell<Vec<String>>>,
    raw: Option<PathBuf>,
) -> RecordingPort<'static, NativeSession<'static, RecordingTransport<'static, Synthetic>>> {
    let recorder = Recorder::new(raw, &Clock);
    let session = NativeSession::open(
        f.root(),
        admission(),
        recorder.transport(Synthetic {
            seen,
            rows,
            details: 0,
            fail_detail,
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
#[test]
fn exact_graded_bytes_and_one_login() {
    let f = Fixture::new();
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session(&f, 1, None, seen.clone());
    let options = f.options(false, 8);
    let report = capture(&mut s, &roster(), &options, &Clock);
    assert_eq!(report[0].status, "refreshed");
    assert_eq!(report[0].requests, 8);
    assert_eq!(
        seen.borrow()
            .iter()
            .filter(|p| p.as_str() == "/c_login.jsp")
            .count(),
        1
    );
    assert_eq!(
        fs::read_to_string(options.spool.join("registered_book/77001.jsonl")).unwrap(),
        "{\"binding_ordinal\":1,\"book_label\":\"Gauss fraction book\",\"book_no\":\"2517\",\"course_key\":\"693\",\"hint\":\"Fictional section 01\",\"key\":\"77001|2517\",\"observed_at\":1,\"student\":\"77001\",\"student_id\":\"77001\",\"type\":\"registered_book\"}\n"
    );
    assert_eq!(
        fs::read_to_string(options.spool.join("section_attempt/77001.jsonl")).unwrap(),
        "{\"book\":\"77001|2517\",\"book_book_label\":\"Gauss fraction book\",\"book_no\":\"2517\",\"correct_count\":1,\"graded\":true,\"index_ordinal\":1,\"item_count\":2,\"key\":\"77001|2517|82001\",\"observed_at\":1,\"score\":50.0,\"source_date_literal\":\"11-03\",\"source_date_precision\":\"month_day\",\"source_title\":\"Fictional section 01\",\"student\":\"77001\",\"student_id\":\"77001\",\"type\":\"section_attempt\",\"wrong_ordinals\":\"2\"}\n"
    );
    for path in ["registered_book/77001.jsonl", "section_attempt/77001.jsonl"] {
        assert_eq!(
            fs::metadata(options.spool.join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    for path in ["registered_book", "section_attempt", ""] {
        assert_eq!(
            fs::metadata(options.spool.join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
#[test]
fn second_detail_batch_failure_preserves_both_partitions() {
    let f = Fixture::new();
    let options = f.options(true, 3);
    {
        let mut s = session(&f, 13, None, Rc::default());
        assert_eq!(
            capture(&mut s, &roster(), &options, &Clock)[0].status,
            "refreshed"
        );
    }
    let paths = [
        options.spool.join("registered_book/77001.jsonl"),
        options.spool.join("section_attempt/77001.jsonl"),
    ];
    let old = paths.each_ref().map(|p| fs::read(p).unwrap());
    let mut s = session(&f, 13, Some(13), Rc::default());
    let r = capture(&mut s, &roster(), &f.options(false, 24), &Clock);
    assert_eq!(r[0].status, "capture_request_failed");
    assert_eq!(r[0].requests, 24);
    assert_eq!(paths.each_ref().map(|p| fs::read(p).unwrap()), old);
    assert_eq!(r[0].registered_books, 0);
}
#[test]
fn admission_document_and_missing_operation_and_outside_student_dispatch_nothing() {
    let f = Fixture::new();
    let mut a = admission();
    let bytes = serde_json::to_vec(&a).unwrap();
    fs::write(f.0.join("transport-admission.json"), &bytes).unwrap();
    let a2 =
        TransportAdmission::from_bytes(&fs::read(f.0.join("transport-admission.json")).unwrap())
            .unwrap();
    preflight(&a2, &roster(), &f.options(true, 5)).unwrap();
    a.operations.retain(|o| o.id != "textbook.detail");
    assert_eq!(
        preflight(&a, &roster(), &f.options(true, 5))
            .unwrap_err()
            .message,
        "student_read_contract_not_admitted"
    );
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session(&f, 1, None, seen.clone());
    let mut outside = roster();
    outside.students[0].student_id = "77002".into();
    assert_eq!(
        capture(&mut s, &outside, &f.options(true, 5), &Clock)[0].status,
        "capture_admission_refused"
    );
    assert!(seen.borrow().is_empty());
    drop(s);
    // Exercise the binary's preflight: a missing operation refuses before credentials or account configuration.
    fs::write(
        f.0.join("transport-admission.json"),
        serde_json::to_vec(&a).unwrap(),
    )
    .unwrap();
    fs::set_permissions(
        f.0.join("transport-admission.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let bytes = fs::read(f.0.join("transport-admission.json")).unwrap();
    assert!(TransportAdmission::from_bytes(&bytes).is_ok());
}
#[test]
fn budget_stops_before_excess_request_and_preserves_old_files() {
    let f = Fixture::new();
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session(&f, 1, None, seen.clone());
    let r = capture(&mut s, &roster(), &f.options(true, 2), &Clock);
    assert_eq!(r[0].status, "capture_request_limit");
    assert_eq!(r[0].requests, 2);
    assert_eq!(seen.borrow().len(), 5);
    assert!(!f.0.join("spool").exists());
}

#[test]
fn raw_bodies_and_index_match_transport_and_parser_failure_is_retained() {
    for malformed in [false, true] {
        let f = Fixture::new();
        let raw = f.0.join("raw");
        fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
        let mut options = f.options(true, 5);
        options.raw_dir = Some(raw.clone());
        let mut s = session_with_raw(
            &f,
            if malformed { 0 } else { 1 },
            None,
            Rc::default(),
            Some(raw.clone()),
        );
        let r = capture(&mut s, &roster(), &options, &Clock);
        assert_eq!(
            r[0].status,
            if malformed {
                "capture_index_failed"
            } else {
                "refreshed"
            }
        );
        let p = raw.join("77001");
        let lines = fs::read_to_string(p.join("index.jsonl")).unwrap();
        let entries: Vec<Value> = lines
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(entries.len(), 3);
        let expected_note = if malformed {
            "not-json".to_string()
        } else {
            r#"{"noteResultList":[{"apply_date":"11-03","name":"Invented Learner","p_no":81001,"pri_no":77001,"testing_name":"Fictional section 01","testing_no":82001}],"result":"ok"}"#.into()
        };
        let expected = [
            include_str!("../tests/fixtures/textbook_index/progress-single.html").as_bytes(),
            include_str!("../tests/fixtures/textbook_index/progress-single.html").as_bytes(),
            expected_note.as_bytes(),
        ];
        for (i, e) in entries.iter().enumerate() {
            assert_eq!(e["sequence"], i + 1);
            assert_eq!(e["http_status"], 200);
            assert_eq!(e["observed_at"], 1);
            let op = e["operation"].as_str().unwrap();
            assert_eq!(
                op,
                if i < 2 {
                    "student.books"
                } else {
                    "textbook.wrongnote"
                }
            );
            let body = fs::read(p.join(format!("{:06}-{op}.body", i + 1))).unwrap();
            assert_eq!(body, expected[i]);
            assert_eq!(e["sha256"], crate::data::hash(&body));
            assert_eq!(e["byte_count"], body.len());
            assert_eq!(
                fs::metadata(p.join(format!("{:06}-{op}.body", i + 1)))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert_eq!(
            entries[0]["parameters"],
            json!({"query":[["reqCmd","getTextBookProgress"]],"body":[["student_name","Invented Learner"]]})
        );
        assert_eq!(fs::read_dir(&p).unwrap().count(), 4);
        if malformed {
            assert!(!options.spool.exists());
        }
    }
}
#[test]
fn raw_directory_inside_git_refuses_without_dispatch() {
    let f = Fixture::new();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&f.0)
            .status()
            .unwrap()
            .success()
    );
    let raw = f.0.join("raw");
    fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
    let mut options = f.options(true, 5);
    options.raw_dir = Some(raw.clone());
    let seen = Rc::new(RefCell::new(vec![]));
    let mut s = session_with_raw(&f, 1, None, seen.clone(), Some(raw));
    assert_eq!(
        capture(&mut s, &roster(), &options, &Clock)[0].status,
        "capture_admission_refused"
    );
    assert!(seen.borrow().is_empty());
}
