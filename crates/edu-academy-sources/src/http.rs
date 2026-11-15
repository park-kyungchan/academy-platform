//! Bounded native HTTP and recorded compatibility. No default runtime activation.
#[path = "name_search.rs"]
pub(crate) mod name_search;

use crate::{
    data::*,
    dom::{self, Document},
    native::{literal_key, literal_name},
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
pub const ORIGIN: &str = "https://dc.gang-a.kr";
pub const MAIN: &str = "/servlet/controller.cct.tutor.DayRecordServlet";
pub const SELECTED: &str = "/servlet/controller.cct.tutor.CourseScheduleServlet";
#[derive(Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub timeout: Duration,
}
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub bytes: Vec<u8>,
    pub url: String,
    pub stalled: bool,
}
impl Response {
    fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
    fn text(&self) -> String {
        let s = String::from_utf8_lossy(&self.bytes);
        s.strip_prefix('\u{feff}').unwrap_or(&s).to_string()
    }
}
pub trait Transport {
    /// Wrappers around NativeTransport must forward this binding. The production
    /// transport refuses dispatch without a retained, valid account lease.
    fn bind_account(&mut self, _lease: std::rc::Rc<crate::owner_root::AccountLease>) {}
    fn dispatch(&mut self, request: &Request) -> Result<Response>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterStudent {
    pub student_name: String,
    pub student_key: String,
    pub course_key: String,
    pub record_key: String,
    pub membership_key: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRoster {
    pub date: String,
    pub group_id: String,
    pub teacher_key: String,
    pub students: Vec<RosterStudent>,
    pub coverage: String,
}
fn err(code: &str) -> Error {
    Error::error(code)
}
pub fn plan(input: &Js) -> Result<Js> {
    let keys = ["family", "teacherLabel", "date", "groupIds", "studentNames"];
    if !object(input) || input.props().len() != 5 || keys.iter().any(|k| !input.has(k)) {
        return Err(err("unexpected_read_request_field"));
    }
    if !input.get("family").is("day_record") {
        return Err(err("invalid_day_record_scope"));
    }
    if !valid_date(input.get("date")) {
        return Err(err("invalid_read_date"));
    }
    for (key, min, max) in [("groupIds", 1, 12), ("studentNames", 1, 64)] {
        let v = input.get(key);
        if !array(v)
            || v.array().len() < min
            || v.array().len() > max
            || edu_academy_core::js::unique(v.array()).len() != v.array().len()
        {
            return Err(err("invalid_day_record_scope"));
        }
    }
    literal_name(input.get("teacherLabel"))?;
    for id in input.get("groupIds").array() {
        literal_key(id)?;
    }
    for name in input.get("studentNames").array() {
        literal_name(name)?;
    }
    Ok(obj(&[
        ("family", "day_record".into()),
        ("teacherLabel", input.get("teacherLabel").clone()),
        ("date", input.get("date").clone()),
        ("groupIds", input.get("groupIds").clone()),
        ("studentNames", input.get("studentNames").clone()),
    ]))
}
pub fn operations() -> Js {
    Js::Arr(vec![
        obj(&[
            ("path", MAIN.into()),
            ("method", "POST".into()),
            ("selectorKey", "p_process".into()),
            ("selector", "Main".into()),
            ("selectorPlacement", "body".into()),
        ]),
        obj(&[
            ("path", SELECTED.into()),
            ("method", "GET".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "SelSglRecord".into()),
            ("selectorPlacement", "query".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.CourseManageServlet".into(),
            ),
            ("method", "GET".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "WebUnPreStudy".into()),
            ("selectorPlacement", "query".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.CourseManageServlet".into(),
            ),
            ("method", "POST".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "WebUnPreStudy".into()),
            ("selectorPlacement", "body".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.CourseManageServlet".into(),
            ),
            ("method", "GET".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "WebCtPreStudy".into()),
            ("selectorPlacement", "query".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.CourseManageServlet".into(),
            ),
            ("method", "POST".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "WebCtPreStudy".into()),
            ("selectorPlacement", "body".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.CourseManageServlet".into(),
            ),
            ("method", "GET".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "TeacherPrestudySummary".into()),
            ("selectorPlacement", "query".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.CourseManageServlet".into(),
            ),
            ("method", "GET".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "WebPreStudy".into()),
            ("selectorPlacement", "query".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.CourseManageServlet".into(),
            ),
            ("method", "GET".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "GaStudyAnswer".into()),
            ("selectorPlacement", "query".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.coursemanage.TextBookManageServlet".into(),
            ),
            ("method", "POST".into()),
            ("selectorKey", "reqCmd".into()),
            ("selector", "getTextBookProgress".into()),
            ("selectorPlacement", "query".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.common.TestpageSelectExServlet".into(),
            ),
            ("method", "POST".into()),
            ("selectorKey", "p_process".into()),
            ("selector", "getWrongAnswerNoteResult".into()),
            ("selectorPlacement", "body".into()),
        ]),
        obj(&[
            (
                "path",
                "/servlet/controller.common.TestpageSelectExServlet".into(),
            ),
            ("method", "POST".into()),
            ("selectorKey", "p_process".into()),
            ("selector", "getStudyResultSingleTestingSingleUser".into()),
            ("selectorPlacement", "body".into()),
        ]),
    ])
}
pub fn admit_operation(
    origin: &str,
    path: &str,
    method: &str,
    query: &[(String, String)],
    body: &[(String, String)],
) -> Option<Js> {
    if origin != ORIGIN {
        return None;
    }
    operations()
        .array()
        .iter()
        .find(|op| {
            let pairs = if op.get("selectorPlacement").is("body") {
                body
            } else {
                query
            };
            op.get("path").is(path)
                && op.get("method").is(method)
                && pairs
                    .iter()
                    .filter(|(k, _)| op.get("selectorKey").is(k))
                    .count()
                    == 1
                && query
                    .iter()
                    .chain(body)
                    .filter(|(k, _)| op.get("selectorKey").is(k))
                    .count()
                    <= 2
                && pairs
                    .iter()
                    .any(|(k, v)| op.get("selectorKey").is(k) && op.get("selector").is(v))
        })
        .cloned()
}
pub struct Client<T: Transport> {
    pub transport: T,
    pub principal: Js,
    pub authenticated: bool,
    attempted: bool,
    cookies: Vec<(String, String, String)>,
}
impl<T: Transport> Client<T> {
    pub fn new(principal: Js, transport: T) -> Result<Self> {
        literal_name(principal.get("teacherLabel"))?;
        literal_key(principal.get("teacherKey"))?;
        literal_key(principal.get("collegeKey"))?;
        Ok(Self {
            transport,
            principal,
            authenticated: false,
            attempted: false,
            cookies: vec![],
        })
    }
    pub fn close(&mut self) {
        self.cookies.clear();
        self.authenticated = false;
    }
    /// Exact-name identities from one fixed, read-qualified DT/ZT page.
    pub fn search_students_by_name(&mut self, input: &Js) -> Result<Js> {
        edu_academy_core::js::exact(input, &["teacherLabel", "studentName"])
            .map_err(error_from_fault)?;
        literal_name(input.get("studentName"))?;
        if input.get("teacherLabel") != self.principal.get("teacherLabel") {
            return Err(err("daily_search_principal_conflict"));
        }
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let html = self
            .request_with_name_headers(
                "POST",
                "/servlet/controller.dailyzerotest.DailyZeroTestServlet",
                vec![("reqCmd".into(), "DtResultSearch".into())],
                vec![
                    ("sort_date".into(), "1".into()),
                    ("stu_name".into(), string(input.get("studentName"))),
                ],
                false,
                None,
                true,
            )?
            .text();
        if html.encode_utf16().count() > 2_000_000 {
            return Err(err("daily_search_response_rejected"));
        }
        if html.contains("webUserPass")
            || html.contains("세션이 만료")
            || name_search::password_input(&html)
        {
            self.close();
            return Err(err("authentication_required"));
        }
        name_search::inspect(&html, input.get("studentName"))
    }
    /// Owner-qualified journal mutation over the existing injected transport.
    /// `now` and `approval` are supplied by the runtime owner, not by caller flags.
    pub fn apply_journal_action(
        &mut self,
        action: &Js,
        approval: &mut dyn edu_academy_core::workflow::approval::Approval,
        now: &str,
    ) -> Result<Js> {
        use edu_academy_core::workflow::http;
        http::validate_action(action).map_err(journal_core_error)?;
        if action.get("target").get("teacherKey") != self.principal.get("teacherKey") {
            return Err(err("journal_principal_conflict"));
        }
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let mut port = JournalHttpPort {
            client: self,
            now,
            read_error: None,
        };
        match http::apply(action, &mut port, approval) {
            Ok(result) => Ok(result),
            Err(fault) => Err(port
                .read_error
                .take()
                .unwrap_or_else(|| journal_core_error(fault))),
        }
    }
    fn request(
        &mut self,
        method: &str,
        path: &str,
        query: Vec<(String, String)>,
        body: Vec<(String, String)>,
        follow: bool,
        operation: Option<&str>,
    ) -> Result<Response> {
        self.request_with_name_headers(method, path, query, body, follow, operation, false)
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Mechanical opt-in header extraction"
    )]
    fn request_with_name_headers(
        &mut self,
        method: &str,
        path: &str,
        query: Vec<(String, String)>,
        body: Vec<(String, String)>,
        follow: bool,
        operation: Option<&str>,
        name_headers: bool,
    ) -> Result<Response> {
        let mut request_url = url::Url::parse(ORIGIN)
            .expect("fixed origin")
            .join(path)
            .map_err(|_| err("fixed_academy_origin_required"))?;
        if request_url.origin().ascii_serialization() != ORIGIN
            || !request_url.username().is_empty()
            || request_url.password().is_some_and(|s| !s.is_empty())
            || request_url.fragment().is_some_and(|s| !s.is_empty())
        {
            return Err(err("fixed_academy_origin_required"));
        }
        if !query.is_empty() {
            request_url
                .query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (k, v)));
        }
        let path = request_url.path();
        let mut headers = vec![
            ("Origin".into(), ORIGIN.into()),
            ("Referer".into(), format!("{ORIGIN}/c_login.jsp")),
        ];
        let cookie = self
            .cookies
            .iter()
            .filter(|(_, _, p)| path.starts_with(p))
            .map(|(k, v, _)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        if !cookie.is_empty() {
            headers.push(("Cookie".into(), cookie))
        }
        if method == "POST" {
            headers.push((
                "Content-Type".into(),
                "application/x-www-form-urlencoded; charset=UTF-8".into(),
            ))
        }
        if name_headers {
            headers.push(("X-Requested-With".into(), "XMLHttpRequest".into()));
        }
        let request = Request {
            method: method.into(),
            path: path.into(),
            query,
            body,
            headers,
            timeout: Duration::from_secs(20),
        };
        let response = self.transport.dispatch(&request).map_err(|error| {
            if matches!(
                error.message.as_str(),
                "native_transport_timeout"
                    | "native_owner_credential_expired"
                    | "native_transport_cancelled"
            ) {
                err(&error.message)
            } else {
                err("academy_transport_failed")
            }
        })?;
        // Every family, including login/discovery, is bounded before parsing.
        if response.stalled {
            return Err(err("source_response_timeout"));
        }
        if response.bytes.len() > MAX_RESPONSE_BYTES {
            return Err(err("source_response_too_large"));
        }
        for (_, raw) in response
            .headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
        {
            let mut parts = raw.split(';');
            let Some(pair) = parts.next() else { continue };
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            let name = name.trim();
            if name != "JSESSIONID" {
                continue;
            }
            let attributes = parts.map(str::trim).collect::<Vec<_>>();
            let attr = |key: &str| {
                attributes.iter().find_map(|a| {
                    a.split_once('=')
                        .filter(|(k, _)| k.eq_ignore_ascii_case(key))
                        .map(|(_, v)| v.trim())
                })
            };
            if let Some(domain) = attr("domain")
                && domain.strip_prefix('.').unwrap_or(domain) != "dc.gang-a.kr"
            {
                return Err(err("foreign_session_cookie_rejected"));
            }
            let path = attr("path").unwrap_or("/");
            if !path.starts_with('/') || value.contains(['\r', '\n', ';']) {
                return Err(err("session_cookie_contract_rejected"));
            }
            let remove = value.is_empty() || attr("max-age") == Some("0");
            if let Some(i) = self.cookies.iter().position(|(k, _, _)| k == name) {
                if remove {
                    self.cookies.remove(i);
                } else {
                    self.cookies[i] = (name.into(), value.into(), path.into());
                }
            } else if !remove {
                self.cookies.push((name.into(), value.into(), path.into()))
            }
        }
        if (300..400).contains(&response.status) {
            let loc = response
                .header("location")
                .ok_or_else(|| err("source_redirect_contract_rejected"))?;
            if loc.is_empty() {
                return Err(err("source_redirect_contract_rejected"));
            }
            let next = request_url
                .join(loc)
                .map_err(|_| err("foreign_source_redirect_rejected"))?;
            if next.origin().ascii_serialization() != ORIGIN
                || !next.username().is_empty()
                || next.password().is_some_and(|s| !s.is_empty())
                || next.fragment().is_some_and(|s| !s.is_empty())
            {
                return Err(err("foreign_source_redirect_rejected"));
            }
            if !follow {
                return Err(err("source_redirect_rejected"));
            }
            return self.request(
                "GET",
                next.path(),
                parse_form(next.query().unwrap_or("")),
                vec![],
                false,
                None,
            );
        }
        if !(200..300).contains(&response.status) {
            if matches!(response.status, 401 | 403) {
                self.close();
            }
            let mut e = Error::new("AcademyHttpFailure", "source_http_failed");
            let op = operation.unwrap_or(match path {
                "/c_login.jsp" => "login_document",
                "/servlet/controller.login.LoginServlet" => "login_submission",
                _ => "owned_source_request",
            });
            e.properties = obj(&[
                ("sourceOperation", op.into()),
                ("httpStatus", Js::Num(f64::from(response.status))),
            ]);
            return Err(e);
        }
        Ok(response)
    }
    pub(crate) fn scoped_request(&mut self, request: Request) -> Result<Response> {
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let response = match self.request(
            &request.method,
            &request.path,
            request.query,
            request.body,
            false,
            None,
        ) {
            Ok(response) => response,
            Err(error) => {
                // Any abnormal scoped exchange permanently invalidates this
                // session, regardless of redirect/status/cookie/size/I/O cause.
                self.close();
                return Err(error);
            }
        };
        if self.authenticated {
            let text = String::from_utf8_lossy(&response.bytes);
            // Generic HTML handling must accept valid unquoted/case-insensitive
            // attributes. The narrow name-search helper preserves a different
            // historical wire contract and must not be broadened globally.
            let document = Document::parse(&text);
            let password = document.select("input").iter().any(|input| {
                input
                    .value()
                    .attr("type")
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("password"))
            });
            let login_form = document.select("form").iter().any(|form| {
                ["id", "name"].iter().any(|key| {
                    form.value()
                        .attr(key)
                        .is_some_and(|value| value.eq_ignore_ascii_case("loginForm"))
                })
            });
            if text.contains("webUserPass")
                || text.contains("세션이 만료")
                || text.contains("normal_session_error.jsp")
                || text.contains("popup_session_error.jsp")
                || password
                || login_form
            {
                self.close();
                return Err(err("authentication_required"));
            }
        }
        Ok(response)
    }
    pub fn authenticate(&mut self, username: &str, password: &str) -> Result<Js> {
        if self.attempted {
            return Err(err("fresh_authentication_attempt_required"));
        }
        self.attempted = true;
        if username.is_empty() || password.is_empty() {
            return Err(err("native_http_credentials_required"));
        }
        let result = (|| {
            let html = self
                .request("GET", "/c_login.jsp", vec![], vec![], false, None)?
                .text();
            let doc = Document::parse(&html);
            let form = doc
                .select("form#loginForm")
                .first()
                .copied()
                .ok_or_else(|| err("login_wire_contract_drift"))?;
            let names = [
                "p_process",
                "p_prino",
                "p_groupno",
                "userpwd",
                "userid",
                "cf_pc",
                "cf_user",
                "autoLoginCheck",
                "autoLoginKey",
            ];
            if form
                .value()
                .attr("method")
                .map(str::to_lowercase)
                .as_deref()
                != Some("post")
                || names
                    .iter()
                    .any(|n| dom::select(form, &format!("[name={n}]")).is_empty())
                || !html.contains(
                    "document.loginForm.action = \"/servlet/controller.login.LoginServlet\"",
                )
                || !html.contains("document.loginForm.p_process.value = \"c_login\"")
            {
                return Err(err("login_wire_contract_drift"));
            }
            let body = vec![
                ("p_process".into(), "c_login".into()),
                ("p_prino".into(), "".into()),
                ("p_groupno".into(), "".into()),
                ("cf_pc".into(), "".into()),
                ("cf_user".into(), "".into()),
                ("autoLoginCheck".into(), "".into()),
                ("autoLoginKey".into(), "".into()),
                ("userid".into(), login_encode(username)?),
                ("userpwd".into(), login_encode(password)?),
            ];
            self.request(
                "POST",
                "/servlet/controller.login.LoginServlet",
                vec![],
                body,
                true,
                None,
            )?;
            let html = self
                .request(
                    "GET",
                    "/servlet/controller.coursemanage.CourseManageServlet",
                    vec![("reqCmd".into(), "StudyCourse".into())],
                    vec![],
                    false,
                    None,
                )?
                .text();
            let doc = Document::parse(&html);
            let val = |s: &str| doc.select(s).first().copied().map(dom::value);
            if !doc.select("input[type=password]").is_empty() {
                return Err(err("source_login_challenge"));
            }
            for (s, key, code) in [
                (
                    "form[name=userForm] [name=pri_no]",
                    "teacherKey",
                    "teacher_user_principal_conflict",
                ),
                (
                    "form[name=form1] [name=g_pri_no]",
                    "teacherKey",
                    "teacher_course_principal_conflict",
                ),
                (
                    "form[name=form1] [name=g_clg_no]",
                    "collegeKey",
                    "teacher_college_conflict",
                ),
            ] {
                if val(s).as_deref() != Some(&string(self.principal.get(key))) {
                    return Err(err(code));
                }
            }
            if val("form[name=form1] [name=g_mem_type]").is_none_or(|s| s.is_empty()) {
                return Err(err("teacher_member_type_missing"));
            }
            if !doc
                .select("a,span")
                .iter()
                .any(|e| dom::content(*e).contains("로그아웃"))
            {
                return Err(err("teacher_logout_marker_drift"));
            }
            self.authenticated = true;
            Ok(obj(&[
                ("state", "authenticated".into()),
                ("principalKey", self.principal.get("teacherKey").clone()),
                ("collegeKey", self.principal.get("collegeKey").clone()),
                ("transport", "direct_http_api".into()),
            ]))
        })();
        if result.is_err() {
            self.close()
        }
        result
    }
    fn read_request(
        &mut self,
        method: &str,
        path: &str,
        query: Vec<(String, String)>,
        body: Vec<(String, String)>,
    ) -> Result<String> {
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let operation = admit_operation(ORIGIN, path, method, &query, &body)
            .ok_or_else(|| err("reviewed_read_operation_required"))?;
        let response = match self.request(
            method,
            path,
            query,
            body,
            false,
            Some(&string(operation.get("selector"))),
        ) {
            Ok(r) => r,
            Err(e) => {
                if (e.name == "AcademyHttpFailure"
                    && matches!(number(e.properties.get("httpStatus")), Some(401. | 403.)))
                    || e.message == "source_redirect_rejected"
                {
                    self.close();
                    return Err(err("authentication_required"));
                }
                return Err(e);
            }
        };
        if response.bytes.len() > 2_000_000 {
            return Err(err("source_response_too_large"));
        }
        let text = String::from_utf8_lossy(&response.bytes).to_string();
        let challenge=regex::Regex::new(r##"(?i)webUserPass|세션이 만료|<input\b[^>]*\btype\s*=\s*["']?password\b|<form\b[^>]*\bname\s*=\s*["']?loginForm\b"##).unwrap();
        if challenge.is_match(&text) {
            self.close();
            return Err(err("authentication_required"));
        }
        Ok(text)
    }
    pub fn discover(&mut self, input: &Js, now: &str) -> Result<Js> {
        if input.props().len() != 2 || !input.has("teacherLabel") || !input.has("date") {
            return Err(err("unexpected or missing domain fields"));
        }
        if !valid_date(input.get("date")) {
            return Err(err("real_calendar_date_required"));
        }
        if input.get("teacherLabel") != self.principal.get("teacherLabel") {
            return Err(err("day_record_group_principal_conflict"));
        }
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let html = self
            .request(
                "POST",
                MAIN,
                vec![],
                vec![
                    ("p_process".into(), "Main".into()),
                    ("std_ymd".into(), string(input.get("date")).replace('-', "")),
                ],
                false,
                None,
            )?
            .text();
        if html.encode_utf16().count() > 2_000_000 {
            return Err(err("source_response_too_large"));
        }
        let doc = Document::parse(&html);
        if !doc.select("input[type=password]").is_empty()
            || html.contains("webUserPass")
            || html.contains("세션이 만료")
        {
            self.close();
            return Err(err("authentication_required"));
        }
        let dates = doc.select("input[name=std_date]");
        let teachers = doc.select("input[name=teacher_pri_no]");
        let selectors = doc.select("select[name=grp_seq]");
        if dates.len() != 1
            || teachers.len() != 1
            || selectors.len() != 1
            || !input.get("date").is(&dom::value(dates[0]))
            || !self
                .principal
                .get("teacherKey")
                .is(&dom::value(teachers[0]))
        {
            return Err(err("day_record_group_scope_conflict"));
        }
        let select = selectors[0];
        let options = dom::select(select, "option");
        if select.value().attr("disabled").is_some() || options.len() > 64 {
            return Err(err("day_record_group_options_rejected"));
        }
        let mut groups = vec![];
        let mut seen = vec![];
        for o in options {
            if o.value().attr("disabled").is_some()
                || dom::closest(o, "optgroup").is_some_and(|g| g.value().attr("disabled").is_some())
            {
                continue;
            }
            let v = dom::value(o);
            if v.is_empty() || v == "0" {
                continue;
            }
            literal_key(&v.clone().into())?;
            let label = Js::Str(trim(&text(&dom::content(o))).to_vec());
            if label.units().is_empty() || seen.contains(&v) {
                return Err(err("day_record_group_options_rejected"));
            }
            seen.push(v.clone());
            groups.push(obj(&[("groupId", v.into()), ("label", label)]));
        }
        if groups.is_empty() || groups.len() > 12 {
            return Err(err("day_record_group_options_rejected"));
        }
        let selected = dom::value(select);
        let selected = if selected.is_empty() || selected == "0" {
            Js::Null
        } else {
            literal_key(&selected.clone().into())?;
            if !seen.contains(&selected) {
                return Err(err("day_record_group_options_rejected"));
            }
            selected.into()
        };
        Ok(obj(&[
            ("teacherKey", self.principal.get("teacherKey").clone()),
            ("date", input.get("date").clone()),
            ("groups", Js::Arr(groups)),
            ("selectedGroupId", selected),
            ("observedAt", now.into()),
            (
                "sourceOperation",
                "POST DayRecordServlet p_process=Main".into(),
            ),
            (
                "coverage",
                "current_date_selector_only_not_student_membership".into(),
            ),
            ("academyWrites", Js::Num(0.)),
            ("parentSends", Js::Num(0.)),
        ]))
    }
    /// Alias for `discover`: groups of one date, with no second request.
    pub fn list_groups_of_date(&mut self, date: &str, now: &str) -> Result<Js> {
        self.discover(
            &obj(&[
                ("teacherLabel", self.principal.get("teacherLabel").clone()),
                ("date", date.into()),
            ]),
            now,
        )
    }
    /// Roster of one group on one date: one `student.day.main` request, no
    /// per-student `SelSglRecord` corroboration.
    pub fn read_group_roster(&mut self, date: &str, group_id: &str) -> Result<GroupRoster> {
        if !valid_date(&date.into()) {
            return Err(err("real_calendar_date_required"));
        }
        literal_key(&group_id.into())?;
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let html = self.read_request(
            "POST",
            MAIN,
            vec![],
            vec![
                ("p_process".into(), "Main".into()),
                ("std_ymd".into(), date.replace('-', "")),
                ("grp_seq".into(), group_id.into()),
            ],
        )?;
        let doc = Document::parse(&html);
        if doc
            .select("[name=std_date]")
            .first()
            .is_none_or(|e| dom::value(*e) != date)
            || doc
                .select("[name=grp_seq]")
                .first()
                .is_none_or(|e| dom::value(*e) != group_id)
            || doc
                .select("[name=teacher_pri_no]")
                .first()
                .is_none_or(|e| dom::value(*e) != string(self.principal.get("teacherKey")))
        {
            return Err(err("day_record_scope_conflict"));
        }
        let links = doc
            .select("[onclick]")
            .into_iter()
            .filter(|e| {
                e.value()
                    .attr("onclick")
                    .unwrap_or("")
                    .contains("onOpenDaysMain")
            })
            .collect::<Vec<_>>();
        if links.len() > 60 {
            return Err(err("day_record_roster_rows_rejected"));
        }
        let teacher_key = string(self.principal.get("teacherKey"));
        if links.is_empty() {
            if doc
                .select("table")
                .iter()
                .any(|t| validate_table_headers(*t).is_ok())
            {
                return Ok(GroupRoster {
                    date: date.into(),
                    group_id: group_id.into(),
                    teacher_key,
                    students: vec![],
                    coverage:
                        "scheduled_rows_of_group_on_date_not_enrolment_not_corroborated_by_selected_record"
                            .into(),
                });
            }
            return Err(err("day_record_column_contract_rejected"));
        }
        let mut students: Vec<RosterStudent> = vec![];
        for link in links {
            let student_name = string(&Js::Str(trim(&text(&dom::content(link))).to_vec()));
            if student_name.is_empty() {
                return Err(err("requested_row_binding_unresolved"));
            }
            let row = day_record_row(link)?;
            validate_headers(row)?;
            let (_fields, binding) = bind_row(&doc, row)?;
            let student = RosterStudent {
                student_name,
                student_key: string(binding.get("stu_pri_no")),
                course_key: string(binding.get("course_seq")),
                record_key: string(binding.get("record_seq")),
                membership_key: string(binding.get("cm_seq")),
            };
            if students.iter().any(|s| {
                s.student_key == student.student_key || s.student_name == student.student_name
            }) {
                return Err(err("ambiguous_day_record_identity"));
            }
            students.push(student);
        }
        students.sort_by(|a, b| {
            a.student_name
                .cmp(&b.student_name)
                .then_with(|| a.student_key.cmp(&b.student_key))
        });
        Ok(GroupRoster {
            date: date.into(),
            group_id: group_id.into(),
            teacher_key,
            students,
            coverage:
                "scheduled_rows_of_group_on_date_not_enrolment_not_corroborated_by_selected_record"
                    .into(),
        })
    }
    pub fn execute(&mut self, input: &Js, now: &str) -> Result<Js> {
        let request = plan(input)?;
        if request.get("teacherLabel") != self.principal.get("teacherLabel") {
            return Err(err("teacher_scope_conflict"));
        }
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let mut rows = vec![];
        for group in request.get("groupIds").array() {
            let html = self.read_request(
                "POST",
                MAIN,
                vec![],
                vec![
                    ("p_process".into(), "Main".into()),
                    (
                        "std_ymd".into(),
                        string(request.get("date")).replace('-', ""),
                    ),
                    ("grp_seq".into(), string(group)),
                ],
            )?;
            let doc = Document::parse(&html);
            if doc
                .select("[name=std_date]")
                .first()
                .is_none_or(|e| !request.get("date").is(&dom::value(*e)))
                || doc
                    .select("[name=grp_seq]")
                    .first()
                    .is_none_or(|e| !group.is(&dom::value(*e)))
                || doc
                    .select("[name=teacher_pri_no]")
                    .first()
                    .is_none_or(|e| !self.principal.get("teacherKey").is(&dom::value(*e)))
            {
                return Err(err("day_record_scope_conflict"));
            }
            let links = doc
                .select("[onclick]")
                .into_iter()
                .filter(|e| {
                    e.value()
                        .attr("onclick")
                        .unwrap_or("")
                        .contains("onOpenDaysMain")
                })
                .collect::<Vec<_>>();
            for name in request.get("studentNames").array() {
                for link in links
                    .iter()
                    .filter(|e| name.units() == trim(&text(&dom::content(**e))))
                {
                    let mut row = dom::closest(*link, "tr");
                    while let Some(r) = row {
                        let controls = dom::select(r, "[onchange],[onfocusout],[onclick]");
                        if ["onAttn", "onFoPrg", "onFoHw", "onFoMm"].iter().all(|h| {
                            controls
                                .iter()
                                .filter(|e| {
                                    ["onchange", "onfocusout", "onclick"].iter().any(|k| {
                                        e.value().attr(k).unwrap_or("").contains(&format!("{h}("))
                                    })
                                })
                                .count()
                                == 1
                        }) {
                            break;
                        }
                        row = dom::closest(r, "tr");
                    }
                    let row = row.ok_or_else(|| err("requested_row_structure_unresolved"))?;
                    validate_headers(row)?;
                    let (fields, binding) = bind_row(&doc, row)?;
                    let query = vec![
                        ("reqCmd".into(), "SelSglRecord".into()),
                        ("course_seq".into(), string(binding.get("course_seq"))),
                        ("stu_pri_no".into(), string(binding.get("stu_pri_no"))),
                        ("record_seq".into(), string(binding.get("record_seq"))),
                        ("sel_grp_seq".into(), string(group)),
                        (
                            "tut_pri_no".into(),
                            string(self.principal.get("teacherKey")),
                        ),
                        ("cm_seq".into(), string(binding.get("cm_seq"))),
                    ];
                    let xml = self.read_request("GET", SELECTED, query, vec![])?;
                    let values = xml_record(&xml)?;
                    let v = values.split('!').collect::<Vec<_>>();
                    let required = [
                        (1, request.get("date")),
                        (2, binding.get("course_seq")),
                        (4, name),
                        (9, group),
                        (10, binding.get("record_seq")),
                        (11, binding.get("stu_pri_no")),
                        (12, binding.get("cm_seq")),
                    ];
                    if required
                        .iter()
                        .any(|(i, expected)| v.get(*i).is_none_or(|s| !expected.is(s)))
                    {
                        return Err(err("independent_occurrence_conflict"));
                    }
                    if rows.iter().any(|r: &Js| {
                        r.get("studentName") == name
                            || (r.get("studentKey") == binding.get("stu_pri_no")
                                && r.get("recordKey") == binding.get("record_seq")
                                && r.get("membershipKey") == binding.get("cm_seq"))
                    }) {
                        return Err(err("ambiguous_day_record_identity"));
                    }
                    rows.push(obj(&[
                        ("studentName", name.clone()),
                        ("date", request.get("date").clone()),
                        ("groupId", group.clone()),
                        ("teacherKey", self.principal.get("teacherKey").clone()),
                        ("studentKey", binding.get("stu_pri_no").clone()),
                        ("courseKey", binding.get("course_seq").clone()),
                        ("recordKey", binding.get("record_seq").clone()),
                        ("membershipKey", binding.get("cm_seq").clone()),
                        ("fields", fields),
                    ]));
                }
            }
        }
        rows.sort_by(|a, b| {
            a.get("studentName")
                .units()
                .cmp(b.get("studentName").units())
        });
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let status = if rows.len() == request.get("studentNames").array().len() {
            "observed"
        } else if rows.is_empty() {
            "blocked"
        } else {
            "partial"
        };
        Ok(obj(&[
            ("schemaVersion", Js::Num(1.)),
            ("family", "day_record".into()),
            ("status", status.into()),
            ("records", Js::Arr(vec![])),
            ("dailyResults", Js::Arr(vec![])),
            ("dayRecords", Js::Arr(rows)),
            ("requestedScope", request),
            ("observedAt", now.into()),
            (
                "coverage",
                "requested_date_groups_students_not_cohort_completeness".into(),
            ),
            (
                "unknownStates",
                strings(&[
                    "whole_cohort_completeness",
                    "immutable_parent_sent_text",
                    "teacher_finalization",
                ]),
            ),
            ("nativePrintInvoked", false.into()),
            ("automaticRetryAllowed", false.into()),
        ]))
    }
    pub fn inspect_wire(&mut self, input: &Js) -> Result<Js> {
        let request = plan(input)?;
        if request.get("teacherLabel") != self.principal.get("teacherLabel") {
            return Err(err("day_record_wire_scope_required"));
        }
        let mut output = vec![];
        let next_function = regex::Regex::new(r"\n\s*function\s+").unwrap();
        for group in request.get("groupIds").array() {
            let html = self.read_request(
                "POST",
                MAIN,
                vec![],
                vec![
                    ("p_process".into(), "Main".into()),
                    (
                        "std_ymd".into(),
                        string(request.get("date")).replace('-', ""),
                    ),
                    ("grp_seq".into(), string(group)),
                ],
            )?;
            let doc = Document::parse(&html);
            let mut functions = obj(&[]);
            for script in doc.select("script") {
                let source = dom::content(script);
                for name in [
                    "onAttn",
                    "onFoPrg",
                    "onFoHw",
                    "onFoMm",
                    "setDailyTest",
                    "openHomeWorkRate",
                    "reallength",
                ] {
                    let re = regex::Regex::new(&format!(r"function\s+{name}\s*\(")).unwrap();
                    let Some(m) = re.find(&source) else { continue };
                    let next = next_function.find(&source[m.end()..]);
                    let end = next.map_or(source.len(), |m2| m.end() + m2.start());
                    let definition = source[m.start()..end].trim();
                    if definition.encode_utf16().count() > 20000 {
                        return Err(err("source_handler_selection_too_large"));
                    }
                    functions.set(name, definition.into());
                }
            }
            let mut popup = vec![];
            for script in doc.select("script") {
                let source = dom::content(script);
                let lines = source.split('\n').collect::<Vec<_>>();
                let mut selected = std::collections::BTreeSet::new();
                for (i, line) in lines.iter().enumerate() {
                    if [
                        "SetDailyTest",
                        "SetHomeWorkRate",
                        "daily_test_radio",
                        "homework_radio",
                        "CourseCommonServlet",
                        "daily_test_confirm",
                        "homework_confirm",
                    ]
                    .iter()
                    .any(|s| line.contains(s))
                    {
                        for n in i.saturating_sub(12)..=(i + 14).min(lines.len() - 1) {
                            selected.insert(n);
                        }
                    }
                }
                if !selected.is_empty() {
                    popup.push(
                        selected
                            .into_iter()
                            .map(|i| lines[i])
                            .collect::<Vec<_>>()
                            .join("\n")
                            .into(),
                    )
                }
            }
            let mut rows = vec![];
            for e in doc.select("[onclick]") {
                let name = dom::content(e).trim().to_string();
                if !request
                    .get("studentNames")
                    .array()
                    .iter()
                    .any(|s| s.is(&name))
                    || !e
                        .value()
                        .attr("onclick")
                        .unwrap_or("")
                        .contains("onOpenDaysMain")
                {
                    continue;
                }
                let mut row = dom::closest(e, "tr");
                while let Some(r) = row {
                    if dom::select(r, "[onfocusout]").iter().any(|e| {
                        e.value()
                            .attr("onfocusout")
                            .unwrap_or("")
                            .contains("onFoPrg(")
                    }) {
                        break;
                    }
                    row = dom::closest(r, "tr");
                }
                let controls = row.map_or(vec![], |r| {
                    dom::select(r, "input,select,textarea,[onclick]")
                        .iter()
                        .map(|e| {
                            obj(&[
                                ("id", e.value().attr("id").unwrap_or("").into()),
                                ("name", e.value().attr("name").map_or(Js::Null, Js::from)),
                                ("type", e.value().attr("type").map_or(Js::Null, Js::from)),
                                (
                                    "value",
                                    if matches!(e.value().name(), "input" | "select" | "textarea") {
                                        dom::value(*e).into()
                                    } else {
                                        Js::Null
                                    },
                                ),
                                (
                                    "handler",
                                    e.value()
                                        .attr("onclick")
                                        .or(e.value().attr("onfocusout"))
                                        .or(e.value().attr("onchange"))
                                        .map_or(Js::Null, Js::from),
                                ),
                            ])
                        })
                        .collect()
                });
                rows.push(obj(&[
                    ("studentName", name.into()),
                    ("controls", Js::Arr(controls)),
                ]));
            }
            let options = doc
                .select("input[name=homework_radio],input[name=daily_test_radio]")
                .iter()
                .map(|e| {
                    let label = dom::closest(*e, "td")
                        .or_else(|| e.parent().and_then(scraper::ElementRef::wrap))
                        .map_or(String::new(), dom::content);
                    obj(&[
                        ("field", e.value().attr("name").unwrap_or("").into()),
                        ("value", dom::value(*e).into()),
                        (
                            "label",
                            label
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                                .into(),
                        ),
                    ])
                })
                .collect();
            output.push(obj(&[
                ("groupId", group.clone()),
                ("date", request.get("date").clone()),
                ("sourceHandlersInvoked", false.into()),
                ("functions", functions),
                ("popupEvents", Js::Arr(popup)),
                ("rows", Js::Arr(rows)),
                ("assessmentOptions", Js::Arr(options)),
            ]));
        }
        Ok(Js::Arr(output))
    }
    pub fn journal_state(&mut self, target: &Js, now: &str) -> Result<Js> {
        if target.get("teacherKey") != self.principal.get("teacherKey") {
            return Err(err("journal_principal_conflict"));
        }
        if !self.authenticated {
            return Err(err("authentication_required"));
        }
        let request = obj(&[
            ("family", "day_record".into()),
            ("teacherLabel", self.principal.get("teacherLabel").clone()),
            ("date", target.get("date").clone()),
            ("groupIds", Js::Arr(vec![target.get("groupId").clone()])),
            (
                "studentNames",
                Js::Arr(vec![target.get("studentName").clone()]),
            ),
        ]);
        let found = self.execute(&request, now)?;
        let row = found
            .get("dayRecords")
            .array()
            .iter()
            .find(|r| r.get("studentName") == target.get("studentName"))
            .ok_or_else(|| err("journal_target_not_observed"))?;
        if crate::native::target(row) != *target {
            return Err(err("journal_target_conflict"));
        }
        let wire = self.inspect_wire(&request)?;
        let wire = wire
            .array()
            .first()
            .ok_or_else(|| err("journal_wire_not_observed"))?;
        if !wire
            .get("rows")
            .array()
            .iter()
            .any(|r| r.get("studentName") == target.get("studentName"))
        {
            return Err(err("journal_wire_not_observed"));
        }
        let digest = wire_digest(wire)?;
        if digest != "650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1" {
            return Err(err("journal_mutation_wire_not_qualified"));
        }
        self.journal_after_qualification(target, row, wire, &digest)
    }
    // Private structural seam; product callers must pass the immutable digest gate above.
    fn journal_after_qualification(
        &mut self,
        target: &Js,
        row: &Js,
        wire: &Js,
        digest: &str,
    ) -> Result<Js> {
        assert_assessment_options(wire.get("assessmentOptions"))?;
        for (handler, selector) in [
            ("onAttn", "udtAttn"),
            ("onFoPrg", "udtPrg"),
            ("onFoHw", "udtHw"),
            ("onFoMm", "udtMemo"),
            ("setDailyTest", "SetDailyTest"),
            ("openHomeWorkRate", "SetHomeWorkRate"),
        ] {
            let source = wire.get("functions").get(handler);
            if !source.truthy() || !string(source).contains(&format!("reqCmd={selector}")) {
                return Err(err("journal_wire_drift"));
            }
        }
        let controls = wire
            .get("rows")
            .array()
            .iter()
            .find(|r| r.get("studentName") == target.get("studentName"))
            .ok_or_else(|| err("journal_wire_not_observed"))?
            .get("controls")
            .array();
        let handlers = |prefix: &str| {
            controls
                .iter()
                .map(|c| string(c.get("handler")))
                .filter(|h| h.starts_with(prefix))
                .collect::<Vec<_>>()
        };
        let daily = handlers("setDailyTest(");
        let daily_re = regex::Regex::new(r"^setDailyTest\(([1-9][0-9]*),([1-9][0-9]*),[0-9]+\)$")
            .expect("static regex");
        let tuple = if daily.len() == 1 {
            daily_re.captures(&daily[0])
        } else {
            None
        };
        if !tuple.as_ref().is_some_and(|m| {
            target.get("recordKey").is(&m[1]) && target.get("membershipKey").is(&m[2])
        }) {
            return Err(err("journal_daily_tuple_conflict"));
        }
        let previous = handlers("openHomeWorkRate(");
        if previous.len() != 1 {
            return Err(err("journal_previous_tuple_ambiguous"));
        }
        let previous_re =
            regex::Regex::new(r"^openHomeWorkRate\(([1-9][0-9]*),([1-9][0-9]*),[0-9]+\)$")
                .expect("static regex");
        let m = previous_re
            .captures(&previous[0])
            .ok_or_else(|| err("journal_previous_tuple_conflict"))?;
        if !target.get("membershipKey").is(&m[2]) || target.get("recordKey").is(&m[1]) {
            return Err(err("journal_previous_tuple_conflict"));
        }
        let previous_key = literal_key(&Js::from(&m[1]))?;
        let query = vec![
            ("reqCmd".into(), "SelSglRecord".into()),
            ("course_seq".into(), string(target.get("courseKey"))),
            ("stu_pri_no".into(), string(target.get("studentKey"))),
            ("record_seq".into(), string(&previous_key)),
            ("sel_grp_seq".into(), string(target.get("groupId"))),
            ("tut_pri_no".into(), string(target.get("teacherKey"))),
            ("cm_seq".into(), string(target.get("membershipKey"))),
        ];
        let source = self.read_request("GET", SELECTED, query, vec![])?;
        if source.encode_utf16().count() > 2_000_000
            || source.contains("webUserPass")
            || source.contains("세션이 만료")
        {
            return Err(err("journal_previous_source_rejected"));
        }
        let (record, parsererror) = match xml_record_observation(&source) {
            Ok(v) => v,
            Err(e) if e.message == "independent_occurrence_contract_rejected" => {
                return Err(err("journal_previous_occurrence_conflict"));
            }
            Err(e) => return Err(e),
        };
        if parsererror {
            return Err(err("journal_previous_occurrence_conflict"));
        }
        let values = record.split('!').collect::<Vec<_>>();
        if [
            (2, "courseKey"),
            (4, "studentName"),
            (11, "studentKey"),
            (12, "membershipKey"),
        ]
        .iter()
        .any(|(i, k)| values.get(*i).is_none_or(|v| !target.get(k).is(v)))
            || values.get(10).is_none_or(|v| !previous_key.is(v))
            || values
                .get(1)
                .is_none_or(|v| v.is_empty() || *v >= string(target.get("date")).as_str())
        {
            return Err(err("journal_previous_occurrence_conflict"));
        }
        let assessment = |field: &str| -> Result<Js> {
            let matches = controls
                .iter()
                .filter(|c| c.get("name").is(field))
                .collect::<Vec<_>>();
            if matches.len() != 1 || !matches[0].get("value").string() {
                return Err(err("journal_assessment_not_observed"));
            }
            Ok(matches[0].get("value").clone())
        };
        let mut fields = row.get("fields").clone();
        fields.set("dailyTestAssessment", assessment("h_daily_test")?);
        fields.set("previousHomeworkAssessment", assessment("h_homework_rate")?);
        let mut previous = target.clone();
        previous.set("recordKey", previous_key.clone());
        previous.set("date", values[1].into());
        let effect_targets = obj(&edu_academy_core::workflow::review::FIELDS
            .map(|field| {
                let physical = if field == "previousHomeworkAssessment" {
                    &previous
                } else {
                    target
                };
                let full = crate::native::observation_target(physical);
                Ok((
                    field,
                    edu_academy_core::workflow::canonical(&full)
                        .map_err(journal_core_error)?
                        .into(),
                ))
            })
            .into_iter()
            .collect::<Result<Vec<_>>>()?);
        Ok(obj(&[
            ("target", target.clone()),
            ("effectTargets", effect_targets),
            ("fields", fields),
            ("previousRecordKey", previous_key),
            ("previousDate", values[1].into()),
            ("wireDigest", digest.into()),
        ]))
    }
}
// Private bridge: only the qualified public entrypoint exposes mutation.
// Retain the original read error for a failing pre-dispatch apply; core now also
// preserves its semantic parts losslessly. After-read failures
// intentionally remain the core's unresolved outcome.
struct JournalHttpPort<'a, T: Transport> {
    client: &'a mut Client<T>,
    now: &'a str,
    read_error: Option<Error>,
}
fn journal_core_error(fault: edu_academy_core::js::Fault) -> Error {
    crate::data::error_from_fault(fault)
}
impl<T: Transport> edu_academy_core::workflow::http::HttpPort for JournalHttpPort<'_, T> {
    fn read(&mut self, target: &Js) -> edu_academy_core::js::Result<Js> {
        self.client
            .journal_state(target, self.now)
            .map_err(|error| {
                let fault = crate::data::fault_from_error(error.clone());
                self.read_error = Some(error);
                fault
            })
    }
    fn write(&mut self, action: &Js) -> edu_academy_core::js::Result<Js> {
        let (method, path, selector, parameter) = match action.get("field").code_units().as_str() {
            "progress" => ("GET", MAIN, "udtPrg", "prg_txt"),
            "homework" => ("GET", MAIN, "udtHw", "hw_txt"),
            "lessonMemo" => ("GET", MAIN, "udtMemo", "memo_txt"),
            "attendance" => ("GET", MAIN, "udtAttn", "attn_yn"),
            "dailyTestAssessment" => (
                "POST",
                "/servlet/controller.cct.common.CourseCommonServlet",
                "SetDailyTest",
                "daily_test_no",
            ),
            "previousHomeworkAssessment" => (
                "POST",
                "/servlet/controller.cct.common.CourseCommonServlet",
                "SetHomeWorkRate",
                "homework_rate_no",
            ),
            _ => {
                return Err(edu_academy_core::js::Fault::error(
                    "journal_http_action_rejected",
                ));
            }
        };
        let target = action.get("target");
        let (query, body) = if method == "GET" {
            (
                vec![
                    ("reqCmd".into(), selector.into()),
                    (parameter.into(), string(action.get("after"))),
                    ("course_seq".into(), string(target.get("courseKey"))),
                    ("stu_pri_no".into(), string(target.get("studentKey"))),
                    ("record_seq".into(), string(action.get("effectRecordKey"))),
                    ("cm_seq".into(), string(target.get("membershipKey"))),
                    ("dummy".into(), self.now.into()),
                ],
                vec![],
            )
        } else {
            (
                vec![],
                vec![
                    ("reqCmd".into(), selector.into()),
                    ("cm_seq".into(), string(target.get("membershipKey"))),
                    ("record_seq".into(), string(action.get("effectRecordKey"))),
                    ("tutor_pri_no".into(), string(target.get("teacherKey"))),
                    (parameter.into(), string(action.get("after"))),
                ],
            )
        };
        self.client
            .request(method, path, query, body, false, None)
            .map(|r| {
                obj(&[
                    ("httpStatus", Js::Num(f64::from(r.status))),
                    ("bodySha256", hash(&r.bytes).into()),
                ])
            })
            .map_err(|error| edu_academy_core::js::Fault::error(error.message))
    }
}
#[cfg(test)]
mod owner_http_write_tests {
    use super::*;
    use edu_academy_core::{js::Fault, workflow::http::HttpPort};

