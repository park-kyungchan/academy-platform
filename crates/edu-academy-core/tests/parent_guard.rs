use edu_academy_core::{
    js::{Js, sha},
    templates::parent_internal,
};
use std::{fs, path::Path};

#[test]
fn parent_guard_matches_pinned_typescript_admission_vectors() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/parent-guard-vectors.json")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for source in fixture["sources"].as_array().unwrap() {
        let bytes = fs::read(root.join(source["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            sha(std::str::from_utf8(&bytes).unwrap()),
            source["sha256"].as_str().unwrap(),
            "TS oracle source changed; regenerate vectors with Bun"
        );
    }
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
