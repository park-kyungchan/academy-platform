// Fictional transport only. The public-entry tests retain the immutable wire gate.
// Real private-write unit tests live beside the private adapter in http.rs.
// Successful public approval/write/after-read proof remains PENDING.
use edu_academy_core::{js::Result as CoreResult, workflow::approval::Approval};
use edu_academy_sources as sources;
use sources::{
    data::*,
    http::{Client, MAIN, ORIGIN, Request, Response, SELECTED, Transport},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

const NOW: &str = "2040-01-04T09:00:00.000Z";
const WIRE: &str = "650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1";
type Trace = Rc<RefCell<Vec<String>>>;

fn target() -> Js {
    obj(&[
        ("studentName", "Fictional Port Learner 😀".into()),
        ("date", "2040-01-04".into()),
        ("groupId", "3".into()),
        ("teacherKey", "99001".into()),
        ("studentKey", "77001".into()),
        ("courseKey", "5".into()),
        ("recordKey", "13".into()),
        ("membershipKey", "8".into()),
    ])
}
fn initial_fields() -> Js {
    obj(&[
        ("progress", "Current".into()),
        ("homework", "Homework".into()),
        ("lessonMemo", "Memo".into()),
        ("attendance", "Y".into()),
        ("dailyTestAssessment", "0".into()),
        ("previousHomeworkAssessment", "3".into()),
        ("persistentMemo", "preserved".into()),
    ])
}
fn action(field: &str, after: &str) -> Js {
    obj(&[
        ("attemptId", "fictional-port-1".into()),
        ("target", target()),
        ("field", field.into()),
        (
            "effectRecordKey",
            if field == "previousHomeworkAssessment" {
                "12"
            } else {
                "13"
            }
            .into(),
        ),
        ("before", initial_fields().get(field).clone()),
        ("after", after.into()),
        ("wireDigest", WIRE.into()),
    ])
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn main_html(fields: &Js) -> String {
    // Own synthetic document, with exact source-observed signatures/column roles.
    // Inert definitions intentionally do not hash to the qualified private source.
    let mut html = format!(
        r#"
<input name="std_date" value="2040-01-04"><input name="teacher_pri_no" value="99001">
<select name="grp_seq"><option value="3" selected>Fictional group</option></select>
<table><thead><tr><th rowspan="2">이름</th><th rowspan="2">과정</th><th rowspan="2">지속사항</th><th colspan="4">지난진도</th><th colspan="6">일괄 발송 금일진도</th></tr>
<tr><th>지난수업일</th><th>진도</th><th>숙제</th><th>메모</th><th></th><th>금일출석</th><th>일일테스트</th><th>진도</th><th>숙제</th><th>메모</th></tr></thead>
<tbody><tr><td><a onclick="onOpenDaysMain('77001')">Fictional Port Learner 😀</a></td><td>Fictional course</td>
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
fn xml(record: &str, previous: bool) -> String {
    format!(
        "<root><record>!{}!5!!Fictional Port Learner 😀!!!!!3!{record}!77001!8</record></root>",
        if previous { "2040-01-02" } else { "2040-01-04" }
    )
}
struct Fictional {
    fields: Js,
    requests: Vec<Request>,
    trace: Trace,
    writes: usize,
    first_status: Option<u16>,
    response_cookie: &'static str,
}
impl Fictional {
    fn new(trace: Trace) -> Self {
        Self {
            fields: initial_fields(),
            requests: vec![],
            trace,
            writes: 0,
            first_status: None,
            response_cookie: "JSESSIONID=fictional-port-session; Path=/; Secure",
        }
    }
}
impl Transport for Fictional {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        self.requests.push(r.clone());
        assert_eq!(r.timeout, Duration::from_secs(20));
        let headers = vec![("Set-Cookie".into(), self.response_cookie.into())];
        if let Some(status) = self.first_status.take() {
            self.trace.borrow_mut().push("read:http_failure".into());
            return Ok(Response {
                status,
                headers,
                bytes: vec![],
                url: format!("{ORIGIN}{}", r.path),
                stalled: false,
            });
        }
        let selected = r
            .query
            .iter()
            .chain(&r.body)
            .find(|(k, _)| k == "reqCmd")
            .map(|(_, v)| v.as_str());
        let status = 200;
        let source = if r.method == "POST"
            && r.path == MAIN
            && r.body.first() == Some(&("p_process".into(), "Main".into()))
        {
            self.trace.borrow_mut().push("read:main".into());
            main_html(&self.fields)
        } else if r.path == SELECTED && selected == Some("SelSglRecord") {
            let record = r
                .query
                .iter()
                .find(|(k, _)| k == "record_seq")
                .unwrap()
                .1
                .as_str();
            assert!(record == "12" || record == "13");
            self.trace.borrow_mut().push(format!("read:{record}"));
            xml(record, record == "12")
        } else {
            self.writes += 1;
            panic!("unexpected fictional request: public refusal must precede any mutation");
        };
        Ok(Response {
            status,
            headers,
            bytes: source.into_bytes(),
            url: format!("{ORIGIN}{}", r.path),
            stalled: false,
        })
    }
}
struct Decision {
    calls: usize,
}
impl Approval for Decision {
    fn consume(&mut self, _: &Js) -> CoreResult<Js> {
        self.calls += 1;
        panic!("public refusal must precede approval consumption");
    }
}
fn setup() -> (Client<Fictional>, Decision, Trace) {
    let trace = Rc::new(RefCell::new(vec![]));
    let mut c = Client::new(
        obj(&[
            ("teacherLabel", "Fictional Port Teacher".into()),
            ("teacherKey", "99001".into()),
            ("collegeKey", "88001".into()),
        ]),
        Fictional::new(trace.clone()),
    )
    .unwrap();
    // Explicit synthetic auth state, not evidence of real authentication.
    c.authenticated = true;
    let decision = Decision { calls: 0 };
    (c, decision, trace)
}

#[test]
fn public_validation_principal_and_authentication_refuse_before_transport() {
    let (mut c, mut d, _) = setup();
    c.authenticated = false;
    let error = c.apply_journal_action(&Js::Null, &mut d, NOW).unwrap_err();
    assert_eq!(error.name, "TypeError");
    assert_eq!(
        error.message,
        "null is not an object (evaluating 'Object.keys(value)')"
    );
    let mut a = action("progress", "After");
    let mut t = target();
    t.set("teacherKey", "99002".into());
    a.set("target", t);
    assert_eq!(
        c.apply_journal_action(&a, &mut d, NOW).unwrap_err().message,
        "journal_principal_conflict"
    );
    assert_eq!(
        c.apply_journal_action(&action("progress", "After"), &mut d, NOW)
            .unwrap_err()
            .message,
        "authentication_required"
    );
    c.authenticated = true;
    let mut a = action("attendance", "invalid");
    assert_eq!(
        c.apply_journal_action(&a, &mut d, NOW).unwrap_err().message,
        "journal_http_attendance_rejected"
    );
    a = action("progress", "After");
    a.set("unexpected", true.into());
    assert_eq!(
        c.apply_journal_action(&a, &mut d, NOW).unwrap_err().message,
        "unexpected or missing domain fields"
    );
    assert!(c.transport.requests.is_empty());
    assert_eq!(d.calls, 0);
}
#[test]
fn public_before_read_preserves_typed_http_error_properties() {
    let (mut c, mut d, trace) = setup();
    c.transport.first_status = Some(503);
    let error = c
        .apply_journal_action(&action("progress", "After"), &mut d, NOW)
        .unwrap_err();
    assert_eq!(error.name, "AcademyHttpFailure");
    assert_eq!(error.message, "source_http_failed");
    assert_eq!(
        error.properties,
        obj(&[
            ("sourceOperation", "Main".into()),
            ("httpStatus", Js::Num(503.0))
        ])
    );
    assert_eq!(*trace.borrow(), vec!["read:http_failure"]);
    assert_eq!(d.calls, 0);
    assert_eq!(c.transport.writes, 0);
}
#[test]
fn public_before_read_preserves_cookie_and_authentication_refusals() {
    let (mut c, mut d, _) = setup();
    c.transport.response_cookie = "JSESSIONID=fictional; Domain=foreign.invalid; Path=/";
    assert_eq!(
        c.apply_journal_action(&action("progress", "After"), &mut d, NOW)
            .unwrap_err()
            .message,
        "foreign_session_cookie_rejected"
    );
    assert_eq!(c.transport.requests.len(), 1);
    assert_eq!(d.calls, 0);
    let (mut c, mut d, _) = setup();
    c.transport.first_status = Some(403);
    assert_eq!(
        c.apply_journal_action(&action("progress", "After"), &mut d, NOW)
            .unwrap_err()
            .message,
        "authentication_required"
    );
    assert!(!c.authenticated);
    assert_eq!(c.transport.writes, 0);
    assert_eq!(d.calls, 0);
}
#[test]
fn public_synthetic_wire_cannot_authorize_any_of_the_six_fields_or_noop() {
    for (field, after) in [
        ("progress", "After"),
        ("homework", "After"),
        ("lessonMemo", "After"),
        ("attendance", "N"),
        ("dailyTestAssessment", "10"),
        ("previousHomeworkAssessment", "5"),
        ("progress", "Current"),
    ] {
        let (mut c, mut d, trace) = setup();
        let error = c
            .apply_journal_action(&action(field, after), &mut d, NOW)
            .unwrap_err();
        assert_eq!(
            error.message, "journal_mutation_wire_not_qualified",
            "{field}"
        );
        assert_eq!(*trace.borrow(), vec!["read:main", "read:13", "read:main"]);
        assert_eq!(c.transport.writes, 0);
        assert_eq!(d.calls, 0);
    }
}
