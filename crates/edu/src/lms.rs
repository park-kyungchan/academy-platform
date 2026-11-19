//! The capture of dates: the family `dailyrecord`'s own functions over one
//! `NativeSession` (one login, the account lease for the whole run), its credential read
//! from the owner's secret file by `SecretFileCredentials`.
use crate::{Activity, Answer, config::Config, refuse};
use edu_academy_core::workflow::Runtime;
use edu_academy_sources::{
    capture::{Recorder, Roster},
    dailyrecord::{self, Options},
    data::Result,
    http::{Cancellation, Request, Response, Transport},
    owner_admission::TransportAdmission,
    owner_root::{AccountLease, OwnerRoot},
    owner_workflow::NativeSession,
    prepare_owner::SecretFileCredentials,
};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

/// Counts every request of the run, login exchanges included.
struct Counting<T> {
    inner: T,
    count: Rc<Cell<usize>>,
}
impl<T: Transport> Transport for Counting<T> {
    fn bind_account(&mut self, lease: Rc<AccountLease>) {
        self.inner.bind_account(lease);
    }
    fn dispatch(&mut self, request: &Request) -> Result<Response> {
        self.count.set(self.count.get() + 1);
        self.inner.dispatch(request)
    }
}
fn options(config: &Config, dates: &[String]) -> Options {
    Options {
        dates: dates.to_vec(),
        max_requests: dates.len(),
        spool: config.spool.clone(),
        raw_dir: config.raw_dir.clone(),
    }
}
/// The family makes the spool's two private directories that an index run needs.
pub(crate) fn prepare_spool(config: &Config) -> Answer<()> {
    let today = [String::from("2000-01-01")];
    Ok(dailyrecord::initialize(&options(config, &today))?)
}
pub(crate) fn capture<T: Transport>(
    config: &Config,
    dates: &[String],
    clock: &dyn Runtime,
    connect: &mut dyn FnMut(&Cancellation) -> Result<T>,
    activity: &mut Activity,
) -> Answer<()> {
    let count = Rc::new(Cell::new(0));
    let result = session_run(config, dates, clock, connect, activity, &count);
    activity.requests = count.get();
    activity.login = activity.requests > 0;
    let reports = result?;
    match reports.iter().find(|r| r.status != "published") {
        Some(report) => Err(refuse(
            report.code.as_deref().unwrap_or("capture_input_invalid"),
        )),
        None => Ok(()),
    }
}
fn session_run<T: Transport>(
    config: &Config,
    dates: &[String],
    clock: &dyn Runtime,
    connect: &mut dyn FnMut(&Cancellation) -> Result<T>,
    activity: &mut Activity,
    count: &Rc<Cell<usize>>,
) -> Answer<Vec<dailyrecord::DateReport>> {
    let root = Rc::new(OwnerRoot::open(&config.owner_root)?);
    let admission = root
        .read(
            "transport-admission.json",
            &config.admission_sha256,
            256 * 1024,
        )?
        .bytes;
    let roster = root.read("roster.json", &config.roster_sha256, 1_048_576)?;
    let roster: Roster =
        serde_json::from_slice(&roster.bytes).map_err(|_| refuse("lesson_roster_refused"))?;
    let options = options(config, dates);
    dailyrecord::preflight(
        &TransportAdmission::from_bytes(&admission)?,
        &roster.students,
        &options,
    )?;
    let cancellation = Cancellation::default();
    let recorder = Recorder::new(options.raw_dir.clone(), clock);
    let wait = Duration::from_secs(config.lock_wait_seconds);
    let start = Instant::now();
    let session = loop {
        let transport = Counting {
            inner: connect(&cancellation)?,
            count: Rc::clone(count),
        };
        let provider = SecretFileCredentials {
            identity: config.identity_file.clone(),
            secret_file: config.secret_file.clone(),
            secret_key: config.secret_key.clone(),
        };
        let opened = NativeSession::open(
            Rc::clone(&root),
            TransportAdmission::from_bytes(&admission)?,
            recorder.transport(transport),
            provider,
            cancellation.clone(),
            clock,
        );
        let busy = matches!(&opened, Err(e) if e.message == "owner_account_busy");
        if busy || activity.waited > 0 {
            activity.waited = start.elapsed().as_secs().max(1);
        }
        if busy && start.elapsed() < wait {
            std::thread::sleep((wait - start.elapsed()).min(Duration::from_millis(250)));
            continue;
        }
        break opened?;
    };
    let mut port = recorder.port(session);
    Ok(dailyrecord::run(
        &mut port,
        &roster.students,
        &options,
        clock,
    ))
}
