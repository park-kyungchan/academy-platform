//! Selected source-port projection. Authentication failure is retained, never retried.
use crate::{
    data::*,
    owner_admission::{OwnerAdmission, is_date_admitted},
};
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
    /// None means the original optional capability is absent, never a failed lookup.
    fn has_name_search(&self) -> bool {
        false
    }
    fn search_students_by_name(&mut self, _input: &Js) -> Option<Result<Js>> {
        None
    }
}
/// Native stage identity is kept out of Js; only this authentication stage grants context.
#[derive(Debug, Clone)]
pub enum StageError {
    Source(Error),
    Authentication(Error),
    StudentNameNotAdmitted,
}
pub type StageResult<T> = std::result::Result<T, StageError>;
impl From<Error> for StageError {
    fn from(e: Error) -> Self {
        Self::Source(e)
    }
}
impl StageError {
    pub fn into_fault(self) -> edu_academy_core::js::Fault {
        match self {
            Self::Authentication(cause) => {
                edu_academy_core::js::Fault::trusted_authentication(Some(fault_from_error(cause)))
            }
            Self::Source(error) => fault_from_error(error),
            Self::StudentNameNotAdmitted => edu_academy_core::js::Fault::error(""),
        }
    }
    pub fn into_error(self) -> Error {
        error_from_fault(self.into_fault())
    }
}
struct ResolvedStudent {
    student: Js,
    lesson_date: Option<Js>,
    at: u64,
}

