//! Learning exports retain opaque joins and source validation order.
use crate::{instant, js::*, knowledge};
fn check(v: &Js, f: impl FnOnce(&Js) -> Result<()>) -> Result<()> {
    knowledge::assert_knowledge(v)?;
    if v.get("state").is("known") {
        f(v.get("value"))?;
    }
    Ok(())
}
fn en(v: &Js, vs: &[&str]) -> Result<()> {
    if vs.iter().any(|s| v.is(s)) {
        Ok(())
    } else {
        Err(Fault::error("invalid domain enum"))
    }
}
fn time(v: &Js) -> Result<()> {
    if test(
        r"^(?:[01][0-9]|2[0-3]):[0-5][0-9]$",
        &crate::cism::js_string(v).into(),
    ) {
        Ok(())
    } else {
        Err(Fault::error("invalid local lesson time"))
    }
}
pub fn assert_context(v: &Js) -> Result<()> {
    exact(
        v,
        &[
            "studentId",
            "lessonDate",
            "occurrenceId",
            "localStartTime",
            "timeZone",
        ],
    )?;
    require_text(v.get("studentId"), "student id")?;
    knowledge::calendar_date(v.get("lessonDate"))?;
    check(v.get("occurrenceId"), |v| require_text(v, "occurrence id"))?;
    check(v.get("localStartTime"), time)?;
    require_text(v.get("timeZone"), "time zone")?;
    // Explicit retained compatibility boundary for Intl: native callers can supply a validator.
    let tz = v.get("timeZone").code_units();
    if !timezone(&tz) {
        return Err(Fault::error("invalid time zone"));
    }
    Ok(())
}
fn timezone(tz: &str) -> bool {
    tz.parse::<chrono_tz::Tz>().is_ok()
        || regex(r"^[+-](?:[01][0-9]|2[0-3])(?::?[0-5][0-9])?$").is_match(tz)
}
pub fn validate_catalog(c: &Js) -> Result<()> {
    if c.absent() {
        return Err(property_error(c, "catalog.editions"));
    }
    let mut rows = vec![];
    for k in ["editions", "units", "pages", "problems"] {
        if !matches!(c.get(k), Js::Arr(_)) {
            return Err(Fault::type_error(
                "Spread syntax requires ...iterable not be null or undefined",
            ));
        }
        rows.extend(c.get(k).array());
    }
    let ids: Vec<_> = rows.iter().map(|r| r.get("id").clone()).collect();
    for id in &ids {
        require_text(id, "catalog id")?;
    }
    if unique(&ids).len() != ids.len() {
        return Err(Fault::error("duplicate catalog id"));
    }
    for e in c.get("editions").array() {
        require_text(e.get("title"), "edition title")?;
        check(e.get("editionLabel"), |v| require_text(v, "edition label"))?;
        knowledge::assert_refs(e.get("provenanceRefs"))?;
    }
    for r in rows.iter().skip(c.get("editions").array().len()) {
        if find(c, "editions", r.get("editionId")).is_none() {
            return Err(Fault::error("unknown catalog edition"));
        }
    }
    for u in c.get("units").array() {
        require_text(u.get("title"), "unit title")?;
        if u.get("level").is("major") {
            if !matches!(u.get("parentId"), Js::Null) {
                return Err(Fault::error("major unit cannot have a parent"));
            }
        } else {
            if !u.get("level").is("middle") && !u.get("level").is("minor") {
                return Err(Fault::error("invalid unit level"));
            }
            check(u.get("parentId"), |v| {
                let p = find(c, "units", v);
                if p.is_none_or(|p| {
                    p.get("editionId") != u.get("editionId")
                        || !p.get("level").is(if u.get("level").is("middle") {
                            "major"
                        } else {
                            "middle"
                        })
                }) {
                    Err(Fault::error(
                        "unit parent must be the preceding level of the same edition",
                    ))
                } else {
                    Ok(())
                }
            })?;
        }
    }
    for p in c.get("pages").array() {
        require_text(p.get("printedLabel"), "printed page label")?;
        check(p.get("minorUnitId"), |v| {
            if find(c, "units", v).is_none_or(|u| {
                !u.get("level").is("minor") || u.get("editionId") != p.get("editionId")
            }) {
                Err(Fault::error("invalid page unit"))
            } else {
                Ok(())
            }
        })?;
    }
    for p in c.get("problems").array() {
        require_text(p.get("printedLabel"), "printed problem label")?;
        check(p.get("pageId"), |v| {
            if find(c, "pages", v).is_none_or(|u| u.get("editionId") != p.get("editionId")) {
                Err(Fault::error("invalid problem page"))
            } else {
                Ok(())
            }
        })?;
    }
    Ok(())
}
fn find<'a>(c: &'a Js, k: &str, id: &Js) -> Option<&'a Js> {
    c.get(k).array().iter().find(|r| r.get("id") == id)
}
fn known(v: &Js) -> Option<&Js> {
    if v.get("state").is("known") {
        Some(v.get("value"))
    } else {
        None
    }
}
pub fn validate_scope(s: &Js, c: Option<&Js>) -> Result<()> {
    exact(s, &["editionId", "unitId", "pageIds", "problemIds"])?;
    check(s.get("editionId"), |v| require_text(v, "scope edition"))?;
    check(s.get("unitId"), |v| require_text(v, "scope unit"))?;
    for k in ["pageIds", "problemIds"] {
        check(s.get(k), |v| {
            if !matches!(v, Js::Arr(_)) {
                return Err(Fault::error("scope needs an array of opaque ids"));
            }
            for id in v.array() {
                require_text(id, "scope content id")?;
            }
            if unique(v.array()).len() != v.array().len() {
                return Err(Fault::error("duplicate scope id"));
            }
            Ok(())
        })?;
    }
    let Some(c) = c.filter(|c| c.truthy()) else {
        return Ok(());
    };
    validate_catalog(c)?;
    let edition = known(s.get("editionId"));
    let unit = known(s.get("unitId"));
    if edition.is_some_and(|id| id.truthy() && find(c, "editions", id).is_none()) {
        return Err(Fault::error("unknown scope edition"));
    }
    let mut scoped = vec![];
    if let Some(id) = unit.filter(|v| v.truthy()) {
        scoped.push(find(c, "units", id));
    }
    for (k, rows) in [("pageIds", "pages"), ("problemIds", "problems")] {
        if let Some(ids) = known(s.get(k)) {
            for id in ids.array() {
                scoped.push(find(c, rows, id));
            }
        }
    }
    if scoped
        .iter()
        .any(|r| r.is_none_or(|r| edition.is_some_and(|e| r.get("editionId") != e)))
    {
        return Err(Fault::error("scope catalog mismatch"));
    }
    let editions: Vec<_> = scoped
        .iter()
        .flatten()
        .map(|r| r.get("editionId").clone())
        .collect();
    if unique(&editions).len() > 1 {
        return Err(Fault::error("scope mixes editions"));
    }
    let mut pages = known(s.get("pageIds")).map_or(vec![], |v| v.array().to_vec());
    if let Some(problems) = known(s.get("problemIds")) {
        for id in problems.array() {
            let p = find(c, "problems", id).expect("validated scope");
            if let Some(page) = known(p.get("pageId")) {
                if let Some(ids) = known(s.get("pageIds"))
                    && !ids.array().is_empty()
                    && !ids.array().contains(page)
                {
                    return Err(Fault::error("problem lies outside the declared page scope"));
                }
                if !pages.contains(page) {
                    pages.push(page.clone());
                }
            }
        }
    }
    if let Some(id) = unit.filter(|v| v.truthy()) {
        let target = find(c, "units", id).expect("validated scope");
        let level = |u: &Js| {
            if u.get("level").is("major") {
                0
            } else if u.get("level").is("middle") {
                1
            } else {
                2
            }
        };
        for page in pages {
            let p = find(c, "pages", &page).expect("validated page");
            if let Some(minor) = known(p.get("minorUnitId")) {
                let mut current = find(c, "units", minor);
                while let Some(u) = current {
                    if u.get("id") == id {
                        break;
                    }
                    if level(u) <= level(target) || u.get("level").is("major") {
                        return Err(Fault::error("page lies outside the declared unit scope"));
                    }
                    current = known(u.get("parentId")).and_then(|id| find(c, "units", id));
                }
            }
        }
    }
    Ok(())
}
pub fn validate_record(r: &Js) -> Result<()> {
    encode(r, Family::Learning, &mut vec![])?;
    if r.get("kind").is("book_assignment") {
        exact(
            r,
            &[
                "kind",
                "studentId",
                "editionId",
                "validFrom",
                "validUntil",
                "authoredBy",
                "evidenceRefs",
            ],
        )?;
        require_text(r.get("studentId"), "student id")?;
        check(r.get("editionId"), |v| require_text(v, "edition id"))?;
        knowledge::calendar_date(r.get("validFrom"))?;
        check(r.get("validUntil"), |v| {
            knowledge::calendar_date(v)?;
            if v.units() < r.get("validFrom").units() {
                Err(Fault::error("assignment ends before it begins"))
            } else {
                Ok(())
            }
        })?;
        en(r.get("authoredBy"), &["source", "teacher"])?;
        knowledge::assert_refs(r.get("evidenceRefs"))?;
        return Ok(());
    }
    assert_context(r.get("context"))?;
    validate_scope(r.get("scope"), None)?;
    if r.get("kind").is("teacher_plan") {
        exact(
            r,
            &[
                "kind",
                "context",
                "teacherId",
                "horizon",
                "purpose",
                "status",
                "scope",
                "assignmentId",
                "basedOnRevisionIds",
                "dueDate",
            ],
        )?;
        require_text(r.get("teacherId"), "teacher id")?;
        en(r.get("horizon"), &["present", "future"])?;
        en(
            r.get("purpose"),
            &["lesson", "homework", "review", "makeup"],
        )?;
        en(r.get("status"), &["draft", "approved", "cancelled"])?;
        check(r.get("assignmentId"), |v| require_text(v, "assignment id"))?;
        check(r.get("dueDate"), |v| {
            knowledge::calendar_date(v).map(|_| ())
        })?;
        let basis = r.get("basedOnRevisionIds");
        if !matches!(basis, Js::Arr(_)) {
            return Err(Fault::error("plan basis needs revision ids"));
        }
        for id in basis.array() {
            require_text(id, "basis revision id")?;
        }
        if unique(basis.array()).len() != basis.array().len() {
            return Err(Fault::error("duplicate basis revision"));
        }
        return Ok(());
    }
    if !r.get("kind").is("source_observation") {
        return Err(Fault::error("unknown learning record kind"));
    }
    exact(
        r,
        &[
            "kind",
            "context",
            "sessionKind",
            "makeupForOccurrenceId",
            "scope",
            "provenance",
            "exactBinding",
            "attendance",
            "activity",
            "correctionChecked",
            "correctness",
        ],
    )?;
    en(r.get("sessionKind"), &["lesson", "makeup"])?;
    check(r.get("makeupForOccurrenceId"), |v| {
        require_text(v, "makeup origin")
    })?;
    let p = r.get("provenance");
    exact(
        p,
        &[
            "system",
            "recordKey",
            "observedAt",
            "sourceTimestamp",
            "contractRef",
            "contentDigest",
        ],
    )?;
    en(
        p.get("system"),
        &[
            "lms_day_record",
            "lms_makeup",
            "spt_classroom",
            "app_submission",
            "teacher_observation",
        ],
    )?;
    require_text(p.get("recordKey"), "source record key")?;
    require_text(p.get("contractRef"), "source contract")?;
    instant::assert_instant(p.get("observedAt"))?;
    check(p.get("sourceTimestamp"), |v| {
        instant::assert_instant(v)?;
        if instant::epoch_ms(v, instant::Grammar::Learning)
            > instant::epoch_ms(p.get("observedAt"), instant::Grammar::Learning)
        {
            Err(Fault::error("source timestamp is later than observation"))
        } else {
            Ok(())
        }
    })?;
    check(p.get("contentDigest"), |v| {
        if test("^[a-f0-9]{64}$", v) {
            Ok(())
        } else {
            Err(Fault::error("invalid source digest"))
        }
    })?;
    check(r.get("exactBinding"), |v| {
        exact(v, &["studentId", "lessonDate", "occurrenceId"])?;
        let c = r.get("context");
        if !c.get("occurrenceId").get("state").is("known")
            || v.get("studentId") != c.get("studentId")
            || v.get("lessonDate") != c.get("lessonDate")
            || v.get("occurrenceId") != c.get("occurrenceId").get("value")
        {
            Err(Fault::error(
                "source binding does not match exact student and lesson",
            ))
        } else {
            Ok(())
        }
    })?;
    check(r.get("attendance"), |v| {
        en(v, &["held", "cancelled", "absent"])
    })?;
    en(
        r.get("activity"),
        &[
            "assigned",
            "attempted",
            "reported_complete",
            "teacher_inspected",
        ],
    )?;
    check(r.get("correctionChecked"), |v| {
        if matches!(v, Js::Bool(_)) {
            Ok(())
        } else {
            Err(Fault::error("invalid correction check"))
        }
    })?;
    check(r.get("correctness"), |v| {
        en(v, &["correct", "incorrect", "partial"])
    })?;
    if r.get("activity").is("teacher_inspected") && !p.get("system").is("teacher_observation") {
        return Err(Fault::error(
            "teacher inspection requires an explicit teacher observation",
        ));
    }
    Ok(())
}
pub fn call(name: &str, args: &[Js]) -> Result<Js> {
    let undef = Js::Undefined;
    let a = |i: usize| args.get(i).unwrap_or(&undef);
    let void = || Ok(Js::Undefined);
    match name {
        "stableId" => {
            for v in args {
                require_text(v, "identity component")?;
            }
            if args.len() < 3 {
                return Err(Fault::error("identity needs a verified key"));
            }
            Ok(format!(
                "{}_{}",
                a(0).code_units(),
                digest(&Js::Arr(args[1..].to_vec()), Family::Learning)?
            )
            .into())
        }
        "known" => knowledge::known(a(0), a(1)),
        "unknown" => knowledge::unknown(a(0)),
        "immutableCopy" => knowledge::immutable_copy(a(0)),
        "canonicalJson" => Ok(encode(a(0), Family::Learning, &mut vec![])?
            .expect("learning bytes")
            .into()),
        "digest" => Ok(digest(a(0), Family::Learning)?.into()),
        "parseCalendarDate" => knowledge::calendar_date(a(0)),
        "assertDate" => {
            knowledge::calendar_date(a(0))?;
            void()
        }
        "assertTime" => {
            time(a(0))?;
            void()
        }
        "assertInstant" => {
            instant::assert_instant(a(0))?;
            void()
        }
        "requireText" => {
            require_text(a(0), &a(1).code_units())?;
            void()
        }
        "assertRefs" => {
            knowledge::assert_refs(a(0))?;
            void()
        }
        "assertKnowledge" => {
            knowledge::assert_knowledge(a(0))?;
            void()
        }
        "assertExactKeys" => {
            let ks: Vec<_> = a(1).array().iter().map(Js::code_units).collect();
            let refs: Vec<_> = ks.iter().map(String::as_str).collect();
            exact(a(0), &refs)?;
            void()
        }
        "assertContext" => {
            assert_context(a(0))?;
            void()
        }
        "validateCatalog" => {
            validate_catalog(a(0))?;
            void()
        }
        "validateScope" => {
            validate_scope(a(0), args.get(1))?;
            void()
        }
        "validateRecord" => {
            validate_record(a(0))?;
            void()
        }
        "studentOf" => {
            if a(0).absent() {
                return Err(property_error(a(0), "record.kind"));
            }
            if a(0).get("kind").is("book_assignment") {
                Ok(a(0).get("studentId").clone())
            } else {
                if a(0).get("context").absent() {
                    return Err(property_error(
                        a(0).get("context"),
                        "record.context.studentId",
                    ));
                }
                Ok(a(0).get("context").get("studentId").clone())
            }
        }
        "contextFromLessonKey" => {
            if a(0).absent() {
                return Err(property_error(a(0), "key.studentId"));
            }
            let c = obj(&[
                ("studentId", a(0).get("studentId").clone()),
                ("lessonDate", a(0).get("lessonDate").clone()),
                (
                    "occurrenceId",
                    knowledge::known(a(0).get("occurrenceId"), a(3))?,
                ),
                ("localStartTime", a(2).clone()),
                ("timeZone", a(1).clone()),
            ]);
            assert_context(&c)?;
            knowledge::immutable_copy(&c)
        }
        "resolveProblemPath" => {
            let c = a(0);
            validate_catalog(c)?;
            let p = find(c, "problems", a(1)).ok_or_else(|| Fault::error("unknown problem"))?;
            let page = known(p.get("pageId"))
                .filter(|v| v.truthy())
                .and_then(|id| find(c, "pages", id));
            let minor = page
                .and_then(|p| known(p.get("minorUnitId")))
                .filter(|v| v.truthy())
                .and_then(|id| find(c, "units", id));
            let parent = |u: Option<&Js>| {
                u.filter(|u| !u.get("level").is("major"))
                    .and_then(|u| known(u.get("parentId")))
                    .filter(|v| v.truthy())
                    .and_then(|id| find(c, "units", id))
            };
            let middle = parent(minor);
            let major = parent(middle);
            let val = |v: Option<&Js>| v.cloned().unwrap_or(Js::Null);
            knowledge::immutable_copy(&obj(&[
                ("editionId", p.get("editionId").clone()),
                ("major", val(major)),
                ("middle", val(middle)),
                ("minor", val(minor)),
                ("page", val(page)),
                ("problem", p.clone()),
            ]))
        }
        _ => Err(Fault::admission("unknown_learning_export")),
    }
}