    const NOW: &str = "2040-01-04T09:00:00.000Z";
    const COMMON: &str = "/servlet/controller.cct.common.CourseCommonServlet";
    #[derive(Default)]
    struct Fictional {
        requests: Vec<Request>,
        fail: bool,
        status: Option<u16>,
        redirect: Option<&'static str>,
        cookie: Option<&'static str>,
    }
    impl Transport for Fictional {
        fn dispatch(&mut self, request: &Request) -> Result<Response> {
            self.requests.push(request.clone());
            if self.fail {
                return Err(Error::new("FictionalFailure", "fictional_lost_response"));
            }
            let mut headers = vec![];
            if let Some(location) = self.redirect {
                headers.push(("Location".into(), location.into()));
            }
            if let Some(cookie) = self.cookie {
                headers.push(("Set-Cookie".into(), cookie.into()));
            }
            Ok(Response {
                status: self.status.unwrap_or(200),
                headers,
                bytes: b"response body is not persistence evidence".to_vec(),
                url: format!("{ORIGIN}{}", request.path),
                stalled: false,
            })
        }
    }
    fn client() -> Client<Fictional> {
        let mut client = Client::new(
            obj(&[
                ("teacherLabel", "Fictional Port Teacher".into()),
                ("teacherKey", "99001".into()),
                ("collegeKey", "88001".into()),
            ]),
            Fictional::default(),
        )
        .unwrap();
        client.authenticated = true;
        client
    }
    fn action(field: &str, after: &str) -> Js {
        obj(&[
            ("field", field.into()),
            ("after", after.into()),
            (
                "effectRecordKey",
                if field == "previousHomeworkAssessment" {
                    "12"
                } else {
                    "13"
                }
                .into(),
            ),
            (
                "target",
                obj(&[
                    ("teacherKey", "99001".into()),
                    ("studentKey", "77001".into()),
                    ("courseKey", "5".into()),
                    ("recordKey", "13".into()),
                    ("membershipKey", "8".into()),
                ]),
            ),
        ])
    }
    fn write(client: &mut Client<Fictional>, action: &Js) -> edu_academy_core::js::Result<Js> {
        // Intentionally tests ONLY the private wire seam, never a qualified apply.
        JournalHttpPort {
            client,
            now: NOW,
            read_error: None,
        }
        .write(action)
    }
    fn encoded(pairs: &[(String, String)]) -> String {
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs)
            .finish()
    }
    #[test]
    fn six_private_field_bindings_and_literal_form_encoding() {
        for (field, after, method, path, expected) in [
            (
                "progress",
                "A &+=?#/😀\n가",
                "GET",
                MAIN,
                "reqCmd=udtPrg&prg_txt=A+%26%2B%3D%3F%23%2F%F0%9F%98%80%0A%EA%B0%80&course_seq=5&stu_pri_no=77001&record_seq=13&cm_seq=8&dummy=2040-01-04T09%3A00%3A00.000Z",
            ),
            (
                "homework",
                "After",
                "GET",
                MAIN,
                "reqCmd=udtHw&hw_txt=After&course_seq=5&stu_pri_no=77001&record_seq=13&cm_seq=8&dummy=2040-01-04T09%3A00%3A00.000Z",
            ),
            (
                "lessonMemo",
                "After",
                "GET",
                MAIN,
                "reqCmd=udtMemo&memo_txt=After&course_seq=5&stu_pri_no=77001&record_seq=13&cm_seq=8&dummy=2040-01-04T09%3A00%3A00.000Z",
            ),
            (
                "attendance",
                " ",
                "GET",
                MAIN,
                "reqCmd=udtAttn&attn_yn=+&course_seq=5&stu_pri_no=77001&record_seq=13&cm_seq=8&dummy=2040-01-04T09%3A00%3A00.000Z",
            ),
            (
                "dailyTestAssessment",
                "10",
                "POST",
                COMMON,
                "reqCmd=SetDailyTest&cm_seq=8&record_seq=13&tutor_pri_no=99001&daily_test_no=10",
            ),
            (
                "previousHomeworkAssessment",
                "-1",
                "POST",
                COMMON,
                "reqCmd=SetHomeWorkRate&cm_seq=8&record_seq=12&tutor_pri_no=99001&homework_rate_no=-1",
            ),
        ] {
            let mut c = client();
            let receipt = write(&mut c, &action(field, after)).unwrap();
            assert_eq!(receipt.get("httpStatus"), &Js::Num(200.0));
            assert!(
                receipt
                    .get("bodySha256")
                    .is(&hash(b"response body is not persistence evidence"))
            );
            assert_eq!(c.transport.requests.len(), 1);
            let r = &c.transport.requests[0];
            assert_eq!(r.method, method);
            assert_eq!(r.path, path);
            assert_eq!(r.timeout, Duration::from_secs(20));
            assert_eq!(
                encoded(if method == "GET" { &r.query } else { &r.body }),
                expected
            );
            assert!(if method == "GET" {
                r.body.is_empty()
            } else {
                r.query.is_empty()
            });
            assert!(r.headers.contains(&("Origin".into(), ORIGIN.into())));
            assert!(
                r.headers
                    .contains(&("Referer".into(), format!("{ORIGIN}/c_login.jsp")))
            );
            assert_eq!(
                r.headers
                    .iter()
                    .filter(|(k, _)| k == "Content-Type")
                    .count(),
                usize::from(method == "POST")
            );
            if method == "POST" {
                assert!(r.headers.contains(&(
                    "Content-Type".into(),
                    "application/x-www-form-urlencoded; charset=UTF-8".into()
                )));
            }
            assert!(admit_operation(ORIGIN, path, method, &r.query, &r.body).is_none());
        }
    }
    #[test]
    fn private_value_uses_usvstring_and_keeps_reserved_data_in_one_parameter() {
        let mut c = client();
        let mut a = action("progress", "");
        a.set("after", Js::Str(vec![0xd800, 0x26, 0xdc00]));
        write(&mut c, &a).unwrap();
        let r = &c.transport.requests[0];
        assert_eq!(r.query.len(), 7);
        assert_eq!(r.query[1], ("prg_txt".into(), "�&�".into()));
        assert_eq!(
            encoded(&r.query),
            "reqCmd=udtPrg&prg_txt=%EF%BF%BD%26%EF%BF%BD&course_seq=5&stu_pri_no=77001&record_seq=13&cm_seq=8&dummy=2040-01-04T09%3A00%3A00.000Z"
        );
    }
    #[test]
    fn private_request_failures_dispatch_once_and_never_follow_redirects() {
        for variant in [
            "transport",
            "http",
            "redirect",
            "foreign_redirect",
            "foreign_cookie",
            "unknown_field",
        ] {
            let mut c = client();
            let mut a = action("progress", "After");
            let expected = match variant {
                "transport" => {
                    c.transport.fail = true;
                    "academy_transport_failed"
                }
                "http" => {
                    c.transport.status = Some(503);
                    "source_http_failed"
                }
                "redirect" => {
                    c.transport.status = Some(302);
                    c.transport.redirect = Some("/fictional-next");
                    "source_redirect_rejected"
                }
                "foreign_redirect" => {
                    c.transport.status = Some(302);
                    c.transport.redirect = Some("https://foreign.invalid/");
                    "foreign_source_redirect_rejected"
                }
                "foreign_cookie" => {
                    c.transport.cookie =
                        Some("JSESSIONID=fictional; Domain=foreign.invalid; Path=/");
                    "foreign_session_cookie_rejected"
                }
                _ => {
                    a.set("field", "persistentMemo".into());
                    "journal_http_action_rejected"
                }
            };
            assert_eq!(write(&mut c, &a).unwrap_err().message, expected);
            assert_eq!(
                c.transport.requests.len(),
                usize::from(variant != "unknown_field")
            );
        }
    }
    #[test]
    fn private_request_reuses_cookie_scope_and_close_clears_it() {
        let mut c = client();
        c.transport.cookie = Some("JSESSIONID=fictional-port; Path=/servlet/; Secure");
        c.request("GET", "/c_login.jsp", vec![], vec![], false, None)
            .unwrap();
        write(&mut c, &action("progress", "After")).unwrap();
        assert!(
            c.transport.requests[1]
                .headers
                .contains(&("Cookie".into(), "JSESSIONID=fictional-port".into()))
        );
        c.request("GET", "/outside", vec![], vec![], false, None)
            .unwrap();
        assert!(
            !c.transport.requests[2]
                .headers
                .iter()
                .any(|(k, _)| k == "Cookie")
        );
        c.close();
        assert!(c.cookies.is_empty());
        assert!(!c.authenticated);
    }
    #[test]
    fn core_fault_conversion_preserves_name_message_and_issues() {
        let e = journal_core_error(Fault {
            name: "TypeError".into(),
            message: "fictional_approval_failed".into(),
            issues: Some(vec![obj(&[("code", "fictional_issue".into())])]),
            ..Fault::error("")
        });
        assert_eq!(e.name, "TypeError");
        assert_eq!(e.message, "fictional_approval_failed");
        assert_eq!(
            e.properties,
            obj(&[(
                "issues",
                Js::Arr(vec![obj(&[("code", "fictional_issue".into())])])
            )])
        );
    }
}
fn login_encode(literal: &str) -> Result<String> {
    use std::io::Read;
    let mut random =
        std::fs::File::open("/dev/urandom").map_err(|_| err("login_entropy_unavailable"))?;
    let mut suffix = String::new();
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
    while suffix.len() < 30 {
        let mut b = [0u8; 1];
        random
            .read_exact(&mut b)
            .map_err(|_| err("login_entropy_unavailable"))?;
        if b[0] < 252 {
            suffix.push(alphabet[(b[0] % 36) as usize] as char)
        }
    }
    Ok(base64(format!("{literal}  {suffix}").as_bytes(), false))
}
fn decode_form(s: &str) -> String {
    let mut bytes = vec![];
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'+' {
            bytes.push(b' ')
        } else if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(n) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                bytes.push(n);
                i += 3;
                continue;
            } else {
                bytes.push(b[i])
            }
        } else {
            bytes.push(b[i])
        }
        i += 1
    }
    String::from_utf8_lossy(&bytes).into()
}
pub fn parse_form(s: &str) -> Vec<(String, String)> {
    s.split('&')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (k, v) = s.split_once('=').unwrap_or((s, ""));
            (decode_form(k), decode_form(v))
        })
        .collect()
}
fn canonical(v: &Js) -> String {
    edu_academy_core::js::encode(v, edu_academy_core::js::Family::Learning, &mut vec![])
        .expect("JSON data")
        .expect("JSON data")
}
pub fn wire_digest(wire: &Js) -> Result<String> {
    let pop = wire.get("popupEvents").array();
    if pop.len() != 1 {
        return Err(err("journal_popup_source_ambiguous"));
    }
    let s = string(&pop[0]);
    let re = regex::Regex::new(r#"(?:^|\n)[\t ]*var g_param\s*=\s*""\s*;"#).unwrap();
    let matches = re.find_iter(&s).collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(err("journal_popup_source_ambiguous"));
    }
    let projection = obj(&[
        ("functions", wire.get("functions").clone()),
        (
            "popupEvents",
            Js::Arr(vec![s[matches[0].start()..].trim_start().into()]),
        ),
        ("assessmentOptions", wire.get("assessmentOptions").clone()),
    ]);
    Ok(hash(canonical(&projection).as_bytes()))
}
fn day_record_row(link: scraper::ElementRef<'_>) -> Result<scraper::ElementRef<'_>> {
    let mut row = dom::closest(link, "tr");
    while let Some(r) = row {
        let controls = dom::select(r, "[onchange],[onfocusout],[onclick]");
        if ["onAttn", "onFoPrg", "onFoHw", "onFoMm"].iter().all(|h| {
            controls
                .iter()
                .filter(|e| {
                    ["onchange", "onfocusout", "onclick"]
                        .iter()
                        .any(|k| e.value().attr(k).unwrap_or("").contains(&format!("{h}(")))
                })
                .count()
                == 1
        }) {
            break;
        }
        row = dom::closest(r, "tr");
    }
    row.ok_or_else(|| err("requested_row_structure_unresolved"))
}
pub(crate) fn validate_headers(row: scraper::ElementRef<'_>) -> Result<()> {
    let table =
        dom::closest(row, "table").ok_or_else(|| err("day_record_column_contract_rejected"))?;
    validate_table_headers(table)?;
    let headers = dom::select(table, "thead > tr");
    for r in headers.iter().skip(2).copied().chain(std::iter::once(row)) {
        let cells = r
            .children()
            .filter_map(scraper::ElementRef::wrap)
            .filter(|c| matches!(c.value().name(), "td" | "th"))
            .collect::<Vec<_>>();
        if cells.len() != 13
            || cells.iter().any(|c| {
                c.value().name() != "td"
                    || c.value().attr("colspan").unwrap_or("1") != "1"
                    || c.value().attr("rowspan").unwrap_or("1") != "1"
            })
        {
            return Err(err("day_record_column_contract_rejected"));
        }
    }
    Ok(())
}
pub(crate) fn capture_headers(table: scraper::ElementRef<'_>) -> Result<()> {
    table_headers(table, true)
}
fn validate_table_headers(table: scraper::ElementRef<'_>) -> Result<()> {
    table_headers(table, false)
}
fn table_headers(table: scraper::ElementRef<'_>, capture: bool) -> Result<()> {
    let headers = if capture {
        dom::select(table, "tr")
            .into_iter()
            .filter(|r| dom::closest(*r, "table").is_some_and(|t| t.id() == table.id()))
            .collect()
    } else {
        dom::select(table, "thead > tr")
    };
    let expected = [
        vec![
            ("이름", 1, 2),
            ("과정", 1, 2),
            ("지속사항", 1, 2),
            ("지난진도", 4, 1),
            ("일괄 발송 금일진도", 6, 1),
        ],
        vec![
            ("지난수업일", 1, 1),
            ("진도", 1, 1),
            ("숙제", 1, 1),
            ("메모", 1, 1),
            ("", 1, 1),
            ("금일출석", 1, 1),
            ("일일테스트", 1, 1),
            ("진도", 1, 1),
            ("숙제", 1, 1),
            ("메모", 1, 1),
        ],
    ];
    if headers.len() < 2 {
        return Err(err("day_record_column_contract_rejected"));
    }
    for (i, header) in headers.iter().take(2).enumerate() {
        let cells = header
            .children()
            .filter_map(scraper::ElementRef::wrap)
            .filter(|c| matches!(c.value().name(), "th" | "td"))
            .collect::<Vec<_>>();
        if cells.len() != expected[i].len() {
            return Err(err("day_record_column_contract_rejected"));
        }
        for (c, (label, colspan, rowspan)) in cells.iter().zip(&expected[i]) {
            let text = dom::content(*c)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if (if capture {
                text.replace(' ', "") != label.replace(' ', "")
            } else {
                text != *label
            }) || c
                .value()
                .attr("colspan")
                .unwrap_or("1")
                .parse::<u32>()
                .unwrap_or(1)
                != *colspan
                || c.value()
                    .attr("rowspan")
                    .unwrap_or("1")
                    .parse::<u32>()
                    .unwrap_or(1)
                    != *rowspan
            {
                return Err(err("day_record_column_contract_rejected"));
            }
        }
    }
    Ok(())
}
fn signatures() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "onAttn",
            vec!["i", "course_seq", "stu_pri_no", "record_seq", "cm_seq"],
        ),
        (
            "onFoPrg",
            vec!["i", "course_seq", "stu_pri_no", "record_seq", "cm_seq"],
        ),
        (
            "onFoHw",
            vec!["i", "course_seq", "stu_pri_no", "record_seq", "cm_seq"],
        ),
        (
            "onFoMm",
            vec!["i", "course_seq", "stu_pri_no", "record_seq", "cm_seq"],
        ),
        ("onFoStuMemo", vec!["i", "stu_pri_no", "course_seq"]),
        ("onOpenDaysMain", vec!["student_pri_no"]),
        (
            "onDailyReport",
            vec!["report_seq", "stu_pri_no", "record_seq", "cm_seq"],
        ),
        ("setDailyTest", vec!["record_seq", "cm_seq", "count"]),
        ("openHomeWorkRate", vec!["record_seq", "cm_seq", "count"]),
    ]
}
pub(crate) fn bind_row(doc: &Document, row: scraper::ElementRef<'_>) -> Result<(Js, Js)> {
    bind_row_mode(doc, row, false)
}
pub(crate) fn bind_capture_row(doc: &Document, row: scraper::ElementRef<'_>) -> Result<(Js, Js)> {
    bind_row_mode(doc, row, true)
}
fn bind_row_mode(doc: &Document, row: scraper::ElementRef<'_>, capture: bool) -> Result<(Js, Js)> {
    let cells = row
        .children()
        .filter_map(scraper::ElementRef::wrap)
        .filter(|c| c.value().name() == "td")
        .collect::<Vec<_>>();
    let columns = [
        vec!["onOpenDaysMain"],
        vec![],
        vec!["onFoStuMemo"],
        vec![],
        vec![],
        vec!["openHomeWorkRate"],
        vec![],
        vec!["onDailyReport"],
        vec!["onAttn"],
        vec!["setDailyTest"],
        vec!["onFoPrg"],
        vec!["onFoHw"],
        vec!["onFoMm"],
    ];
    let definitions = signatures();
    let mut actual = HashMap::<String, Vec<Vec<String>>>::new();
    let re=regex::Regex::new(r"function\s+(onAttn|onFoPrg|onFoHw|onFoMm|onFoStuMemo|onOpenDaysMain|onDailyReport|setDailyTest|openHomeWorkRate)\s*\(([^)]*)\)").unwrap();
    for s in doc.select("script") {
        for c in re.captures_iter(&dom::content(s)) {
            actual
                .entry(c[1].into())
                .or_default()
                .push(c[2].split(',').map(|s| s.trim().into()).collect());
        }
    }
    let call_re = regex::Regex::new(
        r"^\s*(?:javascript:[ \t]*|return[ \t]+)?([A-Za-z_$][\w$]*)\s*\(([\s\S]*)\)\s*;?\s*$",
    )
    .unwrap();
    let lit_re = regex::Regex::new(if capture {
        r##"^\s*(?:([0-9]+)|'([0-9]*)'|"([0-9]*)")\s*$"##
    } else {
        r##"^\s*(?:([0-9]+)|'([0-9]+)'|"([0-9]+)")\s*$"##
    })
    .unwrap();
    let mut calls = HashMap::<String, Js>::new();
    let mut fields = obj(&[]);
    for e in dom::select(row, "[onchange],[onfocusout],[onclick]") {
        for event in ["onchange", "onfocusout", "onclick"] {
            let Some(source) = e.value().attr(event).filter(|s| !s.is_empty()) else {
                continue;
            };
            if !definitions.iter().any(|(h, _)| {
                source.contains(&format!("{h}(")) || source.contains(&format!("{h} ("))
            }) {
                continue;
            }
            let c = call_re
                .captures(source)
                .ok_or_else(|| err("requested_row_binding_unresolved"))?;
            let handler = &c[1];
            let Some((_, signature)) = definitions.iter().find(|(h, _)| *h == handler) else {
                return Err(err("requested_row_binding_unresolved"));
            };
            let column = cells
                .iter()
                .position(|cell| cell.id() == e.id() || e.ancestors().any(|a| a.id() == cell.id()))
                .ok_or_else(|| err("requested_row_binding_unresolved"))?;
            if columns.get(column).is_none_or(|c| !c.contains(&handler)) {
                return Err(err("requested_row_binding_unresolved"));
            }
            let expected_event = if handler == "onAttn" {
                "onchange"
            } else if handler.starts_with("onFo") {
                "onfocusout"
            } else {
                "onclick"
            };
            let tokens = c[2].split(',').collect::<Vec<_>>();
            if source.encode_utf16().count() > 4096
                || event != expected_event
                || calls.contains_key(handler)
                || tokens.len() != signature.len()
                || actual.get(handler).is_none_or(|a| {
                    a.len() != 1
                        || a[0]
                            != signature
                                .iter()
                                .map(|s| (*s).into())
                                .collect::<Vec<String>>()
                })
            {
                return Err(err("requested_row_binding_unresolved"));
            }
            let mut binding = obj(&[]);
            for (token, key) in tokens.iter().zip(signature) {
                let literal = lit_re
                    .captures(token)
                    .ok_or_else(|| err("requested_row_binding_unresolved"))?;
                let value = literal
                    .get(1)
                    .or(literal.get(2))
                    .or(literal.get(3))
                    .unwrap()
                    .as_str();
                if value.len() > 128
                    || !capture
                        && literal.get(1).is_some()
                        && (value.len() > 16
                            || (value.len() > 1 && value.starts_with('0'))
                            || (value.len() == 16 && value > "9007199254740991"))
                {
                    return Err(err("requested_row_binding_unresolved"));
                }
                binding.set(key, value.into());
            }
            calls.insert(handler.into(), binding);
            let field = match handler {
                "onFoPrg" => Some("progress"),
                "onFoHw" => Some("homework"),
                "onFoMm" => Some("lessonMemo"),
                "onFoStuMemo" => Some("persistentMemo"),
                "onAttn" => Some("attendance"),
                _ => None,
            };
            if let Some(field) = field {
                if !matches!(e.value().name(), "input" | "select" | "textarea") {
                    return Err(err("day_record_field_contract_rejected"));
                }
                fields.set(field, dom::value(e).into());
            }
        }
    }
    let canonical = calls
        .get("onAttn")
        .ok_or_else(|| err("requested_row_binding_unresolved"))?;
    for h in ["onAttn", "onFoPrg", "onFoHw", "onFoMm"] {
        let b = calls
            .get(h)
            .ok_or_else(|| err("requested_row_binding_unresolved"))?;
        for key in ["i", "course_seq", "stu_pri_no", "record_seq", "cm_seq"] {
            let s = string(b.get(key));
            if b.get(key) != canonical.get(key)
                || (!capture && key != "i" && !s.bytes().any(|c| matches!(c, b'1'..=b'9')))
            {
                return Err(err("requested_row_binding_unresolved"));
            }
        }
    }
    for (h, keys) in [
        (
            "onFoStuMemo",
            vec![
                ("i", "i"),
                ("course_seq", "course_seq"),
                ("stu_pri_no", "stu_pri_no"),
            ],
        ),
        ("onOpenDaysMain", vec![("student_pri_no", "stu_pri_no")]),
        (
            "onDailyReport",
            vec![
                ("stu_pri_no", "stu_pri_no"),
                ("record_seq", "record_seq"),
                ("cm_seq", "cm_seq"),
            ],
        ),
        (
            "setDailyTest",
            vec![("record_seq", "record_seq"), ("cm_seq", "cm_seq")],
        ),
    ] {
        if let Some(b) = calls.get(h)
            && keys.iter().any(|(a, bkey)| b.get(a) != canonical.get(bkey))
        {
            return Err(err("requested_row_binding_unresolved"));
        }
    }
    if !capture
        && let Some(p) = calls.get("openHomeWorkRate")
        && ["record_seq", "cm_seq"]
            .iter()
            .any(|k| !string(p.get(k)).bytes().any(|c| matches!(c, b'1'..=b'9')))
    {
        return Err(err("requested_row_binding_unresolved"));
    }
    let mut binding = canonical.clone();
    if capture && let Some(previous) = calls.get("openHomeWorkRate") {
        if previous.get("cm_seq") != canonical.get("cm_seq") {
            return Err(err("requested_row_binding_unresolved"));
        }
        binding.set("previous_record_key", previous.get("record_seq").clone());
    }
    Ok((fields, binding))
}
// XML 1.0 names and attribute grammar, checked before record extraction.
// Attribute values are quoted; `>` inside a quoted value is ordinary data.
fn xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}
fn xml_char(c: char) -> bool {
    matches!(c as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}
fn xml_name_start(c: char) -> bool {
    matches!(c, ':' | '_' | 'A'..='Z' | 'a'..='z')
        || matches!(c as u32,
        0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0x2FF | 0x370..=0x37D |
        0x37F..=0x1FFF | 0x200C..=0x200D | 0x2070..=0x218F | 0x2C00..=0x2FEF |
        0x3001..=0xD7FF | 0xF900..=0xFDCF | 0xFDF0..=0xFFFD | 0x10000..=0xEFFFF)
}
fn xml_name(s: &str, at: &mut usize) -> bool {
    let start = *at;
    for c in s[start..].chars() {
        if if *at == start {
            xml_name_start(c)
        } else {
            xml_name_start(c)
                || matches!(c, '-' | '.' | '0'..='9' | '\u{B7}')
                || matches!(c as u32, 0x300..=0x36F | 0x203F..=0x2040)
        } {
            *at += c.len_utf8();
        } else {
            break;
        }
    }
    *at != start
}
struct XmlTag<'a> {
    name: &'a str,
    close: bool,
    empty: bool,
    end: usize,
    attributes: Vec<(&'a str, String)>,
}
fn xml_tag(s: &str, start: usize) -> std::result::Result<XmlTag<'_>, (usize, String)> {
    let bytes = s.as_bytes();
    let next = |at: usize| {
        s.get(at..)
            .and_then(|s| s.chars().next())
            .map_or(s.len(), |c| at + c.len_utf8())
    };
    let mut at = start + 1;
    let close = bytes.get(at) == Some(&b'/');
    if close {
        at += 1;
    }
    let name_start = at;
    if !xml_name(s, &mut at) {
        return Err((at, "disallowed character in tag name.".into()));
    }
    let name = &s[name_start..at];
    let mut attrs = std::collections::HashSet::new();
    let mut attributes = vec![];
    loop {
        let before_space = at;
        while bytes
            .get(at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            at += 1;
        }
        if bytes.get(at) == Some(&b'>') {
            return Ok(XmlTag {
                name,
                close,
                empty: false,
                end: at + 1,
                attributes,
            });
        }
        if !close && bytes.get(at..at + 2) == Some(b"/>") {
            return Ok(XmlTag {
                name,
                close,
                empty: true,
                end: at + 2,
                attributes,
            });
        }
        if at >= bytes.len() {
            return Err((at, "unclosed tag.".into()));
        }
        if close {
            return Err((at, "disallowed character in closing tag.".into()));
        }
        if at == before_space {
            return Err((next(at), "no whitespace between attributes.".into()));
        }
        let attr_start = at;
        if !xml_name(s, &mut at) {
            return Err((at, "disallowed character in attribute name.".into()));
        }
        let attr = &s[attr_start..at];
        while bytes
            .get(at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            at += 1;
        }
        if bytes.get(at) != Some(&b'=') {
            return Err((next(at), "attribute without value.".into()));
        }
        at += 1;
        while bytes
            .get(at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            at += 1;
        }
        let Some(&quote @ (b'\'' | b'"')) = bytes.get(at) else {
            return Err((next(at), "unquoted attribute value.".into()));
        };
        at += 1;
        let value_start = at;
        while let Some(&b) = bytes.get(at) {
            if b == quote {
                break;
            }
            if b == b'<' {
                return Err((next(at), "disallowed character in attribute value.".into()));
            }
            // Advance by a scalar, keeping every error position a UTF-8 boundary.
            at += s[at..].chars().next().expect("present byte").len_utf8();
        }
        if at == bytes.len() {
            return Err((at, "unclosed attribute value.".into()));
        }
        let value =
            xml_entities(&s[value_start..at]).map_err(|_| (at, "undefined entity.".into()))?;
        attributes.push((attr, value));
        at += 1;
        if !attrs.insert(attr) {
            return Err((next(at), format!("duplicate attribute: {attr}.")));
        }
    }
}
fn xml_qname(name: &str) -> Option<(&str, &str)> {
    match name.split_once(':') {
        Some((prefix, local))
            if !prefix.is_empty()
                && !local.is_empty()
                && !local.contains(':')
                && local.chars().next().is_some_and(xml_name_start) =>
        {
            Some((prefix, local))
        }
        None => Some(("", name)),
        _ => None,
    }
}
fn xml_namespaces(
    tag: &XmlTag<'_>,
    parent: Option<&HashMap<String, String>>,
) -> std::result::Result<HashMap<String, String>, String> {
    const XML: &str = "http://www.w3.org/XML/1998/namespace";
    const XMLNS: &str = "http://www.w3.org/2000/xmlns/";
    let mut scope = parent
        .cloned()
        .unwrap_or_else(|| HashMap::from([("xml".into(), XML.into())]));
    for (name, value) in &tag.attributes {
        let (prefix, local) = xml_qname(name).ok_or("malformed qualified attribute name.")?;
        let binding = if *name == "xmlns" {
            Some("")
        } else if prefix == "xmlns" {
            Some(local)
        } else {
            None
        };
        if let Some(prefix) = binding {
            if prefix == "xmlns"
                || value == XMLNS
                || (prefix == "xml") != (value == XML)
                || !prefix.is_empty() && value.is_empty()
            {
                return Err("invalid namespace binding.".into());
            }
            scope.insert(prefix.into(), value.clone());
        }
    }
    let (prefix, _) = xml_qname(tag.name).ok_or("malformed qualified tag name.")?;
    if !prefix.is_empty() && !scope.contains_key(prefix) {
        return Err("unbound namespace prefix.".into());
    }
    let mut expanded = std::collections::HashSet::new();
    for (name, _) in &tag.attributes {
        let (prefix, local) = xml_qname(name).ok_or("malformed qualified attribute name.")?;
        if *name == "xmlns" || prefix == "xmlns" {
            continue;
        }
        let uri = if prefix.is_empty() {
            ""
        } else {
            scope
                .get(prefix)
                .ok_or("unbound namespace prefix.")?
                .as_str()
        };
        if !expanded.insert((uri, local)) {
            return Err(format!("duplicate attribute: {name}."));
        }
    }
    Ok(scope)
}
/// Strict recorded XML token parser; inert text only. Reports the original XML error shape.
pub(crate) fn xml_record(s: &str) -> Result<String> {
    xml_record_observation(s).map(|(record, _)| record)
}
fn xml_record_observation(s: &str) -> Result<(String, bool)> {
    let mut parsererror = false;
    let mut stack: Vec<String> = vec![];
    let mut namespaces = vec![];
    let mut records = vec![];
    let mut record_count = 0;
    let mut record = None::<String>;
    let mut i = 0;
    let mut roots = 0;
    let b = s.as_bytes();
    let syntax = |pos: usize, message: String| {
        // Match Saxes: XML line endings normalize CR/CRLF, and columns count
        // Unicode scalar values (an astral character occupies one column).
        let prefix = s[..pos].replace("\r\n", "\n").replace('\r', "\n");
        let line = prefix.bytes().filter(|c| *c == b'\n').count() + 1;
        let col = prefix.rsplit('\n').next().unwrap_or("").chars().count();
        let mut e = Error::new(
            "SyntaxError",
            format!("about:blank:{line}:{col}: {message}"),
        );
        e.properties.set("code", Js::Num(12.));
        e
    };
    if let Some((at, _)) = s.char_indices().find(|(_, c)| !xml_char(*c)) {
        return Err(syntax(at, "disallowed character.".into()));
    }
    while i < b.len() {
        if b[i] != b'<' {
            let end = s[i..].find('<').map_or(s.len(), |n| i + n);
            let text = &s[i..end];
            if stack.is_empty() {
                // Document-level Misc permits raw XML whitespace, not character
                // references that happen to decode to whitespace. Match the TS
                // sourceDomParser before applying element text/entity semantics.
                if !text.chars().all(xml_space) {
                    // Saxes reports after scanning to '&', '<', or end-of-input.
                    let pos = text.find('&').map_or_else(
                        || if end < s.len() { end + 1 } else { end },
                        |at| i + at + 1,
                    );
                    return Err(syntax(pos, "text data outside of root node.".into()));
                }
                i = end;
                continue;
            }
            if text.contains("]]>") {
                return Err(syntax(
                    end,
                    "the string ]]> is disallowed in char data.".into(),
                ));
            }
            let text = xml_entities(text).map_err(|_| syntax(end, "undefined entity.".into()))?;
            if let Some(r) = &mut record {
                r.push_str(&text);
            }
            i = end;
            continue;
        }
        if s[i..].starts_with("<!--") {
            let end = s[i + 4..]
                .find("-->")
                .map(|n| i + 4 + n)
                .ok_or_else(|| syntax(s.len(), "unclosed comment.".into()))?;
            if s[i + 4..end].contains("--") || s[i + 4..end].ends_with('-') {
                return Err(syntax(end, "malformed comment.".into()));
            }
            i = end + 3;
            continue;
        }
        if s[i..].starts_with("<![CDATA[") {
            if stack.is_empty() {
                return Err(syntax(i, "text data outside of root node.".into()));
            }
            let end = s[i + 9..]
                .find("]]>")
                .map(|n| i + 9 + n)
                .ok_or_else(|| syntax(s.len(), "unclosed cdata.".into()))?;
            if let Some(r) = &mut record {
                r.push_str(&s[i + 9..end])
            }
            i = end + 3;
            continue;
        }
        if s[i..].starts_with("<?") {
            i = s[i + 2..]
                .find("?>")
                .map(|n| i + 2 + n + 2)
                .ok_or_else(|| syntax(s.len(), "unclosed processing instruction.".into()))?;
            continue;
        }
        let tag = xml_tag(s, i).map_err(|(at, message)| syntax(at.min(s.len()), message))?;
        let XmlTag {
            name,
            close,
            empty,
            end: next,
            ..
        } = tag;
        let end = next - 1;
        if close {
            if stack.last().map(String::as_str) != Some(name) {
                return Err(syntax(end + 1, "unexpected close tag.".into()));
            }
            stack.pop();
            namespaces.pop();
            if name == "record"
                && let Some(r) = record.take()
            {
                records.push(r);
            }
        } else {
            let namespace = xml_namespaces(&tag, namespaces.last())
                .map_err(|message| syntax(end + 1, message))?;
            if name == "parsererror" {
                parsererror = true;
            }
            if stack.is_empty() {
                roots += 1;
                if roots > 1 {
                    return Err(syntax(
                        end + 1,
                        "documents may contain only one root.".into(),
                    ));
                }
            }
            if name == "record" {
                record_count += 1;
                record = Some(String::new())
            }
            if !empty {
                namespaces.push(namespace);
                stack.push(name.into())
            } else if name == "record" {
                records.push(String::new());
                record = None;
            }
        }
        i = end + 1;
    }
    if let Some(name) = stack.last() {
        return Err(syntax(s.len(), format!("unclosed tag: {name}")));
    }
    if record_count != 1 || records.len() != 1 {
        return Err(err("independent_occurrence_contract_rejected"));
    }
    Ok((records.remove(0), parsererror))
}
fn xml_entities(s: &str) -> Result<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let end = rest[i..].find(';').ok_or_else(|| err("xml_entity"))? + i;
        let key = &rest[i + 1..end];
        let v = match key {
            "amp" => "&".into(),
            "lt" => "<".into(),
            "gt" => ">".into(),
            "quot" => "\"".into(),
            "apos" => "'".into(),
            _ => {
                let n = if let Some(n) = key.strip_prefix("#x") {
                    u32::from_str_radix(n, 16).ok()
                } else {
                    key.strip_prefix('#').and_then(|n| n.parse().ok())
                };
                n.and_then(char::from_u32)
                    .filter(|c| xml_char(*c))
                    .ok_or_else(|| err("xml_entity"))?
                    .to_string()
            }
        };
        out.push_str(&v);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
