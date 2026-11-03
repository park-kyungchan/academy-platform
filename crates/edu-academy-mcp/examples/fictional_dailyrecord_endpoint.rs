//! UNEXECUTED Cargo-native fictional child. Root pins the compiled executable.
//! Actual RpcSession/DailyRecord/Service/Store; synthetic Source is separate from
//! actual Owner/Client name resolution. No positive public HTTP read proof.
use edu_academy_core::{
    js::{Fault, Result as CoreResult},
    workflow::{
        Runtime,
        approval::Approval,
        review,
        service::{Backend, Service, Source},
        store::Store,
    },
};
use edu_academy_mcp::{
    Js, audit, business::DailyRecord, encode, obj, parse, process, protocol::RpcSession,
    release::ReleaseGraph,
};
use edu_academy_sources::{
    data::Result as SourceResult,
    http::{Client, ORIGIN, Request, Response, Transport},
    owner_workflow::{CredentialProvider, Credentials, Owner},
};
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{self, BufRead, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        audit::wall_time()
    }
    fn epoch_ms(&self) -> u64 {
        edu_academy_mcp::dailyrecord_read::wall_epoch_ms() as u64
    }
    fn uuid(&self) -> CoreResult<String> {
        audit::occurrence().map_err(|e| Fault::error(e.code))
    }
}
fn js(value: serde_json::Value) -> Js {
    parse(&serde_json::to_vec(&value).unwrap()).unwrap()
}
struct Literal;
impl CredentialProvider for Literal {
    fn credential(&mut self) -> SourceResult<Credentials> {
        Ok(Credentials::new(
            "fictional-user".into(),
            "fictional-password".into(),
        ))
    }
}
struct MemoryHttp(VecDeque<(String, String)>);
impl Transport for MemoryHttp {
    fn dispatch(&mut self, input: &Request) -> SourceResult<Response> {
        let (path, text) = self
            .0
            .pop_front()
            .expect("finite fictional HTTP budget; no retry");
        assert_eq!(input.path, path);
        Ok(Response {
            status: 200,
            headers: vec![],
            bytes: text.into_bytes(),
            url: format!("{ORIGIN}{path}"),
            stalled: false,
        })
    }
}
fn transport() -> MemoryHttp {
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
    .map(|k| format!("<input name=\"{k}\">"))
    .collect::<String>();
    MemoryHttp(vec![
        ("/c_login.jsp".into(), format!(r#"<form id="loginForm" method="post">{fields}</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>"#)),
        ("/servlet/controller.login.LoginServlet".into(), "fictional accepted login".into()),
        ("/servlet/controller.coursemanage.CourseManageServlet".into(), r#"<form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="fictional"></form><a>로그아웃</a>"#.into()),
        ("/servlet/controller.dailyzerotest.DailyZeroTestServlet".into(), r#"<script>var fictional={testResultList:JSON.parse('[{"priNo":"77001","userName":"Fictional Learner"}]')};</script>"#.into()),
    ].into())
}
struct Policy;
impl review::Policy for Policy {
    fn parent_pins(&self) -> Js {
        Js::Null
    }
    fn learning_pins(&self) -> Js {
        Js::Null
    }
    fn check_lines(&self, _: &Js) -> bool {
        true
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
struct FictionalSource;
impl Source for FictionalSource {
    fn read(&mut self, scope: &Js, _: &Js, _: &Js) -> CoreResult<Js> {
        let mut target = scope.clone();
        for (k, v) in [
            ("courseId", "course"),
            ("membershipId", "membership"),
            ("recordId", "current-record"),
        ] {
            target.set(k, v.into());
        }
        let mut previous = target.clone();
        previous.set("recordId", "previous-record".into());
        previous.set("lessonDate", "2040-01-03".into());
        let fields = obj(&review::FIELDS.map(|f| {
            (
                f,
                obj(&[
                    ("state", "known".into()),
                    (
                        "value",
                        match f {
                            "attendance" => "Y",
                            "dailyTestAssessment" | "previousHomeworkAssessment" => "0",
                            _ => "- Existing fictional text",
                        }
                        .into(),
                    ),
                    ("evidenceRefs", Js::Arr(vec!["fictional:source".into()])),
                ]),
            )
        }));
        Ok(obj(&[
            ("status", "observed".into()),
            (
                "observation",
                obj(&[
                    ("scope", scope.clone()),
                    ("target", target),
                    ("previousTarget", previous),
                    ("observedAt", audit::wall_time().into()),
                    ("sourceRevision", "1".repeat(64).into()),
                    ("wireDigest", "2".repeat(64).into()),
                    ("fields", fields),
                    ("toneSamples", Js::Arr(vec![])),
                    ("transmissionState", "not_sent".into()),
                ]),
            ),
            (
                "subject",
                obj(&[("displayName", "Fictional Learner".into())]),
            ),
        ]))
    }
}
struct NoWrite;
impl Backend for NoWrite {
    fn configured(&self) -> bool {
        false
    }
    fn save(&mut self, _: &Js, _: &Js, _: &mut dyn Approval, _: &Js) -> CoreResult<Js> {
        panic!("fictional parent get must never save")
    }
}
fn append(path: &Path, value: &Js) {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(&encode(value).unwrap()).unwrap();
    f.write_all(b"\n").unwrap();
    f.sync_all().unwrap();
}
fn main() {
    process::child_parent_guard().unwrap();
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 5);
    assert_eq!(args[1], "--install-root");
    assert_eq!(args[3], "--endpoint");
    assert_eq!(args[4], "dailyrecord");
    let root = Path::new(&args[2]);
    let graph = ReleaseGraph::open(root).unwrap();
    edu_academy_mcp::identity::verify_executable(
        &graph.endpoints["dailyrecord"].executable,
        &std::env::current_exe().unwrap(),
    )
    .unwrap();
    let config = parse(&fs::read(root.join("fictional-config.json")).unwrap()).unwrap();
    assert_eq!(config.get("fictionalOnly"), &Js::Bool(true));
    let state = graph.owner.state_root.clone();
    let clock = Clock;
    let policy = Policy;
    let mut source = FictionalSource;
    let mut backend = NoWrite;
    let store = Store::open(&state.join("workflow"), &state).unwrap();
    let admission = js(
        serde_json::json!({"teacherId":"99001","teacherLabel":"Fictional Teacher","date":"2040-01-04","sourceBundleSha256":"b".repeat(64),"students":[{"studentId":"77001","displayName":"Fictional Learner"}]}),
    );
    let client = Client::new(js(serde_json::json!({"teacherLabel":"Fictional Teacher","teacherKey":"99001","collegeKey":"88001"})), transport()).unwrap();
    let owner = Owner::new(
        client,
        admission,
        None,
        vec!["Fictional Learner".into()],
        Literal,
        &clock,
    )
    .unwrap();
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
    let mut session = RpcSession::endpoint(graph, "dailyrecord".into()).unwrap();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let request = parse(line.unwrap().as_bytes()).unwrap();
        let identity = process::Identity::read(std::process::id()).unwrap();
        append(
            &state.join("child-requests.jsonl"),
            &obj(&[
                ("pid", Js::Num(f64::from(identity.pid))),
                ("startTicks", identity.start.to_string().into()),
                ("request", request.clone()),
            ]),
        );
        let intake = request
            .get("params")
            .get("name")
            .is("academy_dailyrecord_intake");
        if intake && config.get("mode").is("stall_intake") {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let response = session.handle_business(&request, &mut driver).unwrap();
        if intake && config.get("mode").is("close_after_intake") {
            return;
        }
        if let Some(response) = response {
            append(&state.join("child-responses.jsonl"), &response);
            stdout.write_all(&encode(&response).unwrap()).unwrap();
            stdout.write_all(b"\n").unwrap();
            stdout.flush().unwrap();
        }
    }
}
