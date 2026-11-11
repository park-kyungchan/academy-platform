//! E11b synthetic adapter tests for `academy_prestudy_get` and
//! `academy_prestudy_report_get`. Invented names, keys, titles and pages only
//! (Invented Student 77001, Invented Teacher 99001, dates in 2040); no live
//! call, no credential, no real record. Ported vectors name their TS source:
//! `PRC` = `modules/academy-workbench/tools/academy-mcp/tests/prestudy-report-precision.test.ts`,
//! `PSG` = `.../tests/prestudy-section-get.test.ts`.
use edu_academy_mcp::{
    Js, encode, obj, parse,
    prestudy::{
        GET_FALLBACK, PrestudyAdmission, PrestudyCall, PrestudyDriver, REPORT_FALLBACK, TOOLS,
        prestudy_transport_limits, public_failure,
    },
    protocol::RpcSession,
    release::ReleaseGraph,
    stringify, validation,
};
use edu_academy_sources::{
    data::{Error, Result},
    http::{self, Response},
    owner_admission::{TransportAdmission, TransportScope},
    prestudy::{PrestudyPort, SessionPrestudyPort},
    student_course::{self, ReadPort, read_contracts},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    rc::Rc,
};

const GET: &str = "academy_prestudy_get";
const REPORT: &str = "academy_prestudy_report_get";
const NOW: &str = "2040-01-16T09:00:00.000Z";
const NOW_REPORT: &str = "2040-01-02T03:04:00.000Z";
const COVERAGE_LISTS: &str =
    "exact_student_rows_on_waiting_completed_and_one_observed_summary_page_not_all_history";

fn js(s: &str) -> Js {
    parse(s.as_bytes()).unwrap()
}
fn text_of(v: &Js) -> String {
    String::from_utf16(v.units()).unwrap()
}
fn json_of(v: &Js) -> String {
    stringify(v).unwrap()
}
fn keys_of(v: &Js) -> Vec<String> {
    v.props()
        .iter()
        .map(|(k, _)| String::from_utf16_lossy(k))
        .collect()
}

// ------------------------------------------------------------ page builders --
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', "<br>")
}
/// `(student label, unit, date, status, key)`
type ListRow<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str);
fn list_page(cmd: &str, rows: &[ListRow]) -> String {
    let mut html = format!(
        r#"<html><form name="form1"><input type="hidden" name="reqCmd" value="{cmd}"><input type="hidden" name="g_clg_no" value="88001"><input type="hidden" name="g_mem_type" value="1"><input type="hidden" name="g_pri_no" value="99001"><input type="text" name="student_name" value=""></form><table><tr><th>학생이름</th><th>단원명</th><th>날짜</th><th>영상 상태</th><th></th></tr>"#
    );
    for (name, unit, date, status, key) in rows {
        html += &format!(
            r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><button onclick="prestudy_main({key});">예습 검사</button></td></tr>"#,
            esc(name),
            esc(unit),
            esc(date),
            esc(status)
        );
    }
    html + "</table></html>"
}
fn summary_page(rows: &[(&str, &str, &str, &str, &str)]) -> String {
    let mut html = String::from(
        "<table><tr><th>학생</th><th>단원명</th><th>날짜</th><th>Ai총점</th><th>별점</th><th>action</th></tr>",
    );
    for (name, unit, date, ai, rating) in rows {
        html += &format!(
            r#"<tr><td>{}</td><td>{}</td><td>{}</td><td><span>{}</span> <button onclick="openReport(777);">보고서</button></td><td>{}</td><td><button onclick="playVideo('https://cdn.example/prestudy/1_77001_1_prestudy.mp4')">재생</button></td></tr>"#,
            esc(name),
            esc(unit),
            esc(date),
            esc(ai),
            esc(rating)
        );
    }
    html + "</table>"
}
fn detail_page(
    student: &str,
    key: &str,
    unit: Option<&str>,
    date: Option<&str>,
    link: Option<&str>,
) -> String {
    let mut html = format!(
        r#"<html><form name="form1"><input type="hidden" name="prestudy_key" value="{key}"><input type="hidden" name="student_pri_no" value="{student}">"#
    );
    if let Some(unit) = unit {
        html += &format!(
            r#"<input type="hidden" name="sunit_name" value="{}">"#,
            esc(unit)
        );
    }
    if let Some(date) = date {
        html += &format!(
            r#"<input type="hidden" name="create_datetime" value="{}">"#,
            esc(date)
        );
    }
    html += r#"<input type="hidden" name="encoding_result" value="success"></form>"#;
    if let Some(link) = link {
        html += &format!(r#"<button onclick="viewAiReport('{link}');">Ai 보고서 보기</button>"#);
    }
    html + "</html>"
}
/// `(title, subtitle, blocks[(heading, text)])`; `\n` in text becomes `<br>`.
type Sec<'a> = (&'a str, Option<&'a str>, Vec<(Option<&'a str>, String)>);
fn report_page(sections: &[Sec], scores: &str) -> String {
    let mut html = String::from("<html><body>");
    for (title, subtitle, blocks) in sections {
        html += &format!(
            r#"<div class="max-w-5xl mx-auto bg-white rounded-2xl shadow-lg"><div class="p-4"><h1>{}</h1></div><div class="p-4">"#,
            esc(title)
        );
        if let Some(subtitle) = subtitle {
            html += &format!("<h2>{}</h2>", esc(subtitle));
        }
        for (heading, text) in blocks {
            if let Some(heading) = heading {
                html += &format!("<h3>{}</h3>", esc(heading));
            }
            html += &format!(r#"<div class="text-sm">{}</div>"#, esc(text));
        }
        html += "</div></div>";
    }
    html + &format!("<script>const scores = {scores};</script></body></html>")
}
/// The synthetic report of the TS precision tests (`PRC:11-16`), plus a
/// subtitle and an astral character so byte counts differ from char counts.
fn base_sections() -> Vec<Sec<'static>> {
    vec![
        (
            "개념 이해",
            Some("개념을 정확히 이해하고 설명하는가?"),
            vec![(Some("reason"), "πr² = 50%\n−2 < 0".into())],
        ),
        (
            "논리적 설명",
            None,
            vec![(None, "An invented explanation".into())],
        ),
        (
            "총평",
            None,
            vec![(None, "An invented conclusion 😀".into())],
        ),
    ]
}
const BASE_SCORES: &str = "{ m1: 77, m2: 80, m3: 0.0000001 }";

