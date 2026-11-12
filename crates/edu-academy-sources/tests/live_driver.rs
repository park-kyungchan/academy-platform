//! Finite invented documents, scripted transports and offline binary preflight.
use edu_academy_core::{
    js::Result as CoreResult,
    workflow::{
        Runtime,
        native::{NativeBackend, SourcePort as _},
        service::{Backend, Service},
        store::Store,
    },
};
use edu_academy_sources::{
    data::*,
    http::{MAIN, ORIGIN, Request, Response, Transport},
    live_driver::{self, DisconnectedPolicy},
    owner_root::OwnerRoot,
    owner_workflow::{Credentials, Owner},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
};
const NOW: &str = "2040-01-04T09:00:00.000Z";
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        NOW.into()
    }
    fn epoch_ms(&self) -> u64 {
        1000
    }
    fn uuid(&self) -> CoreResult<String> {
        Ok("fictional-driver-uuid".into())
    }
}
struct FiniteHttp {
    replies: VecDeque<(String, u16, String)>,
    seen: Rc<RefCell<Vec<Request>>>,
}
impl Transport for FiniteHttp {
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
fn admission() -> Js {
    edu_academy_core::templates::from_json(
        serde_json::json!({"teacherId":"99001","teacherLabel":"Fictional Teacher","date":"2040-01-04","sourceBundleSha256":"b".repeat(64),"students":[{"studentId":"77001","displayName":"Fictional Learner"}]}),
    )
}
fn input() -> Js {
    edu_academy_core::templates::from_json(
        serde_json::json!({"field":"attendance","requestRef":"fictional-request","value":"N","scope":{"sourceNamespace":"academy.lms.dayrecord","teacherId":"99001","studentId":"77001","lessonDate":"2040-01-04"}}),
    )
}
struct Unused;
impl Backend for Unused {
    fn configured(&self) -> bool {
        false
    }
    fn save(
        &mut self,
        _: &Js,
        _: &Js,
        _: &mut dyn edu_academy_core::workflow::approval::Approval,
        _: &Js,
    ) -> CoreResult<Js> {
        panic!("plan never saves")
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
fn transport(replies: Vec<(String, u16, String)>) -> (FiniteHttp, Rc<RefCell<Vec<Request>>>) {
    let seen = Rc::new(RefCell::new(vec![]));
    (
        FiniteHttp {
            replies: replies.into(),
            seen: Rc::clone(&seen),
        },
        seen,
    )
}
fn principal() -> Js {
    obj(&[
        ("teacherLabel", "Fictional Teacher".into()),
        ("teacherKey", "99001".into()),
        ("collegeKey", "88001".into()),
    ])
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

#[test]
fn product_plan_refuses_wire_with_one_login_and_releases_lease() {
    let root =
        PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap()).join("live-driver-product");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let owner_root = Rc::new(OwnerRoot::open(&root).unwrap());
    let account = hash(format!("{ORIGIN}\0fictional-user").as_bytes());
    for failure in [false, true] {
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
        // discover, current execute + binding, journal execute + binding + wire inspection.
        replies.push((MAIN.into(), if failure { 503 } else { 200 }, html.clone()));
        if !failure {
            for _ in 0..2 {
                replies.push((MAIN.into(), 200, html.clone()));
                replies.push((edu_academy_sources::http::SELECTED.into(),200,"<root><record>!2040-01-04!5!!Fictional Learner!!!!!3!13!77001!8</record></root>".into()));
            }
            replies.push((MAIN.into(), 200, html));
        }
        let repeated = replies[3..].to_vec();
        replies.extend(repeated);
        let (transport, seen) = transport(replies);
        let owner = Owner::leased(
            Rc::clone(&owner_root),
            principal(),
            transport,
            admission(),
            None,
            credentials,
            &Clock,
        )
        .unwrap();
        assert_eq!(
            owner_root.account(&account).err().unwrap().message,
            "owner_account_busy"
        );
        let mut source = owner.source();
        let mut writer = owner.writer();
        let (a, o, n) = owner.write_configuration().unwrap();
        drop(owner);
        assert!(
            owner_root.account(&account).is_err(),
            "views must retain lease"
        );
        {
            let mut backend = NativeBackend::new(&mut writer, a, o, n, &Clock);
            let store = Store::open(&root.join("store"), &root).unwrap();
            let mut svc = Service {
                store: &store,
                source: &mut source,
                backend: &mut backend,
                policy: &DisconnectedPolicy,
                runtime: &Clock,
            };
            let (_, result) = live_driver::prepare(&input(), &mut svc).unwrap();
            private_output_is_absent(&result);
            assert!(result.get("status").is("held"));
            if !failure {
                assert!(
                    result
                        .get("wireGate")
                        .is("journal_mutation_wire_not_qualified")
                );
            }
            let mut request = input();
            request.set("mode", "save".into());
            request.set("approval", "0".repeat(64).into());
            let refused = live_driver::execute(&request, &mut svc).unwrap();
            assert!(
                refused.get("status").is("held"),
                "held save keeps plan cause"
            );
            let (public, exit) = live_driver::response(
                &request,
                Ok(refused.clone()),
                live_driver::stored(&request, &store).unwrap(),
                &Js::Null,
                true,
            );
            assert_eq!(exit, 3);
            assert_eq!(
                public.get("code"),
                result.get("wireGate"),
                "held save reason matches plan"
            );
            assert_eq!(
                public.get("wireGate"),
                result.get("wireGate"),
                "failure retains wire gate"
            );
            if !failure {
                assert!(
                    refused
                        .get("wireGate")
                        .is("journal_mutation_wire_not_qualified")
                );
            }
            assert!(
                edu_academy_core::workflow::state::student_executions(&store, request.get("scope"))
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(seen.borrow().len(), if failure { 5 } else { 15 });
            assert_eq!(
                seen.borrow()
                    .iter()
                    .filter(|r| r.path == "/c_login.jsp")
                    .count(),
                1
            );
            assert!(seen.borrow().iter().all(|r| {
                !r.query
                    .iter()
                    .chain(&r.body)
                    .any(|(k, v)| k == "reqCmd" && v == "udtAttn")
            }));
        }
        writer.authenticate().unwrap();
        writer.authenticate().unwrap();
        drop(writer);
        drop(source);
        assert!(
            owner_root.account(&account).is_ok(),
            "lease retained after read/refusal"
        );
    }
}

struct InventedSource(Rc<RefCell<Js>>, Rc<Cell<usize>>);
impl edu_academy_core::workflow::service::Source for InventedSource {
    fn read(&mut self, _: &Js, _: &Js, _: &Js) -> CoreResult<Js> {
        self.1.set(self.1.get() + 1);
        Ok(obj(&[
            ("status", "observed".into()),
            ("observation", self.0.borrow().clone()),
        ]))
    }
}
struct PanicPolicy;
impl edu_academy_core::workflow::review::Policy for PanicPolicy {
    fn parent_pins(&self) -> Js {
        Js::Null
    }
    fn learning_pins(&self) -> Js {
        Js::Null
    }
    fn check_lines(&self, _: &Js) -> bool {
        panic!("policy_check_lines_called")
    }
    fn excluded_parent_language(&self, _: &Js) -> bool {
        panic!("policy_excluded_language_called")
    }
    fn scope_qualified(&self, _: &Js) -> bool {
        panic!("policy_scope_called")
    }
    fn learning_review(&self, _: &Js, _: &Js, _: &Js) -> Vec<Js> {
        panic!("policy_learning_called")
    }
}
fn observation() -> Js {
    let scope = input().get("scope").clone();
    let mut target = scope.clone();
    for (k, v) in [("courseId", "5"), ("membershipId", "8"), ("recordId", "13")] {
        target.set(k, v.into());
    }
    let mut previous = target.clone();
    previous.set("lessonDate", "2040-01-02".into());
    previous.set("recordId", "12".into());
    let mut fields = obj(&[]);
    for f in edu_academy_core::workflow::review::FIELDS {
        let value = match f {
            "attendance" => "Y",
            "dailyTestAssessment" | "previousHomeworkAssessment" => "0",
            _ => "- Invented before",
        };
        fields.set(
            f,
            obj(&[
                ("state", "known".into()),
                ("value", value.into()),
                ("evidenceRefs", Js::Arr(vec!["invented-read".into()])),
                (
                    "effectRecordKey",
                    if f == "previousHomeworkAssessment" {
                        "12"
                    } else {
                        "13"
                    }
                    .into(),
                ),
                (
                    "effectTarget",
                    edu_academy_core::workflow::canonical(if f == "previousHomeworkAssessment" {
                        &previous
                    } else {
                        &target
                    })
                    .unwrap()
                    .into(),
                ),
            ]),
        );
    }
    obj(&[
        ("scope", scope),
        ("target", target),
        ("previousTarget", previous),
        ("observedAt", NOW.into()),
        ("sourceRevision", "1".repeat(64).into()),
        ("wireDigest", "2".repeat(64).into()),
        ("fields", fields),
        ("toneSamples", Js::Arr(vec![])),
        ("transmissionState", "not_sent".into()),
    ])
}
#[test]
fn supported_review_matches_with_panicking_validation_policy() {
    let root =
        PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap()).join("live-driver-core");
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let store = Store::open(&root.join("store"), &root).unwrap();
    let mut source = InventedSource(Rc::new(RefCell::new(observation())), Rc::new(Cell::new(0)));
    let mut backend = Unused;
    let mut service = Service {
        store: &store,
        source: &mut source,
        backend: &mut backend,
        policy: &DisconnectedPolicy,
        runtime: &Clock,
    };
    for field in [
        "attendance",
        "dailyTestAssessment",
        "previousHomeworkAssessment",
    ] {
        let mut request = input();
        request.set("field", field.into());
        request.set(
            "value",
            if field == "attendance" { "N" } else { "3" }.into(),
        );
        service.policy = &DisconnectedPolicy;
        let (saved, result) = live_driver::prepare(&request, &mut service).unwrap();
        assert!(result.get("status").is("prepared"), "{result:?}");
        assert_eq!(result.get("planDigest").code_units().len(), 64);
        assert_eq!(result.get("otherFieldsSha256").props().len(), 5);
        assert_eq!(result.get("identitySha256").props().len(), 4);
        let review = obj(&[
            ("context", saved.get("context").clone()),
            ("observationHandle", saved.get("observationHandle").clone()),
            ("proposals", saved.get("proposals").clone()),
        ]);
        service.policy = &PanicPolicy;
        let compared = service.review(&review, true).unwrap();
        assert!(compared.get("status").is("prepared"));
        assert_eq!(compared.get("reviewDigest"), saved.get("reviewDigest"));
    }
}

struct TestClock(Cell<&'static str>, Cell<u64>);
impl Runtime for TestClock {
    fn now(&self) -> String {
        self.0.get().into()
    }
    fn epoch_ms(&self) -> u64 {
        1000
    }
    fn uuid(&self) -> CoreResult<String> {
        self.1.set(self.1.get() + 1);
        Ok(format!("invented-{}", self.1.get()))
    }
}
struct SyntheticPort {
    observation: Rc<RefCell<Js>>,
    writes: Rc<Cell<usize>>,
    immediate: Option<&'static str>,
    lost: bool,
    wrong_receipt: bool,
}
impl edu_academy_core::workflow::http::HttpPort for SyntheticPort {
    fn read(&mut self, _: &Js) -> CoreResult<Js> {
        let observation = self.observation.borrow();
        let mut fields = obj(&[]);
        let mut targets = obj(&[]);
        for f in edu_academy_core::workflow::review::FIELDS {
            fields.set(f, observation.get("fields").get(f).get("value").clone());
            targets.set(
                f,
                observation.get("fields").get(f).get("effectTarget").clone(),
            );
        }
        Ok(obj(&[
            (
                "target",
                edu_academy_core::templates::from_json(
                    serde_json::json!({"studentName":"Fictional Learner","date":"2040-01-04","groupId":"3","teacherKey":"99001","studentKey":"77001","courseKey":"5","recordKey":"13","membershipKey":"8"}),
                ),
            ),
            ("fields", fields),
            ("effectTargets", targets),
            ("previousRecordKey", "12".into()),
            ("previousDate", "2040-01-02".into()),
            ("wireDigest", "2".repeat(64).into()),
        ]))
    }
    fn write(&mut self, _: &Js) -> CoreResult<Js> {
        self.writes.set(self.writes.get() + 1);
        if let Some(value) = self.immediate {
            set_field(&self.observation, "attendance", value);
        }
        if self.lost {
            return Err(edu_academy_core::js::Fault::error("invented_lost_answer"));
        }
        Ok(obj(&[
            ("httpStatus", Js::Num(200.)),
            ("bodySha256", hash(b"invented returned body").into()),
        ]))
    }
}
impl edu_academy_core::workflow::native::SourcePort for SyntheticPort {
    fn authenticate(&mut self) -> CoreResult<()> {
        Ok(())
    }
    fn discover_groups(&mut self, _: &Js) -> CoreResult<Js> {
        Ok(edu_academy_core::templates::from_json(
            serde_json::json!({"groups":[{"groupId":"3"}]}),
        ))
    }
    fn execute(&mut self, _: &Js) -> CoreResult<Js> {
        use edu_academy_core::workflow::http::HttpPort;
        let state = self.read(&Js::Null)?;
        Ok(obj(&[(
            "dayRecords",
            Js::Arr(vec![state.get("target").clone()]),
        )]))
    }
    fn journal_state(&mut self, target: &Js) -> CoreResult<Js> {
        edu_academy_core::workflow::http::HttpPort::read(self, target)
    }
    fn apply(
        &mut self,
        action: &Js,
        approval: &mut dyn edu_academy_core::workflow::approval::Approval,
    ) -> CoreResult<Js> {
        let mut result = edu_academy_core::workflow::http::apply(action, self, approval)?;
        if self.wrong_receipt {
            let mut receipt = result.get("receipt").clone();
            receipt.set("actionDigest", "f".repeat(64).into());
            result.set("receipt", receipt);
        }
        Ok(result)
    }
}
fn set_field(observation: &Rc<RefCell<Js>>, field: &str, value: &str) {
    let mut observation = observation.borrow_mut();
    let mut fields = observation.get("fields").clone();
    let mut binding = fields.get(field).clone();
    binding.set("value", value.into());
    fields.set(field, binding);
    observation.set("fields", fields);
}
#[test]
fn modes_use_native_apply_and_status_settles_field_alone_at_due_time() {
    for (index, immediate, lost, late, expected) in [
        (0, Some("N"), false, "N", "settled_persisted"),
        (1, None, false, "N", "reconciled_persisted"),
        (2, None, false, "Y", "reconciled_not_persisted"),
        (3, None, false, "L", "reconciled_conflict"),
        (4, Some("N"), true, "N", "reconciled_persisted"),
        (5, Some("N"), false, "N", "reconciled_persisted"),
    ] {
        let root = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
            .join(format!("live-driver-mode-{index}"));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let store = Store::open(&root.join("store"), &root).unwrap();
        let observation = Rc::new(RefCell::new(observation()));
        let reads = Rc::new(Cell::new(0));
        let writes = Rc::new(Cell::new(0));
        let mut source = InventedSource(Rc::clone(&observation), Rc::clone(&reads));
        let mut port = SyntheticPort {
            observation: Rc::clone(&observation),
            writes: Rc::clone(&writes),
            immediate,
            lost,
            wrong_receipt: index == 5,
        };
        let clock = TestClock(Cell::new(NOW), Cell::new(0));
        let mut evidence = live_driver::EvidencePort {
            inner: &mut port,
            evidence: Js::Null,
        };
        let mut native = NativeBackend::new(&mut evidence, admission(), Js::Null, vec![], &clock);
        let mut service = Service {
            store: &store,
            source: &mut source,
            backend: &mut native,
            policy: &DisconnectedPolicy,
            runtime: &clock,
        };
        let mut input = input();
        input.set("mode", "plan".into());
        let plan = live_driver::execute(&input, &mut service).unwrap();
        assert!(plan.get("status").is("prepared"));
        private_output_is_absent(&plan);
        input.set("mode", "save".into());
        input.set("approval", "0".repeat(64).into());
        let mismatch = live_driver::execute(&input, &mut service).unwrap();
        assert!(mismatch.get("code").is("approval_digest_mismatch"));
        assert_eq!(writes.get(), 0);
        private_output_is_absent(&mismatch);
        input.set("approval", plan.get("planDigest").clone());
        let save = live_driver::execute(&input, &mut service).unwrap();
        let save_input = input.clone();
        let stored_after_save = live_driver::stored(&input, &store).unwrap();
        assert_eq!(writes.get(), 1, "exactly one dispatch: {save:?}");
        private_output_is_absent(&save);
        assert!(
            save.get("executions").array()[0]
                .get("actionDigest")
                .string()
        );
        if index != 0 {
            assert!(save.get("status").is("held"));
            input.set("mode", "status".into());
            input.set("reconcile", true.into());
            clock.0.set("2040-01-04T09:09:59.999Z");
            let before = reads.get();
            let status = live_driver::execute(&input, &mut service).unwrap();
            assert_eq!(reads.get(), before, "status before due time must not read");
            assert!(
                status.get("reconcileStatus").is("held"),
                "before due core status is held"
            );
            assert!(
                status.get("reconcileCode").is("save_confirmation_pending"),
                "before due code is pending"
            );
            let (_, exit) = live_driver::response(
                &input,
                Ok(status.clone()),
                live_driver::stored(&input, &store).unwrap(),
                &Js::Null,
                true,
            );
            assert_eq!(exit, 0, "not-yet-due wait is successful");
            assert!(
                status.get("executions").array()[0]
                    .get("executionState")
                    .is("reconciliation_required")
            );
            clock.0.set("2040-01-04T09:10:00.000Z");
            if index == 1 {
                let mut replies = login();
                replies.push((MAIN.into(), 503, "invented unavailable response".into()));
                let (transport, seen503) = transport(replies);
                let owner503 = Owner::new(
                    edu_academy_sources::http::Client::new(principal(), transport).unwrap(),
                    admission(),
                    None,
                    vec![],
                    credentials,
                    &clock,
                )
                .unwrap();
                let mut source503 = owner503.source();
                let mut unused = Unused;
                let failed = live_driver::execute(
                    &input,
                    &mut Service {
                        store: &store,
                        source: &mut source503,
                        backend: &mut unused,
                        policy: &DisconnectedPolicy,
                        runtime: &clock,
                    },
                )
                .unwrap();
                assert!(
                    failed.get("reconcileStatus").is("held"),
                    "HTTP 503 reconcile is held"
                );
                assert!(
                    failed
                        .get("reconcileCode")
                        .is("readback_unavailable_reconcile_later"),
                    "HTTP 503 reconcile reason retained"
                );
                let (failure, exit) = live_driver::response(
                    &input,
                    Ok(failed),
                    live_driver::stored(&input, &store).unwrap(),
                    &Js::Null,
                    true,
                );
                assert_eq!(exit, 3, "HTTP 503 reconcile must fail the run");
                assert!(
                    failure
                        .get("code")
                        .is("readback_unavailable_reconcile_later")
                );
                assert_eq!(
                    seen503.borrow().len(),
                    4,
                    "one login and one failed settling read"
                );
                assert!(
                    failure.get("stored").get("executions").array()[0]
                        .get("executionState")
                        .is("reconciliation_required")
                );
            }
            set_field(&observation, "attendance", late);
            set_field(&observation, "homework", "- Invented sibling changed");
            let settled = live_driver::execute(&input, &mut service).unwrap();
            private_output_is_absent(&settled);
            assert!(
                settled.get("executions").array()[0]
                    .get("executionState")
                    .is(expected),
                "{settled:?}"
            );
        } else {
            assert!(save.get("status").is("saved"));
            let mut status_input = input.clone();
            status_input.set("mode", "status".into());
            let local = live_driver::execute(&status_input, &mut service).unwrap();
            assert_eq!(
                local.get("reconcileStatus"),
                &Js::Null,
                "no reconcile status when not requested"
            );
            assert_eq!(
                local.get("reconcileCode"),
                &Js::Null,
                "no reconcile code when not requested"
            );
        }
        assert_eq!(writes.get(), 1, "no write retry");
        let _ = service;
        let _ = native;
        private_output_is_absent(&evidence.evidence);
        let (public, exit) = live_driver::response(
            &save_input,
            Ok(save),
            stored_after_save,
            &evidence.evidence,
            true,
        );
        if index == 1 {
            assert_eq!(exit, 3);
            assert_eq!(
                public.get("httpStatus"),
                &Js::Num(200.),
                "pending save retains HTTP status"
            );
            assert!(
                public
                    .get("bodySha256")
                    .is(&hash(b"invented returned body")),
                "pending save retains body hash"
            );
            assert!(
                public.get("comparison").get("attendance").is("not_equal"),
                "pending save retains comparison"
            );
            assert_eq!(public.get("comparison").props().len(), 6);
            private_output_is_absent(&public);
        }
        assert_eq!(evidence.evidence.get("comparison").props().len(), 6);
        assert!(evidence.evidence.get("comparison").get("attendance").is(
            if immediate == Some("N") {
                "equal"
            } else {
                "not_equal"
            }
        ));
    }
}

fn private_output_is_absent(value: &Js) {
    let encoded = stringify(value).unwrap();
    for secret in [
        "Y",
        "N",
        "0",
        "- Invented before",
        "- Invented sibling changed",
        "Fictional Learner",
        "fictional-user",
        "fictional-password",
    ] {
        assert!(
            !encoded.contains(&stringify(&secret.into()).unwrap()),
            "unsafe_fixture_token_in_output"
        );
    }
}

// The native binary gets only malformed synthetic pins: even a removed preflight
// guard cannot reach login or a network dispatch.
fn binary_case(case: &str, expected: &str) {
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
    };
    let root = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("driver-binary-{case}"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let poison = "SYNTHETIC_FILE_CONTENT_NEVER_OUTPUT_948271";
    for name in ["transport.json", "credential.json", "launch.json"] {
        std::fs::write(root.join(name), poison).unwrap();
        std::fs::set_permissions(root.join(name), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut v: serde_json::Value =
        serde_json::from_str(&edu_academy_core::workflow::canonical(&input()).unwrap()).unwrap();
    v["mode"] = "plan".into();
    v["requestRef"] = "request-secret-never-output-129831".into();
    v["ownerRoot"] = root.to_str().unwrap().into();
    v["store"] = root.join("store").to_str().unwrap().into();
    for (key, name) in [
        ("admission", "transport.json"),
        ("credential", "credential.json"),
        ("launchAdmission", "launch.json"),
    ] {
        v[key] = serde_json::json!({"relativePath":name,"sha256":"0".repeat(64)});
    }
    match case {
        "invalid" => {
            v["mode"] = "invalid-mode".into();
            v["ownerRoot"] = root.join("absent").to_str().unwrap().into();
        }
        "save-invalid" => {
            v["mode"] = "save".into();
            v["approval"] = "0".repeat(64).into();
            v["ownerRoot"] = 42.into();
        }
        "narrative" => {
            v["field"] = "progress".into();
            v["ownerRoot"] = root.join("absent").to_str().unwrap().into();
        }
        "absent" => v["ownerRoot"] = root.join("absent").to_str().unwrap().into(),
        "git" => {
            std::fs::create_dir_all(root.join("repo/.git")).unwrap();
            v["store"] = root.join("repo/store").to_str().unwrap().into();
        }
        "symlink" | "child-mode" => {
            std::fs::create_dir_all(root.join("store")).unwrap();
            std::fs::set_permissions(root.join("store"), std::fs::Permissions::from_mode(0o700))
                .unwrap();
            if case == "symlink" {
                std::fs::create_dir_all(root.join("outside")).unwrap();
                std::os::unix::fs::symlink(root.join("outside"), root.join("store/escaped"))
                    .unwrap();
            } else {
                std::fs::create_dir_all(root.join("store/wide")).unwrap();
                std::fs::set_permissions(
                    root.join("store/wide"),
                    std::fs::Permissions::from_mode(0o750),
                )
                .unwrap();
            }
        }
        "mode" => {
            std::fs::create_dir_all(root.join("store")).unwrap();
            std::fs::set_permissions(root.join("store"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        _ => unreachable!(),
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
        .write_all(v.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "{case}: pre-network refusal exit"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("ONE JSON object and no trailing stdout");
    assert!(parsed.is_object());
    if ["plan", "save"].contains(&v["mode"].as_str().unwrap()) {
        assert!(
            parsed.get("wireGate").is_some(),
            "preflight keeps wire gate key"
        );
    }
    if v["mode"] == "save" {
        for key in ["httpStatus", "bodySha256", "comparison"] {
            assert_eq!(
                parsed.get(key),
                Some(&serde_json::Value::Null),
                "preflight save evidence is explicitly null"
            );
        }
    }
    assert_eq!(parsed["status"], "refused", "{case}: refusal status");
    assert_eq!(parsed["code"], expected, "{case}: guard code");
    assert_eq!(
        parsed["stored"],
        serde_json::Value::Null,
        "{case}: store not opened"
    );
    assert_eq!(stderr, format!("{expected}\n"), "typed code only on stderr");
    for secret in [
        v["value"].as_str().unwrap(),
        v["requestRef"].as_str().unwrap(),
        poison,
        "credential.json",
    ] {
        assert!(
            !stdout.contains(secret) && !stderr.contains(secret),
            "{case}: private bytes escaped"
        );
    }
    if case == "narrative" {
        assert!(
            !root.join("store").exists(),
            "narrative: root/store untouched"
        );
    }
}
#[test]
fn binary_invalid_input() {
    binary_case("invalid", "invalid_input");
}
#[test]
fn binary_narrative_before_root() {
    binary_case("narrative", "field_not_supported");
}
#[test]
fn binary_nonexistent_root() {
    binary_case("absent", "owner_root_walk_refused");
}
#[test]
fn binary_git_store() {
    binary_case("git", "store_inside_git_refused");
}
#[test]
fn binary_wide_store() {
    binary_case("mode", "store_mode_refused");
}

#[test]
fn binary_store_symlink_child() {
    binary_case("symlink", "store_tree_refused");
}
#[test]
fn binary_store_wide_child() {
    binary_case("child-mode", "store_tree_refused");
}

#[test]
fn plan_digest_binds_value_date_and_field() {
    let root = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap()).join("digest-binding");
    std::fs::create_dir_all(&root).unwrap();
    let store = Store::open(&root.join("store"), &root).unwrap();
    let observation = Rc::new(RefCell::new(observation()));
    let mut source = InventedSource(observation.clone(), Rc::new(Cell::new(0)));
    let mut unused = Unused;
    let clock = TestClock(Cell::new(NOW), Cell::new(0));
    let mut service = Service {
        store: &store,
        source: &mut source,
        backend: &mut unused,
        policy: &DisconnectedPolicy,
        runtime: &clock,
    };
    let mut request = input();
    request.set("mode", "plan".into());
    let original = live_driver::execute(&request, &mut service).unwrap();
    assert!(original.get("status").is("prepared"));
    request.set("mode", "save".into());
    request.set("approval", original.get("planDigest").clone());
    request.set("value", "L".into());
    let changed_value = live_driver::execute(&request, &mut service).unwrap();
    assert!(
        changed_value.get("code").is("approval_digest_mismatch"),
        "plan for X must refuse value Y before backend/write"
    );
    assert!(
        edu_academy_core::workflow::state::student_executions(&store, request.get("scope"))
            .unwrap()
            .is_empty()
    );
    request.set("mode", "plan".into());
    request.set("field", "dailyTestAssessment".into());
    request.set("value", "1".into());
    let changed_field = live_driver::execute(&request, &mut service).unwrap();
    assert!(changed_field.get("status").is("prepared"));
    assert_ne!(
        original.get("planDigest"),
        changed_field.get("planDigest"),
        "digest changes with field"
    );
    // Make values identical so the field-only control cannot pass due to value hashes.
    request.set("field", "previousHomeworkAssessment".into());
    let other_field = live_driver::execute(&request, &mut service).unwrap();
    assert!(other_field.get("status").is("prepared"));
    assert_ne!(
        changed_field.get("planDigest"),
        other_field.get("planDigest"),
        "digest binds distinct assessment fields"
    );
    request.set("field", "attendance".into());
    request.set("value", "N".into());
    let mut scope = request.get("scope").clone();
    scope.set("lessonDate", "2040-01-05".into());
    request.set("scope", scope.clone());
    let mut obs = observation.borrow_mut();
    obs.set("scope", scope);
    let mut target = obs.get("target").clone();
    target.set("lessonDate", "2040-01-05".into());
    obs.set("target", target.clone());
    let mut fields = obs.get("fields").clone();
    for field in ["attendance", "dailyTestAssessment"] {
        let mut binding = fields.get(field).clone();
        binding.set(
            "effectTarget",
            edu_academy_core::workflow::canonical(&target)
                .unwrap()
                .into(),
        );
        fields.set(field, binding);
    }
    obs.set("fields", fields);
    obs.set("observedAt", "2040-01-05T09:00:00.000Z".into());
    drop(obs);
    clock.0.set("2040-01-05T09:00:00.000Z");
    let changed_date = live_driver::execute(&request, &mut service).unwrap();
    assert!(
        changed_date.get("status").is("prepared"),
        "changed date has valid matching observation"
    );
    assert_ne!(
        original.get("planDigest"),
        changed_date.get("planDigest"),
        "digest changes with lesson date"
    );
}

fn binary_admission_case(case: &str, expected: &str) {
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
    };
    let root = PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap())
        .join(format!("admission-{case}"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut transport = serde_json::json!({"schemaVersion":"academy.transport-admission/1", "ownerId":"synthetic-owner", "teacherId":"99001", "teacherLabel":"Fictional Teacher", "collegeId":"88001", "allowedFrom":"2040-01-01", "allowedThrough":"2040-01-31", "studentIds":["77001"], "sourceNamespaces":["academy.lms.dayrecord"], "operations":[{"id":"inert", "sourceNamespace":"academy.lms.dayrecord", "method":"GET", "path":"/synthetic/inert", "effect":false, "query":[], "body":[]}]});
    let mut launch: serde_json::Value =
        serde_json::from_str(&edu_academy_core::workflow::canonical(&admission()).unwrap())
            .unwrap();
    match case {
        "missing" => {
            launch.as_object_mut().unwrap().remove("teacherLabel");
        }
        "date" => launch["date"] = "2040-02-30".into(),
        "duplicate" => {
            let duplicate = launch["students"][0].clone();
            launch["students"].as_array_mut().unwrap().push(duplicate);
        }
        "identity" => launch["teacherId"] = "99002".into(),
        "label" => launch["teacherLabel"] = "Other Fictional Teacher".into(),
        "student" => launch["students"][0]["studentId"] = "77002".into(),
        "scope-student" => transport["studentIds"] = serde_json::json!(["77002"]),
        "scope-date" => transport["allowedThrough"] = "2040-01-03".into(),
        "scope-namespace" => {
            transport["sourceNamespaces"] = serde_json::json!(["academy.lms.metadata"]);
            transport["operations"][0]["sourceNamespace"] = "academy.lms.metadata".into();
        }
        "scope-input-namespace" => (),
        "extras" => {
            launch["extraIgnored"] = true.into();
        }
        _ => unreachable!(),
    }
    let mut request: serde_json::Value =
        serde_json::from_str(&edu_academy_core::workflow::canonical(&input()).unwrap()).unwrap();
    request["mode"] = "plan".into();
    if case == "scope-input-namespace" {
        request["scope"]["sourceNamespace"] = "academy.lms.metadata".into();
    }
    request["ownerRoot"] = root.to_str().unwrap().into();
    request["store"] = root.join("store").to_str().unwrap().into();
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
        "admission refusal must precede login"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["code"], expected, "{case}: admission guard code");
    assert_eq!(stderr, format!("{expected}\n"));
    assert!(value["stored"].is_object());
    for private in [
        "Fictional Teacher",
        "Fictional Learner",
        "fictional-request",
        "synthetic_invalid_credential",
        "credential.json",
    ] {
        assert!(
            !stdout.contains(private) && !stderr.contains(private),
            "admission fixture bytes escaped"
        );
    }
}
#[test]
fn binary_launch_validation_precedes_login() {
    for case in [
        "missing",
        "date",
        "duplicate",
        "identity",
        "label",
        "student",
    ] {
        binary_admission_case(case, "launch_admission_refused");
    }
}
#[test]
fn binary_transport_scope_precedes_login() {
    for case in [
        "scope-student",
        "scope-date",
        "scope-namespace",
        "scope-input-namespace",
    ] {
        binary_admission_case(case, "native_transport_scope_refused");
    }
}
#[test]
fn binary_launch_extra_keys_and_inert_operations_are_ignored() {
    binary_admission_case("extras", "native_owner_credential_refused");
}

#[test]
fn binary_save_preflight_keeps_null_evidence() {
    binary_case("save-invalid", "invalid_input");
}
