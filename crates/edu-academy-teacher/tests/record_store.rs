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
fn rebind_attempt(d: &mut edu_academy_teacher::record_data::Draft) {
    let a = d.pending_attempt.as_mut().unwrap();
    a.request_ref = "p8s3:".to_owned()
        + &edu_academy_teacher::record_data::content_hash(&(
            &a.binding,
            a.ordinal.get(),
            &a.instruction_hash,
        ))
        .unwrap();
}
#[test]
fn review_repairs_isolate_digest_calendar_and_ledger_filename_checks() {
    for (hash, valid) in [
        ("a".repeat(63), false),
        ("a".repeat(64), true),
        ("a".repeat(65), false),
        ("x".repeat(64), false),
    ] {
        let mut d = stored_draft();
        d.pending_attempt.as_mut().unwrap().instruction_hash = hash;
        rebind_attempt(&mut d);
        let result = validate_draft(&serde_json::to_vec(&d).unwrap(), &identity());
        if valid {
            assert!(result.is_ok());
        } else {
            assert_eq!(error(result), "record_store_corrupt");
        }
    }
    let mut d = stored_draft();
    let mut id = identity();
    id.lesson_date = "2026-02-30".into();
    d.identity = id.clone();
    d.before.identity = id.clone();
    d.before.requested_date = id.lesson_date.clone();
    d.binding.as_mut().unwrap().identity = id.clone();
    d.pending_attempt.as_mut().unwrap().binding.identity = id.clone();
    d.failed_attempts.as_mut().unwrap().binding.identity = id.clone();
    rebind_attempt(&mut d);
    assert_eq!(
        error(validate_draft(&serde_json::to_vec(&d).unwrap(), &id)),
        "record_store_corrupt"
    );
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let scope = scope();
    let key = seed_execution(&t, &scope, 0, true);
    let original = history_dir(&t, &scope).join(sha(&key) + ".json");
    let bytes = fs::read(&original).unwrap();
    let wrong = history_dir(&t, &scope).join(sha("wrong-name") + ".json");
    fs::rename(original, &wrong).unwrap();
    assert_eq!(
        error(a.preflight_executions(&scope, ExecutionAdmission::ReadOnly)),
        "record_store_corrupt"
    );
    assert_eq!(fs::read(wrong).unwrap(), bytes);
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
        (
            "/pending_questions/0/choices/0/answer",
            json!({"type": "text", "value": "yes"}),
        ),
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
        if pointer == "/pending_attempt/instruction_hash" {
            let mut bound = serde_json::from_value(changed).unwrap();
            rebind_attempt(&mut bound);
            changed = serde_json::to_value(bound).unwrap();
        }
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
    let mut future = d.clone();
    let attempt = future.pending_attempt.as_mut().unwrap();
    attempt.ordinal = std::num::NonZeroU64::new(2).unwrap();
    attempt.request_ref = "p8s3:".to_owned()
        + &edu_academy_teacher::record_data::content_hash(&(
            &attempt.binding,
            attempt.ordinal.get(),
            &attempt.instruction_hash,
        ))
        .unwrap();
    assert!(validate_draft(&serde_json::to_vec(&future).unwrap(), &identity()).is_err());
    future.next_attempt_ordinal = std::num::NonZeroU64::new(2).unwrap();
    assert!(validate_draft(&serde_json::to_vec(&future).unwrap(), &identity()).is_ok());
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
fn retained_draft(
    student: &str,
    index: usize,
    open: bool,
) -> edu_academy_teacher::record_data::Draft {
    use edu_academy_teacher::record_data::*;
    let mut d = stored_draft();
    d.identity.student_key = student.into();
    if open {
        d.identity.lesson_date = (chrono::NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()
            + chrono::Duration::days(index as i64))
        .to_string();
    }
    d.generation = format!("synthetic-generation-{index}");
    d.before.identity = d.identity.clone();
    d.before.requested_date = d.identity.lesson_date.clone();
    d.binding.as_mut().unwrap().identity = d.identity.clone();
    d.binding.as_mut().unwrap().generation = d.generation.clone();
    d.pending_attempt = None;
    d.failed_attempts = None;
    if !open {
        d.state = DraftState::Expired;
        d.closed_at = Some(d.last_used.clone());
    }
    d
}
fn put_draft(t: &Scratch, d: &edu_academy_teacher::record_data::Draft) -> PathBuf {
    let path = t
        .root()
        .join(draft_path(&d.identity, &d.generation).unwrap());
    if !path.parent().unwrap().exists() {
        DirBuilder::new()
            .mode(0o700)
            .create(path.parent().unwrap())
            .unwrap();
    }
    put(&path, &serde_json::to_vec(d).unwrap());
    path
}
fn clock_at(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap()
        .to_rfc3339()
}
#[test]
fn draft_lookup_and_cleanup_256_boundaries() {
    for count in [MAX_DRAFT_CANDIDATES, MAX_DRAFT_CANDIDATES + 1] {
        let t = Scratch::new();
        let s = t.open();
        let a = s.lock_account().unwrap();
        let mut paths = vec![];
        for i in 0..count {
            paths.push(put_draft(&t, &retained_draft("1001", i, false)));
        }
        if count == MAX_DRAFT_CANDIDATES {
            assert_eq!(a.lookup_drafts("1001").unwrap().len(), count);
            let clean = a
                .cleanup_drafts(&scope(), "2026-11-19T18:10:00+09:00")
                .unwrap();
            assert_eq!((clean.examined, clean.removed), (count, count));
            assert!(paths.iter().all(|p| !p.exists()));
        } else {
            assert_eq!(error(a.lookup_drafts("1001")), "record_store_capacity");
            assert_eq!(
                error(a.cleanup_drafts(&scope(), "2026-11-19T18:10:00+09:00")),
                "record_store_capacity"
            );
            assert!(paths.iter().all(|p| p.exists()));
        }
    }
}
#[test]
fn prospective_candidate_capacity_preserves_continuation_and_recovers_after_cleanup() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let now = "2026-11-12T18:10:00+09:00";
    let mut originals = vec![];
    for i in 0..MAX_DRAFT_CANDIDATES - 1 {
        let path = put_draft(&t, &retained_draft("1001", i, false));
        originals.push((path.clone(), fs::read(path).unwrap()));
    }
    let next = retained_draft("1001", MAX_DRAFT_CANDIDATES - 1, false);
    let bytes = serde_json::to_vec(&next).unwrap();
    a.admit_draft_write(
        &scope(),
        now,
        Some(&next.identity),
        bytes.len() as u64,
        10000,
    )
    .unwrap();
    {
        let d = a.lock_draft(&next.identity).unwrap();
        let path = draft_path(&next.identity, &next.generation).unwrap();
        d.write_atomic(&path, &bytes).unwrap();
        originals.push((t.root().join(path), bytes.clone()));
    }
    assert_eq!(a.lookup_drafts("1001").unwrap().len(), MAX_DRAFT_CANDIDATES);
    assert!(
        a.admit_draft_write(&scope(), now, None, bytes.len() as u64, 10000)
            .is_ok()
    );
    assert_eq!(
        error(a.admit_draft_write(
            &scope(),
            now,
            Some(&next.identity),
            bytes.len() as u64,
            10000
        )),
        "record_store_capacity"
    );
    for (path, bytes) in &originals {
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    let next = retained_draft("1001", MAX_DRAFT_CANDIDATES, false);
    let bytes = serde_json::to_vec(&next).unwrap();
    a.admit_draft_write(
        &scope(),
        "2026-11-19T18:10:00+09:00",
        Some(&next.identity),
        bytes.len() as u64,
        10000,
    )
    .unwrap();
    assert!(a.lookup_drafts("1001").unwrap().is_empty());
    assert!(originals.iter().all(|(p, _)| !p.exists()));
    // The raw file writer requires an existing parent; typed create is a later
    // part. Cleanup has removed this empty bucket, so arrange it explicitly.
    DirBuilder::new()
        .mode(0o700)
        .create(originals[0].0.parent().unwrap())
        .unwrap();
    let d = a.lock_draft(&next.identity).unwrap();
    d.write_atomic(
        &draft_path(&next.identity, &next.generation).unwrap(),
        &bytes,
    )
    .unwrap();
    assert_eq!(a.lookup_drafts("1001").unwrap(), vec![next]);
}
#[test]
fn empty_bucket_enumeration_at_1024_and_1025() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let root = t.root().join("record-drafts");
    for i in 0..MAX_RETAINED_GENERATIONS {
        DirBuilder::new()
            .mode(0o700)
            .create(root.join(sha(&format!("bucket-{i}"))))
            .unwrap();
    }
    assert_eq!(a.retained_generations().unwrap(), 0);
    let extra = root.join(sha("bucket-extra"));
    DirBuilder::new().mode(0o700).create(&extra).unwrap();
    assert_eq!(error(a.retained_generations()), "record_store_capacity");
    assert_eq!(
        fs::read_dir(&root).unwrap().count(),
        MAX_RETAINED_GENERATIONS + 1
    );
    fs::remove_dir(extra).unwrap();
    assert_eq!(a.retained_generations().unwrap(), 0);
    let now = "2026-11-12T18:10:00+09:00";
    assert_eq!(
        error(a.admit_draft_write(&scope(), now, Some(&identity()), 0, 10000)),
        "record_store_capacity"
    );
    a.admit_draft_write(&scope(), now, None, 0, 10000).unwrap();
    fs::remove_dir(root.join(sha("bucket-0"))).unwrap();
    a.admit_draft_write(&scope(), now, Some(&identity()), 0, 10000)
        .unwrap();
    let bucket = edu_academy_teacher::record_data::content_hash(&(&owner(), "1001")).unwrap();
    DirBuilder::new()
        .mode(0o700)
        .create(root.join(bucket))
        .unwrap();
    // At 1,024 buckets an existing student's bucket still has payload room.
    a.admit_draft_write(&scope(), now, Some(&identity()), 0, 10000)
        .unwrap();
    assert_eq!(a.retained_generations().unwrap(), 0);
}
#[test]
fn admission_failure_after_cleanup_requires_fresh_inventory_before_retry() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let eligible = put_draft(&t, &retained_draft("1001", 0, false));
    let retained = retained_draft("1001", 1, true);
    let kept = put_draft(&t, &retained);
    let kept_bytes = fs::read(&kept).unwrap();
    put(
        &t.root().join("request/retained-evidence"),
        b"synthetic evidence",
    );
    assert_eq!(a.lookup_drafts("1001").unwrap().len(), 2);
    let before = a.usage(10000).unwrap();
    let now = "2026-11-19T18:10:00+09:00";
    assert_eq!(
        error(a.admit_draft_write(&scope(), now, None, u64::MAX, 10000)),
        "record_store_capacity"
    );
    // The failed call already deleted an eligible payload. Never infer rollback
    // from its error or retry with the previous inventory/byte estimate.
    let inventory = a.lookup_drafts("1001").unwrap();
    let after = a.usage(10000).unwrap();
    assert_eq!(inventory, vec![retained]);
    assert!(!eligible.exists());
    assert!(after.bytes < before.bytes);
    assert_eq!(fs::read(kept).unwrap(), kept_bytes);
    assert_eq!(
        fs::read(t.root().join("request/retained-evidence")).unwrap(),
        b"synthetic evidence"
    );
    assert_eq!(
        a.admit_draft_write(&scope(), now, None, 0, 10000).unwrap(),
        after
    );
    assert_eq!(a.lookup_drafts("1001").unwrap(), inventory);
}
#[test]
fn open_date_and_retained_generation_boundaries() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    for i in 0..MAX_OPEN_DATES {
        put_draft(&t, &retained_draft("1001", i, true));
    }
    assert_eq!(a.lookup_drafts("1001").unwrap().len(), MAX_OPEN_DATES);
    let next = retained_draft("1001", MAX_OPEN_DATES, true);
    assert_eq!(
        error(a.admit_draft_write(
            &scope(),
            "2026-11-12T18:10:00+09:00",
            Some(&next.identity),
            0,
            10000
        )),
        "record_store_capacity"
    );
    assert!(
        a.admit_draft_write(
            &scope(),
            "2026-11-12T18:10:00+09:00",
            Some(&identity()),
            0,
            10000
        )
        .is_ok()
    );
    put_draft(&t, &next);
    assert_eq!(error(a.lookup_drafts("1001")), "record_store_capacity");
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    for i in 0..MAX_RETAINED_GENERATIONS {
        let student = ["1001", "1002", "1003", "1004"][i / MAX_DRAFT_CANDIDATES];
        put_draft(&t, &retained_draft(student, i, false));
    }
    assert_eq!(a.retained_generations().unwrap(), MAX_RETAINED_GENERATIONS);
    assert!(
        a.admit_draft_write(&scope(), "2026-11-12T18:10:00+09:00", None, 0, 10000)
            .is_ok()
    );
    assert_eq!(
        error(a.admit_draft_write(
            &scope(),
            "2026-11-12T18:10:00+09:00",
            Some(&identity()),
            0,
            10000
        )),
        "record_store_capacity"
    );
    put_draft(&t, &retained_draft("1005", MAX_RETAINED_GENERATIONS, false));
    assert_eq!(error(a.retained_generations()), "record_store_capacity");
}
#[test]
fn retention_uses_later_closure_or_last_use_at_seven_days_and_above() {
    for (closure_later, over) in [(false, 0), (false, 1), (true, 0), (true, 1)] {
        let t = Scratch::new();
        let s = t.open();
        let a = s.lock_account().unwrap();
        let mut d = retained_draft("1001", 0, false);
        if closure_later {
            d.closed_at = Some("2026-11-12T18:10:00+09:00".into());
        } else {
            d.last_used = "2026-11-12T18:10:00+09:00".into();
        }
        let path = put_draft(&t, &d);
        let bytes = fs::read(&path).unwrap();
        let later = if closure_later {
            d.closed_at.as_ref().unwrap()
        } else {
            &d.last_used
        };
        let end = chrono::DateTime::parse_from_rfc3339(later)
            .unwrap()
            .timestamp_millis()
            + DRAFT_RETENTION_MS;
        let clean = a.cleanup_drafts(&scope(), &clock_at(end - 1)).unwrap();
        assert_eq!((clean.removed, clean.next_eligible_ms), (0, Some(end)));
        assert_eq!(a.lookup_drafts("1001").unwrap()[0], d);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(
            a.cleanup_drafts(&scope(), &clock_at(end + over))
                .unwrap()
                .removed,
            1
        );
        assert!(!path.exists());
    }
}
#[test]
fn unresolved_execution_protects_its_generation_and_unknown_references_are_conservative() {
    use edu_academy_teacher::record_data::DraftState;
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let mut d = stored_draft();
    d.state = DraftState::Expired;
    d.closed_at = Some(d.last_used.clone());
    let path = put_draft(&t, &d);
    let other = put_draft(&t, &retained_draft("1001", 1, false));
    let scope = scope();
    let key = seed_execution(&t, &scope, 0, false);
    let execution = core_path(&t, "execution", &key);
    let mut record: Value = serde_json::from_slice(&fs::read(&execution).unwrap()).unwrap();
    record["record"]["requestRef"] = d
        .pending_attempt
        .as_ref()
        .unwrap()
        .request_ref
        .clone()
        .into();
    fs::write(&execution, serde_json::to_vec(&record).unwrap()).unwrap();
    let original = fs::read(&execution).unwrap();
    let clean = a
        .cleanup_drafts(&scope, "2026-11-20T18:10:00+09:00")
        .unwrap();
    assert_eq!((clean.removed, clean.protected), (1, 1));
    assert!(path.exists());
    assert!(!other.exists());
    assert_eq!(fs::read(&execution).unwrap(), original);
    record["record"]["requestRef"] = "unknown-request".into();
    fs::write(&execution, serde_json::to_vec(&record).unwrap()).unwrap();
    assert_eq!(
        a.cleanup_drafts(&scope, "2026-11-20T18:10:00+09:00")
            .unwrap()
            .protected,
        1
    );
    put(
        &core_path(&t, "outcome", &key),
        &serde_json::to_vec(&json!({"kind":"outcome", "id": key, "record":{"status":"noop"}}))
            .unwrap(),
    );
    assert_eq!(
        a.cleanup_drafts(&scope, "2026-11-20T18:10:00+09:00")
            .unwrap()
            .removed,
        1
    );
    assert!(execution.exists());
    assert!(history_dir(&t, &scope).join(sha(&key) + ".json").exists());
}
#[test]
fn prospective_bytes_are_checked_without_subtracting_old_payload_and_after_cleanup() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    put(&t.root().join("request/old"), b"existing payload");
    let usage = a.usage(100).unwrap();
    let space = MAX_ROOT_BYTES - usage.bytes;
    assert_eq!(
        a.admit_draft_write(&scope(), "2026-11-12T18:10:00+09:00", None, space, 100)
            .unwrap(),
        usage
    );
    for n in [space + 1, u64::MAX] {
        assert_eq!(
            error(a.admit_draft_write(&scope(), "2026-11-12T18:10:00+09:00", None, n, 100)),
            "record_store_capacity"
        );
    }
    let path = put_draft(&t, &retained_draft("1001", 0, false));
    let f = file(&t.root().join("audit/sparse"));
    f.set_len(MAX_ROOT_BYTES - a.usage(100).unwrap().bytes + 1)
        .unwrap();
    assert!(a.usage(100).is_err());
    assert!(
        a.admit_draft_write(&scope(), "2026-11-20T18:10:00+09:00", None, 0, 100)
            .is_ok()
    );
    assert!(!path.exists());
    assert_eq!(
        fs::read(t.root().join("request/old")).unwrap(),
        b"existing payload"
    );
}
#[test]
fn lookup_refuses_corruption_foreign_identity_path_aliases_and_links_before_cleanup() {
    for damage in [
        "corrupt",
        "foreign",
        "wrong-name",
        "file-link",
        "bucket-link",
    ] {
        let t = Scratch::new();
        let s = t.open();
        let a = s.lock_account().unwrap();
        let d = retained_draft("1001", 0, false);
        let path = put_draft(&t, &d);
        let other = put_draft(&t, &retained_draft("1001", 1, false));
        match damage {
            "corrupt" => fs::write(&path, b"not JSON").unwrap(),
            "foreign" => fs::write(
                &path,
                serde_json::to_vec(&retained_draft("1003", 0, false)).unwrap(),
            )
            .unwrap(),
            "wrong-name" => fs::rename(&path, path.with_file_name("wrong.json")).unwrap(),
            _ => {
                let target = if damage == "bucket-link" {
                    path.parent().unwrap()
                } else {
                    &path
                };
                let saved = t.0.join("saved");
                fs::rename(target, &saved).unwrap();
                symlink(&saved, target).unwrap();
            }
        }
        assert!(a.lookup_drafts("1001").is_err());
        assert!(
            a.cleanup_drafts(&scope(), "2026-11-20T18:10:00+09:00")
                .is_err()
        );
        assert!(other.exists());
    }
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let d = retained_draft("1001", 0, true);
    put_draft(&t, &d);
    let mut duplicate = d.clone();
    duplicate.generation.push('x');
    duplicate.binding.as_mut().unwrap().generation = duplicate.generation.clone();
    put_draft(&t, &duplicate);
    assert_eq!(error(a.lookup_drafts("1001")), "record_store_corrupt");
}
#[test]
fn cleanup_takes_the_draft_lock_before_deleting_an_eligible_payload() {
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let d = retained_draft("1001", 0, false);
    let path = put_draft(&t, &d);
    drop(a.lock_draft(&d.identity).unwrap());
    let raw = File::open(
        t.root()
            .join(format!("locks/{}.flock", d.identity.key().unwrap())),
    )
    .unwrap();
    raw.try_lock().unwrap();
    assert_eq!(
        error(a.cleanup_drafts(&scope(), "2026-11-20T18:10:00+09:00")),
        "shared_writer_lock_busy"
    );
    assert!(path.exists());
    raw.unlock().unwrap();
    assert_eq!(
        a.cleanup_drafts(&scope(), "2026-11-20T18:10:00+09:00")
            .unwrap()
            .removed,
        1
    );
    assert!(!path.parent().unwrap().exists());
}

