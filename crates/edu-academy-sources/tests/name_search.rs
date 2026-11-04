//! New actual-Client contract tests, authored only. Finite in-memory transport,
//! literal fictional credentials/principals; no socket or owner root. The existing
//! authenticate implementation reads OS entropy for its synthetic login encoding.
use edu_academy_sources::{
    data::*,
    http::{Client, ORIGIN, Request, Response, Transport},
};
use std::{collections::VecDeque, time::Duration};

const PATH: &str = "/servlet/controller.dailyzerotest.DailyZeroTestServlet";
const NAME: &str = "Fictional + Learner &😀";
const TEACHER: &str = "Fictional Name Teacher";
const COOKIE: &str = "JSESSIONID=fictional-name-session";

type Page = (u16, Vec<(String, String)>, String);

struct Fictional {
    responses: VecDeque<Page>,
    requests: Vec<Request>,
}
impl Transport for Fictional {
    fn dispatch(&mut self, request: &Request) -> Result<Response> {
        self.requests.push(request.clone());
        let (status, headers, body) = self
            .responses
            .pop_front()
            .expect("finite fictional response");
        Ok(Response {
            status,
            headers,
            bytes: body.into_bytes(),
            url: format!("{ORIGIN}{}", request.path),
            stalled: false,
        })
    }
}
fn input(name: Js) -> Js {
    obj(&[("teacherLabel", TEACHER.into()), ("studentName", name)])
}
fn page() -> String {
    format!(
        r#"testResultList:JSON.parse('[{{"userName":"{NAME}","priNo":"007"}},{{"userName":"Other Fictional Learner","priNo":null}},{{"userName":"{NAME}","priNo":7}}]')"#
    )
}
fn client(pages: Vec<Page>) -> Client<Fictional> {
    let login = r#"<form id="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form>
<script>document.loginForm.action = "/servlet/controller.login.LoginServlet";
document.loginForm.p_process.value = "c_login";</script>"#;
    let principal = r#"<form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="fictional"></form><a>로그아웃</a>"#;
    let mut responses = VecDeque::from([
        (
            200,
            vec![("Set-Cookie".into(), format!("{COOKIE}; Path=/; Secure"))],
            login.into(),
        ),
        (200, vec![], String::new()),
        (200, vec![], principal.into()),
    ]);
    responses.extend(pages);
    Client::new(
        obj(&[
            ("teacherLabel", TEACHER.into()),
            ("teacherKey", "99001".into()),
            ("collegeKey", "88001".into()),
        ]),
        Fictional {
            responses,
            requests: vec![],
        },
    )
    .unwrap()
}
fn ready(pages: Vec<Page>) -> Client<Fictional> {
    let mut client = client(pages);
    let result = client
        .authenticate("fictional-name-user", "fictional-name-password")
        .unwrap();
    assert_eq!(result.get("state"), &Js::from("authenticated"));
    assert!(client.authenticated);
    assert_eq!(client.transport.requests.len(), 3);
    let login = &client.transport.requests[0];
    assert_eq!(login.path, "/c_login.jsp");
    assert_eq!(login.method, "GET");
    assert!(login.query.is_empty() && login.body.is_empty());
    let submission = &client.transport.requests[1];
    assert_eq!(submission.path, "/servlet/controller.login.LoginServlet");
    assert_eq!(submission.method, "POST");
    assert_eq!(submission.body[0], ("p_process".into(), "c_login".into()));
    assert_eq!(submission.body.len(), 9);
    let principal = &client.transport.requests[2];
    assert_eq!(
        principal.path,
        "/servlet/controller.coursemanage.CourseManageServlet"
    );
    assert_eq!(principal.method, "GET");
    assert_eq!(
        principal.query,
        vec![("reqCmd".into(), "StudyCourse".into())]
    );
    assert!(principal.body.is_empty());
    for r in &client.transport.requests {
        assert!(
            !r.headers.iter().any(|(key, _)| key == "X-Requested-With"),
            "legacy auth headers unchanged"
        );
    }
    client
}
fn success(body: String) -> Page {
    (200, vec![], body)
}

