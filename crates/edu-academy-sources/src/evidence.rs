//! Unadmitted evidence data. No authority, store, session, grant or admission types.
use crate::data::*;
use crate::owner_root::Capture;
#[derive(Clone)]
pub struct OwnerAssociation {
    pub account: Text,
    pub instance: Text,
    pub existing_owner_evidence: Js,
}
/// Exact tuple namespace; never silently changes a legacy native/saved identity.
pub fn namespace(association: &OwnerAssociation) -> Js {
    obj(&[
        ("account", Js::Str(association.account.clone())),
        ("instance", Js::Str(association.instance.clone())),
    ])
}
/// Select a published field from captured JSON and recompute it, rather than trusting
/// caller-supplied projected values. JSON-pointer identity uses exact UTF-16 keys.
pub fn json_pointer(root: &Js, pointer: &str) -> Result<Js> {
    if pointer.is_empty() {
        return Ok(root.clone());
    }
    if !pointer.starts_with('/') {
        return Err(Error::error("evidence_pointer_invalid"));
    }
    let mut value = root;
    for component in pointer[1..].split('/') {
        let key = component.replace("~1", "/").replace("~0", "~");
        value = if let Js::Arr(a) = value {
            if key != "0" && (key.starts_with('0') || key.is_empty()) {
                return Err(Error::error("evidence_pointer_missing"));
            }
            a.get(
                key.parse::<usize>()
                    .map_err(|_| Error::error("evidence_pointer_invalid"))?,
            )
            .ok_or_else(|| Error::error("evidence_pointer_missing"))?
        } else {
            value
                .props()
                .iter()
                .find(|(k, _)| *k == text(&key))
                .map(|(_, v)| v)
                .ok_or_else(|| Error::error("evidence_pointer_missing"))?
        };
    }
    Ok(value.clone())
}
fn explicit_association(association: &OwnerAssociation) -> Result<()> {
    if association.account.is_empty()
        || association.instance.is_empty()
        || !object(&association.existing_owner_evidence)
    {
        return Err(Error::error(
            "evidence_explicit_account_association_required",
        ));
    }
    Ok(())
}
pub fn project_json(
    association: &OwnerAssociation,
    captures: &[Capture],
    capture: usize,
    pointer: &str,
    legacy_identity: &Js,
) -> Result<Js> {
    explicit_association(association)?;
    let c = captures
        .get(capture)
        .ok_or_else(|| Error::error("evidence_capture_missing"))?;
    let parsed = c.json()?;
    let recomputed = json_pointer(&parsed, pointer)?;
    let manifest = captures
        .iter()
        .map(|c| {
            obj(&[
                ("path", c.relative_path.clone().into()),
                ("sha256", c.sha256.clone().into()),
                ("byteLength", c.bytes.len().into()),
                ("identity", c.identity.value()),
            ])
        })
        .collect();
    Ok(obj(&[
        ("status", "UNADMITTED".into()),
        ("sourceAccount", namespace(association)),
        (
            "existingOwnerAssociation",
            association.existing_owner_evidence.clone(),
        ),
        ("legacyIdentity", legacy_identity.clone()),
        ("capturedBytesBase64", base64(&c.bytes, false).into()),
        ("capturedSha256", c.sha256.clone().into()),
        ("jsonPointer", pointer.into()),
        ("recomputedValue", recomputed),
        ("sourceManifest", Js::Arr(manifest)),
        ("authority", "none".into()),
        ("nativeOwnerVerified", false.into()),
    ]))
}
pub fn raw_utf16(bytes: &[u8], start: usize, end: usize) -> Result<Js> {
    let decoded = String::from_utf8_lossy(bytes);
    let units = text(&decoded);
    if start > end || end > units.len() {
        return Err(Error::error("evidence_source_range_invalid"));
    }
    Ok(obj(&[
        ("capturedBytesBase64", base64(bytes, false).into()),
        ("capturedSha256", hash(bytes).into()),
        ("rangeUnit", "utf16_code_units".into()),
        (
            "sourceRange",
            obj(&[("start", start.into()), ("end", end.into())]),
        ),
        ("raw", Js::Str(units[start..end].to_vec())),
    ]))
}
/// Recompute the selected physical row and fields from captured predecode Main/XML
/// bytes. Account association remains explicit UNADMITTED owner evidence data.
pub fn project_day_record(
    association: &OwnerAssociation,
    captures: &[Capture],
    main: usize,
    xml: usize,
    selection: &Js,
) -> Result<Js> {
    use crate::{dom, http};
    explicit_association(association)?;
    let main = captures
        .get(main)
        .ok_or_else(|| Error::error("evidence_capture_missing"))?;
    let xml = captures
        .get(xml)
        .ok_or_else(|| Error::error("evidence_capture_missing"))?;
    let decoded = String::from_utf8_lossy(&main.bytes);
    let doc = dom::Document::parse(&decoded);
    let links = doc
        .select("[onclick]")
        .into_iter()
        .filter(|e| {
            e.value()
                .attr("onclick")
                .unwrap_or("")
                .contains("onOpenDaysMain")
                && selection.get("studentName").units() == trim(&text(&dom::content(*e)))
        })
        .collect::<Vec<_>>();
    if links.len() != 1 {
        return Err(Error::error("evidence_physical_row_ambiguous"));
    }
    let row = dom::closest(links[0], "tr")
        .ok_or_else(|| Error::error("evidence_physical_row_missing"))?;
    http::validate_headers(row)?;
    let (fields, tuple) = http::bind_row(&doc, row)?;
    for (selector, key) in [
        ("[name=std_date]", "date"),
        ("[name=grp_seq]", "groupId"),
        ("[name=teacher_pri_no]", "teacherKey"),
    ] {
        let nodes = doc.select(selector);
        if nodes.len() != 1 || !selection.get(key).is(&dom::value(nodes[0])) {
            return Err(Error::error("evidence_source_scope_conflict"));
        }
    }
    let xml_text = String::from_utf8_lossy(&xml.bytes);
    let record = http::xml_record(&xml_text)?;
    let parts = record.split('!').collect::<Vec<_>>();
    for (index, value) in [
        (1, selection.get("date")),
        (2, tuple.get("course_seq")),
        (4, selection.get("studentName")),
        (9, selection.get("groupId")),
        (10, tuple.get("record_seq")),
        (11, tuple.get("stu_pri_no")),
        (12, tuple.get("cm_seq")),
    ] {
        if parts.get(index).is_none_or(|s| !value.is(s)) {
            return Err(Error::error("evidence_independent_occurrence_conflict"));
        }
    }
    let legacy = obj(&[
        ("studentName", selection.get("studentName").clone()),
        ("date", selection.get("date").clone()),
        ("groupId", selection.get("groupId").clone()),
        ("teacherKey", selection.get("teacherKey").clone()),
        ("studentKey", tuple.get("stu_pri_no").clone()),
        ("courseKey", tuple.get("course_seq").clone()),
        ("recordKey", tuple.get("record_seq").clone()),
        ("membershipKey", tuple.get("cm_seq").clone()),
    ]);
    let mut spans = vec![];
    for e in dom::select(row, "[onfocusout],[onchange],[onclick]") {
        let Some(r) = doc.range(e) else {
            return Err(Error::error("evidence_raw_location_unqualified"));
        };
        spans.push(obj(&[
            ("tag", e.value().name().into()),
            (
                "eventSource",
                e.value()
                    .attr("onclick")
                    .or(e.value().attr("onfocusout"))
                    .or(e.value().attr("onchange"))
                    .map_or(Js::Null, Js::from),
            ),
            ("sourceRange", r.value()),
            ("raw", doc.raw(&r)),
            (
                "recomputedValue",
                if matches!(e.value().name(), "input" | "select" | "textarea") {
                    dom::value(e).into()
                } else {
                    Js::Null
                },
            ),
        ]));
    }
    let manifest = captures
        .iter()
        .map(|c| {
            obj(&[
                ("path", c.relative_path.clone().into()),
                ("sha256", c.sha256.clone().into()),
                ("byteLength", c.bytes.len().into()),
                ("identity", c.identity.value()),
            ])
        })
        .collect();
    Ok(obj(&[
        ("status", "UNADMITTED".into()),
        ("sourceAccount", namespace(association)),
        (
            "existingOwnerAssociation",
            association.existing_owner_evidence.clone(),
        ),
        ("legacyIdentity", legacy),
        ("physicalTuple", tuple),
        (
            "selectors",
            obj(&[
                ("studentLink", "[onclick]:literal_onOpenDaysMain".into()),
                ("date", "[name=std_date]".into()),
                ("group", "[name=grp_seq]".into()),
                ("teacher", "[name=teacher_pri_no]".into()),
            ]),
        ),
        ("fields", fields),
        ("rangeUnit", "utf16_code_units".into()),
        ("rawControls", Js::Arr(spans)),
        ("capturedMainBytesBase64", base64(&main.bytes, false).into()),
        ("capturedXmlBytesBase64", base64(&xml.bytes, false).into()),
        ("capturedMainSha256", main.sha256.clone().into()),
        ("capturedXmlSha256", xml.sha256.clone().into()),
        ("sourceManifest", Js::Arr(manifest)),
        ("authority", "none".into()),
        ("nativeOwnerVerified", false.into()),
    ]))
}
