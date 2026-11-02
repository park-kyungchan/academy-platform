//! Public API behavior, independently asserted without reference/corpus access.
use edu_academy_core::{
    admission::{Admission, Owner, OwnerRecord, SharedRequest},
    cism,
    instant::{self, Grammar},
    js::*,
    learning, schema,
    wire::{Node, Wire},
};
use std::{cell::RefCell, rc::Rc};
fn request() -> Js {
    obj(&[
        ("runtime", "native-test".into()),
        ("requestRef", "request:1".into()),
        ("requestTextSha256", "a".repeat(64).into()),
        ("currentUserEventRef", "event:1".into()),
    ])
}
fn launch() -> Js {
    let mut v = obj(&[(
        "schemaVersion",
        "academy.dailyrecord-save-admission/1".into(),
    )]);
    for (k, x) in request().props() {
        v.set(&String::from_utf16_lossy(k), x.clone());
    }
    v.set(
        "admitted",
        Js::Arr(vec![obj(&[
            ("studentId", "opaque:1".into()),
            ("lessonDate", "2040-01-16".into()),
            ("field", "previousHomeworkAssessment".into()),
            ("effectRecordKey", "previous-record".into()),
            ("after", "0".into()),
        ])]),
    );
    v
}
fn action() -> Js {
    obj(&[
        ("attemptId", "attempt:1".into()),
        (
            "target",
            obj(&[
                ("studentKey", "opaque:1".into()),
                ("date", "2040-01-16".into()),
                ("recordKey", "current-record".into()),
            ]),
        ),
        ("field", "previousHomeworkAssessment".into()),
        ("effectRecordKey", "previous-record".into()),
        ("before", "-1".into()),
        ("after", "0".into()),
    ])
}
struct Local {
    current: SharedRequest,
    reads: usize,
    consumes: usize,
    result: bool,
    reject_read: bool,
    reject_consume: bool,
}
impl Local {
    fn new() -> Self {
        Self {
            current: Rc::new(RefCell::new(request())),
            reads: 0,
            consumes: 0,
            result: true,
            reject_read: false,
            reject_consume: false,
        }
    }
}
impl Owner for Local {
    fn read_current_request(&mut self) -> Result<SharedRequest> {
        self.reads += 1;
        if self.reject_read {
            Err(Fault::error("owner read rejected"))
        } else {
            Ok(self.current.clone())
        }
    }
    fn consume_once(&mut self, _key: &str, record: &OwnerRecord) -> Result<bool> {
        self.consumes += 1;
        assert!(record.is_shallow_frozen());
        assert!(Rc::ptr_eq(record.request(), &self.current));
        assert!(record.tuple().get("effectRecordKey").is("previous-record"));
        record.request().borrow_mut().set("probe", true.into());
        assert!(self.current.borrow().get("probe").truthy());
        if self.reject_consume {
            Err(Fault::error("owner consume rejected"))
        } else {
            Ok(self.result)
        }
    }
}
#[test]
fn admission_launch_alias_and_initial_tuple_capture_are_separate() {
    let shared = Rc::new(RefCell::new(launch()));
    let mut approval = Admission::new(shared.clone()).unwrap();
    let initial_digest = approval.admission_digest().to_owned();
    let mut owner = Local::new();
    shared.borrow_mut().set("requestRef", "request:2".into());
    assert!(
        approval
            .consume_approval(&action(), &mut owner)
            .unwrap()
            .get("status")
            .is("rejected")
    );
    assert_eq!((owner.reads, owner.consumes), (1, 0));
    owner
        .current
        .borrow_mut()
        .set("requestRef", "request:2".into());
    let mut entry = shared.borrow().get("admitted").array()[0].clone();
    entry.set("after", "changed".into());
    shared.borrow_mut().set("admitted", Js::Arr(vec![entry]));
    let mut changed = action();
    changed.set("after", "changed".into());
    assert!(
        approval
            .consume_approval(&changed, &mut owner)
            .unwrap()
            .get("status")
            .is("rejected")
    );
    assert_eq!(owner.reads, 1);
    let verified = approval.consume_approval(&action(), &mut owner).unwrap();
    assert!(verified.get("status").is("verified"));
    assert_eq!(
        verified.get("actionDigest"),
        &digest(&action(), Family::Admission).unwrap().into()
    );
    assert_eq!(approval.admission_digest(), initial_digest);
    assert!(
        approval
            .consume_approval(&action(), &mut owner)
            .unwrap()
            .get("status")
            .is("rejected")
    );
    assert_eq!((owner.reads, owner.consumes), (2, 1));
}
#[test]
fn owner_false_consumes_local_key_but_callback_errors_do_not() {
    let mut a = Admission::new(Rc::new(RefCell::new(launch()))).unwrap();
    let mut owner = Local::new();
    owner.reject_read = true;
    assert!(a.consume_approval(&action(), &mut owner).is_err());
    assert_eq!(owner.consumes, 0);
    owner.reject_read = false;
    owner.reject_consume = true;
    assert!(a.consume_approval(&action(), &mut owner).is_err());
    owner.reject_consume = false;
    owner.result = false;
    assert!(
        a.consume_approval(&action(), &mut owner)
            .unwrap()
            .get("status")
            .is("rejected")
    );
    let calls = (owner.reads, owner.consumes);
    owner.result = true;
    assert!(
        a.consume_approval(&action(), &mut owner)
            .unwrap()
            .get("status")
            .is("rejected")
    );
    assert_eq!(calls, (owner.reads, owner.consumes));
}
#[test]
fn constructor_duplicate_is_distinct_from_schema_acceptance() {
    let mut l = launch();
    let entry = l.get("admitted").array()[0].clone();
    l.set("admitted", Js::Arr(vec![entry.clone(), entry]));
    assert!(schema::validate("dailyRecordSaveAdmissionSchema", &l).is_ok());
    let e = Admission::new(Rc::new(RefCell::new(l))).err().unwrap();
    assert_eq!(e.message, "dailyrecord_save_duplicate_admitted_action");
}
#[test]
fn immutable_returns_detach_nested_children_and_refs() {
    let mut input = obj(&[
        ("a", obj(&[("z", Js::Num(-0.0))])),
        ("b", obj(&[("z", Js::Num(-0.0))])),
    ]);
    let mut refs = strings(&["teacher:original"]);
    let frozen = learning::known_frozen(&input, &refs).unwrap();
    input.set("a", Js::Null);
    refs = Js::Arr(vec![]);
    assert!(refs.array().is_empty());
    assert!(frozen.all_frozen());
    assert_eq!(frozen.value().get("evidenceRefs").array().len(), 1);
    let v = frozen.value().get("value");
    assert!(!std::ptr::eq(v.get("a"), v.get("b")));
    assert_eq!(stringify(v).unwrap(), "{\"a\":{\"z\":0},\"b\":{\"z\":0}}");
    let mut detached = frozen.detached_copy();
    detached.set("value", Js::Null);
    assert!(!frozen.value().get("value").absent());
    assert_eq!(
        frozen.try_define("probe", true.into()).unwrap_err().name,
        "TypeError"
    );
}
#[test]
fn different_instant_grammars_and_epoch_arithmetic() {
    let parse = |s: &str, g| instant::epoch_ms(&s.into(), g);
    assert_eq!(parse("1970-01-01T00:00:00Z", Grammar::Learning), Some(0));
    assert_eq!(
        parse("2040-01-16T18:00:00.12+09:00", Grammar::Learning),
        parse("2040-01-16T03:30:00.120-05:30", Grammar::Learning)
    );
    assert_eq!(parse("2040-01-16T09:00Z", Grammar::Learning), None);
    assert!(parse("2040-01-16T09:00Z", Grammar::DailyRecord).is_some());
    assert_eq!(
        parse("2040-01-16T09:00:00.123456789Z", Grammar::DailyRecord),
        parse("2040-01-16T09:00:00.123Z", Grammar::DailyRecord)
    );
    assert_eq!(
        parse("0000-01-01T00:00:00Z", Grammar::DailyRecord),
        Some(-62167219200000)
    );
    assert!(parse("9999-12-31T23:59:59-23:59", Grammar::DailyRecord).is_some());
    assert!(parse("2000-02-29T09:00Z", Grammar::DailyRecord).is_some());
    assert_eq!(parse("2100-02-29T09:00Z", Grammar::DailyRecord), None);
    assert_eq!(parse("2040-01-16T09:00.1Z", Grammar::DailyRecord), None);
}
#[test]
fn nonlearning_depth1000_is_carried_and_refusals_remain_family_specific() {
    let mut nodes = vec![Node::Null];
    for i in 0..1000 {
        nodes.push(Node::Array { items: vec![i] });
    }
    let value = Wire { root: 1000, nodes }.decode().unwrap();
    let expected = format!("{}null{}", "[".repeat(1000), "]".repeat(1000));
    for family in [Family::Admission, Family::Cism] {
        assert_eq!(
            encode(&value, family, &mut vec![]).unwrap().unwrap(),
            expected
        );
    }
    assert_eq!(
        encode(&value, Family::Learning, &mut vec![])
            .unwrap_err()
            .message,
        "invalid_json_depth"
    );
    assert_eq!(
        encode(&Js::Cycle, Family::Learning, &mut vec![])
            .unwrap_err()
            .name,
        "Error"
    );
    assert_eq!(
        encode(&Js::Cycle, Family::Admission, &mut vec![])
            .unwrap_err()
            .name,
        "RangeError"
    );
    assert_eq!(
        encode(&Js::Cycle, Family::Cism, &mut vec![])
            .unwrap_err()
            .name,
        "TypeError"
    );
}
#[test]
fn schemas_preserve_multi_issue_order_and_parse_defaults() {
    let e = schema::validate(
        "dailyRecordSaveRequestEvidenceSchema",
        &obj(&[("runtime", true.into()), ("extra", true.into())]),
    )
    .unwrap_err();
    let issues = e.issues.unwrap();
    assert_eq!(issues[0].get("path"), &strings(&["runtime"]));
    assert_eq!(
        issues.last().unwrap().get("code"),
        &"unrecognized_keys".into()
    );
    assert!(
        e.message
            .starts_with("[\n  {\n    \"expected\": \"string\"")
    );
    let valid = schema::validate("dailyRecordSaveAdmissionSchema", &launch()).unwrap();
    assert!(valid.get("runtime").is("native-test"));
    assert!(!valid.has("extra"));
    let mut l = launch();
    l.set("runtime", " ".into());
    assert!(schema::validate("dailyRecordSaveAdmissionSchema", &l).is_ok());
    assert!(schema::validate("dailyRecordIdSchema", &" ".into()).is_err());
}
#[test]
fn cism_membership_preserves_source_order_unknown_kind_and_form_bytes() {
    let f = obj(&[
        ("fieldRef", "opaque".into()),
        ("kind", "runtime-string".into()),
        (
            "source",
            obj(&[
                ("fieldName", "x".into()),
                ("studentKey", "11".into()),
                ("membershipKey", "22".into()),
                ("groupKey", "33".into()),
            ]),
        ),
    ]);
    let r = obj(&[
        ("date", "2040-01-16".into()),
        ("teacherKey", "teacher:1".into()),
        ("fields", Js::Arr(vec![f.clone()])),
    ]);
    let result = cism::write_parameters(&r, &f, &"a + 한글".into()).unwrap();
    assert!(
        result
            .get("wire")
            .code_units()
            .contains("field_value=a+%2B+%ED%95%9C%EA%B8%80")
    );
    let mut changed = f.clone();
    let mut source = f.get("source").props().to_vec();
    source.reverse();
    changed.set("source", Js::Obj(source));
    assert_eq!(
        cism::write_parameters(&r, &changed, &"x".into())
            .unwrap_err()
            .message,
        "cism_fields_unqualified"
    );
    let emoji = "😀".repeat(3000);
    assert!(cism::write_parameters(&r, &f, &emoji.clone().into()).is_ok());
    assert!(cism::write_parameters(&r, &f, &(emoji + "x").into()).is_err());
}
#[test]
fn exact_keys_are_own_enumerable_and_null_has_source_type_error() {
    assert!(exact(&"ab".into(), &["0", "1"]).is_ok());
    assert!(exact(&Js::Inherited(vec![(text("a"), true.into())]), &["a"]).is_ok());
    assert_eq!(exact(&Js::Null, &[]).unwrap_err().name, "TypeError");
}
#[test]
fn iterative_clone_and_drop_do_not_turn_transport_depth_into_source_refusal() {
    let mut value = Js::Null;
    for _ in 0..10000 {
        value = Js::Arr(vec![value]);
    }
    let detached = value.clone();
    assert_eq!(
        encode(&detached, Family::Cism, &mut vec![])
            .unwrap()
            .unwrap()
            .len(),
        20004
    );
    assert_eq!(
        encode(&value, Family::Learning, &mut vec![])
            .unwrap_err()
            .message,
        "invalid_json_depth"
    );
    // This tests native representation safety, not uncharacterized original runtime depth parity.
    drop(value);
    drop(detached);
}
