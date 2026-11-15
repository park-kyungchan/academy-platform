//! One spool lock; both candidates validated before rows, then coverage, are renamed.
use super::*;
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub files: usize,
    pub lines: usize,
    pub bytes: usize,
    pub file_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            files: 190,
            lines: 45_000,
            bytes: 48 * 1024 * 1024,
            file_bytes: 12 * 1024 * 1024,
        }
    }
}
pub(super) fn ensure_dirs(spool: &Path) -> Result<()> {
    for path in [
        spool.to_path_buf(),
        spool.join("lesson_record"),
        spool.join("lesson_day"),
    ] {
        crate::capture::private_dir(&path).map_err(|_| fail("capture_publish_failed"))?;
    }
    Ok(())
}
pub(super) fn open_lock(spool: &Path) -> Result<fs::File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(spool.join(".lesson-record.lock"))
        .map_err(|_| fail("capture_publish_failed"))?;
    let m = file
        .metadata()
        .map_err(|_| fail("capture_publish_failed"))?;
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != rustix::process::geteuid().as_raw()
        || m.permissions().mode() & 0o077 != 0
    {
        return Err(fail("capture_publish_failed"));
    }
    Ok(file)
}
fn old_bytes(path: &Path) -> Result<Vec<u8>> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .map_err(|_| fail("capture_day_old_file_invalid"))?;
    let m = f
        .metadata()
        .map_err(|_| fail("capture_day_old_file_invalid"))?;
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != rustix::process::geteuid().as_raw()
        || m.permissions().mode() & 0o077 != 0
        || m.len() > 16 * 1024 * 1024
    {
        return Err(fail("capture_day_old_file_invalid"));
    }
    let mut bytes = vec![];
    f.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("capture_day_old_file_invalid"))?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(fail("capture_day_old_file_invalid"));
    }
    Ok(bytes)
}
fn key(v: &Value, kind: &str) -> Result<String> {
    if v["type"] != kind {
        return Err(fail("record_type_mismatch"));
    }
    if kind == "lesson_record" {
        record_key(v)
    } else {
        v["lesson_date"]
            .as_str()
            .filter(|d| valid_date(&(*d).into()))
            .map(str::to_string)
            .ok_or_else(|| fail("capture_day_key_invalid"))
    }
}
fn time(v: &Value, now: u64) -> Result<u64> {
    let time = v["observed_at"]
        .as_u64()
        .ok_or_else(|| fail("capture_day_old_file_invalid"))?;
    if time > now {
        return Err(fail("record_time_in_future"));
    }
    Ok(time)
}
struct Candidate {
    path: PathBuf,
    bytes: Vec<u8>,
    before: usize,
}
fn candidate(
    spool: &Path,
    kind: &str,
    stem: &str,
    date: &str,
    records: &[Value],
    now: u64,
    limits: Limits,
) -> Result<Candidate> {
    let dir = spool.join(kind);
    let path = dir.join(format!("{stem}.jsonl"));
    let mut incoming = HashMap::new();
    let mut appended = vec![];
    for r in records {
        let k = key(r, kind)?;
        let t = time(r, now)?;
        if incoming.insert(k, t).is_some() {
            return Err(fail("duplicate_key"));
        }
        serde_json::to_writer(&mut appended, r).map_err(|_| fail("capture_publish_failed"))?;
        appended.push(b'\n');
    }
    let mut files = 0;
    let mut lines = 0;
    let mut bytes = 0;
    let mut found = false;
    let mut kept = vec![];
    let mut seen = HashSet::new();
    let mut before = 0;
    for entry in fs::read_dir(&dir).map_err(|_| fail("capture_day_old_file_invalid"))? {
        let entry = entry.map_err(|_| fail("capture_day_old_file_invalid"))?;
        let file = entry.path();
        if file.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        files += 1;
        let old = old_bytes(&file)?;
        let replacing = file == path;
        if replacing {
            found = true;
        }
        for line in old.split_inclusive(|b| *b == b'\n') {
            let v: Value =
                serde_json::from_slice(line).map_err(|_| fail("capture_day_old_file_invalid"))?;
            let k = key(&v, kind)?;
            let old_date = v["lesson_date"].as_str().expect("validated key");
            if file.file_stem().and_then(|s| s.to_str())
                != Some(&old_date[..if kind == "lesson_record" { 7 } else { 4 }])
            {
                return Err(fail("capture_day_old_file_invalid"));
            }
            let t = time(&v, now)?;
            if !seen.insert(k.clone()) {
                return Err(fail("duplicate_key"));
            }
            if replacing && v["lesson_date"] == date {
                before += 1;
                if incoming.get(&k).is_some_and(|new| *new < t) {
                    return Err(fail("record_older_than_head"));
                }
            } else {
                lines += 1;
                if replacing {
                    kept.extend_from_slice(line);
                }
            }
        }
        if !replacing {
            bytes += old.len();
        }
    }
    if !found {
        files += 1;
    }
    if !kept.is_empty() && !kept.ends_with(b"\n") && !appended.is_empty() {
        return Err(fail("capture_day_old_file_invalid"));
    }
    kept.extend(appended);
    bytes += kept.len();
    lines += records.len();
    for (over, code) in [
        (files > limits.files, "capture_day_too_many_files"),
        (lines > limits.lines, "capture_day_too_many_records"),
        (bytes > limits.bytes, "capture_day_source_text_too_large"),
        (kept.len() > limits.file_bytes, "capture_day_file_too_large"),
    ] {
        if over {
            return Err(fail(code));
        }
    }
    Ok(Candidate {
        path,
        bytes: kept,
        before,
    })
}
struct Staged {
    path: PathBuf,
    destination: PathBuf,
}
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
fn stage(c: &Candidate) -> Result<Staged> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = c.path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| fail("capture_publish_failed"))?;
    let staged = Staged {
        path,
        destination: c.path.clone(),
    };
    f.write_all(&c.bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| fail("capture_publish_failed"))?;
    if old_bytes(&staged.path)? != c.bytes {
        return Err(fail("capture_publish_failed"));
    }
    Ok(staged)
}
pub(super) fn publish(
    spool: &Path,
    date: &str,
    p: &Page,
    now: u64,
    limits: Limits,
) -> Result<usize> {
    publish_with(spool, date, p, now, limits, &|from, to| {
        fs::rename(from, to)
    })
}
pub(super) fn publish_with(
    spool: &Path,
    date: &str,
    p: &Page,
    now: u64,
    limits: Limits,
    rename: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<usize> {
    ensure_dirs(spool)?;
    let lock = open_lock(spool)?;
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive)
        .map_err(|_| fail("capture_publish_failed"))?;
    let day = json!({"type":"lesson_day","source_namespace":DAY_NS,"lesson_date":date,"observed_at":p.observed,"row_count":p.rows,"stored_count":p.records.len(),"outside_roster_count":p.outside,"cut_fields":p.cut});
    let rows = candidate(
        spool,
        "lesson_record",
        &date[..7],
        date,
        &p.records,
        now,
        limits,
    )?;
    let coverage = candidate(spool, "lesson_day", &date[..4], date, &[day], now, limits)?;
    let first = stage(&rows)?;
    let second = stage(&coverage)?;
    rename(&first.path, &first.destination).map_err(|_| fail("capture_publish_failed"))?;
    rename(&second.path, &second.destination).map_err(|_| fail("capture_publish_failed"))?;
    Ok(rows.before)
}
