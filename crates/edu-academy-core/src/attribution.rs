//! Runtime-neutral instruction attribution. No runtime log paths or live effects.
//!
//! The envelope is the strict intersection of the retained JSON Schema and Zod
//! contracts: unique grants, no unknown fields, and at most 256 UTF-16 units.
//! Owner lookup is trusted input, never evidence copied from an adapter envelope.
use crate::{
    instant::{self, Grammar},
    js::{Fault, Js, Result, obj, sha, test, text},
    workflow::canonical,
};
use std::collections::HashSet;

pub const SCHEMA_VERSION: &str = "academy.attribution-envelope/1";

fn malformed() -> Fault {
    Fault::error("attribution_malformed")
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn string(value: &Js) -> Result<String> {
    if !value.string() {
        return Err(malformed());
    }
    String::from_utf16(value.units()).map_err(|_| malformed())
}
fn reference(value: &Js) -> Result<String> {
    let s = string(value)?;
    if value.units().is_empty() || value.units().len() > 256 {
        return Err(malformed());
    }
    Ok(s)
}
fn timestamp(value: &Js) -> Result<i64> {
    // RFC3339 requires seconds; Zod excludes leap seconds and lowercase t/z.
    if !test(
        r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})$",
        value,
    ) {
        return Err(malformed());
    }
    instant::epoch_ms(value, Grammar::DailyRecord).ok_or_else(malformed)
}

/// A detached, validated envelope. Its grants and identity cannot be mutated.
pub struct Envelope {
    value: Js,
    issued_ms: i64,
    expires_ms: i64,
}
impl Envelope {
    pub fn parse(value: &Js) -> Result<Self> {
        let fields = [
            "schemaVersion",
            "runtime",
            "runtimeSessionId",
            "requestRef",
            "requestTextSha256",
            "currentUserEventRef",
            "issuedAt",
            "expiresAt",
            "actionDigests",
        ];
        if !matches!(value, Js::Obj(_)) || value.props().len() != fields.len() {
            return Err(malformed());
        }
        let mut seen = HashSet::new();
        for (key, _) in value.props() {
            if !fields.iter().any(|f| key == &text(f)) || !seen.insert(key) {
                return Err(malformed());
            }
        }
        if !value.get("schemaVersion").is(SCHEMA_VERSION)
            || !["muse", "codex", "hermes"]
                .iter()
                .any(|r| value.get("runtime").is(r))
        {
            return Err(malformed());
        }
        for key in ["runtimeSessionId", "requestRef", "currentUserEventRef"] {
            reference(value.get(key))?;
        }
        if !hash(&string(value.get("requestTextSha256"))?) {
            return Err(malformed());
        }
        let grants = value.get("actionDigests");
        if !matches!(grants, Js::Arr(_)) || !(1..=32).contains(&grants.array().len()) {
            return Err(malformed());
        }
        let mut seen = HashSet::new();
        for grant in grants.array() {
            let grant = string(grant)?;
            if !hash(&grant) || !seen.insert(grant) {
                return Err(malformed());
            }
        }
        Ok(Self {
            issued_ms: timestamp(value.get("issuedAt"))?,
            expires_ms: timestamp(value.get("expiresAt"))?,
            value: value.clone(),
        })
    }
    pub fn value(&self) -> &Js {
        &self.value
    }
    pub fn request_ref(&self) -> String {
        self.value.get("requestRef").code_units()
    }
    fn fresh(&self, now_ms: i64) -> bool {
        self.issued_ms <= now_ms && now_ms < self.expires_ms
    }
}

/// LF joins ordered text blocks; no trimming, newline or Unicode normalization.
pub fn normalized_request_text_hash(blocks: &[String]) -> String {
    sha(&blocks.join("\n"))
}

/// The owner computes this from the actual action and inspected source digest.
/// No optional/null source provenance and no adapter-supplied digest shortcut.
/// The retained envelope contract leaves action hashing to the owner; this
/// explicit Rust binding uses the existing canonical workflow JSON encoder.
pub fn action_digest(action: &Js, source_sha256: &str) -> Result<String> {
    if !hash(source_sha256) {
        return Err(Fault::error("attribution_missing_provenance"));
    }
    Ok(sha(&canonical(&obj(&[
        ("action", action.clone()),
        ("sourceSha256", source_sha256.into()),
    ]))?))
}

/// Fresh, actual-owner evidence for the selected action/source. All fields are
/// obtained from the owner's session/source store, not from the envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub runtime: String,
    pub runtime_session_id: String,
    pub request_ref: String,
    pub current_user_event_ref: String,
    pub text_blocks: Vec<String>,
    pub superseded: bool,
    pub source_sha256: Option<String>,
    /// Actions approved by the actual owner from this instruction. Adapter
    /// envelope grants alone cannot assert the owner's content judgement.
    pub action_digests: Vec<String>,
}