// ------------------------------------------------------------------- fake port --
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    operation: String,
    args: Vec<(String, String)>,
    teacher: String,
    student: String,
    date: String,
}
struct Fake {
    admission: TransportAdmission,
    pages: HashMap<&'static str, String>,
    seen: Rc<RefCell<Vec<Seen>>>,
    fault: Option<&'static str>,
}
impl ReadPort for Fake {
    fn transport_admission(&self) -> &TransportAdmission {
        &self.admission
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<Response> {
        self.seen.borrow_mut().push(Seen {
            operation: operation.into(),
            args: arguments.to_vec(),
            teacher: scope.teacher_id.clone(),
            student: scope.student_id.clone(),
            date: scope.date.clone(),
        });
        if let Some(code) = self.fault {
            return Err(Error::refusal(code));
        }
        let body = self
            .pages
            .get(operation)
            .unwrap_or_else(|| panic!("unexpected synthetic read {operation}"));
        Ok(Response {
            status: 200,
            headers: vec![],
            bytes: body.as_bytes().to_vec(),
            url: http::ORIGIN.into(),
            stalled: false,
        })
    }
}
fn transport_admission(operations: bool) -> TransportAdmission {
    TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "fictional-owner".into(),
        teacher_id: "99001".into(),
        teacher_label: "Invented Teacher".into(),
        college_id: "88001".into(),
        allowed_from: "2040-01-01".into(),
        allowed_through: "2040-12-31".into(),
        student_ids: vec!["77001".into(), "77002".into()],
        source_namespaces: vec![student_course::COURSE_NS.into()],
        operations: if operations { read_contracts() } else { vec![] },
    }
}
fn admission() -> PrestudyAdmission {
    PrestudyAdmission {
        owner_id: "fictional-owner".into(),
        teacher_label: "Invented Teacher".into(),
        teacher_key: "99001".into(),
        college_key: "88001".into(),
        date: "2040-01-16".into(),
        students: vec![
            ("77001".into(), "Invented Student".into()),
            ("77002".into(), "Invented Other".into()),
        ],
    }
}
fn routing_admission() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("allowedFrom", "2040-01-04".into()),
        ("allowedThrough", "2040-01-04".into()),
    ])
}
type Capture = Rc<RefCell<Vec<Seen>>>;
fn default_pages() -> HashMap<&'static str, String> {
    let waiting = list_page(
        "WebUnPreStudy",
        &[
            (
                "Invented Student",
                "9-1 Fictional Unit (기본)(ver. 1)",
                "40.01.02 03:04",
                "OK",
                "12345",
            ),
            (
                "Other Student",
                "9-2 Other Unit(ver. 1)",
                "40.01.02 04:05",
                "OK",
                "12346",
            ),
        ],
    );
    let completed = list_page("WebCtPreStudy", &[]);
    HashMap::from([
        ("prestudy.waiting.shell", waiting.clone()),
        ("prestudy.waiting.search", waiting),
        ("prestudy.completed.shell", completed.clone()),
        ("prestudy.completed.search", completed),
        (
            "prestudy.summary",
            summary_page(&[(
                "Invented Student(초5)",
                "9-1 Fictional Unit(ver. 1)",
                "40.01.02 03:04",
                "77점",
                "-",
            )]),
        ),
        (
            "prestudy.detail",
            detail_page(
                "77001",
                "12345",
                Some("9-1 Invented unit"),
                Some("40.01.02 03:04"),
                Some("55501"),
            ),
        ),
        (
            "prestudy.report",
            report_page(&base_sections(), BASE_SCORES),
        ),
    ])
}
struct Rig {
    driver: PrestudyDriver<Fake>,
    seen: Capture,
}
fn rig_with(
    pages: HashMap<&'static str, String>,
    operations: bool,
    fault: Option<&'static str>,
) -> Rig {
    let seen: Capture = Rc::new(RefCell::new(vec![]));
    let port = Fake {
        admission: transport_admission(operations),
        pages,
        seen: seen.clone(),
        fault,
    };
    Rig {
        driver: PrestudyDriver::new(port, admission(), routing_admission()).unwrap(),
        seen,
    }
}
fn rig() -> Rig {
    rig_with(default_pages(), true, None)
}
fn with_pages(edit: impl FnOnce(&mut HashMap<&'static str, String>)) -> Rig {
    let mut pages = default_pages();
    edit(&mut pages);
    rig_with(pages, true, None)
}
impl Rig {
    fn ops(&self) -> Vec<String> {
        self.seen
            .borrow()
            .iter()
            .map(|s| s.operation.clone())
            .collect()
    }
    fn reads(&self) -> usize {
        self.seen.borrow().len()
    }
}

fn get_input() -> Js {
    js(
        r#"{"displayName":"Invented Student","purpose":"Read fictional prestudy state","currentRequestRef":"synthetic-prestudy-request"}"#,
    )
}
fn report_input() -> Js {
    js(
        r#"{"displayName":"Invented Student","prestudyKey":"12345","purpose":"Review invented report","currentRequestRef":"synthetic-report-precision"}"#,
    )
}
fn report_with(extra: &[(&str, Js)]) -> Js {
    let mut input = report_input();
    for (k, v) in extra {
        input.set(k, v.clone());
    }
    input
}
fn titles(list: &[&str]) -> Js {
    Js::Arr(list.iter().map(|t| (*t).into()).collect())
}
fn call_report(rig: &mut Rig, extra: &[(&str, Js)]) -> Js {
    rig.driver
        .call_at(REPORT, &report_with(extra), NOW_REPORT)
        .unwrap()
}
fn reason(out: &Js) -> String {
    out.get("reasonCode").code_units()
}
fn assert_unavailable(out: &Js, code: &str, category: &str) {
    assert_eq!(
        out.get("status").code_units(),
        "unavailable",
        "{}",
        json_of(out)
    );
    assert_eq!(reason(out), code, "{}", json_of(out));
    let failure = out.get("failure");
    assert_eq!(failure.get("code").code_units(), code);
    assert_eq!(failure.get("category").code_units(), category, "{code}");
    assert_eq!(failure.get("automaticRetryAllowed"), &Js::Bool(false));
    assert!(!out.has("view") && !out.has("sections") && !out.has("waiting"));
}
/// The code is part of the registry entry's `errorCodes` (step 5).
fn listed(tool: &str, code: &str) {
    let v2: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/tool-registry-v2.json")).unwrap();
    let entry = v2["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == tool)
        .unwrap();
    assert!(
        entry["errorCodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == code),
        "{tool} errorCodes lack {code}"
    );
}

// ============================================================ get: R08-1/4 ===
// PSG:11-18 (projection, teacher judgment separate, empty completed list is
// observed) and the byte-equal pin of the TS key order.
#[test]
fn get_projects_all_surfaces_with_teacher_judgment_separate_byte_equal_to_ts() {
    let mut r = rig();
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_eq!(
        json_of(&out),
        r##"{"purpose":"Read fictional prestudy state","currentRequestRef":"synthetic-prestudy-request","displayName":"Invented Student","academyWrites":0,"parentSends":0,"automaticRetryAllowed":false,"sourceContentIsUntrustedData":true,"execution":"read_only","status":"observed","student":{"studentId":"77001","displayName":"Invented Student"},"waiting":[{"unitLabel":"9-1 Fictional Unit (기본)(ver. 1)","dateLiteral":"40.01.02 03:04","statusLiteral":"OK","prestudyKey":"12345"}],"completed":[],"summary":[{"unitLabel":"9-1 Fictional Unit(ver. 1)","dateLiteral":"40.01.02 03:04","aiScoreLiteral":"77점 보고서","ratingLiteral":"-","reportAvailable":true,"videoAvailable":true}],"teacherJudgment":null,"observedAt":"2040-01-16T09:00:00.000Z","coverage":"exact_student_rows_on_waiting_completed_and_one_observed_summary_page_not_all_history"}"##
    );
    assert_eq!(out.get("status").code_units(), "observed");
    assert_eq!(out.get("teacherJudgment"), &Js::Null);
    assert_eq!(out.get("academyWrites"), &Js::Num(0.0));
    assert_eq!(out.get("parentSends"), &Js::Num(0.0));
    // Exact-student filter: the other student's row is not returned.
    assert_eq!(out.get("waiting").array().len(), 1);
    assert_eq!(
        out.get("waiting").array()[0]
            .get("prestudyKey")
            .code_units(),
        "12345"
    );
    // No video URL is retained anywhere.
    assert!(!json_of(&out).contains("cdn.example"));
    assert!(!json_of(&out).contains("playVideo"));
    assert_eq!(out.get("coverage").code_units(), COVERAGE_LISTS);
    // Waiting shell + form POST, completed shell + form POST, then summary.
    assert_eq!(
        r.ops(),
        [
            "prestudy.waiting.shell",
            "prestudy.waiting.search",
            "prestudy.completed.shell",
            "prestudy.completed.search",
            "prestudy.summary"
        ]
    );
    assert_eq!(r.driver.source_requests(), 5);
    listed(GET, "tool_input_invalid");
}

// R08-4 / PSG:11-18: an observed empty surface stays observed and the summary
// is still read; unavailable is a different status with a code.
#[test]
fn get_observed_empty_is_distinct_from_unavailable() {
    let mut r = with_pages(|p| {
        let empty = list_page("WebUnPreStudy", &[]);
        p.insert("prestudy.waiting.shell", empty.clone());
        p.insert("prestudy.waiting.search", empty);
        p.insert("prestudy.summary", summary_page(&[]));
    });
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_eq!(out.get("status").code_units(), "observed");
    for k in ["waiting", "completed", "summary"] {
        assert_eq!(out.get(k).array().len(), 0, "{k}");
        assert!(matches!(out.get(k), Js::Arr(_)));
    }
    assert!(!out.has("failure") && !out.has("reasonCode"));
    assert_eq!(
        r.reads(),
        5,
        "the summary is read even when both lists are empty"
    );
    // An unavailable source is `unavailable`, never an empty observation.
    let mut r = rig_with(default_pages(), true, Some("native_transport_cancelled"));
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_eq!(out.get("status").code_units(), "unavailable");
    assert!(!out.has("waiting") && !out.has("summary"));
}

// Admission (`native-dailyrecord-backend.ts:210-211`): refuse before any read.
#[test]
fn get_and_report_refuse_an_unadmitted_name_before_any_read() {
    for tool in [GET, REPORT] {
        let mut r = rig();
        let mut input = if tool == GET {
            get_input()
        } else {
            report_input()
        };
        input.set("displayName", "Invented Stranger".into());
        let out = r.driver.call_at(tool, &input, NOW).unwrap();
        assert_unavailable(&out, "prestudy_student_not_admitted", "policy_denial");
        assert_eq!(r.reads(), 0, "{tool}");
        listed(tool, "prestudy_student_not_admitted");
        // The substring of an admitted name is not an admitted name.
        input.set("displayName", "Invented".into());
        assert_eq!(
            reason(&r.driver.call_at(tool, &input, NOW).unwrap()),
            "prestudy_student_not_admitted"
        );
        // A name that is not well-formed UTF-16 matches nothing.
        input.set("displayName", Js::Str(vec![0xd800]));
        assert_eq!(
            reason(&r.driver.call_at(tool, &input, NOW).unwrap()),
            "prestudy_student_not_admitted"
        );
        assert_eq!(r.reads(), 0);
    }
}

// Zod `.trim()` (`dailyrecord.ts:79`): the trimmed name is looked up and echoed.
#[test]
fn display_name_is_trimmed_like_zod_before_admission_and_echo() {
    let mut r = rig();
    let mut input = get_input();
    input.set("displayName", "\u{3000} Invented Student\u{feff}\n".into());
    let out = r.driver.call_at(GET, &input, NOW).unwrap();
    assert_eq!(out.get("status").code_units(), "observed");
    assert_eq!(text_of(out.get("displayName")), "Invented Student");
    assert_eq!(
        text_of(out.get("student").get("displayName")),
        "Invented Student"
    );
    // NEL (U+0085) is not JavaScript whitespace: it is not trimmed.
    input.set("displayName", "Invented Student\u{85}".into());
    assert_eq!(
        reason(&r.driver.call_at(GET, &input, NOW).unwrap()),
        "prestudy_student_not_admitted"
    );
}

// Step 2: the port that binds the teacher. Every request carries the admitted
// teacher key and the admitted student's own key in its scope; the session
// port reports the teacher key as its principal.
#[test]
fn source_reads_use_the_session_port_bound_to_teacher_and_student() {
    let mut r = rig();
    r.driver.call_at(GET, &get_input(), NOW).unwrap();
    r.driver
        .call_at(REPORT, &report_input(), NOW_REPORT)
        .unwrap();
    let seen = r.seen.borrow().clone();
    assert_eq!(seen.len(), 7);
    for s in &seen {
        assert_eq!(s.teacher, "99001");
        assert_eq!(s.student, "77001");
        assert_eq!(s.date, "2040-01-16");
    }
    // The detail request carries the exact key, the report request the analysis key.
    assert_eq!(seen[5].operation, "prestudy.detail");
    assert_eq!(
        seen[5].args,
        [("prestudy_key".to_string(), "12345".to_string())]
    );
    assert_eq!(seen[6].operation, "prestudy.report");
    assert_eq!(
        seen[6].args,
        [("analysis_key".to_string(), "55501".to_string())]
    );
    // The other admitted student is read under its own key.
    let mut other = get_input();
    other.set("displayName", "Invented Other".into());
    r.driver.call_at(GET, &other, NOW).unwrap();
    assert_eq!(r.seen.borrow().last().unwrap().student, "77002");
    // The port type the adapter builds exposes the principal key.
    let mut fake = Fake {
        admission: transport_admission(true),
        pages: HashMap::new(),
        seen: Rc::new(RefCell::new(vec![])),
        fault: None,
    };
    let scope = TransportScope {
        owner_id: "fictional-owner".into(),
        source_namespace: student_course::COURSE_NS.into(),
        teacher_id: "99001".into(),
        student_id: "77001".into(),
        date: "2040-01-16".into(),
    };
    assert_eq!(
        SessionPrestudyPort::new(&mut fake, scope).principal_key(),
        Some("99001")
    );
}

// A driver cannot be built over a transport admission of another owner,
// teacher or student set (as `Owner::validate_transport`).
#[test]
fn driver_refuses_a_transport_admission_that_does_not_match() {
    for edit in [
        (|a: &mut TransportAdmission| a.teacher_id = "99002".into()) as fn(&mut TransportAdmission),
        |a| a.owner_id = "other-owner".into(),
        |a| a.teacher_label = "Other Teacher".into(),
        |a| a.college_id = "88002".into(),
        |a| {
            a.student_ids.pop();
        },
        |a| a.student_ids[1] = "77003".into(),
    ] {
        let mut policy = transport_admission(true);
        edit(&mut policy);
        let port = Fake {
            admission: policy,
            pages: default_pages(),
            seen: Rc::new(RefCell::new(vec![])),
            fault: None,
        };
        let err = PrestudyDriver::new(port, admission(), routing_admission())
            .err()
            .expect("mismatch refused");
        assert_eq!(err.code, "student_read_owner_transport_conflict");
    }
}

// ============================================================ input rules ====
// Step 1: Zod rules the JSON Schema cannot state, counted in UTF-16 units.
fn refused(tool: &str, input: &Js) {
    let mut r = rig();
    match r.driver.call_at(tool, input, NOW) {
        Err(fault) => assert_eq!(fault.message, "tool_input_invalid"),
        Ok(out) => panic!("accepted {}: {}", json_of(input), json_of(&out)),
    }
    assert_eq!(r.reads(), 0, "refused before any read");
}
#[test]
fn get_input_rules_match_the_zod_schema() {
    for (key, value) in [
        ("displayName", Js::from("  \t\u{2028} ")),
        ("displayName", Js::from("")),
        ("displayName", Js::Num(5.0)),
        ("displayName", Js::from("a".repeat(201))),
        // 101 astral characters are 101 code points (the JSON Schema passes
        // them) but 202 UTF-16 units: Zod refuses.
        ("displayName", Js::from("😀".repeat(101))),
        ("purpose", Js::from("")),
        ("purpose", Js::from(" \n ")),
        ("purpose", Js::from("a".repeat(257))),
        ("purpose", Js::from("😀".repeat(129))),
        ("purpose", Js::from("a\u{1}b")),
        ("purpose", Js::from("a\u{7f}")),
        ("purpose", Js::from("a\u{b}")),
        ("purpose", Js::from("a\u{0}")),
        ("purpose", Js::Null),
        ("currentRequestRef", Js::from("x\u{1f}")),
        ("currentRequestRef", Js::from("r".repeat(257))),
        ("currentRequestRef", Js::from("\u{feff}")),
    ] {
        let mut input = get_input();
        input.set(key, value);
        refused(GET, &input);
    }
    // Missing and unknown keys: strict object.
    for key in ["displayName", "purpose", "currentRequestRef"] {
        let mut input = get_input();
        if let Js::Obj(props) = &mut input {
            props.retain(|(k, _)| k != &edu_academy_mcp::text(key));
        }
        refused(GET, &input);
    }
    let mut extra = get_input();
    extra.set("sectionTitles", titles(&["a"]));
    refused(GET, &extra);
    refused(GET, &Js::Null);
    // At the bounds: accepted (200 units, 256 units, TAB/LF/CR allowed).
    for (name, purpose) in [
        ("a".repeat(200), "p".repeat(256)),
        ("😀".repeat(100), "😀".repeat(128)),
        (
            "Invented Student".to_string(),
            "line one\tTAB\r\nline two".to_string(),
        ),
    ] {
        let mut r = rig();
        let mut input = get_input();
        input.set("displayName", name.as_str().into());
        input.set("purpose", purpose.as_str().into());
        let out = r.driver.call_at(GET, &input, NOW).unwrap();
        // Accepted input reaches admission (the long names are not admitted).
        assert!(out.has("status"), "{}", json_of(&out));
    }
}
#[test]
fn report_input_rules_match_the_zod_schema() {
    let long = "t".repeat(501);
    let astral_251 = "😀".repeat(251);
    let many: Vec<String> = (0..17).map(|i| format!("title {i}")).collect();
    let many_refs: Vec<&str> = many.iter().map(String::as_str).collect();
    for (key, value) in [
        // prestudyKey: ^[1-9][0-9]{0,15}$
        ("prestudyKey", Js::from("0123")),
        ("prestudyKey", Js::from("")),
        ("prestudyKey", Js::from("12345678901234567")),
        ("prestudyKey", Js::from("12a")),
        ("prestudyKey", Js::from("12345\n")),
        ("prestudyKey", Js::Num(12345.0)),
        // sectionTitles: 1-16 unique strings of 1-500 units
        ("sectionTitles", titles(&[])),
        ("sectionTitles", titles(&["a", "a"])),
        ("sectionTitles", titles(&[""])),
        ("sectionTitles", titles(&[long.as_str()])),
        ("sectionTitles", titles(&[astral_251.as_str()])),
        ("sectionTitles", titles(&many_refs)),
        ("sectionTitles", Js::from("개념 이해")),
        ("sectionTitles", Js::Null),
        ("sectionTitles", Js::Arr(vec![Js::Num(1.0)])),
        // maxOutputBytes: integer 1024..=524288
        ("maxOutputBytes", Js::Num(0.0)),
        ("maxOutputBytes", Js::Num(1023.0)),
        ("maxOutputBytes", Js::Num(524289.0)),
        ("maxOutputBytes", Js::Num(1024.5)),
        ("maxOutputBytes", Js::from("2048")),
        ("maxOutputBytes", Js::Null),
        ("maxOutputBytes", Js::Num(f64::NAN)),
    ] {
        refused(REPORT, &report_with(&[(key, value)]));
    }
    // Unknown key and missing prestudyKey.
    refused(REPORT, &report_with(&[("transcript", true.into())]));
    let mut no_key = report_input();
    if let Js::Obj(props) = &mut no_key {
        props.retain(|(k, _)| k != &edu_academy_mcp::text("prestudyKey"));
    }
    refused(REPORT, &no_key);
    // The same refusals hold for the shared fields.
    refused(
        REPORT,
        &report_with(&[("displayName", Js::from("😀".repeat(101)))]),
    );
    refused(REPORT, &report_with(&[("purpose", Js::from("a\u{1}"))]));
    // Accepted at the bounds: 1,16 titles, 500 units, 1024 and 524288 bytes,
    // a 16-digit key, titles differing only by case or by one unit.
    let exact_500 = "😀".repeat(250);
    let sixteen: Vec<String> = (0..16).map(|i| format!("t{i}")).collect();
    let sixteen_refs: Vec<&str> = sixteen.iter().map(String::as_str).collect();
    for extra in [
        vec![("sectionTitles", titles(&["a"]))],
        vec![("sectionTitles", titles(&sixteen_refs))],
        vec![("sectionTitles", titles(&[exact_500.as_str()]))],
        vec![("sectionTitles", titles(&["Ab", "aB", "ab"]))],
        vec![("maxOutputBytes", Js::Num(1024.0))],
        vec![("maxOutputBytes", Js::Num(524288.0))],
        vec![("prestudyKey", Js::from("1234567890123456"))],
    ] {
        let mut r = rig();
        let out = r
            .driver
            .call_at(REPORT, &report_with(&extra), NOW_REPORT)
            .unwrap();
        assert!(out.has("status"), "{extra:?}");
    }
}

// The registry schema runs first at the protocol layer; both layers refuse.
// ======================================================== report: R08-1..4 ===
// PRC:11-16 plus the byte-equal pin of the full result.
#[test]
fn report_full_retains_sections_envelope_data_and_source_identity_byte_equal_to_ts() {
    let mut r = rig();
    let out = call_report(&mut r, &[]);
    assert_eq!(
        json_of(&out),
        r##"{"purpose":"Review invented report","currentRequestRef":"synthetic-report-precision","displayName":"Invented Student","prestudyKey":"12345","academyWrites":0,"parentSends":0,"automaticRetryAllowed":false,"sourceContentIsUntrustedData":true,"execution":"read_only","studentContent":"teacher_only_do_not_store_or_relay","student":{"studentId":"77001","displayName":"Invented Student"},"sourceRecord":{"prestudyKey":"12345","unitLabel":"9-1 Invented unit","dateLiteral":"40.01.02 03:04"},"sections":[{"title":"개념 이해","subtitle":"개념을 정확히 이해하고 설명하는가?","blocks":[{"heading":"reason","text":"πr² = 50%\n−2 < 0"}]},{"title":"논리적 설명","subtitle":null,"blocks":[{"heading":null,"text":"An invented explanation"}]},{"title":"총평","subtitle":null,"blocks":[{"heading":null,"text":"An invented conclusion 😀"}]}],"scores":[77,80,1e-7],"encodingResult":"success","observedAt":"2040-01-02T03:04:00.000Z","coverage":"exact_prestudy_detail_and_server_rendered_ai_report_sections_not_transcript","status":"observed","view":{"mode":"full","requestedSectionTitles":null,"availableSectionTitles":["개념 이해","논리적 설명","총평"],"returnedSectionTitles":["개념 이해","논리적 설명","총평"],"missingSectionTitles":[],"completeRequestedView":true,"completeSourceReport":true}}"##
    );
    assert_eq!(
        json_of(&out).len(),
        1331,
        "UTF-8 bytes node counts for the same payload"
    );
    assert_eq!(out.get("status").code_units(), "observed");
    assert_eq!(
        out.get("studentContent").code_units(),
        "teacher_only_do_not_store_or_relay"
    );
    assert_eq!(out.get("view").get("mode").code_units(), "full");
    assert_eq!(r.ops(), ["prestudy.detail", "prestudy.report"]);
    // Score numbers print as JavaScript prints them (1e-7, not 0.0000001).
    assert!(json_of(&out).contains("\"scores\":[77,80,1e-7]"));
    assert_eq!(
        edu_academy_sources::prestudy::scores_json(&[77.0, 80.0, 0.0000001]),
        "[77,80,1e-7]"
    );
}

// PRC:18-23 (selection in source order, no unrelated narrative, one read) and
// PRC:25-31 (missing titles stay explicit; none matched is still partial).
#[test]
fn section_selection_exact_partial_missing_and_source_order() {
    let mut r = rig();
    let out = call_report(&mut r, &[("sectionTitles", titles(&["총평", "개념 이해"]))]);
    assert_eq!(
        json_of(&out),
        r##"{"purpose":"Review invented report","currentRequestRef":"synthetic-report-precision","displayName":"Invented Student","prestudyKey":"12345","academyWrites":0,"parentSends":0,"automaticRetryAllowed":false,"sourceContentIsUntrustedData":true,"execution":"read_only","studentContent":"teacher_only_do_not_store_or_relay","student":{"studentId":"77001","displayName":"Invented Student"},"sourceRecord":{"prestudyKey":"12345","unitLabel":"9-1 Invented unit","dateLiteral":"40.01.02 03:04"},"sections":[{"title":"개념 이해","subtitle":"개념을 정확히 이해하고 설명하는가?","blocks":[{"heading":"reason","text":"πr² = 50%\n−2 < 0"}]},{"title":"총평","subtitle":null,"blocks":[{"heading":null,"text":"An invented conclusion 😀"}]}],"scores":[77,80,1e-7],"encodingResult":"success","observedAt":"2040-01-02T03:04:00.000Z","coverage":"exact_prestudy_detail_and_server_rendered_ai_report_sections_not_transcript","status":"observed","view":{"mode":"selected","requestedSectionTitles":["총평","개념 이해"],"availableSectionTitles":["개념 이해","논리적 설명","총평"],"returnedSectionTitles":["개념 이해","총평"],"missingSectionTitles":[],"completeRequestedView":true,"completeSourceReport":false}}"##
    );
    assert_eq!(json_of(&out).len(), 1233);
    assert_eq!(out.get("status").code_units(), "observed");
    assert!(!json_of(&out).contains("An invented explanation"));
    assert_eq!(r.reads(), 2, "one detail read and one report read");
    let view = out.get("view");
    assert_eq!(view.get("mode").code_units(), "selected");
    assert_eq!(view.get("completeRequestedView"), &Js::Bool(true));
    assert_eq!(view.get("completeSourceReport"), &Js::Bool(false));
    // Missing titles: partial, listed, returned sections keep source order.
    for (asked, returned) in [
        (vec!["概念理解", "개념 이해"], 1usize),
        (vec!["概念理解"], 0),
    ] {
        let mut r = rig();
        let out = call_report(&mut r, &[("sectionTitles", titles(&asked))]);
        assert_eq!(out.get("status").code_units(), "partial", "{asked:?}");
        assert_eq!(out.get("sections").array().len(), returned);
        let view = out.get("view");
        assert_eq!(view.get("missingSectionTitles").array().len(), 1);
        assert_eq!(
            text_of(&view.get("missingSectionTitles").array()[0]),
            "概念理解"
        );
        assert_eq!(view.get("completeRequestedView"), &Js::Bool(false));
        assert_eq!(view.get("completeSourceReport"), &Js::Bool(false));
        assert_eq!(r.reads(), 2);
        assert!(!out.has("failure"));
    }
    // Missing title in the middle of the list, byte-equal to TS.
    let mut r = rig();
    let out = call_report(&mut r, &[("sectionTitles", titles(&["総評", "총평"]))]);
    assert_eq!(
        json_of(&out),
        r##"{"purpose":"Review invented report","currentRequestRef":"synthetic-report-precision","displayName":"Invented Student","prestudyKey":"12345","academyWrites":0,"parentSends":0,"automaticRetryAllowed":false,"sourceContentIsUntrustedData":true,"execution":"read_only","studentContent":"teacher_only_do_not_store_or_relay","student":{"studentId":"77001","displayName":"Invented Student"},"sourceRecord":{"prestudyKey":"12345","unitLabel":"9-1 Invented unit","dateLiteral":"40.01.02 03:04"},"sections":[{"title":"총평","subtitle":null,"blocks":[{"heading":null,"text":"An invented conclusion 😀"}]}],"scores":[77,80,1e-7],"encodingResult":"success","observedAt":"2040-01-02T03:04:00.000Z","coverage":"exact_prestudy_detail_and_server_rendered_ai_report_sections_not_transcript","status":"partial","view":{"mode":"selected","requestedSectionTitles":["総評","총평"],"availableSectionTitles":["개념 이해","논리적 설명","총평"],"returnedSectionTitles":["총평"],"missingSectionTitles":["総評"],"completeRequestedView":false,"completeSourceReport":false}}"##
    );
    assert_eq!(json_of(&out).len(), 1066);
    // PRC:94-96-style: all selected sections can comprise the full source.
    let mut r = rig();
    let out = call_report(
        &mut r,
        &[(
            "sectionTitles",
            titles(&["総評", "개념 이해", "논리적 설명", "총평"]),
        )],
    );
    assert_eq!(out.get("status").code_units(), "partial");
    assert_eq!(
        out.get("view").get("completeRequestedView"),
        &Js::Bool(false)
    );
    assert_eq!(out.get("view").get("completeSourceReport"), &Js::Bool(true));
    // A lone-surrogate title equals no source title; it is echoed and missing.
    let lone = Js::Arr(vec![Js::Str(vec![0xd800]), "총평".into()]);
    let mut r = rig();
    let out = call_report(&mut r, &[("sectionTitles", lone)]);
    assert_eq!(out.get("status").code_units(), "partial");
    assert_eq!(
        out.get("view").get("missingSectionTitles").array()[0].units(),
        [0xd800u16].as_slice()
    );
    assert_eq!(
        out.get("view").get("requestedSectionTitles").array()[0].units(),
        [0xd800u16].as_slice()
    );
    assert_eq!(out.get("sections").array().len(), 1);
}

// PRC:33-39: an ambiguous requested title fails with no narrative.
#[test]
fn ambiguous_selected_title_fails_closed_without_leaking_narratives() {
    let mut sections = base_sections();
    sections.push((
        "개념 이해",
        None,
        vec![(None, "Other ambiguous narrative".into())],
    ));
    let mut r = with_pages(|p| {
        p.insert("prestudy.report", report_page(&sections, BASE_SCORES));
    });
    let out = call_report(&mut r, &[("sectionTitles", titles(&["개념 이해"]))]);
    assert_unavailable(
        &out,
        "prestudy_report_section_ambiguous",
        "identity_ambiguity",
    );
    let json = json_of(&out);
    assert!(!json.contains("Other ambiguous narrative") && !json.contains("An invented"));
    assert_eq!(r.reads(), 2);
    listed(REPORT, "prestudy_report_section_ambiguous");
    // Without a selection the duplicate title is simply listed twice.
    let mut r = with_pages(|p| {
        p.insert("prestudy.report", report_page(&sections, BASE_SCORES));
    });
    let out = call_report(&mut r, &[]);
    assert_eq!(out.get("status").code_units(), "observed");
    assert_eq!(
        out.get("view").get("availableSectionTitles").array().len(),
        4
    );
}

// ====================================================== budget (R08-3) =======
fn big_section(title: &'static str, blocks: usize, unit: &str, repeat: usize) -> Sec<'static> {
    (
        title,
        None,
        (0..blocks)
            .map(|i| {
                (
                    Some(if i == 0 { "first" } else { "next" }),
                    unit.repeat(repeat),
                )
            })
            .collect(),
    )
}
fn report_rig(sections: &[Sec]) -> Rig {
    let page = report_page(sections, BASE_SCORES);
    with_pages(|p| {
        p.insert("prestudy.report", page);
    })
}
// PRC:57-62: the whole result is refused, never clipped or leaked.
#[test]
fn budget_refuses_the_whole_report_instead_of_clipping() {
    let mut sections = base_sections();
    sections[0].2[0].1 = "가".repeat(2000);
    let mut r = report_rig(&sections);
    let out = call_report(&mut r, &[("maxOutputBytes", Js::Num(1024.0))]);
    assert_unavailable(&out, "prestudy_report_result_too_large", "partial_coverage");
    assert!(!json_of(&out).contains('가'));
    assert_eq!(r.reads(), 2);
    listed(REPORT, "prestudy_report_result_too_large");
}
// PRC:64-68: a selected view fits a budget the complete report cannot.
#[test]
fn a_selected_view_can_fit_a_budget_when_the_complete_report_cannot() {
    let mut sections = base_sections();
    sections[1] = big_section("논리적 설명", 2, "large invented narrative ", 800);
    let mut r = report_rig(&sections);
    let out = call_report(
        &mut r,
        &[
            ("sectionTitles", titles(&["개념 이해"])),
            ("maxOutputBytes", Js::Num(4096.0)),
        ],
    );
    assert_eq!(out.get("status").code_units(), "observed");
    assert_eq!(out.get("sections").array().len(), 1);
    assert_eq!(
        out.get("view").get("completeSourceReport"),
        &Js::Bool(false)
    );
    let mut r = report_rig(&sections);
    let out = call_report(&mut r, &[("maxOutputBytes", Js::Num(4096.0))]);
    assert_eq!(reason(&out), "prestudy_report_result_too_large");
}
// PRC:70-80 with multi-byte text: `maxOutputBytes` equal to the payload size
// passes, one byte less refuses; bytes are UTF-8, not characters.
#[test]
fn byte_boundary_on_multibyte_payload_passes_at_size_and_refuses_one_byte_less() {
    for selected in [true, false] {
        let mut sections = base_sections();
        sections[0].2[0].1 = format!("{}😀{}", "가나다".repeat(2500), "é".repeat(500));
        let extra: Vec<(&str, Js)> = if selected {
            vec![("sectionTitles", titles(&["개념 이해"]))]
        } else {
            vec![]
        };
        let probe = call_report(&mut report_rig(&sections), &extra);
        assert_eq!(probe.get("status").code_units(), "observed");
        let bytes = json_of(&probe).len();
        let chars = json_of(&probe).chars().count();
        assert!(
            bytes > chars + 10_000,
            "multi-byte text: {bytes} bytes, {chars} chars"
        );
        assert!((1024..=524288).contains(&bytes));
        let mut at = extra.clone();
        at.push(("maxOutputBytes", Js::Num(bytes as f64)));
        let out = call_report(&mut report_rig(&sections), &at);
        assert_eq!(
            out.get("status").code_units(),
            "observed",
            "selected={selected}"
        );
        assert_eq!(json_of(&out), json_of(&probe));
        let mut under = extra.clone();
        under.push(("maxOutputBytes", Js::Num((bytes - 1) as f64)));
        let out = call_report(&mut report_rig(&sections), &under);
        assert_unavailable(&out, "prestudy_report_result_too_large", "partial_coverage");
        // One character count equal to the byte budget would pass if
        // characters were counted: the refusal above proves bytes are.
        let mut chars_only = extra;
        chars_only.push(("maxOutputBytes", Js::Num(chars as f64)));
        assert_eq!(
            reason(&call_report(&mut report_rig(&sections), &chars_only)),
            "prestudy_report_result_too_large"
        );
    }
}
// PRC:82-88 (`17000` bytes, selected): the budget measures one JSON payload,
// not the duplicated MCP envelope; the selected default is 32 KiB.
#[test]
fn selected_budget_measures_one_payload_not_the_duplicated_envelope() {
    let mut sections = base_sections();
    sections[0].2[0].1 = "a".repeat(17000);
    let mut r = report_rig(&sections);
    let out = call_report(&mut r, &[("sectionTitles", titles(&["개념 이해"]))]);
    assert_eq!(out.get("status").code_units(), "observed");
    let payload = json_of(&out).len();
    assert!(payload < 20 * 1024, "{payload}");
    // The dual representation of the same result is over 32 KiB.
    let dual = encode(&edu_academy_mcp::audit::tool_result(out.clone()).unwrap())
        .unwrap()
        .len();
    assert!(dual > 32 * 1024, "{dual}");
}
// PRC:90-92 / PRC:98-104: the default selected budget is 32 KiB, the default
// full budget 512 KiB.
#[test]
fn default_budgets_are_32_kib_selected_and_512_kib_full() {
    let mut sections = base_sections();
    sections[0].2[0] = (Some("reason"), "large invented narrative ".repeat(800));
    sections[0]
        .2
        .push((Some("more"), "large invented narrative ".repeat(800)));
    let mut r = report_rig(&sections);
    let out = call_report(&mut r, &[("sectionTitles", titles(&["개념 이해"]))]);
    assert_unavailable(&out, "prestudy_report_result_too_large", "partial_coverage");
    // The same report, full: 40 KB of text is far below 512 KiB.
    let mut r = report_rig(&sections);
    assert_eq!(
        call_report(&mut r, &[]).get("status").code_units(),
        "observed"
    );
    // An explicit larger budget lifts the selected default.
    let mut r = report_rig(&sections);
    let out = call_report(
        &mut r,
        &[
            ("sectionTitles", titles(&["개념 이해"])),
            ("maxOutputBytes", Js::Num(65536.0)),
        ],
    );
    assert_eq!(out.get("status").code_units(), "observed");
}
// PRC:82-88 (second test, `the existing independent MCP envelope safety cap`):
// a payload under the 512 KiB budget is still refused when its dual
// representation is over 512 KiB.
#[test]
fn independent_dual_representation_cap_applies_below_the_payload_budget() {
    let heading = "가".repeat(1000);
    let sections: Vec<Sec> = ["A", "B"]
        .into_iter()
        .map(|title| {
            (
                title,
                None,
                (0..50)
                    .map(|_| (Some(heading.as_str()), "valid".to_string()))
                    .collect(),
            )
        })
        .collect();
    let mut r = report_rig(&sections);
    let out = call_report(&mut r, &[("maxOutputBytes", Js::Num(524288.0))]);
    assert_unavailable(&out, "prestudy_report_result_too_large", "partial_coverage");
    // The payload alone is under both the 512 KiB budget and the cap: only
    // its doubled representation is over it.
    let raw: usize = sections
        .iter()
        .flat_map(|s| s.2.iter())
        .map(|b| b.0.unwrap().len() + b.1.len())
        .sum();
    assert!(raw < 512 * 1024 && raw * 2 > 512 * 1024, "{raw}");
}
// The 512 KiB dual cap on the lists tool: `prestudy_result_too_large`.
#[test]
fn get_dual_representation_cap_refuses_an_oversized_list_result() {
    let unit = "u".repeat(1500);
    let rows: Vec<ListRow> = (0..200)
        .map(|_| {
            (
                "Invented Student",
                unit.as_str(),
                "40.01.02 03:04",
                "OK",
                "12345",
            )
        })
        .collect();
    let page = list_page("WebUnPreStudy", &rows);
    let mut r = with_pages(|p| {
        p.insert("prestudy.waiting.shell", page.clone());
        p.insert("prestudy.waiting.search", page);
    });
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_unavailable(&out, "prestudy_result_too_large", "partial_coverage");
    listed(GET, "prestudy_result_too_large");
}

// ================================================== SDK wire (PRC:131-145) ===
struct Scene {
    install: PathBuf,
    base: PathBuf,
}
impl Drop for Scene {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn put(path: &PathBuf, bytes: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
fn pin_json(path: &PathBuf) -> Js {
    obj(&[
        ("path", path.to_str().unwrap().into()),
        (
            "sha256",
            edu_academy_mcp::digest(&fs::read(path).unwrap()).into(),
        ),
    ])
}
fn doc(path: &PathBuf, value: &Js) -> Js {
    put(path, &encode(value).unwrap());
    pin_json(path)
}
/// A fictional release graph whose registry entries for the two prestudy
/// tools come from `tool-registry-v2.json` (the frozen v1 registry lacks
/// `academy_prestudy_report_get`).
impl Scene {
    fn new(label: &str, tools: &[&str]) -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let base = home.join(format!(
            ".e11b-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        for name in ["install", "evidence", "state"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(name))
                .unwrap();
        }
        let install = base.join("install");
        let bridge = install.join("bridge");
        put(
            &bridge,
            b"fictional executable metadata only; never launched",
        );
        let launcher = install.join("launcher");
        put(&launcher, b"fictional launcher, never executed");
        let admission_path = install.join("admission.json");
        put(&admission_path, &encode(&routing_admission()).unwrap());
        let routing = obj(&[
            ("admissionPath", admission_path.to_str().unwrap().into()),
            (
                "admissionSha256",
                edu_academy_mcp::digest(&fs::read(&admission_path).unwrap()).into(),
            ),
        ]);
        let routing_pin = doc(&install.join("routing.json"), &routing);
        let owner = doc(
            &install.join("owner.json"),
            &obj(&[
                ("schemaVersion", "academy.rust-owner/1".into()),
                ("kind", "fictional".into()),
                ("uid", Js::Num(rustix::process::geteuid().as_raw() as f64)),
                ("installRoot", install.to_str().unwrap().into()),
                (
                    "evidenceRoot",
                    base.join("evidence").to_str().unwrap().into(),
                ),
                ("stateRoot", base.join("state").to_str().unwrap().into()),
            ]),
        );
        let args = vec![
            "--install-root".to_string(),
            install.to_str().unwrap().into(),
            "--endpoint".into(),
            "dailyrecord".into(),
        ];
        let mut artifact = pin_json(&bridge);
        artifact.set("role", "entry".into());
        let identity = doc(
            &install.join("identity-dailyrecord.json"),
            &obj(&[
                ("schemaVersion", "academy.rust-endpoint/1".into()),
                ("endpoint", "dailyrecord".into()),
                ("semanticContractVersion", "s11-fictional".into()),
                (
                    "launchArgumentsSha256",
                    edu_academy_mcp::digest(&serde_json::to_vec(&args).unwrap()).into(),
                ),
                ("artifacts", Js::Arr(vec![artifact])),
            ]),
        );
        let mut ep = obj(&[]);
        ep.set("executable", pin_json(&bridge));
        ep.set("identity", identity);
        ep.set("args", Js::Arr(args.into_iter().map(Js::from).collect()));
        let endpoints_pin = doc(
            &install.join("endpoints.json"),
            &obj(&[("dailyrecord", ep)]),
        );
        let v2: serde_json::Value =
            serde_json::from_slice(include_bytes!("fixtures/tool-registry-v2.json")).unwrap();
        let entries: Vec<Js> = v2["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| tools.contains(&t["name"].as_str().unwrap()))
            .map(|t| {
                let tool = serde_json::json!({
                    "name": t["name"], "description": t["description"],
                    "inputSchema": t["inputSchema"], "annotations": t["annotations"],
                });
                obj(&[
                    ("endpoint", "dailyrecord".into()),
                    ("upstreamName", t["upstreamName"].as_str().unwrap().into()),
                    ("tool", parse(&serde_json::to_vec(&tool).unwrap()).unwrap()),
                ])
            })
            .collect();
        assert_eq!(entries.len(), tools.len());
        let registry_pin = doc(&install.join("registry.json"), &Js::Arr(entries));
        let source = install.join("source.bin");
        put(&source, b"fictional source version one");
        let mut source_pin = pin_json(&source);
        source_pin.set("role", "source".into());
        let build = doc(
            &install.join("build.json"),
            &obj(&[
                ("schemaVersion", "academy.rust-build/1".into()),
                ("sourceRevision", "0".repeat(40).into()),
                ("semanticContractVersion", "s11-fictional".into()),
                ("cargoLockSha256", "0".repeat(64).into()),
                ("artifacts", Js::Arr(vec![source_pin])),
            ]),
        );
        let pointer = obj(&[
            ("schemaVersion", "academy.mcp-release/3".into()),
            ("generation", "fictional-v1".into()),
            ("bridge", pin_json(&bridge)),
            ("launcher", pin_json(&launcher)),
            ("build", build),
            ("routing", routing_pin),
            ("owner", owner),
            ("registry", registry_pin),
            ("endpoints", endpoints_pin),
        ]);
        put(&install.join("release.json"), &encode(&pointer).unwrap());
        Self { install, base }
    }
    fn graph(&self) -> ReleaseGraph {
        ReleaseGraph::open(&self.install).unwrap()
    }
    fn session(&self) -> RpcSession {
        RpcSession::endpoint(self.graph(), "dailyrecord".into()).unwrap()
    }
}
fn rpc(id: &str, tool: &str, args: &Js) -> Js {
    obj(&[
        ("jsonrpc", "2.0".into()),
        ("id", id.into()),
        ("method", "tools/call".into()),
        (
            "params",
            obj(&[("name", tool.into()), ("arguments", args.clone())]),
        ),
    ])
}
fn loan(session: &mut RpcSession, rig: &mut Rig, id: &str, tool: &str, args: &Js) -> Js {
    session
        .handle_prestudy(&rpc(id, tool, args), &mut rig.driver)
        .unwrap()
        .unwrap()
}

// Both compatible representations survive the loan: `content[0].text` parses
// to `structuredContent` (PRC:131-145 over the Rust protocol).
#[test]
fn selected_and_partial_views_survive_the_wire_in_both_representations() {
    let scene = Scene::new("wire", &TOOLS);
    let mut session = scene.session();
    for (titles_asked, status, complete) in [
        (vec!["개념 이해"], "observed", true),
        (vec!["개념 이해", "Unavailable section"], "partial", false),
    ] {
        let mut r = rig();
        let out = loan(
            &mut session,
            &mut r,
            "1",
            REPORT,
            &report_with(&[("sectionTitles", titles(&titles_asked))]),
        );
        let result = out.get("result");
        assert!(!result.has("isError"), "{}", json_of(&out));
        let structured = result.get("structuredContent");
        assert_eq!(structured.get("status").code_units(), status);
        assert_eq!(structured.get("sections").array().len(), 1);
        assert_eq!(
            text_of(structured.get("sections").array()[0].get("title")),
            "개념 이해"
        );
        assert_eq!(
            text_of(structured.get("sourceRecord").get("unitLabel")),
            "9-1 Invented unit"
        );
        assert_eq!(
            structured.get("view").get("completeRequestedView"),
            &Js::Bool(complete)
        );
        assert_eq!(
            structured.get("view").get("completeSourceReport"),
            &Js::Bool(false)
        );
        let content = result.get("content").array();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0].get("type").code_units(), "text");
        assert_eq!(
            parse(text_of(content[0].get("text")).as_bytes()).unwrap(),
            *structured
        );
        assert_eq!(json_of(structured), text_of(content[0].get("text")));
        assert_eq!(out.get("id").code_units(), "1");
    }
}

// Loan routing: the registry schema refuses first (no read), the Zod rules
// refuse as a tool error, an unadmitted name is an observed refusal, other
// messages traverse the lifecycle, and a registry without the tool is an RPC
// error (the frozen v1 registry lacks `academy_prestudy_report_get`).
// Declared difference (f): TS trims first and then bounds the length
// (`dailyrecord.ts:78`, `.trim().min(1).max(200)`); the loan applies the
// registry JSON Schema `maxLength: 200` to the untrimmed value first
// (`protocol.rs:265`, `validation.rs:322-324`). A name over 200 units before
// the trim and within 200 after it is accepted by TS and refused by the loan
// (fail closed).
#[test]
fn padded_display_name_over_200_before_trim_is_refused_by_the_loan_but_not_the_zod_layer() {
    let scene = Scene::new("padded", &TOOLS);
    let mut session = scene.session();
    let mut r = rig();
    let mut padded = get_input();
    padded.set(
        "displayName",
        format!("Invented Student{}", " ".repeat(190)).into(),
    );
    // The Zod layer (the driver, as TS) trims first and admits the student.
    let out = r.driver.call_at(GET, &padded, NOW).unwrap();
    assert_eq!(out.get("status").code_units(), "observed");
    // The loan refuses it on the untrimmed schema bound.
    let out = loan(&mut session, &mut r, "1", GET, &padded);
    assert!(out.get("result").get("isError").truthy());
    assert_eq!(
        text_of(out.get("result").get("content").array()[0].get("text")),
        "tool_input_invalid"
    );
}

#[test]
fn loan_routes_schema_zod_admission_and_lifecycle() {
    let scene = Scene::new("loan", &TOOLS);
    let mut session = scene.session();
    let mut r = rig();
    // Schema refusal: unknown key.
    let mut bad = get_input();
    bad.set("unexpected", true.into());
    let out = loan(&mut session, &mut r, "1", GET, &bad);
    assert!(out.get("result").get("isError").truthy());
    assert_eq!(
        text_of(out.get("result").get("content").array()[0].get("text")),
        "tool_input_invalid"
    );
    // Zod-only refusal: 202 units pass the JSON Schema (101 code points).
    let mut zod = get_input();
    zod.set("displayName", "😀".repeat(101).into());
    validation_passes(GET, &zod);
    let out = loan(&mut session, &mut r, "2", GET, &zod);
    assert!(out.get("result").get("isError").truthy());
    assert_eq!(
        text_of(out.get("result").get("content").array()[0].get("text")),
        "tool_input_invalid"
    );
    // Duplicate section titles pass the JSON Schema too.
    let dup = report_with(&[("sectionTitles", titles(&["a", "a"]))]);
    validation_passes(REPORT, &dup);
    let out = loan(&mut session, &mut r, "3", REPORT, &dup);
    assert!(out.get("result").get("isError").truthy());
    assert_eq!(r.reads(), 0);
    // Unadmitted name: data result, no read.
    let mut stranger = get_input();
    stranger.set("displayName", "Invented Stranger".into());
    let out = loan(&mut session, &mut r, "4", GET, &stranger);
    assert!(!out.get("result").has("isError"));
    assert_eq!(
        out.get("result")
            .get("structuredContent")
            .get("reasonCode")
            .code_units(),
        "prestudy_student_not_admitted"
    );
    assert_eq!(r.reads(), 0);
    // Success.
    let out = loan(&mut session, &mut r, "5", GET, &get_input());
    assert_eq!(
        out.get("result")
            .get("structuredContent")
            .get("status")
            .code_units(),
        "observed"
    );
    assert_eq!(r.driver.prestudy_source_requests(), 5);
    // Lifecycle: ping and tools/list traverse the unchanged path.
    let ping = obj(&[
        ("jsonrpc", "2.0".into()),
        ("id", 9.into()),
        ("method", "ping".into()),
    ]);
    let out = session
        .handle_prestudy(&ping, &mut r.driver)
        .unwrap()
        .unwrap();
    assert!(out.has("result") && !out.has("error"));
    let list = obj(&[
        ("jsonrpc", "2.0".into()),
        ("id", 10.into()),
        ("method", "tools/list".into()),
    ]);
    let out = session
        .handle_prestudy(&list, &mut r.driver)
        .unwrap()
        .unwrap();
    let names: Vec<String> = out
        .get("result")
        .get("tools")
        .array()
        .iter()
        .map(|t| text_of(t.get("name")))
        .collect();
    assert!(names.contains(&GET.to_string()) && names.contains(&REPORT.to_string()));
    // A name outside the two is not claimed by this loan: the unchanged
    // lifecycle path refuses it, and the driver is not read.
    let reads = r.reads();
    let other = loan(
        &mut session,
        &mut r,
        "11",
        "academy_textbook_catalog",
        &obj(&[]),
    );
    assert!(other.get("result").get("isError").truthy());
    assert_eq!(
        other
            .get("result")
            .get("structuredContent")
            .get("code")
            .code_units(),
        "unknown_tool"
    );
    assert_eq!(r.reads(), reads);
    // A registry without the report tool: the loan refuses at the binding.
    let scene = Scene::new("loan-get-only", &[GET]);
    let mut session = scene.session();
    let mut r = rig();
    let out = loan(&mut session, &mut r, "1", REPORT, &report_input());
    assert!(out.has("error"), "{}", json_of(&out));
    assert_eq!(r.reads(), 0);
    // Another endpoint session does not claim the tools.
    let scene = Scene::new("loan-endpoint", &TOOLS);
    let mut session = RpcSession::endpoint(scene.graph(), "dailyrecord".into()).unwrap();
    let mut wrong = PrestudyDriver::new(
        Fake {
            admission: transport_admission(true),
            pages: default_pages(),
            seen: Rc::new(RefCell::new(vec![])),
            fault: None,
        },
        admission(),
        obj(&[("sourceNamespace", "academy.lms.dayrecord".into())]),
    )
    .unwrap();
    let out = session
        .handle_prestudy(&rpc("1", GET, &get_input()), &mut wrong)
        .unwrap()
        .unwrap();
    assert!(
        out.has("error"),
        "a driver bound to another admission refuses: {}",
        json_of(&out)
    );
}
fn validation_passes(tool: &str, input: &Js) {
    let v2: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/tool-registry-v2.json")).unwrap();
    let entry = v2["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == tool)
        .unwrap();
    let schema = parse(&serde_json::to_vec(&entry["inputSchema"]).unwrap()).unwrap();
    validation::validate(&schema, input).unwrap_or_else(|e| panic!("schema refused: {}", e.code));
}

// ===================================================== errors (step 4) =======
// The twelve report and detail codes, each with its category and the exact
// code, produced through the adapter where a source page can produce them.
#[test]
fn the_twelve_report_and_detail_codes_keep_their_catalogue_cause() {
    // 1. prestudy_report_source_port_unavailable: no admitted contract.
    let mut r = rig_with(default_pages(), false, None);
    let out = call_report(&mut r, &[]);
    assert_unavailable(
        &out,
        "prestudy_report_source_port_unavailable",
        "endpoint_absence",
    );
    assert_eq!(
        out.get("failure").get("evidenceLocator").code_units(),
        "src/native-dailyrecord-backend.ts:sourcePort"
    );
    assert_eq!(r.reads(), 0);
    let mut r = rig_with(default_pages(), false, None);
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_unavailable(&out, "prestudy_source_port_unavailable", "endpoint_absence");
    // 3. prestudy_detail_student_mismatch
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.detail",
            detail_page("77009", "12345", None, None, Some("55501")),
        );
    });
    assert_unavailable(
        &call_report(&mut r, &[]),
        "prestudy_detail_student_mismatch",
        "policy_denial",
    );
    assert_eq!(r.reads(), 1, "the report URL is never followed");
    // 4. prestudy_detail_record_mismatch
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.detail",
            detail_page("77001", "12346", None, None, Some("55501")),
        );
    });
    assert_unavailable(
        &call_report(&mut r, &[]),
        "prestudy_detail_record_mismatch",
        "contract_drift",
    );
    // 5. prestudy_detail_unqualified: duplicate identity inputs, then
    // conflicting report links.
    let dup = detail_page("77001", "12345", None, None, Some("55501")).replace(
        "</form>",
        r#"<input name="student_pri_no" value="77001"></form>"#,
    );
    let mut r = with_pages(|p| {
        p.insert("prestudy.detail", dup);
    });
    assert_unavailable(
        &call_report(&mut r, &[]),
        "prestudy_detail_unqualified",
        "contract_drift",
    );
    let conflict = detail_page("77001", "12345", None, None, Some("55501")).replace(
        "</html>",
        r#"<button onclick="viewAiReport('55502');">x</button></html>"#,
    );
    let mut r = with_pages(|p| {
        p.insert("prestudy.detail", conflict);
    });
    assert_unavailable(
        &call_report(&mut r, &[]),
        "prestudy_detail_unqualified",
        "contract_drift",
    );
    // 8. prestudy_report_unqualified: no section cards.
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.report",
            "<html><body><p>nothing</p></body></html>".into(),
        );
    });
    assert_unavailable(
        &call_report(&mut r, &[]),
        "prestudy_report_unqualified",
        "contract_drift",
    );
    // 9. prestudy_report_source_rejected: more than 60,000 kept text units.
    let big = ["A", "B", "C", "D"].map(|t| big_section(t, 1, "x", 16_000));
    let mut r = report_rig(&big);
    assert_unavailable(
        &call_report(&mut r, &[]),
        "prestudy_report_source_rejected",
        "malformed_response",
    );
    // 10. prestudy_report_not_ready: no analysis link, and the cannot-generate page.
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.detail",
            detail_page("77001", "12345", None, None, None),
        );
    });
    let out = call_report(&mut r, &[("sectionTitles", titles(&["개념 이해"]))]);
    assert_unavailable(&out, "prestudy_report_not_ready", "partial_coverage");
    assert_eq!(r.reads(), 1);
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.report",
            "<html>리포트를 생성할 수 없습니다</html>".into(),
        );
    });
    assert_unavailable(
        &call_report(&mut r, &[]),
        "prestudy_report_not_ready",
        "partial_coverage",
    );
    // 11 and 12 are covered by the budget and ambiguity tests.
    // Not reachable through the adapter: prestudy_report_principal_rejected
    // (label and key come from the admission), prestudy_report_scope_conflict
    // and prestudy_report_result_unqualified (the source builds the record
    // from the detail it just matched): their catalogue mapping is pinned here
    // and the gates in `rebind_*` unit tests inside the module.
    for (code, category, locator) in [
        (
            "prestudy_report_principal_rejected",
            "policy_denial",
            "source-owner:prestudy-detail",
        ),
        (
            "prestudy_report_scope_conflict",
            "contract_drift",
            "source-owner:prestudy-detail",
        ),
        (
            "prestudy_report_result_unqualified",
            "contract_drift",
            "source-owner:prestudy-detail",
        ),
    ] {
        let failure = public_failure(code, REPORT_FALLBACK);
        assert_eq!(failure.get("code").code_units(), code);
        assert_eq!(failure.get("category").code_units(), category);
        assert_eq!(failure.get("evidenceLocator").code_units(), locator);
    }
    for code in [
        "prestudy_report_source_port_unavailable",
        "prestudy_report_principal_rejected",
        "prestudy_detail_student_mismatch",
        "prestudy_detail_record_mismatch",
        "prestudy_detail_unqualified",
        "prestudy_report_scope_conflict",
        "prestudy_report_result_unqualified",
        "prestudy_report_unqualified",
        "prestudy_report_source_rejected",
        "prestudy_report_not_ready",
        "prestudy_report_result_too_large",
        "prestudy_report_section_ambiguous",
    ] {
        listed(REPORT, code);
        assert_eq!(
            public_failure(code, REPORT_FALLBACK)
                .get("code")
                .code_units(),
            code
        );
    }
}

