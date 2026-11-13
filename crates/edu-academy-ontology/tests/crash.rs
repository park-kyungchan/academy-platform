//! Body ordering and recovery against the real engine; all payloads are invented.
use edu_academy_core::{
    js::{Fault, Js, Result, obj, sha},
    workflow::{
        Runtime, codec,
        ledger::{Attempt, AttemptLedger, AttemptTarget},
        ledger_cases::{self, Clock, execution, outcome, reconciliation, scope, target},
        state::{self, FieldState},
        store::{Kind, Store},
    },
};
use edu_academy_ontology::{EngineLedger, open_store};
use std::{cell::Cell, fs, path::PathBuf, rc::Rc};

struct Fixture {
    owner: PathBuf,
    root: PathBuf,
    ledger: EngineLedger,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let owner = ledger_cases::root(name);
        let root = owner.join("store");
        open_store(&root, &owner, &Clock).unwrap();
        let ledger = EngineLedger::new(owner.join("store.ledger/academy.sqlite"), &Clock);
        Self {
            owner,
            root,
            ledger,
        }
    }
    fn store(&self) -> Store {
        open_store(&self.root, &self.owner, &Clock).unwrap()
    }
    fn execution(&self, store: &Store) -> Js {
        let s = scope("12");
        execution(
            store,
            &s,
            "progress",
            "record".into(),
            &target(&s),
            "review",
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.owner).unwrap();
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Call {
    Begin,
    Outcome,
    Reconciliation,
}
struct FaultLedger {
    inner: EngineLedger,
    root: PathBuf,
    fail: Rc<Cell<Option<(Call, bool)>>>,
}
impl FaultLedger {
    fn around(
        &self,
        call: Call,
        kind: Kind,
        key: &str,
        digest: &str,
        apply: impl FnOnce() -> Result<bool>,
    ) -> Result<bool> {
        let path = self.root.join(kind.name()).join(format!(
            "{}.{}.json",
            sha(&format!("{}\n{key}", kind.name())),
            digest
        ));
        let bytes = fs::read(path).expect("a durable body must precede every action");
        assert_eq!(
            sha(std::str::from_utf8(&bytes).unwrap()),
            digest,
            "action must name the already durable bytes"
        );
        let fault = self.fail.get();
        if fault == Some((call, false)) {
            self.fail.set(None);
            return Err(Fault::error("invented_before_action"));
        }
        let answer = apply()?;
        if fault == Some((call, true)) {
            self.fail.set(None);
            return Err(Fault::error("invented_lost_answer"));
        }
        Ok(answer)
    }
}
impl AttemptLedger for FaultLedger {
    fn begin(&self, k: &str, t: &AttemptTarget, d: &str, c: &dyn Runtime) -> Result<bool> {
        self.around(Call::Begin, Kind::Execution, k, d, || {
            self.inner.begin(k, t, d, c)
        })
    }
    fn record_outcome(&self, k: &str, d: &str, s: FieldState, c: &dyn Runtime) -> Result<bool> {
        self.around(Call::Outcome, Kind::Outcome, k, d, || {
            self.inner.record_outcome(k, d, s, c)
        })
    }
    fn record_reconciliation(
        &self,
        k: &str,
        d: &str,
        s: FieldState,
        c: &dyn Runtime,
    ) -> Result<bool> {
        self.around(Call::Reconciliation, Kind::Reconciliation, k, d, || {
            self.inner.record_reconciliation(k, d, s, c)
        })
    }
    fn attempt(&self, k: &str) -> Result<Option<Attempt>> {
        self.inner.attempt(k)
    }
    fn attempts(&self, g: &str, n: usize) -> Result<Vec<Attempt>> {
        self.inner.attempts(g, n)
    }
    fn open_on(&self, t: &AttemptTarget) -> Result<Vec<Attempt>> {
        self.inner.open_on(t)
    }
}
#[test]
fn crash_walk() {
    // Rows 1..7 of design section 5; row 7 covers both ways to close.
    for row in 1..=7 {
        for settle in [false, true] {
            if row != 7 && settle {
                continue;
            }
            let f = Fixture::new(&format!("crash-{row}-{settle}"));
            let fail = Rc::new(Cell::new(None));
            let store = Store::open_with_ledger(
                &f.root,
                &f.owner,
                Box::new(FaultLedger {
                    inner: f.ledger.clone(),
                    root: f.root.clone(),
                    fail: fail.clone(),
                }),
            )
            .unwrap();
            let e = f.execution(&store);
            if row == 1 {
                fail.set(Some((Call::Begin, false)));
                assert!(store.publish(Kind::Execution, "key", &e, &Clock).is_err());
                assert_eq!(store.get(Kind::Execution, "key").unwrap(), Js::Null);
                let mut retry = e.clone();
                retry.set("inventedRetry", "new-body".into());
                assert!(
                    store
                        .publish(Kind::Execution, "key", &retry, &Clock)
                        .unwrap()
                        .0
                );
                assert_eq!(store.get(Kind::Execution, "key").unwrap(), retry);
                continue;
            }
            if row == 3 || row == 4 {
                fail.set(Some((Call::Begin, true)));
            }
            let begun = store.publish(Kind::Execution, "key", &e, &Clock);
            if row == 3 || row == 4 {
                assert!(begun.is_err());
                assert_eq!(
                    state::field_state(&store, "key").unwrap(),
                    FieldState::DispatchUnknown
                );
                assert!(
                    !store.publish(Kind::Execution, "key", &e, &Clock).unwrap().0,
                    "retry must not dispatch"
                );
            } else {
                assert!(begun.unwrap().0);
            }
            if row == 5 {
                fail.set(Some((Call::Outcome, false)));
            }
            if row == 6 || (row == 7 && settle) {
                fail.set(Some((Call::Outcome, true)));
            }
            let out = outcome(settle);
            let answer = store.publish(Kind::Outcome, "key", &out, &Clock);
            if [5, 6].contains(&row) || (row == 7 && settle) {
                assert!(answer.is_err());
            } else {
                assert!(answer.unwrap().0);
            }
            if row == 5 {
                assert_eq!(store.get(Kind::Outcome, "key").unwrap(), Js::Null);
                assert_eq!(
                    state::field_state(&store, "key").unwrap(),
                    FieldState::DispatchUnknown
                );
                assert!(!store.publish(Kind::Execution, "key", &e, &Clock).unwrap().0);
                assert!(store.publish(Kind::Outcome, "key", &out, &Clock).unwrap().0);
            }
            if row == 7 && settle {
                assert_eq!(
                    state::field_state(&store, "key").unwrap(),
                    FieldState::SettledPersisted
                );
                assert!(!store.publish(Kind::Outcome, "key", &out, &Clock).unwrap().0);
            } else {
                assert_eq!(
                    state::field_state(&store, "key").unwrap(),
                    FieldState::ReconciliationRequired
                );
                if row == 2 {
                    fail.set(Some((Call::Reconciliation, false)));
                }
                if row == 7 {
                    fail.set(Some((Call::Reconciliation, true)));
                }
                let rec = reconciliation("- Before".into());
                let answer = store.publish(Kind::Reconciliation, "key", &rec, &Clock);
                if row == 2 {
                    assert!(answer.is_err());
                    assert_eq!(store.get(Kind::Reconciliation, "key").unwrap(), Js::Null);
                    assert_eq!(
                        state::field_state(&store, "key").unwrap(),
                        FieldState::ReconciliationRequired
                    );
                    let reread = reconciliation("- Sent".into());
                    assert!(
                        store
                            .publish(Kind::Reconciliation, "key", &reread, &Clock)
                            .unwrap()
                            .0
                    );
                    assert_eq!(store.get(Kind::Reconciliation, "key").unwrap(), reread);
                } else if row == 7 {
                    assert!(answer.is_err());
                } else {
                    assert!(answer.unwrap().0);
                }
                assert!(
                    !store
                        .publish(Kind::Reconciliation, "key", &rec, &Clock)
                        .unwrap()
                        .0
                );
            }
            assert!(
                f.ledger
                    .open_on(&store.attempt_target(&e).unwrap())
                    .unwrap()
                    .is_empty()
            );
            assert!(
                store
                    .publish(Kind::Execution, "next", &e, &Clock)
                    .unwrap()
                    .0,
                "closed fence must be free row={row}"
            );
            println!("CRASH_WALK row={row} settle={settle} passed");
        }
    }
}
#[test]
fn body_only_through_digest() {
    let f = Fixture::new("body-digest");
    let store = f.store();
    let bytes = codec::bytes(&obj(&[
        ("kind", "execution".into()),
        ("id", "orphan".into()),
        ("record", obj(&[("marker", "invented orphan".into())])),
    ]))
    .unwrap();
    let stem = sha("execution\norphan");
    fs::write(
        f.root.join("execution").join(format!("{stem}.json")),
        &bytes,
    )
    .unwrap();
    fs::write(
        f.root.join("execution").join(format!(
            "{stem}.{}.json",
            sha(std::str::from_utf8(&bytes).unwrap())
        )),
        bytes,
    )
    .unwrap();
    assert_eq!(store.get(Kind::Execution, "orphan").unwrap(), Js::Null);
    let e = f.execution(&store);
    store.publish(Kind::Execution, "key", &e, &Clock).unwrap();
    let a = f.ledger.attempt("key").unwrap().unwrap();
    let path =
        f.root
            .join("execution")
            .join(format!("{}.{}.json", sha("execution\nkey"), a.execution));
    fs::write(path, b"tampered").unwrap();
    assert_eq!(
        store.get(Kind::Execution, "key").unwrap_err().message,
        "workflow_attempt_body_unavailable"
    );
}
#[test]
fn no_text_in_store() {
    let f = Fixture::new("no-text");
    let store = f.store();
    let mut e = f.execution(&store);
    let before = "INVENTED_PRIVATE_BEFORE_716f1";
    let after = "INVENTED_PRIVATE_AFTER_82e61";
    let receipt = "INVENTED_PRIVATE_RECEIPT_9bc21";
    let review_text = "INVENTED_PRIVATE_REVIEW_0d891";
    let review = obj(&[
        ("text", review_text.into()),
        (
            "patch",
            Js::Arr(vec![obj(&[
                ("field", "progress".into()),
                ("target", target(&scope("12"))),
                ("before", before.into()),
                ("after", after.into()),
            ])]),
        ),
    ]);
    store
        .publish(Kind::Review, "private-review", &review, &Clock)
        .unwrap();
    e.set("reviewDigest", "private-review".into());
    store.publish(Kind::Execution, "key", &e, &Clock).unwrap();
    let mut out = outcome(false);
    out.set("receipt", obj(&[("inventedNote", receipt.into())]));
    store.publish(Kind::Outcome, "key", &out, &Clock).unwrap();
    store
        .publish(
            Kind::Reconciliation,
            "key",
            &reconciliation(before.into()),
            &Clock,
        )
        .unwrap();
    assert_eq!(
        state::field_state(&store, "key").unwrap(),
        FieldState::ReconciledNotPersisted
    );
    let path = f.owner.join("store.ledger/academy.sqlite");
    let ctx = ont::Context::new(path.clone(), Clock.epoch_ms() / 1000);
    ont::backup::backup(&ctx, &ont::backup::BackupRequest::default()).unwrap();
    fn files(root: &std::path::Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(root).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                paths.extend(files(&p));
            } else {
                paths.push(p);
            }
        }
        paths
    }
    let paths = files(path.parent().unwrap());
    assert!(
        paths
            .iter()
            .any(|p| p.file_name().unwrap() == "edits.jsonl")
    );
    assert!(
        paths
            .iter()
            .any(|p| p.parent().unwrap().file_name().unwrap() == "backup")
    );
    for path in paths {
        let bytes = fs::read(&path).unwrap();
        for marker in [before, after, receipt, review_text] {
            assert!(
                !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
                "body text leaked to {}",
                path.display()
            );
        }
    }
}
