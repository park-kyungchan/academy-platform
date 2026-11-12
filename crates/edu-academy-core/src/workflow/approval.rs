use super::{
    Runtime, canonical, codec, io_fault,
    state::*,
    store::{Kind, Store, mkdir},
};
use crate::js::{Js, Result, obj, sha};
use std::{
    collections::HashSet, fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt, path::Path,
};
pub trait Approval {
    fn consume(&mut self, action: &Js) -> Result<Js>;
}
pub struct RequestApproval<'a> {
    pub store: &'a Store,
    pub input: Js,
    pub runtime: &'a dyn Runtime,
}
fn rejected() -> Js {
    obj(&[("status", "rejected".into())])
}
impl Approval for RequestApproval<'_> {
    fn consume(&mut self, a: &Js) -> Result<Js> {
        let s = self.input.get("scope");
        let Some(p) = self
            .input
            .get("reviewed")
            .array()
            .iter()
            .find(|p| p.get("field") == a.get("field"))
        else {
            return Ok(rejected());
        };
        super::schema::effect_binding(p)?;
        super::schema::effect_binding(a)?;
        if a.get("effectTarget") != p.get("effectTarget") {
            return Err(crate::js::Fault::error("workflow_effect_target_drift"));
        }
        if a.get("target").get("studentKey") != s.get("studentId")
            || a.get("target").get("teacherKey") != s.get("teacherId")
            || a.get("target").get("date") != s.get("lessonDate")
            || a.get("effectRecordKey") != p.get("effectRecordKey")
            || a.get("before") != p.get("before")
            || a.get("after") != p.get("after")
        {
            return Ok(rejected());
        }
        let key = checked_execution_key(self.store, self.input.get("requestRef"), s, p)?;
        let digest = sha(&canonical(a)?);
        self.store.add_ledger(&lock_key(s)?, &key, self.runtime)?;
        self.runtime.indexed()?;
        let record = obj(&[
            ("effectBindingVersion", Js::Num(2.0)),
            ("effectTarget", p.get("effectTarget").clone()),
            ("requestRef", self.input.get("requestRef").clone()),
            ("scope", s.clone()),
            ("field", p.get("field").clone()),
            ("attribution", self.input.get("attribution").clone()),
            ("reviewDigest", self.input.get("reviewDigest").clone()),
            ("attemptId", a.get("attemptId").clone()),
            ("effectRecordKey", a.get("effectRecordKey").clone()),
            (
                "beforeSha256",
                sha(&String::from_utf16_lossy(a.get("before").units())).into(),
            ),
            (
                "afterSha256",
                sha(&String::from_utf16_lossy(a.get("after").units())).into(),
            ),
            ("actionDigest", digest.clone().into()),
            ("recordedAt", self.runtime.wall_now().into()),
        ]);
        let created = self
            .store
            .publish(Kind::Execution, &key, &record, self.runtime)?
            .0;
        if created {
            self.runtime.fenced()?;
        }
        Ok(if created {
            obj(&[
                ("status", "verified".into()),
                ("actionDigest", digest.into()),
            ])
        } else {
            rejected()
        })
    }
}
/// The owner re-reads the current user event on each consumption.
pub trait CurrentUser {
    fn read_current(&mut self) -> Result<Js>;
}
pub trait ConsumeOwner {
    fn consume_once(&mut self, key: &str, record: &Js) -> Result<bool>;
}
/// An owner-issued transaction handle. Its lifetime holds the same transaction
/// used for *all* instruction/session/source/grant mutations, including
/// revocation. Reading and existing consume-once share this one handle.
pub trait AttributionTransaction: ConsumeOwner {
    fn lookup(&mut self, request_ref: &str) -> Result<Option<crate::attribution::Instruction>>;
    fn now_ms(&self) -> i64;
}
/// The actual owner begins its existing mutation transaction and returns its
/// guard. Dropping the handle ends the transaction. No split current/store
/// adapter or fallback serialization is provided by core.
pub trait AttributionStore {
    type Transaction<'a>: AttributionTransaction
    where
        Self: 'a;
    fn transaction(&mut self) -> Result<Self::Transaction<'_>>;
}

