//! TEST ONLY, harness=false. Fictional wire/ownership faults; no domain port or
//! product answer. Never invoked automatically without an explicit fixture root.
#![allow(dead_code)]
use edu_academy_mcp::{Js, audit, obj, parse, process, release::ReleaseGraph};
use std::{
    fs,
    io::{self, BufRead, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

fn put_new(path: &Path, bytes: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
fn identity_value() -> Js {
    let p = process::Identity::read(std::process::id()).unwrap();
    obj(&[
        ("pid", Js::Num(f64::from(p.pid))),
        ("startTicks", p.start.to_string().into()),
        ("parent", Js::Num(f64::from(p.parent))),
        ("group", Js::Num(f64::from(p.group))),
    ])
}
fn descendant(state: &Path) {
    process::child_parent_guard().unwrap();
    rustix::process::setsid().unwrap();
    let _signals = signal_hook::iterator::Signals::new([signal_hook::consts::SIGTERM]).unwrap();
    put_new(
        &state.join("descendant.json"),
        &edu_academy_mcp::encode(&identity_value()).unwrap(),
    );
    loop {
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn peer(root: &Path, endpoint: &str) {
    process::child_parent_guard().unwrap();
    let graph = ReleaseGraph::open(root).unwrap();
    let state = &graph.owner.state_root;
    let entry = graph.endpoints.get(endpoint).unwrap();
    edu_academy_mcp::identity::verify_executable(
        &entry.executable,
        &std::env::current_exe().unwrap(),
    )
    .unwrap();
    let config = parse(&fs::read(root.join("session-config.json")).unwrap()).unwrap();
    assert_eq!(config.get("fixtureOnly"), &Js::Bool(true));
    let mode = config.get("mode").code_units();
    let _signals = if mode == "ignore_term" {
        Some(signal_hook::iterator::Signals::new([signal_hook::consts::SIGTERM]).unwrap())
    } else {
        None
    };
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut lock = None;
    let mut descendant_child = None;
    for line in stdin.lock().lines() {
        let message = parse(line.unwrap().as_bytes()).unwrap();
        if !message.has("id") {
            continue;
        }
        let method = message.get("method");
        let params = message.get("params");
        let result = if method.is("initialize") {
            obj(&[
                ("protocolVersion", params.get("protocolVersion").clone()),
                ("capabilities", obj(&[("tools", obj(&[]))])),
                (
                    "serverInfo",
                    obj(&[
                        ("name", "fictional-stdio-peer".into()),
                        ("version", "1".into()),
                    ]),
                ),
            ])
        } else if method.is("tools/list") {
            if mode == "before_call" {
                put_new(
                    &state.join("before-call.json"),
                    &edu_academy_mcp::encode(&identity_value()).unwrap(),
                );
                loop {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            obj(&[(
                "tools",
                Js::Arr(if mode == "missing_inventory" {
                    vec![]
                } else {
                    vec![obj(&[
                        ("name", config.get("tool").clone()),
                        ("inputSchema", obj(&[("type", "object".into())])),
                    ])]
                }),
            )])
        } else if method.is("tools/call") && params.get("name").is("academy_endpoint_identity") {
            audit::tool_result(graph.endpoint_identity(endpoint).unwrap()).unwrap()
        } else if method.is("tools/call") && params.get("name").is("academy_runtime_read_admission")
        {
            let admission = edu_academy_mcp::upstream::read_admission(&graph.admission().unwrap());
            audit::tool_result(obj(&[
                ("status", "configured".into()),
                (
                    "admissionSha256",
                    edu_academy_mcp::digest(&edu_academy_mcp::encode(&admission).unwrap()).into(),
                ),
            ]))
            .unwrap()
        } else if method.is("tools/call") {
            assert_eq!(params.get("name"), config.get("tool"));
            let file = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(state.join("fixture.lock"))
                .unwrap();
            rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive).unwrap();
            lock = Some(file);
            if mode == "ignore_term" {
                let parent = process::Identity::read(std::process::id()).unwrap();
                descendant_child = Some(
                    std::process::Command::new(std::env::current_exe().unwrap())
                        .arg("--fixture-descendant")
                        .arg(state)
                        .env_clear()
                        .env("ACADEMY_MCP_PARENT_PID", parent.pid.to_string())
                        .env("ACADEMY_MCP_PARENT_START", parent.start.to_string())
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::inherit())
                        .stderr(std::process::Stdio::inherit())
                        .spawn()
                        .unwrap(),
                );
                setup::wait_file(&state.join("descendant.json"), Duration::from_secs(3));
            }
            let mut called = identity_value();
            let metadata = lock.as_ref().unwrap().metadata().unwrap();
            called.set("lockDevice", metadata.dev().to_string().into());
            called.set("lockInode", metadata.ino().to_string().into());
            put_new(
                &state.join("called.json"),
                &edu_academy_mcp::encode(&called).unwrap(),
            );
            if matches!(mode.as_str(), "hold" | "ignore_term") {
                loop {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            if mode == "flood" {
                let chunk = [b'x'; 65536];
                loop {
                    stdout.write_all(&chunk).unwrap();
                    stdout.flush().unwrap();
                }
            }
            let data = if mode == "upstream_error" {
                obj(&[
                    ("status", "blocked".into()),
                    (
                        "failure",
                        obj(&[
                            ("code", "query_outside_declared_scope".into()),
                            ("message", "private-upstream-message-must-not-escape".into()),
                        ]),
                    ),
                    ("occurrence", "private-invalid-occurrence".into()),
                ])
            } else if mode == "large" {
                obj(&[
                    ("fixtureOnly", true.into()),
                    ("payload", "x".repeat(600000).into()),
                ])
            } else {
                obj(&[
                    ("fixtureOnly", true.into()),
                    (
                        "observedAttribution",
                        edu_academy_core::workflow::nullable(
                            params.get("arguments").get("attribution"),
                        ),
                    ),
                ])
            };
            let mut result = audit::tool_result(data).unwrap();
            if mode == "upstream_error" {
                result.set("isError", true.into());
            }
            result.set("_meta", obj(&[("fixture/preserved", true.into())]));
            result
        } else {
            panic!("unsupported fictional method");
        };
        edu_academy_mcp::framing::write_message(
            &mut stdout,
            &edu_academy_mcp::protocol::rpc_result(message.get("id"), result),
        )
        .unwrap();
    }
    drop(lock);
    drop(descendant_child);
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() == 2 && args[0] == "--fixture-descendant" {
        descendant(Path::new(&args[1]));
        return;
    }
    if args.len() != 4 || args[0] != "--install-root" || args[2] != "--endpoint" {
        eprintln!("explicit_fictional_fixture_root_required");
        std::process::exit(2);
    }
    peer(Path::new(&args[1]), &args[3]);
}

#[path = "../common/mod.rs"]
mod common;

/// Shared NEW-test setup only. Old common test source is included without edits.
pub mod setup {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        io::Read,
        os::unix::{
            fs::{MetadataExt, PermissionsExt},
            process::ExitStatusExt,
        },
        process::{Child, Command, ExitStatus, Stdio},
        time::{Duration, Instant},
    };
    pub struct Scene {
        pub install: PathBuf,
        pub state: PathBuf,
        pub evidence: PathBuf,
        pub pointer: Vec<u8>,
    }
    fn pin(path: &Path) -> Value {
        json!({"path":path,"sha256":edu_academy_mcp::digest(&fs::read(path).unwrap())})
    }
    pub fn scene(mode: &str, tool: &str) -> Scene {
        let fixture = common::Fixture::new();
        let install = fixture.install;
        let root = install.parent().unwrap();
        let native = PathBuf::from(
            std::env::var_os("S11_MCP_BINARY").expect("explicit compiled MCP binary required"),
        );
        let peer = PathBuf::from(
            std::env::var_os("S11_STDIO_PEER").expect("explicit compiled stdio fixture required"),
        );
        assert!(native.is_absolute() && peer.is_absolute());
        fs::copy(native, install.join("bridge")).unwrap();
        fs::set_permissions(install.join("bridge"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::copy(peer, install.join("peer")).unwrap();
        fs::set_permissions(install.join("peer"), fs::Permissions::from_mode(0o700)).unwrap();
        put_new(
            &install.join("session-config.json"),
            &serde_json::to_vec(&json!({"fixtureOnly":true,"mode":mode,"tool":tool})).unwrap(),
        );
        let args = vec![
            "--install-root".to_owned(),
            install.to_str().unwrap().into(),
            "--endpoint".into(),
            "saved".into(),
        ];
        let mut entry = pin(&install.join("peer"));
        entry["role"] = json!("entry");
        let mut config = pin(&install.join("session-config.json"));
        config["role"] = json!("config");
        let identity = json!({"schemaVersion":"academy.rust-endpoint/1","endpoint":"saved",
            "semanticContractVersion":"s11-fictional","launchArgumentsSha256":edu_academy_mcp::digest(&serde_json::to_vec(&args).unwrap()),
            "artifacts":[entry,config]});
        put_new(
            &install.join("session-identity.json"),
            &serde_json::to_vec(&identity).unwrap(),
        );
        fs::write(install.join("endpoints.json"),serde_json::to_vec(&json!({"saved":{
            "executable":pin(&install.join("peer")),"identity":pin(&install.join("session-identity.json")),"args":args}})).unwrap()).unwrap();
        fs::write(
            install.join("registry.json"),
            serde_json::to_vec(&json!([{"endpoint":"saved","upstreamName":tool,
            "tool":{"name":tool,"description":"Fictional wire fixture, no domain implementation",
                "inputSchema":{"type":"object"}}}]))
            .unwrap(),
        )
        .unwrap();
        let mut pointer: Value = serde_json::from_slice(&fixture.pointer).unwrap();
        pointer["bridge"] = pin(&install.join("bridge"));
        pointer["endpoints"] = pin(&install.join("endpoints.json"));
        pointer["registry"] = pin(&install.join("registry.json"));
        let pointer = serde_json::to_vec(&pointer).unwrap();
        fs::write(install.join("release.json"), &pointer).unwrap();
        Scene {
            install: install.clone(),
            state: root.join("state"),
            evidence: root.join("evidence"),
            pointer,
        }
    }
    pub fn wait_file(path: &Path, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while fs::read(path)
            .ok()
            .is_none_or(|bytes| parse(&bytes).is_err())
        {
            assert!(Instant::now() < deadline, "fictional marker deadline");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn changed_graph(scene: &Scene) {
        let mut pointer: Value = serde_json::from_slice(&scene.pointer).unwrap();
        pointer["generation"] = json!("fictional-switched");
        let bytes = serde_json::to_vec(&pointer).unwrap();
        put_new(&scene.install.join("switch.json"), &bytes);
        fs::rename(
            scene.install.join("switch.json"),
            scene.install.join("release.json"),
        )
        .unwrap();
    }
    pub fn receipts(scene: &Scene, suffix: &str) -> Vec<Js> {
        let dir = scene.evidence.join("receipts");
        let Ok(entries) = fs::read_dir(dir) else {
            return vec![];
        };
        entries
            .filter_map(|e| {
                let path = e.unwrap().path();
                if !path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .ends_with(suffix)
                {
                    return None;
                }
                // EXCL receipts are visible before their last write/fsync.
                // Observe complete JSON; never retry the product operation.
                parse(&fs::read(path).unwrap()).ok()
            })
            .collect()
    }
    pub fn wait_receipt(scene: &Scene, suffix: &str) -> Vec<Js> {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let found = receipts(scene, suffix);
            if !found.is_empty() {
                return found;
            }
            assert!(Instant::now() < deadline, "receipt deadline");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn assert_gone(identity: &Js) {
        let pid = identity.get("pid");
        let Js::Num(pid) = pid else {
            panic!("native identity required");
        };
        let start = identity
            .get("startTicks")
            .code_units()
            .parse::<u64>()
            .unwrap();
        assert!(
            process::Identity::read(*pid as u32).is_none_or(|v| v.start != start),
            "owned identity remains"
        );
    }
    pub fn assert_lock_released(scene: &Scene) {
        let path = scene.state.join("fixture.lock");
        let before = fs::metadata(&path).unwrap();
        let called = parse(&fs::read(scene.state.join("called.json")).unwrap()).unwrap();
        assert!(called.get("lockDevice").is(&before.dev().to_string()));
        assert!(called.get("lockInode").is(&before.ino().to_string()));
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive).unwrap();
        assert_eq!(
            (before.dev(), before.ino()),
            (
                file.metadata().unwrap().dev(),
                file.metadata().unwrap().ino()
            )
        );
    }
    pub struct Bridge {
        pub child: Child,
        pending: Vec<u8>,
        capture: PathBuf,
        sequence: u32,
        finished: bool,
    }
    impl Bridge {
        pub fn start(scene: &Scene) -> Self {
            let child = Command::new(scene.install.join("bridge"))
                .arg("--install-root")
                .arg(&scene.install)
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::from(
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(scene.evidence.join("bridge.stderr"))
                        .unwrap(),
                ))
                .spawn()
                .unwrap();
            let out = child.stdout.as_ref().unwrap();
            let flags = rustix::fs::fcntl_getfl(out).unwrap();
            rustix::fs::fcntl_setfl(out, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
            let mut bridge = Self {
                child,
                pending: vec![],
                capture: scene.evidence.clone(),
                sequence: 0,
                finished: false,
            };
            // Observe the native bridge's initialized I/O and signal handlers
            // before starting marker/cancellation/graph-switch assertions.
            bridge.send(&request("fixture-ready".into(), "ping", obj(&[])));
            let ready = bridge.receive();
            assert!(
                ready.get("id").is("fixture-ready") && ready.has("result"),
                "{ready:?}"
            );
            bridge
        }
        pub fn send(&mut self, message: &Js) {
            edu_academy_mcp::framing::write_message(self.child.stdin.as_mut().unwrap(), message)
                .unwrap();
        }
        pub fn receive(&mut self) -> Js {
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                if let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
                    let bytes = self.pending.drain(..=end).collect::<Vec<_>>();
                    self.sequence += 1;
                    put_new(
                        &self
                            .capture
                            .join(format!("response-{}.json", self.sequence)),
                        &bytes,
                    );
                    return parse(&bytes).unwrap();
                }
                let mut bytes = [0u8; 65536];
                match self.child.stdout.as_mut().unwrap().read(&mut bytes) {
                    Ok(0) => panic!("bridge closed before response"),
                    Ok(n) => self.pending.extend_from_slice(&bytes[..n]),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => panic!("fictional pipe error: {e}"),
                }
                assert!(
                    self.pending.len() <= edu_academy_mcp::framing::SDK_CHILD_RECEIVE_BUFFER_BYTES
                );
                assert!(Instant::now() < deadline, "response deadline");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        pub fn eof(&mut self) {
            self.child.stdin.take();
        }
        pub fn term(&mut self) {
            rustix::process::kill_process(
                rustix::process::Pid::from_raw(self.child.id() as i32).unwrap(),
                rustix::process::Signal::TERM,
            )
            .unwrap();
        }
        pub fn finish(&mut self) -> ExitStatus {
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    self.finished = true;
                    return status;
                }
                assert!(Instant::now() < deadline, "bridge exit deadline");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        pub fn close(&mut self) {
            self.eof();
            assert!(self.finish().success());
        }
    }
    impl Drop for Bridge {
        fn drop(&mut self) {
            if self.finished {
                return;
            }
            let _ = rustix::process::kill_process(
                rustix::process::Pid::from_raw(self.child.id() as i32).unwrap(),
                rustix::process::Signal::TERM,
            );
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let _ = self.child.kill();
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            // Do not pretend a failed cleanup was verified; Root's effect readback owns it.
        }
    }
    pub fn request(id: Js, method: &str, params: Js) -> Js {
        obj(&[
            ("jsonrpc", "2.0".into()),
            ("id", id),
            ("method", method.into()),
            ("params", params),
        ])
    }
    pub fn call(name: &str) -> Js {
        request(
            Js::Num(1.0),
            "tools/call",
            obj(&[("name", name.into()), ("arguments", obj(&[]))]),
        )
    }
    pub fn signal(status: ExitStatus) -> Option<i32> {
        status.signal()
    }
}
