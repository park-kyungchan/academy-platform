//! Offline recorder replay: no owner root, lease, credentials or native transport.
use crate::{
    capture::{self, Options, Roster, Student},
    data::{Error, Result, hash, obj},
    http::{self, Request, Response, Transport},
    owner_admission::{TransportAdmission, TransportScope},
    student_course::{COURSE_NS, METADATA_NS, ReadPort},
};
use edu_academy_core::workflow::Runtime;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
fn fail(code: &str) -> Error {
    Error::refusal(code)
}
fn read(path: &Path, limit: u64, code: &str) -> Result<Vec<u8>> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .map_err(|_| fail(code))?;
    let m = f.metadata().map_err(|_| fail(code))?;
    if !m.is_file() || m.len() > limit {
        return Err(fail(code));
    }
    let mut bytes = vec![];
    f.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail(code))?;
    if bytes.len() as u64 > limit {
        return Err(fail(code));
    }
    Ok(bytes)
}
#[derive(Deserialize)]
struct Parameters {
    query: Vec<(String, String)>,
    body: Vec<(String, String)>,
}
#[derive(Deserialize)]
struct Entry {
    sequence: u64,
    operation: String,
    parameters: Parameters,
    http_status: u16,
    byte_count: usize,
    sha256: String,
}
struct Saved {
    response: Option<Response>,
}
impl Transport for Saved {
    fn dispatch(&mut self, _: &Request) -> Result<Response> {
        self.response
            .take()
            .ok_or_else(|| fail("replay_body_missing"))
    }
}
struct ReplayPort<'a> {
    admission: &'a TransportAdmission,
    entries: Vec<Entry>,
    dir: PathBuf,
    client: http::Client<Saved>,
    answered: usize,
    missing: Option<String>,
}
impl ReadPort for ReplayPort<'_> {
    fn transport_admission(&self) -> &TransportAdmission {
        self.admission
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<Response> {
        let request = self.admission.request(scope, operation, arguments, false)?;
        let Some(entry) = self.entries.iter().rev().find(|e| {
            e.operation == operation
                && e.parameters.query == request.query
                && e.parameters.body == request.body
        }) else {
            self.missing = Some(operation.into());
            return Err(fail("replay_body_missing"));
        };
        let bytes = read(
            &self
                .dir
                .join(format!("{:06}-{}.body", entry.sequence, entry.operation)),
            6_000_000,
            "replay_body_refused",
        )?;
        if bytes.len() != entry.byte_count || hash(&bytes) != entry.sha256 {
            return Err(fail("replay_body_digest_mismatch"));
        }
        self.client.transport.response = Some(Response {
            status: entry.http_status,
            headers: vec![],
            bytes,
            url: format!("{}{}", http::ORIGIN, request.path),
            stalled: false,
        });
        self.answered += 1;
        self.client.scoped_request(request)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    teacher_source_key: String,
    teacher_label: String,
    college_source_key: String,
    source_username: String,
}
pub fn run(args: impl Iterator<Item = String>, runtime: &dyn Runtime) -> Result<Value> {
    let mut args = args;
    let mut values = HashMap::new();
    let mut no_detail = false;
    while let Some(k) = args.next() {
        if k == "--no-detail" {
            if no_detail {
                return Err(fail("replay_input_invalid"));
            }
            no_detail = true;
            continue;
        }
        if ![
            "--raw-dir",
            "--students",
            "--identity",
            "--paper-form",
            "--spool",
            "--max-requests",
            "--date",
        ]
        .contains(&k.as_str())
            || values
                .insert(k, args.next().ok_or_else(|| fail("replay_input_invalid"))?)
                .is_some()
        {
            return Err(fail("replay_input_invalid"));
        }
    }
    let get = |k: &str| {
        values
            .get(k)
            .cloned()
            .ok_or_else(|| fail("replay_input_invalid"))
    };
    let identity: Identity = serde_json::from_slice(&read(
        Path::new(&get("--identity")?),
        1_048_576,
        "replay_input_invalid",
    )?)
    .map_err(|_| fail("replay_input_invalid"))?;
    if identity.source_username.is_empty() {
        return Err(fail("replay_input_invalid"));
    }
    let students: Vec<Student> = serde_json::from_slice(&read(
        Path::new(&get("--students")?),
        1_048_576,
        "replay_input_invalid",
    )?)
    .map_err(|_| fail("replay_input_invalid"))?;
    let paper_form: Value = serde_json::from_slice(&read(
        Path::new(&get("--paper-form")?),
        1_048_576,
        "replay_input_invalid",
    )?)
    .map_err(|_| fail("replay_input_invalid"))?;
    let options = Options {
        date: values
            .get("--date")
            .cloned()
            .unwrap_or_else(|| "2000-01-01".into()),
        max_requests: get("--max-requests")?
            .parse()
            .map_err(|_| fail("replay_input_invalid"))?,
        no_detail,
        spool: get("--spool")?.into(),
        raw_dir: None,
    };
    let admission = TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "offline-replay".into(),
        teacher_id: identity.teacher_source_key.clone(),
        teacher_label: identity.teacher_label.clone(),
        college_id: identity.college_source_key.clone(),
        allowed_from: options.date.clone(),
        allowed_through: options.date.clone(),
        student_ids: students.iter().map(|s| s.student_id.clone()).collect(),
        source_namespaces: vec![COURSE_NS.into(), METADATA_NS.into()],
        operations: capture::contracts(),
    };
    let roster = Roster {
        students,
        paper_form,
    };
    capture::preflight(&admission, &roster, &options)
        .map_err(|e| capture::context(e, "replay_input_invalid"))?;
    let raw_root = fs::canonicalize(get("--raw-dir")?).map_err(|_| fail("replay_index_refused"))?;
    let spool_parent = options
        .spool
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .ok_or_else(|| fail("replay_spool_refused"))?;
    if spool_parent.starts_with(&raw_root) {
        return Err(fail("replay_spool_refused"));
    }
    // Exclusive creation, even when no student can be published.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&options.spool)
        .map_err(|_| fail("replay_spool_refused"))?;
    let mut reports = vec![];
    for (i, student) in roster.students.iter().enumerate() {
        let dir = raw_root.join(&student.student_id);
        let bytes = read(&dir.join("index.jsonl"), 16_777_216, "replay_index_refused")?;
        let text = std::str::from_utf8(&bytes).map_err(|_| fail("replay_index_refused"))?;
        let mut entries = vec![];
        for (n, line) in text.lines().enumerate() {
            let entry: Entry =
                serde_json::from_str(line).map_err(|_| fail("replay_index_refused"))?;
            if entry.sequence != (n + 1) as u64
                || !capture::contracts()
                    .iter()
                    .any(|op| op.id == entry.operation)
                || !(100..=599).contains(&entry.http_status)
            {
                return Err(fail("replay_index_refused"));
            }
            entries.push(entry);
        }
        let principal = obj(&[
            ("teacherKey", identity.teacher_source_key.as_str().into()),
            ("teacherLabel", identity.teacher_label.as_str().into()),
            ("collegeKey", identity.college_source_key.as_str().into()),
        ]);
        let mut client = http::Client::new(principal, Saved { response: None })?;
        client.authenticated = true;
        let mut port = ReplayPort {
            admission: &admission,
            entries,
            dir,
            client,
            answered: 0,
            missing: None,
        };
        let single = Roster {
            students: vec![Student {
                student_id: student.student_id.clone(),
                student_name: student.student_name.clone(),
            }],
            paper_form: roster.paper_form.clone(),
        };
        let mut r = capture::capture_with(&mut port, &single, &options, runtime).remove(0);
        r.roster_ordinal = i + 1;
        r.requests = port.answered;
        r.operation = port.missing;
        reports.push(r);
    }
    Ok(json!({"students":reports}))
}
