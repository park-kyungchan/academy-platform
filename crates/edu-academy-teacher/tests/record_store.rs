use edu_academy_core::{
    js::{Js, sha},
    workflow::{codec, state, store::Store},
};
use edu_academy_teacher::{
    record_data::{OwnerScope, RecordIdentity, RecordKind},
    record_store::*,
};
use serde_json::{Value, json};
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
fn scope() -> Js {
    codec::parse(
        &serde_json::to_vec(&json!({
            "sourceNamespace": "synthetic-record",
            "teacherId": "synthetic-account", "studentId": "1001",
            "lessonDate": "2026-11-11"
        }))
        .unwrap(),
    )
    .unwrap()
}
fn history_dir(t: &Scratch, scope: &Js) -> PathBuf {
    t.root()
        .join("ledger")
        .join(sha(&state::lock_key(scope).unwrap()))
}
fn core_path(t: &Scratch, kind: &str, key: &str) -> PathBuf {
    t.root()
        .join(kind)
        .join(sha(&format!("{kind}\n{key}")) + ".json")
}
fn seed_execution(t: &Scratch, scope: &Js, index: usize, terminal: bool) -> String {
    let key = format!("synthetic-execution-{index}");
    let dir = history_dir(t, scope);
    if !dir.exists() {
        DirBuilder::new().mode(0o700).create(&dir).unwrap();
    }
    put(
        &dir.join(sha(&key) + ".json"),
        &serde_json::to_vec(&json!({
            "group": state::lock_key(scope).unwrap(), "key": key
        }))
        .unwrap(),
    );
    let scope_json: Value = serde_json::from_slice(&codec::bytes(scope).unwrap()).unwrap();
    for (kind, record) in [
        (
            "execution",
            json!({"scope": scope_json, "field": "progress",
            "requestRef": "synthetic-request", "effectRecordKey": "synthetic:1001:2026-11-11",
            "beforeSha256": sha("before"), "afterSha256": sha("after")}),
        ),
        ("outcome", json!({"status": "noop"})),
    ] {
        if kind == "outcome" && !terminal {
            continue;
        }
        put(
            &core_path(t, kind, &key),
            &serde_json::to_vec(&json!({
                "kind": kind, "id": key, "record": record
            }))
            .unwrap(),
        );
    }
    key
}
#[test]
fn execution_record_4096_boundary() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let scope = scope();
    let save = |selected_fields| ExecutionAdmission::Save { selected_fields };
    for i in 0..MAX_EXECUTION_RECORDS - 1 {
        seed_execution(&t, &scope, i, true);
    }
    assert_eq!(
        a.preflight_executions(&scope, save(1)).unwrap(),
        MAX_EXECUTION_RECORDS - 1
    );
    assert_eq!(
        error(a.preflight_executions(&scope, save(2))),
        "record_execution_limit"
    );
    // The admitted caller can append one fence under this same account guard.
    seed_execution(&t, &scope, MAX_EXECUTION_RECORDS - 1, true);
    assert_eq!(
        error(a.preflight_executions(&scope, save(1))),
        "record_execution_limit"
    );
    assert_eq!(
        a.preflight_executions(&scope, ExecutionAdmission::ReadOnly)
            .unwrap(),
        MAX_EXECUTION_RECORDS
    );
    // Exercise the actual core's full read-only state scan only after admission.
    let core = Store::open(&t.root(), &t.root()).unwrap();
    let history = state::student_executions(&core, &scope).unwrap();
    assert_eq!(history.len(), MAX_EXECUTION_RECORDS);
    assert!(history.iter().all(|e| !e.state.needs_reconciliation()));
    // An extra orphan counts, and its oversized bytes must not be parsed at preflight.
    let orphan = history_dir(&t, &scope).join(sha("synthetic-orphan") + ".json");
    let f = file(&orphan);
    f.set_len(MAX_FILE_BYTES as u64 + 1).unwrap();
    for mode in [ExecutionAdmission::ReadOnly, save(1)] {
        assert_eq!(
            error(a.preflight_executions(&scope, mode)),
            "record_execution_limit"
        );
    }
    assert_eq!(
        fs::metadata(orphan).unwrap().len(),
        MAX_FILE_BYTES as u64 + 1
    );
}
#[test]
fn execution_preflight_validates_scope_fields_and_orphan_members() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let scope = scope();
    let save = |selected_fields| ExecutionAdmission::Save { selected_fields };
    assert_eq!(a.preflight_executions(&scope, save(6)).unwrap(), 0);
    for n in [0, 7, usize::MAX] {
        assert_eq!(
            error(a.preflight_executions(&scope, save(n))),
            "record_execution_fields_invalid"
        );
    }
    for field in ["sourceNamespace", "teacherId", "studentId"] {
        for value in [Js::Null, "".into()] {
            let mut invalid = scope.clone();
            invalid.set(field, value);
            assert_eq!(
                error(a.preflight_executions(&invalid, save(1))),
                "record_execution_scope_invalid"
            );
        }
    }
    assert!(!history_dir(&t, &scope).exists()); // Missing history is not initialized by a read.
    let key = seed_execution(&t, &scope, 0, false);
    fs::remove_file(core_path(&t, "execution", &key)).unwrap();
    assert_eq!(a.preflight_executions(&scope, save(6)).unwrap(), 1);
    let mut other_student = scope.clone();
    other_student.set("studentId", "1003".into());
    assert_eq!(a.preflight_executions(&other_student, save(1)).unwrap(), 0);
    // A date is not part of the student history group: all dates remain counted.
    let mut other_date = scope.clone();
    other_date.set("lessonDate", "2026-11-09".into());
    assert_eq!(a.preflight_executions(&other_date, save(1)).unwrap(), 1);
}
#[test]
fn execution_preflight_refuses_corrupt_foreign_and_oversized_files() {
    for kind in ["ledger", "execution", "outcome", "reconciliation"] {
        for damage in ["corrupt", "foreign", "oversized"] {
            let t = Scratch::new();
            let s = t.open();
            let a = s.lock_account().unwrap();
            let scope = scope();
            let key = seed_execution(&t, &scope, 0, true);
            let path = if kind == "ledger" {
                history_dir(&t, &scope).join(sha(&key) + ".json")
            } else {
                core_path(&t, kind, &key)
            };
            let bytes = match damage {
                "corrupt" => b"not JSON".to_vec(),
                "foreign" if kind == "ledger" => serde_json::to_vec(&json!({
                    "group": "foreign", "key": key
                }))
                .unwrap(),
                "foreign" => serde_json::to_vec(&json!({
                    "kind": kind, "id": "foreign", "record": {}
                }))
                .unwrap(),
                _ => vec![b' '; MAX_FILE_BYTES + 1],
            };
            if path.exists() {
                fs::write(&path, &bytes).unwrap();
            } else {
                put(&path, &bytes);
            }
            assert!(
                a.preflight_executions(&scope, ExecutionAdmission::ReadOnly)
                    .is_err()
            );
            assert_eq!(fs::read(path).unwrap(), bytes); // Preflight never replaces corruption.
        }
    }
}
#[test]
fn execution_preflight_refuses_links_in_every_opened_path() {
    for kind in [
        "ledger-group",
        "ledger",
        "execution",
        "outcome",
        "reconciliation",
    ] {
        let t = Scratch::new();
        let s = t.open();
        let a = s.lock_account().unwrap();
        let scope = scope();
        let key = seed_execution(&t, &scope, 0, true);
        let path = match kind {
            "ledger-group" => history_dir(&t, &scope),
            "ledger" => history_dir(&t, &scope).join(sha(&key) + ".json"),
            _ => core_path(&t, kind, &key),
        };
        let saved = t.0.join("saved");
        if path.exists() {
            fs::rename(&path, &saved).unwrap();
        } else {
            put(&saved, b"unchanged");
        }
        symlink(&saved, &path).unwrap();
        assert!(
            a.preflight_executions(&scope, ExecutionAdmission::ReadOnly)
                .is_err()
        );
    }
}
#[test]
fn execution_preflight_file_limits_and_execution_scope_binding() {
    for kind in ["ledger", "execution", "outcome", "reconciliation"] {
        let t = Scratch::new();
        let s = t.open();
        let a = s.lock_account().unwrap();
        let scope = scope();
        let key = seed_execution(&t, &scope, 0, true);
        let path = if kind == "ledger" {
            history_dir(&t, &scope).join(sha(&key) + ".json")
        } else {
            core_path(&t, kind, &key)
        };
        if kind == "reconciliation" {
            put(
                &path,
                &serde_json::to_vec(&json!({"kind": kind, "id": key,
                "record": {"classification": "persisted"}}))
                .unwrap(),
            );
        }
        let mut bytes = fs::read(&path).unwrap();
        bytes.resize(MAX_FILE_BYTES, b' ');
        fs::write(&path, &bytes).unwrap();
        assert_eq!(
            a.preflight_executions(&scope, ExecutionAdmission::ReadOnly)
                .unwrap(),
            1
        );
        bytes.push(b' ');
        fs::write(&path, &bytes).unwrap();
        assert!(
            a.preflight_executions(&scope, ExecutionAdmission::ReadOnly)
                .is_err()
        );
    }
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let scope = scope();
    let key = seed_execution(&t, &scope, 0, true);
    let path = core_path(&t, "execution", &key);
    let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["record"]["scope"]["studentId"] = "1003".into();
    fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert_eq!(
        error(a.preflight_executions(&scope, ExecutionAdmission::ReadOnly)),
        "record_store_corrupt"
    );
}
fn stored_draft() -> edu_academy_teacher::record_data::Draft {
    use edu_academy_teacher::record_data::*;
    let fields = RecordFields::from_fn(|_| RecordValue::Unknown);
    let patch = RecordFields::from_fn(|_| None);
    let binding =
        ContentBinding::new(identity(), "synthetic-generation".into(), &patch, &fields).unwrap();
    let instruction_hash = content_hash(&"synthetic instruction").unwrap();
    let request_ref =
        "p8s3:".to_owned() + &content_hash(&(&binding, 1, &instruction_hash)).unwrap();
    serde_json::from_value(json!({
        "identity": identity(), "generation": "synthetic-generation", "revision": 1,
        "before": { "identity": identity(), "requested_date": "2026-11-11",
            "provenance": { "read_at": "2026-11-11T18:10:00+09:00", "evidence_refs": ["S-053"],
                "source_revision": "synthetic-revision", "wire_digest": null,
                "field_reasons": RecordFields::from_fn(|_| Option::<String>::None) },
            "observation": { "state": "not_observed" } },
        "binding": binding, "next_attempt_ordinal": 1,
        "pending_attempt": { "request_ref": request_ref, "ordinal": 1,
            "instruction_hash": instruction_hash, "binding": binding },
        "failed_attempts": { "binding": binding, "definite_no_effect_batches": 0 },
        "editable": RecordFields::from_fn(|_| true),
        "dispositions": RecordFields::from_fn(|_| FieldDisposition::Unresolved),
        "teacher_inputs": TeacherInputs::default(), "entries": [], "patch": patch,
        "rendered_preview": "synthetic preview", "preview_hash": content_hash(&"synthetic preview").unwrap(),
        "pending_questions": [{ "field": "replace", "reason_code": "replace_decision_required",
            "text": "Replace?", "choices": [{ "label": "Replace", "answer": { "type": "boolean", "value": true }}], "revision": 1 }],
        "created": "2026-11-11T18:10:00+09:00", "last_used": "2026-11-11T18:10:00+09:00",
        "closed_at": null, "state": "open"
    })).unwrap()
}
#[test]
fn stored_draft_validation_binds_structure_identity_content_questions_attempts_and_time() {
    let d = stored_draft();
    let bytes = serde_json::to_vec(&d).unwrap();
    assert_eq!(validate_draft(&bytes, &identity()).unwrap(), d);
    for (pointer, value) in [
        ("/identity/owner/account", json!("foreign")),
        ("/before/identity/student_key", json!("1003")),
        ("/before/requested_date", json!("2026-11-09")),
        ("/generation", json!("")),
        ("/revision", json!(0)),
        ("/identity/lesson_date", json!("2026-02-30")),
        ("/pending_questions/0/revision", json!(0)),
        ("/pending_attempt/ordinal", json!(0)),
        ("/next_attempt_ordinal", json!(0)),
        ("/binding/source_hash", json!(sha("foreign"))),
        ("/binding/patch_hash", json!(sha("foreign"))),
        ("/binding/generation", json!("foreign")),
        ("/rendered_preview", json!("changed")),
        ("/pending_questions/0/revision", json!(2)),
        ("/pending_questions/0/choices/0/answer/type", json!("text")),
        ("/pending_attempt/ordinal", json!(2)),
        ("/pending_attempt/instruction_hash", json!("x".repeat(64))),
        ("/pending_attempt/instruction_hash", json!("a".repeat(63))),
        ("/pending_attempt/instruction_hash", json!("a".repeat(65))),
        ("/pending_attempt/request_ref", json!("p8s3:foreign")),
        (
            "/pending_attempt/binding/identity/student_key",
            json!("1003"),
        ),
        ("/failed_attempts/binding/generation", json!("foreign")),
        ("/created", json!("bad time")),
        ("/last_used", json!("2026-11-10T18:10:00+09:00")),
        ("/before/provenance/read_at", json!("bad time")),
        ("/closed_at", json!("2026-11-11T18:10:00+09:00")),
        ("/state", json!("saved")),
    ] {
        let mut changed = serde_json::to_value(&d).unwrap();
        *changed.pointer_mut(pointer).unwrap() = value;
        let bytes = serde_json::to_vec(&changed).unwrap();
        assert_eq!(
            error(validate_draft(&bytes, &identity())),
            "record_store_corrupt",
            "{pointer}"
        );
    }
    let mut extra = serde_json::to_value(&d).unwrap();
    extra["pending_approval"] = json!(true);
    assert!(validate_draft(&serde_json::to_vec(&extra).unwrap(), &identity()).is_err());
    assert!(validate_draft(b"not JSON", &identity()).is_err());
    let mut exact = bytes.clone();
    exact.resize(MAX_FILE_BYTES, b' ');
    assert!(validate_draft(&exact, &identity()).is_ok());
    exact.push(b' ');
    assert!(validate_draft(&exact, &identity()).is_err());
}
#[test]
fn stored_draft_validation_preserves_raw_source_and_historical_attempt_binding() {
    use edu_academy_teacher::record_data::*;
    let mut d = stored_draft();
    let mut fields = RecordFields::from_fn(|_| RecordValue::Unknown);
    fields.lesson_memo = RecordValue::Known(" legacy | value\n<!-- raw --> 😀 ".into());
    fields.attendance = RecordValue::Known(String::new());
    d.before.observation = RecordObservation::Observed {
        occurrence: "S-050".into(),
        fields: fields.clone(),
    };
    d.binding =
        Some(ContentBinding::new(identity(), d.generation.clone(), &d.patch, &fields).unwrap());
    assert_eq!(
        validate_draft(&serde_json::to_vec(&d).unwrap(), &identity()).unwrap(),
        d
    );
    d.patch.homework = Some(FieldPatch {
        effect_target: "synthetic:1001:2026-11-11".into(),
        operation: PatchOperation::Replace,
        value: String::new(),
    });
    assert!(validate_draft(&serde_json::to_vec(&d).unwrap(), &identity()).is_err());
    d.binding =
        Some(ContentBinding::new(identity(), d.generation.clone(), &d.patch, &fields).unwrap());
    assert!(validate_draft(&serde_json::to_vec(&d).unwrap(), &identity()).is_ok());
    d.editable.homework = false;
    assert!(validate_draft(&serde_json::to_vec(&d).unwrap(), &identity()).is_err());
    d.editable.homework = true;
    d.before.observation = RecordObservation::Failed {
        reason_code: "synthetic_timeout".into(),
    };
    assert!(validate_draft(&serde_json::to_vec(&d).unwrap(), &identity()).is_err());
    d.binding = None;
    d.state = DraftState::Expired;
    d.closed_at = Some(d.last_used.clone());
    assert!(validate_draft(&serde_json::to_vec(&d).unwrap(), &identity()).is_ok());
}
