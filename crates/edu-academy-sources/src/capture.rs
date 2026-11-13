//! Read-only textbook capture. No credential discovery or implicit native activation.
use crate::{
    data::{Error, Result},
    owner_admission::{OperationContract, TransportAdmission, TransportScope},
    student_course::{COURSE_NS, METADATA_NS, ReadPort, check_contract, read_contracts},
    student_metadata::PaperFormPin,
    textbook_index::{
        BookScope, GradeOutcome, MAX_SOURCE_TITLES, Scoring, SessionTextbookSectionPort,
        TextbookIndexSource, TextbookStudent,
    },
};
use edu_academy_core::{templates::from_json, workflow::Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
};

const OPERATIONS: &[&str] = &[
    "student.books",
    "textbook.wrongnote",
    "textbook.detail",
    "metadata.paper.initial",
    "metadata.paper.search.7",
];
/// Exact contracts for the owner to publish, not a grant of permission.
pub fn contracts() -> Vec<OperationContract> {
    read_contracts()
        .into_iter()
        .filter(|op| OPERATIONS.contains(&op.id.as_str()))
        .collect()
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Student {
    pub student_id: String,
    pub student_name: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Roster {
    pub students: Vec<Student>,
    pub paper_form: Value,
}
pub struct Options {
    pub date: String,
    pub max_requests: usize,
    pub no_detail: bool,
    pub spool: PathBuf,
    pub raw_dir: Option<PathBuf>,
}
#[derive(Serialize)]
pub struct StudentReport {
    /// One-based roster position, never a private source identity.
    pub roster_ordinal: usize,
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cause: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    pub requests: usize,
    pub registered_books: usize,
    pub section_attempts: usize,
}
fn fail(code: &str) -> Error {
    Error::refusal(code)
}
pub fn causes(error: &Error) -> Vec<String> {
    let mut codes: Vec<String> = error
        .properties
        .get("capture_causes")
        .array()
        .iter()
        .filter_map(|v| {
            if v.string() {
                Some(crate::data::string(v))
            } else {
                None
            }
        })
        .collect();
    codes.push(error.message.clone());
    codes.retain(|c| {
        !c.is_empty()
            && c.len() <= 64
            && c.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    });
    let mut seen = HashSet::new();
    codes.retain(|code| seen.insert(code.clone()));
    codes.truncate(4);
    codes
}
pub fn context(error: Error, code: &str) -> Error {
    let mut outer = fail(code);
    outer.properties.set(
        "capture_causes",
        crate::data::Js::Arr(causes(&error).into_iter().map(Into::into).collect()),
    );
    outer
}
fn scope(a: &TransportAdmission, s: &Student, date: &str, ns: &str) -> TransportScope {
    TransportScope {
        owner_id: a.owner_id.clone(),
        teacher_id: a.teacher_id.clone(),
        student_id: s.student_id.clone(),
        date: date.into(),
        source_namespace: ns.into(),
    }
}
/// Call before opening the owner session: no credentials, lock or login on refusal.
pub fn preflight(a: &TransportAdmission, roster: &Roster, options: &Options) -> Result<()> {
    a.validate()?;
    if let Some(dir) = &options.raw_dir {
        validate_raw_root(dir)?;
    }
    if !(1..=200).contains(&roster.students.len())
        || options.max_requests == 0
        || !options.spool.is_absolute()
    {
        return Err(fail("capture_input_invalid"));
    }
    for op in OPERATIONS {
        check_contract(a, op)?;
    }
    PaperFormPin::parse(&from_json(roster.paper_form.clone()))?;
    let mut ids = HashSet::new();
    for student in &roster.students {
        TextbookStudent::new(&student.student_id, &student.student_name)?;
        if !ids.insert(&student.student_id) {
            return Err(fail("capture_duplicate_student"));
        }
        for ns in [COURSE_NS, METADATA_NS] {
            a.authorize(&scope(a, student, &options.date, ns))?;
        }
    }
    Ok(())
}
struct Bounded<'a, P> {
    port: &'a mut P,
    max: usize,
    count: Rc<Cell<usize>>,
    failure: Rc<RefCell<Option<Error>>>,
}
impl<P: ReadPort> ReadPort for Bounded<'_, P> {
    fn transport_admission(&self) -> &TransportAdmission {
        self.port.transport_admission()
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<crate::http::Response> {
        if self.failure.borrow().is_some() {
            return Err(fail("capture_request_failed"));
        }
        if self.count.get() >= self.max {
            *self.failure.borrow_mut() = Some(fail("capture_request_limit"));
            return Err(fail("capture_request_limit"));
        }
        self.count.set(self.count.get() + 1);
        let result = self.port.read(scope, operation, arguments);
        if let Err(error) = &result {
            *self.failure.borrow_mut() = Some(context(
                error.clone(),
                if error.message.starts_with("replay_")
                    || error.message == "capture_raw_request_mismatch"
                {
                    &error.message
                } else {
                    "capture_request_failed"
                },
            ));
        }
        result
    }
}
/// All source failures, including deliberately degraded paper search, abort publication.
pub fn capture<P: ReadPort>(
    port: &mut RecordingPort<'_, P>,
    roster: &Roster,
    options: &Options,
    runtime: &dyn Runtime,
) -> Vec<StudentReport> {
    if port.recorder.dir != options.raw_dir {
        return admission_reports(roster);
    }
    capture_with(port, roster, options, runtime)
}
fn admission_reports(roster: &Roster) -> Vec<StudentReport> {
    roster
        .students
        .iter()
        .enumerate()
        .map(|(i, _)| StudentReport {
            roster_ordinal: i + 1,
            status: "capture_admission_refused".into(),
            cause: vec!["capture_admission_refused".into()],
            operation: None,
            requests: 0,
            registered_books: 0,
            section_attempts: 0,
        })
        .collect()
}
pub(crate) fn capture_with<P: ReadPort>(
    port: &mut P,
    roster: &Roster,
    options: &Options,
    runtime: &dyn Runtime,
) -> Vec<StudentReport> {
    if let Err(error) = preflight(port.transport_admission(), roster, options) {
        return roster
            .students
            .iter()
            .enumerate()
            .map(|(i, _)| StudentReport {
                roster_ordinal: i + 1,
                status: "capture_admission_refused".into(),
                cause: causes(&context(error.clone(), "capture_admission_refused")),
                operation: None,
                requests: 0,
                registered_books: 0,
                section_attempts: 0,
            })
            .collect();
    }
    roster
        .students
        .iter()
        .enumerate()
        .map(|(i, student)| {
            let count = Rc::new(Cell::new(0));
            let failure = Rc::new(RefCell::new(None));
            let bounded = Bounded {
                port: &mut *port,
                max: options.max_requests,
                count: count.clone(),
                failure: failure.clone(),
            };
            let result = read_student(bounded, student, &roster.paper_form, options, runtime);
            let result = match failure.borrow().clone() {
                Some(mut error) => {
                    if let Err(read_error) = &result {
                        error.properties.set(
                            "capture_causes",
                            crate::data::Js::Arr(
                                causes(read_error).into_iter().map(Into::into).collect(),
                            ),
                        );
                    }
                    Err(error)
                }
                None => result,
            };
            let result = result.and_then(|(books, sections)| {
                publish(&options.spool, &student.student_id, &books, &sections)
                    .map_err(|_| fail("capture_publish_failed"))?;
                Ok((books.len(), sections.len()))
            });
            let (status, cause, books, sections) = match result {
                Ok((b, s)) => ("refreshed".into(), vec![], b, s),
                Err(e) => (e.message.clone(), causes(&e), 0, 0),
            };
            StudentReport {
                roster_ordinal: i + 1,
                status,
                cause,
                operation: None,
                requests: count.get(),
                registered_books: books,
                section_attempts: sections,
            }
        })
        .collect()
}
fn read_student<P: ReadPort>(
    mut port: P,
    student: &Student,
    paper_form: &Value,
    options: &Options,
    runtime: &dyn Runtime,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let a = port.transport_admission();
    let teacher = a.teacher_label.clone();
    let course = scope(a, student, &options.date, COURSE_NS);
    let metadata = scope(a, student, &options.date, METADATA_NS);
    let pin = PaperFormPin::parse(&from_json(paper_form.clone()))
        .map_err(|e| context(e, "capture_form_invalid"))?;
    let session = SessionTextbookSectionPort::new(&mut port, course, metadata, pin);
    let mut source = TextbookIndexSource::new(session, teacher.clone());
    let who = TextbookStudent::new(&student.student_id, &student.student_name)
        .map_err(|e| context(e, "capture_student_invalid"))?;
    let bindings = source
        .bindings(&teacher, &who)
        .map_err(|e| context(e, "capture_bindings_failed"))?;
    let bindings_observed_at = runtime.epoch_ms() / 1000;
    let mut books = vec![];
    let mut sections = vec![];
    let mut keys = HashSet::new();
    for (binding_ordinal, binding) in bindings.iter().enumerate() {
        let book_key = format!("{}|{}", student.student_id, binding.book_no);
        if !keys.insert(book_key.clone()) {
            return Err(fail("capture_duplicate_book"));
        }
        let scope = BookScope::new(&binding.course_key, &binding.book_no)
            .map_err(|e| context(e, "capture_scope_invalid"))?;
        let index = source.index(&teacher, &who, &scope);
        if source.dropped_paper_rows() > 0 {
            return Err(fail("capture_paper_row_dropped"));
        }
        let index = index.map_err(|e| context(e, "capture_index_failed"))?;
        // A structural/form/source error can also be swallowed by the typed index.
        if matches!(
            index.paper_search,
            crate::textbook_index::PaperSearchStatus::Unavailable
        ) {
            return Err(source
                .paper_failure()
                .cloned()
                .map(|e| context(e, "capture_paper_search_failed"))
                .unwrap_or_else(|| fail("capture_paper_search_failed")));
        }
        books.push(json!({"type":"registered_book", "key":book_key, "observed_at":bindings_observed_at,
            "student_id":student.student_id, "student":student.student_id, "book_no":binding.book_no,
            "book_label":binding.book_label, "course_key":binding.course_key, "binding_ordinal":binding_ordinal+1,
            "hint":index.rows.first().map(|r| &r.source_title)}));
        let mut graded = Vec::new();
        if !options.no_detail {
            let titles: Vec<String> = index.rows.iter().map(|r| r.source_title.clone()).collect();
            for batch in titles.chunks(MAX_SOURCE_TITLES) {
                let outcome = source.grade(&teacher, &who, &scope, batch);
                if source.dropped_paper_rows() > 0 {
                    return Err(fail("capture_paper_row_dropped"));
                }
                match outcome.map_err(|e| context(e, "capture_grade_failed"))? {
                    GradeOutcome::Graded(g) if g.paper_search_observed => graded.extend(g.records),
                    _ => {
                        return Err(source
                            .paper_failure()
                            .cloned()
                            .map(|e| context(e, "capture_grade_incomplete"))
                            .unwrap_or_else(|| fail("capture_grade_incomplete")));
                    }
                }
            }
        }
        let mut attempts = HashSet::new();
        for (ordinal, row) in index.rows.iter().enumerate() {
            if !attempts.insert(&row.attempt_key) {
                return Err(fail("capture_duplicate_attempt"));
            }
            let record = graded.iter().find(|g| {
                g.attempt_key == row.attempt_key
                    && g.paper_key == row.paper_key
                    && g.source_title == row.source_title
            });
            if !options.no_detail && record.is_none() {
                return Err(fail("capture_grade_drift"));
            }
            let mut value = json!({"type":"section_attempt", "key":format!("{}|{}", book_key,row.attempt_key),
                "observed_at":runtime.epoch_ms()/1000, "student_id":student.student_id, "student":student.student_id,
                "book":book_key, "book_no":binding.book_no, "book_book_label":binding.book_label, "index_ordinal":ordinal+1,
                "source_title":row.source_title, "source_date_literal":row.source_date_literal,
                "source_date_precision":if row.source_date_literal.as_deref().is_some_and(|s| s.len()==5 && s.as_bytes()[2]==b'-' && s.as_bytes()[..2].iter().all(u8::is_ascii_digit) && s.as_bytes()[3..].iter().all(u8::is_ascii_digit)) {"month_day"} else {"unknown"},
                "correct_count":record.map(|g|g.correct_count()), "item_count":record.map(|g|g.item_count()),
                "score":record.map(|g|g.score), "graded":record.is_some()});
            if let Some(g) = record {
                value["source_date_literal"] = json!(g.source_date.literal);
                value["source_date_precision"] = json!(g.source_date.precision);
                value["wrong_ordinals"] = json!(
                    g.items
                        .iter()
                        .filter(|i| i.scoring == Scoring::Wrong)
                        .map(|i| i.ordinal.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            sections.push(value);
        }
    }
    Ok((books, sections))
}
fn private_dir(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            private_dir(parent)?;
        }
        fs::DirBuilder::new().mode(0o700).create(path)?;
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.permissions().mode() & 0o777 != 0o700 {
        return Err(std::io::Error::other("private_directory_required"));
    }
    Ok(())
}
struct Staged {
    temporary: PathBuf,
    destination: PathBuf,
}
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.temporary);
    }
}
fn stage(dir: &Path, student: &str, records: &[Value]) -> std::io::Result<Staged> {
    private_dir(dir)?;
    let destination = dir.join(format!("{student}.jsonl"));
    if let Ok(meta) = fs::symlink_metadata(&destination)
        && (!meta.is_file() || meta.file_type().is_symlink())
    {
        return Err(std::io::Error::other("regular_partition_required"));
    }
    let nonce = fs::read_to_string("/proc/sys/kernel/random/uuid")?;
    let temporary = dir.join(format!(".{student}.{}.tmp", nonce.trim()));
    let staged = Staged {
        temporary,
        destination,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged.temporary)?;
    for record in records {
        serde_json::to_writer(&mut file, record)?;
        file.write_all(b"\n")?;
    }
    file.sync_all()?;
    let bytes = fs::read(&staged.temporary)?;
    let mut parsed = vec![];
    for line in bytes.split(|b| *b == b'\n').filter(|s| !s.is_empty()) {
        parsed.push(serde_json::from_slice::<Value>(line)?);
    }
    if parsed != records {
        return Err(std::io::Error::other("partition_validation_failed"));
    }
    Ok(staged)
}
fn publish(
    spool: &Path,
    student: &str,
    books: &[Value],
    sections: &[Value],
) -> std::io::Result<()> {
    private_dir(spool)?;
    let books = stage(&spool.join("registered_book"), student, books)?;
    let sections = stage(&spool.join("section_attempt"), student, sections)?;
    fs::rename(&books.temporary, &books.destination)?;
    fs::rename(&sections.temporary, &sections.destination)?;
    Ok(())
}