pub struct AttributionOwner<'a, S: AttributionStore> {
    store: &'a mut S,
}
impl<'a, S: AttributionStore> AttributionOwner<'a, S> {
    pub fn new(store: &'a mut S) -> Self {
        Self { store }
    }
}
impl<S: AttributionStore> crate::attribution::Owner for AttributionOwner<'_, S> {
    fn compare_and_consume(
        &mut self,
        claim: &crate::attribution::Claim<'_>,
    ) -> Result<crate::attribution::Consumption> {
        use crate::attribution::{Consumption, Rejection};
        let mut transaction = self.store.transaction()?;
        let actual = transaction.lookup(&claim.request_ref())?;
        if let Err(reason) = claim.check(actual.as_ref(), transaction.now_ms()) {
            return Ok(Consumption::Refused(reason));
        }
        Ok(
            if transaction.consume_once(&claim.consumption_key(), &claim.record())? {
                Consumption::Consumed
            } else {
                Consumption::Refused(Rejection::Replay)
            },
        )
    }
}
pub struct DurableConsume<'a> {
    pub root: &'a Path,
}
impl ConsumeOwner for DurableConsume<'_> {
    fn consume_once(&mut self, key: &str, record: &Js) -> Result<bool> {
        mkdir(self.root)?;
        let mut f = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.root.join(key.to_owned() + ".json"))
        {
            Ok(f) => f,
            Err(_) => return Ok(false),
        };
        f.write_all(&codec::bytes(record)?)
            .and_then(|()| f.sync_all())
            .map_err(io_fault)?;
        Ok(true)
    }
}
pub struct LaunchApproval<'a> {
    pub admission: Js,
    pub current: &'a mut dyn CurrentUser,
    pub owner: &'a mut dyn ConsumeOwner,
    used: HashSet<String>,
    admitted: HashSet<String>,
}
fn tuple(a: &Js) -> Js {
    obj(&[
        ("studentId", a.get("target").get("studentKey").clone()),
        ("lessonDate", a.get("target").get("date").clone()),
        ("field", a.get("field").clone()),
        ("effectRecordKey", a.get("effectRecordKey").clone()),
        ("after", a.get("after").clone()),
    ])
}
impl<'a> LaunchApproval<'a> {
    pub fn new(
        admission: Js,
        current: &'a mut dyn CurrentUser,
        owner: &'a mut dyn ConsumeOwner,
    ) -> Result<Self> {
        let admission = crate::schema::validate("dailyRecordSaveAdmissionSchema", &admission)?;
        let mut admitted = HashSet::new();
        for v in admission.get("admitted").array() {
            let t = obj(&[
                ("studentId", v.get("studentId").clone()),
                ("lessonDate", v.get("lessonDate").clone()),
                ("field", v.get("field").clone()),
                ("effectRecordKey", v.get("effectRecordKey").clone()),
                ("after", v.get("after").clone()),
            ]);
            if !admitted.insert(crate::js::digest(&t, crate::js::Family::Cism)?) {
                return Err(crate::js::Fault::error(
                    "dailyrecord_save_duplicate_admitted_action",
                ));
            }
        }
        Ok(Self {
            admission,
            current,
            owner,
            used: HashSet::new(),
            admitted,
        })
    }
}
impl Approval for LaunchApproval<'_> {
    fn consume(&mut self, a: &Js) -> Result<Js> {
        let tuple = tuple(a);
        let key = crate::js::digest(&tuple, crate::js::Family::Cism)?;
        if !self.admitted.contains(&key) || self.used.contains(&key) {
            return Ok(rejected());
        }
        let actual = self.current.read_current()?;
        if [
            "runtime",
            "requestRef",
            "requestTextSha256",
            "currentUserEventRef",
        ]
        .iter()
        .any(|k| actual.get(k) != self.admission.get(k))
        {
            return Ok(rejected());
        }
        let record = super::merge(obj(&[("request", actual)]), &tuple);
        let consumed = self
            .owner
            .consume_once(&("dailyrecord-save-".to_owned() + &key), &record)?;
        self.used.insert(key);
        Ok(if consumed {
            obj(&[
                ("status", "verified".into()),
                ("actionDigest", sha(&canonical(a)?).into()),
            ])
        } else {
            rejected()
        })
    }
}

/// Original live owner-evidence reader: explicit path, fresh read per consume,
/// 16KiB guard and the retained current-request schema. No cached admission echo.
pub struct FileCurrent<'a> {
    pub path: &'a Path,
}
impl CurrentUser for FileCurrent<'_> {
    fn read_current(&mut self) -> Result<Js> {
        let bytes = std::fs::read(self.path)
            .map_err(|_| crate::js::Fault::error("dailyrecord_save_current_request_unavailable"))?;
        if bytes.len() > 16384 {
            return Err(crate::js::Fault::error(
                "dailyrecord_save_current_request_unavailable",
            ));
        }
        crate::schema::validate(
            "dailyRecordSaveRequestEvidenceSchema",
            &codec::parse(&bytes)?,
        )
    }
}
