use edu_academy_core::{
    js::{Js, sha},
    templates::parent_internal,
};

#[test]
fn parent_guard_matches_pinned_typescript_admission_vectors() {
    let raw = include_str!("fixtures/parent-guard-vectors-v2.json");
    // Pin the frozen oracle decisions independently of retired TypeScript paths.
    assert_eq!(
        sha(raw),
        "5018e7d584b7806058f46a7412d70ea465774d73297419ed7c4baa38d2b96374"
    );
    let fixture: serde_json::Value = serde_json::from_str(raw).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        assert_eq!(
            parent_internal(&Js::from(text)),
            case["excluded"].as_bool().unwrap(),
            "{text}"
        );
    }
}

#[test]
fn both_parent_notation_holds_route_to_teacher_interview() {
    let actions: serde_json::Value =
        serde_json::from_str(include_str!("../src/hold-actions.json")).unwrap();
    for code in [
        "book_parent_notation_unverified_ask_teacher",
        "book_parent_notation_ambiguous_ask_teacher",
    ] {
        let action = actions
            .as_array()
            .unwrap()
            .iter()
            .find(|a| edu_academy_core::js::test(a["pattern"].as_str().unwrap(), &Js::from(code)))
            .unwrap();
        assert_eq!(action["action"], "teacher_interview");
    }
}
