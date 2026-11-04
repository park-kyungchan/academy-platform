use edu_academy_sources::{
    data::*,
    dom,
    owner_root::{Checkpoint, OwnerRoot},
    presentation,
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};
fn private(p: &Path) {
    fs::create_dir(p).unwrap();
    fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
}
fn fixture(name: &str) -> PathBuf {
    let base = std::env::var("S09_NATIVE_FIXTURE_ROOT")
        .expect("explicit disposable fixture parent required");
    let p = PathBuf::from(base).join(format!("{}-{name}", std::process::id()));
    private(&p);
    p
}
fn leaf(root: &Path) {
    private(&root.join("d"));
    fs::write(root.join("d/leaf"), b"immutable").unwrap();
    fs::set_permissions(root.join("d/leaf"), fs::Permissions::from_mode(0o600)).unwrap();
}
#[test]
fn strict_modes_hash_fifo_links_and_replacement() {
    let r = fixture("owner");
    leaf(&r);
    let reader = OwnerRoot::open(&r).unwrap();
    let h = hash(b"immutable");
    assert_eq!(reader.read("d/leaf", &h, 100).unwrap().bytes, b"immutable");
    assert!(reader.read("d/leaf", &"0".repeat(64), 100).is_err());
    assert!(reader.read("d/leaf", &h, 8).is_err());
    fs::set_permissions(r.join("d/leaf"), fs::Permissions::from_mode(0o666)).unwrap();
    assert!(reader.read("d/leaf", &h, 100).is_err());
    fs::set_permissions(r.join("d/leaf"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(r.join("d/leaf"), r.join("d/hard")).unwrap();
    assert!(reader.read("d/leaf", &h, 100).is_err());
    fs::remove_file(r.join("d/hard")).unwrap();
    symlink("leaf", r.join("d/link")).unwrap();
    assert!(reader.read("d/link", &h, 100).is_err());
    rustix::fs::mknodat(
        rustix::fs::CWD,
        r.join("d/fifo"),
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::from_raw_mode(0o600),
        0,
    )
    .unwrap();
    assert!(reader.read("d/fifo", &h, 100).is_err());
    let result = reader.read_observed("d/leaf", &h, 100, |c| {
        if c == Checkpoint::LeafPinned {
            fs::rename(r.join("d/leaf"), r.join("d/old")).unwrap();
            fs::write(r.join("d/leaf"), b"immutable").unwrap();
            fs::set_permissions(r.join("d/leaf"), fs::Permissions::from_mode(0o600)).unwrap();
        }
    });
    assert!(result.is_err());
}
#[test]
fn strict_parent_and_root_replacement_in_place_and_uid() {
    let r = fixture("races");
    leaf(&r);
    let reader = OwnerRoot::open(&r).unwrap();
    let h = hash(b"immutable");
    let result = reader.read_observed("d/leaf", &h, 100, |c| {
        if c == Checkpoint::BytesCaptured {
            fs::write(r.join("d/leaf"), b"mutated!!").unwrap();
        }
    });
    assert_eq!(
        result.err().unwrap().message,
        "owner_root_leaf_changed_during_read"
    );
    let r = fixture("parent");
    leaf(&r);
    let reader = OwnerRoot::open(&r).unwrap();
    let result = reader.read_observed("d/leaf", &h, 100, |c| {
        if c == Checkpoint::LeafPinned {
            fs::rename(r.join("d"), r.join("old-d")).unwrap();
            leaf(&r);
        }
    });
    assert!(result.is_err());
    let r = fixture("root");
    leaf(&r);
    let reader = OwnerRoot::open(&r).unwrap();
    let old = r.with_extension("old");
    fs::rename(&r, &old).unwrap();
    private(&r);
    leaf(&r);
    assert!(reader.read("d/leaf", &h, 100).is_err());
    if rustix::process::geteuid().as_raw() == 0 {
        let r = fixture("uid");
        leaf(&r);
        rustix::fs::chownat(
            rustix::fs::CWD,
            r.join("d/leaf"),
            Some(rustix::process::Uid::from_raw(12345)),
            None,
            rustix::fs::AtFlags::empty(),
        )
        .unwrap();
        assert!(
            OwnerRoot::open(&r)
                .unwrap()
                .read("d/leaf", &h, 100)
                .is_err()
        );
    }
}
#[test]
fn utf16_and_inert_unseen_dom() {
    let source = "😀<table id=repair><tr><td>one<td>two</table><textarea>\r\nA&amp;B\r\n</textarea><select><option disabled>X<option> B  C </select><script id=handler>function onFoPrg(){return '😀}';}</script>";
    let doc = dom::Document::parse(source);
    assert_eq!(dom::value(doc.select("textarea")[0]), "A&B\n");
    assert_eq!(dom::value(doc.select("select")[0]), "B C");
    let e = doc.select("td")[0];
    let range = doc.range(e).unwrap();
    assert_eq!(string(&doc.raw(&range)), "<td>one");
    assert!(range.start > 2);
    assert_eq!(
        string(&dom::source_function("function f(){return '가😀}';}", "f").unwrap()),
        "function f(){return '가😀}';}"
    );
}
#[test]
fn lone_surrogate_json_order_and_opaque_pointer() {
    let v = parse(r#"{"9":1,"2":2,"😀":"\ud800","opaque\udfff":0}"#).unwrap();
    assert_eq!(json(&v), r#"{"2":2,"9":1,"😀":"\ud800","opaque\udfff":0}"#);
    let c = edu_academy_sources::saved::query_input(
        &parse(r#"{"query":{"studentId":"\ud800","kind":"student_summary"}}"#).unwrap(),
    )
    .unwrap();
    assert_eq!(c.get("query").get("studentId").units(), &[0xd800]);
}
#[test]
fn unseen_presentation_does_not_infer_semantics() {
    let v=presentation::inspect("<label for=difficulty>Unknown</label><select id=difficulty><option value='0'>Zero</select><script>var g_examtype_filter='m';var searchConditionDifficulty=null;var searchConditionBehAreaNo=7;</script>","function previewDBExam(x){return 'new';}").unwrap();
    assert!(
        v.get("unknowns")
            .array()
            .iter()
            .any(|s| s.is("question_grid_not_observed"))
    );
    assert!(
        v.get("filterDeclarations")
            .array()
            .iter()
            .any(|r| r.get("valueRaw").is("m"))
    );
}
#[test]
fn strict_root_and_directory_admission() {
    let r = fixture("root-admission");
    leaf(&r);
    assert!(OwnerRoot::open(Path::new("relative")).is_err());
    let alias = r.with_extension("alias");
    symlink(&r, &alias).unwrap();
    assert!(OwnerRoot::open(&alias).is_err());
    fs::set_permissions(&r, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(OwnerRoot::open(&r).is_err());
    fs::set_permissions(&r, fs::Permissions::from_mode(0o700)).unwrap();
    let reader = OwnerRoot::open(&r).unwrap();
    fs::set_permissions(r.join("d"), fs::Permissions::from_mode(0o777)).unwrap();
    assert!(reader.read("d/leaf", &hash(b"immutable"), 100).is_err());
    let r = fixture("ancestor-mode");
    private(&r.join("root"));
    fs::set_permissions(&r, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(OwnerRoot::open(&r.join("root")).is_err());
}
#[test]
fn strict_caps_and_exact_capture_projection() {
    let r = fixture("caps");
    let manifest = r.join("manifest");
    fs::write(&manifest, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();
    let reader = OwnerRoot::open(&r).unwrap();
    assert!(
        reader
            .read("manifest", &"0".repeat(64), 4 * 1024 * 1024)
            .is_err()
    );
    let r = fixture("projection");
    let raw = b"{\"value\":0,\"missingMeaning\":null}";
    fs::write(r.join("data"), raw).unwrap();
    fs::set_permissions(r.join("data"), fs::Permissions::from_mode(0o600)).unwrap();
    let root = OwnerRoot::open(&r).unwrap();
    let capture = root.read("data", &hash(raw), 100).unwrap();
    let association = edu_academy_sources::evidence::OwnerAssociation {
        account: text("fictional-A"),
        instance: text("fictional-instance"),
        existing_owner_evidence: obj(&[("state", "fictional_only_not_owner_proof".into())]),
    };
    let value = edu_academy_sources::evidence::project_json(
        &association,
        &[capture],
        0,
        "/value",
        &"0001".into(),
    )
    .unwrap();
    assert!(value.get("status").is("UNADMITTED"));
    assert_eq!(value.get("recomputedValue"), &Js::Num(0.));
    assert!(value.get("legacyIdentity").is("0001"));
    let mut other = association.clone();
    other.account = text("fictional-B");
    assert_ne!(
        edu_academy_sources::evidence::namespace(&association),
        edu_academy_sources::evidence::namespace(&other)
    );
    let u = edu_academy_sources::evidence::raw_utf16("가😀Z".as_bytes(), 1, 3).unwrap();
    assert_eq!(u.get("raw").units(), &[0xd83d, 0xde00]);
}
#[test]
fn json_parse_large_exponents_and_non_scalar_source_pointer() {
    assert_eq!(json(&parse("[1e999,-0,1e-999]").unwrap()), "[null,0,0]");
    let audit=br#"{"schemaVersion":"academy.audit.normalized/1.0.0","collectedAt":"2040-01-01T00:00:00Z","students":[{"studentId":"\ud800","name":"A","books":[],"sections":[],"DTZT":{"\udfff/~":null}},{"studentId":"2","name":"B","books":[],"sections":[]},{"studentId":"3","name":"C","books":[],"sections":[]},{"studentId":"4","name":"D","books":[],"sections":[]},{"studentId":"5","name":"E","books":[],"sections":[]}]}"#;
    let lesson=br#"{"schemaVersion":"academy.lesson-overlay/1","lessonDate":"2040-01-01","preparedAt":"2040-01-01T00:00:00Z","students":[]}"#;
    let context = edu_academy_sources::saved::SavedContext::from_bytes(audit, lesson).unwrap();
    let q =
        parse(r#"{"kind":"student_records","studentId":"\ud800","facet":"daily_tests"}"#).unwrap();
    let rows = context.records(&q).unwrap();
    let mut expected = text("/students/0/DTZT/");
    expected.push(0xdfff);
    expected.extend(text("~1~0"));
    assert_eq!(
        rows[0].get("source").get("jsonPointer").units(),
        expected.as_slice()
    );
}

#[test]
fn sibling_publication_does_not_replace_the_admitted_directory_or_leaf() {
    let parent = fixture("sibling-publication");
    let root_path = parent.join("owner");
    private(&root_path);
    let bytes = b"unchanged pinned leaf";
    fs::write(root_path.join("data"), bytes).unwrap();
    fs::set_permissions(root_path.join("data"), fs::Permissions::from_mode(0o600)).unwrap();
    let reader = OwnerRoot::open(&root_path).unwrap();
    // Deterministically reproduce other parallel fixtures publishing siblings.
    private(&parent.join("independent-owner"));
    let capture = reader.read("data", &hash(bytes), 100).unwrap();
    assert_eq!(capture.bytes, bytes);
    // Stable inode alone never excuses an admission-mode change.
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(reader.read("data", &hash(bytes), 100).is_err());
}
