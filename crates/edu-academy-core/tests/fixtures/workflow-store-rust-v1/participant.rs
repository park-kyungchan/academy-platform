//! Named migration operations with explicit roots and typed owner-port callbacks.
//! No native data root, account, policy or source transport defaults.
use edu_academy_core::{
    js::{Fault, Js, Result, obj, stringify},
    workflow::{
        Runtime,
        approval::{self, Approval},
        cism, codec, http, native, review, service, state,
        store::{Kind, LockOptions, Store},
    },
};
use std::{
    cell::RefCell,
    fs::File,
    io::{self, BufRead, Read, Write},
    path::Path,
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const MAX: usize = 4 * 1024 * 1024;
fn read_frame() -> Result<Js> {
    let mut b = vec![];
    io::stdin()
        .lock()
        .take(MAX as u64 + 1)
        .read_until(b'\n', &mut b)
        .map_err(|_| Fault::error("workflow_port_read_failed"))?;
    if b.len() > MAX || b.last() != Some(&b'\n') {
        return Err(Fault::error("workflow_port_frame_rejected"));
    }
    codec::parse(&b)
}
fn emit(v: &Js) -> Result<()> {
    let s = stringify(v)?;
    if s.len() > MAX {
        return Err(Fault::error("workflow_port_output_bound"));
    }
    let mut o = io::stdout().lock();
    writeln!(o, "{s}")
        .and_then(|()| o.flush())
        .map_err(|_| Fault::error("workflow_port_write_failed"))
}
struct Channel;
impl Channel {
    fn call(&mut self, n: &str, v: Js) -> Result<Js> {
        emit(&obj(&[("port", n.into()), ("input", v)]))?;
        let r = read_frame()?;
        if let Js::Num(code) = r.get("abort") {
            std::process::exit(*code as i32);
        }
        if r.has("error") {
            return Err(Fault::error(r.get("error").code_units()));
        }
        if !r.has("value") {
            return Err(Fault::error("workflow_port_reply_rejected"));
        }
        Ok(r.get("value").clone())
    }
}
type Link = Rc<RefCell<Channel>>;
struct Clock {
    now: String,
    link: Link,
}
fn epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("UTC clock")
        .as_millis() as u64
}
fn iso(ms: u64) -> String {
    let mut days = ms / 86400000;
    let mut y = 1970;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    loop {
        let n = if leap(y) { 366 } else { 365 };
        if days < n {
            break;
        }
        days -= n;
        y += 1;
    }
    let months = [
        31,
        if leap(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    while days >= months[m] {
        days -= months[m];
        m += 1;
    }
    format!(
        "{y:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        m + 1,
        days + 1,
        ms / 3600000 % 24,
        ms / 60000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}
impl Runtime for Clock {
    fn now(&self) -> String {
        self.now.clone()
    }
    fn wall_now(&self) -> String {
        iso(epoch())
    }
    fn epoch_ms(&self) -> u64 {
        epoch()
    }
    fn uuid(&self) -> Result<String> {
        let mut b = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut b))
            .map_err(|_| Fault::error("workflow_entropy_unavailable"))?;
        b[6] = (b[6] & 15) | 64;
        b[8] = (b[8] & 63) | 128;
        let h = b.iter().map(|b| format!("{b:02x}")).collect::<String>();
        Ok(format!(
            "{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..]
        ))
    }
    fn indexed(&self) -> Result<()> {
        self.link.borrow_mut().call("durable_index", Js::Null)?;
        Ok(())
    }
    fn fenced(&self) -> Result<()> {
        self.link.borrow_mut().call("durable_fence", Js::Null)?;
        Ok(())
    }
}
struct Source {
    link: Link,
}
impl service::Source for Source {
    fn read(&mut self, s: &Js, r: &Js, t: &Js) -> Result<Js> {
        self.link.borrow_mut().call(
            "read_authoritative",
            obj(&[
                ("scope", s.clone()),
                ("requestRef", r.clone()),
                ("subject", t.clone()),
            ]),
        )
    }
}
struct Policy {
    link: Link,
    parent: Js,
    learning: Js,
}
impl review::Policy for Policy {
    fn parent_pins(&self) -> Js {
        self.parent.clone()
    }
    fn learning_pins(&self) -> Js {
        self.learning.clone()
    }
    fn check_lines(&self, v: &Js) -> bool {
        self.link
            .borrow_mut()
            .call("parent_check_lines", v.clone())
            .is_ok_and(|v| v == Js::Bool(true))
    }
    fn excluded_parent_language(&self, v: &Js) -> bool {
        self.link
            .borrow_mut()
            .call("parent_excluded", v.clone())
            .map_or(true, |v| v == Js::Bool(true))
    }
    fn scope_qualified(&self, v: &Js) -> bool {
        self.link
            .borrow_mut()
            .call("learning_scope_qualified", v.clone())
            .is_ok_and(|v| v == Js::Bool(true))
    }
    fn learning_review(&self, s: &Js, v: &Js, e: &Js) -> Vec<Js> {
        self.link
            .borrow_mut()
            .call(
                "learning_review",
                obj(&[
                    ("scope", s.clone()),
                    ("value", v.clone()),
                    ("entries", e.clone()),
                ]),
            )
            .map_or_else(
                |_| vec!["canonical_learning_policy_not_connected".into()],
                |v| v.array().to_vec(),
            )
    }
    fn learning_suggest(&self, s: &Js, v: &Js, e: &Js) -> Js {
        self.link
            .borrow_mut()
            .call(
                "learning_suggest",
                obj(&[
                    ("scope", s.clone()),
                    ("value", v.clone()),
                    ("entries", edu_academy_core::workflow::nullable(e)),
                ]),
            )
            .unwrap_or(Js::Null)
    }
}
struct Native {
    link: Link,
}
impl http::HttpPort for Native {
    fn read(&mut self, v: &Js) -> Result<Js> {
        self.link.borrow_mut().call("http_read", v.clone())
    }
    fn write(&mut self, v: &Js) -> Result<()> {
        self.link.borrow_mut().call("http_write", v.clone())?;
        Ok(())
    }
}
struct ObservedApproval<'a> {
    inner: &'a mut dyn Approval,
    link: Link,
}
impl Approval for ObservedApproval<'_> {
    fn consume(&mut self, a: &Js) -> Result<Js> {
        let d = self.inner.consume(a)?;
        self.link
            .borrow_mut()
            .call("approval_observed", d.clone())?;
        Ok(d)
    }
}
impl native::SourcePort for Native {
    fn authenticate(&mut self) -> Result<()> {
        self.link.borrow_mut().call("authenticate", Js::Null)?;
        Ok(())
    }
    fn discover_groups(&mut self, v: &Js) -> Result<Js> {
        self.link.borrow_mut().call("discover_groups", v.clone())
    }
    fn execute(&mut self, v: &Js) -> Result<Js> {
        self.link.borrow_mut().call("execute", v.clone())
    }
    fn journal_state(&mut self, v: &Js) -> Result<Js> {
        self.link.borrow_mut().call("journal_state", v.clone())
    }
    fn apply(&mut self, a: &Js, p: &mut dyn Approval) -> Result<Js> {
        http::validate_action(a)?;
        self.link.borrow_mut().call("action_observed", a.clone())?;
        let mut p = ObservedApproval {
            inner: p,
            link: self.link.clone(),
        };
        http::apply(a, self, &mut p)
    }
}
struct Cism {
    link: Link,
}
impl cism::Backend for Cism {
    fn read_report(&mut self, v: &Js) -> Result<Js> {
        self.link.borrow_mut().call("cism_read", v.clone())
    }
    fn apply_field(&mut self, v: &Js, a: &mut dyn FnMut() -> Result<bool>) -> Result<Js> {
        if !a()? {
            return Err(Fault::error("cism_review_required"));
        }
        self.link.borrow_mut().call("cism_apply", v.clone())
    }
}
fn operate(r: &Js) -> Result<Js> {
    let root_s = r.get("root").code_units();
    let owner_s = r.get("ownerRoot").code_units();
    let root = Path::new(&root_s);
    let owner = Path::new(&owner_s);
    let link = Rc::new(RefCell::new(Channel));
    let clock = Clock {
        now: r.get("now").code_units(),
        link: link.clone(),
    };
    if edu_academy_core::instant::epoch_ms(
        &clock.now.clone().into(),
        edu_academy_core::instant::Grammar::DailyRecord,
    )
    .is_none()
    {
        return Err(Fault::error("workflow_clock_required"));
    }
    let op = r.get("operation").code_units();
    let input = r.get("input");
    if op.starts_with("cism_") {
        if !root.is_absolute()
            || !root.starts_with(owner)
            || std::fs::canonicalize(owner).map_err(|_| Fault::error("cism_store_unqualified"))?
                != owner
        {
            return Err(Fault::error("cism_store_unqualified"));
        }
        let mut p = Cism { link };
        let mut s = cism::Service {
            root,
            backend: &mut p,
            runtime: &clock,
        };
        return match op.as_str() {
            "cism_get" => s.get(input),
            "cism_review" => s.review(input),
            "cism_save" => s.save(input),
            _ => Err(Fault::error("workflow_operation_unknown")),
        };
    }
    let store = Store::open(root, owner)?;
    match op.as_str() {
        "store_get" => store.get(
            Kind::parse(&input.get("kind").code_units())?,
            &input.get("id").code_units(),
        ),
        "store_publish" => {
            let (c, v) = store.publish(
                Kind::parse(&input.get("kind").code_units())?,
                &input.get("id").code_units(),
                input.get("record"),
                &clock,
            )?;
            Ok(obj(&[("created", c.into()), ("record", v)]))
        }
        "ledger_add" => {
            store.add_ledger(
                &input.get("group").code_units(),
                &input.get("key").code_units(),
                &clock,
            )?;
            Ok(Js::Null)
        }
        "approval" => {
            let mut a = approval::RequestApproval {
                store: &store,
                input: input.get("binding").clone(),
                runtime: &clock,
            };
            a.consume(input.get("action"))
        }
        "http_action" => {
            let mut p = Native { link: link.clone() };
            let mut a = approval::RequestApproval {
                store: &store,
                input: input.get("binding").clone(),
                runtime: &clock,
            };
            http::apply(input.get("action"), &mut p, &mut a)
        }
        "launch_approval" => {
            let p = input.get("consumeRoot").code_units();
            let current_path = input.get("currentUserPath").code_units();
            let mut c = approval::FileCurrent {
                path: Path::new(&current_path),
            };
            let mut owner = approval::DurableConsume {
                root: Path::new(&p),
            };
            let mut a =
                approval::LaunchApproval::new(input.get("admission").clone(), &mut c, &mut owner)?;
            let mut out = vec![];
            for v in input.get("actions").array() {
                out.push(a.consume(v)?);
            }
            Ok(Js::Arr(out))
        }
        "native_save" => {
            let mut p = Native { link };
            let mut b = native::NativeBackend::new(
                &mut p,
                r.get("admission").clone(),
                r.get("ownerAdmission").clone(),
                r.get("managedNames").array().to_vec(),
                &clock,
            );
            let mut a = approval::RequestApproval {
                store: &store,
                input: input.get("binding").clone(),
                runtime: &clock,
            };
            service::Backend::save(
                &mut b,
                input.get("scope"),
                input.get("patches"),
                &mut a,
                input.get("subject"),
            )
        }
        "field_state" => Ok(state::field_state(&store, &input.get("key").code_units())?
            .name()
            .into()),
        "executions" => Ok(Js::Arr(
            state::student_executions(&store, input.get("scope"))?
                .iter()
                .map(|e| e.value())
                .collect(),
        )),
        "status" => service::status(&store, input),
        "lock" => {
            let ms = |k: &str, d: u64| match input.get(k) {
                Js::Num(n) if n.is_finite() && *n >= 0.0 => *n as u64,
                _ => d,
            };
            let _g = store.lock(
                &input.get("key").code_units(),
                LockOptions {
                    wait: Duration::from_millis(ms("waitMs", 30000)),
                    poll: Duration::from_millis(ms("pollMs", 20)),
                },
                &|| false,
            )?;
            link.borrow_mut().call("lock_acquired", Js::Null)?;
            Ok(obj(&[("status", "acquired".into())]))
        }
        "review" | "save" | "reconcile" => {
            let mut source = Source { link: link.clone() };
            let policy = Policy {
                link: link.clone(),
                parent: r.get("parentPolicy").clone(),
                learning: r.get("learningPolicy").clone(),
            };
            let mut port = Native { link };
            let mut b = native::NativeBackend::new(
                &mut port,
                r.get("admission").clone(),
                r.get("ownerAdmission").clone(),
                r.get("managedNames").array().to_vec(),
                &clock,
            );
            let mut s = service::Service {
                store: &store,
                source: &mut source,
                policy: &policy,
                backend: &mut b,
                runtime: &clock,
            };
            match op.as_str() {
                "review" => s.review(input, true),
                "save" => s.save(input),
                _ => s.reconcile(input),
            }
        }
        _ => Err(Fault::error("workflow_operation_unknown")),
    }
}
pub fn main() {
    let r = read_frame().and_then(|r| operate(&r));
    let v = match r {
        Ok(v) => obj(&[("result", v)]),
        Err(e) => obj(&[("error", e.value())]),
    };
    if emit(&v).is_err() {
        std::process::exit(2);
    }
}
