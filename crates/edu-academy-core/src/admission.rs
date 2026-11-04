//! Pure admission with shared launch input and shallow immutable owner records.
//! The owner port does not implement durable consumption or authorize an effect.
use crate::{js::*, schema};
use std::{cell::RefCell, collections::HashSet, rc::Rc};
pub type SharedRequest = Rc<RefCell<Js>>;
pub trait Owner {
    fn read_current_request(&mut self) -> Result<SharedRequest>;
    fn consume_once(&mut self, key: &str, record: &OwnerRecord) -> Result<bool>;
}
/// Fields are private; the request remains the owner's original mutable reference.
pub struct OwnerRecord {
    request: SharedRequest,
    tuple: Js,
}
impl OwnerRecord {
    pub fn request(&self) -> &SharedRequest {
        &self.request
    }
    pub fn tuple(&self) -> &Js {
        &self.tuple
    }
    pub fn is_shallow_frozen(&self) -> bool {
        true
    }
    pub fn snapshot(&self) -> Js {
        let mut v = obj(&[("request", self.request.borrow().clone())]);
        for (k, x) in self.tuple.props() {
            v.set(&String::from_utf16_lossy(k), x.clone());
        }
        v
    }
}
pub struct Admission {
    launch: SharedRequest,
    admitted: HashSet<String>,
    used: HashSet<String>,
    digest: String,
}
fn tuple(action: &Js) -> Js {
    obj(&[
        ("studentId", action.get("target").get("studentKey").clone()),
        ("lessonDate", action.get("target").get("date").clone()),
        ("field", action.get("field").clone()),
        ("effectRecordKey", action.get("effectRecordKey").clone()),
        ("after", action.get("after").clone()),
    ])
}
fn entry_tuple(entry: &Js) -> Js {
    obj(&[
        ("studentId", entry.get("studentId").clone()),
        ("lessonDate", entry.get("lessonDate").clone()),
        ("field", entry.get("field").clone()),
        ("effectRecordKey", entry.get("effectRecordKey").clone()),
        ("after", entry.get("after").clone()),
    ])
}
impl Admission {
    pub fn new(launch: SharedRequest) -> Result<Self> {
        let input = launch.borrow();
        schema::validate("dailyRecordSaveAdmissionSchema", &input)?;
        let mut admitted = HashSet::new();
        for e in input.get("admitted").array() {
            admitted.insert(digest(&entry_tuple(e), Family::Cism)?);
        }
        if admitted.len() != input.get("admitted").array().len() {
            return Err(Fault::error("dailyrecord_save_duplicate_admitted_action"));
        }
        let digest = digest(&input, Family::Cism)?;
        drop(input);
        Ok(Self {
            launch,
            admitted,
            used: HashSet::new(),
            digest,
        })
    }
    pub fn admission_digest(&self) -> &str {
        &self.digest
    }
    pub fn consume_approval(&mut self, action: &Js, owner: &mut impl Owner) -> Result<Js> {
        let tuple = tuple(action);
        let key = digest(&tuple, Family::Cism)?;
        let rejected = || obj(&[("status", "rejected".into())]);
        if !self.admitted.contains(&key) || self.used.contains(&key) {
            return Ok(rejected());
        }
        let actual = owner.read_current_request()?;
        {
            let actual = actual.borrow();
            let launch = self.launch.borrow();
            for k in [
                "runtime",
                "requestRef",
                "requestTextSha256",
                "currentUserEventRef",
            ] {
                if actual.get(k) != launch.get(k) {
                    return Ok(rejected());
                }
            }
        }
        let record = OwnerRecord {
            request: actual,
            tuple,
        };
        let consumed = owner.consume_once(&format!("dailyrecord-save-{key}"), &record)?;
        self.used.insert(key);
        if consumed {
            Ok(obj(&[
                ("status", "verified".into()),
                ("actionDigest", digest(action, Family::Admission)?.into()),
            ]))
        } else {
            Ok(rejected())
        }
    }
}