/// Supplied recorded exchanges. No sockets, discovery, retry or fallback path exists.
pub struct RecordedTransport {
    pub records: Vec<Js>,
    pub calls: Vec<Js>,
    pub cookie_proof: Vec<Js>,
    pub events: Vec<Js>,
    pub trace: Option<std::fs::File>,
    index: usize,
    pub dispatches: usize,
}
impl RecordedTransport {
    pub fn new(records: Vec<Js>, trace: Option<std::fs::File>) -> Self {
        Self {
            records,
            calls: vec![],
            cookie_proof: vec![],
            events: vec![],
            trace,
            index: 0,
            dispatches: 0,
        }
    }
    fn event(&mut self, event: Js) -> Result<()> {
        use std::io::Write;
        if let Some(f) = &mut self.trace {
            f.write_all(format!("{}\n", json(&event)).as_bytes())
                .map_err(|_| err("effect_trace_write_failed"))?;
            f.flush().map_err(|_| err("effect_trace_write_failed"))?;
        }
        self.events.push(event);
        Ok(())
    }
}
impl Transport for RecordedTransport {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        let record = self
            .records
            .get(self.index)
            .cloned()
            .ok_or_else(|| err("recorded_transport_exhausted"))?;
        self.index += 1;
        let selector = r
            .query
            .iter()
            .find(|(k, _)| k == "reqCmd")
            .or_else(|| r.body.iter().find(|(k, _)| k == "p_process"))
            .map(|(_, v)| v.clone());
        if !record.get("path").is(&r.path)
            || !record.get("method").is(&r.method)
            || (record.has("selector")
                && record.get("selector") != &selector.clone().map_or(Js::Null, Js::from))
        {
            return Err(err("recorded_transport_request_conflict"));
        }
        self.dispatches += 1;
        self.event(obj(&[
            ("event", "recorded_dispatch".into()),
            ("path", r.path.clone().into()),
            ("method", r.method.clone().into()),
            ("selector", selector.clone().map_or(Js::Null, Js::from)),
        ]))?;
        let header = |key: &str| {
            r.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.as_str())
        };
        let cookie = header("Cookie").unwrap_or("").to_string();
        let mut call = obj(&[
            ("path", r.path.clone().into()),
            ("method", r.method.clone().into()),
            ("selector", selector.clone().map_or(Js::Null, Js::from)),
            ("redirect", "manual".into()),
            ("cache", "no-store".into()),
            (
                "hasAbortSignal",
                (r.timeout == Duration::from_secs(20)).into(),
            ),
            ("signalAborted", false.into()),
            ("origin", header("Origin").map_or(Js::Null, Js::from)),
            ("referer", header("Referer").map_or(Js::Null, Js::from)),
            ("hasCookie", (!cookie.is_empty()).into()),
            ("hasAuthorization", header("Authorization").is_some().into()),
        ]);
        if r.path == "/servlet/controller.login.LoginServlet" && r.method == "POST" {
            let proof = ["userid", "userpwd"]
                .iter()
                .map(|key| {
                    let raw = r
                        .body
                        .iter()
                        .find(|(k, _)| k == key)
                        .map(|(_, v)| v.as_str())
                        .unwrap_or("");
                    let b = unbase64(raw).unwrap_or_default();
                    let decoded = String::from_utf8_lossy(&b);
                    let suffix = decoded.as_bytes();
                    let valid = suffix.len() >= 32
                        && &suffix[suffix.len() - 32..suffix.len() - 30] == b"  "
                        && suffix[suffix.len() - 30..]
                            .iter()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
                    let prefix = if valid {
                        &decoded[..decoded.len() - 32]
                    } else {
                        ""
                    };
                    let expected = record.get("credentialProof").get(key);
                    obj(&[
                        ("field", (*key).into()),
                        ("validSuffix", valid.into()),
                        (
                            "prefixMatchesFixture",
                            (expected.string() && expected.is(prefix)).into(),
                        ),
                    ])
                })
                .collect();
            call.set("loginEncoding", Js::Arr(proof));
        } else {
            let pairs = |a: &[(String, String)]| {
                Js::Arr(
                    a.iter()
                        .map(|(k, v)| Js::Arr(vec![k.clone().into(), v.clone().into()]))
                        .collect(),
                )
            };
            call.set("parameters", pairs(&r.body));
            call.set("query", pairs(&r.query));
        }
        self.calls.push(call);
        self.cookie_proof.push(obj(&[
            ("step", self.dispatches.into()),
            ("hasSession", cookie.contains("JSESSIONID=").into()),
            ("foreignCookieForwarded", cookie.contains("OTHER=").into()),
        ]));
        if record.get("abortAfterDeadline").truthy() {
            let deadline = Instant::now() + r.timeout;
            loop {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                std::thread::park_timeout(deadline - now)
            }
            self.event(obj(&[
                ("event", "source_abort_signal".into()),
                ("path", r.path.clone().into()),
                ("selector", selector.map_or(Js::Null, Js::from)),
            ]))?;
            return Err(err("recorded_transport_aborted"));
        }
        if record.has("error") {
            return Err(err("recorded_transport_failed"));
        }
        let headers = record
            .get("headers")
            .array()
            .iter()
            .map(|p| (string(&p.array()[0]), string(&p.array()[1])))
            .collect();
        let bytes = if record.has("bodyBase64") {
            unbase64(&string(record.get("bodyBase64")))?
        } else {
            string(record.get("body")).into_bytes()
        };
        Ok(Response {
            status: number(record.get("status")).unwrap_or(200.) as u16,
            headers,
            bytes,
            url: string(record.get("url")),
            stalled: record.get("stalled").truthy(),
        })
    }
}

