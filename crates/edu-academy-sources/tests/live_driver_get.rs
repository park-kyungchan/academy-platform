//! Mode `get` of the live driver: invented pages, names and ids only.
//! The built binary has no transport seam and a synthetic page cannot reproduce the
//! qualified wire digest, so the success path runs through the library entry
//! (`live_driver::get`) over a scripted source port; the binary tests cover every
//! refusal that precedes the first request.
use edu_academy_core::{js::Result as CoreResult, workflow::Runtime};
use edu_academy_sources::{
    data::*,
    http::{MAIN, ORIGIN, Request, Response, SELECTED, Transport},
    live_driver::{self, RecordSource},
    native::{Backend, SourcePort, StageError},
    owner_root::OwnerRoot,
    owner_workflow::{Credentials, Owner, RecordRead, Recording, read_record},
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    path::{Path, PathBuf},
    rc::Rc,
};

const NOW: &str = "2040-01-04T09:00:00.000Z";
const WIRE: &str = "650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1";
const NAME: &str = "Fictional Learner";
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        NOW.into()
    }
    fn epoch_ms(&self) -> u64 {
        1000
    }
    fn uuid(&self) -> CoreResult<String> {
        Ok("fictional-get-uuid".into())
    }
}
fn base() -> PathBuf {
    let base = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap());
    std::fs::create_dir_all(&base).unwrap();
    base
}
/// A fresh private directory for one case.
fn private(case: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = base().join(format!("get-{case}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn launch() -> Js {
    edu_academy_core::templates::from_json(serde_json::json!({
        "teacherId":"99001","teacherLabel":"Fictional Teacher","date":"2040-01-04",
        "sourceBundleSha256":"b".repeat(64),
        "students":[{"studentId":"77001","displayName":NAME},{"studentId":"77002","displayName":"Other Fictional Learner"}]}))
}
fn scope(id: &str) -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("studentId", id.into()),
        ("lessonDate", "2040-01-04".into()),
    ])
}
fn row(date: &str, record: &str, fields: &[(&str, &str)]) -> Js {
    obj(&[
        ("studentName", NAME.into()),
        ("date", date.into()),
        ("groupId", "3".into()),
        ("teacherKey", "99001".into()),
        ("studentKey", "77001".into()),
        ("courseKey", "5".into()),
        ("recordKey", record.into()),
        ("membershipKey", "8".into()),
        (
            "fields",
            obj(&fields
                .iter()
                .map(|(k, v)| (*k, (*v).into()))
                .collect::<Vec<_>>()),
        ),
    ])
}
#[derive(Clone)]
struct Script {
    current: Vec<(&'static str, &'static str)>,
    previous: Vec<(&'static str, &'static str)>,
    assessments: (Js, Js),
    previous_rows: usize,
    state_error: Option<&'static str>,
    pairs: Vec<(&'static str, &'static str)>,
}
impl Script {
    fn full() -> Self {
        Self {
            current: vec![
                ("progress", "Canary progress"),
                ("homework", "Canary homework"),
                ("lessonMemo", "Canary memo"),
                ("attendance", "Y"),
            ],
            previous: vec![
                ("progress", "Canary prior progress"),
                ("homework", "Canary prior homework"),
                ("lessonMemo", "Canary prior memo"),
                ("attendance", "N"),
            ],
            assessments: ("0".into(), "3".into()),
            previous_rows: 1,
            state_error: None,
            pairs: vec![("77001", NAME)],
        }
    }
}
struct Port {
    script: Script,
    log: Rc<RefCell<Vec<String>>>,
}
impl SourcePort for Port {
    fn authenticate(&mut self) -> Result<Js> {
        self.log.borrow_mut().push("authenticate".into());
        Ok(Js::Null)
    }
    fn discover(&mut self, _: &Js) -> Result<Js> {
        self.log.borrow_mut().push("discover".into());
        Ok(obj(&[(
            "groups",
            Js::Arr(vec![obj(&[("groupId", "3".into())])]),
        )]))
    }
    fn execute(&mut self, input: &Js) -> Result<Js> {
        self.log.borrow_mut().push("execute".into());
        let date = string(input.get("date"));
        let rows = if date == "2040-01-04" {
            vec![row(&date, "13", &self.script.current)]
        } else {
            (0..self.script.previous_rows)
                .map(|_| row(&date, "12", &self.script.previous))
                .collect()
        };
        Ok(obj(&[("dayRecords", Js::Arr(rows))]))
    }
    fn journal_state(&mut self, target: &Js) -> Result<Js> {
        self.log.borrow_mut().push("journalState".into());
        if let Some(code) = self.script.state_error {
            return Err(Error::error(code));
        }
        let mut fields = obj(&[]);
        for (k, v) in &self.script.current {
            fields.set(k, (*v).into());
        }
        fields.set("dailyTestAssessment", self.script.assessments.0.clone());
        fields.set(
            "previousHomeworkAssessment",
            self.script.assessments.1.clone(),
        );
        Ok(obj(&[
            ("target", target.clone()),
            ("wireDigest", WIRE.into()),
            ("previousDate", "2040-01-02".into()),
            ("previousRecordKey", "12".into()),
            ("fields", fields),
        ]))
    }
    fn has_name_search(&self) -> bool {
        true
    }
    fn search_students_by_name(&mut self, _: &Js) -> Option<Result<Js>> {
        self.log.borrow_mut().push("searchStudentsByName".into());
        Some(Ok(Js::Arr(
            self.script
                .pairs
                .iter()
                .map(|(k, n)| obj(&[("studentKey", (*k).into()), ("studentName", (*n).into())]))
                .collect(),
        )))
    }
}
struct Source(Backend<Recording<Port>>);
fn source(script: Script) -> (Source, Rc<RefCell<Vec<String>>>) {
    let log = Rc::new(RefCell::new(vec![]));
    let backend = Backend::with_owner(
        Recording::new(Port {
            script,
            log: Rc::clone(&log),
        }),
        launch(),
        None,
        vec![],
    )
    .unwrap();
    (Source(backend), log)
}
impl RecordSource for Source {
    fn search_name(&mut self, name: &Js) -> CoreResult<Js> {
        self.0
            .search_students_by_name(name, None, None, 1000)
            .map_err(StageError::into_fault)
    }
    fn read_record(&mut self, scope: &Js) -> CoreResult<RecordRead> {
        read_record(&mut self.0, scope, 1000, NOW, "fictional-get-uuid")
            .map_err(StageError::into_fault)
    }
}
fn run(script: Script, name: Option<&str>, out: &Path) -> (CoreResult<Js>, Vec<String>) {
    let (mut source, log) = source(script);
    let name = name.map(Js::from);
    let result = live_driver::get(&mut source, &scope("77001"), name.as_ref(), out);
    let log = log.borrow().clone();
    (result, log)
}
fn code(result: CoreResult<Js>) -> String {
    result.expect_err("a refusal").message
}
fn assert_no_leak(summary: &Js) {
    let encoded = stringify(summary).unwrap();
    for private in [
        "Canary",
        NAME,
        "77001",
        "99001",
        "2040-01-04",
        "Fictional Teacher",
    ] {
        assert!(
            !encoded.contains(private),
            "private value in the summary: {private}"
        );
    }
}
fn assert_nothing_left(dir: &Path) {
    assert_eq!(
        std::fs::read_dir(dir).unwrap().count(),
        0,
        "a failed read leaves nothing in the directory"
    );
}

