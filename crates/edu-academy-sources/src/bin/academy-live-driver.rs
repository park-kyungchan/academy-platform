//! Opt-in native driver. This program has no environment credential discovery.
use edu_academy_core::{
    js::{Fault, Js, Result, obj},
    templates::from_json as j,
    workflow::{
        Runtime, canonical,
        native::{NativeBackend, SourcePort},
        service::Service,
        store::Store,
    },
};
use edu_academy_sources::{
    data::fault_from_error as fault,
    http::{self, Transport},
    live_driver as driver,
    owner_admission::{TransportAdmission, TransportScope},
    owner_root::OwnerRoot,
    owner_workflow::{FileCredentialProvider, Owner, PinnedInput},
};
use serde::Deserialize;
use std::{
    cell::Cell,
    io::{Read, Write},
    path::PathBuf,
    rc::Rc,
    time::{Duration, UNIX_EPOCH},
};

// M/prestudy.rs:52-57; http.rs:3443-3450 supplies the other default limits.
const LIMITS: http::TransportLimits = http::TransportLimits {
    request_bytes: http::MAX_REQUEST_BYTES,
    response_bytes: edu_academy_sources::prestudy::MAX_SOURCE_BYTES + 1,
    timeout: Duration::from_secs(20),
};
struct Clock;
// UTC arithmetic repeats M/audit.rs:98-127. Kernel-generated UUID replaces occurrence():79.
#[rustfmt::skip]
impl Runtime for Clock {
    fn epoch_ms(&self) -> u64 { std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }
    fn uuid(&self) -> Result<String> {
        std::fs::read_to_string("/proc/sys/kernel/random/uuid")
            .map(|s| s.trim().to_owned()).map_err(|_| Fault::error("occurrence_entropy_unavailable"))
    }
    fn now(&self) -> String {
        let ms = self.epoch_ms();
        let z = (ms / 86_400_000) as i64 + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = mp + if mp < 10 { 3 } else { -9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        let second = ms / 1000;
        format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z", second / 3600 % 24, second / 60 % 60, second % 60, ms % 1000)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pin {
    relative_path: String,
    sha256: String,
}
impl Pin {
    fn owned(&self) -> PinnedInput {
        PinnedInput {
            relative_path: self.relative_path.clone(),
            sha256: self.sha256.clone(),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    mode: String,
    scope: serde_json::Value,
    request_ref: Option<String>,
    field: Option<String>,
    value: Option<String>,
    approval: Option<String>,
    reconcile: Option<bool>,
    owner_root: PathBuf,
    admission: Pin,
    credential: Pin,
    launch_admission: Pin,
    store: Option<PathBuf>,
    student_name: Option<String>,
    out: Option<PathBuf>,
}
fn invalid() -> Fault {
    Fault::error("invalid_input")
}
fn parse(bytes: &[u8]) -> Result<(Input, Js)> {
    let input: Input = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let value = j(serde_json::from_slice(bytes).map_err(|_| invalid())?);
    if ["value", "approval"]
        .iter()
        .any(|key| value.has(key) && !matches!(value.get(key), Js::Str(_)))
        || (value.has("reconcile") && !matches!(value.get("reconcile"), Js::Bool(_)))
    {
        return Err(invalid());
    }
    let hash = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if input.mode == "get" {
        return parse_get(input, value, hash);
    }
    if !["plan", "save", "status"].contains(&input.mode.as_str())
        || input.student_name.is_some()
        || input.out.is_some()
        || !input.owner_root.is_absolute()
        || !input.store.as_ref().is_some_and(|s| s.is_absolute())
        || input.field.as_deref().is_none_or(str::is_empty)
        || (input.mode != "status" && input.value.is_none())
        || (input.mode == "save" && !input.approval.as_deref().is_some_and(hash))
        || [&input.admission, &input.credential, &input.launch_admission]
            .iter()
            .any(|p| p.relative_path.is_empty() || !hash(&p.sha256))
    {
        return Err(invalid());
    }
    edu_academy_core::schema::validate("dailyRecordScopeSchema", &j(input.scope.clone()))
        .map_err(|_| invalid())?;
    edu_academy_core::schema::validate(
        "dailyRecordIdSchema",
        &input.request_ref.as_deref().ok_or_else(invalid)?.into(),
    )
    .map_err(|_| invalid())?;
    // Read by serde even when the mode does not use these optional keys.
    let _ = input.reconcile;
    driver::supported(&value)?;
    Ok((input, value))
}
/// `get`: the keys of the mode are exactly these; no write-side key is accepted.
fn parse_get(input: Input, value: Js, hash: impl Fn(&str) -> bool) -> Result<(Input, Js)> {
    let scope = value.get("scope");
    let by_name = input.student_name.is_some();
    let name_ok = input
        .student_name
        .as_deref()
        .is_none_or(|n| !n.trim().is_empty() && n.encode_utf16().count() <= 200);
    if [
        "field",
        "value",
        "approval",
        "store",
        "requestRef",
        "reconcile",
    ]
    .iter()
    .any(|key| value.has(key))
        || !name_ok
        || by_name == scope.has("studentId")
        || !input.owner_root.is_absolute()
        || !input.out.as_ref().is_some_and(|p| p.is_absolute())
        || [&input.admission, &input.credential, &input.launch_admission]
            .iter()
            .any(|p| p.relative_path.is_empty() || !hash(&p.sha256))
    {
        return Err(invalid());
    }
    // A named student has no id in the scope yet: validate it with a stand-in id.
    let mut checked = scope.clone();
    if by_name {
        checked.set("studentId", "1".into());
    }
    edu_academy_core::schema::validate("dailyRecordScopeSchema", &checked)
        .map_err(|_| invalid())?;
    Ok((input, value))
}
struct Tracked<'a>(http::NativeTransport, &'a Cell<bool>, &'a Cell<u32>);
impl Transport for Tracked<'_> {
    fn bind_account(&mut self, lease: Rc<edu_academy_sources::owner_root::AccountLease>) {
        self.0.bind_account(lease);
    }
    fn dispatch(
        &mut self,
        request: &http::Request,
    ) -> edu_academy_sources::data::Result<http::Response> {
        self.1.set(true);
        self.2.set(self.2.get() + 1);
        self.0.dispatch(request)
    }
}
fn run(
    input: &Input,
    value: &Js,
    store: &mut Option<Store>,
    called: &Cell<bool>,
    evidence: &mut Js,
) -> Result<Js> {
    if input.mode == "get" {
        return run_get(input, value, called);
    }
    let root = Rc::new(OwnerRoot::open(&input.owner_root).map_err(fault)?);
    *store = Some(driver::store(
        &input.owner_root,
        input.store.as_ref().ok_or_else(invalid)?,
        &Clock,
    )?);
    let read = |pin: &Pin| {
        root.read(&pin.relative_path, &pin.sha256, 256 * 1024)
            .map_err(fault)
    };
    let admission =
        TransportAdmission::from_bytes(&read(&input.admission)?.bytes).map_err(fault)?;
    let launch = read(&input.launch_admission)?
        .json()
        .map_err(|_| Fault::error("launch_admission_refused"))?;
    validate_launch(&launch, &admission, value.get("scope"))?;
    if !value
        .get("scope")
        .get("sourceNamespace")
        .is("academy.lms.dayrecord")
    {
        return Err(Fault::error("native_transport_scope_refused"));
    }
    admission
        .authorize(&TransportScope {
            owner_id: admission.owner_id.clone(),
            source_namespace: "academy.lms.dayrecord".into(),
            teacher_id: value.get("scope").get("teacherId").code_units(),
            student_id: value.get("scope").get("studentId").code_units(),
            date: value.get("scope").get("lessonDate").code_units(),
        })
        .map_err(fault)?;
    let principal = obj(&[
        ("teacherLabel", admission.teacher_label.as_str().into()),
        ("teacherKey", admission.teacher_id.as_str().into()),
        ("collegeKey", admission.college_id.as_str().into()),
    ]);
    let provider = FileCredentialProvider::new(root.clone(), input.credential.owned(), &Clock);
    let transport = Tracked(
        http::NativeTransport::new(LIMITS, http::Cancellation::default()).map_err(fault)?,
        called,
        &Cell::new(0),
    );
    let owner =
        Owner::leased(root, principal, transport, launch, None, provider, &Clock).map_err(fault)?;
    let (admission, owner_admission, names) = owner.write_configuration()?;
    let mut source = owner.source();
    let mut writer = owner.writer();
    writer.authenticate()?;
    let mut port = driver::EvidencePort {
        inner: &mut writer,
        evidence: Js::Null,
    };
    let mut backend = NativeBackend::new(&mut port, admission, owner_admission, names, &Clock);
    let result = driver::execute(
        value,
        &mut Service {
            store: store.as_ref().ok_or_else(invalid)?,
            source: &mut source,
            backend: &mut backend,
            policy: &driver::DisconnectedPolicy,
            runtime: &Clock,
        },
    );
    *evidence = port.evidence.clone();
    let mut output = result?;
    if input.mode == "save" {
        for key in ["httpStatus", "bodySha256", "comparison"] {
            output.set(
                key,
                edu_academy_core::workflow::nullable(port.evidence.get(key)),
            );
        }
    }
    Ok(output)
}
/// Read-only mode: same owner root, pins, admission checks, lease and one login as
/// `plan`, but no store, no writer and no service. Every refusal that needs no
/// request is decided before the credential is read.
fn run_get(input: &Input, value: &Js, called: &Cell<bool>) -> Result<Js> {
    let out = input.out.as_ref().ok_or_else(invalid)?;
    let root = Rc::new(OwnerRoot::open(&input.owner_root).map_err(fault)?);
    let read = |pin: &Pin| {
        root.read(&pin.relative_path, &pin.sha256, 256 * 1024)
            .map_err(fault)
    };
    let admission =
        TransportAdmission::from_bytes(&read(&input.admission)?.bytes).map_err(fault)?;
    let launch = read(&input.launch_admission)?
        .json()
        .map_err(|_| Fault::error("launch_admission_refused"))?;
    let name: Option<Js> = input.student_name.as_deref().map(Js::from);
    let mut scope = value.get("scope").clone();
    if let Some(name) = &name {
        scope.set("studentId", driver::launch_student_of_name(&launch, name)?);
    }
    validate_launch(&launch, &admission, &scope)?;
    if !scope.get("sourceNamespace").is("academy.lms.dayrecord") {
        return Err(Fault::error("native_transport_scope_refused"));
    }
    admission
        .authorize(&TransportScope {
            owner_id: admission.owner_id.clone(),
            source_namespace: "academy.lms.dayrecord".into(),
            teacher_id: scope.get("teacherId").code_units(),
            student_id: scope.get("studentId").code_units(),
            date: scope.get("lessonDate").code_units(),
        })
        .map_err(fault)?;
    driver::check_out(out)?;
    let principal = obj(&[
        ("teacherLabel", admission.teacher_label.as_str().into()),
        ("teacherKey", admission.teacher_id.as_str().into()),
        ("collegeKey", admission.college_id.as_str().into()),
    ]);
    let provider = FileCredentialProvider::new(root.clone(), input.credential.owned(), &Clock);
    let requests = Cell::new(0);
    let transport = Tracked(
        http::NativeTransport::new(LIMITS, http::Cancellation::default()).map_err(fault)?,
        called,
        &requests,
    );
    let owner =
        Owner::leased(root, principal, transport, launch, None, provider, &Clock).map_err(fault)?;
    let mut source = owner.source();
    let document = driver::get_record(&mut source, &scope, name.as_ref())?;
    let mut output = driver::write_record_file(&document, out)?;
    output.set("requests", Js::Num(f64::from(requests.get())));
    Ok(output)
}
fn validate_launch(launch: &Js, admission: &TransportAdmission, scope: &Js) -> Result<()> {
    let students = launch.get("students").array();
    let mut ids = std::collections::HashSet::new();
    if !matches!(launch, Js::Obj(_))
        || ["teacherId", "teacherLabel", "date", "sourceBundleSha256"]
            .iter()
            .any(|k| !launch.get(k).string())
        || edu_academy_core::schema::validate("dailyRecordDateSchema", launch.get("date")).is_err()
        || students.is_empty()
        || students.iter().any(|s| {
            !matches!(s, Js::Obj(_))
                || !s.get("studentId").string()
                || !s.get("displayName").string()
                || !ids.insert(s.get("studentId").code_units())
        })
        || scope.get("teacherId") != launch.get("teacherId")
        || scope.get("lessonDate") != launch.get("date")
        || !students
            .iter()
            .any(|s| s.get("studentId") == scope.get("studentId"))
        || !launch.get("teacherId").is(&admission.teacher_id)
        || !launch.get("teacherLabel").is(&admission.teacher_label)
    {
        return Err(Fault::error("launch_admission_refused"));
    }
    Ok(())
}
fn main() {
    let called = Cell::new(false);
    let mut store = None;
    let mut evidence = Js::Null;
    let mut bytes = Vec::new();
    let parsed = std::io::stdin()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())
        .and_then(|_| {
            if bytes.len() > 1_048_576 {
                Err(invalid())
            } else {
                parse(&bytes)
            }
        });
    let result = parsed
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|(input, value)| run(input, value, &mut store, &called, &mut evidence));
    // Preserve only the mode for refusal keys when another input key is invalid.
    let fallback = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .map_or(Js::Null, |v| {
            obj(&[(
                "mode",
                v.get("mode")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .into(),
            )])
        });
    let input = parsed.as_ref().map_or(&fallback, |(_, value)| value);
    let stored = store.as_ref().map_or(Js::Null, |store| {
        driver::stored(input, store)
            .unwrap_or_else(|_| obj(&[("code", "store_read_failed".into())]))
    });
    let (output, exit) = driver::response(input, result, stored, &evidence, called.get());
    if exit != 0 {
        eprintln!("{}", output.get("code").code_units());
    }
    let encoded = match canonical(&output) {
        Ok(encoded) => encoded,
        Err(_) => {
            eprintln!("output_failed");
            std::process::exit(if called.get() { 3 } else { 2 });
        }
    };
    if writeln!(std::io::stdout(), "{encoded}").is_err() {
        eprintln!("stdout_failed");
        std::process::exit(if called.get() { 3 } else { 2 });
    }
    std::process::exit(exit);
}
