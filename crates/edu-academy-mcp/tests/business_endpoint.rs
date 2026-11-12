//! SOURCE-ONLY: all cases UNEXECUTED. Root supplies an existing canonical absolute
//! S11_BUSINESS_TEST_ROOT. Fictional files are retained; no children/network/native roots.
use edu_academy_core::{
    js::{Fault, Result as CoreResult},
    workflow::{
        Runtime,
        approval::Approval,
        codec,
        native::NativeBackend,
        review,
        service::{self, Service},
        state,
        store::{Kind, Store},
    },
};
use edu_academy_mcp::{
    Js, business::DailyRecord, digest, encode, obj, parse, protocol::RpcSession,
    release::ReleaseGraph,
};
use edu_academy_sources::data::string;
use edu_academy_sources::{
    data::Result as SourceResult,
    http::{Client, MAIN, ORIGIN, Request, Response, SELECTED, Transport},
    owner_workflow::{CredentialProvider, Credentials, Owner},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
const NOW: &str = "2040-01-04T12:00:00.000Z";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Clock(Cell<u64>);
impl Runtime for Clock {
    fn now(&self) -> String {
        NOW.into()
    }
    fn epoch_ms(&self) -> u64 {
        1000
    }
    fn uuid(&self) -> CoreResult<String> {
        let n = self.0.get() + 1;
        self.0.set(n);
        Ok(format!("00000000-0000-4000-8000-{n:012}"))
    }
}
fn js(v: Value) -> Js {
    codec::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn admission() -> Js {
    js(
        json!({"teacherId":"99001","teacherLabel":"Fictional Teacher","date":"2040-01-04",
    "sourceBundleSha256":"b".repeat(64),"students":[{"studentId":"77001","displayName":"Fictional Learner"}]}),
    )
}
fn routing_admission() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("allowedFrom", "2040-01-04".into()),
        ("allowedThrough", "2040-01-04".into()),
    ])
}
fn owner_config() -> Js {
    js(
        json!({"schemaVersion":"academy.owner-admission/2","decider":"user",
    "decidedAt":"2040-01-01","sourceNamespace":"academy.lms.dayrecord","teacherId":"99001",
    "failClosedDefault":"2026-10-23","allowedFrom":null,"allowedThrough":null,"studentScope":"source_verified"}),
    )
}
fn source_verified_routing() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("allowedFrom", Js::Null),
        ("allowedThrough", Js::Null),
        ("studentScope", "source_verified".into()),
    ])
}
fn put(path: &Path, bytes: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
fn pin(path: &Path) -> Value {
    json!({"path":path,"sha256":digest(&fs::read(path).unwrap())})
}
fn document(path: &Path, value: Value) -> Value {
    put(path, &serde_json::to_vec(&value).unwrap());
    pin(path)
}
struct Scene {
    install: PathBuf,
    state: PathBuf,
    pointer: Vec<u8>,
}
impl Scene {
    fn new(label: &str, routing: &Js, endpoint: &str) -> Self {
        let selected = PathBuf::from(
            std::env::var_os("S11_BUSINESS_TEST_ROOT")
                .expect("Root must supply S11_BUSINESS_TEST_ROOT"),
        );
        assert!(selected.is_absolute());
        assert_eq!(fs::canonicalize(&selected).unwrap(), selected);
        assert!(edu_academy_mcp::roots::canonical_absolute(&selected));
        let base = selected.join(format!(
            "business-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .expect("fresh fictional child required");
        for name in ["install", "evidence", "state"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(name))
                .unwrap();
        }
        let install = base.join("install");
        let state = base.join("state");
        let bridge = install.join("bridge");
        put(&bridge, b"fictional metadata only; never launched");
        let launcher = install.join("launcher");
        put(&launcher, b"fictional launcher; never launched");
        let admission_path = install.join("admission.json");
        put(&admission_path, &encode(routing).unwrap());
        let routing = document(
            &install.join("routing.json"),
            json!({"admissionPath":admission_path,"admissionSha256":pin(&admission_path)["sha256"]}),
        );
        let owner = document(
            &install.join("owner.json"),
            json!({"schemaVersion":"academy.rust-owner/1","kind":"fictional",
            "uid":rustix::process::geteuid().as_raw(),"installRoot":install,"evidenceRoot":base.join("evidence"),"stateRoot":state}),
        );
        let args = vec![
            "--install-root".to_owned(),
            install.to_str().unwrap().into(),
            "--endpoint".into(),
            endpoint.into(),
        ];
        let mut artifact = pin(&bridge);
        artifact["role"] = json!("entry");
        let identity = document(
            &install.join("identity.json"),
            json!({"schemaVersion":"academy.rust-endpoint/1","endpoint":endpoint,
            "semanticContractVersion":"s11-fictional","launchArgumentsSha256":digest(&serde_json::to_vec(&args).unwrap()),"artifacts":[artifact]}),
        );
        let mut endpoints_map = serde_json::Map::new();
        endpoints_map.insert(
            endpoint.into(),
            json!({"executable":pin(&bridge),"identity":identity,"args":args}),
        );
        let endpoints = document(
            &install.join("endpoints.json"),
            Value::Object(endpoints_map),
        );
        // The six actual pinned endpoint schemas, unchanged; the admission probe
        // is endpoint-local, just as in existing RpcSession inventory.
        let original: Value = serde_json::from_slice(include_bytes!(
            "fixtures/mcp-runtime-reference-v1/frozen/inventory/registry.json"
        ))
        .unwrap();
        let names = [
            "academy_dailyrecord_intake",
            "academy_dailyrecord_review",
            "academy_dailyrecord_save",
            "academy_dailyrecord_reconcile",
            "academy_dailyrecord_execution_status",
            "academy_student_resolve",
        ];
        let mut entries = Vec::new();
        for name in names {
            let mut e = original
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["upstreamName"] == name)
                .expect("pinned actual tool required")
                .clone();
            e["endpoint"] = json!(endpoint);
            entries.push(e);
        }
        let registry = document(&install.join("registry.json"), json!(entries));
        let source = install.join("source.bin");
        put(&source, b"fictional pinned source");
        let mut artifact = pin(&source);
        artifact["role"] = json!("source");
        let build = document(
            &install.join("build.json"),
            json!({"schemaVersion":"academy.rust-build/1","sourceRevision":"0".repeat(40),
            "semanticContractVersion":"s11-fictional","cargoLockSha256":"0".repeat(64),"artifacts":[artifact]}),
        );
        let pointer=serde_json::to_vec(&json!({"schemaVersion":"academy.mcp-release/3","generation":"business-fictional-v1",
            "bridge":pin(&bridge),"launcher":pin(&launcher),"build":build,"routing":routing,"owner":owner,"registry":registry,"endpoints":endpoints})).unwrap();
        put(&install.join("release.json"), &pointer);
        Self {
            install,
            state,
            pointer,
        }
    }
    fn session(&self, endpoint: &str) -> RpcSession {
        RpcSession::endpoint(ReleaseGraph::open(&self.install).unwrap(), endpoint.into()).unwrap()
    }
    fn store(&self) -> Store {
        Store::open(&self.state.join("workflow"), &self.state).unwrap()
    }
    fn drift(&self) {
        fs::write(self.install.join("source.bin"), b"fictional drift").unwrap();
    }
}
struct LiteralCredentials(Rc<Cell<usize>>);
impl CredentialProvider for LiteralCredentials {
    fn credential(&mut self) -> SourceResult<Credentials> {
        self.0.set(self.0.get() + 1);
        Ok(Credentials::new(
            "fictional-user".into(),
            "fictional-password".into(),
        ))
    }
}
struct MemoryHttp {
    replies: VecDeque<(String, u16, String)>,
    seen: Rc<RefCell<Vec<Request>>>,
}
impl Transport for MemoryHttp {
    fn dispatch(&mut self, input: &Request) -> SourceResult<Response> {
        self.seen.borrow_mut().push(input.clone());
        let (path, status, body) = self
            .replies
            .pop_front()
            .expect("finite in-memory HTTP budget; no retry permitted");
        assert_eq!(input.path, path);
        Ok(Response {
            status,
            headers: vec![],
            bytes: body.into_bytes(),
            url: format!("{ORIGIN}{path}"),
            stalled: false,
        })
    }
}
type FictionalOwner<'a> = (
    Owner<'a, MemoryHttp, LiteralCredentials>,
    Rc<RefCell<Vec<Request>>>,
    Rc<Cell<usize>>,
);

