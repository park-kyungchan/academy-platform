//! R03 synthetic native qualification. All sockets are literal loopback; random
//! disposable credential bytes never appear in fixtures, diagnostics or traces.
use edu_academy_core::{
    js::Result as CoreResult,
    workflow::{Runtime, approval::Approval, canonical},
};
use edu_academy_sources::{
    data::*,
    http::{Cancellation, NativeTransport, Request, Response, Transport, TransportLimits},
    owner_admission::{TransportAdmission, TransportScope},
    owner_root::OwnerRoot,
    owner_workflow::{
        CredentialProvider, Credentials, EffectIntent, FileCredentialProvider, NativeOwnerInputs,
        NativeSession, PinnedInput,
    },
};
use std::{
    cell::Cell,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener},
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
struct Clock(Cell<u64>);
impl Runtime for Clock {
    fn now(&self) -> String {
        "2040-01-04T09:00:00.000Z".into()
    }
    fn uuid(&self) -> CoreResult<String> {
        Ok("synthetic-native-owner".into())
    }
    fn epoch_ms(&self) -> u64 {
        self.0.get()
    }
}
fn random() -> String {
    let mut b = [0u8; 24];
    fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut b)
        .unwrap();
    hash(&b)
}
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = PathBuf::from(
            std::env::var("S09_NATIVE_FIXTURE_ROOT").expect("explicit synthetic root"),
        )
        .join(format!(
            "e07-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
    fn root(&self) -> Rc<OwnerRoot> {
        Rc::new(OwnerRoot::open(&self.0).unwrap())
    }
    fn pinned(&self, name: &str, bytes: &[u8]) -> PinnedInput {
        fs::write(self.0.join(name), bytes).unwrap();
        fs::set_permissions(self.0.join(name), fs::Permissions::from_mode(0o600)).unwrap();
        PinnedInput {
            relative_path: name.into(),
            sha256: hash(bytes),
        }
    }
    fn credential<'a>(&self, clock: &'a Clock, expires: u64) -> FileCredentialProvider<'a> {
        let input = self.pinned(
            "credential",
            &serde_json::to_vec(
                &serde_json::json!({"schemaVersion":"academy.owner-credential/1",
            "username":random(),"password":random(),"expiresAtMs":expires}),
            )
            .unwrap(),
        );
        FileCredentialProvider::new(self.root(), input, clock)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn scope() -> TransportScope {
    TransportScope {
        owner_id: "synthetic-owner".into(),
        source_namespace: "academy.lms.metadata".into(),
        teacher_id: "99001".into(),
        student_id: "77001".into(),
        date: "2040-01-04".into(),
    }
}
fn policy() -> TransportAdmission {
    TransportAdmission::from_bytes(&serde_json::to_vec(&serde_json::json!({
        "schemaVersion":"academy.transport-admission/1","ownerId":"synthetic-owner","teacherId":"99001",
        "teacherLabel":"Fictional Teacher","collegeId":"88001","allowedFrom":"2040-01-01","allowedThrough":"2040-01-31",
        "studentIds":["77001"],"sourceNamespaces":["academy.lms.metadata","academy.lms.dayrecord"],
        "operations":[
            {"id":"read","sourceNamespace":"academy.lms.metadata","method":"GET","path":"/synthetic/read","effect":false,
             "query":[{"name":"student","value":{"kind":"scope","field":"studentId"}},
                      {"name":"date","value":{"kind":"scope","field":"date"}}],"body":[]},
            {"id":"write","sourceNamespace":"academy.lms.metadata","method":"GET","path":"/synthetic/write","effect":true,
             "query":[{"name":"value","value":{"kind":"argument","name":"after","max_bytes":100}},
                      {"name":"student","value":{"kind":"scope","field":"studentId"}}],"body":[]},
            {"id":"dayread","sourceNamespace":"academy.lms.dayrecord","method":"POST","path":"/synthetic/dayread","effect":false,
             "query":[],"body":[{"name":"student","value":{"kind":"scope","field":"studentId"}}]}
        ]})).unwrap()).unwrap()
}
struct Reply {
    path: &'static str,
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    delay: Duration,
    drop_connection: bool,
    unframed: bool,
    expect_peer_closed: bool,
}
fn reply(path: &'static str, body: &str) -> Reply {
    Reply {
        path,
        status: 200,
        headers: vec![],
        body: body.as_bytes().into(),
        delay: Duration::ZERO,
        drop_connection: false,
        unframed: false,
        expect_peer_closed: false,
    }
}
fn login() -> Vec<Reply> {
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
    .map(|name| format!("<input name=\"{name}\">"))
    .collect::<String>();
    let mut first = reply(
        "/c_login.jsp",
        &format!(
            r#"<form id="loginForm" method="post">{fields}</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>"#
        ),
    );
    first.headers.push((
        "Set-Cookie".into(),
        format!("JSESSIONID={}; Path=/; HttpOnly", random()),
    ));
    vec![
        first,
        reply("/servlet/controller.login.LoginServlet", "accepted"),
        reply(
            "/servlet/controller.coursemanage.CourseManageServlet",
            r#"<form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="fictional"></form><a>로그아웃</a>"#,
        ),
    ]
}
struct Server {
    address: SocketAddr,
    seen: Arc<Mutex<Vec<String>>>,
    task: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(vec![]));
        let capture = Arc::clone(&seen);
        let task = thread::spawn(move || {
            for response in replies {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "synthetic server request deadline"
                            );
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => panic!("synthetic accept failed"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = vec![];
                let header_end = loop {
                    let mut b = [0u8; 1];
                    stream.read_exact(&mut b).unwrap();
                    bytes.push(b[0]);
                    assert!(bytes.len() < 300_000);
                    if bytes.ends_with(b"\r\n\r\n") {
                        break bytes.len();
                    }
                };
                let headers = String::from_utf8_lossy(&bytes);
                let first = headers.lines().next().unwrap();
                assert!(
                    first
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .starts_with(response.path)
                );
                let cookie = headers
                    .lines()
                    .any(|v| v.to_ascii_lowercase().starts_with("cookie: jsessionid="));
                if response.path != "/c_login.jsp" {
                    assert!(cookie, "session cookie must remain owner-local");
                }
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                // Record path/method only; never credentials, query values or cookies.
                capture.lock().unwrap().push(format!(
                    "{} {}",
                    first.split_whitespace().next().unwrap(),
                    response.path
                ));
                drop(body);
                bytes.truncate(header_end);
                if response.drop_connection {
                    continue;
                }
                let mut head = format!(
                    "HTTP/1.1 {} Synthetic\r\nContent-Length: {}\r\nConnection: close\r\n",
                    response.status,
                    response.body.len()
                );
                if response.unframed {
                    head = format!(
                        "HTTP/1.1 {} Synthetic\r\nConnection: close\r\n",
                        response.status
                    );
                }
                for (k, v) in response.headers {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                thread::sleep(response.delay);
                let _ = stream.write_all(&response.body);
                if response.expect_peer_closed {
                    let result = stream.read(&mut [0u8; 1]);
                    assert!(
                        matches!(result, Ok(0))
                            || result.is_err_and(|e| matches!(
                                e.kind(),
                                std::io::ErrorKind::ConnectionReset
                                    | std::io::ErrorKind::BrokenPipe
                            ))
                    );
                }
            }
        });
        Self {
            address,
            seen,
            task: Some(task),
        }
    }
    fn transport(&self) -> NativeTransport {
        NativeTransport::synthetic_loopback(
            self.address,
            TransportLimits {
                timeout: Duration::from_millis(250),
                ..TransportLimits::default()
            },
            Cancellation::default(),
        )
        .unwrap()
    }
    fn finish(mut self) -> Vec<String> {
        self.task.take().unwrap().join().unwrap();
        self.seen.lock().unwrap().clone()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.join().unwrap();
        }
    }
}
fn session<'a>(
    fixture: &Fixture,
    clock: &'a Clock,
    server: &Server,
) -> NativeSession<'a, NativeTransport> {
    NativeSession::open(
        fixture.root(),
        policy(),
        server.transport(),
        fixture.credential(clock, 100_000),
        Cancellation::default(),
        clock,
    )
    .unwrap()
}
struct Approve;
impl Approval for Approve {
    fn consume(&mut self, a: &Js) -> CoreResult<Js> {
        Ok(obj(&[
            ("status", "verified".into()),
            ("actionDigest", hash(canonical(a)?.as_bytes()).into()),
        ]))
    }
}
struct Reject;
impl Approval for Reject {
    fn consume(&mut self, _: &Js) -> CoreResult<Js> {
        Ok(obj(&[("status", "rejected".into())]))
    }
}
fn effect<'a>(
    scope: &'a TransportScope,
    after: &'a [(String, String)],
    expected: &'a str,
) -> EffectIntent<'a> {
    EffectIntent {
        scope,
        operation: "write",
        arguments: after,
        readback_operation: "read",
        readback_arguments: &[],
        expected_sha256: expected,
    }
}
#[test]
fn native_login_once_shared_families_expiry_and_exact_scope_before_io() {
    let root = Fixture::new();
    let clock = Clock(Cell::new(1000));
    let mut responses = login();
    responses.extend([
        reply("/synthetic/read", "first"),
        reply("/synthetic/dayread", "second"),
    ]);
    let server = Server::new(responses);
    let mut owner = session(&root, &clock, &server);
    let mut bad = scope();
    bad.owner_id = "foreign".into();
    assert!(owner.read(&bad, "read", &[]).is_err());
    bad = scope();
    bad.teacher_id = "99002".into();
    assert!(owner.read(&bad, "read", &[]).is_err());
    bad = scope();
    bad.student_id = "77002".into();
    assert!(owner.read(&bad, "read", &[]).is_err());
    bad = scope();
    bad.date = "2040-02-30".into();
    assert!(owner.read(&bad, "read", &[]).is_err());
    bad = scope();
    bad.date = "2040-02-01".into();
    assert!(owner.read(&bad, "read", &[]).is_err());
    assert!(owner.read(&scope(), "dayread", &[]).is_err());
    assert!(
        owner
            .read(&scope(), "write", &[("after".into(), "x".into())])
            .is_err()
    );
    assert!(
        owner
            .read(&scope(), "read", &[("reqCmd".into(), "write".into())])
            .is_err()
    );
    assert!(server.seen.lock().unwrap().is_empty());
    assert_eq!(owner.read(&scope(), "read", &[]).unwrap().bytes, b"first");
    let mut day = scope();
    day.source_namespace = "academy.lms.dayrecord".into();
    assert_eq!(owner.read(&day, "dayread", &[]).unwrap().bytes, b"second");
    clock.0.set(100_000);
    assert_eq!(
        owner.read(&scope(), "read", &[]).err().unwrap().message,
        "native_owner_credential_expired"
    );
    let seen = server.finish();
    assert_eq!(seen.len(), 5);
    assert_eq!(
        seen.iter().filter(|v| v.contains("/c_login.jsp")).count(),
        1
    );
}
#[test]
fn failed_login_retained_and_challenge_closes_without_relogin() {
    for malformed in [true, false] {
        let fixture = Fixture::new();
        let clock = Clock(Cell::new(1000));
        let responses = if malformed {
            vec![reply("/c_login.jsp", "<html>malformed</html>")]
        } else {
            let mut replies = login();
            replies.push(reply("/synthetic/read", "<input type='PASSWORD'>"));
            replies
        };
        let server = Server::new(responses);
        let mut owner = session(&fixture, &clock, &server);
        for _ in 0..2 {
            assert!(owner.read(&scope(), "read", &[]).is_err());
        }
        assert_eq!(server.finish().len(), if malformed { 1 } else { 4 });
    }
}
#[test]
fn pinned_credentials_modes_links_hash_missing_expiry_and_sanitized_errors() {
    let root = Fixture::new();
    let clock = Clock(Cell::new(1000));
    let credential=root.pinned("credential",&serde_json::to_vec(&serde_json::json!({"schemaVersion":"academy.owner-credential/1","username":random(),"password":random(),"expiresAtMs":500})).unwrap());
    let mut provider = FileCredentialProvider::new(root.root(), credential.clone(), &clock);
    assert_eq!(
        provider.credential().err().unwrap().message,
        "native_owner_credential_refused"
    );
    clock.0.set(0);
    assert!(provider.credential().is_ok());
    fs::set_permissions(root.0.join("credential"), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(provider.credential().is_err());
    fs::set_permissions(root.0.join("credential"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(root.0.join("credential"), root.0.join("hardlink")).unwrap();
    assert!(provider.credential().is_err());
    fs::remove_file(root.0.join("hardlink")).unwrap();
    symlink(root.0.join("credential"), root.0.join("symlink")).unwrap();
    assert!(
        FileCredentialProvider::new(
            root.root(),
            PinnedInput {
                relative_path: "symlink".into(),
                ..credential.clone()
            },
            &clock
        )
        .credential()
        .is_err()
    );
    assert!(
        FileCredentialProvider::new(
            root.root(),
            PinnedInput {
                relative_path: "../credential".into(),
                ..credential.clone()
            },
            &clock
        )
        .credential()
        .is_err()
    );
    fs::write(root.0.join("credential"), random()).unwrap();
    assert!(provider.credential().is_err());
    fs::remove_file(root.0.join("credential")).unwrap();
    assert!(provider.credential().is_err());
}
#[test]
fn uncertain_effect_fences_repeated_and_reopened_sessions_until_actual_readback() {
    let root = Fixture::new();
    let clock = Clock(Cell::new(1000));
    let username = random();
    let password = random();
    let credentials = || Ok(Credentials::new(username.clone(), password.clone()));
    let after = vec![("after".into(), "After".into())];
    let expected = hash(b"After");
    let scope = scope();
    let mut replies = login();
    let mut ambiguous = reply("/synthetic/write", "");
    ambiguous.drop_connection = true;
    replies.push(ambiguous);
    let server = Server::new(replies);
    let mut owner = NativeSession::open(
        root.root(),
        policy(),
        server.transport(),
        credentials,
        Cancellation::default(),
        &clock,
    )
    .unwrap();
    assert!(
        owner
            .effect(effect(&scope, &after, &expected), &mut Reject)
            .is_err()
    );
    assert!(!owner.reconciliation_required().unwrap());
    assert_eq!(
        owner
            .effect(effect(&scope, &after, &expected), &mut Approve)
            .err()
            .unwrap()
            .message,
        "native_effect_uncertain"
    );
    assert!(owner.reconciliation_required().unwrap());
    assert!(
        owner
            .effect(effect(&scope, &after, &expected), &mut Approve)
            .is_err()
    );
    assert_eq!(server.finish().len(), 4);
    drop(owner);
    let mut replies = login();
    replies.extend([
        reply("/synthetic/read", "Before"),
        reply("/synthetic/read", "After"),
        reply("/synthetic/write", "accepted"),
        reply("/synthetic/read", "After"),
    ]);
    let server = Server::new(replies);
    let mut owner = NativeSession::open(
        root.root(),
        policy(),
        server.transport(),
        credentials,
        Cancellation::default(),
        &clock,
    )
    .unwrap();
    assert!(
        owner
            .effect(effect(&scope, &after, &expected), &mut Approve)
            .is_err()
    );
    assert!(server.seen.lock().unwrap().is_empty());
    let mut wrong = scope.clone();
    wrong.source_namespace = "academy.lms.dayrecord".into();
    assert!(owner.reconcile(&wrong, "dayread", &[]).is_err());
    assert!(owner.reconcile(&scope, "read", &[]).is_err());
    assert!(owner.reconciliation_required().unwrap());
    assert_eq!(
        owner.reconcile(&scope, "read", &[]).unwrap().bytes,
        b"After"
    );
    assert!(!owner.reconciliation_required().unwrap());
    assert_eq!(
        owner
            .effect(effect(&scope, &after, &expected), &mut Approve)
            .unwrap()
            .bytes,
        b"After"
    );
    assert!(!owner.reconciliation_required().unwrap());
    assert_eq!(server.finish().len(), 7);
}
#[test]
fn account_child() {
    let Ok(root) = std::env::var("E07_CHILD_ROOT") else {
        return;
    };
    let owner = Rc::new(OwnerRoot::open(std::path::Path::new(&root)).unwrap());
    let _lease = owner.account(&hash(b"synthetic-process-account")).unwrap();
    println!("E07_LOCKED");
    std::io::stdout().flush().unwrap();
    thread::sleep(Duration::from_secs(10));
}
#[test]
fn account_excludes_processes_and_releases_on_process_termination() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let key = hash(b"synthetic-process-account");
    let first = root.account(&key).unwrap();
    assert_eq!(
        root.account(&key).err().unwrap().message,
        "owner_account_busy"
    );
    drop(first);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "account_child", "--nocapture"])
        .env_clear()
        .env("E07_CHILD_ROOT", &fixture.0)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let output = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if line.unwrap() == "E07_LOCKED" {
                tx.send(()).unwrap();
                break;
            }
        }
    });
    let ready = rx.recv_timeout(Duration::from_secs(3));
    if ready.is_err() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("synthetic owner child did not acquire lock");
    }
    assert_eq!(
        root.account(&key).err().unwrap().message,
        "owner_account_busy"
    );
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    reader.join().unwrap();
    let recovered = root.account(&key).unwrap();
    assert!(!recovered.reconciliation_required().unwrap());
}
fn request() -> Request {
    Request {
        method: "GET".into(),
        path: "/c_login.jsp".into(),
        query: vec![],
        body: vec![],
        headers: vec![],
        timeout: Duration::from_millis(100),
    }
}
#[test]
fn native_transport_bounds_streamed_readback_stall_headers_requests_and_cancellation() {
    let mut large = reply("/c_login.jsp", &"x".repeat(5000));
    // Content-Length refuses before allocation of the advertised document.
    let server = Server::new(vec![large]);
    let mut transport = NativeTransport::synthetic_loopback(
        server.address,
        TransportLimits {
            response_bytes: 4096,
            ..TransportLimits::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        transport.dispatch(&request()).err().unwrap().message,
        "source_response_too_large"
    );
    assert_eq!(server.finish().len(), 1);
    let mut streamed = reply("/c_login.jsp", &"x".repeat(5000));
    streamed.unframed = true;
    let server = Server::new(vec![streamed]);
    let mut transport = NativeTransport::synthetic_loopback(
        server.address,
        TransportLimits {
            response_bytes: 4096,
            ..TransportLimits::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert!(transport.dispatch(&request()).is_err());
    assert_eq!(server.finish().len(), 1);
    large = reply("/c_login.jsp", "x");
    large.headers.push(("X-Long".into(), "x".repeat(17000)));
    let server = Server::new(vec![large]);
    assert!(server.transport().dispatch(&request()).is_err());
    assert_eq!(server.finish().len(), 1);
    let mut stalled = reply("/c_login.jsp", "eventually");
    stalled.delay = Duration::from_millis(300);
    let server = Server::new(vec![stalled]);
    let start = Instant::now();
    assert!(server.transport().dispatch(&request()).is_err());
    assert!(start.elapsed() < Duration::from_millis(250));
    assert_eq!(server.finish().len(), 1);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let cancel = Cancellation::default();
    let mut transport = NativeTransport::synthetic_loopback(
        listener.local_addr().unwrap(),
        TransportLimits {
            request_bytes: 512,
            ..TransportLimits::default()
        },
        cancel.clone(),
    )
    .unwrap();
    let mut oversized = request();
    oversized.query.push(("argument".into(), "x".repeat(1000)));
    assert!(transport.dispatch(&oversized).is_err());
    let mut foreign = request();
    foreign.path = "//foreign.invalid/".into();
    assert!(transport.dispatch(&foreign).is_err());
    foreign.path = "/../unexpected".into();
    assert!(transport.dispatch(&foreign).is_err());
    cancel.cancel();
    assert_eq!(
        transport.dispatch(&request()).err().unwrap().message,
        "native_transport_cancelled"
    );
    assert!(listener.accept().is_err());
    assert!(
        NativeTransport::synthetic_loopback(
            "192.0.2.1:80".parse().unwrap(),
            TransportLimits::default(),
            Cancellation::default()
        )
        .is_err()
    );
}
#[test]
fn installed_owner_accepts_only_pinned_explicit_inputs_without_network() {
    let fixture = Fixture::new();
    let clock = Clock(Cell::new(1000));
    let admission = fixture.pinned("admission", &serde_json::to_vec(&policy()).unwrap());
    let credential = fixture.pinned(
        "credential",
        &serde_json::to_vec(
            &serde_json::json!({"schemaVersion":"academy.owner-credential/1",
        "username":random(),"password":random(),"expiresAtMs":10000}),
        )
        .unwrap(),
    );
    let inputs = NativeOwnerInputs {
        root: fixture.0.clone(),
        admission,
        credential,
    };
    // Construction validates file pins and obtains the lock; it never logs in.
    let owner = NativeSession::installed(
        inputs,
        TransportLimits::default(),
        Cancellation::default(),
        &clock,
    )
    .unwrap();
    assert!(!owner.reconciliation_required().unwrap());
}
#[test]
fn malformed_policy_unknown_namespaces_duplicate_selectors_and_paths_refuse() {
    let original = serde_json::to_value(policy()).unwrap();
    for case in 0..6 {
        let mut value = original.clone();
        match case {
            0 => value["operations"][0]["sourceNamespace"] = "academy.foreign".into(),
            1 => value["operations"][0]["path"] = "//foreign.invalid/".into(),
            2 => {
                let duplicate = value["operations"][0]["query"][0].clone();
                value["operations"][0]["query"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
            }
            3 => value["allowedThrough"] = "2039-12-31".into(),
            4 => {
                value["operations"][0]["body"] = serde_json::json!([{"name":"reqCmd","value":{"kind":"literal","value":"write"}}])
            }
            _ => value["unexpected"] = "not owner admitted".into(),
        }
        assert!(TransportAdmission::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
struct Stalled;
impl Transport for Stalled {
    fn dispatch(&mut self, _: &Request) -> Result<Response> {
        Ok(Response {
            status: 200,
            headers: vec![],
            bytes: vec![],
            url: String::new(),
            stalled: true,
        })
    }
}
#[test]
fn recorded_stalled_login_fails_instead_of_parking_forever() {
    let mut client = edu_academy_sources::http::Client::new(
        obj(&[
            ("teacherLabel", "Fictional Teacher".into()),
            ("teacherKey", "99001".into()),
            ("collegeKey", "88001".into()),
        ]),
        Stalled,
    )
    .unwrap();
    let start = Instant::now();
    assert_eq!(
        client
            .authenticate(&random(), &random())
            .err()
            .unwrap()
            .message,
        "source_response_timeout"
    );
    assert!(start.elapsed() < Duration::from_millis(100));
}
/// The resolver port models libc remaining blocked after the ureq caller's
/// timeout. No DNS/network runs; the retained worker owns only a release flag.
#[derive(Debug)]
struct TimedResolver {
    release: Arc<std::sync::atomic::AtomicBool>,
    worker: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}
impl ureq::unversioned::resolver::Resolver for TimedResolver {
    fn resolve(
        &self,
        _: &ureq::http::Uri,
        _: &ureq::config::Config,
        timeout: ureq::unversioned::transport::NextTimeout,
    ) -> std::result::Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let release = Arc::clone(&self.release);
        let (tx, rx) = std::sync::mpsc::sync_channel::<()>(1);
        *self.worker.lock().unwrap() = Some(thread::spawn(move || {
            while !release.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(2));
            }
            let _ = tx.send(());
        }));
        assert!(rx.recv_timeout(*timeout.after).is_err());
        Err(ureq::Error::Timeout(timeout.reason))
    }
}
#[test]
fn resolve_timeout_returns_timeout_and_dropped_owner_releases_lock_before_resolver_finishes() {
    let fixture = Fixture::new();
    let clock = Clock(Cell::new(1000));
    let username = random();
    let password = random();
    let account = hash(format!("{}\0{}", edu_academy_sources::http::ORIGIN, username).as_bytes());
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker = Arc::new(Mutex::new(None));
    let transport = NativeTransport::with_resolver(
        TransportLimits {
            timeout: Duration::from_millis(60),
            ..TransportLimits::default()
        },
        Cancellation::default(),
        TimedResolver {
            release: Arc::clone(&release),
            worker: Arc::clone(&worker),
        },
    )
    .unwrap();
    let root = fixture.root();
    let mut owner = NativeSession::open(
        Rc::clone(&root),
        policy(),
        transport,
        || Ok(Credentials::new(username.clone(), password.clone())),
        Cancellation::default(),
        &clock,
    )
    .unwrap();
    let start = Instant::now();
    let error = owner.read(&scope(), "read", &[]).err().unwrap();
    assert_eq!(error.message, "native_transport_timeout");
    assert!(start.elapsed() < Duration::from_millis(250));
    assert!(!worker.lock().unwrap().as_ref().unwrap().is_finished());
    drop(owner);
    let recovered = root.account(&account).unwrap();
    assert!(!recovered.reconciliation_required().unwrap());
    release.store(true, Ordering::Release);
    worker.lock().unwrap().take().unwrap().join().unwrap();
}
#[test]
fn effect_child() {
    let Ok(path) = std::env::var("E07_EFFECT_ROOT") else {
        return;
    };
    let root = Rc::new(OwnerRoot::open(std::path::Path::new(&path)).unwrap());
    let clock = Clock(Cell::new(1000));
    let provider = FileCredentialProvider::new(
        Rc::clone(&root),
        PinnedInput {
            relative_path: "credential".into(),
            sha256: std::env::var("E07_CREDENTIAL_PIN").unwrap(),
        },
        &clock,
    );
    let transport = NativeTransport::synthetic_loopback(
        std::env::var("E07_SERVER").unwrap().parse().unwrap(),
        TransportLimits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut owner = NativeSession::open(
        root,
        policy(),
        transport,
        provider,
        Cancellation::default(),
        &clock,
    )
    .unwrap();
    let _ = owner.effect(
        effect(
            &scope(),
            &[("after".into(), "After".into())],
            &hash(b"After"),
        ),
        &mut Approve,
    );
}
#[test]
fn killed_inflight_effect_keeps_durable_fence_and_releases_socket_and_account() {
    let fixture = Fixture::new();
    let clock = Clock(Cell::new(1000));
    let input = fixture.pinned(
        "credential",
        &serde_json::to_vec(
            &serde_json::json!({"schemaVersion":"academy.owner-credential/1",
        "username":random(),"password":random(),"expiresAtMs":100_000}),
        )
        .unwrap(),
    );
    let mut replies = login();
    let mut write = reply("/synthetic/write", "accepted");
    write.delay = Duration::from_millis(300);
    write.expect_peer_closed = true;
    replies.push(write);
    let server = Server::new(replies);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "effect_child", "--nocapture"])
        .env_clear()
        .env("E07_EFFECT_ROOT", &fixture.0)
        .env("E07_CREDENTIAL_PIN", &input.sha256)
        .env("E07_SERVER", server.address.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while server.seen.lock().unwrap().len() < 4 {
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("synthetic effect child deadline");
        }
        thread::sleep(Duration::from_millis(2));
    }
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    assert_eq!(server.finish().len(), 4);
    let server = Server::new({
        let mut r = login();
        r.push(reply("/synthetic/read", "After"));
        r
    });
    let mut owner = NativeSession::open(
        fixture.root(),
        policy(),
        server.transport(),
        FileCredentialProvider::new(fixture.root(), input, &clock),
        Cancellation::default(),
        &clock,
    )
    .unwrap();
    assert!(owner.reconciliation_required().unwrap());
    assert!(
        owner
            .effect(
                effect(
                    &scope(),
                    &[("after".into(), "After".into())],
                    &hash(b"After")
                ),
                &mut Approve
            )
            .is_err()
    );
    assert!(server.seen.lock().unwrap().is_empty());
    owner.reconcile(&scope(), "read", &[]).unwrap();
    assert!(!owner.reconciliation_required().unwrap());
    assert_eq!(server.finish().len(), 4);
}
#[test]
fn account_lock_inode_is_retained_and_replacement_links_or_corrupt_markers_refuse() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let key = hash(b"synthetic-account-file");
    let name = format!("account-{key}.lock");
    let lease = root.account(&key).unwrap();
    assert!(fixture.0.join(&name).is_file());
    drop(lease);
    assert!(fixture.0.join(&name).is_file());
    fs::hard_link(fixture.0.join(&name), fixture.0.join("alias")).unwrap();
    assert!(root.account(&key).is_err());
    fs::remove_file(fixture.0.join("alias")).unwrap();
    let lease = root.account(&key).unwrap();
    fs::rename(fixture.0.join(&name), fixture.0.join("old-inode")).unwrap();
    fixture.pinned(&name, b"0\n");
    assert!(lease.reconciliation_required().is_err());
    drop(lease);
    fs::remove_file(fixture.0.join(&name)).unwrap();
    symlink(fixture.0.join("old-inode"), fixture.0.join(&name)).unwrap();
    assert!(root.account(&key).is_err());
    fs::remove_file(fixture.0.join(&name)).unwrap();
    fixture.pinned(&name, b"{truncated");
    assert_eq!(
        root.account(&key).err().unwrap().message,
        "owner_account_reconciliation_required"
    );
}
#[test]
fn review_unquoted_password_and_empty_login_form_challenges_close_the_session() {
    for challenge in [
        "<input type=password>",
        "<input TYPE = PaSsWoRd>",
        "<form id=loginForm></form>",
    ] {
        let fixture = Fixture::new();
        let clock = Clock(Cell::new(1000));
        let mut replies = login();
        replies.push(reply("/synthetic/read", challenge));
        let server = Server::new(replies);
        let mut owner = session(&fixture, &clock, &server);
        assert_eq!(
            owner
                .read(&scope(), "read", &[])
                .err()
                .expect("login challenge must be refused")
                .message,
            "authentication_required"
        );
        assert_eq!(
            owner.read(&scope(), "read", &[]).err().unwrap().message,
            "native_owner_session_expired"
        );
        assert_eq!(server.finish().len(), 4);
    }
}
struct ExpireOnAuthentication<'a> {
    transport: NativeTransport,
    clock: &'a Clock,
    calls: Rc<Cell<usize>>,
}
impl Transport for ExpireOnAuthentication<'_> {
    fn dispatch(&mut self, request: &Request) -> Result<Response> {
        self.calls.set(self.calls.get() + 1);
        let response = self.transport.dispatch(request)?;
        if request.path == "/servlet/controller.coursemanage.CourseManageServlet" {
            self.clock.0.set(100_000);
        }
        Ok(response)
    }
}
#[test]
fn review_credential_expiry_during_login_refuses_before_source_dispatch() {
    let fixture = Fixture::new();
    let clock = Clock(Cell::new(1000));
    let server = Server::new(login());
    let calls = Rc::new(Cell::new(0));
    let transport = ExpireOnAuthentication {
        transport: server.transport(),
        clock: &clock,
        calls: Rc::clone(&calls),
    };
    let mut owner = NativeSession::open(
        fixture.root(),
        policy(),
        transport,
        fixture.credential(&clock, 100_000),
        Cancellation::default(),
        &clock,
    )
    .unwrap();
    assert_eq!(
        owner.read(&scope(), "read", &[]).err().unwrap().message,
        "native_owner_credential_expired"
    );
    assert_eq!(calls.get(), 3, "no source dispatch after credential expiry");
    assert_eq!(
        owner.read(&scope(), "read", &[]).err().unwrap().message,
        "native_owner_credential_expired"
    );
    assert_eq!(server.finish().len(), 3);
}
