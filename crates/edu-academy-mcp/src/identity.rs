use crate::{Error, Result, digest};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pin {
    pub path: PathBuf,
    pub sha256: String,
}
impl Pin {
    pub fn validate(&self) -> Result<()> {
        if !crate::roots::canonical_absolute(&self.path) || !valid_hash(&self.sha256) {
            return Err(Error::new("release_pin_unqualified"));
        }
        Ok(())
    }
}
pub fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn valid_name(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
#[derive(Clone, Debug)]
pub struct Artifact {
    pub role: String,
    pub pin: Pin,
}
impl<'de> Deserialize<'de> for Artifact {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            role: String,
            path: PathBuf,
            sha256: String,
        }
        let fields = Fields::deserialize(deserializer)?;
        Ok(Self {
            role: fields.role,
            pin: Pin {
                path: fields.path,
                sha256: fields.sha256,
            },
        })
    }
}
impl Artifact {
    pub fn validate(&self) -> Result<()> {
        self.pin.validate()?;
        if ![
            "entry",
            "source",
            "policy",
            "admission",
            "config",
            "dependency",
        ]
        .contains(&self.role.as_str())
        {
            return Err(Error::new("release_artifact_role_unqualified"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndpointIdentity {
    pub schema_version: String,
    pub endpoint: String,
    pub semantic_contract_version: String,
    pub launch_arguments_sha256: String,
    pub artifacts: Vec<Artifact>,
}
impl EndpointIdentity {
    pub fn validate(&self, endpoint: &str, executable: &Pin, arguments: &[String]) -> Result<()> {
        if self.schema_version != "academy.rust-endpoint/1"
            || self.endpoint != endpoint
            || !valid_name(endpoint, 64)
            || !contract_version(&self.semantic_contract_version)
            || self.artifacts.is_empty()
            || self.artifacts.len() > 128
        {
            return Err(Error::new("release_endpoint_identity_mismatch"));
        }
        for artifact in &self.artifacts {
            artifact.validate()?;
        }
        let entries = self
            .artifacts
            .iter()
            .filter(|a| a.role == "entry")
            .collect::<Vec<_>>();
        if entries.len() != 1
            || &entries[0].pin != executable
            || digest(
                &serde_json::to_vec(arguments).map_err(|_| Error::new("release_argv_invalid"))?,
            ) != self.launch_arguments_sha256
        {
            return Err(Error::new("release_endpoint_identity_mismatch"));
        }
        Ok(())
    }
}
pub fn contract_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._/-".contains(&b))
}
pub fn verify_executable(pin: &Pin, actual: &Path) -> Result<()> {
    if actual != pin.path {
        return Err(Error::new("release_bridge_identity_mismatch"));
    }
    use std::{io::Read, os::unix::fs::MetadataExt};
    let disk =
        std::fs::metadata(&pin.path).map_err(|_| Error::new("release_bridge_identity_mismatch"))?;
    let mut executable = std::fs::File::open("/proc/self/exe")
        .map_err(|_| Error::new("loaded_executable_identity_unavailable"))?;
    let loaded = executable
        .metadata()
        .map_err(|_| Error::new("loaded_executable_identity_unavailable"))?;
    if disk.dev() != loaded.dev() || disk.ino() != loaded.ino() || disk.mode() & 0o100 == 0 {
        return Err(Error::new("release_bridge_identity_mismatch"));
    }
    let mut bytes = Vec::new();
    executable
        .by_ref()
        .take(crate::release::ARTIFACT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new("loaded_executable_identity_unavailable"))?;
    if bytes.len() > crate::release::ARTIFACT_BYTES || digest(&bytes) != pin.sha256 {
        return Err(Error::new("release_bridge_identity_mismatch"));
    }
    Ok(())
}