fn owner<'a>(
    clock: &'a Clock,
    replies: Vec<(String, u16, String)>,
    v2: bool,
) -> FictionalOwner<'a> {
    let seen = Rc::new(RefCell::new(vec![]));
    let credentials = Rc::new(Cell::new(0));
    let client = Client::new(
        js(json!({"teacherLabel":"Fictional Teacher","teacherKey":"99001","collegeKey":"88001"})),
        MemoryHttp {
            replies: replies.into(),
            seen: seen.clone(),
        },
    )
    .unwrap();
    let config = owner_config();
    (
        Owner::new(
            client,
            admission(),
            v2.then_some(&config),
            vec!["Fictional Learner".into()],
            LiteralCredentials(credentials.clone()),
            clock,
        )
        .unwrap(),
        seen,
        credentials,
    )
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
    vec![("/c_login.jsp".into(),200,format!(r#"<form id="loginForm" method="post">{fields}</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>"#)),
        ("/servlet/controller.login.LoginServlet".into(),200,"fictional accepted login".into()),
        ("/servlet/controller.coursemanage.CourseManageServlet".into(),200,r#"<form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="fictional"></form><a>로그아웃</a>"#.into())]
}
struct Policy;
impl review::Policy for Policy {
    fn parent_pins(&self) -> Js {
        js(
            json!({"sourceRef":"fictional:business-parent","policySha256":"3".repeat(64),"safeguardSourceSha256":"4".repeat(64)}),
        )
    }
    fn learning_pins(&self) -> Js {
        js(
            json!({"bundleSha256":"5".repeat(64),"configSha256":"6".repeat(64),"catalogSha256":"7".repeat(64)}),
        )
    }
    fn check_lines(&self, v: &Js) -> bool {
        v.units().starts_with(&[45, 32])
    }
    fn excluded_parent_language(&self, _: &Js) -> bool {
        false
    }
    fn scope_qualified(&self, _: &Js) -> bool {
        true
    }
    fn learning_review(&self, _: &Js, _: &Js, _: &Js) -> Vec<Js> {
        vec![]
    }
}
struct FictionalSource {
    value: Rc<RefCell<Js>>,
    calls: Rc<Cell<usize>>,
    fail: Rc<RefCell<Option<Fault>>>,
    drift: Option<PathBuf>,
}
impl FictionalSource {
    fn new() -> Self {
        Self {
            value: Rc::new(RefCell::new(observation())),
            calls: Rc::new(Cell::new(0)),
            fail: Rc::new(RefCell::new(None)),
            drift: None,
        }
    }
}
impl service::Source for FictionalSource {
    fn read(&mut self, scope: &Js, request: &Js, _: &Js) -> CoreResult<Js> {
        self.calls.set(self.calls.get() + 1);
        assert_eq!(scope, context().get("scope"));
        assert!(request.is("fictional-request"));
        if let Some(path) = &self.drift {
            fs::write(path, b"fictional drift during read").unwrap();
        }
        if let Some(error) = self.fail.borrow().clone() {
            return Err(error);
        }
        Ok(obj(&[
            ("status", "observed".into()),
            ("observation", self.value.borrow().clone()),
            (
                "subject",
                obj(&[("displayName", "Fictional Learner".into())]),
            ),
        ]))
    }
}
/// Fictional uncertainty only: invokes the real request approval/fence and then
/// throws. Never returns a fabricated persisted/HTTP success.
struct UncertainBackend {
    facts: Option<Js>,
    calls: Rc<Cell<usize>>,
    drift: Option<PathBuf>,
}
impl UncertainBackend {
    fn new() -> Self {
        Self {
            facts: None,
            calls: Rc::new(Cell::new(0)),
            drift: None,
        }
    }
}
impl service::Backend for UncertainBackend {
    fn configured(&self) -> bool {
        true
    }
    fn save(
        &mut self,
        scope: &Js,
        patches: &Js,
        approval: &mut dyn Approval,
        _: &Js,
    ) -> CoreResult<Js> {
        self.calls.set(self.calls.get() + 1);
        let p = &patches.array()[0];
        let action = obj(&[
            ("attemptId", "fictional-business-attempt".into()),
            (
                "target",
                obj(&[
                    ("studentKey", scope.get("studentId").clone()),
                    ("teacherKey", scope.get("teacherId").clone()),
                    ("date", scope.get("lessonDate").clone()),
                ]),
            ),
            ("field", p.get("field").clone()),
            ("effectTarget", p.get("effectTarget").clone()),
            ("effectRecordKey", "current-record".into()),
            ("before", p.get("before").clone()),
            ("after", p.get("value").clone()),
        ]);
        assert!(approval.consume(&action)?.get("status").is("verified"));
        if let Some(path) = &self.drift {
            fs::write(path, b"fictional drift after durable fence").unwrap();
        }
        if let Some(facts) = &self.facts {
            return Ok(Js::Arr(vec![obj(&[
                ("field", p.get("field").clone()),
                ("status", "unresolved".into()),
                ("transport", "returned".into()),
                ("dispatched", true.into()),
                ("completionFacts", facts.clone()),
            ])]));
        }
        Err(Fault::error("fictional_business_uncertain"))
    }
}
fn request(id: Js, method: &str, params: Js) -> Js {
    obj(&[
        ("jsonrpc", "2.0".into()),
        ("id", id),
        ("method", method.into()),
        ("params", params),
    ])
}
fn wire(tool: &str, args: Js) -> Js {
    request(
        "fictional-wire-id".into(),
        "tools/call",
        obj(&[("name", tool.into()), ("arguments", args)]),
    )
}
fn invoke<T: Transport, C: CredentialProvider>(
    session: &mut RpcSession,
    driver: &mut DailyRecord<'_, '_, T, C>,
    tool: &str,
    args: Js,
) -> Js {
    session
        .handle_business(&parse(&encode(&wire(tool, args)).unwrap()).unwrap(), driver)
        .unwrap()
        .unwrap()
}
fn data(response: &Js) -> Js {
    assert!(!response.has("error"), "{response:?}");
    let result = response.get("result");
    assert!(!result.get("isError").truthy(), "{response:?}");
    let structured = result.get("structuredContent").clone();
    let text = result.get("content").array()[0].get("text").code_units();
    assert_eq!(
        parse(text.as_bytes()).unwrap(),
        structured,
        "dual representations must agree"
    );
    structured
}
fn review_input(handle: &Js) -> Js {
    obj(&[
        ("context", context()),
        ("observationHandle", handle.clone()),
        (
            "proposals",
            Js::Arr(vec![proposal("progress", "- Updated progress")]),
        ),
    ])
}
fn save_input(handle: &Js, digest: &Js) -> Js {
    let mut input = review_input(handle);
    input.set("reviewDigest", digest.clone());
    input.set(
        "request",
        obj(&[
            ("requestRef", "fictional-request".into()),
            (
                "instruction",
                obj(&[("text", "Save this fictional reviewed progress once.".into())]),
            ),
        ]),
    );
    input
}
fn prepare<T: Transport, C: CredentialProvider>(
    session: &mut RpcSession,
    driver: &mut DailyRecord<'_, '_, T, C>,
) -> (Js, Js) {
    let intake = data(&invoke(
        session,
        driver,
        "academy_dailyrecord_intake",
        obj(&[("context", context())]),
    ));
    assert!(intake.get("observationPersistence").is("owner_durable"));
    let handle = intake.get("observationHandle").clone();
    assert!(handle.string());
    let reviewed = data(&invoke(
        session,
        driver,
        "academy_dailyrecord_review",
        review_input(&handle),
    ));
    assert!(reviewed.get("status").is("prepared"), "{reviewed:?}");
    assert!(reviewed.get("reviewRecord").get("persisted").truthy());
    (handle, reviewed.get("reviewDigest").clone())
}

fn context() -> Js {
    obj(&[
        ("schemaVersion", "academy.dailyrecord-user-context/1".into()),
        ("purpose", "Fictional intake contract verification".into()),
        ("currentRequestRef", "fictional-request".into()),
        (
            "scope",
            obj(&[
                ("sourceNamespace", "academy.lms.dayrecord".into()),
                ("teacherId", "99001".into()),
                ("studentId", "77001".into()),
                ("lessonDate", "2040-01-04".into()),
            ]),
        ),
        ("selectedFields", js(json!(["progress"]))),
        (
            "memoAnswer",
            obj(&[
                ("kind", "no_addition".into()),
                ("requestRef", "fictional-request".into()),
            ]),
        ),
        ("facts", Js::Arr(vec![fact("progress", "Updated progress")])),
    ])
}
fn observation() -> Js {
    let scope = context().get("scope").clone();
    let mut target = scope.clone();
    for (key, value) in [
        ("courseId", "course"),
        ("membershipId", "membership"),
        ("recordId", "current-record"),
    ] {
        target.set(key, value.into());
    }
    let mut previous = target.clone();
    previous.set("recordId", "previous-record".into());
    previous.set("lessonDate", "2040-01-03".into());
    let mut fields = obj(&[
        ("progress", known("- Existing progress")),
        ("homework", known("- Existing homework")),
        ("lessonMemo", known("- Existing memo")),
        ("attendance", known("Y")),
        ("dailyTestAssessment", known("0")),
        ("previousHomeworkAssessment", known("0")),
    ]);
    for field in review::FIELDS {
        let previous_field = field == "previousHomeworkAssessment";
        let mut value = fields.get(field).clone();
        value.set(
            "effectTarget",
            if previous_field {
                "synthetic-previous-target"
            } else {
                "synthetic-target"
            }
            .into(),
        );
        value.set(
            "effectRecordKey",
            if previous_field {
                previous.get("recordId")
            } else {
                target.get("recordId")
            }
            .clone(),
        );
        fields.set(field, value);
    }
    obj(&[
        ("scope", scope),
        ("target", target),
        ("previousTarget", previous.clone()),
        ("observedAt", NOW.into()),
        ("sourceRevision", "1".repeat(64).into()),
        ("wireDigest", "2".repeat(64).into()),
        ("fields", fields),
        (
            "toneSamples",
            Js::Arr(vec![obj(&[
                ("target", previous),
                ("observedAt", "2040-01-03T12:00:00.000Z".into()),
                ("text", "- Allowed fictional tone".into()),
                ("evidenceRefs", js(json!(["fictional-tone-evidence"]))),
            ])]),
        ),
        ("transmissionState", "not_sent".into()),
    ])
}
fn known(value: &str) -> Js {
    obj(&[
        ("state", "known".into()),
        ("value", value.into()),
        ("evidenceRefs", js(json!(["fictional-read"]))),
    ])
}
fn fact(field: &str, value: &str) -> Js {
    // origin intentionally omitted: the existing intake schema supplies its default.
    js(
        json!({"ref":format!("fact-{field}"), "field":field, "text":value,
        "explicitValue":value, "requiredFragments":[], "questionCounts":[],
        "deadlines":[], "stageClaims":[]}),
    )
}
fn proposal(field: &str, value: &str) -> Js {
    js(
        json!({"field":field, "value":value, "factRefs":[format!("fact-{field}")],
        "decision":{"kind":"replace", "requestRef":"fictional-request"}}),
    )
}

// Inert public fixture helper copied from accepted owner_workflow test, read-only.
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
fn seven_wire_routes_persist_exact_review_and_reconcile_unknown_dispatch_after_reentry() {
    let scene = Scene::new("seven", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let mut replies = login();
    replies.push(("/servlet/controller.dailyzerotest.DailyZeroTestServlet".into(),200,
        r#"<script>var fictional = { testResultList: JSON.parse('[{"priNo":"77001","userName":"Fictional Learner"}]') };</script>"#.into()));
    let (owner, http, credentials) = owner(&clock, replies, false);
    let mut source = FictionalSource::new();
    let value = source.value.clone();
    let reads = source.calls.clone();
    let mut backend = UncertainBackend::new();
    let writes = backend.calls.clone();
    let policy = Policy;
    let mut session = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let admission = data(&invoke(
        &mut session,
        &mut driver,
        "academy_runtime_read_admission",
        obj(&[]),
    ));
    assert_eq!(
        admission,
        obj(&[
            ("status", "configured".into()),
            (
                "admissionSha256",
                digest(&encode(&routing_admission()).unwrap()).into(),
            ),
        ])
    );
    assert!(
        http.borrow().is_empty(),
        "digest probe must not authenticate or read source"
    );
    let resolved = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional test","currentRequestRef":"fictional-request","displayName":"  Fictional Learner  "}),
        ),
    ));
    assert!(resolved.get("status").is("resolved"));
    assert!(resolved.get("displayName").is("Fictional Learner"));
    assert!(resolved.get("purpose").is("Fictional test"));
    assert!(resolved.get("studentKey").is("77001"));
    let (handle, review_digest) = prepare(&mut session, &mut driver);
    let stored = store.get(Kind::Observation, &handle.code_units()).unwrap();
    assert_eq!(stored.get("scope"), context().get("scope"));
    assert!(stored.get("requestRef").is("fictional-request"));
    assert_eq!(stored.get("observation"), &observation());
    let review_record = store
        .get(Kind::Review, &review_digest.code_units())
        .unwrap();
    assert_eq!(review_record.get("observationHandle"), &handle);
    assert!(
        review_record.get("patch").array()[0]
            .get("before")
            .is("- Existing progress")
    );
    assert!(
        review_record.get("patch").array()[0]
            .get("after")
            .is("- Updated progress")
    );
    // A request change cannot consume the durable handle or cause a source read.
    let before_reads = reads.get();
    let mut wrong = review_input(&handle);
    let mut c = context();
    c.set("currentRequestRef", "wrong-request".into());
    wrong.set("context", c);
    let rejected = invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_review",
        wrong,
    );
    assert!(rejected.get("result").get("isError").truthy());
    assert!(
        rejected.get("result").get("content").array()[0]
            .get("text")
            .is("exact_backend_observation_handle_required")
    );
    assert_eq!(reads.get(), before_reads);
    // A policy/proposal mismatch cannot reach the backend, even with a valid digest.
    let mut wrong = save_input(&handle, &review_digest);
    wrong.set(
        "proposals",
        Js::Arr(vec![proposal("progress", "- Other progress")]),
    );
    let held = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_save",
        wrong,
    ));
    assert!(held.get("reasonCode").is("prior_review_record_mismatch"));
    assert_eq!(writes.get(), 0);
    let saved = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_save",
        save_input(&handle, &review_digest),
    ));
    assert!(saved.get("status").is("held"));
    assert!(saved.get("reasonCode").is("save_reconciliation_required"));
    assert_eq!(saved.get("academyWrites"), &Js::Null);
    assert!(!saved.get("automaticRetryAllowed").truthy());
    let status = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_execution_status",
        obj(&[
            ("scope", context().get("scope").clone()),
            ("requestRef", "fictional-request".into()),
        ]),
    ));
    assert!(status.get("status").is("recorded"));
    assert_eq!(status.get("fields").array().len(), 1);
    assert!(
        status.get("fields").array()[0].get("executions").array()[0]
            .get("executionState")
            .is("reconciliation_required")
    );
    let replay = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_save",
        save_input(&handle, &review_digest),
    ));
    assert!(replay.get("reasonCode").is("save_confirmation_pending"));
    assert_eq!(writes.get(), 1);
    let unresolved = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_reconcile",
        obj(&[("context", context())]),
    ));
    assert!(unresolved.get("reasonCode").is("save_confirmation_pending"));
    drop(driver);
    // R30: a new Store and RpcSession retain the fence even after equality;
    // readback cannot establish final completion of the lost answer.
    let reopened = scene.store();
    let mut session = scene.session("dailyrecord");
    let mut observed = value.borrow().clone();
    let mut fields = observed.get("fields").clone();
    let mut progress = fields.get("progress").clone();
    progress.set("value", "- Updated progress".into());
    fields.set("progress", progress);
    observed.set("fields", fields);
    *value.borrow_mut() = observed;
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &reopened,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let settled = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_reconcile",
        obj(&[("context", context())]),
    ));
    assert!(settled.get("status").is("held"));
    let executions = state::student_executions(&reopened, context().get("scope")).unwrap();
    assert_eq!(executions.len(), 1);
    assert!(executions[0].state.needs_reconciliation());
    assert!(
        reopened
            .get(Kind::Reconciliation, &executions[0].key)
            .unwrap()
            .absent()
    );
    assert_eq!(writes.get(), 1);
    assert_eq!(credentials.get(), 1);
    assert_eq!(http.borrow().len(), 4);
    let key = state::request_key(context().get("scope"), "fictional-request").unwrap();
    let audit = reopened.audit(&key, 128).unwrap();
    assert_eq!(
        audit
            .array()
            .iter()
            .filter(|e| e.get("event").is("dispatch_failed"))
            .count(),
        1
    );
    assert_eq!(
        audit
            .array()
            .iter()
            .filter(|e| e.get("event").is("reconciled"))
            .count(),
        0
    );
}