fn draft_seed() -> edu_academy_teacher::record_data::Draft {
    let mut d = stored_draft();
    d.pending_attempt = None;
    d.failed_attempts = None;
    d.teacher_inputs.memo = Some("explicit synthetic intent".into());
    d.teacher_inputs.replace = Some(true);
    d
}
const DRAFT_NOW: &str = "2026-11-11T18:10:00+09:00";
fn draft_start_ms() -> i64 {
    chrono::DateTime::parse_from_rfc3339(DRAFT_NOW)
        .unwrap()
        .timestamp_millis()
}
#[test]
fn draft_reopens_expires_and_revalidates() {
    use edu_academy_teacher::record_data::{
        DraftState, RecordFields, RecordObservation, RecordValue, content_hash,
    };
    let t = Scratch::new();
    let first = {
        let s = t.open();
        let a = s.lock_account().unwrap();
        assert!(a.load_draft(&identity(), None).unwrap().is_none());
        assert_eq!(
            error(a.load_draft(&identity(), Some("unknown"))),
            "draft_replaced"
        );
        assert_eq!(
            error(a.create_draft(&scope(), draft_seed(), DRAFT_NOW, 0)),
            "record_store_capacity"
        );
        let d = a
            .create_draft(&scope(), draft_seed(), DRAFT_NOW, 10000)
            .unwrap();
        assert_eq!(d.generation.len(), GENERATION_NONCE_BYTES * 2);
        assert!(d.generation.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(d.generation, draft_seed().generation);
        assert_eq!(d.revision.get(), 1);
        d
    };
    let s = t.open();
    let a = s.lock_account().unwrap();
    let path = t
        .root()
        .join(draft_path(&first.identity, &first.generation).unwrap());
    let original = fs::read(&path).unwrap();
    assert_eq!(
        a.load_draft(&identity(), Some(&first.generation)).unwrap(),
        Some(first.clone())
    );
    assert_eq!(fs::read(&path).unwrap(), original); // Reads never refresh idle time.
    assert_eq!(
        error(a.load_draft(&identity(), Some("unknown-generation"))),
        "draft_replaced"
    );
    assert_eq!(
        error(a.create_draft(&scope(), draft_seed(), DRAFT_NOW, 10000)),
        "draft_exists"
    );
    let mut fresh = first.before.clone();
    fresh.provenance.read_at = clock_at(draft_start_ms() + 1);
    fresh
        .provenance
        .evidence_refs
        .push("new-read-only-evidence".into());
    let used = a
        .revalidate_draft(&scope(), &first, &fresh, &fresh.provenance.read_at, 10000)
        .unwrap();
    assert!(!used.source_changed);
    assert_eq!(used.draft.revision, first.revision);
    assert_eq!(used.draft.binding, first.binding);
    assert_eq!(used.draft.last_used, fresh.provenance.read_at);
    let mut next = used.draft.clone();
    next.teacher_inputs.memo = Some("edited intent".into());
    let edited = a
        .replace_draft(
            &scope(),
            &used.draft,
            next,
            &fresh.provenance.read_at,
            10000,
        )
        .unwrap();
    assert!(edited.revision > first.revision);
    assert!(
        edited
            .pending_questions
            .iter()
            .all(|q| q.revision == edited.revision)
    );
    let saved_bytes = fs::read(&path).unwrap();
    assert_eq!(
        error(a.replace_draft(&scope(), &first, first.clone(), DRAFT_NOW, 10000)),
        "draft_revision_conflict"
    );
    assert_eq!(fs::read(&path).unwrap(), saved_bytes);
    let mut raw = RecordFields::from_fn(|_| RecordValue::Unknown);
    raw.lesson_memo = RecordValue::Known("가".repeat(601) + "raw suffix A");
    fresh.observation = RecordObservation::Observed {
        occurrence: "synthetic-occurrence".into(),
        fields: raw.clone(),
    };
    let changed = a
        .revalidate_draft(&scope(), &edited, &fresh, &fresh.provenance.read_at, 10000)
        .unwrap();
    assert!(changed.source_changed);
    let d = changed.draft;
    assert!(d.revision > edited.revision);
    assert!(d.binding.is_none() && d.pending_questions.is_empty() && d.rendered_preview.is_empty());
    assert_eq!(d.preview_hash, content_hash(&String::new()).unwrap());
    assert!(d.patch.lesson_memo.is_none());
    assert_eq!(d.teacher_inputs.memo, edited.teacher_inputs.memo);
    assert_eq!(d.entries, edited.entries);
    assert!(d.teacher_inputs.replace.is_none());
    // Same visible prefix, different complete raw value must invalidate again.
    raw.lesson_memo = RecordValue::Known("가".repeat(601) + "raw suffix B");
    fresh.observation = RecordObservation::Observed {
        occurrence: "synthetic-occurrence".into(),
        fields: raw,
    };
    let d2 = a
        .revalidate_draft(&scope(), &d, &fresh, &fresh.provenance.read_at, 10000)
        .unwrap();
    assert!(d2.source_changed);
    fresh.provenance.source_revision = Some("source-revision-2".into());
    let d3 = a
        .revalidate_draft(
            &scope(),
            &d2.draft,
            &fresh,
            &fresh.provenance.read_at,
            10000,
        )
        .unwrap();
    assert!(d3.source_changed && d3.draft.revision > d2.draft.revision);
    let replacement = a
        .restart_draft(
            &scope(),
            &d3.draft,
            draft_seed(),
            &fresh.provenance.read_at,
            10000,
        )
        .unwrap();
    assert_ne!(replacement.generation, first.generation);
    assert!(replacement.revision > d3.draft.revision);
    let retained = validate_draft(&fs::read(&path).unwrap(), &identity()).unwrap();
    assert_eq!(retained.state, DraftState::Expired);
    assert!(replacement.revision > retained.revision);
    a.cleanup_drafts(
        &scope(),
        &clock_at(draft_start_ms() + DRAFT_RETENTION_MS + 2),
    )
    .unwrap();
    assert!(!path.exists());
    assert_eq!(
        error(a.load_draft(&identity(), Some(&first.generation))),
        "draft_replaced"
    );
    assert_eq!(
        error(a.replace_draft(&scope(), &first, first.clone(), DRAFT_NOW, 10000)),
        "draft_replaced"
    );
    assert_eq!(a.load_draft(&identity(), None).unwrap(), Some(replacement));
    for delta in [DRAFT_IDLE_MS - 1, DRAFT_IDLE_MS, DRAFT_IDLE_MS + 1] {
        let t = Scratch::new();
        let s = t.open();
        let a = s.lock_account().unwrap();
        let d = a
            .create_draft(&scope(), draft_seed(), DRAFT_NOW, 10000)
            .unwrap();
        let now = clock_at(draft_start_ms() + delta);
        let result = a.revalidate_draft(&scope(), &d, &d.before, &now, 10000);
        if delta < DRAFT_IDLE_MS {
            let refreshed = result.unwrap().draft;
            assert_eq!(refreshed.last_used, now);
            let later = clock_at(draft_start_ms() + DRAFT_IDLE_MS);
            assert!(
                a.revalidate_draft(&scope(), &refreshed, &refreshed.before, &later, 10000)
                    .is_ok()
            );
        } else {
            assert_eq!(error(result), "draft_expired");
            let closed = a
                .load_draft(&identity(), Some(&d.generation))
                .unwrap()
                .unwrap();
            assert_eq!(closed.state, DraftState::Expired);
            assert_eq!(closed.last_used, d.last_used);
            assert_eq!(closed.closed_at.as_deref(), Some(now.as_str()));
            assert!(closed.revision > d.revision);
            assert_eq!(
                error(a.replace_draft(&scope(), &closed, closed.clone(), &now, 10000)),
                "draft_expired"
            );
            let new = a
                .restart_draft(&scope(), &closed, draft_seed(), &now, 10000)
                .unwrap();
            assert!(new.revision > closed.revision && new.generation != closed.generation);
        }
    }
}
#[test]
fn draft_capacity_scope_and_revision_fail_before_mutation() {
    use std::num::NonZeroU64;
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let d = a
        .create_draft(&scope(), draft_seed(), DRAFT_NOW, 10000)
        .unwrap();
    let path = t
        .root()
        .join(draft_path(&identity(), &d.generation).unwrap());
    let before = fs::read(&path).unwrap();
    assert_eq!(
        error(a.replace_draft(&scope(), &d, d.clone(), DRAFT_NOW, 0)),
        "record_store_capacity"
    );
    assert_eq!(
        error(a.revalidate_draft(&scope(), &d, &d.before, DRAFT_NOW, 0)),
        "record_store_capacity"
    );
    let mut invalid = draft_seed();
    invalid.identity.student_key = "1002".into();
    assert_eq!(
        error(a.restart_draft(&scope(), &d, invalid, DRAFT_NOW, 10000)),
        "record_store_corrupt"
    );
    for i in 0..MAX_DRAFT_CANDIDATES - 1 {
        let mut old = retained_draft("1001", i, false);
        old.identity.lesson_date = "2026-11-10".into();
        old.before.identity = old.identity.clone();
        old.before.requested_date = old.identity.lesson_date.clone();
        old.binding.as_mut().unwrap().identity = old.identity.clone();
        put_draft(&t, &old);
    }
    assert_eq!(
        error(a.restart_draft(&scope(), &d, draft_seed(), DRAFT_NOW, 10000)),
        "record_store_capacity"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(a.lookup_drafts("1001").unwrap().len(), MAX_DRAFT_CANDIDATES);
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    let mut d = draft_seed();
    d.revision = NonZeroU64::new(u64::MAX).unwrap();
    for q in &mut d.pending_questions {
        q.revision = d.revision;
    }
    let path = put_draft(&t, &d);
    let before = fs::read(&path).unwrap();
    assert_eq!(
        error(a.replace_draft(&scope(), &d, d.clone(), DRAFT_NOW, 10000)),
        "record_store_capacity"
    );
    assert_eq!(fs::read(path).unwrap(), before);
}
fn wait_control(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "synthetic barrier deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn draft_process_writer() {
    let Some(root) = std::env::var_os("A2B3_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let control = root.parent().unwrap();
    let role = std::env::var("A2B3_ROLE").unwrap();
    let phase = std::env::var("A2B3_PHASE").unwrap();
    let s = DraftStore::open(&root, owner()).unwrap();
    let expected = s
        .lock_account()
        .unwrap()
        .load_draft(&identity(), None)
        .unwrap()
        .unwrap();
    put(&control.join(format!("ready-{role}")), b"ready");
    wait_control(&control.join(format!("go-{role}")));
    let a = s.lock_account().unwrap();
    let result = if phase == "restart" && role == "a" {
        a.restart_draft(&scope(), &expected, draft_seed(), DRAFT_NOW, 10000)
    } else {
        let mut next = expected.clone();
        next.teacher_inputs.memo = Some(role.clone());
        a.replace_draft(&scope(), &expected, next, DRAFT_NOW, 10000)
    };
    let status = result.map(|_| "ok".into()).unwrap_or_else(|e| e.message);
    put(&control.join(format!("result-{role}")), status.as_bytes());
}
#[test]
fn draft_two_processes_one_record() {
    for phase in ["edit", "restart"] {
        let t = Scratch::new();
        let s = t.open();
        let initial = s
            .lock_account()
            .unwrap()
            .create_draft(&scope(), draft_seed(), DRAFT_NOW, 10000)
            .unwrap();
        let mut children: Vec<_> = ["a", "b"]
            .iter()
            .map(|role| {
                Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "draft_process_writer"])
                    .env("A2B3_ROOT", t.root())
                    .env("A2B3_ROLE", role)
                    .env("A2B3_PHASE", phase)
                    .stdout(Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        for role in ["a", "b"] {
            wait_control(&t.0.join(format!("ready-{role}")));
        }
        put(&t.0.join("go-a"), b"go");
        if phase == "restart" {
            wait_control(&t.0.join("result-a"));
        }
        put(&t.0.join("go-b"), b"go");
        for child in &mut children {
            finish(child);
        }
        let statuses: Vec<_> = ["a", "b"]
            .iter()
            .map(|role| fs::read_to_string(t.0.join(format!("result-{role}"))).unwrap())
            .collect();
        assert_eq!(statuses.iter().filter(|s| s.as_str() == "ok").count(), 1);
        let a = s.lock_account().unwrap();
        let current = a.load_draft(&identity(), None).unwrap().unwrap();
        if phase == "edit" {
            assert!(statuses.iter().any(|s| s == "draft_revision_conflict"));
            assert_eq!(current.generation, initial.generation);
            assert_eq!(current.revision.get(), initial.revision.get() + 1);
            let winner = if statuses[0] == "ok" { "a" } else { "b" };
            assert_eq!(current.teacher_inputs.memo.as_deref(), Some(winner));
        } else {
            assert_eq!(statuses, ["ok", "draft_replaced"]);
            assert_ne!(current.generation, initial.generation);
            assert_eq!(
                current.teacher_inputs.memo,
                draft_seed().teacher_inputs.memo
            );
            let old_path = draft_path(&initial.identity, &initial.generation).unwrap();
            let archived =
                validate_draft(&a.read_bytes(&old_path).unwrap().unwrap(), &identity()).unwrap();
            assert_eq!(
                archived.state,
                edu_academy_teacher::record_data::DraftState::Expired
            );
            assert_eq!(archived.teacher_inputs, initial.teacher_inputs);
        }
        assert_eq!(
            a.lookup_drafts("1001")
                .unwrap()
                .iter()
                .filter(|d| d.state == edu_academy_teacher::record_data::DraftState::Open)
                .count(),
            1
        );
    }
}
#[test]
fn usage_breadth_under_child_descriptor_limit() {
    use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
    if std::env::var_os("A2B3_FD_CHILD").is_none() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "usage_breadth_under_child_descriptor_limit"])
            .env("A2B3_FD_CHILD", "1")
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        finish(&mut child);
        return;
    }
    let limit = getrlimit(Resource::Nofile);
    setrlimit(
        Resource::Nofile,
        Rlimit {
            current: Some(64.min(limit.current.unwrap_or(64))),
            maximum: limit.maximum,
        },
    )
    .unwrap();
    let t = Scratch::new();
    let s = t.open();
    let a = s.lock_account().unwrap();
    for i in 0..MAX_RETAINED_GENERATIONS {
        DirBuilder::new()
            .mode(0o700)
            .create(
                t.root()
                    .join("record-drafts")
                    .join(sha(&format!("empty-{i}"))),
            )
            .unwrap();
    }
    assert_eq!(a.retained_generations().unwrap(), 0);
    assert!(a.usage(10000).unwrap().entries >= MAX_RETAINED_GENERATIONS);
}