fn assert_assessment_options(options: &Js) -> Result<()> {
    let homework = [
        "미확인",
        "미실시",
        "숙제 수행도 C",
        "숙제 수행도 B",
        "숙제 수행도 B⁺",
        "숙제 수행도 A",
        "숙제 수행도 A⁺",
    ];
    let mut expected = vec![];
    for (i, label) in homework.iter().enumerate() {
        expected.push(obj(&[
            ("field", "homework_radio".into()),
            ("value", (i as i32 - 1).to_string().into()),
            ("label", (*label).into()),
        ]));
    }
    for value in -1..=10 {
        expected.push(obj(&[
            ("field", "daily_test_radio".into()),
            ("value", value.to_string().into()),
            (
                "label",
                match value {
                    -1 => "미확인".into(),
                    0 => "미실시".into(),
                    _ => format!("일일 테스트 {}점", value * 10).into(),
                },
            ),
        ]));
    }
    // Exact multiset retains duplicate rows, extra properties and unnormalized labels.
    let mut actual = options.array().iter().map(canonical).collect::<Vec<_>>();
    let mut expected = expected.iter().map(canonical).collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    if actual != expected {
        return Err(err("journal_assessment_options_drift"));
    }
    Ok(())
}

#[cfg(test)]
mod review_fixes_fictional_journal {
    use super::*;
    // All values below are fictional structural proof, never a qualified recording.
    struct Fake {
        source: String,
        calls: Vec<Request>,
    }
    impl Transport for Fake {
        fn dispatch(&mut self, request: &Request) -> Result<Response> {
            self.calls.push(request.clone());
            Ok(Response {
                status: 200,
                headers: vec![],
                bytes: self.source.as_bytes().to_vec(),
                url: format!("{ORIGIN}{SELECTED}"),
                stalled: false,
            })
        }
    }
    fn fixture() -> (Js, Js, Js, String) {
        let target = parse(r#"{"studentName":"Fictional 😀 Learner","date":"2040-01-04","groupId":"3","teacherKey":"99001","studentKey":"77001","courseKey":"5","recordKey":"13","membershipKey":"8"}"#).unwrap();
        let row = obj(&[("fields",parse(r#"{"progress":"Current","homework":"","lessonMemo":"Memo","attendance":"Y","persistentMemo":"preserved"}"#).unwrap())]);
        let options = parse(include_str!(
            "../tests/fixtures/source-adapter-review-fixes-v2/ASSESSMENT-EXPECTED.json"
        ))
        .unwrap()
        .get("options")
        .clone();
        let mut functions = obj(&[]);
        for (name, selector) in [
            ("onAttn", "udtAttn"),
            ("onFoPrg", "udtPrg"),
            ("onFoHw", "udtHw"),
            ("onFoMm", "udtMemo"),
            ("setDailyTest", "SetDailyTest"),
            ("openHomeWorkRate", "SetHomeWorkRate"),
        ] {
            functions.set(
                name,
                format!("function {name}(){{ return 'reqCmd={selector}'; }}").into(),
            );
        }
        let controls = parse(r#"[{"name":"h_daily_test","value":"0"},{"name":"h_homework_rate","value":"3"},{"handler":"setDailyTest(13,8,0)"},{"handler":"openHomeWorkRate(12,8,3)"}]"#).unwrap();
        let wire = obj(&[
            ("functions", functions),
            ("assessmentOptions", options),
            (
                "rows",
                Js::Arr(vec![obj(&[
                    ("studentName", target.get("studentName").clone()),
                    ("controls", controls),
                ])]),
            ),
        ]);
        (
            target,
            row,
            wire,
            "<root><record>!2040-01-02!5!!Fictional 😀 Learner!!!!!!12!77001!8</record></root>"
                .into(),
        )
    }
    fn controls(wire: &mut Js, change: impl FnOnce(&mut Vec<Js>)) {
        let mut rows = wire.get("rows").array().to_vec();
        let mut controls = rows[0].get("controls").array().to_vec();
        change(&mut controls);
        rows[0].set("controls", Js::Arr(controls));
        wire.set("rows", Js::Arr(rows));
    }
    #[test]
    fn matching_journal_occurrence_cannot_hide_entities_outside_the_xml_root() {
        for outside in ["&#32;", "&#x20;", "&#9;", "&#xA;", "&#13;"] {
            for before in [true, false] {
                let (target, row, wire, source) = fixture();
                let source = if before {
                    format!("{outside}{source}")
                } else {
                    format!("{source}{outside}")
                };
                let principal = obj(&[
                    ("teacherLabel", "Fictional Teacher".into()),
                    ("teacherKey", "99001".into()),
                    ("collegeKey", "88001".into()),
                ]);
                let mut client = Client::new(
                    principal,
                    Fake {
                        source,
                        calls: vec![],
                    },
                )
                .unwrap();
                client.authenticated = true;
                let error = client
                    .journal_after_qualification(&target, &row, &wire, "fictional-structural-only")
                    .unwrap_err();
                assert_eq!(error.name, "SyntaxError");
                assert_eq!(error.properties.get("code"), &Js::Num(12.0));
                assert_eq!(client.transport.calls.len(), 1);
                assert_eq!(client.transport.calls[0].method, "GET");
                assert_eq!(client.transport.calls[0].path, SELECTED);
            }
        }
    }
    #[test]
    fn rejected_requests_never_reach_transport() {
        let principal = parse(
            r#"{"teacherLabel":"Fictional Teacher","teacherKey":"99001","collegeKey":"88001"}"#,
        )
        .unwrap();
        let mut client = Client::new(
            principal,
            Fake {
                source: String::new(),
                calls: vec![],
            },
        )
        .unwrap();
        client.authenticated = true;
        let mut cases = vec![];
        for selector in [
            "udtPrg",
            "udtHw",
            "udtMemo",
            "udtAttn",
            "SetDailyTest",
            "SetHomeWorkRate",
        ] {
            cases.push((
                selector,
                "GET",
                MAIN,
                vec![("reqCmd".into(), selector.into())],
                vec![],
            ));
        }
        let selected = ("reqCmd".into(), "SelSglRecord".into());
        cases.push((
            "total_three_get",
            "GET",
            SELECTED,
            vec![selected.clone()],
            vec![selected.clone(); 2],
        ));
        cases.push((
            "duplicate_query",
            "GET",
            SELECTED,
            vec![selected; 2],
            vec![],
        ));
        let main = ("p_process".into(), "Main".into());
        cases.push((
            "total_three_post",
            "POST",
            MAIN,
            vec![main.clone(); 2],
            vec![main.clone()],
        ));
        cases.push(("duplicate_body", "POST", MAIN, vec![], vec![main; 2]));
        let mut evidence = vec![];
        for (name, method, path, query, body) in cases {
            let result = client.read_request(method, path, query, body);
            assert_eq!(
                result.as_ref().unwrap_err().message,
                "reviewed_read_operation_required",
                "{name}"
            );
            assert!(client.transport.calls.is_empty(), "{name}");
            evidence.push(obj(&[
                ("case", name.into()),
                ("result", observe(result.map(Js::from))),
                ("dispatches", client.transport.calls.len().into()),
            ]));
        }
        if let Ok(path) = std::env::var("S09_REVIEW_ADMISSION_EVIDENCE") {
            std::fs::write(path, format!("{}\n", json(&Js::Arr(evidence)))).unwrap();
        }
    }
    #[test]
    fn fictional_post_qualification_join_and_refusal_order() {
        let mut evidence = vec![];
        for variant in [
            "success",
            "options_duplicate",
            "options_missing",
            "options_label",
            "options_extra",
            "handler_onAttn",
            "handler_onFoPrg",
            "handler_onFoHw",
            "handler_onFoMm",
            "handler_setDailyTest",
            "handler_openHomeWorkRate",
            "daily_missing",
            "daily_duplicate",
            "daily_record",
            "daily_member",
            "daily_zero",
            "daily_space",
            "previous_missing",
            "previous_duplicate",
            "previous_current",
            "previous_member",
            "previous_zero",
            "previous_malformed",
            "control_daily_missing",
            "control_daily_duplicate",
            "control_daily_nonstring",
            "control_homework_missing",
            "xml_course",
            "xml_name",
            "xml_record",
            "xml_student",
            "xml_member",
            "xml_date",
            "xml_empty_date",
            "xml_missing_record",
            "xml_duplicate_record",
            "xml_parsererror",
            "xml_malformed",
            "source_challenge",
            "assessment_and_xml",
            "noncalendar_prior_date",
        ] {
            let (target, row, mut wire, mut source) = fixture();
            let (expected, dispatches) = match variant {
                "options_duplicate" | "options_missing" | "options_label" | "options_extra" => {
                    let mut options = wire.get("assessmentOptions").array().to_vec();
                    match variant {
                        "options_duplicate" => options.push(options[0].clone()),
                        "options_missing" => {
                            options.remove(0);
                        }
                        "options_label" => {
                            options[0].set("label", "drift".into());
                        }
                        _ => {
                            options[0].set("extra", true.into());
                        }
                    }
                    wire.set("assessmentOptions", Js::Arr(options));
                    (Some("journal_assessment_options_drift"), 0)
                }
                v if v.starts_with("handler_") => {
                    let mut f = wire.get("functions").clone();
                    f.set(&v[8..], "reqCmd=wrong".into());
                    wire.set("functions", f);
                    (Some("journal_wire_drift"), 0)
                }
                v if v.starts_with("daily_") => {
                    controls(&mut wire, |c| match v {
                        "daily_missing" => {
                            c.remove(2);
                        }
                        "daily_duplicate" => c.push(c[2].clone()),
                        _ => {
                            c[2].set(
                                "handler",
                                match v {
                                    "daily_record" => "setDailyTest(14,8,0)",
                                    "daily_member" => "setDailyTest(13,9,0)",
                                    "daily_zero" => "setDailyTest(013,8,0)",
                                    _ => "setDailyTest(13, 8,0)",
                                }
                                .into(),
                            );
                        }
                    });
                    (Some("journal_daily_tuple_conflict"), 0)
                }
                v if v.starts_with("previous_") => {
                    controls(&mut wire, |c| match v {
                        "previous_missing" => {
                            c.remove(3);
                        }
                        "previous_duplicate" => c.push(c[3].clone()),
                        _ => {
                            c[3].set(
                                "handler",
                                match v {
                                    "previous_current" => "openHomeWorkRate(13,8,3)",
                                    "previous_member" => "openHomeWorkRate(12,9,3)",
                                    "previous_zero" => "openHomeWorkRate(012,8,3)",
                                    _ => "openHomeWorkRate(12,8,-1)",
                                }
                                .into(),
                            );
                        }
                    });
                    (
                        Some(if matches!(v, "previous_missing" | "previous_duplicate") {
                            "journal_previous_tuple_ambiguous"
                        } else {
                            "journal_previous_tuple_conflict"
                        }),
                        0,
                    )
                }
                v if v.starts_with("control_") => {
                    controls(&mut wire, |c| match v {
                        "control_daily_missing" => {
                            c.remove(0);
                        }
                        "control_daily_duplicate" => c.push(c[0].clone()),
                        "control_daily_nonstring" => {
                            c[0].set("value", Js::Num(0.));
                        }
                        _ => {
                            c.remove(1);
                        }
                    });
                    (Some("journal_assessment_not_observed"), 1)
                }
                "source_challenge" => {
                    source = "webUserPass".into();
                    (Some("authentication_required"), 1)
                }
                "xml_malformed" => {
                    source = "<root><record>".into();
                    (Some("about:blank:1:14: unclosed tag: record"), 1)
                }
                "xml_missing_record" => {
                    source = "<root/>".into();
                    (Some("journal_previous_occurrence_conflict"), 1)
                }
                "xml_duplicate_record" => {
                    source = source.replace("</root>", "<record>x</record></root>");
                    (Some("journal_previous_occurrence_conflict"), 1)
                }
                "xml_parsererror" => {
                    source = source.replace("</root>", "<parsererror/></root>");
                    (Some("journal_previous_occurrence_conflict"), 1)
                }
                "assessment_and_xml" => {
                    controls(&mut wire, |c| {
                        c.remove(0);
                    });
                    source = source.replace("!5!", "!6!");
                    (Some("journal_previous_occurrence_conflict"), 1)
                }
                v if v.starts_with("xml_") => {
                    let (from, to) = match v {
                        "xml_course" => ("!5!", "!6!"),
                        "xml_name" => ("Learner", "learner"),
                        "xml_record" => ("!12!", "!11!"),
                        "xml_student" => ("!77001!", "!77002!"),
                        "xml_member" => ("!8</", "!9</"),
                        "xml_date" => ("2040-01-02", "2040-01-04"),
                        _ => ("2040-01-02", ""),
                    };
                    source = source.replace(from, to);
                    (Some("journal_previous_occurrence_conflict"), 1)
                }
                "noncalendar_prior_date" => {
                    source = source.replace("2040-01-02", "2040-00-00");
                    (None, 1)
                }
                _ => (None, 1),
            };
            let principal = obj(&[
                ("teacherLabel", "Fictional Teacher".into()),
                ("teacherKey", "99001".into()),
                ("collegeKey", "88001".into()),
            ]);
            let mut client = Client::new(
                principal,
                Fake {
                    source,
                    calls: vec![],
                },
            )
            .unwrap();
            client.authenticated = true;
            let unchanged = (target.clone(), row.clone(), wire.clone());
            let result = client.journal_after_qualification(
                &target,
                &row,
                &wire,
                "fictional-structural-only",
            );
            assert_eq!(client.transport.calls.len(), dispatches, "{variant}");
            assert_eq!(
                (&target, &row, &wire),
                (&unchanged.0, &unchanged.1, &unchanged.2),
                "{variant}"
            );
            let observed = observe(result.clone());
            if let Some(expected) = expected {
                assert_eq!(result.unwrap_err().message, expected, "{variant}");
            } else {
                let result = result.unwrap();
                let mut fields = row.get("fields").clone();
                fields.set("dailyTestAssessment", "0".into());
                fields.set("previousHomeworkAssessment", "3".into());
                let effect_targets =
                    obj(&edu_academy_core::workflow::review::FIELDS.map(|field| {
                        let previous = field == "previousHomeworkAssessment";
                        let full = obj(&[
                            ("sourceNamespace", "academy.lms.dayrecord".into()),
                            ("teacherId", "99001".into()),
                            ("studentId", "77001".into()),
                            ("courseId", "5".into()),
                            ("membershipId", "8".into()),
                            ("recordId", if previous { "12" } else { "13" }.into()),
                            (
                                "lessonDate",
                                if !previous {
                                    "2040-01-04"
                                } else if variant == "success" {
                                    "2040-01-02"
                                } else {
                                    "2040-00-00"
                                }
                                .into(),
                            ),
                        ]);
                        (
                            field,
                            edu_academy_core::workflow::canonical(&full).unwrap().into(),
                        )
                    }));
                assert_eq!(
                    result,
                    obj(&[
                        ("target", target.clone()),
                        ("effectTargets", effect_targets),
                        ("fields", fields),
                        ("previousRecordKey", "12".into()),
                        (
                            "previousDate",
                            if variant == "success" {
                                "2040-01-02"
                            } else {
                                "2040-00-00"
                            }
                            .into()
                        ),
                        ("wireDigest", "fictional-structural-only".into())
                    ])
                );
            }
            let requests = client
                .transport
                .calls
                .iter()
                .map(|r| {
                    assert_eq!(r.method, "GET");
                    assert_eq!(r.path, SELECTED);
                    assert!(r.body.is_empty());
                    assert_eq!(
                        r.query,
                        vec![
                            ("reqCmd".into(), "SelSglRecord".into()),
                            ("course_seq".into(), "5".into()),
                            ("stu_pri_no".into(), "77001".into()),
                            ("record_seq".into(), "12".into()),
                            ("sel_grp_seq".into(), "3".into()),
                            ("tut_pri_no".into(), "99001".into()),
                            ("cm_seq".into(), "8".into())
                        ]
                    );
                    obj(&[
                        ("method", r.method.clone().into()),
                        ("path", r.path.clone().into()),
                        (
                            "query",
                            Js::Arr(
                                r.query
                                    .iter()
                                    .map(|(k, v)| Js::Arr(vec![k.clone().into(), v.clone().into()]))
                                    .collect(),
                            ),
                        ),
                    ])
                })
                .collect();
            evidence.push(obj(&[
                ("variant", variant.into()),
                ("fictionalStructuralOnly", true.into()),
                ("target", target),
                ("result", observed),
                ("requests", Js::Arr(requests)),
                ("sourceAndTargetUnchanged", true.into()),
            ]));
        }
        if let Ok(path) = std::env::var("S09_REVIEW_UNIT_EVIDENCE") {
            std::fs::write(path, format!("{}\n", json(&Js::Arr(evidence)))).unwrap();
        }
    }
}

#[cfg(test)]
mod xml_well_formed_tests {
    use super::*;
    #[test]
    fn document_boundary_matches_the_typescript_source_dom_parser() {
        let cases = parse(include_str!(
            "../tests/fixtures/xml-document-boundaries.json"
        ))
        .unwrap();
        for case in cases.array() {
            let source = string(case.get("source"));
            let result = xml_record(&source);
            if case.get("outcome").is("returned") {
                assert_eq!(
                    result.unwrap(),
                    string(case.get("record")),
                    "{}",
                    string(case.get("id"))
                );
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    error.name,
                    string(case.get("name")),
                    "{}",
                    string(case.get("id"))
                );
                assert_eq!(
                    error.message,
                    string(case.get("message")),
                    "{}",
                    string(case.get("id"))
                );
                assert_eq!(
                    error.properties.get("code"),
                    case.get("code"),
                    "{}",
                    string(case.get("id"))
                );
            }
        }
    }
    #[test]
    fn malformed_opening_tags_and_duplicate_attributes_never_return_record_text() {
        for source in [
            "<root a=\"1\" a=\"2\"><record>ok</record></root>",
            "<root a><record>ok</record></root>",
            "<root a=bad><record>ok</record></root>",
            "<root a=\"1\"b=\"2\"><record>ok</record></root>",
            "<root><record a='1' a='2'>ok</record></root>",
            "<root><record a='1' / >ok</record></root>",
            "<root><record a='<'>ok</record></root>",
            "<root a='&missing;'><record>ok</record></root>",
            "<root><record>ok</record bogus></root>",
            "<root xmlns:a='urn:x' xmlns:b='urn:x' a:x='1' b:x='2'><record>ok</record></root>",
            "<root a:unbound='1'><record>ok</record></root>",
            "<root><:record>ok</:record></root>",
        ] {
            let error = xml_record(source).unwrap_err();
            assert_eq!(error.name, "SyntaxError", "{source}");
            assert_eq!(error.properties.get("code"), &Js::Num(12.0));
        }
    }
    #[test]
    fn valid_attributes_quotes_entities_and_unicode_preserve_record_text() {
        for source in [
            "<root a=\"1>2\"><record b='x'>한글 &amp; 😀</record></root>",
            "<root a = '1' A=\"2\"><record><![CDATA[한글 & 😀]]></record></root>",
            "<뿌리 속성='값'><record>한글 &amp; 😀</record><tail a='ok'/></뿌리>",
            "<root xmlns:a='urn:x' xmlns:b='urn:y' a:x='1' b:x='2'><record>한글 &amp; 😀</record></root>",
        ] {
            assert_eq!(xml_record(source).unwrap(), "한글 & 😀");
        }
    }
}

/// Upper bounds apply before encoding/dispatch and while reading the body.
pub const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;
#[derive(Clone, Copy)]
pub struct TransportLimits {
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub timeout: Duration,
}
impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            request_bytes: MAX_REQUEST_BYTES,
            response_bytes: MAX_RESPONSE_BYTES,
            timeout: Duration::from_secs(20),
        }
    }
}
impl TransportLimits {
    fn validate(self) -> Result<Self> {
        if self.request_bytes == 0
            || self.request_bytes > MAX_REQUEST_BYTES
            || self.response_bytes == 0
            || self.response_bytes > MAX_RESPONSE_BYTES
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(20)
        {
            return Err(err("native_transport_limits_invalid"));
        }
        Ok(self)
    }
}
/// Cancellation is sticky. An in-flight blocking call ends by its overall deadline;
/// no request worker, child process, automatic retry or persistent cookie jar exists.
/// ureq may leave a timed-out DNS lookup thread until libc returns. It holds no
/// credentials or account flock, cannot issue a request, and dies with the owner
/// process. Dropping NativeSession releases the flock independently of DNS.
#[derive(Clone, Default)]
pub struct Cancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn check(&self) -> Result<()> {
        if self.0.load(std::sync::atomic::Ordering::Acquire) {
            Err(err("native_transport_cancelled"))
        } else {
            Ok(())
        }
    }
}
/// Fixed Academy origin in production. The only alternate target is a literal
/// loopback SocketAddr for synthetic qualification; it cannot name a remote host.
/// No environment/proxy/credential discovery and no executable invocation.
pub struct NativeTransport {
    account: Option<std::rc::Rc<crate::owner_root::AccountLease>>,
    destination: url::Url,
    limits: TransportLimits,
    cancellation: Cancellation,
    agent: ureq::Agent,
    deadline: std::sync::Arc<SocketDeadline>,
}
impl NativeTransport {
    pub fn new(limits: TransportLimits, cancellation: Cancellation) -> Result<Self> {
        Self::configured(
            url::Url::parse(ORIGIN).expect("fixed origin"),
            limits,
            cancellation,
            ureq::unversioned::resolver::DefaultResolver::default(),
        )
    }
    /// Resolver injection for deterministic offline timeout qualification. Origin,
    /// certificate verification, transport bounds and no-proxy policy stay fixed.
    pub fn with_resolver(
        limits: TransportLimits,
        cancellation: Cancellation,
        resolver: impl ureq::unversioned::resolver::Resolver,
    ) -> Result<Self> {
        Self::configured(
            url::Url::parse(ORIGIN).expect("fixed origin"),
            limits,
            cancellation,
            resolver,
        )
    }
    pub fn synthetic_loopback(
        address: std::net::SocketAddr,
        limits: TransportLimits,
        cancellation: Cancellation,
    ) -> Result<Self> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(err("synthetic_loopback_required"));
        }
        let destination = url::Url::parse(&format!("http://{address}"))
            .map_err(|_| err("synthetic_loopback_required"))?;
        Self::configured(
            destination,
            limits,
            cancellation,
            ureq::unversioned::resolver::DefaultResolver::default(),
        )
    }
    fn configured(
        destination: url::Url,
        limits: TransportLimits,
        cancellation: Cancellation,
        resolver: impl ureq::unversioned::resolver::Resolver,
    ) -> Result<Self> {
        let limits = limits.validate()?;
        // A fresh socket per operation removes pooled-connection replay paths.
        // Disable library redirects: Client alone permits one login redirect.
        let agent = ureq::Agent::config_builder()
            .proxy(None)
            .https_only(destination.scheme() == "https")
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .max_idle_connections(0)
            .max_idle_connections_per_host(0)
            .max_response_header_size(16 * 1024)
            .timeout_global(Some(limits.timeout))
            .timeout_resolve(Some(limits.timeout))
            .timeout_connect(Some(limits.timeout))
            .timeout_send_request(Some(limits.timeout))
            .timeout_send_body(Some(limits.timeout))
            .timeout_recv_response(Some(limits.timeout))
            .timeout_recv_body(Some(limits.timeout))
            .build();
        let deadline = std::sync::Arc::new(SocketDeadline {
            at: std::sync::Mutex::new(Instant::now()),
            cancellation: cancellation.clone(),
        });
        use ureq::unversioned::transport::Connector;
        let connector = DeadlineConnector(std::sync::Arc::clone(&deadline))
            .chain(ureq::unversioned::transport::RustlsConnector::default());
        let agent = ureq::Agent::with_parts(agent, connector, resolver);
        Ok(Self {
            account: None,
            destination,
            limits,
            cancellation,
            agent,
            deadline,
        })
    }
    fn transport_error(&self, error: ureq::Error) -> Error {
        if self.cancellation.check().is_err() {
            return err("native_transport_cancelled");
        }
        if matches!(error, ureq::Error::Timeout(_)) || self.deadline.expired() {
            err("native_transport_timeout")
        } else {
            err("academy_transport_failed")
        }
    }
}
impl Transport for NativeTransport {
    fn bind_account(&mut self, lease: std::rc::Rc<crate::owner_root::AccountLease>) {
        // Never replace a live lease: closing the transport releases its account.
        if self.account.is_none() {
            self.account = Some(lease);
        }
    }
    fn dispatch(&mut self, request: &Request) -> Result<Response> {
        self.cancellation.check()?;
        if self.destination.scheme() == "https" {
            self.account
                .as_ref()
                .ok_or_else(|| err("owner_account_lease_required"))?
                .validate()?;
        }
        if !matches!(request.method.as_str(), "GET" | "POST")
            || !request.path.starts_with('/')
            || request.path.starts_with("//")
            || request.path.contains(['?', '#', '\\', '\r', '\n'])
            || request.timeout.is_zero()
            || (request.method == "GET" && !request.body.is_empty())
        {
            return Err(err("native_transport_request_refused"));
        }
        let raw_size = request.path.len().saturating_add(
            request
                .query
                .iter()
                .chain(&request.body)
                .chain(&request.headers)
                .fold(0usize, |n, (k, v)| {
                    n.saturating_add(k.len()).saturating_add(v.len())
                }),
        );
        if raw_size > self.limits.request_bytes {
            return Err(err("native_request_too_large"));
        }
        let mut url = self
            .destination
            .join(&request.path)
            .map_err(|_| err("native_transport_request_refused"))?;
        if url.origin() != self.destination.origin() || url.path() != request.path {
            return Err(err("native_transport_request_refused"));
        }
        url.query_pairs_mut().extend_pairs(request.query.iter());
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(request.body.iter())
            .finish();
        if url
            .as_str()
            .len()
            .saturating_add(body.len())
            .saturating_add(raw_size)
            > self.limits.request_bytes
        {
            return Err(err("native_request_too_large"));
        }
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(url.as_str());
        for (key, value) in &request.headers {
            if !matches!(
                key.to_ascii_lowercase().as_str(),
                "origin" | "referer" | "cookie" | "content-type" | "accept" | "x-requested-with"
            ) || value.contains(['\r', '\n'])
            {
                return Err(err("native_transport_header_refused"));
            }
            builder = builder.header(key.as_str(), value.as_str());
        }
        let wire = builder
            .header("Connection", "close")
            .body(body.as_bytes())
            .map_err(|_| err("native_transport_request_refused"))?;
        let started = Instant::now();
        let timeout = self.limits.timeout.min(request.timeout);
        self.deadline.arm(started + timeout)?;
        let wire = self
            .agent
            .configure_request(wire)
            .timeout_global(Some(timeout))
            .build();
        let mut response = self
            .agent
            .run(wire)
            .map_err(|error| self.transport_error(error))?;
        self.cancellation.check()?;
        if response
            .headers()
            .get("content-encoding")
            .is_some_and(|v| v != "identity")
        {
            return Err(err("native_content_encoding_refused"));
        }
        if response.headers().get("content-length").is_some_and(|v| {
            v.to_str()
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .is_none_or(|n| n > self.limits.response_bytes as u64)
        }) {
            return Err(err("source_response_too_large"));
        }
        let status = response.status().as_u16();
        // Keep only protocol headers. Never stringify a client error or response.
        let headers = response
            .headers()
            .iter()
            .filter(|(k, _)| matches!(k.as_str(), "set-cookie" | "location" | "content-type"))
            .map(|(k, v)| {
                v.to_str()
                    .map(|v| (k.to_string(), v.to_string()))
                    .map_err(|_| err("native_response_header_refused"))
            })
            .collect::<Result<Vec<_>>>()?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(self.limits.response_bytes as u64)
            .read_to_vec()
            .map_err(|error| {
                if self.deadline.expired() || self.cancellation.check().is_err() {
                    self.transport_error(error)
                } else {
                    err("native_response_read_failed")
                }
            })?;
        self.cancellation.check()?;
        if started.elapsed() > timeout {
            return Err(err("source_response_timeout"));
        }
        Ok(Response {
            status,
            headers,
            bytes,
            url: format!("{ORIGIN}{}", request.path),
            stalled: false,
        })
    }
}

