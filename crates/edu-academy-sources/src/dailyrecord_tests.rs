//! All identities, contents and answers are invented; no network.
use super::store::{open_lock, publish_with};
use super::*;
use std::{
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    sync::atomic::{AtomicUsize, Ordering},
};
const HTML: &str = include_str!("../tests/fixtures/dailyrecord/day.html");
const LESSON_RECORD_LINE: &str = "{\"attendance\":\"Y\",\"attendance_label\":\"Present\",\"course_id\":\"3\",\"course_label\":\"Algebra A\",\"cut_fields\":0,\"daily_test_assessment\":\"2\",\"homework\":\"p.41-44\",\"lesson_date\":\"2031-03-03\",\"lesson_memo\":\"line one\\nline two | pipe\",\"membership_key\":\"50001\",\"observed_at\":1,\"previous_homework\":\"p.30-33\",\"previous_homework_assessment\":\"3\",\"previous_lesson_date_literal\":\"02/28\",\"previous_lesson_memo\":\"\",\"previous_progress\":\"Unit 2\",\"previous_record_key\":\"40\",\"progress\":\"Unit 3 to p.40\",\"record_key\":\"41\",\"row_ordinal\":0,\"source_namespace\":\"academy.lms.dayrecord\",\"student_id\":\"77001\",\"student_memo\":\"Invented memo\",\"student_name\":\"Ada Invented\",\"type\":\"lesson_record\"}";
const LESSON_DAY_LINE: &str = "{\"cut_fields\":0,\"lesson_date\":\"2031-03-03\",\"observed_at\":1,\"outside_roster_count\":0,\"row_count\":1,\"source_namespace\":\"academy.lms.dayrecord\",\"stored_count\":1,\"type\":\"lesson_day\"}";
struct Clock;
impl Runtime for Clock {
    fn epoch_ms(&self) -> u64 {
        1000
    }
    fn now(&self) -> String {
        String::new()
    }
    fn uuid(&self) -> edu_academy_core::js::Result<String> {
        Ok("invented".into())
    }
}
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = PathBuf::from(std::env::var("S10_TEST_ROOT").unwrap()).join(format!(
            "dailyrecord-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        Self(p)
    }
    fn options(&self) -> Options {
        Options {
            dates: vec!["2031-03-03".into()],
            max_requests: 1,
            spool: self.0.join("spool"),
            raw_dir: None,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn students() -> Vec<Student> {
    vec![Student {
        student_id: "77001".into(),
        student_name: "Different roster name".into(),
    }]
}
fn admission() -> TransportAdmission {
    TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "invented".into(),
        teacher_id: "99001".into(),
        teacher_label: "Invented Teacher".into(),
        college_id: "88001".into(),
        allowed_from: "2031-03-01".into(),
        allowed_through: "2031-03-31".into(),
        student_ids: vec!["77001".into()],
        source_namespaces: vec![DAY_NS.into()],
        operations: crate::capture::contracts_for(&[crate::capture::Family::DailyRecord]),
    }
}
fn page(html: &str) -> Page {
    parse_page(html, "2031-03-03", "99001", &students(), 1).unwrap()
}
fn row_html() -> String {
    HTML[HTML.find("<tr><td><span").unwrap()..HTML.find("</table></form>").unwrap()].to_string()
}
fn twice(change: bool) -> String {
    let row = row_html();
    let mut second = row
        .replace("(0,", "(1,")
        .replace("txt0", "txt1")
        .replace("attn0", "attn1")
        .replace("_0", "_1");
    if change {
        second = second.replace("p.41-44", "Different homework");
    }
    HTML.replace(&row, &format!("{row}{second}"))
}
#[test]
fn pins_contract_lines_and_row_values() {
    let f = Fixture::new();
    let p = page(HTML);
    let o = f.options();
    assert_eq!(p.records[0].to_string(), LESSON_RECORD_LINE);
    publish(&o.spool, "2031-03-03", &p, 1, Limits::default()).unwrap();
    assert_eq!(
        fs::read_to_string(o.spool.join("lesson_record/2031-03.jsonl")).unwrap(),
        format!("{LESSON_RECORD_LINE}\n")
    );
    assert_eq!(
        fs::read_to_string(o.spool.join("lesson_day/2031.jsonl")).unwrap(),
        format!("{LESSON_DAY_LINE}\n")
    );
}
#[test]
fn page_scope_completion_and_ordinals_refuse() {
    for (html, code) in [
        (
            HTML.replace("value=\"2031-03-03\"", "value=\"2031-03-04\""),
            "capture_day_date_conflict",
        ),
        (
            HTML.replace("<option value=\"3\">", "<option value=\"3\" selected>"),
            "capture_day_group_conflict",
        ),
        (HTML.replace("</html>", ""), "capture_day_incomplete"),
        (HTML.replace("(0,", "(2,"), "capture_day_ordinal_gap"),
        (
            HTML.replace("h_daily_test\"", "missing\""),
            "capture_day_structure_invalid",
        ),
    ] {
        assert_eq!(
            parse_page(&html, "2031-03-03", "99001", &students(), 1)
                .err()
                .unwrap()
                .message,
            code
        );
    }
}
#[test]
fn duplicate_keys_compare_before_cutting_and_outside_roster_is_not_stored() {
    let same = page(&twice(false));
    assert_eq!((same.rows, same.records.len()), (2, 1));
    assert_eq!(
        parse_page(&twice(true), "2031-03-03", "99001", &students(), 1)
            .err()
            .unwrap()
            .message,
        "duplicate_key"
    );
    let outside = page(&HTML.replace("77001", "77002"));
    assert_eq!(
        (outside.rows, outside.outside, outside.records.len()),
        (1, 1, 0)
    );
    for id in ["", "1|2", "abc", &"7".repeat(33)] {
        assert!(
            parse_page(
                &HTML.replace("77001", id),
                "2031-03-03",
                "99001",
                &students(),
                1
            )
            .is_err()
        );
    }
    for key in ["1|2", &"7".repeat(33)] {
        assert!(
            parse_page(
                &HTML.replace("50001", key),
                "2031-03-03",
                "99001",
                &students(),
                1
            )
            .is_err()
        );
    }
    let zeros = HTML.replace("77001", "007").replace("50001", "000");
    let roster = vec![Student {
        student_id: "007".into(),
        student_name: String::new(),
    }];
    assert_eq!(
        parse_page(&zeros, "2031-03-03", "99001", &roster, 1)
            .unwrap()
            .records[0]["student_id"],
        "007"
    );
}
#[test]
fn escaped_bounds_cut_text_names_labels_and_refuse_keys_codes() {
    for (field, bound) in [
        ("progress", 6000),
        ("homework", 6000),
        ("lesson_memo", 6000),
        ("student_memo", 6000),
        ("previous_progress", 6000),
        ("previous_homework", 6000),
        ("previous_lesson_memo", 6000),
        ("student_name", 200),
        ("course_label", 200),
        ("attendance_label", 64),
        ("previous_lesson_date_literal", 64),
    ] {
        let mut r = page(HTML).records.remove(0);
        let raw = "\"\\\n가".repeat(bound);
        r[field] = json!(raw);
        bound_row(&mut r).unwrap();
        let value = r[field].as_str().unwrap();
        assert!(escaped_len(value) <= bound);
        assert!(value.ends_with(&format!(" [cut: {} bytes in the LMS]", raw.len())));
        assert_eq!(r["cut_fields"], 1);
    }
    for (field, bound) in [
        ("student_id", 32),
        ("membership_key", 32),
        ("course_id", 32),
        ("record_key", 32),
        ("previous_record_key", 32),
        ("attendance", 16),
        ("daily_test_assessment", 16),
        ("previous_homework_assessment", 16),
    ] {
        let mut r = page(HTML).records.remove(0);
        r[field] = json!("\"".repeat(bound / 2 + 1));
        assert!(bound_row(&mut r).is_err(), "{field}");
    }
}
#[test]
fn optional_keys_and_empty_fields_are_explicit() {
    let html = HTML.replace("'41'", "''").replace("'40'", "''");
    let p = page(&html);
    assert!(p.records[0].get("record_key").is_none());
    assert!(p.records[0].get("previous_record_key").is_none());
    assert_eq!(p.records[0]["previous_lesson_memo"], "");
}
#[test]
fn replacement_preserves_other_dates_and_partitions_and_publishes_zero() {
    let f = Fixture::new();
    let o = f.options();
    let p = page(HTML);
    publish(&o.spool, "2031-03-03", &p, 1, Limits::default()).unwrap();
    let other = LESSON_RECORD_LINE.replace("2031-03-03", "2031-03-04");
    let month = o.spool.join("lesson_record/2031-03.jsonl");
    fs::write(&month, format!("{other}  \n{LESSON_RECORD_LINE}\n")).unwrap();
    let other_month = o.spool.join("lesson_record/2031-04.jsonl");
    fs::write(
        &other_month,
        format!("{}\n", other.replace("2031-03-04", "2031-04-04")),
    )
    .unwrap();
    fs::set_permissions(&other_month, fs::Permissions::from_mode(0o600)).unwrap();
    let bytes = fs::read(&other_month).unwrap();
    let empty = page(&HTML.replace(&row_html(), ""));
    assert_eq!(
        publish(&o.spool, "2031-03-03", &empty, 1, Limits::default()).unwrap(),
        1
    );
    assert_eq!(fs::read_to_string(&month).unwrap(), format!("{other}  \n"));
    assert_eq!(fs::read(&other_month).unwrap(), bytes);
    let day: Value = serde_json::from_str(
        fs::read_to_string(o.spool.join("lesson_day/2031.jsonl"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(day["row_count"], 0);
}
#[test]
fn old_files_type_time_duplicates_and_each_capacity_refuse_without_replacement() {
    for bad in [
        "not json".to_string(),
        LESSON_DAY_LINE.into(),
        format!("{LESSON_RECORD_LINE}\n{LESSON_RECORD_LINE}"),
        LESSON_RECORD_LINE.replace("\"observed_at\":1", "\"observed_at\":2"),
    ] {
        let f = Fixture::new();
        let o = f.options();
        publish(&o.spool, "2031-03-03", &page(HTML), 1, Limits::default()).unwrap();
        let path = o.spool.join("lesson_record/2031-03.jsonl");
        fs::write(&path, &bad).unwrap();
        let day = fs::read(o.spool.join("lesson_day/2031.jsonl")).unwrap();
        assert!(publish(&o.spool, "2031-03-03", &page(HTML), 1, Limits::default()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), bad);
        assert_eq!(
            fs::read(o.spool.join("lesson_day/2031.jsonl")).unwrap(),
            day
        );
    }
    for limits in [
        Limits {
            files: 0,
            ..Limits::default()
        },
        Limits {
            lines: 0,
            ..Limits::default()
        },
        Limits {
            bytes: 1,
            ..Limits::default()
        },
        Limits {
            file_bytes: 1,
            ..Limits::default()
        },
    ] {
        let f = Fixture::new();
        let o = f.options();
        assert!(publish(&o.spool, "2031-03-03", &page(HTML), 1, limits).is_err());
        assert!(!o.spool.join("lesson_record/2031-03.jsonl").exists());
        assert!(!o.spool.join("lesson_day/2031.jsonl").exists());
    }
    let f = Fixture::new();
    let o = f.options();
    let mut p = page(HTML);
    p.records[0]["observed_at"] = json!(2);
    assert!(publish(&o.spool, "2031-03-03", &p, 1, Limits::default()).is_err());
}
struct Port {
    a: TransportAdmission,
    reads: usize,
    logins: usize,
    authenticated: bool,
    bad: Option<usize>,
    page: Option<String>,
}
impl ReadPort for Port {
    fn transport_admission(&self) -> &TransportAdmission {
        &self.a
    }
    fn authenticate_for_reads(&mut self) -> Result<()> {
        if !self.authenticated {
            self.logins += 1;
            self.authenticated = true;
        }
        Ok(())
    }
    fn read(
        &mut self,
        s: &crate::owner_admission::TransportScope,
        op: &str,
        args: &[(String, String)],
    ) -> Result<crate::http::Response> {
        assert_eq!(op, OPERATION);
        self.a.request(s, op, args, false)?;
        self.reads += 1;
        let html = if self.bad == Some(self.reads) {
            "broken".into()
        } else {
            self.page
                .as_deref()
                .unwrap_or(HTML)
                .replace("2031-03-03", &s.date)
        };
        Ok(crate::http::Response {
            status: 200,
            headers: vec![],
            bytes: html.into_bytes(),
            url: format!("{}{}", crate::http::ORIGIN, crate::http::MAIN),
            stalled: false,
        })
    }
}
fn port() -> Port {
    Port {
        a: admission(),
        reads: 0,
        logins: 0,
        authenticated: false,
        bad: None,
        page: None,
    }
}
#[test]
fn one_login_three_dates_and_refusal_stops_with_safe_counts() {
    let f = Fixture::new();
    let mut o = f.options();
    o.dates = vec![
        "2031-03-03".into(),
        "2031-03-04".into(),
        "2031-03-05".into(),
    ];
    o.max_requests = 3;
    let mut p = port();
    let reports = run(&mut p, &students(), &o, &Clock);
    assert!(reports.iter().all(|r| r.status == "published"));
    assert_eq!((p.reads, p.logins), (3, 1));
    let mut p = port();
    p.bad = Some(2);
    let reports = run(&mut p, &students(), &o, &Clock);
    assert_eq!((p.reads, p.logins), (2, 1));
    assert_eq!(
        reports
            .iter()
            .map(|r| r.status.as_str())
            .collect::<Vec<_>>(),
        ["published", "refused", "not_run"]
    );
    let output = serde_json::to_string(&reports).unwrap();
    for private in ["Ada Invented", "Algebra A", "Unit 3"] {
        assert!(!output.contains(private));
    }
    let mut p = port();
    o.max_requests = 1;
    let reports = run(&mut p, &students(), &o, &Clock);
    assert_eq!(reports[1].code.as_deref(), Some("capture_request_limit"));
    assert_eq!(p.reads, 1);
}
#[test]
fn invalid_inputs_missing_contract_and_window_never_leave_a_request() {
    let f = Fixture::new();
    for dates in [
        vec![],
        vec!["2031-02-29".into()],
        vec!["20310303".into()],
        vec!["2031-3-03".into()],
        vec!["2031-03-03".into(); 2],
        vec!["2031-03-03".into(); 32],
    ] {
        let mut p = port();
        let mut o = f.options();
        o.dates = dates;
        run(&mut p, &students(), &o, &Clock);
        assert_eq!((p.reads, p.logins), (0, 0));
    }
    let mut maximum = f.options();
    maximum.dates = (1..=31).map(|d| format!("2031-03-{d:02}")).collect();
    assert!(preflight(&admission(), &students(), &maximum).is_ok());
    maximum.dates.push("2031-04-01".into());
    let mut count = port();
    run(&mut count, &students(), &maximum, &Clock);
    assert_eq!((count.reads, count.logins), (0, 0));
    let mut p = port();
    p.a.operations.clear();
    run(&mut p, &students(), &f.options(), &Clock);
    assert_eq!((p.reads, p.logins), (0, 0));
    let mut p = port();
    let mut o = f.options();
    o.dates = vec!["2031-04-01".into()];
    let r = run(&mut p, &students(), &o, &Clock);
    assert_eq!(r[0].code.as_deref(), Some("native_transport_scope_refused"));
    assert_eq!((p.reads, p.logins), (0, 0));
    for kind in ["lesson_record", "lesson_day"] {
        assert_eq!(
            fs::metadata(o.spool.join(kind))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
#[test]
fn both_types_are_bounded_and_coverage_is_renamed_last() {
    for kind in ["lesson_record", "lesson_day"] {
        for dimension in ["files", "lines", "bytes", "file_bytes"] {
            let f = Fixture::new();
            let o = f.options();
            let p = if kind == "lesson_day" {
                page(&HTML.replace(&row_html(), ""))
            } else {
                page(HTML)
            };
            let mut limits = Limits::default();
            match dimension {
                "files" => {
                    ensure_dirs(&o.spool).unwrap();
                    let extra = o.spool.join(kind).join("1999.jsonl");
                    fs::write(&extra, b"").unwrap();
                    fs::set_permissions(extra, fs::Permissions::from_mode(0o600)).unwrap();
                    limits.files = 1;
                }
                "lines" => limits.lines = 0,
                "bytes" => limits.bytes = 1,
                _ => limits.file_bytes = 1,
            }
            let code = match dimension {
                "files" => "capture_day_too_many_files",
                "lines" => "capture_day_too_many_records",
                "bytes" => "capture_day_source_text_too_large",
                _ => "capture_day_file_too_large",
            };
            assert_eq!(
                publish(&o.spool, "2031-03-03", &p, 1, limits)
                    .err()
                    .unwrap()
                    .message,
                code,
                "{kind} {dimension}"
            );
            assert!(!o.spool.join("lesson_day/2031.jsonl").exists());
        }
    }
    let f = Fixture::new();
    let o = f.options();
    let order = std::cell::RefCell::new(Vec::new());
    let result = publish_with(
        &o.spool,
        "2031-03-03",
        &page(HTML),
        1,
        Limits::default(),
        &|from, to| {
            order.borrow_mut().push(
                to.parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            );
            if order.borrow().len() == 2 {
                return Err(std::io::Error::other("invented failure"));
            }
            fs::rename(from, to)
        },
    );
    assert!(result.is_err());
    assert_eq!(*order.borrow(), ["lesson_record", "lesson_day"]);
    assert!(o.spool.join("lesson_record/2031-03.jsonl").exists());
    assert!(!o.spool.join("lesson_day/2031.jsonl").exists());
}
#[test]
fn spool_lock_covers_old_read_through_both_renames() {
    use std::{sync::mpsc, time::Duration};
    let f = Fixture::new();
    let o = f.options();
    ensure_dirs(&o.spool).unwrap();
    let lock = open_lock(&o.spool).unwrap();
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive).unwrap();
    let (tx, rx) = mpsc::channel();
    let spool = o.spool.clone();
    let child = std::thread::spawn(move || {
        publish(&spool, "2031-03-03", &page(HTML), 1, Limits::default()).unwrap();
        tx.send(()).unwrap();
    });
    assert!(rx.recv_timeout(Duration::from_millis(40)).is_err());
    // A writer that read before obtaining the lock would lose this new date.
    let other = LESSON_RECORD_LINE.replace("2031-03-03", "2031-03-04");
    fs::write(
        o.spool.join("lesson_record/2031-03.jsonl"),
        format!("{other}\n"),
    )
    .unwrap();
    fs::set_permissions(
        o.spool.join("lesson_record/2031-03.jsonl"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::Unlock).unwrap();
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    child.join().unwrap();
    assert!(
        fs::read_to_string(o.spool.join("lesson_record/2031-03.jsonl"))
            .unwrap()
            .starts_with(&other)
    );
}
#[test]
fn replay_uses_the_recorded_all_groups_answer_and_identical_publisher() {
    let f = Fixture::new();
    let raw = f.0.join("raw");
    fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
    let dir = raw.join("77001");
    fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    let request = admission()
        .request(
            &scope(&admission(), &students()[0], "2031-03-03", DAY_NS),
            OPERATION,
            &[("std_ymd".into(), "20310303".into())],
            false,
        )
        .unwrap();
    fs::write(dir.join("000001-student.day.groups.body"), HTML).unwrap();
    fs::write(dir.join("index.jsonl"),format!("{}\n",json!({"sequence":1,"operation":OPERATION,"parameters":{"query":request.query,"body":request.body},"http_status":200,"byte_count":HTML.len(),"sha256":crate::data::hash(HTML.as_bytes()),"observed_at":1}))).unwrap();
    let identity = f.0.join("identity.json");
    fs::write(&identity,json!({"teacherSourceKey":"99001","teacherLabel":"Invented Teacher","collegeSourceKey":"88001","sourceUsername":"invented"}).to_string()).unwrap();
    let roster = f.0.join("students.json");
    fs::write(&roster, serde_json::to_vec(&students()).unwrap()).unwrap();
    let spool = f.0.join("replayed");
    let args = vec![
        "--family".into(),
        "dailyrecord".into(),
        "--raw-dir".into(),
        raw.to_str().unwrap().into(),
        "--identity".into(),
        identity.to_str().unwrap().into(),
        "--students".into(),
        roster.to_str().unwrap().into(),
        "--spool".into(),
        spool.to_str().unwrap().into(),
        "--dates".into(),
        "2031-03-03".into(),
    ];
    struct Later;
    impl Runtime for Later {
        fn epoch_ms(&self) -> u64 {
            100_000
        }
        fn now(&self) -> String {
            String::new()
        }
        fn uuid(&self) -> edu_academy_core::js::Result<String> {
            Ok("invented".into())
        }
    }
    let out = crate::replay::run(args.into_iter(), &Later).unwrap();
    assert_eq!(out["dates"][0]["status"], "published");
    assert_eq!(
        fs::read_to_string(spool.join("lesson_record/2031-03.jsonl")).unwrap(),
        format!("{LESSON_RECORD_LINE}\n")
    );
    assert_eq!(
        fs::read_to_string(spool.join("lesson_day/2031.jsonl")).unwrap(),
        format!("{LESSON_DAY_LINE}\n")
    );
}
#[test]
fn native_session_logs_in_once_keeps_lease_and_records_only_source_answers() {
    use crate::{
        capture::Recorder,
        http::{self, Request, Response, Transport},
        owner_root::OwnerRoot,
        owner_workflow::{Credentials, NativeSession},
    };
    use std::{cell::RefCell, rc::Rc};
    struct Tick(std::cell::Cell<u64>);
    impl Runtime for Tick {
        fn epoch_ms(&self) -> u64 {
            let n = self.0.get();
            self.0.set(n + 1000);
            n
        }
        fn now(&self) -> String {
            String::new()
        }
        fn uuid(&self) -> edu_academy_core::js::Result<String> {
            Ok("invented".into())
        }
    }
    let clock = Tick(std::cell::Cell::new(1000));
    struct Wire(Rc<RefCell<Vec<String>>>);
    impl Transport for Wire {
        fn dispatch(&mut self, r: &Request) -> Result<Response> {
            self.0.borrow_mut().push(r.path.clone());
            let mut headers = vec![];
            let body = if r.path == "/c_login.jsp" {
                headers.push((
                    "Set-Cookie".into(),
                    "JSESSIONID=invented; Path=/; HttpOnly".into(),
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
            } else if r.path == http::MAIN {
                let date = r
                    .body
                    .iter()
                    .find(|(k, _)| k == "std_ymd")
                    .unwrap()
                    .1
                    .clone();
                HTML.replace(
                    "2031-03-03",
                    &format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]),
                )
            } else {
                "<form name=userForm><input name=pri_no value=99001></form><form name=form1><input name=g_pri_no value=99001><input name=g_clg_no value=88001><input name=g_mem_type value=invented></form><a>로그아웃</a>".into()
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
    let f = Fixture::new();
    let shared = f.0.join("shared.lock");
    fs::write(&shared, b"invented").unwrap();
    let config = f.0.join("account-lock.json");
    fs::write(
        &config,
        json!({"schemaVersion":"academy.account-lock/1","path":shared}).to_string(),
    )
    .unwrap();
    fs::set_permissions(config, fs::Permissions::from_mode(0o600)).unwrap();
    let raw = f.0.join("raw");
    fs::DirBuilder::new().mode(0o700).create(&raw).unwrap();
    let mut o = f.options();
    o.raw_dir = Some(raw.clone());
    o.dates = vec![
        "2031-03-03".into(),
        "2031-03-04".into(),
        "2031-03-05".into(),
    ];
    o.max_requests = 3;
    let seen = Rc::new(RefCell::new(Vec::new()));
    let recorder = Recorder::new(o.raw_dir.clone(), &clock);
    let session = NativeSession::open(
        Rc::new(OwnerRoot::open(&f.0).unwrap()),
        admission(),
        recorder.transport(Wire(seen.clone())),
        || Ok(Credentials::new("invented".into(), "invented".into())),
        http::Cancellation::default(),
        &clock,
    )
    .unwrap();
    let mut session = recorder.port(session);
    let reports = run(&mut session, &students(), &o, &clock);
    assert!(reports.iter().all(|r| r.status == "published"));
    assert_eq!(
        seen.borrow()
            .iter()
            .filter(|p| p.as_str() == "/servlet/controller.login.LoginServlet")
            .count(),
        1
    );
    assert_eq!(
        seen.borrow()
            .iter()
            .filter(|p| p.as_str() == http::MAIN)
            .count(),
        3
    );
    let lock = OpenOptions::new().read(true).open(&shared).unwrap();
    assert!(
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_err()
    );
    drop(session);
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).unwrap();
    let index = fs::read_to_string(raw.join("77001/index.jsonl")).unwrap();
    assert_eq!(index.lines().count(), 3);
    assert!(!index.contains("login"));
    let stored = fs::read_to_string(o.spool.join("lesson_record/2031-03.jsonl")).unwrap();
    for (entry, line) in index.lines().zip(stored.lines()) {
        let raw: Value = serde_json::from_str(entry).unwrap();
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            row["observed_at"], raw["observed_at"],
            "capture must use the recorder's exact read second"
        );
    }
}
#[test]
fn older_replacement_refuses_even_when_both_times_are_before_now() {
    let f = Fixture::new();
    let o = f.options();
    publish(&o.spool, "2031-03-03", &page(HTML), 1, Limits::default()).unwrap();
    let path = o.spool.join("lesson_record/2031-03.jsonl");
    let before = fs::read(&path).unwrap();
    let mut p = page(HTML);
    p.records[0]["observed_at"] = json!(0);
    assert_eq!(
        publish(&o.spool, "2031-03-03", &p, 2, Limits::default())
            .err()
            .unwrap()
            .message,
        "record_older_than_head"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::write(
        o.spool.join("lesson_day/2031.jsonl"),
        LESSON_DAY_LINE.replace("\"observed_at\":1", "\"observed_at\":2"),
    )
    .unwrap();
    assert_eq!(
        publish(&o.spool, "2031-03-03", &page(HTML), 3, Limits::default())
            .err()
            .unwrap()
            .message,
        "record_older_than_head"
    );
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn owner_preparation_adds_only_the_date_family_contract_and_namespace() {
    let f = Fixture::new();
    let put = |name: &str, bytes: &[u8]| {
        let p = f.0.join(name);
        fs::write(&p, bytes).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        p
    };
    let input=crate::prepare_owner::Inputs {
        identity:put("identity.json",br#"{"teacherSourceKey":"99001","teacherLabel":"Invented Teacher","collegeSourceKey":"88001","sourceUsername":"invented"}"#),
        secret_env:put("secret.env",b"INVENTED_KEY=invented\n"),secret_key:"INVENTED_KEY".into(),lock:put("shared.lock",b"invented"),
        students:put("students.json",&serde_json::to_vec(&students()).unwrap()),paper_form:put("form.json",b"null"),allowed_from:"2031-03-01".into(),allowed_through:"2031-03-31".into(),expires_at:"2031-03-31T23:00:00Z".into(),owner_root:f.0.join("owner"),
    };
    let result = crate::prepare_owner::prepare_families(
        &input,
        &[crate::capture::Family::DailyRecord],
        &Clock,
    )
    .unwrap();
    assert_eq!(result["operationCount"], 1);
    let a = TransportAdmission::from_bytes(
        &fs::read(input.owner_root.join("transport-admission.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(a.operations[0].id, OPERATION);
    assert!(a.source_namespaces.contains(&DAY_NS.to_string()));
    assert_eq!(a.student_ids, ["77001"]);
    let roster: crate::capture::Roster =
        serde_json::from_slice(&fs::read(input.owner_root.join("roster.json")).unwrap()).unwrap();
    assert_eq!(roster.students[0].student_name, "Different roster name");
    assert!(roster.paper_form.is_null());
}
#[test]
fn absent_or_changed_operation_contract_and_later_window_refusal_are_checked_per_request() {
    for changed in [false, true] {
        let f = Fixture::new();
        let mut p = port();
        if changed {
            p.a.operations[0].body[1].name = "other_date".into();
        } else {
            p.a.operations = crate::capture::contracts_for(&[crate::capture::Family::DailyTest]);
            p.a.source_namespaces
                .push(crate::student_course::METADATA_NS.into());
        }
        assert!(p.a.validate().is_ok());
        let r = run(&mut p, &students(), &f.options(), &Clock);
        assert_eq!(
            r[0].code.as_deref(),
            Some("student_read_contract_not_admitted")
        );
        assert_eq!((p.reads, p.logins), (0, 0));
        for kind in ["lesson_record", "lesson_day"] {
            assert_eq!(
                fs::metadata(f.options().spool.join(kind))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }
    let f = Fixture::new();
    let mut p = port();
    let mut o = f.options();
    o.dates = vec![
        "2031-03-03".into(),
        "2031-04-01".into(),
        "2031-03-05".into(),
    ];
    o.max_requests = 3;
    let reports = run(&mut p, &students(), &o, &Clock);
    assert_eq!(reports[0].status, "published");
    assert_eq!(
        reports[1].code.as_deref(),
        Some("native_transport_scope_refused")
    );
    assert_eq!(reports[2].status, "not_run");
    assert_eq!((p.reads, p.logins), (1, 1));
}
#[test]
fn conflicting_duplicates_are_compared_before_truncation_and_outside_counts_are_rows() {
    let long = format!("{}tail", "x".repeat(6100));
    let html = twice(false).replacen("p.41-44", &long, 1).replacen(
        "p.41-44",
        &format!("{long}different"),
        1,
    );
    assert_eq!(
        parse_page(&html, "2031-03-03", "99001", &students(), 1)
            .err()
            .unwrap()
            .message,
        "duplicate_key"
    );
    let p = page(&twice(false).replace("77001", "77002"));
    assert_eq!((p.rows, p.outside, p.records.len()), (2, 2, 0));
}
#[test]
fn unreadable_coverage_keeps_both_prior_files_and_other_year_is_untouched() {
    let f = Fixture::new();
    let o = f.options();
    publish(&o.spool, "2031-03-03", &page(HTML), 1, Limits::default()).unwrap();
    let path = o.spool.join("lesson_record/2031-03.jsonl");
    let rows = fs::read(&path).unwrap();
    let other = o.spool.join("lesson_day/2030.jsonl");
    let bytes = LESSON_DAY_LINE.replace("2031-03-03", "2030-03-03");
    fs::write(&other, &bytes).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o600)).unwrap();
    let coverage = o.spool.join("lesson_day/2031.jsonl");
    fs::remove_file(&coverage).unwrap();
    std::os::unix::fs::symlink(&other, &coverage).unwrap();
    assert!(publish(&o.spool, "2031-03-03", &page(HTML), 1, Limits::default()).is_err());
    assert_eq!(fs::read(path).unwrap(), rows);
    assert_eq!(fs::read_to_string(other).unwrap(), bytes);
    assert!(
        fs::symlink_metadata(coverage)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
fn swap(html: &str, from: &str, to: &str) -> String {
    assert!(html.contains(from), "invented row lacks {from}");
    html.replacen(from, to, 1)
}
/// The fixture's row as row `n` of a page: its own ordinal, membership and record keys.
fn numbered_row(n: usize) -> String {
    row_html()
        .replace("(0,", &format!("({n},"))
        .replace("txt0", &format!("txt{n}"))
        .replace("attn0", &format!("attn{n}"))
        .replace("_0\"", &format!("_{n}\""))
        .replace("50001", &format!("5000{}", n + 1))
        .replace("'41'", &format!("'4{}'", n + 1))
        .replace("'40'", &format!("'3{}'", n + 1))
}
const RATE_HANDLER: &str =
    "<img id=\"imgHomeworkRate_1\" onclick=\"openHomeWorkRate('32','50002',3)\">";
const RATE_CONTROL: &str =
    "<input type=\"hidden\" id=\"h_homework_rate_1\" name=\"h_homework_rate\" value=\"3\">";
/// The middle row (number 1) of a three-row page, as the test changes it.
fn three_rows(middle: impl Fn(String) -> String) -> String {
    let rows = [numbered_row(0), middle(numbered_row(1)), numbered_row(2)].concat();
    HTML.replace(&row_html(), &rows)
}
/// Row 1 with the control and the handler of the previous homework removed and the four previous
/// cells set to `[date, progress, homework, memo]`.
fn without_previous_lesson(row: String, texts: [&str; 4]) -> String {
    let [date, progress, homework, memo] = texts;
    let row = swap(
        &row,
        "<textarea readonly></textarea>",
        &format!("<textarea readonly>{memo}</textarea>"),
    );
    let row = swap(&row, "<td>02/28</td>", &format!("<td>{date}</td>"));
    let row = swap(&row, "Unit 2", progress);
    let row = swap(&row, "p.30-33", homework);
    swap(&swap(&row, RATE_CONTROL, ""), RATE_HANDLER, "")
}
fn first_lesson(row: String) -> String {
    without_previous_lesson(row, ["", "", "", ""])
}
#[test]
fn l2_a_row_without_a_previous_lesson_is_stored() {
    let f = Fixture::new();
    let mut o = f.options();
    let html = three_rows(first_lesson);
    let mut p = port();
    p.page = Some(html.clone());
    o.max_requests = 1;
    let reports = run(&mut p, &students(), &o, &Clock);
    assert_eq!(reports[0].status, "published");
    assert_eq!(reports[0].code, None);
    assert_eq!((reports[0].rows, reports[0].stored), (3, 3));
    let records = page(&html).records;
    assert_eq!(records.len(), 3);
    let spooled = fs::read_to_string(o.spool.join("lesson_record/2031-03.jsonl")).unwrap();
    let lines: Vec<_> = spooled.lines().collect();
    assert_eq!(lines.len(), 3);
    for (line, record) in lines.iter().zip(&records) {
        assert_eq!(*line, record.to_string());
    }
    // The rows around the middle one are read as before.
    for (n, ordinal) in [(0, 0), (2, 2)] {
        let line = lines[ordinal];
        assert_eq!(
            line,
            LESSON_RECORD_LINE
                .replace("\"row_ordinal\":0", &format!("\"row_ordinal\":{ordinal}"))
                .replace(
                    "\"membership_key\":\"50001\"",
                    &format!("\"membership_key\":\"5000{}\"", n + 1)
                )
                .replace(
                    "\"record_key\":\"41\"",
                    &format!("\"record_key\":\"4{}\"", n + 1)
                )
                .replace(
                    "\"previous_record_key\":\"40\"",
                    &format!("\"previous_record_key\":\"3{}\"", n + 1)
                )
        );
    }
    let middle: Value = serde_json::from_str(lines[1]).unwrap();
    for key in [
        "previous_lesson_date_literal",
        "previous_progress",
        "previous_homework",
        "previous_lesson_memo",
        "previous_homework_assessment",
    ] {
        assert_eq!(middle[key], "", "{key}");
    }
    assert!(middle.get("previous_record_key").is_none());
    assert_eq!(middle["record_key"], "42");
    assert_eq!(middle["row_ordinal"], 1);
    assert_eq!(middle["progress"], "Unit 3 to p.40");
    let day: Value = serde_json::from_str(
        fs::read_to_string(o.spool.join("lesson_day/2031.jsonl"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(
        (day["row_count"].clone(), day["stored_count"].clone()),
        (json!(3), json!(3))
    );
}
fn spool_files(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(d) = pending.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    files
}
#[test]
fn l2_a_half_previous_row_refuses() {
    let mut pages = vec![
        // the handler without the control
        three_rows(|row| swap(&row, RATE_CONTROL, "")),
        // the control without the handler
        three_rows(|row| swap(&row, RATE_HANDLER, "")),
    ];
    // neither, while one of the four previous cells holds text
    for held in 0..4 {
        let mut texts = [""; 4];
        texts[held] = "02/27";
        pages.push(three_rows(|row| without_previous_lesson(row, texts)));
    }
    // no handler, the control in another cell than its usual one, the four previous cells empty
    pages.push(three_rows(|row| {
        let row = first_lesson(row);
        swap(&row, "<td></td>", &format!("<td>{RATE_CONTROL}</td>"))
    }));
    for html in pages {
        let f = Fixture::new();
        let o = f.options();
        // A published date of the same day must stay as it is.
        let mut good = port();
        good.page = Some(three_rows(|row| row));
        assert_eq!(
            run(&mut good, &students(), &o, &Clock)[0].status,
            "published"
        );
        let before = spool_files(&o.spool);
        assert!(!before.is_empty());
        assert_eq!(
            parse_page(&html, "2031-03-03", "99001", &students(), 1)
                .err()
                .unwrap()
                .message,
            "capture_day_structure_invalid"
        );
        let mut p = port();
        p.page = Some(html);
        let reports = run(&mut p, &students(), &o, &Clock);
        assert_eq!(reports[0].status, "refused");
        assert_eq!(
            reports[0].code.as_deref(),
            Some("capture_day_structure_invalid")
        );
        assert_eq!((reports[0].rows, reports[0].stored), (0, 0));
        assert_eq!(spool_files(&o.spool), before);
    }
}