pub struct Backend<P: SourcePort> {
    pub port: P,
    pub admission: Js,
    authentication: Option<Result<Js>>,
    owner: Option<OwnerAdmission>,
    managed_names: Vec<Js>,
    source_resolved: Vec<(Js, ResolvedStudent)>,
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
            owner: None,
            managed_names: vec![],
            source_resolved: vec![],
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
        self.ensure_authenticated()
            .map_err(StageError::into_error)?;
        self.read_observation(scope, &student, now, uuid)
    }
    /// One raw result, including synchronous credential failures, for every owner view.
    pub fn ensure_authenticated(&mut self) -> StageResult<Js> {
        if self.authentication.is_none() {
            self.authentication = Some(self.port.authenticate());
        }
        match self.authentication.as_ref() {
            Some(result) => result.clone().map_err(StageError::Authentication),
            None => unreachable!("authentication was assigned"),
        }
    }
    fn read_observation(&mut self, scope: &Js, student: &Js, now: &str, uuid: &str) -> Result<Js> {
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
        for field in FIELDS {
            let target = observation_target(if field == "previousHomeworkAssessment" {
                prior
            } else {
                row
            });
            let mut value = projected.get(field).clone();
            value.set(
                "effectTarget",
                edu_academy_core::workflow::canonical(&target)
                    .map_err(error_from_fault)?
                    .into(),
            );
            value.set("effectRecordKey", target.get("recordId").clone());
            projected.set(field, value);
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
    pub fn with_owner(
        port: P,
        admission: Js,
        owner: Option<&Js>,
        managed_names: Vec<Js>,
    ) -> Result<Self> {
        let owner = owner.map(OwnerAdmission::parse).transpose()?;
        if let Some(owner) = &owner {
            owner.validate_launch(&admission)?;
        }
        let mut backend = Self::new(port, admission);
        backend.owner = owner;
        backend.managed_names = managed_names;
        Ok(backend)
    }
    pub fn managed_names(&self) -> &[Js] {
        &self.managed_names
    }
    pub fn owner_admission(&self) -> Js {
        self.owner.as_ref().map_or(Js::Null, |o| o.value().clone())
    }
    fn source_verified(&self) -> bool {
        self.owner
            .as_ref()
            .is_some_and(OwnerAdmission::source_verified)
    }
    fn date_admitted(&self, date: &Js) -> bool {
        self.owner.as_ref().map_or_else(
            || date == self.admission.get("date"),
            |o| is_date_admitted(date, Some(o)),
        )
    }
    pub fn read_routing_admission(&self) -> Js {
        let owner = self.owner.as_ref().map(OwnerAdmission::value);
        let mut routing = obj(&[
            ("sourceNamespace", "academy.lms.dayrecord".into()),
            ("teacherId", self.admission.get("teacherId").clone()),
            (
                "allowedFrom",
                owner.map_or_else(
                    || self.admission.get("date").clone(),
                    |o| o.get("allowedFrom").clone(),
                ),
            ),
            (
                "allowedThrough",
                owner.map_or_else(
                    || self.admission.get("date").clone(),
                    |o| o.get("allowedThrough").clone(),
                ),
            ),
        ]);
        if let Some(o) = owner.filter(|o| o.get("schemaVersion").is("academy.owner-admission/2")) {
            routing.set(
                "studentScope",
                if matches!(o.get("studentScope"), Js::Undefined) {
                    "configured".into()
                } else {
                    o.get("studentScope").clone()
                },
            );
        }
        routing
    }
    /// Future MCP resolver entry: request and optional date must be passed through unchanged.
    pub fn search_students_by_name(
        &mut self,
        exact_name: &Js,
        request_ref: Option<&Js>,
        lesson_date: Option<&Js>,
        now_ms: u64,
    ) -> StageResult<Js> {
        let request_ref = request_ref.filter(|r| r.truthy());
        if let Some(request_ref) = request_ref {
            self.source_resolved.retain(|(r, _)| r != request_ref);
        }
        if self.source_verified() && !self.managed_names.contains(exact_name) {
            return Err(StageError::StudentNameNotAdmitted);
        }
        if let Some(date) = lesson_date.filter(|d| d.truthy() && self.source_verified()) {
            if !self.date_admitted(date) {
                return Err(Error::error("native_dailyrecord_scope_refused").into());
            }
            self.ensure_authenticated()?;
            let selector = self.port.discover(&obj(&[
                ("teacherLabel", self.admission.get("teacherLabel").clone()),
                ("date", date.clone()),
            ]))?;
            let found = self.port.execute(&obj(&[
                ("family", "day_record".into()),
                ("teacherLabel", self.admission.get("teacherLabel").clone()),
                ("date", date.clone()),
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
                ("studentNames", Js::Arr(vec![exact_name.clone()])),
            ]))?;
            let rows = found.get("dayRecords");
            if array(rows) && rows.array().len() > 1 {
                return Err(Error::error("native_dated_name_ambiguous").into());
            }
            if !array(rows)
                || rows.array().iter().any(|r| {
                    r.get("studentName") != exact_name
                        || r.get("date") != date
                        || r.get("teacherKey") != self.admission.get("teacherId")
                })
            {
                return Err(Error::error("native_dated_name_scope_conflict").into());
            }
            let pairs = Js::Arr(
                rows.array()
                    .iter()
                    .map(|r| {
                        obj(&[
                            ("studentKey", r.get("studentKey").clone()),
                            ("studentName", r.get("studentName").clone()),
                        ])
                    })
                    .collect(),
            );
            return self
                .remember_resolved(&pairs, exact_name, request_ref, Some(date), now_ms)
                .map_err(Into::into);
        }
        // Probe capability without dispatch: legacy ports have no capability; actual Client always does.
        if self.port.has_name_search() {
            self.ensure_authenticated()?;
            let raw = self
                .port
                .search_students_by_name(&obj(&[
                    ("teacherLabel", self.admission.get("teacherLabel").clone()),
                    ("studentName", exact_name.clone()),
                ]))
                .ok_or_else(|| {
                    Error::error("native_name_search_unqualified_no_automatic_retry")
                })??;
            if !array(&raw) || raw.array().iter().any(|r| !object(r)) {
                return Err(
                    Error::error("native_name_search_unqualified_no_automatic_retry").into(),
                );
            }
            let pairs = Js::Arr(
                raw.array()
                    .iter()
                    .map(|r| {
                        obj(&[
                            ("studentKey", r.get("studentKey").clone()),
                            ("studentName", r.get("studentName").clone()),
                        ])
                    })
                    .collect(),
            );
            return self
                .remember_resolved(&pairs, exact_name, request_ref, None, now_ms)
                .map_err(Into::into);
        }
        let admitted = self
            .admission
            .get("students")
            .array()
            .iter()
            .filter(|s| s.get("displayName") == exact_name)
            .map(|s| {
                obj(&[
                    ("studentKey", s.get("studentId").clone()),
                    ("studentName", s.get("displayName").clone()),
                ])
            })
            .collect::<Vec<_>>();
        if admitted.is_empty() {
            return Err(StageError::StudentNameNotAdmitted);
        }
        Ok(Js::Arr(admitted))
    }
    fn remember_resolved(
        &mut self,
        raw: &Js,
        exact_name: &Js,
        request_ref: Option<&Js>,
        lesson_date: Option<&Js>,
        now_ms: u64,
    ) -> Result<Js> {
        // Reuse the existing strict row validator. Its resolver boundary permits 201
        // rows; this native owner schema has a distinct maximum of exactly 200.
        let parsed = crate::saved::student_search_rows(raw);
        let pairs = if array(raw) && raw.array().len() > 200 {
            let mut issues = match parsed {
                Err(error) => error.properties.get("issues").array().to_vec(),
                Ok(_) => vec![],
            };
            issues.retain(|i| {
                !(i.get("path").array().is_empty()
                    && i.get("origin").is("array")
                    && i.get("code").is("too_big"))
            });
            issues.push(obj(&[
                ("origin", "array".into()),
                ("code", "too_big".into()),
                ("maximum", 200.into()),
                ("inclusive", true.into()),
                ("path", Js::Arr(vec![])),
                (
                    "message",
                    "Too big: expected array to have <=200 items".into(),
                ),
            ]));
            return Err(Error::zod(issues));
        } else {
            parsed?
        };
        let first = pairs.array().first();
        if let Some((request_ref, first)) = request_ref.zip(first).filter(|(_, first)| {
            self.source_verified()
                && pairs.array().iter().all(|r| {
                    r.get("studentName") == exact_name
                        && r.get("studentKey") == first.get("studentKey")
                })
        }) {
            let entry = ResolvedStudent {
                student: obj(&[
                    ("studentId", first.get("studentKey").clone()),
                    ("displayName", first.get("studentName").clone()),
                ]),
                lesson_date: lesson_date.cloned(),
                at: now_ms,
            };
            // JS Map.set updates in place; searches delete their request before reaching here.
            if let Some((_, old)) = self
                .source_resolved
                .iter_mut()
                .find(|(r, _)| r == request_ref)
            {
                *old = entry;
            } else {
                self.source_resolved.push((request_ref.clone(), entry));
            }
            if self.source_resolved.len() > 32 {
                self.source_resolved.remove(0);
            }
        }
        Ok(pairs)
    }
    pub fn read_owner(
        &mut self,
        scope: &Js,
        request_ref: Option<&Js>,
        recorded_subject: Option<&Js>,
        now_ms: u64,
        now: &str,
        uuid: &str,
    ) -> StageResult<Js> {
        let source = request_ref
            .filter(|r| r.truthy())
            .and_then(|r| self.source_resolved.iter().find(|(id, _)| id == r))
            .map(|(_, s)| s)
            .filter(|s| {
                self.source_verified()
                    && s.student.get("studentId") == scope.get("studentId")
                    && s.lesson_date
                        .as_ref()
                        .is_none_or(|d| d == scope.get("lessonDate"))
                    && now_ms.checked_sub(s.at).is_some_and(|age| age <= 300000)
            })
            .map(|s| s.student.clone());
        let recorded = recorded_subject
            .filter(|s| self.source_verified() && self.managed_names.contains(s.get("displayName")))
            .map(|s| {
                obj(&[
                    ("studentId", scope.get("studentId").clone()),
                    ("displayName", s.get("displayName").clone()),
                ])
            });
        let student = self
            .admission
            .get("students")
            .array()
            .iter()
            .find(|s| s.get("studentId") == scope.get("studentId"))
            .cloned()
            .or(source)
            .or(recorded);
        if !scope.get("sourceNamespace").is("academy.lms.dayrecord")
            || scope.get("teacherId") != self.admission.get("teacherId")
            || !self.date_admitted(scope.get("lessonDate"))
            || student.is_none()
        {
            return Err(Error::error("native_dailyrecord_scope_refused").into());
        }
        self.ensure_authenticated()?;
        self.read_observation(scope, &student.expect("qualified student"), now, uuid)
            .map_err(Into::into)
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