#[test]
fn actual_owner_typed_not_admitted_and_auth_failure_cache_survive_independent_calls() {
    let scene = Scene::new("auth-cache", &source_verified_routing(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let (owner, http, credentials) = owner(
        &clock,
        vec![("/c_login.jsp".into(), 503, String::new())],
        true,
    );
    let mut reader = owner.source();
    let mut writer = owner.writer();
    let (a, o, n) = owner.write_configuration().unwrap();
    let mut backend = NativeBackend::new(&mut writer, a, o, n, &clock);
    let mut session = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut reader,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let refused = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional test","currentRequestRef":"fictional-request","displayName":"Unmanaged Fictional"}),
        ),
    ));
    assert!(refused.get("reasonCode").is("student_name_not_admitted"));
    assert!(!refused.has("failure"));
    assert_eq!(credentials.get(), 0);
    assert!(http.borrow().is_empty());
    let first = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional test","currentRequestRef":"fictional-request","displayName":"Fictional Learner"}),
        ),
    ));
    assert!(first.get("reasonCode").is("source_authentication_failed"));
    assert_eq!(
        first.get("failure").get("cause").get("sourceHttpStatus"),
        &Js::Num(503.0)
    );
    assert!(
        first
            .get("failure")
            .get("cause")
            .get("sourceOperation")
            .is("login_document")
    );
    let intake = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[("context", context())]),
    ));
    assert!(intake.get("sourceAvailability").is("unavailable"));
    assert_eq!(intake.get("failure"), first.get("failure"));
    let again = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional second judgement","currentRequestRef":"another-request","displayName":"Fictional Learner"}),
        ),
    ));
    assert_eq!(again.get("failure"), first.get("failure"));
    assert!(again.get("purpose").is("Fictional second judgement"));
    assert_eq!(credentials.get(), 1);
    assert_eq!(http.borrow().len(), 1);
    // Authentication cache does not become an invented session-wide tool latch.
    assert!(
        data(&invoke(
            &mut session,
            &mut driver,
            "academy_runtime_read_admission",
            obj(&[])
        ))
        .get("status")
        .is("configured")
    );
    assert!(
        data(&invoke(
            &mut session,
            &mut driver,
            "academy_dailyrecord_execution_status",
            obj(&[("scope", context().get("scope").clone())])
        ))
        .get("status")
        .is("recorded")
    );
    assert!(
        state::student_executions(&store, context().get("scope"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn actual_owner_success_cache_non_auth_failure_and_trim_request_scope_reentry() {
    let scene = Scene::new("source-failure", &source_verified_routing(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let name_page = r#"<script>var fictional = { testResultList: JSON.parse('[{"priNo":"77002","userName":"Fictional Learner"}]') };</script>"#;
    let mut replies = login();
    replies.extend([
        (
            "/servlet/controller.dailyzerotest.DailyZeroTestServlet".into(),
            200,
            name_page.into(),
        ),
        (MAIN.into(), 503, String::new()),
        (
            "/servlet/controller.dailyzerotest.DailyZeroTestServlet".into(),
            200,
            name_page.into(),
        ),
        (MAIN.into(), 503, String::new()),
    ]);
    let (owner, http, credentials) = owner(&clock, replies, true);
    let mut reader = owner.source();
    let mut writer = owner.writer();
    let (a, o, n) = owner.write_configuration().unwrap();
    let mut backend = NativeBackend::new(&mut writer, a, o, n, &clock);
    let mut session = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut reader,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let resolved = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional exact name","currentRequestRef":"fictional-request","displayName":"\u{feff}Fictional Learner\u{a0}"}),
        ),
    ));
    assert!(resolved.get("studentKey").is("77002"));
    assert!(resolved.get("displayName").is("Fictional Learner"));
    let mut c = context();
    let mut s = c.get("scope").clone();
    s.set("studentId", "77002".into());
    c.set("scope", s);
    let failed = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[("context", c)]),
    ));
    assert!(failed.get("failure").get("code").is("source_http_failed"));
    assert!(
        failed
            .get("failure")
            .get("category")
            .is("upstream_unavailable")
    );
    assert!(
        !failed.get("failure").has("cause"),
        "successful auth must not reclassify later HTTP failure"
    );
    assert_eq!(
        failed.get("failure").get("sourceHttpStatus"),
        &Js::Num(503.0)
    );
    let resolved = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional next request","currentRequestRef":"fictional-request","displayName":"Fictional Learner"}),
        ),
    ));
    assert!(resolved.get("status").is("resolved"));
    assert_eq!(credentials.get(), 1);
    let dated = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional dated occurrence", "currentRequestRef":"dated-request",
            "displayName":"Fictional Learner", "lessonDate":"2040-01-05"}),
        ),
    ));
    assert!(dated.get("reasonCode").is("source_http_failed"));
    let seen = http.borrow();
    assert_eq!(seen.len(), 7);
    assert_eq!(seen[6].path, MAIN);
    assert!(
        seen[6]
            .body
            .iter()
            .any(|(k, v)| k == "std_ymd" && v == "20400105"),
        "optional lessonDate must select the owner occurrence path, not name POST"
    );
    assert_eq!(
        seen[3].body,
        vec![
            ("sort_date".into(), "1".into()),
            ("stu_name".into(), "Fictional Learner".into())
        ]
    );
    assert_eq!(seen[5].body, seen[3].body);
}

