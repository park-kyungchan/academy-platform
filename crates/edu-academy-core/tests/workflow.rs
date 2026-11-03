//! Changed-boundary tests operate only in a caller-supplied private evidence root.
use edu_academy_core::{
    js::{Js, obj, stringify},
    workflow::{
        Runtime,
        cism::CismStore,
        codec,
        state::{self, FieldState},
        store::{Kind, LockOptions, Store, mkdir},
    },
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        "2040-01-04T12:00:00.000Z".into()
    }
    fn epoch_ms(&self) -> u64 {
        2209291200000
    }
    fn uuid(&self) -> edu_academy_core::js::Result<String> {
        Ok(format!(
            "00000000-0000-4000-8000-{:012}",
            NEXT.fetch_add(1, Ordering::SeqCst)
        ))
    }
}
fn root(name: &str) -> PathBuf {
    let p = PathBuf::from(
        std::env::var("S10_TEST_ROOT").expect("explicit private evidence root required"),
    )
    .join(format!(
        "{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    mkdir(&p).unwrap();
    p
}
#[test]
fn publication_is_immutable_and_identity_checked_after_readback() {
    let p = root("publication");
    let s = Store::open(&p.join("store"), &p).unwrap();
    let first = obj(&[("b", "한글".into()), ("a", "second".into())]);
    assert!(
        s.publish(Kind::Observation, "old-handle", &first, &Clock)
            .unwrap()
            .0
    );
    let path = fs::read_dir(s.root().join("observation"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let before = fs::read(&path).unwrap();
    let inode = fs::metadata(&path).unwrap().ino();
    let (created, read) = s
        .publish(
            Kind::Observation,
            "old-handle",
            &obj(&[("changed", true.into())]),
            &Clock,
        )
        .unwrap();
    assert!(!created);
    assert_eq!(read, first);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    fs::write(
        &path,
        b"{\"kind\":\"review\",\"id\":\"old-handle\",\"record\":null}",
    )
    .unwrap();
    assert_eq!(
        s.get(Kind::Observation, "old-handle").unwrap_err().message,
        "workflow_store_record_identity_conflict"
    );
}
#[test]
fn index_only_is_available_and_physical_fence_ignores_request_and_date() {
    let p = root("fence");
    let s = Store::open(&p.join("store"), &p).unwrap();
    let scope = obj(&[
        ("sourceNamespace", "namespace".into()),
        ("teacherId", "teacher".into()),
        ("studentId", "student".into()),
        ("lessonDate", "2040-01-04".into()),
    ]);
    s.add_ledger(&state::lock_key(&scope).unwrap(), "index-only", &Clock)
        .unwrap();
    assert!(state::student_executions(&s, &scope).unwrap().is_empty());
    assert_eq!(
        state::field_state(&s, "index-only").unwrap(),
        FieldState::Available
    );
    let r = obj(&[
        ("scope", scope.clone()),
        ("field", "progress".into()),
        ("effectRecordKey", "physical".into()),
    ]);
    s.publish(Kind::Execution, "index-only", &r, &Clock)
        .unwrap();
    let mut other = scope.clone();
    other.set("lessonDate", "2040-01-05".into());
    assert_eq!(
        state::unresolved_on_field(&s, &other, "progress", "physical")
            .unwrap()
            .len(),
        1
    );
    assert!(
        state::unresolved_on_field(&s, &other, "homework", "physical")
            .unwrap()
            .is_empty()
    );
    assert!(
        state::unresolved_on_field(&s, &other, "progress", "other")
            .unwrap()
            .is_empty()
    );
    s.publish(
        Kind::Outcome,
        "index-only",
        &obj(&[("status", "persisted".into())]),
        &Clock,
    )
    .unwrap();
    assert_eq!(
        state::field_state(&s, "index-only").unwrap(),
        FieldState::SettledPersisted
    );
    s.publish(
        Kind::Reconciliation,
        "index-only",
        &obj(&[("classification", "conflict".into())]),
        &Clock,
    )
    .unwrap();
    assert_eq!(
        state::field_state(&s, "index-only").unwrap(),
        FieldState::ReconciledConflict
    );
}
#[test]
fn cancelled_waiter_does_not_replace_or_release_holder_object() {
    let p = root("lock");
    let s = Store::open(&p.join("store"), &p).unwrap();
    let opts = LockOptions {
        wait: Duration::from_millis(15),
        poll: Duration::from_millis(1),
    };
    let held = s.lock("same", opts, &|| false).unwrap();
    let before = held.file().metadata().unwrap();
    assert!(s.lock("same", opts, &|| true).is_err());
    assert!(s.lock("same", opts, &|| false).is_err());
    drop(held);
    let new = s.lock("same", opts, &|| false).unwrap();
    let after = new.file().metadata().unwrap();
    assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
}
#[test]
fn cism_rejects_linked_public_symlink_and_oversized_receipts() {
    let p = root("cism-guards");
    let c = CismStore::open(&p).unwrap();
    assert!(
        c.exclusive("original.json", &obj(&[("fieldRef", "f".into())]))
            .unwrap()
    );
    fs::hard_link(p.join("original.json"), p.join("linked.json")).unwrap();
    assert!(c.read("original.json").is_err());
    assert!(c.read("linked.json").is_err());
    c.exclusive("public.json", &Js::Null).unwrap();
    fs::set_permissions(p.join("public.json"), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(c.read("public.json").is_err());
    symlink(p.join("public.json"), p.join("symlink.json")).unwrap();
    assert!(c.read("symlink.json").is_err());
    c.exclusive("large.json", &Js::Null).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(p.join("large.json"))
        .unwrap()
        .set_len(1_000_001)
        .unwrap();
    assert!(c.read("large.json").is_err());
}
#[test]
fn old_json_preserves_utf16_key_order_duplicate_keys_and_numeric_values() {
    let v = codec::parse(br#"{"z":"\ud800","a":"\udc00","z":"\ud801","2":-0,"1":1e400}"#).unwrap();
    assert_eq!(v.get("z").units(), &[0xd801]);
    assert_eq!(v.get("a").units(), &[0xdc00]);
    assert_eq!(
        stringify(&v).unwrap(),
        "{\"1\":null,\"2\":0,\"z\":\"\\ud801\",\"a\":\"\\udc00\"}"
    );
    for invalid in ["[1,]", "{\"a\":1,}", "01", "[true false]", "\"\\uXYZW\""] {
        assert!(codec::parse(invalid.as_bytes()).is_err());
    }
    let deep = "[".repeat(2000) + "null" + &"]".repeat(2000);
    assert_eq!(
        stringify(&codec::parse(deep.as_bytes()).unwrap()).unwrap(),
        deep
    );
}
