//! The host's engine run as a process: one JSON request on its standard input,
//! one JSON answer on its standard output. The command never opens a store.
use serde_json::{Map, Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::Refusal;

/// The engine run when `EDU_ONT_BIN` is unset or empty.
pub const DEFAULT_BIN: &str = "/usr/local/bin/ont";
/// The largest page a search asks for; every continuation is followed.
const PAGE: u64 = 1000;

pub struct Engine {
    bin: String,
    store: Option<String>,
    namespace: Value,
}

/// An object as `get` answers it: its head revision and its current fields.
pub struct Got {
    pub head: String,
    pub fields: Map<String, Value>,
}

/// An object as `search` answers it: its native key and its current fields.
pub struct Found {
    pub key: String,
    pub fields: Map<String, Value>,
}

/// The value of a field when the engine reports it as a value.
pub fn value<'a>(fields: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    let field = fields.get(name)?;
    if field.get("kind").and_then(Value::as_str) == Some("value") {
        field.get("value")
    } else {
        None
    }
}

/// A field as text: a string as is, a reference by its native key.
pub fn text(fields: &Map<String, Value>, name: &str) -> Option<String> {
    match value(fields, name)? {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o
            .get("native_key")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

impl Engine {
    pub fn new(store: Option<String>, namespace: Value) -> Self {
        let bin = std::env::var("EDU_ONT_BIN")
            .ok()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| DEFAULT_BIN.to_owned());
        Self {
            bin,
            store,
            namespace,
        }
    }

    /// The full Id of an object of the domain.
    pub fn id(&self, object_type: &str, key: &str) -> Value {
        json!({"object_type": object_type, "namespace": self.namespace, "native_key": key})
    }

    fn call(&self, command: &[&str], input: &Value) -> Result<Value, Refusal> {
        let mut process = Command::new(&self.bin);
        if let Some(store) = &self.store {
            process.arg("--store").arg(store);
        }
        process
            .args(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = process.spawn().map_err(|e| {
            Refusal::engine_missing(&format!("cannot run the engine: {}", e.kind()))
        })?;
        let body = input.to_string();
        let mut stdin = child.stdin.take();
        let output = std::thread::scope(|scope| {
            scope.spawn(move || {
                if let Some(pipe) = stdin.as_mut() {
                    // A closed pipe shows in the answer, which is then refused.
                    let _ = pipe.write_all(body.as_bytes());
                }
                drop(stdin);
            });
            child.wait_with_output()
        })
        .map_err(|e| {
            Refusal::engine_missing(&format!("the engine did not answer: {}", e.kind()))
        })?;
        let answer: Value = serde_json::from_slice(&output.stdout)
            .map_err(|_| Refusal::engine_missing("the engine's answer is not one JSON object"))?;
        match answer.get("status").and_then(Value::as_str) {
            Some("ok") => Ok(answer.get("result").cloned().unwrap_or(Value::Null)),
            Some("refused") => Err(Refusal::from_engine(&answer)),
            _ => Err(Refusal::engine_missing("the engine's answer has no status")),
        }
    }

    /// The object, or `None` when the engine answers `unavailable`.
    pub fn get(&self, object_type: &str, key: &str) -> Result<Option<Got>, Refusal> {
        let answer = match self.call(&["get"], &json!({"id": self.id(object_type, key)})) {
            Ok(answer) => answer,
            Err(refusal) if refusal.code == "unavailable" => return Ok(None),
            Err(refusal) => return Err(refusal),
        };
        let Some(object) = answer
            .get("objects")
            .and_then(Value::as_array)
            .and_then(|o| o.first())
        else {
            return Ok(None);
        };
        let head = object
            .get("head")
            .and_then(Value::as_str)
            .ok_or_else(|| Refusal::engine_missing("the engine's object has no head"))?
            .to_owned();
        let observations = object
            .get("observations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let current = observations
            .iter()
            .find(|o| o.get("revision").and_then(Value::as_str) == Some(head.as_str()))
            .or_else(|| observations.last());
        let fields = current
            .and_then(|o| o.get("fields"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        Ok(Some(Got { head, fields }))
    }

    /// Every object of the type the filter selects, following each continuation.
    pub fn search(&self, object_type: &str, filter: Value) -> Result<Vec<Found>, Refusal> {
        let mut found = Vec::new();
        let mut continuation: Option<String> = None;
        loop {
            let mut request = json!({"object_type": object_type, "where": filter, "limit": PAGE});
            if let Some(c) = &continuation {
                request["continuation"] = json!(c);
            }
            let answer = self.call(&["search"], &request)?;
            for object in answer
                .get("objects")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let key = object
                    .pointer("/id/native_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Refusal::engine_missing("the engine's object has no key"))?
                    .to_owned();
                let fields = object
                    .pointer("/head/fields")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                found.push(Found { key, fields });
            }
            let next = answer
                .get("continuation")
                .and_then(Value::as_str)
                .map(str::to_owned);
            match next {
                Some(next) if continuation.as_deref() != Some(next.as_str()) => {
                    continuation = Some(next);
                }
                _ => return Ok(found),
            }
        }
    }

    /// Apply one action; the answer is the engine's outcome.
    pub fn apply(
        &self,
        action: &str,
        parameters: &Map<String, Value>,
        request_id: &str,
    ) -> Result<String, Refusal> {
        let answer = self.call(
            &["action", "apply"],
            &json!({"action_type": action, "parameters": parameters, "request_id": request_id}),
        )?;
        Ok(answer
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("applied")
            .to_owned())
    }
}
