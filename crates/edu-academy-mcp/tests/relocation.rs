mod common;
use edu_academy_mcp::{
    identity::{Pin, verify_executable},
    roots::OwnerDirectory,
};
use std::path::Path;

#[test]
fn implicit_relative_or_symlink_root_is_refused() {
    assert!(OwnerDirectory::open(Path::new("relative-root")).is_err());
    let fixture = common::Fixture::new();
    let alias = fixture.install.with_file_name("install-alias");
    std::os::unix::fs::symlink(&fixture.install, &alias).unwrap();
    assert!(OwnerDirectory::open(&alias).is_err());
}
#[test]
fn moving_executable_without_new_graph_refuses_identity() {
    let fixture = common::Fixture::new();
    let graph = fixture.graph();
    let old = fixture.install.join("bridge");
    let moved = fixture.install.join("bridge-moved");
    std::fs::rename(&old, &moved).unwrap();
    assert!(graph.verify_unchanged().is_err());
    let pin = Pin {
        path: old,
        sha256: graph.pointer.bridge.sha256.clone(),
    };
    assert_eq!(
        verify_executable(&pin, &moved).unwrap_err().code,
        "release_bridge_identity_mismatch"
    );
    // Neither relocation nor graph refusal moves or recreates the evidence/state root.
    assert!(graph.owner.evidence_root.is_dir());
    assert!(graph.owner.state_root.is_dir());
}
