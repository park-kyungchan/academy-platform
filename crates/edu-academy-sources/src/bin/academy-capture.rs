//! Explicit opt-in owner launch. Reads only; stdout is counts and fixed codes.
use edu_academy_core::workflow::Runtime;
use edu_academy_sources::{
    capture::{self, Options, Roster},
    data::{Error, Result},
    http,
    owner_admission::TransportAdmission,
    owner_root::OwnerRoot,
    owner_workflow::{FileCredentialProvider, NativeSession, PinnedInput},
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
    rc::Rc,
    time::UNIX_EPOCH,
};
struct Clock;
impl Runtime for Clock {
    fn epoch_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
    fn now(&self) -> String {
        String::new()
    }
    fn uuid(&self) -> edu_academy_core::js::Result<String> {
        Err(edu_academy_core::js::Fault::error("unsupported"))
    }
}
fn invalid() -> Error {
    Error::refusal("capture_input_invalid")
}
fn run() -> Result<serde_json::Value> {
    let mut args = std::env::args().skip(1);
    let mode = args.next();
    if mode.as_deref() == Some("replay") {
        return edu_academy_sources::replay::run(args, &Clock);
    }
    if mode.as_deref() == Some("propose-paper-form") {
        return edu_academy_sources::propose_paper_form::run(args);
    }
    if mode.as_deref() == Some("prepare-owner") {
        let mut args = args.peekable();
        if args.next_if(|a| a == "--standing").is_some() {
            let (inputs, families) = edu_academy_sources::prepare_owner::parse_standing_args(args)?;
            return edu_academy_sources::prepare_owner::prepare_standing(
                &inputs, &families, &Clock,
            );
        }
        let (inputs, families) = edu_academy_sources::prepare_owner::parse_args_families(args)?;
        return edu_academy_sources::prepare_owner::prepare_families(&inputs, &families, &Clock);
    }
    let daily = mode.as_deref() == Some("dailytest");
    let record = mode.as_deref() == Some("dailyrecord");
    if mode.as_deref() != Some("textbook") && !daily && !record {
        return Err(invalid());
    }
    let mut values = HashMap::new();
    let mut no_detail = false;
    while let Some(arg) = args.next() {
        if arg == "--no-detail" && !daily && !record {
            if no_detail {
                return Err(invalid());
            }
            no_detail = true;
            continue;
        }
        let known = [
            "--raw-dir",
            "--owner-root",
            "--admission",
            "--admission-sha256",
            "--credential",
            "--credential-sha256",
            "--roster",
            "--spool",
            "--date",
            "--max-requests",
        ]
        .contains(&arg.as_str())
            || record && ["--dates", "--roster-sha256"].contains(&arg.as_str());
        if !known
            || values
                .insert(arg, args.next().ok_or_else(invalid)?)
                .is_some()
        {
            return Err(invalid());
        }
    }
    if record {
        return launch_dailyrecord(&values);
    }
    let get = |k: &str| values.get(k).cloned().ok_or_else(invalid);
    let root = Rc::new(OwnerRoot::open(&PathBuf::from(get("--owner-root")?))?);
    let admission = root.read(
        &get("--admission")?,
        &get("--admission-sha256")?,
        256 * 1024,
    )?;
    let admission = TransportAdmission::from_bytes(&admission.bytes)?;
    let roster_file = std::fs::File::open(get("--roster")?).map_err(|_| invalid())?;
    let meta = roster_file.metadata().map_err(|_| invalid())?;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 || meta.nlink() != 1 {
        return Err(invalid());
    }
    let mut bytes = vec![];
    roster_file
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > 1_048_576 {
        return Err(invalid());
    }
    let roster: Roster = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if daily && !roster.paper_form.is_object() && !roster.paper_form.is_null() {
        return Err(invalid());
    }
    let options = Options {
        date: get("--date")?,
        spool: get("--spool")?.into(),
        raw_dir: values.get("--raw-dir").map(PathBuf::from),
        max_requests: get("--max-requests")?.parse().map_err(|_| invalid())?,
        no_detail,
    };
    let refused = if daily {
        edu_academy_sources::dailytest::preflight(&admission, &roster.students, &options)
    } else {
        capture::preflight(&admission, &roster, &options)
    };
    if let Err(error) = refused {
        return Ok(
            serde_json::json!({"status":"capture_admission_refused","students":roster.students.iter().enumerate().map(|(i,_)|serde_json::json!({"roster_ordinal":i+1,"status":"capture_admission_refused","cause":capture::causes(&capture::context(error.clone(), "capture_admission_refused")),"requests":0})).collect::<Vec<_>>()}),
        );
    }
    let provider = FileCredentialProvider::new(
        root.clone(),
        PinnedInput {
            relative_path: get("--credential")?,
            sha256: get("--credential-sha256")?,
        },
        &Clock,
    );
    let cancellation = http::Cancellation::default();
    let transport = http::NativeTransport::new(
        http::TransportLimits {
            request_bytes: http::MAX_REQUEST_BYTES,
            response_bytes: 6_000_001,
            timeout: std::time::Duration::from_secs(20),
        },
        cancellation.clone(),
    )?;
    let recorder = capture::Recorder::new(options.raw_dir.clone(), &Clock);
    let session = NativeSession::open(
        root,
        admission,
        recorder.transport(transport),
        provider,
        cancellation,
        &Clock,
    )
    .map_err(|e| capture::context(e, "capture_owner_refused"))?;
    let mut session = recorder.port(session);
    let students = if daily {
        capture::capture_dailytest(&mut session, &roster.students, &options, &Clock)
    } else {
        capture::capture(&mut session, &roster, &options, &Clock)
    };
    Ok(serde_json::json!({ "students": students }))
}
fn launch_dailyrecord(values: &HashMap<String, String>) -> Result<serde_json::Value> {
    use edu_academy_sources::dailyrecord;
    if values.contains_key("--date") {
        return Err(invalid());
    }
    let get = |k: &str| values.get(k).cloned().ok_or_else(invalid);
    let dates = dailyrecord::dates(&get("--dates")?)?;
    let options = dailyrecord::Options {
        max_requests: values
            .get("--max-requests")
            .map(|s| s.parse())
            .transpose()
            .map_err(|_| invalid())?
            .unwrap_or(dates.len()),
        dates,
        spool: get("--spool")?.into(),
        raw_dir: values.get("--raw-dir").map(PathBuf::from),
    };
    let result = (|| {
        dailyrecord::initialize(&options)?;
        let root_path = PathBuf::from(get("--owner-root")?);
        let root = Rc::new(OwnerRoot::open(&root_path)?);
        let admission = root.read(
            &get("--admission")?,
            &get("--admission-sha256")?,
            256 * 1024,
        )?;
        let admission = TransportAdmission::from_bytes(&admission.bytes)?;
        let roster_path = PathBuf::from(get("--roster")?);
        let relative = if roster_path.is_absolute() {
            roster_path
                .strip_prefix(&root_path)
                .map_err(|_| invalid())?
        } else {
            &roster_path
        };
        let roster = root.read(
            relative.to_str().ok_or_else(invalid)?,
            &get("--roster-sha256")?,
            1_048_576,
        )?;
        let roster: Roster = serde_json::from_slice(&roster.bytes).map_err(|_| invalid())?;
        if let Err(error) = dailyrecord::preflight(&admission, &roster.students, &options) {
            return Ok(
                serde_json::json!({"dates":dailyrecord::refused(&options.dates,&error.message)}),
            );
        }
        let provider = FileCredentialProvider::new(
            root.clone(),
            PinnedInput {
                relative_path: get("--credential")?,
                sha256: get("--credential-sha256")?,
            },
            &Clock,
        );
        let cancellation = http::Cancellation::default();
        let transport = http::NativeTransport::new(
            http::TransportLimits {
                request_bytes: http::MAX_REQUEST_BYTES,
                response_bytes: 6_000_001,
                timeout: std::time::Duration::from_secs(20),
            },
            cancellation.clone(),
        )?;
        let recorder = capture::Recorder::new(options.raw_dir.clone(), &Clock);
        let session = NativeSession::open(
            root,
            admission,
            recorder.transport(transport),
            provider,
            cancellation,
            &Clock,
        )
        .map_err(|e| capture::context(e, "capture_owner_refused"))?;
        let mut session = recorder.port(session);
        Ok(
            serde_json::json!({"dates":dailyrecord::run(&mut session,&roster.students,&options,&Clock)}),
        )
    })();
    result.or_else(|error: Error| {
        Ok(serde_json::json!({"dates":dailyrecord::refused(&options.dates,&error.message)}))
    })
}
fn main() {
    let result = run();
    let (output, exit) = match result {
        Ok(output) => {
            let failed = output["dates"]
                .as_array()
                .is_some_and(|a| a.iter().any(|d| d["status"] != "published"))
                || output["status"] == "capture_admission_refused"
                || output["students"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|s| s["status"] != "refreshed"));
            (output, if failed { 2 } else { 0 })
        }
        Err(error) => {
            let code = if error.message.starts_with("prepare_")
                || error.message.starts_with("propose_")
                || error.message.starts_with("replay_")
            {
                error.message.as_str()
            } else {
                "capture_launch_refused"
            };
            (
                serde_json::json!({"status":code,"cause":capture::causes(&error)}),
                2,
            )
        }
    };
    if writeln!(std::io::stdout(), "{output}").is_err() {
        std::process::exit(2);
    }
    std::process::exit(exit);
}
