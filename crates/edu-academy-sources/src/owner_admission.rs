//! Pinned owner-admission/1 and /2 data policy, without owner/runtime activation.
use crate::data::*;

pub const FAIL_CLOSED_DEFAULT: &str = "2026-10-23";

fn issue(fields: &[(&str, Js)], path: &[Js], message: impl Into<String>) -> Js {
    let mut v = obj(fields);
    v.set("path", Js::Arr(path.to_vec()));
    let message: String = message.into();
    v.set("message", message.into());
    v
}
fn received(v: &Js) -> &str {
    match v {
        Js::Undefined => "undefined",
        Js::Null => "null",
        Js::Bool(_) => "boolean",
        Js::Num(_) => "number",
        Js::Str(_) => "string",
        Js::Arr(_) => "array",
        _ => "object",
    }
}
fn typed(v: &Js, expected: &str, path: &[Js], issues: &mut Vec<Js>) -> bool {
    let yes = if expected == "object" {
        object(v)
    } else {
        v.string()
    };
    if !yes {
        issues.push(issue(
            &[
                ("expected", expected.into()),
                ("code", "invalid_type".into()),
            ],
            path,
            format!(
                "Invalid input: expected {expected}, received {}",
                received(v)
            ),
        ));
    }
    yes
}
fn choice(v: &Js, values: &[&str], path: &[Js], issues: &mut Vec<Js>) {
    if !values.iter().any(|s| v.is(s)) {
        issues.push(issue(
            &[
                ("code", "invalid_value".into()),
                ("values", strings(values)),
            ],
            path,
            if values.len() == 1 {
                format!("Invalid input: expected {}", json(&values[0].into()))
            } else {
                format!(
                    "Invalid option: expected one of {}",
                    values
                        .iter()
                        .map(|s| json(&(*s).into()))
                        .collect::<Vec<_>>()
                        .join("|")
                )
            },
        ));
    }
}
fn date(v: &Js, key: &str, issues: &mut Vec<Js>) {
    if let Err(e) = edu_academy_core::schema::validate("dailyRecordDateSchema", v) {
        // Reuse the existing date validator and Zod issue parts; prefix its local path.
        for mut part in error_from_fault(e)
            .properties
            .get("issues")
            .array()
            .to_vec()
        {
            let mut path = vec![key.into()];
            path.extend_from_slice(part.get("path").array());
            part.set("path", Js::Arr(path));
            issues.push(part);
        }
    }
}