#[test]
fn schema_unknown_crossendpoint_and_unconfigured_calls_never_enter_business() {
    let scene = Scene::new("guards", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let (owner, http, credentials) = owner(&clock, vec![], false);
    let mut source = FictionalSource::new();
    let reads = source.calls.clone();
    let mut backend = UncertainBackend::new();
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let mut session = scene.session("dailyrecord");
    let bad = invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[]),
    );
    assert!(bad.get("result").get("isError").truthy());
    assert!(!bad.get("result").has("structuredContent"));
    let unknown = invoke(&mut session, &mut driver, "academy_source_status", obj(&[]));
    assert_eq!(unknown.get("error").get("code"), &Js::Num(-32602.0));
    assert!(
        unknown
            .get("error")
            .get("message")
            .is("Tool academy_source_status not found")
    );
    let bad = invoke(
        &mut session,
        &mut driver,
        "academy_runtime_read_admission",
        obj(&[("extra", true.into())]),
    );
    assert!(bad.get("result").get("isError").truthy());
    let other = Scene::new("cross-endpoint", &routing_admission(), "saved");
    let mut other_session = other.session("saved");
    let bad = invoke(
        &mut other_session,
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[("context", context())]),
    );
    assert!(
        bad.get("error")
            .get("message")
            .is("business_endpoint_mismatch")
    );
    let mut neutral = RpcSession::new(ReleaseGraph::open(&scene.install).unwrap());
    let bad = invoke(
        &mut neutral,
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[("context", context())]),
    );
    assert!(
        bad.get("error")
            .get("message")
            .is("business_endpoint_mismatch")
    );
    // A driver is never installed persistently: the default path still refuses.
    let default = session
        .handle(&wire(
            "academy_dailyrecord_intake",
            obj(&[("context", context())]),
        ))
        .unwrap()
        .unwrap();
    assert!(
        default
            .get("result")
            .get("structuredContent")
            .get("code")
            .is("configured_rust_route_unimplemented")
    );
    assert_eq!(reads.get(), 0);
    assert_eq!(credentials.get(), 0);
    assert!(http.borrow().is_empty());
    for (id, version) in [(Js::Num(7.0), "2024-11-05"), ("again".into(), "2025-11-25")] {
        let response=session.handle_business(&request(id.clone(),"initialize",js(json!({"protocolVersion":version,"capabilities":{},"clientInfo":{"name":"fictional","version":"1"}}))),&mut driver).unwrap().unwrap();
        assert_eq!(response.get("id"), &id);
        assert!(response.get("result").get("protocolVersion").is(version));
    }
    let list = session
        .handle_business(&request("list".into(), "tools/list", obj(&[])), &mut driver)
        .unwrap()
        .unwrap();
    assert_eq!(
        list.get("result").get("tools").array().len(),
        8,
        "six registered + identity + admission, no widening"
    );
    let notify = obj(&[
        ("jsonrpc", "2.0".into()),
        ("method", "notifications/initialized".into()),
    ]);
    assert!(
        session
            .handle_business(&notify, &mut driver)
            .unwrap()
            .is_none()
    );
    assert!(session.initialized);
    assert_eq!(reads.get(), 0);
}