/// The observer is armed with one exact admitted source request. Login exchanges
/// cannot match it, and are never saved. Recording happens beneath Client parsing.
#[derive(Clone)]
pub struct Recorder<'a> {
    dir: Option<PathBuf>,
    pending: Rc<RefCell<Option<Pending>>>,
    mismatch: Rc<Cell<bool>>,
    runtime: &'a dyn Runtime,
}
struct Pending {
    student_id: String,
    operation: String,
    request: crate::http::Request,
}
pub struct RecordingTransport<'a, T> {
    inner: T,
    recorder: Recorder<'a>,
}
pub struct RecordingPort<'a, P> {
    inner: P,
    recorder: Recorder<'a>,
}
impl<'a> Recorder<'a> {
    pub fn new(dir: Option<PathBuf>, runtime: &'a dyn Runtime) -> Self {
        Self {
            dir,
            pending: Rc::default(),
            mismatch: Rc::new(Cell::new(false)),
            runtime,
        }
    }
    pub fn transport<T>(&self, inner: T) -> RecordingTransport<'a, T> {
        RecordingTransport {
            inner,
            recorder: self.clone(),
        }
    }
    pub fn port<P>(&self, inner: P) -> RecordingPort<'a, P> {
        RecordingPort {
            inner,
            recorder: self.clone(),
        }
    }
}
impl<T: crate::http::Transport> crate::http::Transport for RecordingTransport<'_, T> {
    fn bind_account(&mut self, lease: Rc<crate::owner_root::AccountLease>) {
        self.inner.bind_account(lease);
    }
    fn dispatch(&mut self, request: &crate::http::Request) -> Result<crate::http::Response> {
        let pending = self.recorder.pending.borrow();
        if let Some(p) = pending.as_ref()
            && (request.method != p.request.method
                || request.path != p.request.path
                || request.query != p.request.query
                || request.body != p.request.body)
        {
            self.recorder.mismatch.set(true);
            return Err(fail("capture_raw_request_mismatch"));
        }
        let response = self.inner.dispatch(request)?;
        if let Some(dir) = &self.recorder.dir
            && let Some(p) = pending.as_ref()
        {
            save_raw(
                dir,
                &p.student_id,
                &p.operation,
                request,
                &response,
                self.recorder.runtime,
            )
            .map_err(|_| fail("capture_raw_failed"))?;
        }
        Ok(response)
    }
}
impl<P: ReadPort> ReadPort for RecordingPort<'_, P> {
    fn transport_admission(&self) -> &TransportAdmission {
        self.inner.transport_admission()
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<crate::http::Response> {
        let request = self
            .inner
            .transport_admission()
            .request(scope, operation, arguments, false)?;
        // Authenticate while unarmed: login exchanges are never source bodies.
        self.inner.authenticate_for_reads()?;
        *self.recorder.pending.borrow_mut() = Some(Pending {
            student_id: scope.student_id.clone(),
            operation: operation.into(),
            request,
        });
        let result = self.inner.read(scope, operation, arguments);
        self.recorder.pending.borrow_mut().take();
        if self.recorder.mismatch.get() {
            return Err(fail("capture_raw_request_mismatch"));
        }
        result
    }
}

