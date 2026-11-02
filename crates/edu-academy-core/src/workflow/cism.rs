//! Original CISM field-wide and operation-specific durable receipts.
use super::{
    Runtime, codec, io_fault,
    store::{mkdir, sync_dir},
};
use crate::js::{Family, Fault, Js, Result, digest, obj, stringify};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
pub const POLICY: &str = r#"{"audience":"deputy_director","style":"mixed_by_field_concise_factual_bullets_and_polite_explanatory_prose","basis":"same_week_dailyrecord_and_student_records","unknowns":"do_not_infer_numeric_scores_class_atmosphere_absence_or_completion","pdfGeneration":"user_action_only"}"#;
pub fn policy() -> Js {
    codec::parse(POLICY.as_bytes()).expect("source policy constant")
}
fn fail() -> Fault {
    Fault::error("cism_store_unqualified")
}
fn current_uid() -> Result<u32> {
    Ok(fs::metadata("/proc/self").map_err(io_fault)?.uid())
}
pub struct CismStore {
    root: PathBuf,
    uid: u32,
}
impl CismStore {
    pub fn open(root: &Path) -> Result<Self> {
        mkdir(root)?;
        let m = fs::symlink_metadata(root).map_err(io_fault)?;
        let uid = current_uid()?;
        if !m.is_dir()
            || m.file_type().is_symlink()
            || fs::canonicalize(root).map_err(io_fault)? != root
            || m.mode() & 0o077 != 0
            || m.uid() != uid
        {
            return Err(fail());
        }
        Ok(Self {
            root: root.into(),
            uid,
        })
    }
    pub fn read(&self, name: &str) -> Result<Js> {
        if Path::new(name).components().count() != 1 {
            return Err(fail());
        }
        // Linux O_NOFOLLOW. No libc dependency or unsafe operation.
        let mut f = OpenOptions::new()
            .read(true)
            .custom_flags(0o400000)
            .open(self.root.join(name))
            .map_err(|e| {
                if e.raw_os_error() == Some(40) {
                    fail()
                } else {
                    io_fault(e)
                }
            })?;
        let m = f.metadata().map_err(io_fault)?;
        if !m.is_file()
            || m.nlink() != 1
            || m.len() > 1_000_000
            || m.mode() & 0o077 != 0
            || m.uid() != self.uid
        {
            return Err(fail());
        }
        let mut b = vec![];
        f.read_to_end(&mut b).map_err(io_fault)?;
        codec::parse(&b)
    }
    pub fn exclusive(&self, name: &str, value: &Js) -> Result<bool> {
        if Path::new(name).components().count() != 1 {
            return Err(fail());
        }
        let mut f = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.root.join(name))
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
            Err(e) => return Err(io_fault(e)),
        };
        f.write_all(&codec::bytes(value)?)
            .and_then(|()| f.sync_all())
            .map_err(io_fault)?;
        drop(f);
        sync_dir(&self.root)?;
        Ok(true)
    }
    pub fn field_executions(&self, report: &Js, field: &Js) -> Result<Vec<(Js, String)>> {
        let key = field_key(report, field)?;
        let mut out = vec![];
        // Preserve readdir's native order (the original does not explicitly sort).
        for e in fs::read_dir(&self.root).map_err(io_fault)? {
            let n = e
                .map_err(io_fault)?
                .file_name()
                .to_string_lossy()
                .into_owned();
            let operation = n
                .strip_prefix(&(key.clone() + "."))
                .and_then(|n| n.strip_suffix(".dispatch.json"));
            let selected = n == key.clone() + ".dispatch.json"
                || operation.is_some_and(|h| {
                    h.len() == 64
                        && h.bytes()
                            .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
                });
            if selected {
                out.push((
                    self.read(&n)?,
                    n.trim_end_matches(".dispatch.json").to_owned() + ".persisted.json",
                ));
            }
        }
        Ok(out)
    }
    pub fn settle(&self, report: &Js, field: &Js, execution: &(Js, String)) -> Result<bool> {
        let receipt = match self.read(&execution.1) {
            Ok(r) => r,
            Err(e) if e.message == "ENOENT" => Js::Null,
            Err(e) => return Err(e),
        };
        if !receipt.absent() {
            if receipt.get("fieldRef") != field.get("fieldRef")
                || receipt.get("after") != execution.0.get("after")
            {
                return Err(fail());
            }
            return Ok(true);
        }
        if field.get("value") != execution.0.get("after") {
            return Ok(false);
        }
        self.exclusive(
            &execution.1,
            &obj(&[
                ("fieldRef", field.get("fieldRef").clone()),
                ("after", execution.0.get("after").clone()),
                ("observedAt", report.get("observedAt").clone()),
            ]),
        )?;
        Ok(true)
    }
    pub fn execution_readback(&self, report: &Js) -> Result<Js> {
        let mut states = vec![];
        for f in report.get("fields").array() {
            for e in self.field_executions(report, f)? {
                let persisted = self.settle(report, f, &e)?;
                states.push(obj(&[
                    ("fieldRef", f.get("fieldRef").clone()),
                    (
                        "status",
                        if persisted { "persisted" } else { "uncertain" }.into(),
                    ),
                    ("requestRef", e.0.get("requestRef").clone()),
                    ("reviewDigest", e.0.get("reviewDigest").clone()),
                ]));
            }
        }
        Ok(Js::Arr(states))
    }
}
fn hash(v: &Js) -> Result<String> {
    digest(v, Family::Cism)
}
pub fn field_key(report: &Js, field: &Js) -> Result<String> {
    hash(&Js::Arr(vec![
        report.get("teacherKey").clone(),
        report.get("periodStart").clone(),
        field.get("fieldRef").clone(),
    ]))
}
pub trait Backend {
    fn read_report(&mut self, date: &Js) -> Result<Js>;
    fn apply_field(
        &mut self,
        input: &Js,
        authorize: &mut dyn FnMut() -> Result<bool>,
    ) -> Result<Js>;
}
pub fn verify(input: &Js, report: &Js) -> Result<String> {
    if report.get("reportSha256") != input.get("expectedReportSha256") {
        return Err(Fault::error("cism_report_drift"));
    }
    let mut refs = vec![];
    for c in input.get("changes").array() {
        if refs.contains(c.get("fieldRef")) {
            return Err(Fault::error("cism_duplicate_field"));
        }
        refs.push(c.get("fieldRef").clone());
        let Some(f) = report
            .get("fields")
            .array()
            .iter()
            .find(|f| f.get("fieldRef") == c.get("fieldRef"))
        else {
            return Err(Fault::error("cism_stale_before"));
        };
        if f.get("value") != c.get("before") {
            return Err(Fault::error("cism_stale_before"));
        }
        let empty_or_zero = crate::js::trim(f.get("value").units()).is_empty()
            || f.get("kind").is("class_mode") && f.get("value").is("0");
        let narrative_completion =
            f.get("kind").is("narrative") && c.get("edit").is("complete_existing_narrative");
        if !empty_or_zero && !narrative_completion {
            return Err(Fault::error("cism_existing_value_preserved"));
        }
        crate::cism::write_parameters(report, f, c.get("after"))?;
    }
    hash(&obj(&[
        ("input", input.clone()),
        ("periodStart", report.get("periodStart").clone()),
        ("teacherKey", report.get("teacherKey").clone()),
        ("wireSha256", report.get("wireSha256").clone()),
        ("policy", policy()),
    ]))
}
fn without(input: &Js, keys: &[&str]) -> Js {
    Js::Obj(
        input
            .props()
            .iter()
            .filter(|(k, _)| !keys.iter().any(|s| k == &crate::js::text(s)))
            .cloned()
            .collect(),
    )
}
pub struct Service<'a> {
    pub root: &'a Path,
    pub backend: &'a mut dyn Backend,
    pub runtime: &'a dyn Runtime,
}
impl Service<'_> {
    pub fn get(&mut self, input: &Js) -> Result<Js> {
        let input = super::schema::validate("cismGetInput", input)?;
        let r = self.backend.read_report(input.get("date"))?;
        let store = CismStore::open(self.root)?;
        Ok(obj(&[
            ("status", "observed".into()),
            ("report", r.clone()),
            ("executionReadback", store.execution_readback(&r)?),
            ("authoringPolicy", policy()),
        ]))
    }
    pub fn review(&mut self, raw: &Js) -> Result<Js> {
        let raw = super::schema::validate("cismReviewInput", raw)?;
        let input = without(&raw, &["attribution"]);
        let r = self.backend.read_report(input.get("date"))?;
        let d = verify(&input, &r)?;
        let store = CismStore::open(self.root)?;
        store.exclusive(
            &(d.clone() + ".review.json"),
            &obj(&[
                ("input", input.clone()),
                ("reviewDigest", d.clone().into()),
                ("periodStart", r.get("periodStart").clone()),
                ("teacherKey", r.get("teacherKey").clone()),
                ("wireSha256", r.get("wireSha256").clone()),
                (
                    "attribution",
                    if raw.get("attribution").absent() {
                        super::observe_attribution(self.runtime, &Js::Null)
                    } else {
                        raw.get("attribution").clone()
                    },
                ),
            ]),
        )?;
        Ok(obj(&[
            ("status", "reviewed".into()),
            ("reviewDigest", d.into()),
            ("periodStart", r.get("periodStart").clone()),
            ("periodEnd", r.get("periodEnd").clone()),
            ("changes", input.get("changes").clone()),
            ("authoringPolicy", policy()),
            ("academyWrites", Js::Num(0.0)),
            ("pdfGenerated", false.into()),
        ]))
    }
    pub fn save(&mut self, raw: &Js) -> Result<Js> {
        let raw = super::schema::validate("cismSaveInput", raw)?;
        let input = without(&raw, &["reviewDigest", "attribution"]);
        let d = raw.get("reviewDigest").code_units();
        let store = CismStore::open(self.root)?;
        let saved = store.read(&(d.clone() + ".review.json"))?;
        if !saved.get("reviewDigest").is(&d) || stringify(saved.get("input"))? != stringify(&input)?
        {
            return Err(Fault::error("cism_prior_review_mismatch"));
        }
        let mut current = self.backend.read_report(input.get("date"))?;
        if verify(&input, &current)? != d {
            return Err(Fault::error("cism_prior_review_mismatch"));
        }
        let request_name = hash(&Js::Arr(vec![
            current.get("teacherKey").clone(),
            input.get("request").get("requestRef").clone(),
        ]))? + ".request.json";
        if !store.exclusive(&request_name, input.get("request"))?
            && stringify(&store.read(&request_name)?)? != stringify(input.get("request"))?
        {
            return Err(Fault::error("cism_request_rebound"));
        }
        let mut results = vec![];
        for c in input.get("changes").array() {
            let key = field_key(&current, c)?;
            let operation = key + "." + &d;
            let dispatch = operation.clone() + ".dispatch.json";
            let done = operation + ".persisted.json";
            for execution in store.field_executions(&current, c)? {
                let f = current
                    .get("fields")
                    .array()
                    .iter()
                    .find(|f| f.get("fieldRef") == c.get("fieldRef"))
                    .ok_or_else(|| Fault::error("cism_stale_before"))?;
                if !store.settle(&current, f, &execution)? {
                    return Err(Fault::error("cism_uncertain_dispatch"));
                }
                if execution.0.get("requestRef") == input.get("request").get("requestRef") {
                    return Err(Fault::error("cism_request_rebound"));
                }
            }
            let mut issued = false;
            let mut authorize = || -> Result<bool> {
                if issued {
                    return Ok(false);
                }
                issued = true;
                store.exclusive(
                    &dispatch,
                    &obj(&[
                        ("fieldRef", c.get("fieldRef").clone()),
                        ("after", c.get("after").clone()),
                        ("before", c.get("before").clone()),
                        ("requestRef", input.get("request").get("requestRef").clone()),
                        ("reviewDigest", d.clone().into()),
                        (
                            "attribution",
                            if raw.get("attribution").absent() {
                                super::observe_attribution(self.runtime, &Js::Null)
                            } else {
                                raw.get("attribution").clone()
                            },
                        ),
                        ("recordedAt", self.runtime.wall_now().into()),
                    ]),
                )
            };
            let outcome = self.backend.apply_field(
                &obj(&[
                    ("date", input.get("date").clone()),
                    ("fieldRef", c.get("fieldRef").clone()),
                    ("before", c.get("before").clone()),
                    ("after", c.get("after").clone()),
                    ("wireSha256", current.get("wireSha256").clone()),
                    (
                        "completeExistingNarrative",
                        c.get("edit").is("complete_existing_narrative").into(),
                    ),
                ]),
                &mut authorize,
            )?;
            if !issued
                || !outcome.get("status").is("persisted")
                || outcome.get("fieldRef") != c.get("fieldRef")
                || outcome
                    .get("report")
                    .get("fields")
                    .array()
                    .iter()
                    .find(|f| f.get("fieldRef") == c.get("fieldRef"))
                    .map(|f| f.get("value"))
                    != Some(c.get("after"))
            {
                return Err(Fault::error("cism_readback_mismatch"));
            }
            store.exclusive(
                &done,
                &obj(&[
                    ("fieldRef", c.get("fieldRef").clone()),
                    ("after", c.get("after").clone()),
                    (
                        "observedAt",
                        outcome.get("report").get("observedAt").clone(),
                    ),
                ]),
            )?;
            current = outcome.get("report").clone();
            results.push(outcome);
        }
        Ok(obj(&[
            ("status", "persisted".into()),
            ("academyWrites", results.len().into()),
            ("results", Js::Arr(results)),
            ("pdfGenerated", false.into()),
            ("parentSends", Js::Num(0.0)),
        ]))
    }
}