const EXPECTED_FULL: &str = r##"{"lessonDate":"2040-01-04","observed_at":"2040-01-04T09:00:00.000Z","previous":{"date":"2040-01-02","fields":{"attendance":{"state":"known","value":"N"},"dailyTestAssessment":{"reasonCode":"native_field_not_observed","state":"unknown"},"homework":{"state":"known","value":"Canary prior homework"},"lessonMemo":{"state":"known","value":"Canary prior memo"},"previousHomeworkAssessment":{"reasonCode":"native_field_not_observed","state":"unknown"},"progress":{"state":"known","value":"Canary prior progress"}},"identity":{"courseId":"5","lessonDate":"2040-01-02","membershipId":"8","recordId":"12","sourceNamespace":"academy.lms.dayrecord","studentId":"77001","teacherId":"99001"}},"record":{"date":"2040-01-04","fields":{"attendance":{"state":"known","value":"Y"},"dailyTestAssessment":{"state":"known","value":"0"},"homework":{"state":"known","value":"Canary homework"},"lessonMemo":{"state":"known","value":"Canary memo"},"previousHomeworkAssessment":{"state":"known","value":"3"},"progress":{"state":"known","value":"Canary progress"}},"identity":{"courseId":"5","lessonDate":"2040-01-04","membershipId":"8","recordId":"13","sourceNamespace":"academy.lms.dayrecord","studentId":"77001","teacherId":"99001"}},"schemaVersion":"academy.dailyrecord-get/1","studentId":"77001","toneSamples":[{"evidenceRefs":["native-dayrecord-read:fictional-get-uuid:previous"],"observedAt":"2040-01-04T09:00:00.000Z","target":{"courseId":"5","lessonDate":"2040-01-02","membershipId":"8","recordId":"12","sourceNamespace":"academy.lms.dayrecord","studentId":"77001","teacherId":"99001"},"text":"Canary prior progress\nCanary prior homework\nCanary prior memo"}],"transmissionState":"unknown"}"##;

