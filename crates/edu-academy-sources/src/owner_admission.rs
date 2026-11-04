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