#[test]
fn checked_store_and_admission_bindings_cannot_promote_fictional_owner_to_native() {
    let scene = Scene::new("bindings", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let (owner, http, _) = owner(&clock, vec![], false);
    let mut source = FictionalSource::new();
    let reads = source.calls.clone();
    let mut backend = UncertainBackend::new();
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let other = Scene::new("foreign-store", &routing_admission(), "dailyrecord");
    let refused = invoke(
        &mut other.session("dailyrecord"),
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[("context", context())]),
    );
    assert!(
        refused
            .get("error")
            .get("message")
            .is("business_store_graph_mismatch")
    );
    let mut admission = routing_admission();
    admission.set("teacherId", "99002".into());
    let mismatch = Scene::new("wrong-admission", &admission, "dailyrecord");
    let mismatch_store = mismatch.store();
    drop(driver);
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &mismatch_store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let refused = invoke(
        &mut mismatch.session("dailyrecord"),
        &mut driver,
        "academy_runtime_read_admission",
        obj(&[]),
    );
    assert!(
        refused
            .get("error")
            .get("message")
            .is("release_upstream_admission_mismatch")
    );
    let mut owner_json: Value =
        serde_json::from_slice(&fs::read(scene.install.join("owner.json")).unwrap()).unwrap();
    owner_json["kind"] = json!("native");
    let owner_pin = document(&scene.install.join("native-owner.json"), owner_json);
    let mut pointer: Value = serde_json::from_slice(&scene.pointer).unwrap();
    pointer["owner"] = owner_pin;
    fs::write(
        scene.install.join("release.json"),
        serde_json::to_vec(&pointer).unwrap(),
    )
    .unwrap();
    assert!(
        matches!(ReleaseGraph::open(&scene.install),Err(e) if e.code=="native_owner_binding_unqualified")
    );
    assert_eq!(reads.get(), 0);
    assert!(http.borrow().is_empty());
}

