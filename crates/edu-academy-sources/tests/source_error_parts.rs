//! Semantic error parts and the real Client before-read boundary; fictional transport only.
//! Authored only, not executed by the source worker.
use edu_academy_core::{
    js::{Fault, Result as CoreResult},
    workflow::{approval::Approval, read_failure},
};
use edu_academy_sources::{
    data::*,
    http::{Client, ORIGIN, Request, Response, Transport},
};

#[test]
fn source_core_source_roundtrip_retains_dynamic_name_order_absence_null_and_issues() {
    for issues in [
        None,
        Some(Js::Null),
        Some(Js::Undefined),
        Some(Js::Arr(vec![obj(&[("code", "fictional".into())])])),
    ] {
        let mut error = Error::new(&format!("{}HttpFailure", "Fictional"), "source_http_failed");
        error.properties = obj(&[
            ("z", "first".into()),
            ("httpStatus", Js::Num(503.0)),
            ("cause", Js::Null),
            ("sourceOperation", "login_document".into()),
            ("surrogate", Js::Str(vec![0xd800])),
            ("absentValue", Js::Undefined),
        ]);
        if let Some(issues) = issues {
            error.properties.set("issues", issues);
        }
        let original = error.clone();
        let fault = fault_from_error(error);
        assert_eq!(fault.name, original.name);
        assert_eq!(fault.message, original.message);
        assert_eq!(fault.properties, original.properties);
        assert!(fault.trusted_context.is_none());
        assert_eq!(fault.value(), original.value());
        let back = error_from_fault(fault);
        assert_eq!(back.name, original.name);
        assert_eq!(back.message, original.message);
        assert_eq!(back.properties, original.properties);
        assert_eq!(back.value(), original.value());
    }
    let issues = vec![obj(&[("code", "legacy_issue".into())])];
    let error = error_from_fault(Fault {
        issues: Some(issues.clone()),
        ..Fault::type_error("old")
    });
    assert_eq!(error.properties, obj(&[("issues", Js::Arr(issues))]));
    assert_eq!(error_from_fault(Fault::error("old")).properties, obj(&[]));
    // Generic parts conversion never upgrades a name/code/marker into class authority.
    let mut forged = Error::new("AcademyReadFailure", "source_authentication_failed");
    forged.properties = obj(&[
        ("code", "source_authentication_failed".into()),
        ("cause", Error::error("source_http_failed").value()),
        ("trusted_context", true.into()),
    ]);
    let fault = fault_from_error(forged);
    assert!(fault.trusted_context.is_none());
    assert!(!read_failure::public_read_failure(&fault, "fallback").has("cause"));
    let trusted = Fault::trusted_authentication(Some(Fault::error("source_http_failed")));
    let semantic_parts = trusted.properties.clone();
    let reimported = fault_from_error(error_from_fault(trusted));
    assert_eq!(reimported.properties, semantic_parts);
    assert!(!read_failure::public_read_failure(&reimported, "fallback").has("cause"));
    assert!(
        reimported.trusted_context.is_none(),
        "opaque native authority is not serialized"
    );
}
struct Fictional {
    requests: Vec<Request>,
}
impl Transport for Fictional {
    fn dispatch(&mut self, request: &Request) -> Result<Response> {
        assert_eq!(request.method, "POST");
        assert!(request.body.contains(&("p_process".into(), "Main".into())));
        self.requests.push(request.clone());
        assert_eq!(self.requests.len(), 1, "only the before-read request");
        Ok(Response {
            status: 503,
            headers: vec![],
            bytes: vec![],
            url: format!("{ORIGIN}{}", request.path),
            stalled: false,
        })
    }
}
struct RefuseApproval;
impl Approval for RefuseApproval {
    fn consume(&mut self, _: &Js) -> CoreResult<Js> {
        panic!("before-read failure must precede approval")
    }
}
#[test]
fn actual_client_before_read_error_retains_typed_properties_through_core_roundtrip() {
    let mut client = Client::new(
        obj(&[
            ("teacherLabel", "Fictional Teacher".into()),
            ("teacherKey", "99001".into()),
            ("collegeKey", "88001".into()),
        ]),
        Fictional { requests: vec![] },
    )
    .unwrap();
    client.authenticated = true; // Synthetic state, not an authentication claim.
    let target = obj(&[
        ("studentName", "Fictional Learner".into()),
        ("date", "2040-01-04".into()),
        ("groupId", "3".into()),
        ("teacherKey", "99001".into()),
        ("studentKey", "77001".into()),
        ("courseKey", "5".into()),
        ("recordKey", "13".into()),
        ("membershipKey", "8".into()),
    ]);
    let action = obj(&[
        ("attemptId", "fictional-error-parts".into()),
        ("target", target),
        ("field", "progress".into()),
        ("effectRecordKey", "13".into()),
        ("before", "Before".into()),
        ("after", "After".into()),
        (
            "wireDigest",
            "650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1".into(),
        ),
    ]);
    let error = client
        .apply_journal_action(&action, &mut RefuseApproval, "2040-01-04T09:00:00.000Z")
        .unwrap_err();
    assert_eq!(error.name, "AcademyHttpFailure");
    assert_eq!(error.message, "source_http_failed");
    assert_eq!(
        error.properties,
        obj(&[
            ("sourceOperation", "Main".into()),
            ("httpStatus", Js::Num(503.0))
        ])
    );
    let fault = fault_from_error(error.clone());
    let failure = read_failure::public_read_failure(&fault, "fallback");
    assert_eq!(failure.get("sourceHttpStatus"), &Js::Num(503.0));
    assert!(
        !failure.has("sourceOperation"),
        "Main is deliberately outside the original allowlist"
    );
    assert_eq!(error_from_fault(fault).properties, error.properties);
    assert_eq!(client.transport.requests.len(), 1);
}
