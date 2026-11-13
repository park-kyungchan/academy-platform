//! A2: missing operation refuses before any credential or shared-lock file exists.
use edu_academy_sources::{capture, data::hash, owner_admission::TransportAdmission};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    process::Command,
};
#[test]
fn missing_operation_refuses_binary_before_owner_login() {
    let root = PathBuf::from(std::env::var("S09_NATIVE_FIXTURE_ROOT").unwrap())
        .join(format!("capture-cli-{}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let fields:Vec<Value>=["clg_no","cls_no","p_pageno","check_da_test","check_fa_test","check_na_test","stu_name","chioce_cls","sort_date1","sort_date2"].iter().map(|name|json!({"name":name,"tag":"input","type":"hidden","value":null,"checked":null,"options":[]})).collect();
    let roster = json!({"students":[{"student_id":"77001","student_name":"Invented Learner"}],"paper_form":{"schemaVersion":"academy.paper-search-form/1","action":"/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&ass_no=1001","method":"post","fields":fields}});
    fs::write(root.join("roster.json"), roster.to_string()).unwrap();
    fs::set_permissions(root.join("roster.json"), fs::Permissions::from_mode(0o600)).unwrap();
    let mut a = TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "fictional-owner".into(),
        teacher_id: "99001".into(),
        teacher_label: "Fictional Teacher".into(),
        college_id: "88001".into(),
        allowed_from: "2040-01-01".into(),
        allowed_through: "2040-01-31".into(),
        student_ids: vec!["77001".into()],
        source_namespaces: vec![
            "academy.lms.student_course".into(),
            "academy.lms.metadata".into(),
        ],
        operations: capture::contracts(),
    };
    // Publish a complete document first and check the exact typed operation set.
    let complete = serde_json::to_vec(&a).unwrap();
    fs::write(root.join("complete-admission.json"), &complete).unwrap();
    assert_eq!(
        TransportAdmission::from_bytes(&complete)
            .unwrap()
            .operations
            .len(),
        5
    );
    a.operations.retain(|op| op.id != "textbook.detail");
    let missing = serde_json::to_vec(&a).unwrap();
    fs::write(root.join("admission.json"), &missing).unwrap();
    fs::set_permissions(
        root.join("admission.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_academy-capture"))
        .args(["textbook", "--owner-root"])
        .arg(&root)
        .args([
            "--admission",
            "admission.json",
            "--admission-sha256",
            &hash(&missing),
            "--credential",
            "absent.json",
            "--credential-sha256",
            &"0".repeat(64),
            "--roster",
        ])
        .arg(root.join("roster.json"))
        .arg("--spool")
        .arg(root.join("spool"))
        .args(["--date", "2040-01-04", "--max-requests", "5", "--no-detail"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "capture_admission_refused");
    assert_eq!(response["students"][0]["requests"], 0);
    assert!(!root.join("spool").exists());
    assert!(!root.join("account-lock.json").exists());
    fs::remove_dir_all(root).unwrap();
}