// Lists: the codes the list read produces, and the one the TS catalogue lacks.
#[test]
fn list_read_codes_and_principal_rejection_collapse_to_the_fallback() {
    // prestudy_search_form_missing / _drift / authentication / source rejected
    // / row and summary-row unqualified.
    let cases: Vec<(&str, HashMap<&'static str, String>, &str, &str)> = vec![
        (
            "form missing",
            HashMap::from([("prestudy.waiting.shell", "<html>no form</html>".to_string())]),
            "prestudy_search_form_missing",
            "contract_drift",
        ),
        (
            "form drift",
            HashMap::from([(
                "prestudy.waiting.shell",
                list_page("WebCtPreStudy", &[]),
            )]),
            "prestudy_search_form_drift",
            "contract_drift",
        ),
        (
            "expired",
            HashMap::from([(
                "prestudy.waiting.shell",
                "<html>세션이 만료되었습니다</html>".to_string(),
            )]),
            "prestudy_authentication_required",
            "authentication",
        ),
        (
            "row",
            HashMap::from([(
                "prestudy.waiting.search",
                list_page("WebUnPreStudy", &[("", "unit", "date", "OK", "12345")]),
            )]),
            "prestudy_row_unqualified",
            "malformed_response",
        ),
        (
            "summary row",
            HashMap::from([(
                "prestudy.summary",
                "<table><tr><td>a</td><td>b</td><td><button onclick=\"openReport(1)\">r</button></td></tr></table>"
                    .to_string(),
            )]),
            "prestudy_summary_row_unqualified",
            "malformed_response",
        ),
    ];
    for (label, override_pages, code, category) in cases {
        let mut r = with_pages(|p| p.extend(override_pages));
        let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
        assert_unavailable(&out, code, category);
        listed(GET, code);
        assert!(r.reads() >= 1, "{label}");
    }
    // prestudy_source_rejected: a body over the 8,000,000-byte bound.
    let mut r = with_pages(|p| {
        p.insert("prestudy.waiting.shell", "x".repeat(8_000_001));
    });
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_unavailable(&out, "prestudy_source_rejected", "malformed_response");
    listed(GET, "prestudy_source_rejected");
    // `prestudy_principal_rejected` is in neither TS nor Rust catalogue: it
    // collapses to the fallback with the owner-read-boundary locator.
    let failure = public_failure("prestudy_principal_rejected", GET_FALLBACK);
    assert_eq!(failure.get("code").code_units(), GET_FALLBACK);
    assert_eq!(failure.get("category").code_units(), "upstream_unavailable");
    assert_eq!(
        failure.get("evidenceLocator").code_units(),
        "academy-mcp:owner-read-boundary"
    );
    // The same code for the report tool is catalogued and keeps itself.
    assert_eq!(
        public_failure("prestudy_report_principal_rejected", REPORT_FALLBACK)
            .get("code")
            .code_units(),
        "prestudy_report_principal_rejected"
    );
    listed(GET, GET_FALLBACK);
    listed(REPORT, REPORT_FALLBACK);
}