/// Validated configuration; private fields prevent unvalidated construction.
#[derive(Clone)]
pub struct OwnerAdmission {
    value: Js,
}
impl OwnerAdmission {
    pub fn parse(v: &Js) -> Result<Self> {
        let mut issues = vec![];
        if !typed(v, "object", &[], &mut issues) {
            return Err(Error::zod(issues));
        }
        let version = v.get("schemaVersion");
        if !version.is("academy.owner-admission/1") && !version.is("academy.owner-admission/2") {
            return Err(Error::zod(vec![issue(
                &[
                    ("code", "invalid_union".into()),
                    ("errors", Js::Arr(vec![])),
                    ("note", "No matching discriminator".into()),
                    ("discriminator", "schemaVersion".into()),
                    (
                        "options",
                        strings(&["academy.owner-admission/1", "academy.owner-admission/2"]),
                    ),
                ],
                &["schemaVersion".into()],
                "Invalid discriminator value. Expected 'academy.owner-admission/1' | 'academy.owner-admission/2'",
            )]));
        }
        choice(
            v.get("decider"),
            &["user"],
            &["decider".into()],
            &mut issues,
        );
        date(v.get("decidedAt"), "decidedAt", &mut issues);
        choice(
            v.get("sourceNamespace"),
            &["academy.lms.dayrecord"],
            &["sourceNamespace".into()],
            &mut issues,
        );
        let teacher = v.get("teacherId");
        if typed(teacher, "string", &["teacherId".into()], &mut issues)
            && !edu_academy_core::js::test("^[1-9][0-9]{0,15}$", teacher)
        {
            issues.push(issue(
                &[
                    ("origin", "string".into()),
                    ("code", "invalid_format".into()),
                    ("format", "regex".into()),
                    ("pattern", "/^[1-9][0-9]{0,15}$/".into()),
                ],
                &["teacherId".into()],
                "Invalid string: must match pattern /^[1-9][0-9]{0,15}$/",
            ));
        }
        choice(
            v.get("failClosedDefault"),
            &[FAIL_CLOSED_DEFAULT],
            &["failClosedDefault".into()],
            &mut issues,
        );
        if !matches!(v.get("note"), Js::Undefined)
            && typed(v.get("note"), "string", &["note".into()], &mut issues)
            && v.get("note").units().len() > 500
        {
            issues.push(issue(
                &[
                    ("origin", "string".into()),
                    ("code", "too_big".into()),
                    ("maximum", 500.into()),
                    ("inclusive", true.into()),
                ],
                &["note".into()],
                "Too big: expected string to have <=500 characters",
            ));
        }
        let v2 = version.is("academy.owner-admission/2");
        for key in ["allowedFrom", "allowedThrough"] {
            if !(v2 && matches!(v.get(key), Js::Null)) {
                date(v.get(key), key, &mut issues);
            }
        }
        if v2 && !matches!(v.get("studentScope"), Js::Undefined) {
            choice(
                v.get("studentScope"),
                &["configured", "source_verified"],
                &["studentScope".into()],
                &mut issues,
            );
        }
        let mut keys = vec![
            "schemaVersion",
            "decider",
            "decidedAt",
            "sourceNamespace",
            "teacherId",
            "failClosedDefault",
            "note",
            "allowedFrom",
            "allowedThrough",
        ];
        if v2 {
            keys.push("studentScope");
        }
        let extra = v
            .props()
            .iter()
            .filter(|(k, _)| !keys.iter().any(|s| text(s) == *k))
            .map(|(k, _)| Js::Str(k.clone()))
            .collect::<Vec<_>>();
        if !extra.is_empty() {
            let message = format!(
                "Unrecognized key{}: {}",
                if extra.len() == 1 { "" } else { "s" },
                extra.iter().map(json).collect::<Vec<_>>().join(", ")
            );
            issues.push(issue(
                &[
                    ("code", "unrecognized_keys".into()),
                    ("keys", Js::Arr(extra)),
                ],
                &[],
                message,
            ));
        }
        // Zod superRefine runs after continuable string/date checks, but not
        // after type, literal/enum or strict-object failures.
        let aborted = issues.iter().any(|i| {
            ["invalid_type", "invalid_value", "unrecognized_keys"]
                .iter()
                .any(|c| i.get("code").is(c))
        });
        if !aborted
            && v.get("allowedFrom").string()
            && v.get("allowedThrough").string()
            && v.get("allowedThrough").units() < v.get("allowedFrom").units()
        {
            issues.push(issue(
                &[("code", "custom".into())],
                &[],
                "admission_range_inverted",
            ));
        }
        if !issues.is_empty() {
            return Err(Error::zod(issues));
        }
        // Reconstruct in schema order, including explicit optional undefined when supplied.
        let mut value = obj(&[]);
        for key in keys {
            if v.has(key) {
                value.set(key, v.get(key).clone());
            }
        }
        Ok(Self { value })
    }
    pub fn value(&self) -> &Js {
        &self.value
    }
    pub fn source_verified(&self) -> bool {
        self.value
            .get("schemaVersion")
            .is("academy.owner-admission/2")
            && self.value.get("studentScope").is("source_verified")
    }
    pub fn validate_launch(&self, admission: &Js) -> Result<()> {
        if self.value.get("teacherId") != admission.get("teacherId")
            || !is_date_admitted(admission.get("date"), Some(self))
        {
            return Err(Error::error("native_dailyrecord_owner_admission_mismatch"));
        }
        Ok(())
    }
}

/// Standalone policy uses the historical default; native launch reads use their selected date.
pub fn is_date_admitted(date: &Js, owner: Option<&OwnerAdmission>) -> bool {
    if edu_academy_core::schema::validate("dailyRecordDateSchema", date).is_err() {
        return false;
    }
    let Some(owner) = owner else {
        return date.is(FAIL_CLOSED_DEFAULT);
    };
    let v = owner.value();
    (matches!(v.get("allowedFrom"), Js::Null) || v.get("allowedFrom").units() <= date.units())
        && (matches!(v.get("allowedThrough"), Js::Null)
            || date.units() <= v.get("allowedThrough").units())
}
pub fn admission_blocker(date: &Js, owner: Option<&OwnerAdmission>) -> Option<&'static str> {
    if is_date_admitted(date, owner) {
        None
    } else {
        Some("lesson_date_out_of_admitted_scope")
    }
}

