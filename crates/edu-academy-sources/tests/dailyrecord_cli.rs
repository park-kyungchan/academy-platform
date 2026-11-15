//! Invented owner documents; all refusals precede a possible session opening.
use edu_academy_sources::{
    capture::{self, Family},
    data::hash,
    owner_admission::TransportAdmission,
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    process::Command,
};
#[test]
fn dailyrecord_cli_rejects_bad_dates_and_each_document_digest_before_owner_open() {
    let root = PathBuf::from(std::env::var("S10_TEST_ROOT").unwrap())
        .join(format!("day-cli-{}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let admission = TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "invented".into(),
        teacher_id: "99001".into(),
        teacher_label: "Invented Teacher".into(),
        college_id: "88001".into(),
        allowed_from: "2031-03-01".into(),
        allowed_through: "2031-03-31".into(),
        student_ids: vec!["77001".into()],
        source_namespaces: vec!["academy.lms.dayrecord".into()],
        operations: capture::contracts_for(&[Family::DailyRecord]),
    };
    let documents=[("admission.json",serde_json::to_vec(&admission).unwrap()),("roster.json",serde_json::to_vec(&json!({"students":[{"student_id":"77001","student_name":"Invented"}],"paper_form":null})).unwrap()),("credential.json",serde_json::to_vec(&json!({"schemaVersion":"academy.owner-credential/1","username":"invented","password":"invented","expiresAtMs":9999999999999u64})).unwrap())];
    for (name, bytes) in &documents {
        fs::write(root.join(name), bytes).unwrap();
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    for bad in ["dates", "admission", "roster", "credential"] {
        let digest = |name: &str| {
            if bad == name.trim_end_matches(".json") {
                "0".repeat(64)
            } else {
                hash(&documents.iter().find(|(n, _)| *n == name).unwrap().1)
            }
        };
        let output = Command::new(env!("CARGO_BIN_EXE_academy-capture"))
            .args(["dailyrecord", "--owner-root"])
            .arg(&root)
            .args([
                "--admission",
                "admission.json",
                "--admission-sha256",
                &digest("admission.json"),
                "--roster",
                "roster.json",
                "--roster-sha256",
                &digest("roster.json"),
                "--credential",
                "credential.json",
                "--credential-sha256",
                &digest("credential.json"),
                "--spool",
            ])
            .arg(root.join("spool"))
            .args([
                "--dates",
                if bad == "dates" {
                    "2031-02-29"
                } else {
                    "2031-03-03"
                },
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        let out: Value = serde_json::from_slice(&output.stdout).unwrap();
        let text = out.to_string();
        assert!(
            !text.contains("owner_account"),
            "preflight must refuse before account opening: {text}"
        );
        assert!(!text.contains("Invented") && !text.contains("password"));
        assert!(!root.join("account-lock.json").exists());
    }
    fs::remove_dir_all(root).unwrap();
}