// Shape limits -> `source_response_malformed` (a ZodError in TS); every other
// failure -> the tool fallback with no source text; a catalogued transport
// code keeps itself.
#[test]
fn shape_limits_and_uncatalogued_failures_never_leak_source_text() {
    // A 20,001-unit block passes the 60,000-unit parser bound and fails the
    // backend shape (`text.max(20000)`).
    let sections = vec![big_section("개념 이해", 1, "x", 20_001)];
    let mut r = report_rig(&sections);
    let out = call_report(&mut r, &[]);
    assert_unavailable(&out, "source_response_malformed", "malformed_response");
    assert_eq!(
        out.get("failure").get("evidenceLocator").code_units(),
        "academy-mcp:response-contract"
    );
    listed(REPORT, "source_response_malformed");
    // A 501-unit unit label (detail field) also fails the shape.
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.detail",
            detail_page(
                "77001",
                "12345",
                Some(&"u".repeat(501)),
                None,
                Some("55501"),
            ),
        );
    });
    assert_unavailable(
        &call_report(&mut r, &[]),
        "source_response_malformed",
        "malformed_response",
    );
    // List shape: a 201-unit date literal.
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.waiting.search",
            list_page(
                "WebUnPreStudy",
                &[("Invented Student", "unit", &"d".repeat(201), "OK", "12345")],
            ),
        );
    });
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_unavailable(&out, "source_response_malformed", "malformed_response");
    listed(GET, "source_response_malformed");
    // A key that is not ^[1-9][0-9]{0,15}$ fails the list shape.
    let mut r = with_pages(|p| {
        p.insert(
            "prestudy.waiting.search",
            list_page(
                "WebUnPreStudy",
                &[("Invented Student", "unit", "d", "OK", "01234")],
            ),
        );
    });
    let out = r.driver.call_at(GET, &get_input(), NOW).unwrap();
    assert_unavailable(&out, "source_response_malformed", "malformed_response");
    // Uncatalogued upstream code: the tool fallback, no source text.
    for (tool, fallback) in [(GET, GET_FALLBACK), (REPORT, REPORT_FALLBACK)] {
        let mut r = rig_with(default_pages(), true, Some("uncatalogued_upstream_xyz"));
        let input = if tool == GET {
            get_input()
        } else {
            report_input()
        };
        let out = r.driver.call_at(tool, &input, NOW).unwrap();
        assert_unavailable(&out, fallback, "upstream_unavailable");
        assert!(!json_of(&out).contains("uncatalogued_upstream_xyz"));
        assert_eq!(
            out.get("failure").get("evidenceLocator").code_units(),
            "academy-mcp:owner-read-boundary"
        );
        assert_eq!(r.reads(), 1);
    }
}