const EXPECTED_EMPTY: &str = r##"{"lessonDate":"2040-01-04","observed_at":"2040-01-04T09:00:00.000Z","previous":{"date":"2040-01-02","fields":{"attendance":{"state":"known","value":""},"dailyTestAssessment":{"reasonCode":"native_field_not_observed","state":"unknown"},"homework":{"state":"known","value":""},"lessonMemo":{"state":"known","value":""},"previousHomeworkAssessment":{"reasonCode":"native_field_not_observed","state":"unknown"},"progress":{"state":"known","value":""}},"identity":{"courseId":"5","lessonDate":"2040-01-02","membershipId":"8","recordId":"12","sourceNamespace":"academy.lms.dayrecord","studentId":"77001","teacherId":"99001"}},"record":{"date":"2040-01-04","fields":{"attendance":{"state":"known","value":""},"dailyTestAssessment":{"reasonCode":"native_field_not_observed","state":"unknown"},"homework":{"state":"known","value":""},"lessonMemo":{"state":"known","value":""},"previousHomeworkAssessment":{"reasonCode":"native_field_not_observed","state":"unknown"},"progress":{"state":"known","value":""}},"identity":{"courseId":"5","lessonDate":"2040-01-04","membershipId":"8","recordId":"13","sourceNamespace":"academy.lms.dayrecord","studentId":"77001","teacherId":"99001"}},"schemaVersion":"academy.dailyrecord-get/1","studentId":"77001","toneSamples":[],"transmissionState":"unknown"}"##;

