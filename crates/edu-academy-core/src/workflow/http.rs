//! Actual action validation, approval consumption, one dispatch and authoritative readback.
use super::review::FIELDS;
use super::{approval::Approval, canonical};
use crate::{js::*, knowledge};
fn narrative(f: &str) -> bool {
    matches!(f, "progress" | "homework" | "lessonMemo")
}
fn valid_value(f: &str, v: &Js) -> bool {
    crate::decision::valid_value(f, v)
}
fn literal_key(v: &Js) -> Result<()> {
    if !v.string()
        || (!test(r"^[1-9][0-9]{0,15}$", v)
            || v.code_units().parse::<u64>().unwrap_or(u64::MAX) > 9007199254740991)
    {
        Err(Fault::error("invalid_read_key"))
    } else {
        Ok(())
    }
}
fn http_target(v: &Js) -> Result<()> {
    exact(
        v,
        &[
            "studentName",
            "date",
            "groupId",
            "teacherKey",
            "studentKey",
            "courseKey",
            "recordKey",
            "membershipKey",
        ],
    )?;
    if !v.get("studentName").string()
        || trim(v.get("studentName").units()).is_empty()
        || v.get("studentName").units().len() > 200
    {
        return Err(Fault::error("invalid_read_name"));
    }
    knowledge::calendar_date(v.get("date"))?;
    for k in [
        "groupId",
        "teacherKey",
        "studentKey",
        "courseKey",
        "recordKey",
        "membershipKey",
    ] {
        literal_key(v.get(k))?;
    }
    Ok(())
}
pub fn validate_action(a: &Js) -> Result<()> {
    exact(
        a,
        &[
            "attemptId",
            "target",
            "field",
            "effectRecordKey",
            "effectTarget",
            "before",
            "after",
            "wireDigest",
        ],
    )?;
    super::schema::effect_binding(a)?;
    require_text(a.get("attemptId"), "attempt")?;
    if !test(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,80}$", a.get("attemptId")) {
        return Err(Fault::error("journal_http_attempt_rejected"));
    }
    http_target(a.get("target"))?;
    literal_key(a.get("effectRecordKey"))?;
    let f = a.get("field").code_units();
    if !FIELDS.contains(&f.as_str())
        || !a.get("before").string()
        || !a.get("after").string()
        || !test("^[a-f0-9]{64}$", a.get("wireDigest"))
    {
        return Err(Fault::error("journal_http_action_rejected"));
    }
    if narrative(&f)
        && crate::leaf::journal_wire_width(a.get("after").units()).expect("bounded text") > 400
    {
        return Err(Fault::error("journal_http_text_too_long"));
    }
    if !narrative(&f) && !valid_value(&f, a.get("after")) {
        return Err(Fault::error(match f.as_str() {
            "attendance" => "journal_http_attendance_rejected",
            "dailyTestAssessment" => "journal_http_daily_scale_rejected",
            _ => "journal_http_homework_scale_rejected",
        }));
    }
    Ok(())
}
pub fn verified_state(a: &Js, s: &Js) -> Result<()> {
    exact(
        s,
        &[
            "target",
            "fields",
            "previousRecordKey",
            "previousDate",
            "wireDigest",
            "effectTargets",
        ],
    )?;
    http_target(s.get("target"))?;
    if digest(s.get("target"), Family::Learning)? != digest(a.get("target"), Family::Learning)?
        || s.get("wireDigest") != a.get("wireDigest")
    {
        return Err(Fault::error("journal_http_source_drift"));
    }
    if s.get("effectTargets").get(&a.get("field").code_units()) != a.get("effectTarget") {
        return Err(Fault::error("workflow_effect_target_drift"));
    }
    let previous = a.get("field").is("previousHomeworkAssessment");
    let record = if previous {
        s.get("previousRecordKey")
    } else {
        s.get("target").get("recordKey")
    };
    if a.get("effectRecordKey") != record {
        return Err(Fault::error("journal_http_effect_record_conflict"));
    }
    if previous {
        knowledge::calendar_date(s.get("previousDate"))?;
        literal_key(s.get("previousRecordKey"))?;
        if s.get("previousDate").units() >= s.get("target").get("date").units()
            || s.get("previousRecordKey") == s.get("target").get("recordKey")
        {
            return Err(Fault::error("journal_http_previous_occurrence_unverified"));
        }
    }
    if !s.get("fields").get(&a.get("field").code_units()).string() {
        return Err(Fault::error("journal_http_before_unknown"));
    }
    Ok(())
}

pub trait HttpPort {
    fn read(&mut self, target: &Js) -> Result<Js>;
    fn write(&mut self, action: &Js) -> Result<()>;
}
pub fn apply(action: &Js, port: &mut dyn HttpPort, approval: &mut dyn Approval) -> Result<Js> {
    validate_action(action)?;
    let before = port.read(action.get("target"))?;
    verified_state(action, &before)?;
    let field = action.get("field").code_units();
    if before.get("fields").get(&field) != action.get("before") {
        return Err(Fault::error("journal_http_before_drift"));
    }
    let result = |status: &str, transport: &str, after: Option<Js>| {
        let mut out = obj(&[
            ("status", status.into()),
            ("action", action.clone()),
            ("before", before.clone()),
        ]);
        if let Some(after) = after {
            out.set("after", after);
        }
        out.set("transport", transport.into());
        out.set("parentSends", Js::Num(0.0));
        out.set("automaticRetryAllowed", false.into());
        out
    };
    if action.get("before") == action.get("after") {
        return Ok(result("noop", "not_invoked", Some(before.clone())));
    }
    let decision = approval.consume(action)?;
    if !decision.get("status").is("verified")
        || !decision.get("actionDigest").is(&sha(&canonical(action)?))
    {
        return Err(Fault::error("journal_http_approval_rejected"));
    }
    let transport = if port.write(action).is_ok() {
        "returned"
    } else {
        "uncertain"
    };
    let after = match port.read(action.get("target")).and_then(|s| {
        verified_state(action, &s)?;
        Ok(s)
    }) {
        Ok(s) => s,
        Err(_) => return Ok(result("unresolved", transport, None)),
    };
    let preserved = before.get("fields").props().iter().all(|(k, v)| {
        k == &text(&field) || after.get("fields").get(&String::from_utf16_lossy(k)) == v
    });
    let status = if preserved && after.get("fields").get(&field) == action.get("after") {
        "persisted"
    } else if preserved && after.get("fields").get(&field) == action.get("before") {
        "not_persisted"
    } else {
        "conflict"
    };
    Ok(result(status, transport, Some(after)))
}