#[test]
fn graph_drift_before_dispatch_latches_without_any_business_effect() {
    let scene = Scene::new("before-drift", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let (owner, http, _) = owner(&clock, vec![], false);
    let mut source = FictionalSource::new();
    let reads = source.calls.clone();
    let mut backend = UncertainBackend::new();
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let mut session = scene.session("dailyrecord");
    scene.drift();
    let refused = invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[("context", context())]),
    );
    assert!(
        refused
            .get("error")
            .get("message")
            .is("release_artifact_mismatch")
    );
    fs::write(scene.install.join("source.bin"), b"fictional pinned source").unwrap();
    assert!(
        invoke(
            &mut session,
            &mut driver,
            "academy_dailyrecord_intake",
            obj(&[("context", context())])
        )
        .has("error")
    );
    assert_eq!(reads.get(), 0);
    assert!(http.borrow().is_empty());
    assert!(
        fs::read_dir(store.root().join("observation"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn graph_drift_after_intake_keeps_durable_observation_and_latched_failure() {
    let scene = Scene::new("after-intake", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let (owner, _, _) = owner(&clock, vec![], false);
    let mut source = FictionalSource::new();
    source.drift = Some(scene.install.join("source.bin"));
    let reads = source.calls.clone();
    let mut backend = UncertainBackend::new();
    let mut session = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let refused = invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_intake",
        obj(&[("context", context())]),
    );
    assert!(
        refused
            .get("error")
            .get("data")
            .get("businessDispatched")
            .truthy()
    );
    assert!(
        refused
            .get("error")
            .get("data")
            .get("localWorkflowEffects")
            .is("unknown_reconcile_owner_store")
    );
    let handle = "dailyrecord-observation:00000000-0000-4000-8000-000000000001";
    assert!(
        !store.get(Kind::Observation, handle).unwrap().absent(),
        "published handle must survive final graph failure"
    );
    assert_eq!(reads.get(), 1);
    fs::write(scene.install.join("source.bin"), b"fictional pinned source").unwrap();
    assert!(
        invoke(
            &mut session,
            &mut driver,
            "academy_dailyrecord_intake",
            obj(&[("context", context())])
        )
        .has("error")
    );
    assert_eq!(reads.get(), 1);
}

#[test]
fn graph_drift_after_save_preserves_unknown_effect_fence_and_reentry_status_without_retry() {
    let scene = Scene::new("after-save", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let (owner, _, _) = owner(&clock, vec![], false);
    let mut source = FictionalSource::new();
    let mut backend = UncertainBackend::new();
    backend.drift = Some(scene.install.join("source.bin"));
    let writes = backend.calls.clone();
    let mut session = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let (handle, digest) = prepare(&mut session, &mut driver);
    let failed = invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_save",
        save_input(&handle, &digest),
    );
    let error = failed.get("error");
    assert!(error.get("message").is("release_artifact_mismatch"));
    assert_eq!(error.get("data").get("academyWrites"), &Js::Null);
    assert!(error.get("data").get("effectStatus").is("unknown"));
    assert!(!error.get("data").get("automaticRetryAllowed").truthy());
    let e = state::student_executions(&store, context().get("scope")).unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].state, state::FieldState::ReconciliationRequired);
    assert_eq!(
        store
            .get(Kind::Outcome, &e[0].key)
            .unwrap()
            .get("dispatched"),
        &Js::Null
    );
    fs::write(scene.install.join("source.bin"), b"fictional pinned source").unwrap();
    assert!(
        invoke(
            &mut session,
            &mut driver,
            "academy_dailyrecord_save",
            save_input(&handle, &digest)
        )
        .has("error")
    );
    assert_eq!(writes.get(), 1);
    drop(driver);
    let reopened = scene.store();
    let mut fresh = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &reopened,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let status = data(&invoke(
        &mut fresh,
        &mut driver,
        "academy_dailyrecord_execution_status",
        obj(&[("scope", context().get("scope").clone())]),
    ));
    assert!(
        status.get("fields").array()[0].get("executions").array()[0]
            .get("executionState")
            .is("reconciliation_required")
    );
    let replay = data(&invoke(
        &mut fresh,
        &mut driver,
        "academy_dailyrecord_save",
        save_input(&handle, &digest),
    ));
    assert!(replay.get("reasonCode").is("save_confirmation_pending"));
    assert_eq!(writes.get(), 1);
}

#[test]
fn actual_native_backend_client_keeps_public_6508_refusal_without_approval_or_mutation() {
    let scene = Scene::new("public-wire", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let html = owner_unqualified_main(&js(
        json!({"persistentMemo":"Fictional memo","attendance":"Y","dailyTestAssessment":"0",
        "previousHomeworkAssessment":"3","progress":"Before","homework":"Homework","lessonMemo":"Memo"}),
    ));
    let selected =
        "<root><record>!2040-01-04!5!!Fictional Learner!!!!!3!13!77001!8</record></root>";
    let mut replies = login();
    replies.extend([
        (MAIN.into(), 200, html.clone()), // discovery
        (MAIN.into(), 200, html.clone()),
        (SELECTED.into(), 200, selected.into()), // independent occurrence
        (MAIN.into(), 200, html.clone()),
        (SELECTED.into(), 200, selected.into()),
        (MAIN.into(), 200, html),
    ]); // journal + wire
    let (owner, http, credentials) = owner(&clock, replies, false);
    let mut writer = owner.writer();
    let (a, o, n) = owner.write_configuration().unwrap();
    let mut backend = NativeBackend::new(&mut writer, a, o, n, &clock);
    // Positive workflow preparation uses fictional before-state. The real writer
    // remains unqualified and must refuse before consuming any approval.
    let mut source = FictionalSource::new();
    let mut session = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let (handle, digest) = prepare(&mut session, &mut driver);
    let held = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_save",
        save_input(&handle, &digest),
    ));
    assert!(held.get("reasonCode").is("save_reconciliation_required"));
    assert_eq!(held.get("academyWrites"), &Js::Null);
    assert!(
        state::student_executions(&store, context().get("scope"))
            .unwrap()
            .is_empty(),
        "6508 refusal precedes approval consumption"
    );
    let audit = store
        .audit(
            &state::request_key(context().get("scope"), "fictional-request").unwrap(),
            128,
        )
        .unwrap();
    assert!(audit.array().iter().any(|e| {
        e.get("event").is("dispatch_failed")
            && e.get("reasonCode")
                .is("dailyrecord_save_unqualified_no_automatic_retry")
    }));
    assert_eq!(credentials.get(), 1);
    let requests = http.borrow();
    assert_eq!(requests.len(), 9);
    for request in requests.iter().skip(3) {
        if request.path == MAIN {
            assert!(
                request
                    .body
                    .iter()
                    .any(|(k, v)| k == "p_process" && v == "Main")
            );
        } else {
            assert_eq!(request.path, SELECTED);
            assert!(
                request
                    .query
                    .iter()
                    .any(|(k, v)| k == "reqCmd" && v == "SelSglRecord")
            );
        }
    }
}

