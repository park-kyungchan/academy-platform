use edu_academy_teacher::{
    record_data::{OwnerScope, RecordIdentity, RecordKind},
    record_store::*,
};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const DIRS: [&str; 10] = [
    "locks",
    "record-drafts",
    "observation",
    "review",
    "request",
    "execution",
    "outcome",
    "reconciliation",
    "ledger",
    "audit",
];
fn mode(path: impl AsRef<Path>) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "record-store-a2a-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        DirBuilder::new().mode(0o700).create(&p).unwrap();
        Self(p)
    }
    fn root(&self) -> PathBuf {
        self.0.join("root")
    }
    fn open(&self) -> DraftStore {
        DraftStore::open(&self.root(), owner()).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn owner() -> OwnerScope {
    OwnerScope {
        owner: "synthetic-owner".into(),
        account: "synthetic-account".into(),
    }
}
fn identity() -> RecordIdentity {
    RecordIdentity {
        owner: owner(),
        student_key: "1001".into(),
        lesson_date: "2026-11-11".into(),
        kind: RecordKind::DailyRecord,
    }
}
fn file(path: &Path) -> File {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap()
}
fn put(path: &Path, bytes: &[u8]) {
    file(path).write_all(bytes).unwrap();
}
fn rel(s: &str) -> &Path {
    Path::new(s)
}
fn refused<T>(r: edu_academy_core::js::Result<T>) {
    assert_eq!(error(r), "record_store_not_synthetic");
}
fn error<T>(r: edu_academy_core::js::Result<T>) -> String {
    r.err().expect("expected refusal").message
}
#[test]
fn bootstrap_owner_isolation_and_permissions() {
    let t = Scratch::new();
    let s = t.open();
    assert_eq!(s.root(), t.root());
    for d in std::iter::once("").chain(DIRS) {
        assert_eq!(mode(t.root().join(d)), 0o700);
    }
    assert_eq!(mode(t.root().join(MARKER)), 0o600);
    let before = fs::read(t.root().join(MARKER)).unwrap();
    let mut other = owner();
    other.account.push('x');
    refused(DraftStore::open(&t.root(), other.clone()));
    assert_eq!(fs::read(t.root().join(MARKER)).unwrap(), before);
    DraftStore::open(&t.0.join("other"), other).unwrap();
    let a = s.lock_account().unwrap();
    let mut wrong = identity();
    wrong.owner.account.push('x');
    refused(a.lock_draft(&wrong));
    let empty = t.0.join("empty");
    fs::create_dir(&empty).unwrap();
    fs::set_permissions(&empty, fs::Permissions::from_mode(0o777)).unwrap();
    refused(DraftStore::open(&empty, owner()));
    fs::set_permissions(&empty, fs::Permissions::from_mode(0o755)).unwrap();
    DraftStore::open(&empty, owner()).unwrap();
}
#[test]
fn unmarked_corrupt_missing_and_oversized_markers_never_adopt_a_root() {
    for d in DIRS {
        let t = Scratch::new();
        fs::create_dir(t.root()).unwrap();
        fs::create_dir(t.root().join(d)).unwrap();
        refused(DraftStore::open(&t.root(), owner()));
        assert!(!t.root().join(MARKER).exists());
    }
    for bytes in [b"not JSON".to_vec(), vec![b'x'; MAX_FILE_BYTES + 1]] {
        let t = Scratch::new();
        let s = t.open();
        fs::write(t.root().join(MARKER), &bytes).unwrap();
        refused(DraftStore::open(&t.root(), owner()));
        assert!(s.lock_account().is_err());
        assert_eq!(fs::read(t.root().join(MARKER)).unwrap(), bytes);
    }
    let t = Scratch::new();
    let s = t.open();
    let path = t.root().join(MARKER);
    let mut marker: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    marker["format"] = "another-format".into();
    fs::write(&path, serde_json::to_vec(&marker).unwrap()).unwrap();
    refused(DraftStore::open(&t.root(), owner()));
    refused(s.lock_account());
    let t = Scratch::new();
    let s = t.open();
    fs::remove_file(t.root().join(MARKER)).unwrap();
    assert!(s.lock_account().is_err());
    assert!(DraftStore::open(&t.root(), owner()).is_err());
}
#[test]
fn symbolic_links_at_root_ancestors_marker_and_store_directories_are_refused() {
    let t = Scratch::new();
    t.open();
    symlink(t.root(), t.0.join("alias")).unwrap();
    assert!(DraftStore::open(&t.0.join("alias"), owner()).is_err());
    assert!(DraftStore::open(&t.0.join("alias/new"), owner()).is_err());
    assert!(!t.root().join("new").exists());
    for d in std::iter::once(MARKER).chain(DIRS) {
        let t = Scratch::new();
        t.open();
        let p = t.root().join(d);
        let original = t.0.join("original");
        fs::rename(&p, &original).unwrap();
        symlink(&original, &p).unwrap();
        refused(DraftStore::open(&t.root(), owner()));
    }
}
#[test]
fn guarded_reads_writes_locks_and_temp_paths_do_not_follow_links() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let d = a.lock_draft(&identity()).unwrap();
    let outside = t.0.join("outside");
    put(&outside, b"unchanged");
    let target = rel("record-drafts/value.json");
    symlink(&outside, t.root().join(target)).unwrap();
    assert!(a.read_bytes(target).is_err());
    assert!(d.write_atomic(target, b"changed").is_err());
    assert!(a.usage(100).is_err());
    fs::remove_file(t.root().join(target)).unwrap();
    let tmp = t.root().join(format!(
        "record-drafts/.record-write-{}.tmp",
        std::process::id()
    ));
    symlink(&outside, &tmp).unwrap();
    assert!(d.write_atomic(target, b"changed").is_err());
    fs::remove_file(tmp).unwrap();
    fs::rename(t.root().join("record-drafts"), t.0.join("saved-dir")).unwrap();
    symlink(&t.0, t.root().join("record-drafts")).unwrap();
    assert!(a.read_bytes(target).is_err());
    assert!(d.write_atomic(target, b"changed").is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"unchanged");
    drop(d);
    drop(a);
    fs::remove_file(t.root().join("locks/account.flock")).unwrap();
    symlink(&outside, t.root().join("locks/account.flock")).unwrap();
    assert!(s.lock_account().is_err());
}
#[test]
fn draft_lock_links_permissions_hardlinks_and_relocated_roots_are_refused() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let lock = t
        .root()
        .join(format!("locks/{}.flock", identity().key().unwrap()));
    symlink(t.root().join(MARKER), lock).unwrap();
    assert!(a.lock_draft(&identity()).is_err());
    let target = t.root().join("record-drafts/data");
    put(&target, b"private");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(a.read_bytes(rel("record-drafts/data")).is_err());
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(&target, t.0.join("hardlink")).unwrap();
    assert!(a.read_bytes(rel("record-drafts/data")).is_err());
    fs::rename(t.root(), t.0.join("old-root")).unwrap();
    t.open();
    assert!(a.read_bytes(rel("record-drafts/data")).is_err());
}
#[test]
fn file_size_and_atomic_replacement_boundaries() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let d = a.lock_draft(&identity()).unwrap();
    let p = rel("record-drafts/data");
    let mut bytes = vec![b' '; MAX_FILE_BYTES];
    bytes[..4].copy_from_slice(b"null");
    d.write_atomic(p, &bytes).unwrap();
    assert_eq!(a.read_bytes(p).unwrap().unwrap(), bytes);
    assert!(d.write_atomic(p, &vec![b'x'; MAX_FILE_BYTES + 1]).is_err());
    assert_eq!(a.read_bytes(p).unwrap().unwrap(), bytes);
    d.write_atomic(p, b"first").unwrap();
    assert_eq!(a.read_bytes(p).unwrap().unwrap(), b"first");
    d.write_atomic(p, b"replacement of existing file").unwrap();
    assert_eq!(
        a.read_bytes(p).unwrap().unwrap(),
        b"replacement of existing file"
    );
    assert_eq!(mode(t.root().join(p)), 0o600);
    assert_eq!(
        fs::read_dir(t.root().join("record-drafts"))
            .unwrap()
            .count(),
        1
    );
    assert!(d.write_atomic(p, &vec![b'x'; MAX_FILE_BYTES + 1]).is_err());
    assert_eq!(
        a.read_bytes(p).unwrap().unwrap(),
        b"replacement of existing file"
    );
    fs::write(t.root().join(p), vec![b'x'; MAX_FILE_BYTES + 1]).unwrap();
    assert!(a.read_bytes(p).is_err());
    fs::remove_file(t.root().join(p)).unwrap();
    d.write_atomic(p, b"replaced").unwrap();
    assert_eq!(a.read_bytes(p).unwrap().unwrap(), b"replaced");
    assert!(d.write_atomic(rel(MARKER), b"bad").is_err());
    assert!(d.write_atomic(rel("../escape"), b"bad").is_err());
    assert!(a.read_bytes(rel("/outside")).is_err());
    assert_eq!(mode(t.root().join(p)), 0o600);
    let missing = a.read_bytes(rel("record-drafts/missing")).unwrap();
    assert!(missing.is_none());
}
#[test]
fn aggregate_bytes_and_entry_budget_boundaries() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let initial = a.usage(100).unwrap();
    let f = file(&t.root().join("record-drafts/sparse"));
    f.set_len(MAX_ROOT_BYTES - initial.bytes).unwrap();
    let usage = a.usage(100).unwrap();
    assert_eq!(usage.bytes, MAX_ROOT_BYTES);
    assert!(a.usage(usage.entries).is_ok());
    assert!(a.usage(usage.entries - 1).is_err());
    f.set_len(MAX_ROOT_BYTES - initial.bytes + 1).unwrap();
    assert_eq!(error(a.usage(100)), "record_store_capacity");
}
fn finish(child: &mut Child) {
    for _ in 0..600 {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    panic!("synthetic child deadline");
}
#[test]
fn child_lock_worker() {
    let Some(root) = std::env::var_os("A2A_CHILD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let s = DraftStore::open(&root, owner()).unwrap();
    put(&root.parent().unwrap().join("ready"), b"ready");
    if std::env::var_os("A2A_TIMEOUT").is_some() {
        assert_eq!(error(s.lock_account()), "shared_writer_lock_busy");
        return;
    }
    let a = s.lock_account().unwrap();
    if std::env::var_os("A2A_DRAFT_TIMEOUT").is_some() {
        assert_eq!(error(a.lock_draft(&identity())), "shared_writer_lock_busy");
        return;
    }
    let d = a.lock_draft(&identity()).unwrap();
    d.write_atomic(rel("record-drafts/order"), b"child")
        .unwrap();
}
#[test]
fn two_process_account_before_draft_and_lock_timeout() {
    for timeout in [false, true] {
        let t = Scratch::new();
        let s = t.open();
        let a = s.lock_account().unwrap();
        {
            let d = a.lock_draft(&identity()).unwrap();
            d.write_atomic(rel("record-drafts/order"), b"parent")
                .unwrap();
        }
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args(["--exact", "child_lock_worker"])
            .env("A2A_CHILD_ROOT", t.root())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if timeout {
            cmd.env("A2A_TIMEOUT", "1");
        }
        let start = Instant::now();
        let mut child = cmd.spawn().unwrap();
        while !t.0.join("ready").exists() {
            if start.elapsed() > Duration::from_secs(3) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("child not ready");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let raw = File::open(
            t.root()
                .join(format!("locks/{}.flock", identity().key().unwrap())),
        )
        .unwrap();
        raw.try_lock().unwrap();
        raw.unlock().unwrap();
        if timeout {
            finish(&mut child);
            assert!(start.elapsed() >= Duration::from_millis(LOCK_WAIT_MS));
            drop(a);
        } else {
            drop(a);
            finish(&mut child);
            let a = s.lock_account().unwrap();
            assert_eq!(
                a.read_bytes(rel("record-drafts/order")).unwrap().unwrap(),
                b"child"
            );
        }
    }
}
#[test]
fn second_process_with_account_lock_cannot_take_held_draft_lock() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    drop(a.lock_draft(&identity()).unwrap());
    let raw = File::open(
        t.root()
            .join(format!("locks/{}.flock", identity().key().unwrap())),
    )
    .unwrap();
    raw.try_lock().unwrap();
    drop(a); // The next account holder must still contend on the draft inode.
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_lock_worker"])
        .env("A2A_CHILD_ROOT", t.root())
        .env("A2A_DRAFT_TIMEOUT", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    finish(&mut child);
    raw.unlock().unwrap();
}