#[test]
fn get_by_id_writes_the_expected_file_for_a_full_record() {
    use std::os::unix::fs::PermissionsExt;
    let dir = private("full");
    let out = dir.join("record.json");
    let (result, log) = run(Script::full(), None, &out);
    let summary = result.unwrap();
    assert_no_leak(&summary);
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        format!("{EXPECTED_FULL}\n")
    );
    assert_eq!(
        std::fs::metadata(&out).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        1,
        "no temporary name is left"
    );
    assert_eq!(summary.get("fileBytes"), &Js::Num(bytes.len() as f64));
    assert_eq!(
        summary.get("fileSha256"),
        &Js::from(edu_academy_core::js::sha(
            &String::from_utf8(bytes).unwrap()
        ))
    );
    assert_eq!(
        log,
        [
            "authenticate",
            "discover",
            "execute",
            "journalState",
            "execute"
        ]
    );
}
#[test]
fn get_record_returns_the_value_and_writes_nothing_over_one_session_for_many_reads() {
    let dir = private("value");
    let (mut source, log) = source(Script::full());
    let first = live_driver::get_record(&mut source, &scope("77001"), None).unwrap();
    let second = live_driver::get_record(&mut source, &scope("77001"), Some(&NAME.into())).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        edu_academy_core::workflow::canonical(&first).unwrap(),
        EXPECTED_FULL
    );
    assert_nothing_left(&dir);
    // one authentication serves both reads
    assert_eq!(
        log.borrow()
            .iter()
            .filter(|op| *op == "authenticate")
            .count(),
        1
    );
}
#[test]
fn get_by_name_writes_the_same_file_after_one_search() {
    let dir = private("by-name");
    let out = dir.join("record.json");
    let (result, log) = run(Script::full(), Some(NAME), &out);
    assert_no_leak(&result.unwrap());
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        format!("{EXPECTED_FULL}\n")
    );
    assert_eq!(
        log,
        [
            "authenticate",
            "searchStudentsByName",
            "discover",
            "execute",
            "journalState",
            "execute"
        ]
    );
}
#[test]
fn get_writes_empty_fields_as_known_empty_and_unobserved_as_unknown() {
    let dir = private("empty");
    let out = dir.join("record.json");
    let mut script = Script::full();
    script.current = vec![
        ("progress", ""),
        ("homework", ""),
        ("lessonMemo", ""),
        ("attendance", ""),
    ];
    script.previous = vec![
        ("progress", ""),
        ("homework", ""),
        ("lessonMemo", ""),
        ("attendance", ""),
    ];
    script.assessments = (Js::Null, Js::Null);
    let (result, _) = run(script, None, &out);
    let summary = result.unwrap();
    assert_eq!(summary.get("recordFieldsKnown"), &Js::Num(4.0));
    assert_eq!(summary.get("recordFieldsUnknown"), &Js::Num(2.0));
    assert_eq!(summary.get("previousFieldsKnown"), &Js::Num(4.0));
    assert_eq!(summary.get("previousFieldsUnknown"), &Js::Num(2.0));
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        format!("{EXPECTED_EMPTY}\n")
    );
}
#[test]
fn a_record_without_a_previous_record_is_refused_by_the_source_layer_and_writes_nothing() {
    // The source layer has no typed "no previous record": the journal state needs the
    // previous tuple, and the previous page must list exactly one row.
    for (state_error, previous_rows, expected) in [
        (
            Some("journal_previous_tuple_ambiguous"),
            1,
            "journal_previous_tuple_ambiguous",
        ),
        (None, 0, "native_dailyrecord_previous_unverified"),
        (None, 2, "native_dailyrecord_previous_unverified"),
    ] {
        let dir = private("no-previous");
        let mut script = Script::full();
        script.state_error = state_error;
        script.previous_rows = previous_rows;
        let (result, _) = run(script, None, &dir.join("record.json"));
        assert_eq!(code(result), expected);
        assert_nothing_left(&dir);
    }
}
#[test]
fn name_refusals_precede_the_day_read_and_never_print_an_id() {
    for (pairs, expected) in [
        (vec![], "student_name_not_found"),
        (
            vec![("77001", NAME), ("77003", NAME)],
            "student_name_ambiguous",
        ),
        (vec![("77009", NAME)], "student_name_not_admitted"),
    ] {
        let dir = private("name-refused");
        let mut script = Script::full();
        script.pairs = pairs;
        let (result, log) = run(script, Some(NAME), &dir.join("record.json"));
        let fault = result.err().unwrap();
        assert_eq!(fault.message, expected);
        assert!(
            !format!("{fault:?}").contains("7700"),
            "an id escaped in the refusal"
        );
        assert_eq!(
            log,
            ["authenticate", "searchStudentsByName"],
            "no day read after a name refusal"
        );
        assert_nothing_left(&dir);
    }
    let launch = launch();
    assert_eq!(
        live_driver::launch_student_of_name(&launch, &NAME.into()).unwrap(),
        Js::from("77001")
    );
    assert_eq!(
        live_driver::launch_student_of_name(&launch, &"Unlisted Fictional".into())
            .err()
            .unwrap()
            .message,
        "student_name_not_admitted"
    );
    let mut twice = launch.clone();
    let mut students = twice.get("students").array().to_vec();
    students[1].set("displayName", NAME.into());
    twice.set("students", Js::Arr(students));
    assert_eq!(
        live_driver::launch_student_of_name(&twice, &NAME.into())
            .err()
            .unwrap()
            .message,
        "student_name_ambiguous"
    );
}
#[test]
fn out_refusals_write_nothing_and_ask_for_no_read() {
    use std::os::unix::fs::PermissionsExt;
    let dir = private("out");
    let existing = dir.join("exists.json");
    std::fs::write(&existing, b"keep").unwrap();
    assert_eq!(
        live_driver::check_out(&existing).err().unwrap().message,
        "get_out_exists"
    );
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep");
    assert_eq!(
        live_driver::check_out(Path::new("relative.json"))
            .err()
            .unwrap()
            .message,
        "invalid_input"
    );
    assert_eq!(
        live_driver::check_out(&dir.join("../escape.json"))
            .err()
            .unwrap()
            .message,
        "invalid_input"
    );
    let loose = private("out-loose");
    std::fs::set_permissions(&loose, std::fs::Permissions::from_mode(0o750)).unwrap();
    assert_eq!(
        live_driver::check_out(&loose.join("record.json"))
            .err()
            .unwrap()
            .message,
        "get_out_directory_refused"
    );
    assert_eq!(
        live_driver::check_out(&dir.join("absent-directory/record.json"))
            .err()
            .unwrap()
            .message,
        "get_out_directory_refused"
    );
    let link = dir.join("link");
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    assert_eq!(
        live_driver::check_out(&link.join("record.json"))
            .err()
            .unwrap()
            .message,
        "get_out_directory_refused"
    );
}
#[test]
fn a_failed_previous_read_leaves_no_file_and_the_port_saw_reads_only() {
    let dir = private("failed-previous");
    let mut script = Script::full();
    script.previous_rows = 0;
    let (result, log) = run(script, None, &dir.join("record.json"));
    assert_eq!(code(result), "native_dailyrecord_previous_unverified");
    assert_nothing_left(&dir);
    assert!(
        log.iter().all(|op| [
            "authenticate",
            "discover",
            "execute",
            "journalState",
            "searchStudentsByName"
        ]
        .contains(&op.as_str())),
        "a source operation other than a read: {log:?}"
    );
}