// =========================================== registry and registration (5) ===
#[test]
fn registry_entries_state_the_session_loan_route_shape_and_codes() {
    let v2: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/tool-registry-v2.json")).unwrap();
    let find = |name: &str| {
        v2["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()
            .clone()
    };
    for (tool, input, expected_status) in [
        (GET, get_input(), vec!["observed"]),
        (REPORT, report_input(), vec!["observed", "partial"]),
    ] {
        let entry = find(tool);
        assert_eq!(entry["rustRoute"], "session_loan");
        assert_eq!(entry["endpoint"], "dailyrecord");
        let mut r = rig();
        let out = r.driver.call_at(tool, &input, NOW).unwrap();
        let ordered: Vec<String> = entry["outputShape"]["orderedKeys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            keys_of(&out),
            ordered,
            "{tool} orderedKeys equal the emitted keys"
        );
        assert_eq!(
            entry["outputShape"]["statusLiterals"],
            serde_json::json!(expected_status)
        );
        assert!(expected_status.contains(&out.get("status").code_units().as_str()));
        // Every listed code is itself (or the tool fallback) through the projection.
        let fallback = if tool == GET {
            GET_FALLBACK
        } else {
            REPORT_FALLBACK
        };
        for code in entry["errorCodes"].as_array().unwrap() {
            let code = code.as_str().unwrap();
            if code == "tool_input_invalid" {
                continue;
            }
            assert_eq!(
                public_failure(code, fallback).get("code").code_units(),
                code,
                "{tool} lists a code the projection does not keep"
            );
        }
    }
    // The registry description is the current TS registration text (E10
    // decision Q5), never the frozen file.
    let ts = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord-tools.ts"),
    )
    .unwrap();
    for tool in TOOLS {
        let marker = format!("academyRoutingDescription('{tool}','");
        let start = ts.find(&marker).expect("registration") + marker.len();
        let rest = &ts[start..];
        let mut end = 0;
        let bytes = rest.as_bytes();
        while end < bytes.len() && !(bytes[end] == b'\'' && bytes[end - 1] != b'\\') {
            end += 1;
        }
        let description = rest[..end].replace("\\'", "'");
        assert_eq!(find(tool)["description"], description.as_str(), "{tool}");
    }
}