/// Armed before resolution. ureq's Rustls adapter reuses relative timeouts in
/// complete_io; this lower TCP boundary clamps every read/partial write to the
/// remaining absolute wall time, including TLS handshake. No credential/lease.
struct SocketDeadline {
    at: std::sync::Mutex<Instant>,
    cancellation: Cancellation,
}
impl SocketDeadline {
    fn arm(&self, at: Instant) -> Result<()> {
        *self
            .at
            .lock()
            .map_err(|_| err("native_transport_deadline_failed"))? = at;
        Ok(())
    }
    fn expired(&self) -> bool {
        self.at.lock().map_or(true, |at| Instant::now() >= *at)
    }
    fn remaining(
        &self,
        timeout: &ureq::unversioned::transport::NextTimeout,
    ) -> std::result::Result<Duration, ureq::Error> {
        self.cancellation
            .check()
            .map_err(|_| ureq::Error::Io(std::io::Error::other("native_transport_cancelled")))?;
        let at = *self.at.lock().map_err(|_| ureq::Error::ConnectionFailed)?;
        let remaining = at
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or(ureq::Error::Timeout(ureq::Timeout::Global))?;
        Ok(timeout
            .not_zero()
            .map_or(remaining, |relative| remaining.min(*relative)))
    }
}
struct DeadlineConnector(std::sync::Arc<SocketDeadline>);
struct DeadlineTcp {
    stream: std::net::TcpStream,
    buffers: ureq::unversioned::transport::LazyBuffers,
    deadline: std::sync::Arc<SocketDeadline>,
}
impl std::fmt::Debug for DeadlineConnector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DeadlineConnector")
    }
}
impl std::fmt::Debug for DeadlineTcp {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DeadlineTcp")
    }
}
impl ureq::unversioned::transport::Connector for DeadlineConnector {
    type Out = DeadlineTcp;
    fn connect(
        &self,
        details: &ureq::unversioned::transport::ConnectionDetails,
        _: Option<()>,
    ) -> std::result::Result<Option<Self::Out>, ureq::Error> {
        for address in &details.addrs {
            let remaining = self.0.remaining(&details.timeout)?;
            let stream = match std::net::TcpStream::connect_timeout(address, remaining) {
                Ok(stream) => stream,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    return Err(ureq::Error::Timeout(details.timeout.reason));
                }
                Err(error) => return Err(error.into()),
            };
            self.0.remaining(&details.timeout)?;
            stream.set_nodelay(details.config.no_delay())?;
            return Ok(Some(DeadlineTcp {
                stream,
                buffers: ureq::unversioned::transport::LazyBuffers::new(
                    details.config.input_buffer_size(),
                    details.config.output_buffer_size(),
                ),
                deadline: std::sync::Arc::clone(&self.0),
            }));
        }
        Err(ureq::Error::ConnectionFailed)
    }
}
impl ureq::unversioned::transport::Transport for DeadlineTcp {
    fn buffers(&mut self) -> &mut dyn ureq::unversioned::transport::Buffers {
        &mut self.buffers
    }
    fn transmit_output(
        &mut self,
        amount: usize,
        timeout: ureq::unversioned::transport::NextTimeout,
    ) -> std::result::Result<(), ureq::Error> {
        use std::io::Write;
        use ureq::unversioned::transport::Buffers;
        let mut offset = 0;
        while offset < amount {
            let remaining = self.deadline.remaining(&timeout)?;
            self.stream.set_write_timeout(Some(remaining))?;
            match self.stream.write(&self.buffers.output()[offset..amount]) {
                Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into()),
                Ok(written) => offset += written,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    return Err(ureq::Error::Timeout(timeout.reason));
                }
                Err(error) => return Err(error.into()),
            }
        }
        self.deadline.remaining(&timeout)?;
        Ok(())
    }
    fn maybe_await_input(
        &mut self,
        timeout: ureq::unversioned::transport::NextTimeout,
    ) -> std::result::Result<bool, ureq::Error> {
        use ureq::unversioned::transport::Buffers;
        self.deadline.remaining(&timeout)?;
        if self.buffers.can_use_input() {
            Ok(true)
        } else {
            self.await_input(timeout)
        }
    }
    fn await_input(
        &mut self,
        timeout: ureq::unversioned::transport::NextTimeout,
    ) -> std::result::Result<bool, ureq::Error> {
        use std::io::Read;
        use ureq::unversioned::transport::Buffers;
        loop {
            let remaining = self.deadline.remaining(&timeout)?;
            self.stream.set_read_timeout(Some(remaining))?;
            let amount = match self.stream.read(self.buffers.input_append_buf()) {
                Ok(amount) => amount,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    return Err(ureq::Error::Timeout(timeout.reason));
                }
                Err(error) => return Err(error.into()),
            };
            self.deadline.remaining(&timeout)?;
            self.buffers.input_appended(amount);
            return Ok(amount > 0);
        }
    }
    // Never return a socket to a pool or permit request replay.
    fn is_open(&mut self) -> bool {
        false
    }
}