// ---- the same mode over the real Client and a scripted transport ----
struct Finite {
    replies: VecDeque<(String, u16, String)>,
    seen: Rc<RefCell<Vec<Request>>>,
}
impl Transport for Finite {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        self.seen.borrow_mut().push(r.clone());
        let (path, status, body) = self
            .replies
            .pop_front()
            .expect("finite scripted request budget");
        assert_eq!(r.path, path);
        Ok(Response {
            status,
            headers: vec![],
            bytes: body.into_bytes(),
            url: format!("{ORIGIN}{path}"),
            stalled: false,
        })
    }
}
fn login() -> Vec<(String, u16, String)> {
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
    .map(|n| format!("<input name=\"{n}\">"))
    .collect::<String>();
    vec![
        ("/c_login.jsp".into(), 200, format!(r#"<form id="loginForm" method="post">{fields}</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>"#)),
        ("/servlet/controller.login.LoginServlet".into(), 200, "fictional accepted login".into()),
        ("/servlet/controller.coursemanage.CourseManageServlet".into(), 200,
            r#"<form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="fictional"></form><a>로그아웃</a>"#.into()),
    ]
}
fn credentials() -> Result<Credentials> {
    Ok(Credentials::new(
        "fictional-user".into(),
        "fictional-password".into(),
    ))
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn owner_unqualified_main(fields: &Js) -> String {
    // Own synthetic document, with exact source-observed signatures/column roles.
    // Inert definitions intentionally do not hash to the qualified private source.
    let mut html = format!(
        r#"
<input name="std_date" value="2040-01-04"><input name="teacher_pri_no" value="99001">
<select name="grp_seq"><option value="3" selected>Fictional group</option></select>
<table><thead><tr><th rowspan="2">이름</th><th rowspan="2">과정</th><th rowspan="2">지속사항</th><th colspan="4">지난진도</th><th colspan="6">일괄 발송 금일진도</th></tr>
<tr><th>지난수업일</th><th>진도</th><th>숙제</th><th>메모</th><th></th><th>금일출석</th><th>일일테스트</th><th>진도</th><th>숙제</th><th>메모</th></tr></thead>
<tbody><tr><td><a onclick="onOpenDaysMain('77001')">Fictional Learner</a></td><td>Fictional course</td>
<td><textarea onfocusout="onFoStuMemo('0','77001','5')">{}</textarea></td>
<td>2040-01-02</td><td>prior progress</td><td><a onclick="openHomeWorkRate(12,8,3)">prior assessment</a></td><td>prior memo</td><td></td>
<td><select onchange="onAttn('0','5','77001','13','8')"><option value="{}" selected>Fictional attendance</option></select></td>
<td><a onclick="setDailyTest(13,8,0)">DT</a><input name="h_daily_test" value="{}"><input name="h_homework_rate" value="{}"></td>
<td><textarea onfocusout="onFoPrg('0','5','77001','13','8')">{}</textarea></td>
<td><textarea onfocusout="onFoHw('0','5','77001','13','8')">{}</textarea></td>
<td><input onfocusout="onFoMm('0','5','77001','13','8')" value="{}"></td></tr></tbody></table>
<script>
function onAttn(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtAttn'; }}
function onFoPrg(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtPrg'; }}
function onFoHw(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtHw'; }}
function onFoMm(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtMemo'; }}
function onFoStuMemo(i,stu_pri_no,course_seq){{ return 'inert'; }}
function onOpenDaysMain(student_pri_no){{ return 'inert'; }}
function onDailyReport(report_seq,stu_pri_no,record_seq,cm_seq){{ return 'inert'; }}
function setDailyTest(record_seq,cm_seq,count){{ return 'reqCmd=SetDailyTest'; }}
function openHomeWorkRate(record_seq,cm_seq,count){{ return 'reqCmd=SetHomeWorkRate'; }}
function reallength(value){{ return value.length; }}

var g_param = "";
var daily_test_radio = 'inert';
</script>
"#,
        escape(&string(fields.get("persistentMemo"))),
        escape(&string(fields.get("attendance"))),
        escape(&string(fields.get("dailyTestAssessment"))),
        escape(&string(fields.get("previousHomeworkAssessment"))),
        escape(&string(fields.get("progress"))),
        escape(&string(fields.get("homework"))),
        escape(&string(fields.get("lessonMemo")))
    );
    let homework = [
        "미확인",
        "미실시",
        "숙제 수행도 C",
        "숙제 수행도 B",
        "숙제 수행도 B⁺",
        "숙제 수행도 A",
        "숙제 수행도 A⁺",
    ];
    let daily = [
        "미확인",
        "미실시",
        "일일 테스트 10점",
        "일일 테스트 20점",
        "일일 테스트 30점",
        "일일 테스트 40점",
        "일일 테스트 50점",
        "일일 테스트 60점",
        "일일 테스트 70점",
        "일일 테스트 80점",
        "일일 테스트 90점",
        "일일 테스트 100점",
    ];
    for (name, labels) in [
        ("homework_radio", homework.as_slice()),
        ("daily_test_radio", daily.as_slice()),
    ] {
        // The original collector walks DOM order. JS Object.entries enumerates
        // nonnegative integer keys before the string key '-1'.
        for i in (1..labels.len()).chain(std::iter::once(0)) {
            html.push_str(&format!(
                "<table><tr><td><input name=\"{name}\" value=\"{}\">{}</td></tr></table>",
                i as i32 - 1,
                labels[i]
            ));
        }
    }
    html
}

fn configure_account_lock(root: &Path) {
    let lock = root.join("shared.lock");
    std::fs::write(&lock, b"").unwrap();
    let config = root.join("account-lock.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"academy.account-lock/1", "path":lock
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(config, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
}
fn principal() -> Js {
    obj(&[
        ("teacherLabel", "Fictional Teacher".into()),
        ("teacherKey", "99001".into()),
        ("collegeKey", "88001".into()),
    ])
}
#[test]
fn get_over_the_real_client_sends_reads_only_and_one_login() {
    let dir = private("client");
    let root = std::fs::canonicalize(&dir).unwrap();
    configure_account_lock(&root);
    let out_dir = private("client-out");
    let out = out_dir.join("record.json");
    let owner_root = Rc::new(OwnerRoot::open(&root).unwrap());
    let html = owner_unqualified_main(&obj(&[
        ("persistentMemo", "Fictional memo".into()),
        ("attendance", "Y".into()),
        ("dailyTestAssessment", "0".into()),
        ("previousHomeworkAssessment", "3".into()),
        ("progress", "Before".into()),
        ("homework", "Homework".into()),
        ("lessonMemo", "Memo".into()),
    ]));
    let mut replies = login();
    replies.push((
        "/servlet/controller.dailyzerotest.DailyZeroTestServlet".into(),
        200,
        format!(r#"testResultList:JSON.parse('[{{"userName":"{NAME}","priNo":"77001"}}]')"#),
    ));
    // discover, current execute + binding, journal execute + binding, wire inspection.
    replies.push((MAIN.into(), 200, html.clone()));
    for _ in 0..2 {
        replies.push((MAIN.into(), 200, html.clone()));
        replies.push((
            SELECTED.into(),
            200,
            "<root><record>!2040-01-04!5!!Fictional Learner!!!!!3!13!77001!8</record></root>"
                .into(),
        ));
    }
    replies.push((MAIN.into(), 200, html));
    let seen = Rc::new(RefCell::new(vec![]));
    let transport = Finite {
        replies: replies.into(),
        seen: Rc::clone(&seen),
    };
    let owner = Owner::leased(
        owner_root,
        principal(),
        transport,
        launch(),
        None,
        credentials,
        &Clock,
    )
    .unwrap();
    let mut source = owner.source();
    let fault = live_driver::get(&mut source, &scope("77001"), Some(&NAME.into()), &out)
        .err()
        .unwrap();
    // The scripted page cannot reproduce the qualified wire digest: the read stops there.
    assert_eq!(fault.message, "journal_mutation_wire_not_qualified");
    assert!(!out.exists());
    assert_nothing_left(&out_dir);
    let seen = seen.borrow();
    assert_eq!(
        seen.len(),
        10,
        "login 3, name search 1, day read up to the wire page 6"
    );
    assert_eq!(seen.iter().filter(|r| r.path == "/c_login.jsp").count(), 1);
    assert!(
        seen.iter().all(|r| {
            r.path != "/servlet/controller.cct.common.CourseCommonServlet"
                && !r.query.iter().chain(&r.body).any(|(k, v)| {
                    k == "reqCmd"
                        && [
                            "udtAttn",
                            "udtPrg",
                            "udtHw",
                            "udtMemo",
                            "SetDailyTest",
                            "SetHomeWorkRate",
                        ]
                        .contains(&v.as_str())
                })
        }),
        "a write-shaped request"
    );
}

// ---- the built binary: every refusal that precedes the first request ----
fn binary_get_case(case: &str, expected: &str) {
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
    };
    let root = private(&format!("binary-{case}"));
    let out_dir = root.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::set_permissions(&out_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let transport = serde_json::json!({"schemaVersion":"academy.transport-admission/1", "ownerId":"synthetic-owner", "teacherId":"99001", "teacherLabel":"Fictional Teacher", "collegeId":"88001", "allowedFrom":"2040-01-01", "allowedThrough":"2040-01-31", "studentIds":["77001","77002"], "sourceNamespaces":["academy.lms.dayrecord"], "operations":[{"id":"inert", "sourceNamespace":"academy.lms.dayrecord", "method":"GET", "path":"/synthetic/inert", "effect":false, "query":[], "body":[]}]});
    let mut launch: serde_json::Value =
        serde_json::from_str(&edu_academy_core::workflow::canonical(&launch()).unwrap()).unwrap();
    let mut request = serde_json::json!({
        "mode":"get",
        "scope":{"sourceNamespace":"academy.lms.dayrecord","teacherId":"99001","lessonDate":"2040-01-04"},
        "ownerRoot": root.to_str().unwrap(),
        "out": out_dir.join("record.json").to_str().unwrap(),
    });
    match case {
        "id" | "reaches-credential-by-id" => request["scope"]["studentId"] = "77001".into(),
        "name" | "reaches-credential-by-name" => request["studentName"] = NAME.into(),
        "relative-out" => {
            request["scope"]["studentId"] = "77001".into();
            request["out"] = "relative.json".into();
        }
        "value" => {
            request["scope"]["studentId"] = "77001".into();
            request["value"] = "Canary value".into();
        }
        "field" => {
            request["scope"]["studentId"] = "77001".into();
            request["field"] = "attendance".into();
        }
        "both" => {
            request["scope"]["studentId"] = "77001".into();
            request["studentName"] = NAME.into();
        }
        "neither" => (),
        "existing-out" => {
            request["scope"]["studentId"] = "77001".into();
            std::fs::write(out_dir.join("record.json"), b"keep").unwrap();
        }
        "loose-directory" => {
            request["scope"]["studentId"] = "77001".into();
            std::fs::set_permissions(&out_dir, std::fs::Permissions::from_mode(0o750)).unwrap();
        }
        "unknown-name" => request["studentName"] = "Unlisted Fictional".into(),
        "two-names" => {
            launch["students"][1]["displayName"] = NAME.into();
            request["studentName"] = NAME.into();
        }
        "id-outside" => request["scope"]["studentId"] = "77009".into(),
        "id-outside-transport" => {
            launch["students"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"studentId":"77003","displayName":"Third Fictional"}));
            request["scope"]["studentId"] = "77003".into();
        }
        _ => unreachable!(),
    }
    for (key, name, contents) in [
        ("admission", "transport.json", transport.to_string()),
        ("launchAdmission", "launch.json", launch.to_string()),
        (
            "credential",
            "credential.json",
            "synthetic_invalid_credential".into(),
        ),
    ] {
        std::fs::write(root.join(name), &contents).unwrap();
        std::fs::set_permissions(root.join(name), std::fs::Permissions::from_mode(0o600)).unwrap();
        request[key] = serde_json::json!({"relativePath":name,"sha256":hash(contents.as_bytes())});
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_academy-live-driver"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "{case}: refusal before any request"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["status"], "refused", "{case}");
    assert_eq!(value["code"], expected, "{case}: guard code");
    assert_eq!(
        value["stored"],
        serde_json::Value::Null,
        "{case}: no store is opened"
    );
    assert_eq!(stderr, format!("{expected}\n"));
    for private in [
        NAME,
        "Unlisted Fictional",
        "Fictional Teacher",
        "Canary value",
        "synthetic_invalid_credential",
        "credential.json",
        "77001",
        "77009",
    ] {
        assert!(
            !stdout.contains(private) && !stderr.contains(private),
            "{case}: private bytes escaped: {private}"
        );
    }
    let produced = out_dir.join("record.json");
    if case == "existing-out" {
        assert_eq!(std::fs::read(&produced).unwrap(), b"keep");
    } else {
        assert!(!produced.exists(), "{case}: a refusal writes no file");
    }
    assert!(!root.join("store").exists(), "{case}: no store is created");
}
#[test]
fn binary_get_refuses_before_the_first_request() {
    for (case, expected) in [
        ("relative-out", "invalid_input"),
        ("value", "invalid_input"),
        ("field", "invalid_input"),
        ("both", "invalid_input"),
        ("neither", "invalid_input"),
        ("existing-out", "get_out_exists"),
        ("loose-directory", "get_out_directory_refused"),
        ("unknown-name", "student_name_not_admitted"),
        ("two-names", "student_name_ambiguous"),
        ("id-outside", "launch_admission_refused"),
        ("id-outside-transport", "native_transport_scope_refused"),
        // Every check passed: only the credential (invalid here) stops the run.
        (
            "reaches-credential-by-id",
            "native_owner_credential_refused",
        ),
        (
            "reaches-credential-by-name",
            "native_owner_credential_refused",
        ),
    ] {
        binary_get_case(case, expected);
    }
}
