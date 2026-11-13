//! Explicit owner publication. No environment discovery, login or private diagnostics.
use crate::{
    capture::{self, Options, Roster, Student},
    data::{Error, Result, hash},
    owner_admission::TransportAdmission,
    owner_root::OwnerRoot,
    owner_workflow::{CredentialProvider, FileCredentialProvider, PinnedInput},
};
use edu_academy_core::{
    instant::{self, Grammar},
    workflow::Runtime,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct Inputs {
    pub identity: PathBuf,
    pub secret_env: PathBuf,
    pub secret_key: String,
    pub lock: PathBuf,
    pub students: PathBuf,
    pub paper_form: PathBuf,
    pub allowed_from: String,
    pub allowed_through: String,
    pub expires_at: String,
    pub owner_root: PathBuf,
}
fn refused(code: &str) -> Error {
    Error::refusal(code)
}
pub fn parse_args(args: impl Iterator<Item = String>) -> Result<Inputs> {
    let mut args = args;
    let mut values = HashMap::new();
    while let Some(key) = args.next() {
        if ![
            "--identity",
            "--secret-env",
            "--secret-key",
            "--lock",
            "--students",
            "--paper-form",
            "--allowed-from",
            "--allowed-through",
            "--expires-at",
            "--owner-root",
        ]
        .contains(&key.as_str())
            || values
                .insert(
                    key,
                    args.next()
                        .ok_or_else(|| refused("prepare_input_invalid"))?,
                )
                .is_some()
        {
            return Err(refused("prepare_input_invalid"));
        }
    }
    let mut get = |key: &str| {
        values
            .remove(key)
            .ok_or_else(|| refused("prepare_input_invalid"))
    };
    Ok(Inputs {
        identity: get("--identity")?.into(),
        secret_env: get("--secret-env")?.into(),
        secret_key: get("--secret-key")?,
        lock: get("--lock")?.into(),
        students: get("--students")?.into(),
        paper_form: get("--paper-form")?.into(),
        allowed_from: get("--allowed-from")?,
        allowed_through: get("--allowed-through")?,
        expires_at: get("--expires-at")?,
        owner_root: get("--owner-root")?.into(),
    })
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    if !path.is_absolute() {
        return Err(refused("prepare_input_refused"));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .map_err(|_| refused("prepare_input_refused"))?;
    let meta = file
        .metadata()
        .map_err(|_| refused("prepare_input_refused"))?;
    if !meta.is_file()
        || meta.nlink() != 1
        || meta.uid() != rustix::process::geteuid().as_raw()
        || meta.len() > limit
    {
        return Err(refused("prepare_input_refused"));
    }
    let mut bytes = vec![];
    (&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| refused("prepare_input_refused"))?;
    if bytes.len() as u64 > limit {
        return Err(refused("prepare_input_refused"));
    }
    Ok(bytes)
}
fn instant(value: &str) -> Result<u64> {
    instant::epoch_ms(&value.into(), Grammar::Learning)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| refused("prepare_time_refused"))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    teacher_source_key: String,
    teacher_label: String,
    college_source_key: String,
    source_username: String,
}
fn secret(bytes: &[u8], key: &str) -> Result<String> {
    if key.is_empty()
        || key.len() > 128
        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || key.as_bytes()[0].is_ascii_digit()
    {
        return Err(refused("prepare_secret_refused"));
    }
    let input = std::str::from_utf8(bytes).map_err(|_| refused("prepare_secret_refused"))?;
    let mut found = None;
    for line in input.lines() {
        let Some((name, value)) = line.trim().split_once('=') else {
            continue;
        };
        if name != key {
            continue;
        }
        if found.is_some() {
            return Err(refused("prepare_secret_refused"));
        }
        let value = value.trim();
        let value = if value.starts_with(['\'', '"']) {
            if value.len() < 2 || value.as_bytes().first() != value.as_bytes().last() {
                return Err(refused("prepare_secret_refused"));
            }
            &value[1..value.len() - 1]
        } else {
            value
        };
        if value.is_empty() || value.len() > 8192 || value.contains(['\r', '\n', '\0']) {
            return Err(refused("prepare_secret_refused"));
        }
        found = Some(value.to_owned());
    }
    found.ok_or_else(|| refused("prepare_secret_refused"))
}
struct Rollback {
    root: PathBuf,
    files: Vec<PathBuf>,
    armed: bool,
}
impl Drop for Rollback {
    fn drop(&mut self) {
        if self.armed {
            for path in self.files.iter().rev() {
                let _ = fs::remove_file(path);
            }
            let _ = fs::remove_dir(&self.root);
        }
    }
}
fn exclusive(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.sync_all()
}
pub fn prepare(input: &Inputs, runtime: &dyn Runtime) -> Result<Value> {
    prepare_with(input, runtime, exclusive)
}
fn prepare_with(
    input: &Inputs,
    runtime: &dyn Runtime,
    write: fn(&Path, &[u8]) -> std::io::Result<()>,
) -> Result<Value> {
    if fs::symlink_metadata(&input.owner_root).is_ok() {
        return Err(refused("prepare_root_exists"));
    }
    if !input.owner_root.is_absolute()
        || input.owner_root.components().any(|p| {
            matches!(
                p,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(refused("prepare_root_refused"));
    }
    let expires = instant(&input.expires_at)?;
    let from = instant(&format!("{}T00:00:00Z", input.allowed_from))?;
    let through = instant(&format!("{}T00:00:00Z", input.allowed_through))?;
    if from > through || expires <= runtime.epoch_ms() || expires > through + 86_400_000 {
        return Err(refused("prepare_time_refused"));
    }
    if !input.lock.is_absolute()
        || fs::canonicalize(&input.lock).ok().as_ref() != Some(&input.lock)
        || !fs::symlink_metadata(&input.lock).is_ok_and(|m| m.is_file())
    {
        return Err(refused("prepare_lock_refused"));
    }
    let identity: Identity = serde_json::from_slice(&read(&input.identity, 65536)?)
        .map_err(|_| refused("prepare_identity_refused"))?;
    if identity.source_username.is_empty() || identity.source_username.len() > 1024 {
        return Err(refused("prepare_identity_refused"));
    }
    let password = secret(&read(&input.secret_env, 65536)?, &input.secret_key)?;
    let students: Vec<Student> = serde_json::from_slice(&read(&input.students, 1_048_576)?)
        .map_err(|_| refused("prepare_students_refused"))?;
    let paper_form: Value = serde_json::from_slice(&read(&input.paper_form, 262144)?)
        .map_err(|_| refused("prepare_form_refused"))?;
    let admission = TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "academy-capture".into(),
        teacher_id: identity.teacher_source_key,
        teacher_label: identity.teacher_label,
        college_id: identity.college_source_key,
        allowed_from: input.allowed_from.clone(),
        allowed_through: input.allowed_through.clone(),
        student_ids: students.iter().map(|s| s.student_id.clone()).collect(),
        source_namespaces: vec![
            crate::student_course::COURSE_NS.into(),
            crate::student_course::METADATA_NS.into(),
        ],
        operations: capture::contracts(),
    };
    let roster = Roster {
        students,
        paper_form,
    };
    let options = Options {
        date: input.allowed_from.clone(),
        max_requests: 5,
        no_detail: true,
        spool: input.owner_root.join("spool"),
        raw_dir: None,
    };
    capture::preflight(&admission, &roster, &options)
        .map_err(|_| refused("prepare_scope_refused"))?;
    let student_count = roster.students.len();
    let docs = [
        (
            "account-lock.json",
            json!({"schemaVersion":"academy.account-lock/1","path":input.lock}),
        ),
        (
            "credential.json",
            json!({"schemaVersion":"academy.owner-credential/1","username":identity.source_username,"password":password,"expiresAtMs":expires}),
        ),
        (
            "transport-admission.json",
            serde_json::to_value(&admission).map_err(|_| refused("prepare_encode_failed"))?,
        ),
        (
            "roster.json",
            serde_json::to_value(&roster).map_err(|_| refused("prepare_encode_failed"))?,
        ),
    ];
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&input.owner_root)
        .map_err(|_| refused("prepare_root_refused"))?;
    let mut rollback = Rollback {
        root: input.owner_root.clone(),
        files: vec![],
        armed: true,
    };
    fs::set_permissions(&input.owner_root, fs::Permissions::from_mode(0o700))
        .map_err(|_| refused("prepare_root_refused"))?;
    let root =
        Rc::new(OwnerRoot::open(&input.owner_root).map_err(|_| refused("prepare_root_refused"))?);
    let mut pins = serde_json::Map::new();
    for (name, doc) in docs {
        let bytes = serde_json::to_vec(&doc).map_err(|_| refused("prepare_encode_failed"))?;
        let path = input.owner_root.join(name);
        // Register before writing so partial writes are removed on every failure.
        rollback.files.push(path.clone());
        write(&path, &bytes).map_err(|_| refused("prepare_write_failed"))?;
        let digest = hash(&bytes);
        root.read(name, &digest, 1_048_576)
            .map_err(|_| refused("prepare_readback_failed"))?;
        pins.insert(
            name.into(),
            json!({"path":path,"relativePath":name,"sha256":digest,"bytes":bytes.len()}),
        );
    }
    let credential_pin = PinnedInput {
        relative_path: "credential.json".into(),
        sha256: pins["credential.json"]["sha256"]
            .as_str()
            .ok_or_else(|| refused("prepare_readback_failed"))?
            .into(),
    };
    FileCredentialProvider::new(root, credential_pin, runtime)
        .credential()
        .map_err(|_| refused("prepare_credential_refused"))?;
    rollback.armed = false;
    Ok(
        json!({"prepared":true,"ownerRoot":input.owner_root,"documents":pins,"studentCount":student_count,"operationCount":admission.operations.len(),"credentialExpiresAt":input.expires_at,"credentialExpiresAtMs":expires}),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static WRITES: AtomicUsize = AtomicUsize::new(0);
    fn broken(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        exclusive(path, bytes)?;
        if WRITES.fetch_add(1, Ordering::Relaxed) == 1 {
            Err(std::io::Error::other("synthetic_write_failure"))
        } else {
            Ok(())
        }
    }
    // The complete CLI-to-native-session test supplies these inputs and invokes this
    // boundary too; fail after the second exclusive write, not before publication.
    pub(crate) fn fail_second_write(input: &Inputs, runtime: &dyn Runtime) -> Result<Value> {
        WRITES.store(0, Ordering::Relaxed);
        prepare_with(input, runtime, broken)
    }
}