#[test]
fn explicitly_absent_read_owner_returns_original_unavailable_probe_without_source_request() {
    let scene = Scene::new("no-read-owner", &Js::Null, "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let mut source = FictionalSource::new();
    let reads = source.calls.clone();
    let mut backend = UncertainBackend::new();
    let mut driver: DailyRecord<'_, '_, MemoryHttp, LiteralCredentials> =
        DailyRecord::from_borrowed_service_without_read_owner(Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        });
    let mut session = scene.session("dailyrecord");
    let absent = data(&invoke(
        &mut session,
        &mut driver,
        "academy_runtime_read_admission",
        obj(&[]),
    ));
    assert_eq!(
        absent,
        obj(&[
            ("status", "unavailable".into()),
            ("admissionSha256", Js::Null),
        ])
    );
    let resolve = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        js(
            json!({"purpose":"Fictional absence","currentRequestRef":"fictional-request","displayName":"Fictional Learner"}),
        ),
    ));
    assert!(
        resolve
            .get("reasonCode")
            .is("student_resolve_backend_not_configured")
    );
    assert_eq!(reads.get(), 0);
    assert!(
        fs::read_dir(store.root().join("observation"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn actual_resolver_zero_multikey_malformed_and_utf16_validation_do_not_guess_or_latch() {
    let scene = Scene::new("resolver-rows", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let path = "/servlet/controller.dailyzerotest.DailyZeroTestServlet";
    let mut replies = login();
    replies.extend([
        (path.into(),200,r#"<script>var v={testResultList:JSON.parse('[]')};</script>"#.into()),
        (path.into(),200,r#"<script>var v={testResultList:JSON.parse('[{"priNo":"77001","userName":"Fictional Learner"},{"priNo":"77002","userName":"Fictional Learner"}]')};</script>"#.into()),
        (path.into(),200,r#"<script>var v={testResultList:JSON.parse('{')};</script>"#.into()),
        (path.into(),200,r#"<script>var v={testResultList:JSON.parse('[{"priNo":"77001","userName":"Fictional Learner"}]')};</script>"#.into())]);
    let (owner, http, credentials) = owner(&clock, replies, false);
    let mut reader = owner.source();
    let mut writer = owner.writer();
    let (a, o, n) = owner.write_configuration().unwrap();
    let mut backend = NativeBackend::new(&mut writer, a, o, n, &clock);
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut reader,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let mut session = scene.session("dailyrecord");
    let input = |name: Js| {
        obj(&[
            ("purpose", "Fictional row verification".into()),
            ("currentRequestRef", "fictional-request".into()),
            ("displayName", name),
        ])
    };
    let invalid = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        input("\u{feff}\u{a0} ".into()),
    ));
    assert!(invalid.get("reasonCode").is("student_name_invalid"));
    assert!(http.borrow().is_empty());
    let too_long = invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        input("😀".repeat(101).into()),
    );
    assert!(too_long.get("result").get("isError").truthy());
    assert_eq!(credentials.get(), 0);
    let zero = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        input("Fictional Learner".into()),
    ));
    assert!(zero.get("reasonCode").is("student_name_not_found"));
    let ambiguous = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        input("Fictional Learner".into()),
    ));
    assert!(ambiguous.get("reasonCode").is("student_name_ambiguous"));
    assert!(!ambiguous.has("studentKey"));
    let malformed = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        input("Fictional Learner".into()),
    ));
    assert!(malformed.get("reasonCode").is("source_response_malformed"));
    assert!(malformed.get("failure").get("errorKind").is("SyntaxError"));
    assert!(
        !malformed.get("failure").has("cause"),
        "JSON failure after auth is not an authentication stage"
    );
    let next = data(&invoke(
        &mut session,
        &mut driver,
        "academy_student_resolve",
        input("Fictional Learner".into()),
    ));
    assert!(next.get("studentKey").is("77001"));
    assert_eq!(credentials.get(), 1);
    assert_eq!(http.borrow().len(), 7);
}

#[test]
fn wire_returns_opaque_completion_facts_without_qualification() {
    let scene = Scene::new("facts", &routing_admission(), "dailyrecord");
    let clock = Clock(Cell::new(0));
    let store = scene.store();
    let (owner, _, _) = owner(&clock, vec![], false);
    let mut source = FictionalSource::new();
    let mut backend = UncertainBackend::new();
    let facts = js(json!({"claim":"unqualified", "target":"invented-target", "extra":[null,3]}));
    backend.facts = Some(facts.clone());
    let mut session = scene.session("dailyrecord");
    let mut driver = DailyRecord::from_borrowed_service(
        Service {
            store: &store,
            source: &mut source,
            policy: &Policy,
            backend: &mut backend,
            runtime: &clock,
        },
        &owner,
    );
    let (handle, digest) = prepare(&mut session, &mut driver);
    let saved = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_save",
        save_input(&handle, &digest),
    ));
    assert!(saved.get("status").is("held"));
    assert_eq!(
        saved.get("fields").array()[0].get("completionFacts"),
        &facts
    );
    let status = data(&invoke(
        &mut session,
        &mut driver,
        "academy_dailyrecord_execution_status",
        obj(&[("scope", context().get("scope").clone())]),
    ));
    assert_eq!(
        status.get("fields").array()[0].get("executions").array()[0].get("completionFacts"),
        &facts
    );
}