/// Existing private-root checks reject symlinks, writable ancestors and foreign owners.
fn validate_raw_root(dir: &Path) -> Result<()> {
    crate::owner_root::OwnerRoot::open(dir).map_err(|e| context(e, "capture_raw_root_refused"))?;
    if dir
        .ancestors()
        .any(|p| fs::symlink_metadata(p.join(".git")).is_ok())
    {
        return Err(fail("capture_raw_root_refused"));
    }
    Ok(())
}
fn save_raw(
    dir: &Path,
    student_id: &str,
    operation: &str,
    request: &crate::http::Request,
    response: &crate::http::Response,
    runtime: &dyn Runtime,
) -> std::io::Result<()> {
    validate_raw_root(dir).map_err(|_| std::io::Error::other("raw_root_refused"))?;
    let partition = dir.join(student_id);
    private_dir(&partition)?;
    let index_path = partition.join("index.jsonl");
    use std::os::unix::fs::MetadataExt;
    let mut index = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(index_path)?;
    let meta = index.metadata()?;
    if !meta.is_file()
        || meta.permissions().mode() & 0o077 != 0
        || meta.nlink() != 1
        || meta.uid() != rustix::process::geteuid().as_raw()
        || meta.len() > 16_777_216
    {
        return Err(std::io::Error::other("raw_index_refused"));
    }
    use std::io::Read;
    let mut previous = String::new();
    index.read_to_string(&mut previous)?;
    let mut sequence = 0;
    for line in previous.lines() {
        let entry: Value = serde_json::from_str(line)?;
        let next = entry["sequence"]
            .as_u64()
            .ok_or_else(|| std::io::Error::other("raw_index_invalid"))?;
        if next != sequence + 1 {
            return Err(std::io::Error::other("raw_index_invalid"));
        }
        sequence = next;
    }
    sequence += 1;
    let destination = partition.join(format!("{sequence:06}-{operation}.body"));
    if destination.exists() {
        return Err(std::io::Error::other("raw_body_exists"));
    }
    let nonce = fs::read_to_string("/proc/sys/kernel/random/uuid")?;
    let staged = Staged {
        temporary: partition.join(format!(".{}.tmp", nonce.trim())),
        destination,
    };
    let mut body = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged.temporary)?;
    body.write_all(&response.bytes)?;
    body.sync_all()?;
    fs::rename(&staged.temporary, &staged.destination)?;
    let entry = json!({"sequence":sequence,"operation":operation,"parameters":{"query":request.query,"body":request.body},"http_status":response.status,"byte_count":response.bytes.len(),"sha256":crate::data::hash(&response.bytes),"observed_at":runtime.epoch_ms()/1000});
    serde_json::to_writer(&mut index, &entry)?;
    index.write_all(b"\n")?;
    index.sync_all()?;
    Ok(())
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
