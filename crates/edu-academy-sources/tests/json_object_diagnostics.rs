//! One strict, read-only original/candidate comparison. Authored only by F;
//! Root owns execution with the explicit immutable diagnostic reference root.
use edu_academy_sources::data::*;
use std::{ffi::OsStr, fs, path::PathBuf};

#[test]
fn json_object_key_diagnostics_match_all_20_pinned_original_cases() {
    const ROOT: &str =
        "/home/palantirkc/work/ws-replan/preparation/slices/S11/json-object-reference-v1";
    const IDS: [&str; 20] = [
        "bare-first-key",
        "different-bare-first-key",
        "numeric-first-key",
        "single-quoted-first-key",
        "space-before-invalid-key",
        "bare-key-after-comma",
        "trailing-comma",
        "space-trailing-comma",
        "object-first-key-eof",
        "object-first-key-space-eof",
        "object-after-comma-eof",
        "nested-first-key",
        "array-object-first-key",
        "empty-object",
        "spaced-empty-object",
        "one-member",
        "duplicate-member",
        "nested-valid",
        "key-looking-string",
        "array-control",
    ];
    let selected = std::env::var_os("S11_JSON_OBJECT_REFERENCE_ROOT")
        .expect("Root must explicitly supply S11_JSON_OBJECT_REFERENCE_ROOT; no fallback or skip");
    let root = PathBuf::from(selected);
    assert!(root.is_absolute());
    assert_eq!(
        root.as_os_str(),
        OsStr::new(ROOT),
        "exact canonical root only"
    );
    assert!(
        fs::symlink_metadata(&root)
            .expect("reference root required")
            .file_type()
            .is_dir()
    );
    assert_eq!(fs::canonicalize(&root).expect("canonical root"), root);
    let path = root.join("ORIGINAL-RESULTS.json");
    assert!(
        fs::symlink_metadata(&path)
            .expect("immutable original results required")
            .file_type()
            .is_file()
    );
    let bytes = fs::read(&path).expect("read original reference only");
    assert_eq!(
        hash(&bytes),
        "a2bde719454cefa73e432c10724ce3d12227379ec738a2040a19c81a289cc06f",
        "exact original capture bytes; no repinning"
    );
    let reference =
        parse(std::str::from_utf8(&bytes).expect("reference UTF-8")).expect("reference JSON");
    assert_eq!(reference.get("bunVersion"), &Js::from("1.4.3"));
    assert_eq!(reference.get("originalOnly"), &Js::Bool(true));
    assert_eq!(reference.get("productImports"), &Js::Num(0.0));
    assert_eq!(reference.get("serviceRequests"), &Js::Num(0.0));
    let cases = reference.get("cases");
    assert!(array(cases));
    assert_eq!(
        cases.array().len(),
        IDS.len(),
        "all twenty ordered cases required"
    );
    let mut mismatches = vec![];
    for (case, id) in cases.array().iter().zip(IDS) {
        assert_eq!(case.get("id"), &Js::from(id), "exact ordered IDs");
        assert!(case.get("input").string());
        let input =
            String::from_utf16(case.get("input").units()).expect("captured input is scalar text");
        assert_eq!(
            case.get("inputSha256"),
            &Js::from(hash(input.as_bytes())),
            "input SHA256: {id}"
        );
        let actual = match parse(&input) {
            Ok(value) => obj(&[("outcome", "returned".into()), ("value", value)]),
            Err(error) => obj(&[
                ("outcome", "threw".into()),
                ("name", error.name.into()),
                ("message", error.message.into()),
            ]),
        };
        // Preserve ordered values and exact error name/message. Compare every case
        // before failing; no case-specific normalization or diagnostic exceptions.
        if &actual != case.get("observed") {
            mismatches.push((id, actual, case.get("observed").clone()));
        }
    }
    assert!(
        mismatches.is_empty(),
        "all20 original/candidate comparison (actual, original): {mismatches:?}"
    );
}
