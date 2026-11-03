//! Selected source-port projection. Authentication failure is retained, never retried.
use crate::data::*;
pub const FIELDS: [&str; 6] = [
    "progress",
    "homework",
    "lessonMemo",
    "attendance",
    "dailyTestAssessment",
    "previousHomeworkAssessment",
];
pub const TARGET: [&str; 8] = [
    "studentName",
    "date",
    "groupId",
    "teacherKey",
    "studentKey",
    "courseKey",
    "recordKey",
    "membershipKey",
];
pub fn literal_key(v: &Js) -> Result<Js> {
    let u = v.units();
    if !v.string()
        || u.is_empty()
        || u.len() > 16
        || u[0] < 49
        || u[0] > 57
        || !u.iter().all(|c| (48..=57).contains(c))
        || (u.len() == 16 && u > text("9007199254740991").as_slice())
    {
        return Err(Error::error("invalid_read_key"));
    }
    Ok(v.clone())
}
pub fn literal_name(v: &Js) -> Result<Js> {
    if !v.string() || trim(v.units()).is_empty() || v.units().len() > 200 {
        return Err(Error::error("invalid_read_name"));
    }
    Ok(v.clone())
}
pub trait SourcePort {
    fn authenticate(&mut self) -> Result<Js>;
    fn discover(&mut self, input: &Js) -> Result<Js>;
    fn execute(&mut self, input: &Js) -> Result<Js>;
    fn journal_state(&mut self, target: &Js) -> Result<Js>;
}
pub struct Backend<P: SourcePort> {
    pub port: P,
    pub admission: Js,
    authentication: Option<Result<Js>>,
}
pub fn target(row: &Js) -> Js {
    obj(&TARGET.map(|k| (k, row.get(k).clone())))
}
pub fn observation_target(t: &Js) -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", t.get("teacherKey").clone()),
        ("studentId", t.get("studentKey").clone()),
        ("lessonDate", t.get("date").clone()),
        ("courseId", t.get("courseKey").clone()),
        ("membershipId", t.get("membershipKey").clone()),
        ("recordId", t.get("recordKey").clone()),
    ])
}
impl<P: SourcePort> Backend<P> {
    pub fn new(port: P, admission: Js) -> Self {
        Self {
            port,
            admission,
            authentication: None,
        }
    }
    pub fn read(&mut self, scope: &Js, now: &str, uuid: &str) -> Result<Js> {
        let student = self
            .admission
            .get("students")
            .array()
            .iter()
            .find(|s| s.get("studentId") == scope.get("studentId"))
            .cloned();
        if !scope.get("sourceNamespace").is("academy.lms.dayrecord")
            || scope.get("teacherId") != self.admission.get("teacherId")
            || scope.get("lessonDate") != self.admission.get("date")
            || student.is_none()
        {
            return Err(Error::error("native_dailyrecord_scope_refused"));
        }
        let student = student.unwrap();
        if self.authentication.is_none() {
            self.authentication = Some(self.port.authenticate().map_err(|cause| {
                let mut e = Error::new("Error", "source_authentication_failed");
                e.properties = obj(&[
                    ("code", "source_authentication_failed".into()),
                    ("category", "authentication".into()),
                    (
                        "evidenceLocator",
                        "src/native-dailyrecord-backend.ts:#authenticate".into(),
                    ),
                    ("cause", cause.value()),
                ]);
                e
            }));
        }
        self.authentication.as_ref().unwrap().clone()?;
        let selector = self.port.discover(&obj(&[
            ("teacherLabel", self.admission.get("teacherLabel").clone()),
            ("date", scope.get("lessonDate").clone()),
        ]))?;
        let request = obj(&[
            ("family", "day_record".into()),
            ("teacherLabel", self.admission.get("teacherLabel").clone()),
            ("date", scope.get("lessonDate").clone()),
            (
                "groupIds",
                Js::Arr(
                    selector
                        .get("groups")
                        .array()
                        .iter()
                        .map(|g| g.get("groupId").clone())
                        .collect(),
                ),
            ),
            (
                "studentNames",
                Js::Arr(vec![student.get("displayName").clone()]),
            ),
        ]);
        let current = self.port.execute(&request)?;
        let rows = current.get("dayRecords").array();
        if rows.len() != 1 {
            return Err(Error::error("native_dailyrecord_occurrence_unverified"));
        }
        let row = &rows[0];
        if row.get("studentKey") != scope.get("studentId")
            || row.get("studentName") != student.get("displayName")
            || row.get("teacherKey") != scope.get("teacherId")
            || row.get("date") != scope.get("lessonDate")
        {
            return Err(Error::error("native_dailyrecord_identity_conflict"));
        }
        let state = self.port.journal_state(&target(row))?;
        if json(&target(state.get("target"))) != json(&target(row)) {
            return Err(Error::error("native_dailyrecord_state_conflict"));
        }
        let mut previous = request.clone();
        previous.set("date", state.get("previousDate").clone());
        previous.set("groupIds", Js::Arr(vec![row.get("groupId").clone()]));
        let previous = self.port.execute(&previous)?;
        let rows = previous.get("dayRecords").array();
        if rows.len() != 1 {
            return Err(Error::error("native_dailyrecord_previous_unverified"));
        }
        let prior = &rows[0];
        if ["studentKey", "teacherKey", "courseKey", "membershipKey"]
            .iter()
            .any(|k| prior.get(k) != row.get(k))
            || prior.get("recordKey") != state.get("previousRecordKey")
            || prior.get("date") != state.get("previousDate")
        {
            return Err(Error::error("native_dailyrecord_previous_conflict"));
        }
        let reference = format!("native-dayrecord-read:{uuid}");
        let tone = ["progress", "homework", "lessonMemo"]
            .iter()
            .map(|k| prior.get("fields").get(k))
            .filter(|v| v.truthy())
            .map(|v| {
                if v.string() {
                    v.clone()
                } else {
                    Js::from(json(v))
                }
            })
            .collect::<Vec<_>>();
        let mut units = vec![];
        for (i, v) in tone.iter().enumerate() {
            if i > 0 {
                units.push(10)
            }
            units.extend(v.units())
        }
        let mut projected = obj(&[]);
        for field in FIELDS {
            let value = state.get("fields").get(field);
            projected.set(
                field,
                if value.string() {
                    obj(&[
                        ("state", "known".into()),
                        ("value", value.clone()),
                        (
                            "evidenceRefs",
                            Js::Arr(vec![
                                format!(
                                    "{reference}:{}",
                                    if field == "previousHomeworkAssessment" {
                                        "previous"
                                    } else {
                                        "current"
                                    }
                                )
                                .into(),
                            ]),
                        ),
                    ])
                } else {
                    obj(&[
                        ("state", "unknown".into()),
                        ("reasonCode", "native_field_not_observed".into()),
                    ])
                },
            );
        }
        let observation = obj(&[
            ("scope", scope.clone()),
            ("target", observation_target(row)),
            ("previousTarget", observation_target(prior)),
            ("observedAt", now.into()),
            (
                "sourceRevision",
                hash(
                    format!(
                        "{}:{}",
                        string(self.admission.get("sourceBundleSha256")),
                        string(state.get("wireDigest"))
                    )
                    .as_bytes(),
                )
                .into(),
            ),
            ("wireDigest", state.get("wireDigest").clone()),
            ("fields", projected),
            (
                "toneSamples",
                Js::Arr(if units.is_empty() {
                    vec![]
                } else {
                    vec![obj(&[
                        ("target", observation_target(prior)),
                        ("observedAt", now.into()),
                        ("text", Js::Str(units)),
                        (
                            "evidenceRefs",
                            Js::Arr(vec![format!("{reference}:previous").into()]),
                        ),
                    ])]
                }),
            ),
            ("transmissionState", "unknown".into()),
        ]);
        let observation =
            edu_academy_core::schema::validate("dailyRecordObservationSchema", &observation)
                .map_err(error_from_fault)?;
        Ok(obj(&[
            ("status", "observed".into()),
            ("observation", observation),
            (
                "subject",
                obj(&[("displayName", student.get("displayName").clone())]),
            ),
        ]))
    }
}
/// Recorded SourcePort data, matched in order. No case names or expected-result lookup.
pub struct RecordedPort {
    pub exchanges: Vec<Js>,
    pub events: Vec<Js>,
    index: usize,
}
impl RecordedPort {
    pub fn new(exchanges: Vec<Js>) -> Self {
        Self {
            exchanges,
            events: vec![],
            index: 0,
        }
    }
    fn call(&mut self, op: &str, input: Option<&Js>) -> Result<Js> {
        let mut event = obj(&[("operation", op.into())]);
        if let Some(v) = input {
            event.set(
                if op == "journalState" {
                    "target"
                } else {
                    "input"
                },
                v.clone(),
            )
        }
        self.events.push(event);
        let record = self
            .exchanges
            .get(self.index)
            .ok_or_else(|| Error::error("recorded_source_exchange_missing"))?;
        self.index += 1;
        if !record.get("operation").is(op)
            || (record.has("input") && Some(record.get("input")) != input)
        {
            return Err(Error::error("recorded_source_exchange_conflict"));
        }
        if record.has("error") {
            let e = record.get("error");
            return Err(Error::new(&string(e.get("name")), string(e.get("message"))));
        }
        Ok(record.get("value").clone())
    }
}
impl SourcePort for RecordedPort {
    fn authenticate(&mut self) -> Result<Js> {
        self.call("authenticate", None)
    }
    fn discover(&mut self, v: &Js) -> Result<Js> {
        self.call("discover", Some(v))
    }
    fn execute(&mut self, v: &Js) -> Result<Js> {
        self.call("execute", Some(v))
    }
    fn journal_state(&mut self, v: &Js) -> Result<Js> {
        self.call("journalState", Some(v))
    }
}
pub fn resolve_name(name: &Js, rows: &Js) -> Js {
    let unavailable = |s: &str| obj(&[("status", "unavailable".into()), ("reasonCode", s.into())]);
    if !name.string() || trim(name.units()).is_empty() || trim(name.units()).len() > 200 {
        return obj(&[
            ("value", unavailable("student_name_invalid")),
            ("calls", Js::Num(0.)),
        ]);
    }
    let name = Js::Str(trim(name.units()).to_vec());
    let validation = crate::saved::student_search_rows(rows);
    let value = if let Err(error) = validation {
        let failure = obj(&[
            ("category", "malformed_response".into()),
            ("code", "source_response_malformed".into()),
            ("evidenceLocator", "academy-mcp:response-contract".into()),
            ("automaticRetryAllowed", false.into()),
            ("errorKind", "Error".into()),
            (
                "errorSha256",
                hash(format!("{}\0{}", error.name, error.message).as_bytes()).into(),
            ),
        ]);
        obj(&[
            ("status", "unavailable".into()),
            ("reasonCode", "source_response_malformed".into()),
            ("failure", failure),
        ])
    } else if rows.array().len() > 200 {
        unavailable("student_resolve_search_unqualified_no_automatic_retry")
    } else if rows.array().iter().any(|r| r.get("studentName") != &name) {
        unavailable("student_name_scope_conflict")
    } else if rows.array().is_empty() {
        unavailable("student_name_not_found")
    } else if rows
        .array()
        .iter()
        .any(|r| r.get("studentKey") != rows.array()[0].get("studentKey"))
    {
        unavailable("student_name_ambiguous")
    } else {
        obj(&[
            ("status", "resolved".into()),
            ("studentKey", rows.array()[0].get("studentKey").clone()),
            ("displayName", name),
        ])
    };
    obj(&[("value", value), ("calls", Js::Num(1.))])
}
