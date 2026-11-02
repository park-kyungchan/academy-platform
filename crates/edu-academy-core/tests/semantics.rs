//! Literal source-derived checks. Lead runs these after obtaining the build slot.
use edu_academy_core::{js::*, knowledge, templates};
#[test]
fn three_encoders_preserve_distinct_bytes_and_access_counts() {
    let object = obj(&[("b", Js::Num(1.0)), ("a", Js::Num(2.0))]);
    assert_eq!(
        encode(&object, Family::Learning, &mut vec![])
            .unwrap()
            .unwrap(),
        "{\"a\":2,\"b\":1}"
    );
    assert_eq!(
        encode(&object, Family::Cism, &mut vec![]).unwrap().unwrap(),
        "{\"b\":1,\"a\":2}"
    );
    let omitted = obj(&[("a", Js::Undefined), ("b", Js::Null)]);
    assert_eq!(
        encode(&omitted, Family::Admission, &mut vec![])
            .unwrap()
            .unwrap(),
        "{\"b\":null}"
    );
    assert_eq!(
        encode(&omitted, Family::Learning, &mut vec![])
            .unwrap_err()
            .message,
        "only finite JSON data is allowed"
    );
    let a = Js::Accessor(vec![(text("a"), Js::Num(1.0))]);
    let mut events = vec![];
    assert_eq!(
        encode(&a, Family::Admission, &mut events).unwrap().unwrap(),
        "{\"a\":1}"
    );
    assert_eq!(events.len(), 2);
    assert_eq!(
        digest(&a, Family::Admission).unwrap(),
        "015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862"
    );
}
#[test]
fn utf16_order_escaping_and_binary64_are_not_normalized() {
    let o = Js::Obj(vec![
        (vec![0xe000], Js::Num(3.0)),
        (vec![0xd83d, 0xde00], Js::Num(2.0)),
    ]);
    assert_eq!(
        encode(&o, Family::Learning, &mut vec![]).unwrap().unwrap(),
        "{\"😀\":2,\"\u{e000}\":3}"
    );
    assert_eq!(quote(&[0xd800, 0x61, 0xdfff]), "\"\\ud800a\\udfff\"");
    let n = Js::Arr(vec![
        Js::Num(-0.0),
        Js::Num(1e-6),
        Js::Num(1e-7),
        Js::Num(1e20),
        Js::Num(1e21),
    ]);
    assert_eq!(
        stringify(&n).unwrap(),
        "[0,0.000001,1e-7,100000000000000000000,1e+21]"
    );
    assert_ne!(
        digest(&"é".into(), Family::Learning).unwrap(),
        digest(&"e\u{301}".into(), Family::Learning).unwrap()
    );
}
#[test]
fn holes_and_depth_have_family_specific_source_or_adapter_results() {
    assert_eq!(
        encode(&Js::Sparse(2), Family::Admission, &mut vec![])
            .unwrap()
            .unwrap(),
        "[,]"
    );
    assert_eq!(
        encode(&Js::Sparse(2), Family::Cism, &mut vec![])
            .unwrap()
            .unwrap(),
        "[null,null]"
    );
    assert_eq!(
        encode(&Js::Sparse(2), Family::Learning, &mut vec![])
            .unwrap_err()
            .message,
        "sparse or decorated arrays are not allowed"
    );
    let mut d = Js::Null;
    for _ in 0..64 {
        d = Js::Arr(vec![d]);
    }
    assert!(encode(&d, Family::Learning, &mut vec![]).is_ok());
    d = Js::Arr(vec![d]);
    assert_eq!(
        encode(&d, Family::Learning, &mut vec![])
            .unwrap_err()
            .message,
        "invalid_json_depth"
    );
    assert_eq!(
        encode(&Js::Cycle, Family::Admission, &mut vec![])
            .unwrap_err()
            .name,
        "AdapterAdmission"
    );
}
#[test]
fn missing_unknown_blank_zero_and_not_performed_are_distinct() {
    assert_eq!(
        knowledge::assert_knowledge(&Js::Undefined)
            .unwrap_err()
            .message,
        "missing knowledge state"
    );
    for v in [Js::Num(0.0), "".into(), "not_performed".into()] {
        let k = knowledge::known(&v, &strings(&["teacher:synthetic"])).unwrap();
        assert_eq!(k.get("value"), &v);
    }
    let u = knowledge::unknown(&"No source observation".into()).unwrap();
    assert!(u.get("state").is("unknown"));
    assert!(!u.has("value"));
    assert!(knowledge::known(&Js::Null, &strings(&["source:synthetic"])).is_err());
}
#[test]
fn learning_calendar_admission_is_separate_from_accepted_cism_leaf() {
    for s in ["0000-01-01", "0099-12-31", "2000-02-29", "9999-12-31"] {
        assert!(knowledge::calendar_date(&s.into()).is_ok());
    }
    for s in ["1900-02-29", "2100-02-29"] {
        assert!(knowledge::calendar_date(&s.into()).is_err());
    }
    assert!(edu_academy_core::leaf::cism_week("0099-12-31").is_err());
}
#[test]
fn immutable_copy_preserves_source_clone_order_and_zero() {
    let k = knowledge::known(
        &obj(&[("z", Js::Num(-0.0)), ("a", Js::Bool(false))]),
        &strings(&["teacher:synthetic"]),
    )
    .unwrap();
    assert_eq!(stringify(k.get("value")).unwrap(), "{\"a\":false,\"z\":0}");
    if let Js::Num(n) = k.get("value").get("z") {
        assert!(!n.is_sign_negative());
    } else {
        panic!("number expected");
    }
}
#[test]
fn template_empty_is_literal_and_rendering_grants_no_approval() {
    let input = obj(&[
        ("lessonDate", "2040-01-16".into()),
        ("part", Js::Num(1.0)),
        ("progress", Js::Arr(vec![])),
        (
            "homework",
            obj(&[
                ("dueDate", "2040-01-20".into()),
                ("tasks", Js::Arr(vec![])),
                ("qualification", "verified".into()),
            ]),
        ),
        ("memo", Js::Arr(vec![])),
    ]);
    let r = templates::render(&input).unwrap();
    for f in ["progress", "homework", "memo"] {
        assert!(r.get(f).is(""));
    }
    assert_eq!(r.get("evidenceRefs").array().len(), 0);
    assert!(r.get("templateRevision").is("3"));
    assert!(r.get("approval").is("not_granted_by_rendering"));
}
