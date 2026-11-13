//! Cross-runtime exclusion uses disposable files only; no production socket or path.
use edu_academy_core::{js::Result as CoreResult, workflow::Runtime};
use edu_academy_sources::{
    data::*,
    http::{Cancellation, Request, Response, Transport},
    owner_admission::TransportAdmission,
    owner_root::OwnerRoot,
    owner_workflow::{Credentials, NativeSession},
};
use std::{
    cell::Cell,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    process::{Child, Command},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(std::env::var("S09_NATIVE_FIXTURE_ROOT").unwrap()).join(format!(
            "shared-lock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let fixture = Self(path);
        fs::write(fixture.lock(), b"synthetic retained lock bytes\n").unwrap();
        fixture.configure();
        fixture
    }
    fn lock(&self) -> PathBuf {
        self.0.join("shared.lock")
    }
    fn configure(&self) {
        let config = self.0.join("account-lock.json");
        fs::write(
            &config,
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion":"academy.account-lock/1", "path": self.lock()
            }))
            .unwrap(),
        )
        .unwrap();
        fs::set_permissions(config, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn root(&self) -> Rc<OwnerRoot> {
        Rc::new(OwnerRoot::open(&self.0).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().unwrap();
    }
}
fn probe(path: &std::path::Path) -> i32 {
    Command::new("/usr/bin/flock")
        .arg("--nonblock")
        .arg(path)
        .arg("true")
        .status()
        .unwrap()
        .code()
        .unwrap()
}
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        "2040-01-04T09:00:00.000Z".into()
    }
    fn uuid(&self) -> CoreResult<String> {
        Ok("synthetic-lock".into())
    }
    fn epoch_ms(&self) -> u64 {
        1000
    }
}
struct Counter(Rc<Cell<usize>>);
impl Transport for Counter {
    fn dispatch(&mut self, _: &Request) -> Result<Response> {
        self.0.set(self.0.get() + 1);
        Err(Error::error("synthetic_request_refused"))
    }
}
fn policy() -> TransportAdmission {
    TransportAdmission::from_bytes(
        &serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"academy.transport-admission/1", "ownerId":"synthetic-owner",
            "teacherId":"99001", "teacherLabel":"Fictional Teacher", "collegeId":"88001",
            "allowedFrom":"2040-01-01", "allowedThrough":"2040-01-31", "studentIds":["77001"],
            "sourceNamespaces":["academy.lms.metadata"],
            "operations":[{"id":"read","sourceNamespace":"academy.lms.metadata","method":"GET",
                "path":"/synthetic/read","effect":false,"query":[],"body":[]}]
        }))
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn a1_external_flock_refuses_before_transport_and_owner_state() {
    let fixture = Fixture::new();
    let mut child = ChildGuard(
        Command::new("/usr/bin/flock")
            .arg("--no-fork")
            .arg(fixture.lock())
            .args(["sleep", "30"])
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while probe(&fixture.lock()) != 1 {
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < deadline, "child lock readiness deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
    let before = fs::read_dir(&fixture.0).unwrap().count();
    let calls = Rc::new(Cell::new(0));
    let result = NativeSession::open(
        fixture.root(),
        policy(),
        Counter(calls.clone()),
        || {
            Ok(Credentials::new(
                "synthetic-user".into(),
                "synthetic-password".into(),
            ))
        },
        Cancellation::default(),
        &Clock,
    );
    assert_eq!(
        result
            .err()
            .expect("external holder must refuse lease")
            .message,
        "owner_account_busy"
    );
    assert_eq!(calls.get(), 0, "busy admission must not dispatch transport");
    assert_eq!(
        fs::read_dir(&fixture.0).unwrap().count(),
        before,
        "busy admission leaves no owner marker"
    );
}
#[test]
fn a2_lease_excludes_flock_and_preserves_shared_file_and_missing_path() {
    let fixture = Fixture::new();
    let before = fs::metadata(fixture.lock()).unwrap();
    let root = fixture.root();
    let lease = root.account(&hash(b"synthetic-account")).unwrap();
    assert_eq!(probe(&fixture.lock()), 1, "Rust lease excludes launcher");
    drop(lease);
    assert_eq!(probe(&fixture.lock()), 0, "drop releases launcher lock");
    let after = fs::metadata(fixture.lock()).unwrap();
    assert_eq!(
        (
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec()
        ),
        (after.ino(), after.len(), after.mtime(), after.mtime_nsec())
    );
    fs::remove_file(fixture.lock()).unwrap();
    assert!(
        root.account(&hash(b"synthetic-account")).is_err(),
        "missing shared file fails closed"
    );
    assert!(!fixture.lock().exists());
}

#[test]
fn missing_configuration_nonregular_and_symlink_shared_paths_fail_closed() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let key = hash(b"synthetic-refusal");
    fs::remove_file(fixture.0.join("account-lock.json")).unwrap();
    assert_eq!(
        root.account(&key).err().unwrap().message,
        "owner_shared_account_configuration_required"
    );
    fixture.configure();
    fs::remove_file(fixture.lock()).unwrap();
    fs::create_dir(fixture.lock()).unwrap();
    assert!(root.account(&key).is_err());
    fs::remove_dir(fixture.lock()).unwrap();
    let other = fixture.0.join("other.lock");
    fs::write(&other, b"retained").unwrap();
    std::os::unix::fs::symlink(&other, fixture.lock()).unwrap();
    assert!(root.account(&key).is_err());
    fs::remove_file(fixture.lock()).unwrap();
    assert!(!fixture.0.join(format!("account-{key}.lock")).exists());
}

#[test]
fn production_transport_refuses_without_a_lease_before_resolver() {
    use edu_academy_sources::http::{NativeTransport, TransportLimits};
    #[derive(Debug)]
    struct NoResolve;
    impl ureq::unversioned::resolver::Resolver for NoResolve {
        fn resolve(
            &self,
            _: &ureq::http::Uri,
            _: &ureq::config::Config,
            _: ureq::unversioned::transport::NextTimeout,
        ) -> std::result::Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error>
        {
            panic!("unleased transport must never resolve or connect");
        }
    }
    let mut transport = NativeTransport::with_resolver(
        TransportLimits::default(),
        Cancellation::default(),
        NoResolve,
    )
    .unwrap();
    let request = Request {
        method: "GET".into(),
        path: "/c_login.jsp".into(),
        query: vec![],
        body: vec![],
        headers: vec![],
        timeout: Duration::from_millis(10),
    };
    assert_eq!(
        transport.dispatch(&request).err().unwrap().message,
        "owner_account_lease_required"
    );
}
