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
    drop_paper_row: bool,
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
            if self.drop_paper_row {
                return Ok(Response {
                    status: 200,
                    headers: vec![],
                    bytes: include_bytes!("../tests/fixtures/textbook_index/wrong-notes.json")
                        .to_vec(),
                    url: format!("{}{}", http::ORIGIN, r.path),
                    stalled: false,
                });
            }
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
            if self.drop_paper_row {
                let raw=json!([{"priNo":"77001","userName":"Invented Learner","pNo":"81099","testingNo":"82099","testingName":42}]).to_string();
                return Ok(Response {status:200,headers:vec![],bytes:format!("{OWNER}<script>const results={{testResultList:JSON.parse('{raw}')}};</script>").into_bytes(),url:format!("{}{}",http::ORIGIN,r.path),stalled:false});
            }
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
            drop_paper_row: false,
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

#[test]
fn prepare_owner_binary_documents_feed_native_capture_and_partial_write_rolls_back() {
    let f = Fixture::new();
    let put = |name: &str, value: &[u8]| {
        let path = f.0.join(name);
        fs::write(&path, value).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    };
    let identity=put("identity.json",br#"{"teacherSourceKey":"99001","teacherLabel":"Fictional Teacher","collegeSourceKey":"88001","sourceUsername":"OwnerUsername_CANARY"}"#);
    let secret = put("secret.env", b"SYNTHETIC_KEY=\"OwnerPassword_CANARY\"\n");
    let students = put(
        "students.json",
        &serde_json::to_vec(&roster().students).unwrap(),
    );
    let form = put(
        "form.json",
        &serde_json::to_vec(&roster().paper_form).unwrap(),
    );
    let output_root = f.0.join("prepared");
    let binary = academy_capture_binary();
    let out = std::process::Command::new(binary)
        .args(["prepare-owner", "--identity"])
        .arg(&identity)
        .arg("--secret-env")
        .arg(&secret)
        .args(["--secret-key", "SYNTHETIC_KEY", "--lock"])
        .arg(f.0.join("shared.lock"))
        .arg("--students")
        .arg(&students)
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
        .arg(&output_root)
        .output()
        .unwrap();
    for payload in [&out.stdout, &out.stderr] {
        for value in [
            "OwnerUsername_CANARY",
            "OwnerPassword_CANARY",
            "Invented Learner",
            "Fictional Teacher",
            "77001",
            "99001",
            "88001",
        ] {
            assert!(
                !String::from_utf8_lossy(payload).contains(value),
                "CLI privacy violation"
            );
        }
    }
    assert!(out.status.success());
    let output: Value = serde_json::from_slice(&out.stdout).unwrap();
    let root = Rc::new(OwnerRoot::open(&output_root).unwrap());
    let read = |name: &str| {
        root.read(
            name,
            output["documents"][name]["sha256"].as_str().unwrap(),
            1_048_576,
        )
        .unwrap()
        .bytes
    };
    let admission = TransportAdmission::from_bytes(&read("transport-admission.json")).unwrap();
    assert_eq!(
        serde_json::to_value(&admission.operations).unwrap(),
        serde_json::to_value(contracts()).unwrap()
    );
    let roster: Roster = serde_json::from_slice(&read("roster.json")).unwrap();
    let provider = crate::owner_workflow::FileCredentialProvider::new(
        root.clone(),
        crate::owner_workflow::PinnedInput {
            relative_path: "credential.json".into(),
            sha256: output["documents"]["credential.json"]["sha256"]
                .as_str()
                .unwrap()
                .into(),
        },
        &Clock,
    );
    let seen = Rc::new(RefCell::new(vec![]));
    let recorder = Recorder::new(None, &Clock);
    let session = NativeSession::open(
        root,
        admission,
        recorder.transport(Synthetic {
            seen: seen.clone(),
            rows: 1,
            details: 0,
            fail_detail: None,
            drop_paper_row: false,
        }),
        provider,
        Cancellation::default(),
        &Clock,
    )
    .unwrap();
    let mut session = recorder.port(session);
    assert_eq!(
        capture(&mut session, &roster, &f.options(true, 3), &Clock)[0].status,
        "refreshed"
    );
    assert_eq!(
        seen.borrow()
            .iter()
            .filter(|s| s.as_str() == "/c_login.jsp")
            .count(),
        1
    );
    // Actual publication failure after two files: cleanup must remove both.
    let inputs = crate::prepare_owner::Inputs {
        identity,
        secret_env: secret,
        secret_key: "SYNTHETIC_KEY".into(),
        lock: f.0.join("shared.lock"),
        students,
        paper_form: form,
        allowed_from: "2040-01-01".into(),
        allowed_through: "2040-01-31".into(),
        expires_at: "2040-01-31T23:00:00Z".into(),
        owner_root: f.0.join("rollback"),
    };
    assert_eq!(
        crate::prepare_owner::tests::fail_second_write(&inputs, &Clock)
            .unwrap_err()
            .message,
        "prepare_write_failed"
    );
    assert!(!inputs.owner_root.exists());
}
#[test]
fn dropped_paper_row_refuses_student_and_preserves_both_old_partitions() {
    let f = Fixture::new();
    let options = f.options(true, 5);
    {
        let mut p = session(&f, 1, None, Rc::default());
        assert_eq!(
            capture(&mut p, &roster(), &options, &Clock)[0].status,
            "refreshed"
        );
    }
    let paths = [
        options.spool.join("registered_book/77001.jsonl"),
        options.spool.join("section_attempt/77001.jsonl"),
    ];
    let old = paths.each_ref().map(|p| fs::read(p).unwrap());
    let recorder = Recorder::new(None, &Clock);
    let s = NativeSession::open(
        f.root(),
        admission(),
        recorder.transport(Synthetic {
            seen: Rc::default(),
            rows: 1,
            details: 0,
            fail_detail: None,
            drop_paper_row: true,
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
    let mut port = recorder.port(s);
    let report = capture(&mut port, &roster(), &options, &Clock);
    assert_eq!(report[0].status, "capture_paper_row_dropped");
    assert_eq!(paths.each_ref().map(|p| fs::read(p).unwrap()), old);
}
struct Alter<P>(P);
impl<P: ReadPort> ReadPort for Alter<P> {
    fn transport_admission(&self) -> &TransportAdmission {
        self.0.transport_admission()
    }
    fn authenticate_for_reads(&mut self) -> Result<()> {
        self.0.authenticate_for_reads()
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        args: &[(String, String)],
    ) -> Result<Response> {
        let mut changed = args.to_vec();
        if let Some((_, value)) = changed.iter_mut().find(|(k, _)| k == "student_name") {
            value.push(' ');
        }
        self.0.read(scope, operation, &changed)
    }
}
#[test]
fn raw_request_mismatch_refuses_student_and_preserves_old_partitions() {
    let f = Fixture::new();
    let mut options = f.options(true, 5);
    {
        let mut p = session(&f, 1, None, Rc::default());
        assert_eq!(
            capture(&mut p, &roster(), &options, &Clock)[0].status,
            "refreshed"
        );
    }
    let paths = [
        options.spool.join("registered_book/77001.jsonl"),
        options.spool.join("section_attempt/77001.jsonl"),
    ];
    let old = paths.each_ref().map(|p| fs::read(p).unwrap());
    let raw = f.0.join("raw");
    fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
    options.raw_dir = Some(raw.clone());
    let recorder = Recorder::new(Some(raw.clone()), &Clock);
    let seen = Rc::new(RefCell::new(vec![]));
    let s = NativeSession::open(
        f.root(),
        admission(),
        recorder.transport(Synthetic {
            seen: seen.clone(),
            rows: 1,
            details: 0,
            fail_detail: None,
            drop_paper_row: false,
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
    let mut port = recorder.port(Alter(s));
    let report = capture(&mut port, &roster(), &options, &Clock);
    assert_eq!(report[0].status, "capture_raw_request_mismatch");
    assert_eq!(paths.each_ref().map(|p| fs::read(p).unwrap()), old);
    assert_eq!(fs::read_dir(raw).unwrap().count(), 0);
    assert_eq!(seen.borrow().len(), 3);
}

struct ReplaySynthetic(Synthetic);
impl Transport for ReplaySynthetic {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        let mut response = self.0.dispatch(r)?;
        if r.query
            .iter()
            .chain(&r.body)
            .any(|(k, v)| k == "p_process" && v == "UserBySearchTestResult")
        {
            response.bytes=format!("{OWNER}<script>{} let re=/a+/; let t=`x`; let q=4/2;</script><script>testResultList:JSON.parse('[]')</script>","a;".repeat(10_000)).into_bytes();
        }
        Ok(response)
    }
}
fn replay_cli(f: &Fixture, raw: &Path, spool: &Path, max: usize) -> (Value, String) {
    replay_cli_mode(f, raw, spool, max, true)
}
fn replay_cli_mode(
    f: &Fixture,
    raw: &Path,
    spool: &Path,
    max: usize,
    no_detail: bool,
) -> (Value, String) {
    fs::write(
        f.0.join("replay-students.json"),
        serde_json::to_vec(&roster().students).unwrap(),
    )
    .unwrap();
    fs::write(
        f.0.join("replay-form.json"),
        serde_json::to_vec(&roster().paper_form).unwrap(),
    )
    .unwrap();
    fs::write(f.0.join("replay-identity.json"),br#"{"teacherSourceKey":"99001","teacherLabel":"Fictional Teacher","collegeSourceKey":"88001","sourceUsername":"Username_CANARY"}"#).unwrap();
    let binary = academy_capture_binary();
    let mut command = std::process::Command::new(binary);
    command
        .arg("replay")
        .arg("--raw-dir")
        .arg(raw)
        .arg("--spool")
        .arg(spool)
        .arg("--students")
        .arg(f.0.join("replay-students.json"))
        .arg("--identity")
        .arg(f.0.join("replay-identity.json"))
        .arg("--paper-form")
        .arg(f.0.join("replay-form.json"))
        .args(["--max-requests", &max.to_string()]);
    if no_detail {
        command.arg("--no-detail");
    }
    let out = command.output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    for payload in [&stdout, &stderr] {
        for canary in [
            "Username_CANARY",
            "Invented Learner",
            "Fictional Teacher",
            "Payload_CANARY",
            "77001",
            "99001",
            "88001",
        ] {
            assert!(!payload.contains(canary), "private output");
        }
    }
    (serde_json::from_str(&stdout).unwrap(), stderr)
}
fn recorded_replay_fixture(f: &Fixture) -> PathBuf {
    let raw = f.0.join("raw");
    fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
    let recorder = Recorder::new(Some(raw.clone()), &Clock);
    let native = NativeSession::open(
        f.root(),
        admission(),
        recorder.transport(ReplaySynthetic(Synthetic {
            seen: Rc::default(),
            rows: 1,
            details: 0,
            fail_detail: None,
            drop_paper_row: true,
        })),
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
    let mut port = recorder.port(native);
    let mut options = f.options(true, 5);
    options.raw_dir = Some(raw.clone());
    let reports = capture(&mut port, &roster(), &options, &Clock);
    assert_eq!(reports[0].status, "refreshed");
    assert_eq!(reports[0].requests, 5);
    raw
}
fn rewrite_saved(raw: &Path, operation: &str, bytes: &[u8]) {
    let dir = raw.join("77001");
    let entries = fs::read_to_string(dir.join("index.jsonl")).unwrap();
    let mut changed = vec![];
    for line in entries.lines() {
        let mut e: Value = serde_json::from_str(line).unwrap();
        if e["operation"] == operation {
            fs::write(
                dir.join(format!(
                    "{:06}-{operation}.body",
                    e["sequence"].as_u64().unwrap()
                )),
                bytes,
            )
            .unwrap();
            e["byte_count"] = json!(bytes.len());
            e["sha256"] = json!(crate::data::hash(bytes));
        }
        changed.push(e.to_string());
    }
    fs::write(dir.join("index.jsonl"), changed.join("\n") + "\n").unwrap();
}
#[test]
fn replay_binary_matches_capture_and_reports_safe_causes_and_missing_operation() {
    let f = Fixture::new();
    let raw = recorded_replay_fixture(&f);
    let (out, stderr) = replay_cli(&f, &raw, &f.0.join("replay"), 13);
    assert!(stderr.is_empty());
    assert_eq!(out["students"][0]["status"], "refreshed");
    assert!(out["students"][0].get("cause").is_none());
    for partition in ["registered_book/77001.jsonl", "section_attempt/77001.jsonl"] {
        let normalize = |path: PathBuf| {
            fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|line| {
                    let mut v: Value = serde_json::from_str(line).unwrap();
                    v["observed_at"] = json!(1);
                    v.to_string()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(
            normalize(f.0.join("spool").join(partition)),
            normalize(f.0.join("replay").join(partition))
        );
    }
    let existing = replay_cli(&f, &raw, &f.0.join("replay"), 13).0;
    assert_eq!(existing["status"], "replay_spool_refused");
    // No saved paper-search result: identify operation, count only answered reads.
    let index = raw.join("77001/index.jsonl");
    let original = fs::read(&index).unwrap();
    let lines = String::from_utf8(original.clone()).unwrap();
    fs::write(
        &index,
        lines.lines().take(4).collect::<Vec<_>>().join("\n") + "\n",
    )
    .unwrap();
    let missing = replay_cli(&f, &raw, &f.0.join("missing"), 13).0;
    assert_eq!(missing["students"][0]["status"], "replay_body_missing");
    assert_eq!(
        missing["students"][0]["operation"],
        "metadata.paper.search.7"
    );
    assert_eq!(missing["students"][0]["requests"], 4);
    fs::write(&index, original).unwrap();
    // Every distinct reachable boundary keeps its inner refusal through the binary.
    for (n, operation, body, status, cause) in [
        (
            0,
            "student.books",
            "Payload_CANARY",
            "capture_bindings_failed",
            "textbook_binding_source_unqualified",
        ),
        (
            1,
            "textbook.wrongnote",
            "Payload_CANARY",
            "capture_index_failed",
            "source_json_rejected",
        ),
        (
            2,
            "metadata.paper.initial",
            "Payload_CANARY",
            "capture_paper_search_failed",
            "contract_drift",
        ),
        (
            3,
            "metadata.paper.search.7",
            "Payload_CANARY",
            "capture_paper_search_failed",
            "student_read_source_identity_conflict",
        ),
    ] {
        let original_index = fs::read(&index).unwrap();
        let entries: Vec<Value> = lines
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let saved: Vec<_> = entries
            .iter()
            .filter(|e| e["operation"] == operation)
            .map(|e| {
                let p = raw.join("77001").join(format!(
                    "{:06}-{operation}.body",
                    e["sequence"].as_u64().unwrap()
                ));
                let b = fs::read(&p).unwrap();
                (p, b)
            })
            .collect();
        rewrite_saved(&raw, operation, body.as_bytes());
        let out = replay_cli(&f, &raw, &f.0.join(format!("failure-{n}")), 13).0;
        let r = &out["students"][0];
        assert_eq!(r["status"], status, "{out}");
        assert!(
            r["cause"].as_array().is_some_and(|a| !a.is_empty()),
            "{out}"
        );
        // Exact codes are asserted below for paper's swallowed hook; the others
        // retain their parser's canonical code, independent of input text.
        if n <= 3 {
            assert!(
                r["cause"].as_array().unwrap().contains(&json!(cause)),
                "{out}"
            );
        }
        for (p, b) in saved {
            fs::write(p, b).unwrap();
        }
        fs::write(&index, original_index).unwrap();
    }
    let out = replay_cli(&f, &raw, &f.0.join("budget"), 2).0;
    assert_eq!(out["students"][0]["status"], "capture_request_limit");
    assert!(
        out["students"][0]["cause"]
            .as_array()
            .unwrap()
            .contains(&json!("capture_request_limit"))
    );
}
#[test]
fn replay_binary_refuses_missing_index_body_and_digest() {
    let f = Fixture::new();
    let raw = recorded_replay_fixture(&f);
    let body = raw.join("77001/000002-student.books.body");
    let original = fs::read(&body).unwrap();
    fs::write(&body, b"Payload_CANARY").unwrap();
    let out = replay_cli(&f, &raw, &f.0.join("digest"), 13).0;
    assert_eq!(out["students"][0]["status"], "replay_body_digest_mismatch");
    fs::remove_file(&body).unwrap();
    let out = replay_cli(&f, &raw, &f.0.join("absent"), 13).0;
    assert_eq!(out["students"][0]["status"], "replay_body_refused");
    fs::write(body, original).unwrap();
    fs::remove_file(raw.join("77001/index.jsonl")).unwrap();
    let out = replay_cli(&f, &raw, &f.0.join("no-index"), 13).0;
    assert_eq!(out["status"], "replay_index_refused");
}

#[test]
fn replay_binary_graded_capture_and_detail_and_swallowed_grade_causes() {
    let f = Fixture::new();
    let raw = f.0.join("raw");
    fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
    let mut options = f.options(false, 8);
    options.raw_dir = Some(raw.clone());
    let mut port = session_with_raw(&f, 1, None, Rc::default(), Some(raw.clone()));
    assert_eq!(
        capture(&mut port, &roster(), &options, &Clock)[0].status,
        "refreshed"
    );
    drop(port);
    let out = replay_cli_mode(&f, &raw, &f.0.join("graded"), 8, false).0;
    assert_eq!(out["students"][0]["status"], "refreshed");
    assert_eq!(out["students"][0]["requests"], 8);
    for partition in ["registered_book/77001.jsonl", "section_attempt/77001.jsonl"] {
        let normalize = |path: PathBuf| {
            fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|line| {
                    let mut v: Value = serde_json::from_str(line).unwrap();
                    v["observed_at"] = json!(1);
                    v.to_string()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(
            normalize(f.0.join("spool").join(partition)),
            normalize(f.0.join("graded").join(partition))
        );
    }
    let detail = fs::read(raw.join("77001/000008-textbook.detail.body")).unwrap();
    rewrite_saved(&raw, "textbook.detail", b"Payload_CANARY");
    let out = replay_cli_mode(&f, &raw, &f.0.join("detail-failed"), 8, false).0;
    assert_eq!(out["students"][0]["status"], "capture_grade_failed");
    assert_eq!(
        out["students"][0]["cause"],
        json!(["source_json_rejected", "capture_grade_failed"])
    );
    rewrite_saved(&raw, "textbook.detail", &detail);
    rewrite_saved(&raw, "metadata.paper.initial", b"Payload_CANARY");
    let out = replay_cli_mode(&f, &raw, &f.0.join("grade-incomplete"), 8, false).0;
    assert_eq!(out["students"][0]["status"], "capture_grade_incomplete");
    assert_eq!(
        out["students"][0]["cause"],
        json!(["contract_drift", "capture_grade_incomplete"])
    );
}
#[test]
fn replay_binary_duplicate_book_attempt_and_dropped_row_causes() {
    for case in 0..3 {
        let f = Fixture::new();
        let raw = recorded_replay_fixture(&f);
        let expected = match case {
            0 => {
                let page = include_str!("../tests/fixtures/textbook_index/progress-single.html");
                rewrite_saved(
                    &raw,
                    "student.books",
                    format!("{page}{}", page.replace("693,2517", "694,2517")).as_bytes(),
                );
                "capture_duplicate_book"
            }
            1 => {
                let mut notes: Value = serde_json::from_slice(include_bytes!(
                    "../tests/fixtures/textbook_index/wrong-notes.json"
                ))
                .unwrap();
                let mut other = notes["noteResultList"][0].clone();
                other["p_no"] = json!(81004);
                notes["noteResultList"].as_array_mut().unwrap().push(other);
                rewrite_saved(&raw, "textbook.wrongnote", notes.to_string().as_bytes());
                "capture_duplicate_attempt"
            }
            _ => {
                let rows = json!([{"priNo":77001,"userName":"Invented Learner","pNo":81004,"testingNo":82004,"testingName":42}]);
                rewrite_saved(
                    &raw,
                    "metadata.paper.search.7",
                    format!("{OWNER}<script>testResultList:JSON.parse('{rows}')</script>")
                        .as_bytes(),
                );
                "capture_paper_row_dropped"
            }
        };
        let out = replay_cli(&f, &raw, &f.0.join("failure"), 13).0;
        assert_eq!(out["students"][0]["status"], expected, "{out}");
        assert_eq!(out["students"][0]["cause"], json!([expected]), "{out}");
    }
}
#[test]
fn replay_binary_latest_exact_parameters_status_and_raw_write_protection() {
    let f = Fixture::new();
    let raw = recorded_replay_fixture(&f);
    let dir = raw.join("77001");
    let index = dir.join("index.jsonl");
    let original = fs::read_to_string(&index).unwrap();
    let mut entries: Vec<Value> = original
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    // Later unrelated parameters must not override a matching saved answer.
    let mut extra = entries[2].clone();
    extra["sequence"] = json!(6);
    extra["parameters"]["body"] = json!([["p_process", "Payload_CANARY"]]);
    extra["http_status"] = json!(500);
    let body = fs::read(dir.join("000003-textbook.wrongnote.body")).unwrap();
    fs::write(dir.join("000006-textbook.wrongnote.body"), &body).unwrap();
    entries.push(extra.clone());
    fs::write(
        &index,
        entries
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    assert_eq!(
        replay_cli(&f, &raw, &f.0.join("exact"), 13).0["students"][0]["status"],
        "refreshed"
    );
    // With matching parameters, the later saved status wins.
    extra["sequence"] = json!(7);
    extra["parameters"] = entries[2]["parameters"].clone();
    entries.push(extra);
    fs::write(dir.join("000007-textbook.wrongnote.body"), body).unwrap();
    fs::write(
        &index,
        entries
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    let out = replay_cli(&f, &raw, &f.0.join("latest"), 13).0;
    assert_eq!(out["students"][0]["status"], "capture_request_failed");
    assert!(
        out["students"][0]["cause"].as_array().unwrap().len() >= 2,
        "{out}"
    );
    assert_eq!(
        replay_cli(&f, &raw, &raw.join("forbidden-spool"), 13).0["status"],
        "replay_spool_refused"
    );
    assert!(!raw.join("forbidden-spool").exists());
}

#[test]
fn textbook_binary_admission_refusal_retains_cause_without_opening_credentials() {
    let f = Fixture::new();
    let mut a = admission();
    a.operations.retain(|op| op.id != "textbook.detail");
    let bytes = serde_json::to_vec(&a).unwrap();
    let name = "transport-admission.json";
    fs::write(f.0.join(name), &bytes).unwrap();
    fs::set_permissions(f.0.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    let roster_path = f.0.join("roster.json");
    fs::write(&roster_path, serde_json::to_vec(&roster()).unwrap()).unwrap();
    fs::set_permissions(&roster_path, fs::Permissions::from_mode(0o600)).unwrap();
    let binary = academy_capture_binary();
    let out = std::process::Command::new(binary)
        .args(["textbook", "--owner-root"])
        .arg(&f.0)
        .args([
            "--admission",
            name,
            "--admission-sha256",
            &crate::data::hash(&bytes),
            "--roster",
        ])
        .arg(roster_path)
        .arg("--spool")
        .arg(f.0.join("spool"))
        .args(["--date", "2040-01-04", "--max-requests", "1", "--no-detail"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stderr.is_empty());
    let text = String::from_utf8(out.stdout).unwrap();
    for canary in [
        "Invented Learner",
        "Fictional Teacher",
        "77001",
        "99001",
        "88001",
    ] {
        assert!(!text.contains(canary));
    }
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value["students"][0]["cause"],
        json!([
            "student_read_contract_not_admitted",
            "capture_admission_refused"
        ])
    );
    assert_eq!(value["students"][0]["requests"], 0);
}

#[test]
fn report_causes_are_bounded_codes_only_and_never_include_properties() {
    let mut error = fail("source_json_rejected");
    error.name = "Name_CANARY".into();
    error.properties.set("label", "Payload_CANARY".into());
    for code in [
        "inner_wrapper",
        "middle_wrapper",
        "outer_wrapper",
        "final_wrapper",
    ] {
        error = context(error, code);
    }
    let chain = causes(&error);
    assert_eq!(chain.len(), 4);
    assert!(chain.iter().all(|s| {
        !s.is_empty()
            && s.len() <= 64
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    }));
    assert!(!serde_json::to_string(&chain).unwrap().contains("CANARY"));
    assert!(causes(&Error::error("Value CANARY/path")).is_empty());
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