#[test]
fn actual_name_client_reuses_one_synthetic_auth_flow_and_fixed_post_cookie_headers_and_pairs() {
    let mut client = ready(vec![success(page()), success(page())]);
    for _ in 0..2 {
        assert_eq!(
            client.search_students_by_name(&input(NAME.into())).unwrap(),
            Js::Arr(vec![
                obj(&[("studentKey", "007".into()), ("studentName", NAME.into())]),
                obj(&[("studentKey", "7".into()), ("studentName", NAME.into())]),
            ])
        );
    }
    assert_eq!(client.transport.requests.len(), 5);
    for r in &client.transport.requests[3..] {
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, PATH);
        assert_eq!(r.query, vec![("reqCmd".into(), "DtResultSearch".into())]);
        assert_eq!(
            r.body,
            vec![
                ("sort_date".into(), "1".into()),
                ("stu_name".into(), NAME.into())
            ]
        );
        assert_eq!(
            r.headers,
            vec![
                ("Origin".into(), ORIGIN.into()),
                ("Referer".into(), format!("{ORIGIN}/c_login.jsp")),
                ("Cookie".into(), COOKIE.into()),
                (
                    "Content-Type".into(),
                    "application/x-www-form-urlencoded; charset=UTF-8".into()
                ),
                ("X-Requested-With".into(), "XMLHttpRequest".into()),
            ]
        );
        assert_eq!(r.timeout, Duration::from_secs(20));
        assert!(
            !r.query
                .iter()
                .chain(&r.body)
                .any(|(key, _)| key == "studentKey" || key == "pri_no")
        );
    }
    assert!(client.transport.responses.is_empty());
    assert!(client.authenticated);
}

#[test]
fn actual_name_client_rejects_keys_name_principal_and_missing_auth_before_search() {
    let mut client = client(vec![]);
    let mut extra = input(NAME.into());
    extra.set("studentKey", "7".into());
    let mut wrong_teacher = input(NAME.into());
    wrong_teacher.set("teacherLabel", "Other Fictional Teacher".into());
    for (request, code) in [
        (extra, "unexpected or missing domain fields"),
        (
            obj(&[("teacherLabel", TEACHER.into())]),
            "unexpected or missing domain fields",
        ),
        (input("\u{feff}\u{2028}".into()), "invalid_read_name"),
        (input("😀".repeat(101).into()), "invalid_read_name"),
        (input(Js::Num(1.0)), "invalid_read_name"),
        (wrong_teacher, "daily_search_principal_conflict"),
        (input(NAME.into()), "authentication_required"),
    ] {
        assert_eq!(
            client
                .search_students_by_name(&request)
                .unwrap_err()
                .message,
            code
        );
    }
    assert_eq!(
        client.search_students_by_name(&Js::Null).unwrap_err().name,
        "TypeError"
    );
    assert_eq!(
        client
            .search_students_by_name(&Js::Undefined)
            .unwrap_err()
            .name,
        "TypeError"
    );
    assert!(client.transport.requests.is_empty());
    let mut authenticated = ready(vec![]);
    let mut request = input(NAME.into());
    request.set("studentKey", "7".into());
    assert_eq!(
        authenticated
            .search_students_by_name(&request)
            .unwrap_err()
            .message,
        "unexpected or missing domain fields"
    );
    assert_eq!(authenticated.transport.requests.len(), 3);
}

#[test]
fn actual_name_client_typed_503_and_redirects_are_finite_without_closing_or_retry() {
    for (status, headers, expected) in [
        (503, vec![], "source_http_failed"),
        (
            302,
            vec![("Location".into(), "/c_login.jsp".into())],
            "source_redirect_rejected",
        ),
        (
            302,
            vec![("Location".into(), "https://fictional.invalid/login".into())],
            "foreign_source_redirect_rejected",
        ),
    ] {
        let mut client = ready(vec![(status, headers, String::new())]);
        let error = client
            .search_students_by_name(&input(NAME.into()))
            .unwrap_err();
        assert_eq!(error.message, expected);
        if status == 503 {
            assert_eq!(error.name, "AcademyHttpFailure");
            // Pinned TS call omits sourceOperation: retain the generic request label.
            assert_eq!(
                error.properties,
                obj(&[
                    ("sourceOperation", "owned_source_request".into()),
                    ("httpStatus", Js::Num(503.0))
                ])
            );
            let fault = fault_from_error(error.clone());
            assert_eq!(error_from_fault(fault).properties, error.properties);
        }
        assert!(
            client.authenticated,
            "original name method does not close on HTTP/redirect failure"
        );
        assert_eq!(client.transport.requests.len(), 4);
        assert!(client.transport.responses.is_empty());
    }
}