/// Installed-owner transport policy, independent of the frozen DailyRecord schemas.
/// It selects exact namespaces and wire templates, never caller-supplied URLs or
/// an inferred read/write classification (some Academy writes use GET).
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransportAdmission {
    pub schema_version: String,
    pub owner_id: String,
    pub teacher_id: String,
    pub teacher_label: String,
    pub college_id: String,
    pub allowed_from: String,
    pub allowed_through: String,
    pub student_ids: Vec<String>,
    pub source_namespaces: Vec<String>,
    pub operations: Vec<OperationContract>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransportScope {
    pub owner_id: String,
    pub source_namespace: String,
    pub teacher_id: String,
    pub student_id: String,
    pub date: String,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationContract {
    pub id: String,
    pub source_namespace: String,
    pub method: String,
    pub path: String,
    pub effect: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrored_parameters: Vec<String>,
    pub query: Vec<TransportParameter>,
    pub body: Vec<TransportParameter>,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportParameter {
    pub name: String,
    pub value: ParameterValue,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParameterValue {
    Literal { value: String },
    Scope { field: ScopeField },
    Argument { name: String, max_bytes: usize },
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScopeField {
    TeacherId,
    StudentId,
    Date,
}
fn transport_refused() -> Error {
    Error::error("native_transport_scope_refused")
}
fn transport_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
impl TransportAdmission {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let value: Self = serde_json::from_slice(bytes).map_err(|_| transport_refused())?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != "academy.transport-admission/1"
            || !transport_identifier(&self.owner_id)
            || crate::native::literal_key(&self.teacher_id.as_str().into()).is_err()
            || crate::native::literal_key(&self.college_id.as_str().into()).is_err()
            || crate::native::literal_name(&self.teacher_label.as_str().into()).is_err()
            || !valid_date(&self.allowed_from.as_str().into())
            || !valid_date(&self.allowed_through.as_str().into())
            || self.allowed_from > self.allowed_through
            || self.student_ids.is_empty()
            || self.student_ids.len() > 200
            || self
                .student_ids
                .iter()
                .any(|id| crate::native::literal_key(&id.as_str().into()).is_err())
            || self.source_namespaces.is_empty()
            || self.source_namespaces.len() > 32
            || self
                .source_namespaces
                .iter()
                .any(|v| !transport_identifier(v) || !v.starts_with("academy."))
            || self.operations.is_empty()
            || self.operations.len() > 128
        {
            return Err(transport_refused());
        }
        for values in [&self.student_ids, &self.source_namespaces] {
            if values
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != values.len()
            {
                return Err(transport_refused());
            }
        }
        let mut ids = std::collections::HashSet::new();
        for operation in &self.operations {
            if !ids.insert(&operation.id)
                || !transport_identifier(&operation.id)
                || !self.source_namespaces.contains(&operation.source_namespace)
                || !matches!(operation.method.as_str(), "GET" | "POST")
                || !operation.path.starts_with('/')
                || operation.path.starts_with("//")
                || operation.path.contains(['?', '#', '\\', '\r', '\n', '%'])
                || operation.path.split('/').any(|v| matches!(v, "." | ".."))
                || (operation.method == "GET" && !operation.body.is_empty())
                || operation.query.len() + operation.body.len() > 64
            {
                return Err(transport_refused());
            }
            let mirrored = operation
                .mirrored_parameters
                .iter()
                .collect::<std::collections::HashSet<_>>();
            if mirrored.len() != operation.mirrored_parameters.len() {
                return Err(transport_refused());
            }
            for name in &mirrored {
                // Lead DECISION-1: this is a pinned StudyCourse exception, never
                // a generic permission to duplicate fields on arbitrary routes.
                if operation.id != "student.course.search"
                    || operation.source_namespace != "academy.lms.student_course"
                    || operation.method != "POST"
                    || operation.effect
                    || operation.path != "/servlet/controller.coursemanage.CourseManageServlet"
                    || !["reqCmd", "student_name", "cls_no", "grade_no"].contains(&name.as_str())
                {
                    return Err(transport_refused());
                }
                let a = operation
                    .query
                    .iter()
                    .find(|p| &p.name == *name)
                    .ok_or_else(transport_refused)?;
                let b = operation
                    .body
                    .iter()
                    .find(|p| &p.name == *name)
                    .ok_or_else(transport_refused)?;
                if serde_json::to_value(&a.value).ok() != serde_json::to_value(&b.value).ok() {
                    return Err(transport_refused());
                }
            }
            let selector_ok = |parameters: &[TransportParameter]| {
                parameters.iter().any(|p| p.name == "reqCmd" && matches!(&p.value, ParameterValue::Literal { value } if value == "StudyCourse"))
            };
            if !mirrored.is_empty() {
                if !selector_ok(&operation.query) {
                    return Err(transport_refused());
                }
                if !selector_ok(&operation.body) {
                    return Err(transport_refused());
                }
            }
            let mut keys = std::collections::HashSet::new();
            for (placement, parameter) in operation
                .query
                .iter()
                .map(|p| (false, p))
                .chain(operation.body.iter().map(|p| (true, p)))
            {
                if !transport_identifier(&parameter.name)
                    || !keys.insert((placement, &parameter.name))
                    || placement
                        && operation.query.iter().any(|p| p.name == parameter.name)
                        && !mirrored.contains(&parameter.name)
                {
                    return Err(transport_refused());
                }
                match &parameter.value {
                    ParameterValue::Literal { value } if value.len() > 8192 => {
                        return Err(transport_refused());
                    }
                    ParameterValue::Argument { name, max_bytes }
                        if !transport_identifier(name) || *max_bytes == 0 || *max_bytes > 65536 =>
                    {
                        return Err(transport_refused());
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }
    pub fn authorize(&self, scope: &TransportScope) -> Result<()> {
        if scope.owner_id != self.owner_id
            || scope.teacher_id != self.teacher_id
            || !self.student_ids.contains(&scope.student_id)
            || !self.source_namespaces.contains(&scope.source_namespace)
            || !valid_date(&scope.date.as_str().into())
            || scope.date < self.allowed_from
            || scope.date > self.allowed_through
        {
            Err(transport_refused())
        } else {
            Ok(())
        }
    }
    pub(crate) fn request(
        &self,
        scope: &TransportScope,
        id: &str,
        arguments: &[(String, String)],
        effect: bool,
    ) -> Result<crate::http::Request> {
        self.authorize(scope)?;
        let operation = self
            .operations
            .iter()
            .find(|op| {
                op.id == id && op.source_namespace == scope.source_namespace && op.effect == effect
            })
            .ok_or_else(transport_refused)?;
        if arguments.len() > 64
            || arguments
                .iter()
                .map(|(k, _)| k)
                .collect::<std::collections::HashSet<_>>()
                .len()
                != arguments.len()
        {
            return Err(transport_refused());
        }
        let mut used = std::collections::HashSet::new();
        let mut render = |parameters: &[TransportParameter]| -> Result<Vec<(String, String)>> {
            parameters
                .iter()
                .map(|p| {
                    let value = match &p.value {
                        ParameterValue::Literal { value } => value.clone(),
                        ParameterValue::Scope { field } => match field {
                            ScopeField::TeacherId => scope.teacher_id.clone(),
                            ScopeField::StudentId => scope.student_id.clone(),
                            ScopeField::Date => scope.date.clone(),
                        },
                        ParameterValue::Argument { name, max_bytes } => {
                            let (_, value) = arguments
                                .iter()
                                .find(|(k, _)| k == name)
                                .ok_or_else(transport_refused)?;
                            if value.len() > *max_bytes {
                                return Err(transport_refused());
                            }
                            used.insert(name.clone());
                            value.clone()
                        }
                    };
                    Ok((p.name.clone(), value))
                })
                .collect()
        };
        let query = render(&operation.query)?;
        let body = render(&operation.body)?;
        for name in &operation.mirrored_parameters {
            let a = query
                .iter()
                .find(|(key, _)| key == name)
                .ok_or_else(transport_refused)?;
            let b = body
                .iter()
                .find(|(key, _)| key == name)
                .ok_or_else(transport_refused)?;
            if a.1.as_bytes() != b.1.as_bytes() {
                return Err(transport_refused());
            }
        }
        if used.len() != arguments.len() {
            return Err(transport_refused());
        }
        Ok(crate::http::Request {
            method: operation.method.clone(),
            path: operation.path.clone(),
            query,
            body,
            headers: vec![],
            timeout: std::time::Duration::from_secs(20),
        })
    }
}
