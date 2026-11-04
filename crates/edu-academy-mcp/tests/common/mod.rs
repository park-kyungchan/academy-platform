#![allow(
    dead_code,
    reason = "Fixture members are shared by separate integration test crates"
)]
use edu_academy_mcp::{digest, release::ReleaseGraph};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
};

/// Only explicit task-owned fictional roots. Artifacts are retained for readback.
pub struct Fixture {
    pub install: PathBuf,
    pub source: PathBuf,
    pub pointer: Vec<u8>,
}
fn put(path: &std::path::Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn pin(path: &std::path::Path) -> Value {
    json!({"path":path,"sha256":digest(&fs::read(path).unwrap())})
}
fn document(path: &std::path::Path, value: Value) -> Value {
    put(path, &serde_json::to_vec(&value).unwrap());
    pin(path)
}
impl Fixture {
    pub fn new() -> Self {
        let base = PathBuf::from(
            std::env::var_os("S11_TEST_ROOT").expect("explicit S11_TEST_ROOT required"),
        );
        assert!(base.is_absolute());
        let unique = format!(
            "source-graph-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let base = base.join(unique);
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        for name in ["install", "evidence", "state"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(name))
                .unwrap();
        }
        let install = base.join("install");
        let source = install.join("source.bin");
        put(&source, b"fictional source version one");
        let bridge = install.join("bridge");
        put(
            &bridge,
            b"fictional executable metadata only; never launched",
        );
        let launcher = install.join("launcher");
        put(
            &launcher,
            b"fictional launcher metadata only; never launched",
        );
        let admission = document(&install.join("admission.json"), json!({"fictional":true}));
        let routing = document(
            &install.join("routing.json"),
            json!({"admissionPath":admission["path"],"admissionSha256":admission["sha256"]}),
        );
        let owner = document(
            &install.join("owner.json"),
            json!({"schemaVersion":"academy.rust-owner/1","kind":"fictional",
            "uid":rustix::process::geteuid().as_raw(),"installRoot":install,"evidenceRoot":base.join("evidence"),"stateRoot":base.join("state")}),
        );
        let mut entry = pin(&bridge);
        entry["role"] = json!("entry");
        let args = vec![
            "--install-root".to_owned(),
            install.to_str().unwrap().into(),
            "--endpoint".into(),
            "saved".into(),
        ];
        let identity = document(
            &install.join("identity.json"),
            json!({"schemaVersion":"academy.rust-endpoint/1","endpoint":"saved",
            "semanticContractVersion":"s11-fictional","launchArgumentsSha256":digest(&serde_json::to_vec(&args).unwrap()),"artifacts":[entry]}),
        );
        let endpoints = document(
            &install.join("endpoints.json"),
            json!({"saved":{"executable":pin(&bridge),"identity":identity,"args":args}}),
        );
        let original = serde_json::from_slice::<Value>(include_bytes!(
            "../../../../migration/mcp-runtime-reference-v1/frozen/inventory/registry.json"
        ))
        .unwrap();
        let registry = document(
            &install.join("registry.json"),
            json!([original
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["tool"]["name"] == "academy_source_status")
                .unwrap()]),
        );
        let mut source_pin = pin(&source);
        source_pin["role"] = json!("source");
        let build = document(
            &install.join("build.json"),
            json!({"schemaVersion":"academy.rust-build/1","sourceRevision":"0000000000000000000000000000000000000000",
            "semanticContractVersion":"s11-fictional","cargoLockSha256":"0000000000000000000000000000000000000000000000000000000000000000","artifacts":[source_pin]}),
        );
        let pointer = serde_json::to_vec(&json!({"schemaVersion":"academy.mcp-release/3","generation":"fictional-v1",
            "bridge":pin(&bridge),"launcher":pin(&launcher),"build":build,"routing":routing,"owner":owner,"registry":registry,"endpoints":endpoints})).unwrap();
        put(&install.join("release.json"), &pointer);
        Self {
            install,
            source,
            pointer,
        }
    }
    pub fn graph(&self) -> ReleaseGraph {
        ReleaseGraph::open(&self.install).unwrap()
    }
    pub fn replace_pointer(&self, bytes: &[u8]) {
        let path = self.install.join("next-pointer.json");
        put(&path, bytes);
        fs::rename(path, self.install.join("release.json")).unwrap();
    }
}
