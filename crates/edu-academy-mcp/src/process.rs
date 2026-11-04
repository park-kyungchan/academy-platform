//! One owned native request. Library construction never installs global handlers.
use crate::{
    Error, Js, Result,
    endpoint::LaunchPlan,
    obj,
    release::{ConnectionGate, ReleaseGraph},
};
use rustix::{
    fd::OwnedFd,
    process::{Pid, PidfdFlags, Signal, WaitOptions},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};
pub const CANCELLATION_GAP: &str = "safe_process_cancellation_unqualified";
pub fn require_supervised_execution() -> Result<()> {
    Err(Error::new(CANCELLATION_GAP))
}
pub const TICK: Duration = Duration::from_millis(5);
const PROCESS_LIMIT: usize = 256;
#[derive(Clone, Copy)]
pub struct Limits {
    pub work: Duration,
    pub cleanup: Duration,
    pub graceful: Duration,
    pub term: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            work: Duration::from_secs(30),
            cleanup: Duration::from_secs(2),
            graceful: Duration::from_millis(50),
            term: Duration::from_millis(150),
        }
    }
}
impl Limits {
    pub fn validate(self) -> Result<Self> {
        if self.work.is_zero()
            || self.work > Duration::from_secs(80)
            || self.cleanup.is_zero()
            || self.cleanup > Duration::from_secs(3)
            || self.graceful + self.term >= self.cleanup
        {
            return Err(Error::new("supervision_budget_unqualified"));
        }
        Ok(self)
    }
}
pub trait Control {
    /// Invoke the session's latch before consuming any valid notification,
    /// including cancellation arriving during an upstream conversation.
    fn poll(
        &mut self,
        request_id: &Js,
        before_notification: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()>;
    /// Dedicated isolated product binary only, after subreaper installation.
    fn adopt_orphans(&self) -> bool {
        false
    }
}
pub struct Context<'a> {
    pub graph: &'a ReleaseGraph,
    pub gate: &'a mut ConnectionGate,
    pub control: &'a mut dyn Control,
    pub request_id: &'a Js,
    pub deadline: Instant,
    next_verification: Instant,
}
impl<'a> Context<'a> {
    pub fn new(
        graph: &'a ReleaseGraph,
        gate: &'a mut ConnectionGate,
        control: &'a mut dyn Control,
        request_id: &'a Js,
        deadline: Instant,
    ) -> Self {
        Self {
            graph,
            gate,
            control,
            request_id,
            deadline,
            next_verification: Instant::now(),
        }
    }
    pub fn verify(&mut self) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(Error::new("upstream_deadline_exceeded"));
        }
        self.gate.check(self.graph.verify_unchanged())?;
        self.next_verification = Instant::now() + Duration::from_millis(100);
        Ok(())
    }
    pub fn check(&mut self) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(Error::new("upstream_deadline_exceeded"));
        }
        if Instant::now() >= self.next_verification {
            self.verify()?;
        }
        let graph = self.graph;
        let gate = &mut self.gate;
        self.control.poll(self.request_id, &mut || {
            gate.check(graph.verify_unchanged())
        })?;
        if Instant::now() >= self.deadline {
            return Err(Error::new("upstream_deadline_exceeded"));
        }
        Ok(())
    }
    pub fn pause(&mut self) -> Result<()> {
        self.check()?;
        std::thread::sleep(TICK);
        self.check()
    }
}
#[derive(Clone, Debug)]
pub struct Identity {
    pub pid: u32,
    pub parent: u32,
    pub group: u32,
    pub start: u64,
}
impl Identity {
    pub fn read(pid: u32) -> Option<Self> {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let (_, tail) = stat.rsplit_once(')')?;
        let fields = tail.split_whitespace().collect::<Vec<_>>();
        Some(Self {
            pid,
            parent: fields.get(1)?.parse().ok()?,
            group: fields.get(2)?.parse().ok()?,
            start: fields.get(19)?.parse().ok()?,
        })
    }
    fn same(&self) -> bool {
        Self::read(self.pid).is_some_and(|v| v.start == self.start)
    }
}
fn children(pid: u32) -> Result<Vec<u32>> {
    let tasks = match fs::read_dir(format!("/proc/{pid}/task")) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err(Error::new("owned_process_inventory_unavailable")),
    };
    let mut out = BTreeSet::new();
    for (i, entry) in tasks.enumerate() {
        if i >= PROCESS_LIMIT {
            return Err(Error::new("owned_process_inventory_limit"));
        }
        let path = entry
            .map_err(|_| Error::new("owned_process_inventory_unavailable"))?
            .path()
            .join("children");
        let text = match fs::read_to_string(path) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error::new("owned_process_inventory_unavailable")),
        };
        for word in text.split_whitespace() {
            out.insert(
                word.parse()
                    .map_err(|_| Error::new("owned_process_inventory_unavailable"))?,
            );
            if out.len() > PROCESS_LIMIT {
                return Err(Error::new("owned_process_inventory_limit"));
            }
        }
    }
    Ok(out.into_iter().collect())
}
/// Endpoint binary entry only, before store/source work. The pre-exec hard-kill
/// window is a separately disclosed native proof gap; no unsafe pre_exec.
pub fn child_parent_guard() -> Result<()> {
    let expected = std::env::var("ACADEMY_MCP_PARENT_PID")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| Error::new("owned_parent_identity_required"))?;
    let start = std::env::var("ACADEMY_MCP_PARENT_START")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| Error::new("owned_parent_identity_required"))?;
    rustix::process::set_parent_process_death_signal(Some(Signal::KILL))
        .map_err(|_| Error::new("owned_parent_guard_unavailable"))?;
    let me = Identity::read(std::process::id())
        .ok_or_else(|| Error::new("owned_parent_identity_required"))?;
    if me.parent != expected || Identity::read(expected).is_none_or(|p| p.start != start) {
        return Err(Error::new("owned_parent_identity_changed"));
    }
    Ok(())
}
/// Called only by the dedicated product binary, never the library/test host.
pub fn install_binary_subreaper() -> Result<()> {
    rustix::process::set_child_subreaper(Some(Pid::INIT))
        .map_err(|_| Error::new("owned_subreaper_unavailable"))?;
    if rustix::process::child_subreaper().ok().flatten().is_none() {
        return Err(Error::new("owned_subreaper_unavailable"));
    }
    Ok(())
}
struct OwnedProcess {
    identity: Identity,
    fd: OwnedFd,
}
pub struct OwnedChild {
    child: Child,
    input: Option<ChildStdin>,
    output: Option<ChildStdout>,
    errors: Option<ChildStderr>,
    root: u32,
    parent: Identity,
    root_identity: Option<Identity>,
    baseline: BTreeSet<u32>,
    tracked: BTreeMap<u32, OwnedProcess>,
    adopt: bool,
    cleaned: bool,
    output_eof: bool,
    errors_eof: bool,
    stderr_bytes: u64,
}
#[derive(Clone, Debug)]
pub struct Cleanup {
    pub spawned: bool,
    pub qualified: bool,
    pub root_pid: Option<u32>,
    pub root_start: Option<u64>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub reaped: Vec<(u32, Option<i32>, Option<i32>)>,
    pub stdout_eof: bool,
    pub stderr_eof: bool,
    pub stderr_bytes: u64,
    pub reason: Option<&'static str>,
}
impl Cleanup {
    fn absent() -> Self {
        Self {
            spawned: false,
            qualified: true,
            root_pid: None,
            root_start: None,
            exit_code: None,
            signal: None,
            reaped: vec![],
            stdout_eof: true,
            stderr_eof: true,
            stderr_bytes: 0,
            reason: None,
        }
    }
    pub fn evidence(&self) -> Js {
        let number = |v: Option<i32>| v.map_or(Js::Null, |v| Js::Num(f64::from(v)));
        obj(&[
            ("spawned", self.spawned.into()),
            ("qualified", self.qualified.into()),
            (
                "pid",
                self.root_pid.map_or(Js::Null, |v| Js::Num(f64::from(v))),
            ),
            (
                "startTicks",
                self.root_start.map_or(Js::Null, |v| v.to_string().into()),
            ),
            ("exitCode", number(self.exit_code)),
            ("signal", number(self.signal)),
            ("stdoutEof", self.stdout_eof.into()),
            ("stderrEof", self.stderr_eof.into()),
            ("stderrBytes", Js::Num(self.stderr_bytes as f64)),
            ("reasonCode", self.reason.map_or(Js::Null, Into::into)),
            (
                "reaped",
                Js::Arr(
                    self.reaped
                        .iter()
                        .map(|(pid, code, signal)| {
                            obj(&[
                                ("pid", Js::Num(f64::from(*pid))),
                                ("exitCode", number(*code)),
                                ("signal", number(*signal)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}
pub struct Supervised<T> {
    pub result: Result<T>,
    pub cleanup: Cleanup,
}
impl OwnedChild {
    fn spawn(plan: &LaunchPlan, adopt: bool) -> Result<Self> {
        let parent = Identity::read(std::process::id())
            .ok_or_else(|| Error::new("owned_parent_identity_required"))?;
        let baseline = children(parent.pid)?.into_iter().collect();
        plan.executable.validate()?;
        // Context rechecks all pins before spawning; the endpoint verifies its
        // loaded inode/digest before any domain work. No shell or inherited env.
        let mut child = Command::new(&plan.executable.path)
            .args(&plan.arguments)
            .env_clear()
            .env("ACADEMY_MCP_PARENT_PID", parent.pid.to_string())
            .env("ACADEMY_MCP_PARENT_START", parent.start.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|_| Error::new("owned_child_spawn_failed"))?;
        Ok(Self {
            root: child.id(),
            input: child.stdin.take(),
            output: child.stdout.take(),
            errors: child.stderr.take(),
            child,
            parent,
            root_identity: None,
            baseline,
            tracked: BTreeMap::new(),
            adopt,
            cleaned: false,
            output_eof: false,
            errors_eof: false,
            stderr_bytes: 0,
        })
    }
    fn prepare(&mut self) -> Result<()> {
        fn nonblocking(fd: &impl std::os::fd::AsFd) -> Result<()> {
            let flags =
                rustix::fs::fcntl_getfl(fd).map_err(|_| Error::new("owned_pipe_setup_failed"))?;
            rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)
                .map_err(|_| Error::new("owned_pipe_setup_failed"))
        }
        // No cleanup read may touch a pipe whose nonblocking setup failed.
        let input_ok = self.input.as_ref().is_some_and(|v| nonblocking(v).is_ok());
        let output_ok = self.output.as_ref().is_some_and(|v| nonblocking(v).is_ok());
        let errors_ok = self.errors.as_ref().is_some_and(|v| nonblocking(v).is_ok());
        if !(input_ok && output_ok && errors_ok) {
            self.input.take();
            self.output.take();
            self.errors.take();
            return Err(Error::new("owned_pipe_setup_failed"));
        }
        let identity = Identity::read(self.root)
            .ok_or_else(|| Error::new("owned_child_identity_unavailable"))?;
        if identity.parent != self.parent.pid || identity.group != self.root {
            return Err(Error::new("owned_child_identity_unqualified"));
        }
        self.root_identity = Some(identity.clone());
        self.track(identity)
    }
    fn track(&mut self, identity: Identity) -> Result<()> {
        if self.tracked.contains_key(&identity.pid) {
            return Ok(());
        }
        if self.tracked.len() >= PROCESS_LIMIT {
            return Err(Error::new("owned_process_inventory_limit"));
        }
        let pid = Pid::from_raw(identity.pid as i32)
            .ok_or_else(|| Error::new("owned_child_identity_unqualified"))?;
        let fd = rustix::process::pidfd_open(pid, PidfdFlags::empty())
            .map_err(|_| Error::new("owned_pidfd_unavailable"))?;
        if !identity.same() {
            return Err(Error::new("owned_child_identity_changed"));
        }
        self.tracked
            .insert(identity.pid, OwnedProcess { identity, fd });
        Ok(())
    }
    fn discover(&mut self) -> Result<()> {
        let mut parents = self
            .tracked
            .values()
            .map(|v| v.identity.clone())
            .collect::<Vec<_>>();
        if self.adopt {
            parents.push(self.parent.clone());
        }
        for parent in parents {
            if !parent.same() {
                continue;
            }
            for pid in children(parent.pid)? {
                if self.baseline.contains(&pid) || self.tracked.contains_key(&pid) {
                    continue;
                }
                let Some(identity) = Identity::read(pid) else {
                    continue;
                };
                if identity.parent != parent.pid || !parent.same() {
                    continue;
                }
                if parent.pid == self.parent.pid
                    && identity.start < self.root_identity.as_ref().map_or(u64::MAX, |v| v.start)
                {
                    continue;
                }
                self.track(identity)?;
            }
        }
        Ok(())
    }
    pub fn write(&mut self, bytes: &[u8], context: &mut Context<'_>) -> Result<()> {
        let mut offset = 0;
        while offset < bytes.len() {
            context.check()?;
            self.discover()?;
            self.drain_errors()?;
            match self
                .input
                .as_mut()
                .ok_or_else(|| Error::new("upstream_input_closed"))?
                .write(&bytes[offset..])
            {
                Ok(0) => return Err(Error::new("upstream_input_closed")),
                Ok(n) => offset += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => context.pause()?,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => (),
                Err(_) => return Err(Error::new("upstream_transport_unavailable")),
            }
        }
        Ok(())
    }
    pub fn read(&mut self, bytes: &mut [u8], context: &mut Context<'_>) -> Result<Option<usize>> {
        context.check()?;
        self.discover()?;
        self.drain_errors()?;
        match self
            .output
            .as_mut()
            .ok_or_else(|| Error::new("upstream_output_closed"))?
            .read(bytes)
        {
            Ok(0) => {
                self.output_eof = true;
                Err(Error::new("upstream_output_closed"))
            }
            Ok(n) => Ok(Some(n)),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(_) => Err(Error::new("upstream_transport_unavailable")),
        }
    }
    fn drain_errors(&mut self) -> Result<()> {
        let mut bytes = [0u8; 65536];
        for _ in 0..16 {
            match self
                .errors
                .as_mut()
                .ok_or_else(|| Error::new("owned_pipe_setup_failed"))?
                .read(&mut bytes)
            {
                Ok(0) => {
                    self.errors_eof = true;
                    break;
                }
                Ok(n) => self.stderr_bytes = self.stderr_bytes.saturating_add(n as u64),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(_) => return Err(Error::new("upstream_transport_unavailable")),
            }
        }
        Ok(())
    }
    fn finish(&mut self, limits: Limits) -> Cleanup {
        self.cleaned = true;
        self.input.take();
        let start = Instant::now();
        let deadline = start + limits.cleanup;
        let mut result = Cleanup {
            spawned: true,
            qualified: false,
            root_pid: Some(self.root),
            root_start: self.root_identity.as_ref().map(|v| v.start),
            stdout_eof: false,
            stderr_eof: false,
            ..Cleanup::absent()
        };
        if self.root_identity.is_none() {
            result.reason = Some("owned_child_identity_unavailable");
        }
        if !self.adopt {
            result.reason.get_or_insert("owned_adoption_unqualified");
        }
        let mut root_done = false;
        let mut quiet = 0;
        loop {
            if let Err(e) = self.discover() {
                result.reason.get_or_insert(e.code);
            }
            let elapsed = start.elapsed();
            if elapsed >= limits.graceful {
                let signal = if elapsed >= limits.graceful + limits.term {
                    Signal::KILL
                } else {
                    Signal::TERM
                };
                // The retained direct child anchors its PGID until reaping.
                if !root_done && let Some(pid) = Pid::from_raw(self.root as i32) {
                    let _ = rustix::process::kill_process_group(pid, signal);
                }
                for process in self.tracked.values() {
                    let _ = rustix::process::pidfd_send_signal(&process.fd, signal);
                }
            }
            let mut scratch = [0u8; 65536];
            if let Some(out) = self.output.as_mut() {
                for _ in 0..16 {
                    match out.read(&mut scratch) {
                        Ok(0) => {
                            self.output_eof = true;
                            break;
                        }
                        Ok(_) => (),
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                            ) =>
                        {
                            break;
                        }
                        Err(_) => {
                            result.reason.get_or_insert("owned_output_drain_failed");
                            break;
                        }
                    }
                }
            }
            if let Err(e) = self.drain_errors() {
                result.reason.get_or_insert(e.code);
            }
            if !root_done {
                match self.child.try_wait() {
                    Ok(Some(status)) => {
                        root_done = true;
                        result.exit_code = status.code();
                        result.signal = status.signal();
                        result
                            .reaped
                            .push((self.root, status.code(), status.signal()));
                    }
                    Ok(None) => (),
                    Err(_) => {
                        result.reason.get_or_insert("owned_child_reap_failed");
                    }
                }
            }
            for (pid, process) in &self.tracked {
                if *pid == self.root || result.reaped.iter().any(|v| v.0 == *pid) {
                    continue;
                }
                if Identity::read(*pid).is_some_and(|v| {
                    v.start == process.identity.start && v.parent == self.parent.pid
                }) && let Some(pid_value) = Pid::from_raw(*pid as i32)
                {
                    match rustix::process::waitpid(Some(pid_value), WaitOptions::NOHANG) {
                        Ok(Some((_, status))) => result.reaped.push((
                            *pid,
                            status.exit_status(),
                            status.terminating_signal(),
                        )),
                        Ok(None) => (),
                        Err(_) => {
                            result.reason.get_or_insert("owned_descendant_reap_failed");
                        }
                    }
                }
            }
            let alive = self.tracked.values().any(|v| v.identity.same());
            if root_done && !alive && self.output_eof && self.errors_eof {
                quiet += 1;
            } else {
                quiet = 0;
            }
            if quiet >= 2 {
                result.qualified = result.reason.is_none();
                break;
            }
            if Instant::now() >= deadline {
                result
                    .reason
                    .get_or_insert("owned_cleanup_deadline_exceeded");
                break;
            }
            std::thread::sleep(TICK);
        }
        result.stdout_eof = self.output_eof;
        result.stderr_eof = self.errors_eof;
        result.stderr_bytes = self.stderr_bytes;
        result
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = self.finish(Limits::default());
        }
    }
}
pub fn supervise<T>(
    plan: &LaunchPlan,
    context: &mut Context<'_>,
    limits: Limits,
    conversation: impl FnOnce(&mut OwnedChild, &mut Context<'_>) -> Result<T>,
) -> Supervised<T> {
    let limits = match limits.validate() {
        Ok(v) => v,
        Err(e) => {
            return Supervised {
                result: Err(e),
                cleanup: Cleanup::absent(),
            };
        }
    };
    if let Err(e) = context.check() {
        return Supervised {
            result: Err(e),
            cleanup: Cleanup::absent(),
        };
    }
    let mut child = match OwnedChild::spawn(plan, context.control.adopt_orphans()) {
        Ok(v) => v,
        Err(e) => {
            return Supervised {
                result: Err(e),
                cleanup: Cleanup::absent(),
            };
        }
    };
    let work = child
        .prepare()
        .and_then(|()| conversation(&mut child, context));
    let cleanup = child.finish(limits);
    let result = if cleanup.qualified {
        work
    } else {
        Err(Error::new("owned_cleanup_unqualified"))
    };
    Supervised { result, cleanup }
}
