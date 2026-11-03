//! New, authored-only data policy checks. In-memory fictional configuration only.
use edu_academy_sources::{
    data::*,
    native::{Backend, RecordedPort},
    owner_admission::*,
};

fn owner(version: &str) -> Js {
    obj(&[
        ("schemaVersion", version.into()),
        ("decider", "user".into()),
        ("decidedAt", "2040-01-01".into()),
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("failClosedDefault", "2026-10-23".into()),
        ("allowedFrom", "2040-01-01".into()),
        ("allowedThrough", "2040-01-31".into()),
    ])
}
fn admission() -> Js {
    obj(&[
        ("teacherId", "99001".into()),
        ("teacherLabel", "Fictional Teacher".into()),
        ("date", "2040-01-04".into()),
        ("students", Js::Arr(vec![])),
    ])
}
fn parsed(v: &Js) -> OwnerAdmission {
    OwnerAdmission::parse(v).unwrap()
}

#[test]
fn owner_v1_v2_nullable_bounds_and_inclusive_calendar_policy() {
    let v1 = parsed(&owner("academy.owner-admission/1"));
    for (date, admitted) in [
        ("2039-12-31", false),
        ("2040-01-01", true),
        ("2040-01-31", true),
        ("2040-02-01", false),
        ("2040-02-30", false),
    ] {
        assert_eq!(is_date_admitted(&date.into(), Some(&v1)), admitted);
        assert_eq!(
            admission_blocker(&date.into(), Some(&v1)).is_none(),
            admitted
        );
    }
    for (from, through, date, expected) in [
        (Js::Null, Js::Null, "2040-02-29", true),
        (Js::Null, "2040-01-31".into(), "1900-01-01", true),
        ("2040-01-01".into(), Js::Null, "2100-01-01", true),
        ("2040-01-01".into(), Js::Null, "2039-01-01", false),
        (Js::Null, Js::Null, "2100-02-29", false),
    ] {
        let mut value = owner("academy.owner-admission/2");
        value.set("allowedFrom", from);
        value.set("allowedThrough", through);
        assert_eq!(
            is_date_admitted(&date.into(), Some(&parsed(&value))),
            expected
        );
    }
    assert!(is_date_admitted(&"2026-10-23".into(), None));
    assert!(!is_date_admitted(&"2040-01-04".into(), None));
}

#[test]
fn owner_schema_rejects_missing_null_unknown_invalid_fields_with_zod_parts() {
    let invalid = [
        ("decider", "assistant".into(), "invalid_value"),
        ("decidedAt", "2040-02-30".into(), "custom"),
        ("sourceNamespace", "other".into(), "invalid_value"),
        ("teacherId", "0".into(), "invalid_format"),
        ("failClosedDefault", "2040-01-04".into(), "invalid_value"),
        ("note", "x".repeat(501).into(), "too_big"),
        ("note", Js::Null, "invalid_type"),
        ("allowedFrom", Js::Undefined, "invalid_type"),
        ("allowedThrough", Js::Null, "invalid_type"),
    ];
    for (key, field, code) in invalid {
        let mut value = owner("academy.owner-admission/1");
        value.set(key, field);
        let error = OwnerAdmission::parse(&value).err().unwrap();
        assert_eq!(error.name, "ZodError");
        let issue = &error.properties.get("issues").array()[0];
        assert!(issue.get("code").is(code));
        assert_eq!(issue.get("path"), &Js::Arr(vec![key.into()]));
        assert!(!issue.get("message").units().is_empty());
    }
    let mut inverted = owner("academy.owner-admission/2");
    inverted.set("allowedFrom", "2040-02-01".into());
    let e = OwnerAdmission::parse(&inverted).err().unwrap();
    assert!(
        e.properties.get("issues").array()[0]
            .get("message")
            .is("admission_range_inverted")
    );
    let mut extra = owner("academy.owner-admission/1");
    extra.set("studentScope", "source_verified".into());
    assert!(
        OwnerAdmission::parse(&extra)
            .err()
            .unwrap()
            .properties
            .get("issues")
            .array()[0]
            .get("code")
            .is("unrecognized_keys")
    );
    let mut v2 = owner("academy.owner-admission/2");
    v2.set("studentScope", "all".into());
    assert!(OwnerAdmission::parse(&v2).is_err());
    v2.set("studentScope", Js::Null);
    assert!(OwnerAdmission::parse(&v2).is_err());
    v2.set("studentScope", Js::Undefined);
    assert!(OwnerAdmission::parse(&v2).is_ok());
    let mut missing = owner("academy.owner-admission/2");
    if let Js::Obj(p) = &mut missing {
        p.retain(|(k, _)| k != &text("allowedFrom"));
    }
    assert!(OwnerAdmission::parse(&missing).is_err());
    assert!(
        OwnerAdmission::parse(&obj(&[(
            "schemaVersion",
            "academy.owner-admission/3".into()
        )]))
        .is_err()
    );
}

#[test]
fn owner_launch_match_and_routing_preserve_selected_date_and_v2_student_scope() {
    let launch = admission();
    let backend =
        Backend::with_owner(RecordedPort::new(vec![]), launch.clone(), None, vec![]).unwrap();
    assert_eq!(
        backend.read_routing_admission(),
        obj(&[
            ("sourceNamespace", "academy.lms.dayrecord".into()),
            ("teacherId", "99001".into()),
            ("allowedFrom", "2040-01-04".into()),
            ("allowedThrough", "2040-01-04".into()),
        ])
    );
    for version in ["academy.owner-admission/1", "academy.owner-admission/2"] {
        let mut v = owner(version);
        let backend =
            Backend::with_owner(RecordedPort::new(vec![]), launch.clone(), Some(&v), vec![])
                .unwrap();
        let routing = backend.read_routing_admission();
        assert!(routing.get("allowedFrom").is("2040-01-01"));
        assert!(routing.get("allowedThrough").is("2040-01-31"));
        assert_eq!(routing.has("studentScope"), version.ends_with("/2"));
        if version.ends_with("/2") {
            assert!(routing.get("studentScope").is("configured"));
            v.set("allowedFrom", Js::Null);
            v.set("allowedThrough", Js::Null);
            v.set("studentScope", "source_verified".into());
            let backend =
                Backend::with_owner(RecordedPort::new(vec![]), launch.clone(), Some(&v), vec![])
                    .unwrap();
            assert!(
                backend
                    .read_routing_admission()
                    .get("studentScope")
                    .is("source_verified")
            );
            assert_eq!(
                backend.read_routing_admission().get("allowedFrom"),
                &Js::Null
            );
        }
        for (key, field) in [("teacherId", "99002"), ("allowedFrom", "2040-01-10")] {
            let mut mismatch = owner(version);
            mismatch.set(key, field.into());
            let error = Backend::with_owner(
                RecordedPort::new(vec![]),
                launch.clone(),
                Some(&mismatch),
                vec![],
            )
            .err()
            .unwrap();
            assert_eq!(error.message, "native_dailyrecord_owner_admission_mismatch");
        }
    }
}
