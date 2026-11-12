//! Persistent invented record observations only; no transport or save authority.
use crate::{
    record_data::*,
    record_store::{DraftStore, MAX_ROOT_BYTES},
};
use edu_academy_core::js::{Fault, Result};
use rustix::fs::{self as rfs, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path},
};

pub const MAX_SOURCE_SNAPSHOT_BYTES: usize = 1_048_576;
pub const MAX_SOURCE_RECORDS: usize = 256;
pub const MAX_SOURCE_OPERATIONS: usize = 128;
pub const MAX_SOURCE_JSON_DEPTH: usize = 32;
pub const SOURCE_SNAPSHOT: &str = "record-source.json";

/// Only concrete owned synthetic data can enter this constructor boundary.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordSeed {
    pub owner: OwnerScope,
    pub records: Vec<RecordSnapshot>,
}
/// Counters belong to this process, while every read reloads the durable source.
pub struct SyntheticRecordState {
    store: DraftStore,
    root: File,
    owner: OwnerScope,
    reads: u64,
    operations: usize,
}
fn unavailable() -> Fault {
    Fault::error("record_source_unavailable")
}
fn invalid() -> Fault {
    Fault::error("record_source_invalid")
}
fn limit() -> Fault {
    Fault::error("record_source_limit")
}
fn io<T, E>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|_| unavailable())
}
fn flags() -> OFlags {
    OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK
}
fn root_file(path: &Path) -> Result<File> {
    let mut root: File = io(rfs::open(
        "/",
        flags() | OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
    ))?
    .into();
    for part in path.components() {
        match part {
            Component::Normal(name) => {
                root = io(rfs::openat(
                    &root,
                    name,
                    flags() | OFlags::RDONLY | OFlags::DIRECTORY,
                    Mode::empty(),
                ))?
                .into()
            }
            Component::RootDir => {}
            _ => return Err(invalid()),
        }
    }
    Ok(root)
}
fn private(file: &File) -> Result<()> {
    let m = io(file.metadata())?;
    if !m.is_file()
        || m.nlink() != 1
        || m.mode() & 0o777 != 0o600
        || m.uid() != rustix::process::geteuid().as_raw()
    {
        return Err(invalid());
    }
    Ok(())
}
fn read_bytes(root: &File) -> Result<Option<Vec<u8>>> {
    let file: File = match rfs::openat(
        root,
        SOURCE_SNAPSHOT,
        flags() | OFlags::RDONLY,
        Mode::empty(),
    ) {
        Ok(fd) => fd.into(),
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(unavailable()),
    };
    private(&file)?;
    if io(file.metadata())?.len() > MAX_SOURCE_SNAPSHOT_BYTES as u64 {
        return Err(limit());
    }
    let mut bytes = Vec::new();
    io(file
        .take((MAX_SOURCE_SNAPSHOT_BYTES + 1) as u64)
        .read_to_end(&mut bytes))?;
    if bytes.len() > MAX_SOURCE_SNAPSHOT_BYTES {
        return Err(limit());
    }
    Ok(Some(bytes))
}
fn depth(bytes: &[u8]) -> Result<()> {
    let (mut level, mut quoted, mut escaped) = (0usize, false, false);
    for &b in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    level += 1;
                    if level > MAX_SOURCE_JSON_DEPTH {
                        return Err(limit());
                    }
                }
                b'}' | b']' => level = level.checked_sub(1).ok_or_else(invalid)?,
                _ => {}
            }
        }
    }
    Ok(()) // The typed JSON decoder checks balanced syntax and string endings.
}
fn valid_identity(id: &RecordIdentity, owner: &OwnerScope) -> Result<()> {
    let date =
        chrono::NaiveDate::parse_from_str(&id.lesson_date, "%Y-%m-%d").map_err(|_| invalid())?;
    if &id.owner != owner
        || id.student_key.is_empty()
        || date.format("%Y-%m-%d").to_string() != id.lesson_date
    {
        return Err(invalid());
    }
    Ok(())
}
fn validate(seed: &RecordSeed, owner: &OwnerScope) -> Result<()> {
    if &seed.owner != owner || owner.owner.is_empty() || owner.account.is_empty() {
        return Err(invalid());
    }
    if seed.records.len() > MAX_SOURCE_RECORDS {
        return Err(limit());
    }
    let mut identities = BTreeSet::new();
    for record in &seed.records {
        valid_identity(&record.identity, owner)?;
        if record.requested_date != record.identity.lesson_date
            || !identities.insert(record.identity.key()?)
        {
            return Err(invalid());
        }
        chrono::DateTime::parse_from_rfc3339(&record.provenance.read_at).map_err(|_| invalid())?;
        // Absence is represented by a missing row; transient failures are not records.
        if !matches!(&record.observation, RecordObservation::Observed { occurrence, .. } if !occurrence.is_empty())
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn decode(bytes: &[u8], owner: &OwnerScope) -> Result<RecordSeed> {
    if bytes.len() > MAX_SOURCE_SNAPSHOT_BYTES {
        return Err(limit());
    }
    depth(bytes)?;
    let seed: RecordSeed = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    validate(&seed, owner)?;
    Ok(seed)
}
fn initialize(root: &File, bytes: &[u8]) -> Result<()> {
    let temp = format!(".record-source-{}.tmp", std::process::id());
    let mut file: File = io(rfs::openat(
        root,
        temp.as_str(),
        flags() | OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
        Mode::from_raw_mode(0o600),
    ))?
    .into();
    let result = (|| {
        private(&file)?;
        io(file.write_all(bytes).and_then(|()| file.sync_all()))?;
        io(rfs::renameat(root, temp.as_str(), root, SOURCE_SNAPSHOT))?;
        io(root.sync_all())
    })();
    if result.is_err() {
        // No retry: bytes may already be visible after an uncertain directory sync.
        let _ = rfs::unlinkat(root, temp.as_str(), rfs::AtFlags::empty());
    }
    result.map_err(|mut error| {
        let status = match read_bytes(root) {
            Ok(Some(value)) if value == bytes => "matches",
            Ok(Some(_)) => "different",
            Ok(None) => "missing",
            Err(_) => "unreadable",
        };
        error.properties.set("writeOutcome", "uncertain".into());
        error.properties.set("targetReadback", status.into());
        error
    })
}
impl SyntheticRecordState {
    /// Open the marked owner root first. Seed only an absent snapshot; an existing
    /// snapshot is validated and reopened, never merged with or replaced by seed.
    /// entry_limit bounds root accounting during first publication (including temp).
    pub fn open(root: &Path, seed: RecordSeed, entry_limit: usize) -> Result<Self> {
        let store = DraftStore::open(root, seed.owner.clone())?;
        let account = store.lock_account()?;
        let directory = root_file(root)?;
        if let Some(bytes) = read_bytes(&directory)? {
            decode(&bytes, &seed.owner)?;
        } else {
            validate(&seed, &seed.owner)?;
            let bytes = serde_json::to_vec(&seed).map_err(|_| invalid())?;
            decode(&bytes, &seed.owner)?;
            let usage = account.usage(entry_limit)?;
            if usage
                .bytes
                .checked_add(bytes.len() as u64)
                .is_none_or(|n| n > MAX_ROOT_BYTES)
                || usage.entries >= entry_limit
            {
                return Err(Fault::error("record_store_capacity"));
            }
            initialize(&directory, &bytes)?;
        }
        drop(account);
        Ok(Self {
            store,
            root: directory,
            owner: seed.owner,
            reads: 0,
            operations: 0,
        })
    }
    /// The server starts one logical teacher call; this is not an approval gate.
    pub fn begin_call(&mut self) {
        self.operations = 0;
    }
    pub fn read_count(&self) -> u64 {
        self.reads
    }
    pub fn source_operations(&self) -> usize {
        self.operations
    }
    /// Exact resolved identity only. Never searches another date or projects a
    /// known blank/unknown/attribution reason to an invented academic value.
    pub fn read(&mut self, id: &RecordIdentity, read_at: &str) -> Result<RecordSnapshot> {
        valid_identity(id, &self.owner)?;
        chrono::DateTime::parse_from_rfc3339(read_at).map_err(|_| invalid())?;
        if self.operations == MAX_SOURCE_OPERATIONS {
            return Err(limit());
        }
        self.operations += 1;
        self.reads = self.reads.checked_add(1).ok_or_else(limit)?;
        let _account = self.store.lock_account()?;
        let bytes = read_bytes(&self.root)?.ok_or_else(unavailable)?;
        let source = decode(&bytes, &self.owner)?;
        let mut record = source
            .records
            .into_iter()
            .find(|r| r.identity == *id)
            .unwrap_or_else(|| RecordSnapshot {
                identity: id.clone(),
                requested_date: id.lesson_date.clone(),
                provenance: RecordProvenance {
                    read_at: read_at.into(),
                    evidence_refs: vec![],
                    source_revision: None,
                    wire_digest: None,
                    field_reasons: RecordFields::from_fn(|_| None),
                },
                observation: RecordObservation::NotObserved,
            });
        record.provenance.read_at = read_at.into();
        Ok(record)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_depth_boundary_ignores_escaped_string_contents() {
        for n in [MAX_SOURCE_JSON_DEPTH, MAX_SOURCE_JSON_DEPTH + 1] {
            let bytes = format!("{}0{}", "[".repeat(n), "]".repeat(n));
            assert_eq!(depth(bytes.as_bytes()).is_ok(), n == MAX_SOURCE_JSON_DEPTH);
        }
        depth(br#"{"text":"[\"{\\]"}"#).unwrap();
    }
}