#[test]
fn actual_name_client_challenge_closes_session_and_never_reauthenticates() {
    for challenge in [
        "webUserPass",
        "세션이 만료",
        "<INPUT TYPE='PASSWORD'",
        "<input data-type\u{feff}=\"password'",
    ] {
        let mut client = ready(vec![success(challenge.into())]);
        assert_eq!(
            client
                .search_students_by_name(&input(NAME.into()))
                .unwrap_err()
                .message,
            "authentication_required"
        );
        assert!(!client.authenticated);
        assert_eq!(
            client
                .search_students_by_name(&input(NAME.into()))
                .unwrap_err()
                .message,
            "authentication_required"
        );
        assert_eq!(
            client
                .authenticate("fictional-name-user", "fictional-name-password")
                .unwrap_err()
                .message,
            "fresh_authentication_attempt_required"
        );
        assert_eq!(client.transport.requests.len(), 4);
        assert!(client.transport.responses.is_empty());
    }
}

#[test]
fn actual_name_client_parser_failures_remain_finite_and_keep_the_authenticated_session() {
    for (body, code) in [
        (
            "<p>empty fictional page</p>",
            "result_list_missing_or_ambiguous",
        ),
        (
            "testResultList:JSON.parse('[]');testResultList:JSON.parse('[]')",
            "result_list_missing_or_ambiguous",
        ),
        (
            "testResultList:JSON.parse('null')",
            "student_search_list_rejected",
        ),
        (
            "testResultList:JSON.parse('[null]')",
            "invalid_capture_shape",
        ),
        (
            r#"testResultList:JSON.parse('[{"userName":"F","priNo":null}]')"#,
            "invalid_source_key",
        ),
        (
            r"testResultList:JSON.parse('\v')",
            "invalid_embedded_escape",
        ),
    ] {
        let mut client = ready(vec![success(body.into())]);
        assert_eq!(
            client
                .search_students_by_name(&input("F".into()))
                .unwrap_err()
                .message,
            code
        );
        assert!(client.authenticated);
        assert_eq!(client.transport.requests.len(), 4);
        assert!(client.transport.responses.is_empty());
    }
}

#[test]
fn actual_name_client_response_bound_precedes_challenge_and_uses_utf16_not_bytes() {
    let empty = "testResultList:JSON.parse('[]')";
    let mut boundary = empty.to_string();
    boundary.push_str(&"😀".repeat((2_000_000 - empty.len()) / 2));
    if boundary.encode_utf16().count() < 2_000_000 {
        boundary.push(' ');
    }
    let mut client = ready(vec![
        success(boundary.clone()),
        success(boundary + "webUserPass"),
    ]);
    assert_eq!(
        client.search_students_by_name(&input(NAME.into())).unwrap(),
        Js::Arr(vec![])
    );
    assert_eq!(
        client
            .search_students_by_name(&input(NAME.into()))
            .unwrap_err()
            .message,
        "daily_search_response_rejected"
    );
    assert!(
        client.authenticated,
        "oversize check precedes auth challenge/close"
    );
    assert_eq!(client.transport.requests.len(), 5);
    assert!(client.transport.responses.is_empty());
}

#[test]
fn actual_name_client_usv_form_conversion_keeps_parser_requested_utf16_identity() {
    let name = Js::Str(vec![0xd800]);
    let mut client = ready(vec![success(
        r#"testResultList:JSON.parse('[{"userName":"\uD800","priNo":"0007"}]')"#.into(),
    )]);
    let result = client
        .search_students_by_name(&input(name.clone()))
        .unwrap();
    assert_eq!(
        result,
        Js::Arr(vec![obj(&[
            ("studentKey", "0007".into()),
            ("studentName", name)
        ])])
    );
    assert_eq!(
        client.transport.requests[3].body[1],
        ("stu_name".into(), "\u{fffd}".into())
    );
    assert_eq!(client.transport.requests.len(), 4);
}
