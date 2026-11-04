//! SOURCE-ONLY, UNEXECUTED finite history source child for dailyrecord/paper1/paper2.
//! Real RpcSession + business::History + source algorithms, raw fictional inputs.
use edu_academy_mcp::{
    Js, business, encode, obj, parse, process, protocol::RpcSession, release::ReleaseGraph,
};
use edu_academy_sources::{
    data::{Error, Result},
    history::{History, Port},
};
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{self, BufRead, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
fn append(path: &Path, value: &Js) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(&encode(value).unwrap()).unwrap();
    file.write_all(b"\n").unwrap();
    file.sync_all().unwrap();
}
struct Fictional {
    captures: VecDeque<Js>,
    events: PathBuf,
}
impl Fictional {
    fn take(&mut self, operation: &str, input: &Js) -> Result<Js> {
        let capture = self
            .captures
            .pop_front()
            .expect("finite source budget; no retry");
        assert!(capture.get("operation").is(operation));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&encode(capture.get("input")).unwrap())
                .unwrap(),
            serde_json::from_slice::<serde_json::Value>(&encode(input).unwrap()).unwrap()
        );
        append(
            &self.events,
            &obj(&[
                ("event", "source".into()),
                ("operation", operation.into()),
                ("input", input.clone()),
            ]),
        );
        if capture.has("error") {
            return Err(Error::error(capture.get("error").code_units()));
        }
        Ok(capture.get("raw").clone())
    }
}
impl Port for Fictional {
    fn dated_students(&mut self, input: &Js) -> Result<Js> {
        self.take("dated_students", input)
    }
    fn daily_page(&mut self, student: &Js, selector: &Js) -> Result<Js> {
        self.take(
            "daily_page",
            &obj(&[("student", student.clone()), ("selector", selector.clone())]),
        )
    }
    fn paper_header(&mut self, student: &Js, code: &Js) -> Result<Js> {
        self.take(
            "paper_header",
            &obj(&[("studentId", student.clone()), ("paperCode", code.clone())]),
        )
    }
}
fn main() {
    process::child_parent_guard().unwrap();
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 5);
    assert_eq!(args[1], "--install-root");
    assert_eq!(args[3], "--endpoint");
    let endpoint = &args[4];
    assert!(["dailyrecord", "paper1", "paper2"].contains(&endpoint.as_str()));
    let install = Path::new(&args[2]);
    let graph = ReleaseGraph::open(install).unwrap();
    edu_academy_mcp::identity::verify_executable(
        &graph.endpoints[endpoint].executable,
        &std::env::current_exe().unwrap(),
    )
    .unwrap();
    let config = parse(&fs::read(install.join("fictional-config.json")).unwrap()).unwrap();
    assert_eq!(config.get("fictionalOnly"), &Js::Bool(true));
    let state = graph.owner.state_root.clone();
    // Fictional local contention witness, unrelated to any Academy account lock.
    // Keep the file; kernel lock lifetime is exactly this owned process lifetime.
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(state.join("fictional-shared.flock"))
        .unwrap();
    lock.try_lock()
        .expect("previous fictional child must have fully closed");
    let identity = process::Identity::read(std::process::id()).unwrap();
    let events = state.join("history-events.jsonl");
    append(
        &events,
        &obj(&[
            ("event", "open".into()),
            ("endpoint", endpoint.clone().into()),
            ("pid", (identity.pid as usize).into()),
            ("startTicks", identity.start.to_string().into()),
        ]),
    );
    let source = History::new(
        Fictional {
            captures: config.get("captures").get(endpoint).array().to_vec().into(),
            events: events.clone(),
        },
        graph.admission().unwrap(),
        config.get("metadataAdmission").clone(),
        config.get("students").array().to_vec(),
        config.get("managedNames").array().to_vec(),
        config.get("paperStudents").get(endpoint).array().to_vec(),
    )
    .unwrap();
    let mut source = business::History::from_source(source);
    let mut session = RpcSession::endpoint(graph, endpoint.clone()).unwrap();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let request = parse(line.unwrap().as_bytes()).unwrap();
        append(&state.join(format!("{endpoint}-requests.jsonl")), &request);
        let response = session.handle_history(&request, &mut source).unwrap();
        if let Some(mut response) = response {
            // Negative transport faults are applied only after the actual
            // session/driver/source algorithms have produced their response.
            // Never replace the source read with a canned handler success.
            let tool = request.get("params").get("name");
            let fault = config.get("responseFault");
            if fault.is("metadata") && tool.is("academy_student_metadata_get")
                || fault.is("paper") && tool.is("academy_paper_headers")
            {
                let mut data = response.get("result").get("structuredContent").clone();
                assert!(data.get("status").is("observed"));
                if fault.is("metadata") {
                    let mut scope = data.get("scope").clone();
                    scope.set("sourceNamespace", "fictional.malformed.namespace".into());
                    data.set("scope", scope);
                } else {
                    let mut observation = data.get("observation").clone();
                    let mut rows = observation.get("rows").array().to_vec();
                    let mut header = rows[0].get("contentHeader").clone();
                    header.set("sourceSha256", "fictional malformed hash".into());
                    rows[0].set("contentHeader", header);
                    observation.set("rows", Js::Arr(rows));
                    data.set("observation", observation);
                }
                append(
                    &events,
                    &obj(&[
                        ("event", "transport-fault".into()),
                        ("source", fault.clone()),
                    ]),
                );
                response.set("result", edu_academy_mcp::audit::tool_result(data).unwrap());
            }
            append(
                &state.join(format!("{endpoint}-responses.jsonl")),
                &response,
            );
            stdout.write_all(&encode(&response).unwrap()).unwrap();
            stdout.write_all(b"\n").unwrap();
            stdout.flush().unwrap();
        }
    }
    append(
        &events,
        &obj(&[
            ("event", "close".into()),
            ("endpoint", endpoint.clone().into()),
            ("pid", (identity.pid as usize).into()),
            ("startTicks", identity.start.to_string().into()),
        ]),
    );
    drop(lock);
}
