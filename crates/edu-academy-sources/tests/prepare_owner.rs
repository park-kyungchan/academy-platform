//! Owner publication CLI: synthetic files only; every output is checked for leakage.
use edu_academy_sources::data::hash;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
};
const PRIVATE: &[&str] = &[
    "OwnerPassword_CANARY",
    "OwnerUsername_CANARY",
    "Invented Learner",
    "Fictional Teacher",
    "77001",
    "99001",
    "88001",
];
struct Fixture {
    base: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = PathBuf::from(std::env::var("S09_NATIVE_FIXTURE_ROOT").unwrap())
            .join(format!("prepare-cli-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        let f = Self { base };
        f.put("identity.json",json!({"teacherSourceKey":"99001","teacherLabel":"Fictional Teacher","collegeSourceKey":"88001","sourceUsername":"OwnerUsername_CANARY"}).to_string().as_bytes());
        f.put(
            "secret.env",
            b"# synthetic\nSECRET_KEY='OwnerPassword_CANARY'\n",
        );
        f.put(
            "students.json",
            br#"[{"student_id":"77001","student_name":"Invented Learner"}]"#,
        );
        let fields:Vec<Value>=["clg_no","cls_no","p_pageno","check_da_test","check_fa_test","check_na_test","stu_name","chioce_cls","sort_date1","sort_date2"].iter().map(|n|json!({"name":n,"tag":"input","type":"hidden","value":null,"checked":null,"options":[]})).collect();
        f.put("form.json",json!({"schemaVersion":"academy.paper-search-form/1","action":"/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&ass_no=1001","method":"post","fields":fields}).to_string().as_bytes());
        f.put("lock", b"retained synthetic lock");
        f
    }
    fn put(&self, name: &str, bytes: &[u8]) {
        fs::write(self.base.join(name), bytes).unwrap();
        fs::set_permissions(self.base.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn run(&self, root: &Path, overrides: &[(&str, String)]) -> Output {
        let mut args = vec![
            (
                "--identity",
                self.base.join("identity.json").display().to_string(),
            ),
            (
                "--secret-env",
                self.base.join("secret.env").display().to_string(),
            ),
            ("--secret-key", "SECRET_KEY".into()),
            ("--lock", self.base.join("lock").display().to_string()),
            (
                "--students",
                self.base.join("students.json").display().to_string(),
            ),
            (
                "--paper-form",
                self.base.join("form.json").display().to_string(),
            ),
            ("--allowed-from", "2040-01-01".into()),
            ("--allowed-through", "2040-01-31".into()),
            ("--expires-at", "2040-01-31T23:00:00Z".into()),
            ("--owner-root", root.display().to_string()),
        ];
        for (key, value) in overrides {
            args.iter_mut().find(|(k, _)| k == key).unwrap().1 = value.clone();
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_academy-capture"));
        command.current_dir(&self.base).arg("prepare-owner");
        for (k, v) in args {
            command.arg(k).arg(v);
        }
        let output = command.output().unwrap();
        for value in PRIVATE {
            assert!(
                !String::from_utf8_lossy(&output.stdout).contains(value),
                "stdout privacy violation"
            );
            assert!(
                !String::from_utf8_lossy(&output.stderr).contains(value),
                "stderr privacy violation"
            );
        }
        output
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
#[test]
fn prepare_cli_success_refusals_rollback_and_output_privacy() {
    let f = Fixture::new();
    let root = f.base.join("owner");
    let result = f.run(&root, &[]);
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    println!(
        "synthetic_prepare_output={}",
        String::from_utf8_lossy(&result.stdout)
    );
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["studentCount"], 1);
    assert_eq!(value["operationCount"], 5);
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in [
        "credential.json",
        "account-lock.json",
        "transport-admission.json",
        "roster.json",
    ] {
        let bytes = fs::read(root.join(name)).unwrap();
        assert_eq!(value["documents"][name]["sha256"], hash(&bytes));
        assert_eq!(value["documents"][name]["bytes"], bytes.len());
        assert_eq!(
            fs::metadata(root.join(name)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Existing roots are retained, rather than erased to satisfy a refusal.
    let old = fs::read(root.join("credential.json")).unwrap();
    assert!(!f.run(&root, &[]).status.success());
    assert_eq!(fs::read(root.join("credential.json")).unwrap(), old);
    fs::remove_dir_all(&root).unwrap();
    for (flag, input) in [
        ("--expires-at", "2020-01-01T00:00:00Z".into()),
        ("--expires-at", "2040-02-02T00:00:00Z".into()),
        ("--expires-at", "invalid-time".into()),
        ("--allowed-from", "2040-02-01".into()),
        ("--lock", f.base.join("missing-lock").display().to_string()),
        ("--lock", f.base.display().to_string()),
        ("--secret-key", "MISSING_KEY".into()),
    ] {
        assert!(!f.run(&root, &[(flag, input)]).status.success());
        assert!(!root.exists());
    }
    f.put("students.json", b"[]");
    assert!(!f.run(&root, &[]).status.success());
    assert!(!root.exists());
    f.put(
        "students.json",
        br#"[{"student_id":"77001","student_name":"Invented Learner"}]"#,
    );
    let bad_parent = f.base.join("untrusted");
    fs::create_dir(&bad_parent).unwrap();
    fs::set_permissions(&bad_parent, fs::Permissions::from_mode(0o777)).unwrap();
    let bad_root = bad_parent.join("owner");
    assert!(!f.run(&bad_root, &[]).status.success());
    assert!(!bad_root.exists());
    assert!(!f.run(Path::new("relative-owner"), &[]).status.success());
    assert!(!f.base.join("relative-owner").exists());
    std::os::unix::fs::symlink(f.base.join("lock"), f.base.join("lock-link")).unwrap();
    assert!(
        !f.run(
            &root,
            &[("--lock", f.base.join("lock-link").display().to_string())]
        )
        .status
        .success()
    );
    assert!(!root.exists());
    f.put(
        "secret.env",
        b"SECRET_KEY=OwnerPassword_CANARY\nSECRET_KEY=OwnerPassword_CANARY\n",
    );
    assert!(!f.run(&root, &[]).status.success());
    assert!(!root.exists());
    f.put("secret.env", b"SECRET_KEY=OwnerPassword_CANARY\n");
    f.put("form.json", b"{}");
    assert!(!f.run(&root, &[]).status.success());
    assert!(!root.exists());
    assert_eq!(
        fs::read(f.base.join("lock")).unwrap(),
        b"retained synthetic lock"
    );
    assert!(!f.base.join("missing-lock").exists());
}