// ============================================ 7c: the 8,000,000-byte bound ===
#[test]
fn the_prestudy_transport_limit_is_the_source_body_bound() {
    assert_eq!(prestudy_transport_limits().response_bytes, 8_000_001);
    assert_eq!(
        prestudy_transport_limits().request_bytes,
        http::TransportLimits::default().request_bytes
    );
    // (body length, refusal code or None when the body is accepted). One
    // byte over the source bound is refused while it is read; a longer
    // Content-Length is refused before the body is read.
    for (len, refusal) in [
        (7_999_999usize, None),
        (8_000_000, None),
        (8_000_001, Some("native_response_read_failed")),
        (8_000_002, Some("source_response_too_large")),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            let header = format!(
                "HTTP/1.1 200 Synthetic\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(header.as_bytes()).unwrap();
            // The over-bound body is refused from its Content-Length before
            // it is read; send only part of it.
            let sent = if len <= 8_000_001 { len } else { 1024 };
            let _ = stream.write_all(&vec![b'x'; sent]);
        });
        let mut transport = http::NativeTransport::synthetic_loopback(
            address,
            prestudy_transport_limits(),
            http::Cancellation::default(),
        )
        .unwrap();
        use http::Transport;
        let outcome = transport.dispatch(&http::Request {
            method: "GET".into(),
            path: "/c_login.jsp".into(),
            query: vec![],
            body: vec![],
            headers: vec![],
            timeout: std::time::Duration::from_secs(10),
        });
        match (refusal, outcome) {
            (None, Ok(response)) => assert_eq!(response.bytes.len(), len),
            (Some(code), Err(e)) => assert_eq!(e.message, code, "len {len}"),
            (_, other) => panic!("len {len}: {:?}", other.map(|r| r.bytes.len())),
        }
        server.join().unwrap();
    }
}
