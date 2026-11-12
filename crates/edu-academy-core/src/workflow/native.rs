//! Selected native backend algorithm over an explicitly supplied owner source port.
use super::{Runtime, approval::Approval, service::Backend};
use crate::js::{Fault, Js, Result, obj, stringify};
pub trait SourcePort {
    fn authenticate(&mut self) -> Result<()>;
    fn discover_groups(&mut self, input: &Js) -> Result<Js>;
    fn execute(&mut self, input: &Js) -> Result<Js>;
    fn journal_state(&mut self, target: &Js) -> Result<Js>;
    /// Mandatory approved owner application; the owner client owns action/readback.
    fn apply(&mut self, action: &Js, approval: &mut dyn Approval) -> Result<Js>;
}
fn target_of(v: &Js) -> Js {
    obj(&[
        "studentName",
        "date",
        "groupId",
        "teacherKey",
        "studentKey",
        "courseKey",
        "recordKey",
        "membershipKey",
    ]
    .map(|k| (k, v.get(k).clone())))
}
pub struct NativeBackend<'a> {
    pub port: &'a mut dyn SourcePort,
    pub admission: Js,
    pub owner_admission: Js,
    pub managed_names: Vec<Js>,
    pub runtime: &'a dyn Runtime,
    authenticated: bool,
}
impl<'a> NativeBackend<'a> {
    pub fn new(
        port: &'a mut dyn SourcePort,
        admission: Js,
        owner_admission: Js,
        managed_names: Vec<Js>,
        runtime: &'a dyn Runtime,
    ) -> Self {
        Self {
            port,
            admission,
            owner_admission,
            managed_names,
            runtime,
            authenticated: false,
        }
    }
}
impl Backend for NativeBackend<'_> {
    fn configured(&self) -> bool {
        true
    }
    fn save(
        &mut self,
        scope: &Js,
        patches: &Js,
        approval: &mut dyn Approval,
        subject: &Js,
    ) -> Result<Js> {
        for patch in patches.array() {
            super::schema::effect_binding(patch)?;
        }
        let student = self
            .admission
            .get("students")
            .array()
            .iter()
            .find(|s| s.get("studentId") == scope.get("studentId"))
            .cloned()
            .or_else(|| {
                if self
                    .owner_admission
                    .get("schemaVersion")
                    .is("academy.owner-admission/2")
                    && self
                        .owner_admission
                        .get("studentScope")
                        .is("source_verified")
                    && self.managed_names.contains(subject.get("displayName"))
                {
                    Some(obj(&[
                        ("studentId", scope.get("studentId").clone()),
                        ("displayName", subject.get("displayName").clone()),
                    ]))
                } else {
                    None
                }
            });
        let owner = &self.owner_admission;
        let date = scope.get("lessonDate");
        let admitted = if owner.absent() {
            date == self.admission.get("date")
        } else {
            (owner.get("allowedFrom").absent() || owner.get("allowedFrom").units() <= date.units())
                && (owner.get("allowedThrough").absent()
                    || owner.get("allowedThrough").units() >= date.units())
        };
        if !scope.get("sourceNamespace").is("academy.lms.dayrecord")
            || scope.get("teacherId") != self.admission.get("teacherId")
            || !admitted
            || student.is_none()
        {
            return Err(Fault::error("native_dailyrecord_scope_refused"));
        }
        let student = student.expect("qualified student");
        if !self.authenticated {
            self.port.authenticate()?;
            self.authenticated = true;
        }
        let selector = self.port.discover_groups(&obj(&[
            ("teacherLabel", self.admission.get("teacherLabel").clone()),
            ("date", date.clone()),
        ]))?;
        let rows = self.port.execute(&obj(&[
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
            (
                "studentNames",
                Js::Arr(vec![student.get("displayName").clone()]),
            ),
        ]))?;
        if rows.get("dayRecords").array().len() != 1 {
            return Err(Fault::error("native_dailyrecord_occurrence_unverified"));
        }
        let row = &rows.get("dayRecords").array()[0];
        if row.get("studentKey") != scope.get("studentId")
            || row.get("studentName") != student.get("displayName")
            || row.get("teacherKey") != scope.get("teacherId")
            || row.get("date") != date
        {
            return Err(Fault::error("native_dailyrecord_identity_conflict"));
        }
        let target = target_of(row);
        let state = self.port.journal_state(&target)?;
        if stringify(&target_of(state.get("target")))? != stringify(&target)? {
            return Err(Fault::error("native_dailyrecord_state_conflict"));
        }
        if patches.array().iter().any(|p| {
            p.get("wireDigest") != state.get("wireDigest")
                || state.get("fields").get(&p.get("field").code_units()) != p.get("before")
        }) {
            return Ok(Js::Arr(
                patches
                    .array()
                    .iter()
                    .map(|p| {
                        obj(&[
                            ("field", p.get("field").clone()),
                            ("status", "before_drift".into()),
                            ("dispatched", false.into()),
                            ("transport", "not_invoked".into()),
                        ])
                    })
                    .collect(),
            ));
        }
        if patches.array().iter().any(|p| {
            state.get("effectTargets").get(&p.get("field").code_units()) != p.get("effectTarget")
        }) {
            return Err(Fault::error("workflow_effect_target_drift"));
        }
        let mut fields = vec![];
        for (i, p) in patches.array().iter().enumerate() {
            let action = obj(&[
                ("attemptId", self.runtime.uuid()?.into()),
                ("target", target.clone()),
                ("field", p.get("field").clone()),
                ("effectRecordKey", p.get("effectRecordKey").clone()),
                ("effectTarget", p.get("effectTarget").clone()),
                ("before", p.get("before").clone()),
                ("after", p.get("value").clone()),
                ("wireDigest", state.get("wireDigest").clone()),
            ]);
            let result = self.port.apply(&action, approval);
            let (status, dispatched, transport) = match result {
                Ok(r) => (
                    r.get("status").clone(),
                    !r.get("transport").is("not_invoked"),
                    r.get("transport").clone(),
                ),
                Err(_) => ("unresolved".into(), true, "uncertain".into()),
            };
            fields.push(obj(&[
                ("field", p.get("field").clone()),
                ("status", status.clone()),
                ("dispatched", dispatched.into()),
                ("transport", transport),
            ]));
            if !status.is("persisted") && !status.is("noop") {
                for p in patches.array().iter().skip(i + 1) {
                    fields.push(obj(&[
                        ("field", p.get("field").clone()),
                        ("status", "not_attempted".into()),
                        ("dispatched", false.into()),
                        ("transport", "not_invoked".into()),
                    ]));
                }
                break;
            }
        }
        Ok(Js::Arr(fields))
    }
}
