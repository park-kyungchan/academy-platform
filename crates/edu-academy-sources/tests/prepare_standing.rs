//! The standing form of owner preparation and the secret-file credential provider.
//! Synthetic files only; no login, no lock, no network.
use edu_academy_sources::{
    owner_workflow::CredentialProvider,
    prepare_owner::{SecretFileCredentials, prepare_standing},
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    process::Command,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new(name: &str) -> Self {
        let base = PathBuf::from(std::env::var("S09_NATIVE_FIXTURE_ROOT").unwrap())
            .join(format!("prepare-standing-{name}-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        let f = Self(base);
        f.put("identity.json", &json!({"teacherSourceKey":"99001","teacherLabel":"Fictional Teacher","collegeSourceKey":"88001","sourceUsername":"Standing_USER_CANARY"}).to_string());
        f.put(
            "secret.env",
            "# synthetic\nSTANDING_KEY='Standing_PASSWORD_CANARY'\n",
        );
        f.put(
            "students.json",
            r#"[{"student_id":"77001","student_name":"Invented Learner"}]"#,
        );
        f.put("lock", "synthetic lock");
        f
    }
    fn put(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
        fs::set_permissions(self.0.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn path(&self, name: &str) -> String {
        self.0.join(name).display().to_string()
    }
    fn prepare(&self, extra: &[&str]) -> (i32, Value) {
        let out = Command::new(env!("CARGO_BIN_EXE_academy-capture"))
            .arg("prepare-owner")
            .arg("--standing")
            .args(["--identity", &self.path("identity.json")])
            .args(["--lock", &self.path("lock")])
            .args(["--students", &self.path("students.json")])
            .args([
                "--allowed-from",
                "2040-01-01",
                "--allowed-through",
                "2040-12-31",
            ])
            .args(["--owner-root", &self.path("owner")])
            .args(extra)
            .output()
            .unwrap();
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        for private in [
            "Standing_USER_CANARY",
            "Standing_PASSWORD_CANARY",
            "Invented Learner",
        ] {
            assert!(!text.contains(private), "{text}");
        }
        (
            out.status.code().unwrap(),
            serde_json::from_str(text.trim()).unwrap(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_standing_form_writes_no_credential_document() {
    let f = Fixture::new("form");
    let (code, out) = f.prepare(&["--families", "dailyrecord"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["standing"], true);
    let mut names: Vec<_> = out["documents"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "account-lock.json",
            "roster.json",
            "transport-admission.json"
        ]
    );
    assert!(out.get("credentialExpiresAtMs").is_none());
    let mut files: Vec<_> = fs::read_dir(f.0.join("owner"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    assert_eq!(files, names);
    let roster: Value =
        serde_json::from_slice(&fs::read(f.0.join("owner/roster.json")).unwrap()).unwrap();
    assert_eq!(roster["paper_form"], Value::Null);
    let admission = fs::read_to_string(f.0.join("owner/transport-admission.json")).unwrap();
    assert!(admission.contains("student.day.groups") && admission.contains("2040-12-31"));
    // An existing root is kept; a credential argument is not part of the form.
    assert_eq!(f.prepare(&[]).1["status"], "prepare_root_exists");
    fs::remove_dir_all(f.0.join("owner")).unwrap();
    for extra in [
        vec!["--secret-env", "/x"],
        vec!["--expires-at", "2040-12-31T00:00:00Z"],
        vec!["--families", "dailyrecord,dailyrecord"],
    ] {
        let (code, out) = f.prepare(&extra);
        assert_eq!(
            (code, out["status"].as_str()),
            (2, Some("prepare_input_invalid"))
        );
    }
    assert!(!f.0.join("owner").exists());
}

#[test]
fn the_secret_file_provider_reads_the_password_and_the_user_name_at_call_time() {
    let f = Fixture::new("provider");
    let mut provider = SecretFileCredentials {
        identity: f.0.join("identity.json"),
        secret_file: f.0.join("secret.env"),
        secret_key: "STANDING_KEY".into(),
    };
    // Nothing is read at construction: a file written afterwards is what the call reads.
    f.put("secret.env", "STANDING_KEY=\"Later_PASSWORD\"\n");
    assert!(provider.credential().is_ok());
    for (file, text) in [
        ("secret.env", "OTHER=1\n"),
        ("secret.env", "STANDING_KEY=a\nSTANDING_KEY=b\n"),
        (
            "identity.json",
            r#"{"teacherSourceKey":"1","teacherLabel":"x","collegeSourceKey":"2","sourceUsername":""}"#,
        ),
        ("identity.json", "not json"),
    ] {
        let g = Fixture::new("provider-bad");
        g.put(file, text);
        let mut bad = SecretFileCredentials {
            identity: g.0.join("identity.json"),
            secret_file: g.0.join("secret.env"),
            secret_key: "STANDING_KEY".into(),
        };
        let error = bad.credential().err().unwrap();
        assert_eq!(error.message, "native_owner_credential_refused");
        drop(g);
    }
    // A symlink or a relative path is refused by the preparation's file checks.
    std::os::unix::fs::symlink(f.0.join("secret.env"), f.0.join("link.env")).unwrap();
    for secret_file in [f.0.join("link.env"), PathBuf::from("secret.env")] {
        let mut bad = SecretFileCredentials {
            identity: f.0.join("identity.json"),
            secret_file,
            secret_key: "STANDING_KEY".into(),
        };
        assert!(bad.credential().is_err());
    }
    let _ = prepare_standing;
}