pub trait Owner {
    /// One atomic operation under the owner's mutation transaction: re-read all
    /// instruction/session/grant/source/time bindings, check the claim, then
    /// consume once. There is deliberately no independent owner lookup API.
    /// Errors may follow durable consumption and must not trigger a retry.
    fn compare_and_consume(&mut self, claim: &Claim<'_>) -> Result<Consumption>;
}

pub enum Consumption {
    Consumed,
    Refused(Rejection),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Expired,
    ActionNotBound,
    RequestNotFound,
    Superseded,
    OwnerMismatch,
    CurrentEventMismatch,
    HashMismatch,
    MissingProvenance,
    SourceMismatch,
    Replay,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Verified { action_digest: String },
    Rejected(Rejection),
}

/// Only verification constructs a claim; it is not a reusable approval token.
pub struct Claim<'a> {
    envelope: &'a Envelope,
    action_digest: String,
    source_sha256: &'a str,
}
impl Claim<'_> {
    pub fn action_digest(&self) -> &str {
        &self.action_digest
    }
    pub fn request_ref(&self) -> String {
        self.envelope.request_ref()
    }
    /// Action-digest scope deliberately survives envelope reissue, session
    /// recreation and process restart; timestamps cannot reset consumption.
    pub fn consumption_key(&self) -> String {
        format!("academy-attribution-{}", self.action_digest)
    }
    pub fn record(&self) -> Js {
        obj(&[
            ("request", self.envelope.value.clone()),
            ("actionDigest", self.action_digest.clone().into()),
            ("sourceSha256", self.source_sha256.into()),
        ])
    }
    /// Call only on fresh evidence while holding the owner-issued transaction
    /// through consumption. Success here alone is never a write approval.
    pub fn check(
        &self,
        actual: Option<&Instruction>,
        now_ms: i64,
    ) -> std::result::Result<(), Rejection> {
        if !self.envelope.fresh(now_ms) {
            return Err(Rejection::Expired);
        }
        let e = &self.envelope.value;
        if !e
            .get("actionDigests")
            .array()
            .iter()
            .any(|d| d.is(&self.action_digest))
        {
            return Err(Rejection::ActionNotBound);
        }
        let Some(actual) = actual else {
            return Err(Rejection::RequestNotFound);
        };
        if actual.superseded {
            return Err(Rejection::Superseded);
        }
        if !e.get("runtime").is(&actual.runtime)
            || !e.get("runtimeSessionId").is(&actual.runtime_session_id)
            || !e.get("requestRef").is(&actual.request_ref)
        {
            return Err(Rejection::OwnerMismatch);
        }
        if !e
            .get("currentUserEventRef")
            .is(&actual.current_user_event_ref)
        {
            return Err(Rejection::CurrentEventMismatch);
        }
        if !e
            .get("requestTextSha256")
            .is(&normalized_request_text_hash(&actual.text_blocks))
        {
            return Err(Rejection::HashMismatch);
        }
        let Some(source) = actual.source_sha256.as_deref().filter(|s| hash(s)) else {
            return Err(Rejection::MissingProvenance);
        };
        if source != self.source_sha256 {
            return Err(Rejection::SourceMismatch);
        }
        if !actual.action_digests.contains(&self.action_digest) {
            return Err(Rejection::ActionNotBound);
        }
        Ok(())
    }
}

pub struct Approval {
    envelope: Envelope,
    attempted: HashSet<String>,
}
impl Approval {
    pub fn new(envelope: &Js) -> Result<Self> {
        Ok(Self {
            envelope: Envelope::parse(envelope)?,
            attempted: HashSet::new(),
        })
    }
    pub fn consume(
        &mut self,
        action: &Js,
        source_sha256: &str,
        now_ms: i64,
        owner: &mut impl Owner,
    ) -> Result<Verdict> {
        let digest = action_digest(action, source_sha256)?;
        if self.attempted.contains(&digest) {
            return Ok(Verdict::Rejected(Rejection::Replay));
        }
        if !self.envelope.fresh(now_ms) {
            return Ok(Verdict::Rejected(Rejection::Expired));
        }
        if !self
            .envelope
            .value
            .get("actionDigests")
            .array()
            .iter()
            .any(|d| d.is(&digest))
        {
            return Ok(Verdict::Rejected(Rejection::ActionNotBound));
        }
        let claim = Claim {
            envelope: &self.envelope,
            action_digest: digest.clone(),
            source_sha256,
        };
        // Fence before dispatch: an owner error can follow a durable write.
        self.attempted.insert(digest.clone());
        Ok(match owner.compare_and_consume(&claim)? {
            Consumption::Consumed => Verdict::Verified {
                action_digest: digest,
            },
            Consumption::Refused(reason) => Verdict::Rejected(reason),
        })
    }
}
