//! NEW checks, authored only. Actual Client uses finite in-memory HTTP documents.
//! Pure resolver/cache/observation tests below explicitly use a fictional SourcePort.
use edu_academy_core::{
    js::{Fault, Result as CoreResult, TrustedContext},
    workflow::{
        Runtime,
        approval::Approval,
        native::{NativeBackend, SourcePort as WritePort},
        review::Policy,
        service::{Service, Source},
        store::Store,
    },
};
use edu_academy_sources::{
    data::*,
    http::{Client, MAIN, ORIGIN, Request, Response, Transport},
    native::{Backend, SourcePort, StageError},
    owner_workflow::{Credentials, Owner},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};
const NOW: &str = "2040-01-04T09:00:00.000Z";
const WIRE: &str = "650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1";
struct Clock;
impl Runtime for Clock {
    fn now(&self) -> String {
        NOW.into()
    }
    fn uuid(&self) -> CoreResult<String> {
        Ok("fictional-owner-uuid".into())
    }
    fn epoch_ms(&self) -> u64 {
        1000
    }
}
fn admission() -> Js {
    obj(&[
        ("teacherId", "99001".into()),
        ("teacherLabel", "Fictional Teacher".into()),
        ("date", "2040-01-04".into()),
        ("sourceBundleSha256", "b".repeat(64).into()),
        (
            "students",
            Js::Arr(vec![obj(&[
                ("studentId", "77001".into()),
                ("displayName", "Fictional Learner".into()),
            ])]),
        ),
    ])
}
fn owner_config() -> Js {
    obj(&[
        ("schemaVersion", "academy.owner-admission/2".into()),
        ("decider", "user".into()),
        ("decidedAt", "2040-01-01".into()),
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("failClosedDefault", "2026-10-23".into()),
        ("allowedFrom", Js::Null),
        ("allowedThrough", Js::Null),
        ("studentScope", "source_verified".into()),
    ])
}
fn scope(id: &str, date: &str) -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("studentId", id.into()),
        ("lessonDate", date.into()),
    ])
}
fn target() -> Js {
    obj(&[
        ("studentName", "Fictional Learner".into()),
        ("date", "2040-01-04".into()),
        ("groupId", "3".into()),
        ("teacherKey", "99001".into()),
        ("studentKey", "77001".into()),
        ("courseKey", "5".into()),
        ("recordKey", "13".into()),
        ("membershipKey", "8".into()),
    ])
}
fn action() -> Js {
    obj(&[
        ("attemptId", "fictional-attempt".into()),
        ("target", target()),
        ("field", "progress".into()),
        ("effectRecordKey", "13".into()),
        ("before", "Before".into()),
        ("after", "After".into()),
        ("wireDigest", WIRE.into()),
    ])
}
struct NoApproval {
    calls: usize,
}
impl Approval for NoApproval {
    fn consume(&mut self, _: &Js) -> CoreResult<Js> {
        self.calls += 1;
        Err(Fault::error("fictional_approval_must_not_be_reached"))
    }
}
struct FiniteHttp {
    replies: VecDeque<(String, u16, String)>,
    seen: Rc<RefCell<Vec<Request>>>,
}
impl Transport for FiniteHttp {
    fn dispatch(&mut self, input: &Request) -> Result<Response> {
        let (path, status, body) = self
            .replies
            .pop_front()
            .expect("finite fictional request budget");
        assert_eq!(input.path, path);
        self.seen.try_borrow_mut().unwrap().push(input.clone());
        Ok(Response {
            status,
            headers: vec![],
            bytes: body.into_bytes(),
            url: format!("{ORIGIN}{path}"),
            stalled: false,
        })
    }
}
fn login() -> Vec<(String, u16, String)> {
    let fields = [
        "p_process",
        "p_prino",
        "p_groupno",
        "userpwd",
        "userid",
        "cf_pc",
        "cf_user",
        "autoLoginCheck",
        "autoLoginKey",
    ]
    .iter()
    .map(|n| format!("<input name=\"{n}\">"))
    .collect::<String>();
    vec![
        ("/c_login.jsp".into(), 200, format!(r#"<form id="loginForm" method="post">{fields}</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>"#)),
        ("/servlet/controller.login.LoginServlet".into(), 200, "fictional accepted login".into()),
        ("/servlet/controller.coursemanage.CourseManageServlet".into(), 200,
            r#"<form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="fictional"></form><a>로그아웃</a>"#.into()),
    ]
}
fn client(replies: Vec<(String, u16, String)>) -> (Client<FiniteHttp>, Rc<RefCell<Vec<Request>>>) {
    let seen = Rc::new(RefCell::new(vec![]));
    let client = Client::new(
        obj(&[
            ("teacherLabel", "Fictional Teacher".into()),
            ("teacherKey", "99001".into()),
            ("collegeKey", "88001".into()),
        ]),
        FiniteHttp {
            replies: replies.into(),
            seen: Rc::clone(&seen),
        },
    )
    .unwrap();
    (client, seen)
}
fn credentials() -> Result<Credentials> {
    Ok(Credentials::new(
        "fictional-user".into(),
        "fictional-password".into(),
    ))
}
fn authentication_cause(f: &Fault) -> &Fault {
    assert_eq!(f.message, "source_authentication_failed");
    match &f.trusted_context {
        Some(TrustedContext::Authentication { cause: Some(cause) }) => cause,
        _ => panic!("known authentication stage must retain typed raw cause"),
    }
}

#[test]
fn owner_actual_client_login_http_failure_retained_read_write_read_and_search() {
    let (client, seen) = client(vec![("/c_login.jsp".into(), 503, "".into())]);
    let calls = Rc::new(Cell::new(0));
    let count = Rc::clone(&calls);
    let owner = Owner::new(
        client,
        admission(),
        None,
        vec![],
        move || {
            count.set(count.get() + 1);
            credentials()
        },
        &Clock,
    )
    .unwrap();
    let mut source = owner.source();
    let mut writer = owner.writer();
    let mut approval = NoApproval { calls: 0 };
    let first = Source::read(
        &mut source,
        &scope("77001", "2040-01-04"),
        &"request-a".into(),
        &Js::Null,
    )
    .unwrap_err();
    let second = writer.apply(&action(), &mut approval).unwrap_err();
    let third = Source::read(
        &mut source,
        &scope("77001", "2040-01-04"),
        &"request-a".into(),
        &Js::Null,
    )
    .unwrap_err();
    let search = source
        .search_students_by_name(&"Fictional Learner".into(), Some(&"request-a".into()), None)
        .unwrap_err()
        .into_fault();
    for f in [&first, &second, &third, &search] {
        let cause = authentication_cause(f);
        assert_eq!(cause.name, "AcademyHttpFailure");
        assert_eq!(cause.message, "source_http_failed");
        assert_eq!(cause.properties.get("httpStatus"), &Js::Num(503.));
        assert!(cause.properties.get("sourceOperation").is("login_document"));
        assert_eq!(cause.value(), authentication_cause(&first).value());
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(seen.try_borrow().unwrap().len(), 1);
    assert_eq!(approval.calls, 0);
}

#[test]
fn owner_synchronous_credential_failure_retained_without_transport_or_approval() {
    let (client, seen) = client(vec![]);
    let calls = Rc::new(Cell::new(0));
    let count = Rc::clone(&calls);
    let owner = Owner::new(
        client,
        admission(),
        None,
        vec![],
        move || {
            count.set(count.get() + 1);
            let mut e = Error::new("FictionalCredentialFailure", "fictional_unavailable");
            e.properties = obj(&[
                ("code", "fictional_provider_failure".into()),
                ("httpStatus", Js::Null),
            ]);
            Err(e)
        },
        &Clock,
    )
    .unwrap();
    let mut source = owner.source();
    let mut writer = owner.writer();
    let mut approval = NoApproval { calls: 0 };
    let errors = [
        Source::read(
            &mut source,
            &scope("77001", "2040-01-04"),
            &"req".into(),
            &Js::Null,
        )
        .unwrap_err(),
        writer.apply(&action(), &mut approval).unwrap_err(),
        Source::read(
            &mut source,
            &scope("77001", "2040-01-04"),
            &"req".into(),
            &Js::Null,
        )
        .unwrap_err(),
    ];
    for error in errors {
        let cause = authentication_cause(&error);
        assert_eq!(cause.name, "FictionalCredentialFailure");
        assert_eq!(cause.properties.get("httpStatus"), &Js::Null);
    }
    assert_eq!(calls.get(), 1);
    assert!(seen.try_borrow().unwrap().is_empty());
    assert_eq!(approval.calls, 0);
}

#[test]
fn owner_successful_three_step_login_is_shared_across_source_and_write_views() {
    let mut replies = login();
    replies.extend((0..3).map(|_| (MAIN.into(), 503, "".into())));
    let (client, seen) = client(replies);
    let calls = Rc::new(Cell::new(0));
    let count = Rc::clone(&calls);
    let owner = Owner::new(
        client,
        admission(),
        None,
        vec![],
        move || {
            count.set(count.get() + 1);
            credentials()
        },
        &Clock,
    )
    .unwrap();
    let mut source = owner.source();
    let mut writer = owner.writer();
    let mut approval = NoApproval { calls: 0 };
    let first = Source::read(
        &mut source,
        &scope("77001", "2040-01-04"),
        &"req".into(),
        &Js::Null,
    )
    .unwrap_err();
    writer.authenticate().unwrap();
    let second = writer.apply(&action(), &mut approval).unwrap_err();
    let third = Source::read(
        &mut source,
        &scope("77001", "2040-01-04"),
        &"req".into(),
        &Js::Null,
    )
    .unwrap_err();
    for f in [first, second, third] {
        assert!(f.trusted_context.is_none());
        assert_eq!(f.name, "AcademyHttpFailure");
        assert_eq!(f.properties.get("httpStatus"), &Js::Num(503.));
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(approval.calls, 0);
    let requests = seen.try_borrow().unwrap();
    assert_eq!(requests.len(), 6);
    assert_eq!(
        requests.iter().filter(|r| r.path == "/c_login.jsp").count(),
        1
    );
}

#[test]
fn owner_scope_and_invalid_configuration_refuse_before_credentials_or_transport() {
    for mut bad in [
        scope("77001", "2040-01-05"),
        scope("77002", "2040-01-04"),
        scope("77001", "2040-01-04"),
    ] {
        if bad.get("studentId").is("77001") && bad.get("lessonDate").is("2040-01-04") {
            bad.set("teacherId", "99002".into());
        }
        let (client, seen) = client(vec![]);
        let calls = Rc::new(Cell::new(0));
        let count = Rc::clone(&calls);
        let owner = Owner::new(
            client,
            admission(),
            None,
            vec![],
            move || {
                count.set(count.get() + 1);
                credentials()
            },
            &Clock,
        )
        .unwrap();
        let mut source = owner.source();
        let mut writer = owner.writer();
        assert_eq!(
            Source::read(&mut source, &bad, &"req".into(), &Js::Null)
                .unwrap_err()
                .message,
            "native_dailyrecord_scope_refused"
        );
        let (admission, config, names) = owner.write_configuration().unwrap();
        let mut backend = NativeBackend::new(&mut writer, admission, config, names, &Clock);
        use edu_academy_core::workflow::service::Backend as _;
        let mut approval = NoApproval { calls: 0 };
        assert_eq!(
            backend
                .save(&bad, &Js::Arr(vec![]), &mut approval, &Js::Null)
                .unwrap_err()
                .message,
            "native_dailyrecord_scope_refused"
        );
        assert_eq!(calls.get(), 0);
        assert!(seen.try_borrow().unwrap().is_empty());
        assert_eq!(approval.calls, 0);
    }
    for (key, field) in [
        ("teacherId", "99002"),
        ("allowedFrom", "2041-01-01"),
        ("decider", "assistant"),
    ] {
        let mut bad = owner_config();
        bad.set(key, field.into());
        let (client, seen) = client(vec![]);
        let calls = Rc::new(Cell::new(0));
        let count = Rc::clone(&calls);
        assert!(
            Owner::new(
                client,
                admission(),
                Some(&bad),
                vec![],
                move || {
                    count.set(count.get() + 1);
                    credentials()
                },
                &Clock
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
        assert!(seen.try_borrow().unwrap().is_empty());
    }
}

#[test]
fn owner_actual_client_name_dependency_receives_name_only_then_reads_same_session() {
    let mut replies = login();
    replies.push(("/servlet/controller.dailyzerotest.DailyZeroTestServlet".into(), 200,
        r#"<script>var fictional = { testResultList: JSON.parse('[{"priNo":"77002","userName":"Fictional Learner"}]') };</script>"#.into()));
    replies.push((MAIN.into(), 503, "".into()));
    let (client, seen) = client(replies);
    let owner = Owner::new(
        client,
        admission(),
        Some(&owner_config()),
        vec!["Fictional Learner".into()],
        credentials,
        &Clock,
    )
    .unwrap();
    let mut source = owner.source();
    let mut writer = owner.writer();
    let pairs = source
        .search_students_by_name(
            &"Fictional Learner".into(),
            Some(&"request-source".into()),
            None,
        )
        .unwrap();
    assert_eq!(
        pairs,
        Js::Arr(vec![obj(&[
            ("studentKey", "77002".into()),
            ("studentName", "Fictional Learner".into())
        ])])
    );
    writer.authenticate().unwrap();
    // No configured 77002: this read must use the request-bound resolution.
    let error = Source::read(
        &mut source,
        &scope("77002", "2040-01-04"),
        &"request-source".into(),
        &Js::Null,
    )
    .unwrap_err();
    assert_eq!(error.name, "AcademyHttpFailure");
    assert!(error.trusted_context.is_none());
    let requests = seen.try_borrow().unwrap();
    assert_eq!(requests.len(), 5);
    let lookup = &requests[3];
    assert_eq!(lookup.method, "POST");
    assert_eq!(
        lookup.query,
        vec![("reqCmd".into(), "DtResultSearch".into())]
    );
    assert_eq!(
        lookup.body,
        vec![
            ("sort_date".into(), "1".into()),
            ("stu_name".into(), "Fictional Learner".into())
        ]
    );
}

#[test]
fn owner_reentrant_credential_callback_returns_native_borrow_error_before_nested_transport() {
    type Callback = Rc<RefCell<Option<Box<dyn FnMut()>>>>;
    let callback: Callback = Rc::new(RefCell::new(None));
    let call = Rc::clone(&callback);
    let nested_error = Rc::new(RefCell::new(None));
    let result = Rc::clone(&nested_error);
    let (client, seen) = client(vec![("/c_login.jsp".into(), 503, "".into())]);
    let owner = Owner::new(
        client,
        admission(),
        None,
        vec![],
        move || {
            if let Some(f) = call.try_borrow_mut().unwrap().as_mut() {
                f();
            }
            credentials()
        },
        &Clock,
    )
    .unwrap();
    let mut nested = owner.writer();
    *callback.try_borrow_mut().unwrap() = Some(Box::new(move || {
        *result.try_borrow_mut().unwrap() = Some(nested.authenticate().unwrap_err());
    }));
    let mut source = owner.source();
    let outer = Source::read(
        &mut source,
        &scope("77001", "2040-01-04"),
        &"req".into(),
        &Js::Null,
    )
    .unwrap_err();
    assert_eq!(authentication_cause(&outer).name, "AcademyHttpFailure");
    let native = RefCell::new(());
    let _held = native.try_borrow_mut().unwrap();
    let expected = native.try_borrow_mut().unwrap_err().to_string();
    let error = nested_error.try_borrow().unwrap();
    let error = error.as_ref().unwrap();
    assert_eq!(error.message, expected);
    assert!(error.trusted_context.is_none());
    assert_eq!(seen.try_borrow().unwrap().len(), 1);
    // Break the fixture-only callback cycle; no persistent owner registration exists.
    *callback.try_borrow_mut().unwrap() = None;
}

// This function is type checked with the new target but never performs filesystem work.
// It constructs the real Service + existing NativeBackend, and the real store/runtime approval.
#[allow(dead_code)]
fn real_service_interface_fit(store: &Store, policy: &dyn Policy, runtime: &dyn Runtime) {
    let (client, _) = client(vec![]);
    let owner = Owner::new(
        client,
        admission(),
        Some(&owner_config()),
        vec!["Fictional Learner".into()],
        credentials,
        runtime,
    )
    .unwrap();
    let (admission, config, names) = owner.write_configuration().unwrap();
    let mut source = owner.source();
    let mut writer = owner.writer();
    let mut backend = NativeBackend::new(&mut writer, admission, config, names, runtime);
    let _service = Service {
        store,
        source: &mut source,
        backend: &mut backend,
        policy,
        runtime,
    };
    let _approval = edu_academy_core::workflow::approval::RequestApproval {
        store,
        input: obj(&[]),
        runtime,
    };
}

// Honest algorithm seam: these cases exercise native scope/resolver/cache/projection,
// not Client authentication, HTTP qualification, native owner activation or writes.
struct FictionalSource {
    auth: usize,
    discovery: Vec<Js>,
    execution: Vec<Js>,
    searches: Vec<Js>,
    pairs: Js,
    dated_rows: Option<Js>,
    fail_discovery: bool,
    name_capability: bool,
    conflict: Option<&'static str>,
}
impl FictionalSource {
    fn new() -> Self {
        Self {
            auth: 0,
            discovery: vec![],
            execution: vec![],
            searches: vec![],
            pairs: Js::Arr(vec![obj(&[
                ("studentKey", "77002".into()),
                ("studentName", "Fictional Learner".into()),
            ])]),
            dated_rows: None,
            fail_discovery: true,
            name_capability: true,
            conflict: None,
        }
    }
}
fn row(id: &str, date: &str, name: &str) -> Js {
    let mut row = target();
    row.set("studentKey", id.into());
    row.set("date", date.into());
    row.set("studentName", name.into());
    if date == "2040-01-02" {
        row.set("recordKey", "12".into());
    }
    row.set(
        "fields",
        obj(&[
            ("progress", "Prior progress".into()),
            ("homework", "Prior homework".into()),
            ("lessonMemo", "Prior memo".into()),
        ]),
    );
    row
}
impl SourcePort for FictionalSource {
    fn authenticate(&mut self) -> Result<Js> {
        self.auth += 1;
        Ok(Js::Null)
    }
    fn discover(&mut self, input: &Js) -> Result<Js> {
        self.discovery.push(input.clone());
        if self.fail_discovery {
            return Err(Error::error("fictional_after_scope_preflight"));
        }
        Ok(obj(&[(
            "groups",
            Js::Arr(vec![obj(&[("groupId", "3".into())])]),
        )]))
    }
    fn execute(&mut self, input: &Js) -> Result<Js> {
        self.execution.push(input.clone());
        if let Some(rows) = &self.dated_rows {
            return Ok(obj(&[("dayRecords", rows.clone())]));
        }
        let date = string(input.get("date"));
        let mut found = row(
            "77002",
            &date,
            &string(&input.get("studentNames").array()[0]),
        );
        if self.conflict == Some("identity") {
            found.set("studentKey", "77003".into());
        }
        if self.conflict == Some("previous") && date == "2040-01-02" {
            found.set("membershipKey", "9".into());
        }
        Ok(obj(&[("dayRecords", Js::Arr(vec![found]))]))
    }
    fn journal_state(&mut self, target: &Js) -> Result<Js> {
        let mut bound = target.clone();
        if self.conflict == Some("state") {
            bound.set("recordKey", "99".into());
        }
        Ok(obj(&[
            ("target", bound),
            ("wireDigest", WIRE.into()),
            ("previousDate", "2040-01-02".into()),
            ("previousRecordKey", "12".into()),
            (
                "fields",
                obj(&[
                    ("progress", "Current progress".into()),
                    ("homework", "Current homework".into()),
                    ("lessonMemo", "Current memo".into()),
                    ("attendance", "Y".into()),
                    ("dailyTestAssessment", Js::Null),
                    ("previousHomeworkAssessment", "3".into()),
                ]),
            ),
        ]))
    }
    fn has_name_search(&self) -> bool {
        self.name_capability
    }
    fn search_students_by_name(&mut self, input: &Js) -> Option<Result<Js>> {
        assert!(self.name_capability);
        self.searches.push(input.clone());
        Some(Ok(self.pairs.clone()))
    }
}
fn algorithm() -> Backend<FictionalSource> {
    Backend::with_owner(
        FictionalSource::new(),
        admission(),
        Some(&owner_config()),
        vec!["Fictional Learner".into()],
    )
    .unwrap()
}
fn scope_read(
    b: &mut Backend<FictionalSource>,
    id: &str,
    date: &str,
    request: &str,
    subject: Option<&Js>,
    now_ms: u64,
) -> std::result::Result<Js, StageError> {
    b.read_owner(
        &scope(id, date),
        Some(&request.into()),
        subject,
        now_ms,
        NOW,
        "fictional-observation",
    )
}
fn read_error(
    b: &mut Backend<FictionalSource>,
    id: &str,
    date: &str,
    request: &str,
    now_ms: u64,
) -> String {
    scope_read(b, id, date, request, None, now_ms)
        .unwrap_err()
        .into_error()
        .message
}
fn search(
    b: &mut Backend<FictionalSource>,
    request: &str,
    date: Option<&str>,
    at: u64,
) -> std::result::Result<Js, StageError> {
    b.search_students_by_name(
        &"Fictional Learner".into(),
        Some(&request.into()),
        date.map(Js::from).as_ref(),
        at,
    )
}

#[test]
fn owner_algorithm_cache_age_inclusive_negative_stale_date_id_and_request_binding() {
    let mut backend = algorithm();
    search(&mut backend, "req", None, 1000).unwrap();
    assert_eq!(backend.port.auth, 1);
    for at in [1000, 301000] {
        assert_eq!(
            read_error(&mut backend, "77002", "2040-01-04", "req", at),
            "fictional_after_scope_preflight"
        );
    }
    for (id, date, request, at) in [
        ("77002", "2040-01-04", "req", 999),
        ("77002", "2040-01-04", "req", 301001),
        ("77003", "2040-01-04", "req", 1001),
        ("77002", "2040-01-04", "other", 1001),
    ] {
        assert_eq!(
            read_error(&mut backend, id, date, request, at),
            "native_dailyrecord_scope_refused"
        );
    }
    // Dated source selection, not the HTTP name search, binds its cache to one date.
    backend.port.fail_discovery = false;
    backend.port.dated_rows = Some(Js::Arr(vec![row(
        "77002",
        "2040-01-04",
        "Fictional Learner",
    )]));
    search(&mut backend, "dated", Some("2040-01-04"), 2000).unwrap();
    backend.port.fail_discovery = true;
    assert_eq!(
        read_error(&mut backend, "77002", "2040-01-04", "dated", 2001),
        "fictional_after_scope_preflight"
    );
    assert_eq!(
        read_error(&mut backend, "77002", "2040-01-05", "dated", 2001),
        "native_dailyrecord_scope_refused"
    );
    assert_eq!(backend.port.searches.len(), 1);
    assert_eq!(
        backend.port.execution.last().unwrap(),
        &obj(&[
            ("family", "day_record".into()),
            ("teacherLabel", "Fictional Teacher".into()),
            ("date", "2040-01-04".into()),
            ("groupIds", Js::Arr(vec!["3".into()])),
            ("studentNames", Js::Arr(vec!["Fictional Learner".into()]))
        ])
    );
}

#[test]
fn owner_algorithm_cache_33_entry_eviction_and_request_replacement_before_failure() {
    let mut backend = algorithm();
    for i in 0..32 {
        search(&mut backend, &format!("request-{i}"), None, 1000).unwrap();
    }
    // Replacing request-0 deletes then reinserts; request-1 becomes the oldest.
    search(&mut backend, "request-0", None, 1001).unwrap();
    search(&mut backend, "request-32", None, 1002).unwrap();
    assert_eq!(
        read_error(&mut backend, "77002", "2040-01-04", "request-1", 1003),
        "native_dailyrecord_scope_refused"
    );
    for r in ["request-0", "request-2", "request-32"] {
        assert_eq!(
            read_error(&mut backend, "77002", "2040-01-04", r, 1003),
            "fictional_after_scope_preflight"
        );
    }
    let failure = backend
        .search_students_by_name(
            &"Unmanaged Fictional".into(),
            Some(&"request-0".into()),
            None,
            1004,
        )
        .unwrap_err();
    assert!(matches!(failure, StageError::StudentNameNotAdmitted));
    assert_eq!(
        read_error(&mut backend, "77002", "2040-01-04", "request-0", 1005),
        "native_dailyrecord_scope_refused"
    );
    assert_eq!(backend.port.auth, 1);
    assert_eq!(backend.port.searches.len(), 34);
}

#[test]
fn owner_algorithm_name_pairs_preserve_order_unique_key_cache_and_fail_closed_ambiguity() {
    let mut backend = algorithm();
    let pair =
        |id: &str, name: &str| obj(&[("studentKey", id.into()), ("studentName", name.into())]);
    for (rows, cached) in [
        (
            vec![
                pair("77002", "Fictional Learner"),
                pair("77002", "Fictional Learner"),
            ],
            true,
        ),
        (
            vec![
                pair("77003", "Fictional Learner"),
                pair("77002", "Fictional Learner"),
            ],
            false,
        ),
        (
            vec![
                pair("77002", "Fictional Learner"),
                pair("77002", "Foreign Fictional"),
            ],
            false,
        ),
        (vec![], false),
    ] {
        backend.port.pairs = Js::Arr(rows.clone());
        assert_eq!(
            search(&mut backend, "req", None, 1000).unwrap(),
            Js::Arr(rows)
        );
        assert_eq!(
            read_error(&mut backend, "77002", "2040-01-04", "req", 1001),
            if cached {
                "fictional_after_scope_preflight"
            } else {
                "native_dailyrecord_scope_refused"
            }
        );
    }
    backend.port.pairs = Js::Arr(vec![pair("77002", "Fictional Learner"); 200]);
    assert_eq!(
        search(&mut backend, "req", None, 1000)
            .unwrap()
            .array()
            .len(),
        200
    );
    backend.port.pairs = Js::Arr(vec![pair("77002", "Fictional Learner"); 201]);
    assert_eq!(
        search(&mut backend, "req", None, 1001)
            .unwrap_err()
            .into_error()
            .name,
        "ZodError"
    );
    assert_eq!(
        read_error(&mut backend, "77002", "2040-01-04", "req", 1002),
        "native_dailyrecord_scope_refused"
    );
    for rows in [
        Js::Null,
        Js::Arr(vec![Js::Null]),
        Js::Arr(vec![pair("0", "Fictional Learner")]),
        Js::Arr(vec![obj(&[("studentKey", "77002".into())])]),
    ] {
        backend.port.pairs = rows;
        assert!(search(&mut backend, "req", None, 1003).is_err());
    }
    for input in &backend.port.searches {
        assert_eq!(
            input,
            &obj(&[
                ("teacherLabel", "Fictional Teacher".into()),
                ("studentName", "Fictional Learner".into())
            ])
        );
    }
}

#[test]
fn owner_algorithm_dated_name_ambiguity_identity_and_date_refusals_precede_cache_use() {
    let mut backend = algorithm();
    backend.port.fail_discovery = false;
    for (rows, expected) in [
        (
            Js::Arr(vec![row("77002", "2040-01-04", "Fictional Learner"); 2]),
            "native_dated_name_ambiguous",
        ),
        (Js::Null, "native_dated_name_scope_conflict"),
        (
            Js::Arr(vec![row("77002", "2040-01-05", "Fictional Learner")]),
            "native_dated_name_scope_conflict",
        ),
        (
            Js::Arr(vec![row("77002", "2040-01-04", "Foreign Fictional")]),
            "native_dated_name_scope_conflict",
        ),
    ] {
        backend.port.dated_rows = Some(rows);
        assert_eq!(
            search(&mut backend, "req", Some("2040-01-04"), 1000)
                .unwrap_err()
                .into_error()
                .message,
            expected
        );
        assert_eq!(
            read_error(&mut backend, "77002", "2040-01-04", "req", 1001),
            "native_dailyrecord_scope_refused"
        );
    }
    let mut wrong_teacher = row("77002", "2040-01-04", "Fictional Learner");
    wrong_teacher.set("teacherKey", "99002".into());
    backend.port.dated_rows = Some(Js::Arr(vec![wrong_teacher]));
    assert_eq!(
        search(&mut backend, "req", Some("2040-01-04"), 1000)
            .unwrap_err()
            .into_error()
            .message,
        "native_dated_name_scope_conflict"
    );
    let mut bounded = owner_config();
    bounded.set("allowedThrough", "2040-01-04".into());
    let mut backend = Backend::with_owner(
        FictionalSource::new(),
        admission(),
        Some(&bounded),
        vec!["Fictional Learner".into()],
    )
    .unwrap();
    assert_eq!(
        search(&mut backend, "req", Some("2040-01-05"), 1000)
            .unwrap_err()
            .into_error()
            .message,
        "native_dailyrecord_scope_refused"
    );
    assert_eq!(backend.port.auth, 0);
    assert!(backend.port.discovery.is_empty());
}

#[test]
fn owner_algorithm_absent_name_capability_uses_admission_only_and_typed_not_admitted() {
    let mut port = FictionalSource::new();
    port.name_capability = false;
    let mut backend = Backend::new(port, admission());
    assert_eq!(
        search(&mut backend, "req", None, 0).unwrap(),
        Js::Arr(vec![obj(&[
            ("studentKey", "77001".into()),
            ("studentName", "Fictional Learner".into())
        ])])
    );
    assert!(matches!(
        backend.search_students_by_name(&"Other Fictional".into(), None, None, 0),
        Err(StageError::StudentNameNotAdmitted)
    ));
    assert_eq!(backend.port.auth, 0);
    assert!(backend.port.searches.is_empty());
    let forged = Error::new("StudentNameNotAdmitted", "source_authentication_failed");
    let fault = StageError::Source(forged).into_fault();
    assert!(fault.trusted_context.is_none());
    // Error parts/class-looking names never manufacture typed not-admitted or auth context.
    assert_eq!(fault.name, "StudentNameNotAdmitted");
}

#[test]
fn owner_algorithm_configured_cache_recorded_subject_precedence_and_observation_reuse() {
    let subject = obj(&[("displayName", "Fictional Learner".into())]);
    let mut backend = algorithm();
    assert_eq!(
        scope_read(
            &mut backend,
            "77002",
            "2040-01-04",
            "none",
            Some(&subject),
            1000
        )
        .unwrap_err()
        .into_error()
        .message,
        "fictional_after_scope_preflight"
    );
    let foreign = obj(&[("displayName", "Unmanaged Fictional".into())]);
    assert_eq!(
        scope_read(
            &mut backend,
            "77002",
            "2040-01-04",
            "none",
            Some(&foreign),
            1000
        )
        .unwrap_err()
        .into_error()
        .message,
        "native_dailyrecord_scope_refused"
    );
    backend.port.fail_discovery = false;
    let result = scope_read(
        &mut backend,
        "77002",
        "2040-01-04",
        "none",
        Some(&subject),
        1000,
    )
    .unwrap();
    let observation = result.get("observation");
    assert!(result.get("status").is("observed"));
    assert!(observation.get("target").get("studentId").is("77002"));
    assert!(observation.get("previousTarget").get("recordId").is("12"));
    assert!(
        observation
            .get("sourceRevision")
            .is(&hash(format!("{}:{WIRE}", "b".repeat(64)).as_bytes()))
    );
    assert!(
        observation
            .get("fields")
            .get("progress")
            .get("value")
            .is("Current progress")
    );
    assert!(
        observation
            .get("fields")
            .get("dailyTestAssessment")
            .get("state")
            .is("unknown")
    );
    assert!(
        observation
            .get("fields")
            .get("previousHomeworkAssessment")
            .get("evidenceRefs")
            .array()[0]
            .is("native-dayrecord-read:fictional-observation:previous")
    );
    assert!(
        observation.get("toneSamples").array()[0]
            .get("text")
            .is("Prior progress\nPrior homework\nPrior memo")
    );
    assert_eq!(
        backend.port.execution[1].get("date"),
        &Js::from("2040-01-02")
    );
    assert_eq!(
        backend.port.execution[1].get("groupIds"),
        &Js::Arr(vec!["3".into()])
    );
    for (conflict, error) in [
        ("identity", "native_dailyrecord_identity_conflict"),
        ("state", "native_dailyrecord_state_conflict"),
        ("previous", "native_dailyrecord_previous_conflict"),
    ] {
        backend.port.conflict = Some(conflict);
        assert_eq!(
            scope_read(
                &mut backend,
                "77002",
                "2040-01-04",
                "none",
                Some(&subject),
                1000
            )
            .unwrap_err()
            .into_error()
            .message,
            error
        );
    }
    // Configured identity takes precedence over a recorded subject; the observed request shows it.
    backend.port.conflict = None;
    let configured = scope_read(
        &mut backend,
        "77001",
        "2040-01-04",
        "none",
        Some(&foreign),
        1000,
    )
    .unwrap_err()
    .into_error();
    assert_eq!(configured.message, "native_dailyrecord_identity_conflict");
    assert_eq!(
        backend.port.execution.last().unwrap().get("studentNames"),
        &Js::Arr(vec!["Fictional Learner".into()])
    );
}

// Reuse the existing fictional DOM shape (helpers only), with this owner identity.
// Its inert functions intentionally cannot satisfy the immutable 6508 preimage.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn owner_unqualified_main(fields: &Js) -> String {
    // Own synthetic document, with exact source-observed signatures/column roles.
    // Inert definitions intentionally do not hash to the qualified private source.
    let mut html = format!(
        r#"
<input name="std_date" value="2040-01-04"><input name="teacher_pri_no" value="99001">
<select name="grp_seq"><option value="3" selected>Fictional group</option></select>
<table><thead><tr><th rowspan="2">이름</th><th rowspan="2">과정</th><th rowspan="2">지속사항</th><th colspan="4">지난진도</th><th colspan="6">일괄 발송 금일진도</th></tr>
<tr><th>지난수업일</th><th>진도</th><th>숙제</th><th>메모</th><th></th><th>금일출석</th><th>일일테스트</th><th>진도</th><th>숙제</th><th>메모</th></tr></thead>
<tbody><tr><td><a onclick="onOpenDaysMain('77001')">Fictional Learner</a></td><td>Fictional course</td>
<td><textarea onfocusout="onFoStuMemo('0','77001','5')">{}</textarea></td>
<td>2040-01-02</td><td>prior progress</td><td><a onclick="openHomeWorkRate(12,8,3)">prior assessment</a></td><td>prior memo</td><td></td>
<td><select onchange="onAttn('0','5','77001','13','8')"><option value="{}" selected>Fictional attendance</option></select></td>
<td><a onclick="setDailyTest(13,8,0)">DT</a><input name="h_daily_test" value="{}"><input name="h_homework_rate" value="{}"></td>
<td><textarea onfocusout="onFoPrg('0','5','77001','13','8')">{}</textarea></td>
<td><textarea onfocusout="onFoHw('0','5','77001','13','8')">{}</textarea></td>
<td><input onfocusout="onFoMm('0','5','77001','13','8')" value="{}"></td></tr></tbody></table>
<script>
function onAttn(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtAttn'; }}
function onFoPrg(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtPrg'; }}
function onFoHw(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtHw'; }}
function onFoMm(i,course_seq,stu_pri_no,record_seq,cm_seq){{ return 'reqCmd=udtMemo'; }}
function onFoStuMemo(i,stu_pri_no,course_seq){{ return 'inert'; }}
function onOpenDaysMain(student_pri_no){{ return 'inert'; }}
function onDailyReport(report_seq,stu_pri_no,record_seq,cm_seq){{ return 'inert'; }}
function setDailyTest(record_seq,cm_seq,count){{ return 'reqCmd=SetDailyTest'; }}
function openHomeWorkRate(record_seq,cm_seq,count){{ return 'reqCmd=SetHomeWorkRate'; }}
function reallength(value){{ return value.length; }}

var g_param = "";
var daily_test_radio = 'inert';
</script>
"#,
        escape(&string(fields.get("persistentMemo"))),
        escape(&string(fields.get("attendance"))),
        escape(&string(fields.get("dailyTestAssessment"))),
        escape(&string(fields.get("previousHomeworkAssessment"))),
        escape(&string(fields.get("progress"))),
        escape(&string(fields.get("homework"))),
        escape(&string(fields.get("lessonMemo")))
    );
    let homework = [
        "미확인",
        "미실시",
        "숙제 수행도 C",
        "숙제 수행도 B",
        "숙제 수행도 B⁺",
        "숙제 수행도 A",
        "숙제 수행도 A⁺",
    ];
    let daily = [
        "미확인",
        "미실시",
        "일일 테스트 10점",
        "일일 테스트 20점",
        "일일 테스트 30점",
        "일일 테스트 40점",
        "일일 테스트 50점",
        "일일 테스트 60점",
        "일일 테스트 70점",
        "일일 테스트 80점",
        "일일 테스트 90점",
        "일일 테스트 100점",
    ];
    for (name, labels) in [
        ("homework_radio", homework.as_slice()),
        ("daily_test_radio", daily.as_slice()),
    ] {
        // The original collector walks DOM order. JS Object.entries enumerates
        // nonnegative integer keys before the string key '-1'.
        for i in (1..labels.len()).chain(std::iter::once(0)) {
            html.push_str(&format!(
                "<table><tr><td><input name=\"{name}\" value=\"{}\">{}</td></tr></table>",
                i as i32 - 1,
                labels[i]
            ));
        }
    }
    html
}

#[test]
fn owner_actual_client_authenticated_apply_preserves_6508_refusal_and_no_approval() {
    let html = owner_unqualified_main(&obj(&[
        ("persistentMemo", "Fictional memo".into()),
        ("attendance", "Y".into()),
        ("dailyTestAssessment", "0".into()),
        ("previousHomeworkAssessment", "3".into()),
        ("progress", "Before".into()),
        ("homework", "Homework".into()),
        ("lessonMemo", "Memo".into()),
    ]));
    let mut replies = login();
    // journal_state executes the one target and then independently inspects its wire.
    replies.push((MAIN.into(), 200, html.clone()));
    replies.push((MAIN.into(), 200, html));
    let (client, seen) = client(replies);
    let owner = Owner::new(client, admission(), None, vec![], credentials, &Clock).unwrap();
    let mut writer = owner.writer();
    let mut approval = NoApproval { calls: 0 };
    let error = writer.apply(&action(), &mut approval).unwrap_err();
    assert_eq!(error.message, "journal_mutation_wire_not_qualified");
    assert!(error.trusted_context.is_none());
    assert_eq!(approval.calls, 0);
    let requests = seen.try_borrow().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(requests.iter().skip(3).all(|r| r.method == "POST"
        && r.path == MAIN
        && r.body.first() == Some(&("p_process".into(), "Main".into()))));
    // No mutation or fabricated approval-success path is supplied by this fixture.
}

#[test]
fn owner_algorithm_v1_and_v2_configured_read_policy_does_not_widen_student_or_date_scope() {
    for version in ["academy.owner-admission/1", "academy.owner-admission/2"] {
        let mut config = owner_config();
        config.set("schemaVersion", version.into());
        config.set("allowedFrom", "2040-01-01".into());
        config.set("allowedThrough", "2040-01-10".into());
        if let Js::Obj(p) = &mut config {
            p.retain(|(k, _)| k != &text("studentScope"));
        }
        let mut backend = Backend::with_owner(
            FictionalSource::new(),
            admission(),
            Some(&config),
            vec!["Fictional Learner".into()],
        )
        .unwrap();
        assert_eq!(
            read_error(&mut backend, "77001", "2040-01-05", "req", 1000),
            "fictional_after_scope_preflight"
        );
        let before = backend.port.discovery.len();
        for (id, date) in [("77001", "2040-01-11"), ("77002", "2040-01-05")] {
            assert_eq!(
                scope_read(
                    &mut backend,
                    id,
                    date,
                    "req",
                    Some(&obj(&[("displayName", "Fictional Learner".into())])),
                    1000
                )
                .unwrap_err()
                .into_error()
                .message,
                "native_dailyrecord_scope_refused"
            );
        }
        assert_eq!(backend.port.discovery.len(), before);
        // A name result in configured mode cannot grant a new student's read admission.
        search(&mut backend, "req", None, 1000).unwrap();
        assert_eq!(
            read_error(&mut backend, "77002", "2040-01-05", "req", 1001),
            "native_dailyrecord_scope_refused"
        );
    }
    let mut backend = Backend::new(FictionalSource::new(), admission());
    assert_eq!(
        read_error(&mut backend, "77001", "2040-01-04", "req", 0),
        "fictional_after_scope_preflight"
    );
    assert_eq!(
        read_error(&mut backend, "77001", "2026-10-23", "req", 0),
        "native_dailyrecord_scope_refused"
    );
}

#[test]
fn owner_algorithm_cache_precedes_managed_recorded_subject_and_configured_student_precedes_cache() {
    let config = owner_config();
    let mut backend = Backend::with_owner(
        FictionalSource::new(),
        admission(),
        Some(&config),
        vec!["Fictional Learner".into(), "Alternate Fictional".into()],
    )
    .unwrap();
    search(&mut backend, "req", None, 1000).unwrap();
    backend.port.fail_discovery = false;
    let subject = obj(&[("displayName", "Alternate Fictional".into())]);
    scope_read(
        &mut backend,
        "77002",
        "2040-01-04",
        "req",
        Some(&subject),
        1001,
    )
    .unwrap();
    assert_eq!(
        backend.port.execution[0].get("studentNames"),
        &Js::Arr(vec!["Fictional Learner".into()])
    );
    backend.port.pairs = Js::Arr(vec![obj(&[
        ("studentKey", "77001".into()),
        ("studentName", "Alternate Fictional".into()),
    ])]);
    backend
        .search_students_by_name(
            &"Alternate Fictional".into(),
            Some(&"configured-request".into()),
            None,
            1000,
        )
        .unwrap();
    assert_eq!(
        scope_read(
            &mut backend,
            "77001",
            "2040-01-04",
            "configured-request",
            Some(&subject),
            1001
        )
        .unwrap_err()
        .into_error()
        .message,
        "native_dailyrecord_identity_conflict"
    );
    assert_eq!(
        backend.port.execution.last().unwrap().get("studentNames"),
        &Js::Arr(vec!["Fictional Learner".into()])
    );
    // Both native row boundaries stay at max200, even beyond the resolver helper's 201.
    backend.port.pairs = Js::Arr(vec![
        obj(&[
            ("studentKey", "77002".into()),
            ("studentName", "Fictional Learner".into())
        ]);
        202
    ]);
    let error = search(&mut backend, "req", None, 1000)
        .unwrap_err()
        .into_error();
    let size = error.properties.get("issues").array().last().unwrap();
    assert_eq!(size.get("maximum"), &Js::Num(200.));
    assert!(
        size.get("message")
            .is("Too big: expected array to have <=200 items")
    );
}