fn property_error(v: &Js, expression: &str) -> Fault {
    Fault::type_error(format!(
        "{} is not an object (evaluating '{expression}')",
        if matches!(v, Js::Null) {
            "null"
        } else {
            "undefined"
        }
    ))
}
/// Native deeply immutable, detached counterpart. JS descriptor observers keep the original TS port.
pub fn known_frozen(value: &Js, refs: &Js) -> Result<crate::frozen::FrozenValue> {
    Ok(crate::frozen::FrozenValue::from_validated(
        knowledge::known(value, refs)?,
    ))
}
pub fn unknown_frozen(reason: &Js) -> Result<crate::frozen::FrozenValue> {
    Ok(crate::frozen::FrozenValue::from_validated(
        knowledge::unknown(reason)?,
    ))
}
pub fn context_from_lesson_key(
    key: &Js,
    time_zone: &Js,
    start: &Js,
    refs: &Js,
) -> Result<crate::frozen::FrozenValue> {
    Ok(crate::frozen::FrozenValue::from_validated(call(
        "contextFromLessonKey",
        &[key.clone(), time_zone.clone(), start.clone(), refs.clone()],
    )?))
}
pub fn resolve_problem_path(catalog: &Js, id: &Js) -> Result<crate::frozen::FrozenValue> {
    Ok(crate::frozen::FrozenValue::from_validated(call(
        "resolveProblemPath",
        &[catalog.clone(), id.clone()],
    )?))
}
pub fn stable_id(kind: &Js, namespace: &Js, keys: &[Js]) -> Result<Js> {
    let mut args = vec![kind.clone(), namespace.clone()];
    args.extend_from_slice(keys);
    call("stableId", &args)
}
pub fn canonical_json(value: &Js) -> Result<String> {
    encode(value, Family::Learning, &mut vec![])?
        .ok_or_else(|| Fault::error("only finite JSON data is allowed"))
}
pub fn learning_digest(value: &Js) -> Result<String> {
    digest(value, Family::Learning)
}
