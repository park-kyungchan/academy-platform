//! Regression expectations are frozen independently from the actual original TS
//! class. Only explicit fictional source/policy ports participate in these tests.
use edu_academy_core::{
    js::{Fault, Js, Result, obj},
    workflow::{
        self, Runtime,
        approval::Approval,
        codec, read_failure, review, service,
        store::{Kind, Store, mkdir},
    },
};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

type Calls = Rc<RefCell<Vec<Js>>>;
struct Clock(String);
impl Runtime for Clock {
    fn now(&self) -> String {
        self.0.clone()
    }
    fn epoch_ms(&self) -> u64 {
        0
    }
    fn uuid(&self) -> Result<String> {
        panic!("held review must not publish")
    }
}
struct Source {
    read: Js,
    calls: Calls,
}
impl service::Source for Source {
    fn read(&mut self, scope: &Js, request: &Js, subject: &Js) -> Result<Js> {
        self.calls.borrow_mut().push(obj(&[
            ("port", "read".into()),
            ("scope", scope.clone()),
            ("requestRef", request.clone()),
            ("subject", workflow::nullable(subject)),
        ]));
        if self.read.has("throw") {
            return Err(Fault::error(self.read.get("throw").code_units()));
        }
        Ok(self.read.clone())
    }
}
struct Backend;
impl service::Backend for Backend {
    fn configured(&self) -> bool {
        true
    }
    fn save(&mut self, _: &Js, _: &Js, _: &mut dyn Approval, _: &Js) -> Result<Js> {
        panic!("review must never save/consume/replay")
    }
}
struct Policy {
    value: Js,
    calls: Calls,
}
impl Policy {
    fn call(&self, name: &str, scope: &Js, value: &Js, entries: &Js) {
        self.calls.borrow_mut().push(obj(&[
            ("port", name.into()),
            ("scope", scope.clone()),
            ("value", value.clone()),
            ("entries", workflow::nullable(entries)),
        ]));
    }
}
impl review::Policy for Policy {
    fn parent_pins(&self) -> Js {
        obj(&[
            ("sourceRef", "s10:fake-owner-parent-policy".into()),
            ("policySha256", "3".repeat(64).into()),
            ("safeguardSourceSha256", "4".repeat(64).into()),
        ])
    }
    fn learning_pins(&self) -> Js {
        obj(&[
            ("bundleSha256", "5".repeat(64).into()),
            ("configSha256", "6".repeat(64).into()),
            ("catalogSha256", "7".repeat(64).into()),
        ])
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
    fn learning_review(&self, scope: &Js, value: &Js, entries: &Js) -> Vec<Js> {
        self.call("learning_review", scope, value, entries);
        self.value.get("blockers").array().to_vec()
    }
    fn learning_suggest(&self, scope: &Js, value: &Js, entries: &Js) -> Js {
        assert!(
            matches!(entries, Js::Undefined | Js::Arr(_)),
            "pass the original optional entries to suggest"
        );
        if self.value.has("suggestion") {
            self.call("learning_suggest", scope, value, entries);
        }
        workflow::nullable(self.value.get("suggestion"))
    }
}
fn cases() -> Js {
    codec::parse(include_bytes!(
        "../../../migration/workflow-store-review-fixes-v2/CASES.json"
    ))
    .unwrap()
}
fn expected() -> Js {
    codec::parse(include_bytes!(
        "../../../migration/workflow-store-review-fixes-v2/EXPECTED.json"
    ))
    .unwrap()
}
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, p: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(p).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut out = vec![];
    visit(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn service_matches_independent_original_results_and_owner_callback_values() {
    let cases = cases();
    let expected = expected();
    assert_eq!(cases.array().len(), expected.array().len());
    let root =
        PathBuf::from(std::env::var("S10_TEST_ROOT").expect("explicit private evidence root"))
            .join(format!("review-fixes-{}", std::process::id()));
    mkdir(&root).unwrap();
    for (case, expected) in cases.array().iter().zip(expected.array()) {
        let dir = root.join(case.get("id").code_units());
        mkdir(&dir).unwrap();
        let store = Store::open(&dir.join("store"), &dir).unwrap();
        let clock = Clock(case.get("now").code_units());
        let calls: Calls = Rc::new(RefCell::new(vec![]));
        let policy = Policy {
            value: case.get("policy").clone(),
            calls: calls.clone(),
        };
        let mut source = Source {
            read: case.get("read").clone(),
            calls: calls.clone(),
        };
        let mut backend = Backend;
        if !case.get("held").absent() {
            // Pre-existing observation, independent of any review approval.
            struct SeedClock;
            impl Runtime for SeedClock {
                fn now(&self) -> String {
                    "2040-01-04T12:00:00.000Z".into()
                }
                fn epoch_ms(&self) -> u64 {
                    0
                }
                fn uuid(&self) -> Result<String> {
                    Ok("00000000-0000-4000-8000-000000000001".into())
                }
            }
            store
                .publish(
                    Kind::Observation,
                    "dailyrecord-observation:s10-regression",
                    &obj(&[
                        (
                            "scope",
                            if case.has("storedScope") {
                                case.get("storedScope").clone()
                            } else {
                                case.get("input").get("context").get("scope").clone()
                            },
                        ),
                        (
                            "requestRef",
                            if case.has("storedRequest") {
                                case.get("storedRequest").clone()
                            } else {
                                case.get("input")
                                    .get("context")
                                    .get("currentRequestRef")
                                    .clone()
                            },
                        ),
                        ("observation", case.get("held").clone()),
                        (
                            "subject",
                            obj(&[("displayName", "Invented S10 Student".into())]),
                        ),
                    ]),
                    &SeedClock,
                )
                .unwrap();
        }
        let before = snapshot(&dir);
        let mut svc = service::Service {
            store: &store,
            source: &mut source,
            policy: &policy,
            backend: &mut backend,
            runtime: &clock,
        };
        let result = svc.review(case.get("input"), true);
        match result {
            Ok(result) => {
                assert!(
                    result.get("status").is("held"),
                    "{}",
                    case.get("id").code_units()
                );
                assert!(
                    workflow::same(&result, expected.get("result")).unwrap(),
                    "{}: {:?}",
                    case.get("id").code_units(),
                    result
                );
            }
            Err(error) => assert_eq!(
                error.message,
                expected.get("error").code_units(),
                "{}",
                case.get("id").code_units()
            ),
        }
        assert_eq!(snapshot(&dir), before, "held review changed durable bytes");
        assert!(
            workflow::same(&Js::Arr(calls.borrow().clone()), expected.get("calls")).unwrap(),
            "{} callback values",
            case.get("id").code_units()
        );
    }
}

#[test]
fn public_observation_review_api_accepts_absent_held_observation() {
    let c = cases().array()[0].clone();
    let e = expected().array()[0].clone();
    let policy = Policy {
        value: c.get("policy").clone(),
        calls: Rc::new(RefCell::new(vec![])),
    };
    for absent in [Js::Null, Js::Undefined] {
        let result = review::review(
            c.get("input"),
            &absent,
            &Js::Null,
            &policy,
            &Clock(c.get("now").code_units()),
        )
        .unwrap();
        assert!(workflow::same(&result, e.get("result")).unwrap());
    }
}

#[test]
fn typed_failure_classification_uses_original_catalog_and_safe_fallback() {
    let known = read_failure::unavailable(&Fault::error("native_dailyrecord_scope_refused"));
    assert!(
        known
            .get("reasonCode")
            .is("native_dailyrecord_scope_refused")
    );
    assert!(known.get("failure").get("category").is("policy_denial"));
    let private =
        read_failure::unavailable(&Fault::type_error("fictional_private_exception_payload"));
    assert!(
        private
            .get("reasonCode")
            .is("dailyrecord_source_unqualified_no_automatic_retry")
    );
    assert!(private.get("failure").get("errorKind").is("TypeError"));
    assert!(
        !workflow::canonical(&private)
            .unwrap()
            .contains("fictional_private_exception_payload")
    );
    let malformed = read_failure::unavailable(&Fault {
        name: "SyntaxError".into(),
        message: "fictional bad JSON".into(),
        issues: None,
        ..Fault::error("")
    });
    assert!(malformed.get("reasonCode").is("source_response_malformed"));
    assert!(
        malformed
            .get("failure")
            .get("category")
            .is("malformed_response")
    );
}
