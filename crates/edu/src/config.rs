//! The configuration file and the run log. The file is the only input that names a path.
use crate::{Answer, refuse};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Config {
    schema_version: String,
    /// The standing owner root: `account-lock.json`, `transport-admission.json` and
    /// `roster.json`, the last two pinned by their digests.
    pub owner_root: PathBuf,
    pub admission_sha256: String,
    pub roster_sha256: String,
    pub spool: PathBuf,
    pub store: PathBuf,
    pub secret_file: PathBuf,
    pub secret_key: String,
    pub identity_file: PathBuf,
    #[serde(default)]
    pub raw_dir: Option<PathBuf>,
    #[serde(default = "default_wait")]
    pub lock_wait_seconds: u64,
}
fn default_wait() -> u64 {
    30
}
fn flags() -> i32 {
    (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32
}
fn caller() -> u32 {
    rustix::process::geteuid().as_raw()
}
/// A regular file of the caller, not writable by group or others, at most 64 KiB, with
/// the members above only, absolute paths and a wait of at most an hour.
pub(crate) fn load(path: &Path) -> Answer<Config> {
    let refused = |_: std::io::Error| refuse("lesson_config_refused");
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(flags())
        .open(path)
        .map_err(refused)?;
    let meta = file.metadata().map_err(refused)?;
    if !meta.is_file() || meta.uid() != caller() || meta.mode() & 0o022 != 0 || meta.len() > 65536 {
        return Err(refuse("lesson_config_refused"));
    }
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).map_err(refused)?;
    let config: Config =
        serde_json::from_slice(&bytes).map_err(|_| refuse("lesson_config_refused"))?;
    let paths = [
        Some(&config.owner_root),
        Some(&config.spool),
        Some(&config.store),
        Some(&config.secret_file),
        Some(&config.identity_file),
        config.raw_dir.as_ref(),
    ];
    if config.schema_version != "edu.lesson-config/1"
        || paths.into_iter().flatten().any(|p| !p.is_absolute())
        || config.store.parent().is_none()
        || config.lock_wait_seconds > 3600
    {
        return Err(refuse("lesson_config_refused"));
    }
    Ok(config)
}
/// Append one line to `<directory of the store>/lesson-runs.jsonl`, a private file.
pub(crate) fn append_run(config: &Config, line: &Value) -> std::io::Result<()> {
    let path = config.store.with_file_name("lesson-runs.jsonl");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(flags())
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.uid() != caller() || meta.mode() & 0o077 != 0 || meta.nlink() != 1 {
        return Err(std::io::Error::other("run_log_not_private"));
    }
    file.write_all(format!("{line}\n").as_bytes())
}
