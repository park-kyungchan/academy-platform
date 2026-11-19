//! The read store through the engine's library entry, `ont::run`, as the tests call it.
//! It never installs a schema.
use crate::{Answer, config::Config, refuse};
use edu_academy_core::workflow::Runtime;
use edu_academy_ontology::{ACADEMY_READ_SCHEMA, ACADEMY_READ_SPOOL_ROOT};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub(crate) struct Store<'a> {
    path: String,
    expected: String,
    clock: &'a dyn Runtime,
    ids: (u32, u32),
}
pub(crate) struct Coverage {
    pub observed_at: u64,
    pub rows: u64,
    pub stored: u64,
    pub outside_roster: u64,
    pub cut_fields: u64,
}
fn unavailable() -> crate::Refusal {
    refuse("lesson_store_unavailable")
}
/// A property of a found object, when it holds a value.
fn field<'a>(object: &'a Value, name: &str) -> Option<&'a Value> {
    let field = &object["head"]["fields"][name];
    (field["kind"] == "value").then(|| &field["value"])
}
pub(crate) fn text(object: &Value, name: &str) -> String {
    field(object, name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
pub(crate) fn number(object: &Value, name: &str) -> u64 {
    field(object, name).and_then(Value::as_u64).unwrap_or(0)
}
pub(crate) fn observed_at(object: &Value) -> u64 {
    object["head"]["observed_at"].as_u64().unwrap_or(0)
}
/// Every property that holds a value, by its name.
pub(crate) fn members(object: &Value) -> Map<String, Value> {
    let mut members = Map::new();
    if let Some(fields) = object["head"]["fields"].as_object() {
        for name in fields.keys() {
            if let Some(value) = field(object, name) {
                members.insert(name.clone(), value.clone());
            }
        }
    }
    members
}
impl<'a> Store<'a> {
    /// The digest compared is the embedded read schema's, with its spool root
    /// replaced by the configured spool (the text the owner installs for that spool).
    pub fn new(config: &Config, clock: &'a dyn Runtime, ids: (u32, u32)) -> Self {
        let schema = ACADEMY_READ_SCHEMA
            .replace(ACADEMY_READ_SPOOL_ROOT, &config.spool.display().to_string());
        Self {
            path: config.store.display().to_string(),
            expected: format!("{:x}", Sha256::digest(schema.as_bytes())),
            clock,
            ids,
        }
    }
    /// The engine's time is the clock's at each call: a capture moves it forward.
    pub fn now(&self) -> u64 {
        self.clock.epoch_ms() / 1000
    }
    fn call(&self, args: &[&str], stdin: &Value) -> Answer<(i32, Value)> {
        let mut all = vec![
            "--store".to_string(),
            self.path.clone(),
            "--now".into(),
            self.now().to_string(),
        ];
        all.extend(args.iter().map(|a| a.to_string()));
        let out = ont::run(&all, stdin.to_string().as_bytes(), self.ids.0, self.ids.1);
        let answer = serde_json::from_str(out.stdout.trim()).map_err(|_| unavailable())?;
        Ok((out.code, answer))
    }
    /// The installed `academy_read` schema is the one this crate embeds. A `describe`
    /// the engine refused (for one, a caller that is not 0 and 0: `principal_not_trusted`)
    /// ends with the engine's own code; only an answered one is compared.
    pub fn check_schema(&self) -> Answer<()> {
        if !std::fs::symlink_metadata(&self.path).is_ok_and(|m| m.is_file()) {
            return Err(unavailable());
        }
        let (code, answer) = self.call(&["describe"], &json!({}))?;
        if code != 0 || answer["status"] != "ok" {
            return Err(match answer["code"].as_str() {
                Some(code) if crate::fixed(code) => refuse(code),
                _ => unavailable(),
            });
        }
        let installed = answer["result"]["domains"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|d| d["domain"] == "academy_read");
        if installed.is_none_or(|d| d["sha256"] != self.expected.as_str()) {
            return Err(refuse("lesson_schema_mismatch"));
        }
        Ok(())
    }
    /// Index the two lesson sources; one failed source ends the command.
    pub fn index(&self) -> Answer<()> {
        let args = [
            "index",
            "--source",
            "lesson_record",
            "--source",
            "lesson_day",
        ];
        let (code, answer) = self.call(&args, &json!({}))?;
        let sources = answer["result"]["sources"].as_array();
        if code != 0
            || answer["status"] != "ok"
            || sources.is_none_or(|s| s.len() != 2 || s.iter().any(|s| s["status"] != "ok"))
        {
            return Err(refuse("lesson_index_failed"));
        }
        Ok(())
    }
    /// Every object of the type the filter selects, through all continuations.
    pub fn search(&self, object_type: &str, filter: Value) -> Answer<Vec<Value>> {
        let mut objects = vec![];
        let mut continuation = Value::Null;
        loop {
            let request = json!({"object_type":object_type,"where":filter,"limit":1000,"continuation":continuation});
            let (code, answer) = self.call(&["search"], &request)?;
            let found = answer["result"]["objects"]
                .as_array()
                .ok_or_else(unavailable)?;
            if code != 0 {
                return Err(unavailable());
            }
            objects.extend(found.iter().cloned());
            continuation = answer["result"]["continuation"].clone();
            if continuation.is_null() {
                return Ok(objects);
            }
        }
    }
    pub fn coverage(&self, date: &str) -> Answer<Option<Coverage>> {
        let found = self.search("lesson_day", json!({"eq":["lesson_date",date]}))?;
        Ok(found.first().map(|day| Coverage {
            observed_at: observed_at(day),
            rows: number(day, "row_count"),
            stored: number(day, "stored_count"),
            outside_roster: number(day, "outside_roster_count"),
            cut_fields: number(day, "cut_fields"),
        }))
    }
    /// The view's table, unchanged.
    pub fn render(&self, view: &str, args: Value) -> Answer<String> {
        let (code, answer) = self.call(&["render"], &json!({"view":view,"args":args}))?;
        match answer["markdown"].as_str() {
            Some(table) if code == 0 => Ok(table.into()),
            _ => Err(unavailable()),
        }
    }
}
