//! V3 is an explicit Rust graph contract. Legacy graphs are never converted.
use crate::{
    Error, Js, Result,
    identity::{Artifact, EndpointIdentity, Pin},
    obj,
    roots::OwnerDirectory,
};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub const POINTER_BYTES: usize = 65536;
// V3 immutable artifact budget only; neither is a source/MCP response bound.
pub const MANIFEST_BYTES: usize = 4 * 1024 * 1024;
pub const ARTIFACT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pointer {
    pub schema_version: String,
    pub generation: String,
    pub bridge: Pin,
    pub launcher: Pin,
    pub build: Pin,
    pub routing: Pin,
    pub endpoints: Pin,
    pub registry: Pin,
    pub owner: Pin,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Endpoint {
    pub executable: Pin,
    pub identity: Pin,
    pub args: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Owner {
    pub schema_version: String,
    pub kind: String,
    pub uid: u32,
    pub install_root: PathBuf,
    pub evidence_root: PathBuf,
    pub state_root: PathBuf,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Build {
    pub schema_version: String,
    pub source_revision: String,
    pub semantic_contract_version: String,
    pub cargo_lock_sha256: String,
    pub artifacts: Vec<Artifact>,
}
pub struct ReleaseGraph {
    install: OwnerDirectory,
    evidence: OwnerDirectory,
    state: OwnerDirectory,
    pub pointer: Pointer,
    pub owner: Owner,
    pub build: Build,
    pub endpoints: BTreeMap<String, Endpoint>,
    pub registry: crate::registry::ToolRegistry,
    pub routing: Js,
    pub pointer_sha256: String,
    pins: BTreeMap<PathBuf, String>,
}
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|_| Error::new("release_manifest_unqualified"))
}
fn pin_read(directory: &OwnerDirectory, pin: &Pin, cap: usize) -> Result<Vec<u8>> {
    pin.validate()?;
    let captured = directory.read(&pin.path, cap)?;
    if captured.sha256 != pin.sha256 {
        return Err(Error::new("release_artifact_mismatch"));
    }
    Ok(captured.bytes)
}
impl ReleaseGraph {
    pub fn open(base: &Path) -> Result<Self> {
        let install = OwnerDirectory::open(base)?;
        let captured = install.read(&base.join("release.json"), POINTER_BYTES)?;
        let version = crate::parse(&captured.bytes)
            .map_err(|_| Error::new("release_manifest_unqualified"))?;
        if !version.get("schemaVersion").is("academy.mcp-release/3") {
            return Err(Error::new("legacy_graph_refused"));
        }
        let pointer: Pointer = decode(&captured.bytes)?;
        if pointer.schema_version != "academy.mcp-release/3" {
            return Err(Error::new("legacy_graph_refused"));
        }
        if pointer.generation.is_empty()
            || pointer.generation.len() > 64
            || [".", ".."].contains(&pointer.generation.as_str())
            || !pointer
                .generation
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Error::new("release_generation_unqualified"));
        }
        let owner: Owner = decode(&pin_read(&install, &pointer.owner, MANIFEST_BYTES)?)?;
        if owner.schema_version != "academy.rust-owner/1"
            || owner.install_root != base
            || owner.uid != rustix::process::geteuid().as_raw()
        {
            return Err(Error::new("release_owner_identity_mismatch"));
        }
        // No native/container identity can be established by a self-asserted JSON file.
        if owner.kind != "fictional" {
            return Err(Error::new("native_owner_binding_unqualified"));
        }
        let evidence = OwnerDirectory::open(&owner.evidence_root)?;
        let state = OwnerDirectory::open(&owner.state_root)?;
        if owner.evidence_root == owner.state_root
            || owner.install_root == owner.state_root
            || owner.install_root == owner.evidence_root
        {
            return Err(Error::new("release_roots_must_be_distinct"));
        }
        let build: Build = decode(&pin_read(&install, &pointer.build, MANIFEST_BYTES)?)?;
        if build.schema_version != "academy.rust-build/1"
            || build.source_revision.len() != 40
            || !build
                .source_revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !crate::identity::contract_version(&build.semantic_contract_version)
            || !crate::identity::valid_hash(&build.cargo_lock_sha256)
            || build.artifacts.is_empty()
            || build.artifacts.len() > 128
        {
            return Err(Error::new("release_build_identity_unqualified"));
        }
        let endpoints: BTreeMap<String, Endpoint> =
            decode(&pin_read(&install, &pointer.endpoints, MANIFEST_BYTES)?)?;
        if endpoints.is_empty() || endpoints.len() > 256 {
            return Err(Error::new("release_endpoints_unqualified"));
        }
        let registry = crate::registry::ToolRegistry::parse(&pin_read(
            &install,
            &pointer.registry,
            MANIFEST_BYTES,
        )?)?;
        registry.verify_endpoints(endpoints.keys().map(String::as_str))?;
        let routing = crate::parse(&pin_read(&install, &pointer.routing, MANIFEST_BYTES)?)?;
        crate::routing::validate_configuration(&routing)?;
        let mut graph = Self {
            install,
            evidence,
            state,
            pointer,
            owner,
            build,
            endpoints,
            registry,
            routing,
            pointer_sha256: captured.sha256,
            pins: BTreeMap::new(),
        };
        for pin in [
            graph.pointer.bridge.clone(),
            graph.pointer.launcher.clone(),
            graph.pointer.build.clone(),
            graph.pointer.owner.clone(),
            graph.pointer.routing.clone(),
            graph.pointer.endpoints.clone(),
            graph.pointer.registry.clone(),
        ] {
            graph.add_pin(&pin)?;
        }
        graph.add_pin(&Pin {
            path: PathBuf::from(crate::string(graph.routing.get("admissionPath"))?),
            sha256: crate::string(graph.routing.get("admissionSha256"))?,
        })?;
        for artifact in graph.build.artifacts.clone() {
            artifact.validate()?;
            graph.add_pin(&artifact.pin)?;
        }
        for (name, endpoint) in graph.endpoints.clone() {
            if !crate::identity::valid_name(&name, 64) {
                return Err(Error::new("release_endpoint_identity_mismatch"));
            }
            graph.add_pin(&endpoint.executable)?;
            graph.add_pin(&endpoint.identity)?;
            let identity: EndpointIdentity =
                decode(&graph.read_pin(&endpoint.identity, MANIFEST_BYTES)?)?;
            identity.validate(&name, &endpoint.executable, &endpoint.args)?;
            for artifact in identity.artifacts {
                graph.add_pin(&artifact.pin)?;
            }
            crate::endpoint::validate_arguments(&graph.owner.install_root, &name, &endpoint.args)?;
        }
        graph.verify_unchanged()?;
        Ok(graph)
    }
    fn read_pin(&self, pin: &Pin, cap: usize) -> Result<Vec<u8>> {
        if pin.path.starts_with(self.install.path()) {
            pin_read(&self.install, pin, cap)
        } else if pin.path.starts_with(self.evidence.path()) {
            pin_read(&self.evidence, pin, cap)
        } else {
            Err(Error::new("release_artifact_outside_explicit_root"))
        }
    }
    fn add_pin(&mut self, pin: &Pin) -> Result<()> {
        self.read_pin(pin, ARTIFACT_BYTES)?;
        if self
            .pins
            .get(&pin.path)
            .is_some_and(|old| old != &pin.sha256)
        {
            return Err(Error::new("release_conflicting_artifact_pins"));
        }
        self.pins.insert(pin.path.clone(), pin.sha256.clone());
        Ok(())
    }
    pub fn verify_unchanged(&self) -> Result<()> {
        self.install.revalidate()?;
        self.evidence.revalidate()?;
        self.state.revalidate()?;
        let current = self
            .install
            .read(&self.install.path().join("release.json"), POINTER_BYTES)?;
        if current.sha256 != self.pointer_sha256 {
            return Err(Error::new("release_pointer_changed"));
        }
        for (path, sha256) in &self.pins {
            self.read_pin(
                &Pin {
                    path: path.clone(),
                    sha256: sha256.clone(),
                },
                ARTIFACT_BYTES,
            )?;
        }
        let after = self
            .install
            .read(&self.install.path().join("release.json"), POINTER_BYTES)?;
        if after.sha256 != self.pointer_sha256 {
            return Err(Error::new("release_pointer_changed"));
        }
        Ok(())
    }
    pub fn identity(&self) -> Js {
        obj(&[
            ("schemaVersion", "academy.mcp-release/3".into()),
            ("generation", self.pointer.generation.clone().into()),
            ("pointerSha256", self.pointer_sha256.clone().into()),
            ("bridgeSha256", self.pointer.bridge.sha256.clone().into()),
            ("graphQualification", "artifact_pins_only".into()),
            (
                "qualification",
                "incomplete_rust_facade_native_ownership_unverified".into(),
            ),
        ])
    }
    pub fn endpoint_names(&self) -> BTreeSet<&str> {
        self.endpoints.keys().map(String::as_str).collect()
    }
    pub fn endpoint_identity(&self, endpoint: &str) -> Result<Js> {
        let entry = self
            .endpoints
            .get(endpoint)
            .ok_or_else(|| Error::new("release_endpoint_absent"))?;
        crate::parse(&self.read_pin(&entry.identity, MANIFEST_BYTES)?)
    }
    pub fn admission(&self) -> Result<Js> {
        crate::parse(&self.read_pin(
            &Pin {
                path: PathBuf::from(crate::string(self.routing.get("admissionPath"))?),
                sha256: crate::string(self.routing.get("admissionSha256"))?,
            },
            MANIFEST_BYTES,
        )?)
    }
    pub fn receipt(&self, name: &str, value: &Js) -> Result<()> {
        let mut bytes = crate::encode(value)?;
        bytes.push(b'\n');
        self.evidence.write_receipt(name, &bytes)
    }
}

/// Any observed mismatch permanently refuses this connection, even after restoration.
#[derive(Default)]
pub struct ConnectionGate {
    refusal: Option<Error>,
}
impl ConnectionGate {
    pub fn check(&mut self, observation: Result<()>) -> Result<()> {
        if let Some(error) = self.refusal {
            return Err(error);
        }
        if let Err(error) = observation {
            self.refusal = Some(error);
            return Err(error);
        }
        Ok(())
    }
    pub fn refused(&self) -> bool {
        self.refusal.is_some()
    }
}
