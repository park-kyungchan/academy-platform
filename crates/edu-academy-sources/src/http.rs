//! Recorded HTTP compatibility. Injected transport only; no network implementation.
#[path = "name_search.rs"]
mod name_search;

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
        if self.stalled {
            loop {
                std::thread::park();
            }
        }
        let s = String::from_utf8_lossy(&self.bytes);
        s.strip_prefix('\u{feff}').unwrap_or(&s).to_string()
    }
}
pub trait Transport {
    fn dispatch(&mut self, request: &Request) -> Result<Response>;
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
        let response = self
            .transport
            .dispatch(&request)
            .map_err(|_| err("academy_transport_failed"))?;
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
        // Deliberately preserve legacy stalled-body lifetime: header timeout is not a body deadline.
        if response.stalled {
            loop {
                std::thread::park();
            }
        }
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
        Ok(obj(&[
            ("target", target.clone()),
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
    fn write(&mut self, action: &Js) -> edu_academy_core::js::Result<()> {
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
            .map(|_| ())
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
    fn write(client: &mut Client<Fictional>, action: &Js) -> edu_academy_core::js::Result<()> {
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
            write(&mut c, &action(field, after)).unwrap();
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
pub(crate) fn validate_headers(row: scraper::ElementRef<'_>) -> Result<()> {
    let table =
        dom::closest(row, "table").ok_or_else(|| err("day_record_column_contract_rejected"))?;
    let headers = dom::select(table, "thead > tr");
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
            if dom::content(*c)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                != *label
                || c.value()
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
    let lit_re = regex::Regex::new(r##"^\s*(?:([0-9]+)|'([0-9]+)'|"([0-9]+)")\s*$"##).unwrap();
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
            if !columns[column].contains(&handler) {
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
                    || literal.get(1).is_some()
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
                || (key != "i" && !s.bytes().any(|c| matches!(c, b'1'..=b'9')))
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
    if let Some(p) = calls.get("openHomeWorkRate")
        && ["record_seq", "cm_seq"]
            .iter()
            .any(|k| !string(p.get(k)).bytes().any(|c| matches!(c, b'1'..=b'9')))
    {
        return Err(err("requested_row_binding_unresolved"));
    }
    Ok((fields, canonical.clone()))
}
/// Strict recorded XML token parser; inert text only. Reports the original XML error shape.
pub(crate) fn xml_record(s: &str) -> Result<String> {
    xml_record_observation(s).map(|(record, _)| record)
}
fn xml_record_observation(s: &str) -> Result<(String, bool)> {
    let mut parsererror = false;
    let mut stack: Vec<String> = vec![];
    let mut records = vec![];
    let mut record_count = 0;
    let mut record = None::<String>;
    let mut i = 0;
    let mut roots = 0;
    let b = s.as_bytes();
    let syntax = |pos: usize, message: String| {
        let prefix = &s[..pos];
        let line = prefix.bytes().filter(|c| *c == b'\n').count() + 1;
        let col = prefix
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .encode_utf16()
            .count();
        let mut e = Error::new(
            "SyntaxError",
            format!("about:blank:{line}:{col}: {message}"),
        );
        e.properties.set("code", Js::Num(12.));
        e
    };
    while i < b.len() {
        if b[i] != b'<' {
            let end = s[i..].find('<').map_or(s.len(), |n| i + n);
            if let Some(r) = &mut record {
                r.push_str(
                    &xml_entities(&s[i..end])
                        .map_err(|_| syntax(end, "undefined entity.".into()))?,
                )
            } else if stack.is_empty() && !s[i..end].trim().is_empty() {
                return Err(syntax(i, "text data outside of root node.".into()));
            }
            i = end;
            continue;
        }
        if s[i..].starts_with("<!--") {
            i = s[i + 4..]
                .find("-->")
                .map(|n| i + 4 + n + 3)
                .ok_or_else(|| syntax(s.len(), "unclosed comment.".into()))?;
            continue;
        }
        if s[i..].starts_with("<![CDATA[") {
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
        let end = s[i..]
            .find('>')
            .map(|n| i + n)
            .ok_or_else(|| syntax(s.len(), "unclosed tag.".into()))?;
        let raw = s[i + 1..end].trim();
        let close = raw.starts_with('/');
        let empty = raw.ends_with('/');
        let name = raw
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        if name.is_empty() {
            return Err(syntax(end + 1, "disallowed character in tag name.".into()));
        }
        if close {
            if stack.last().map(String::as_str) != Some(name) {
                return Err(syntax(end + 1, "unexpected close tag.".into()));
            }
            stack.pop();
            if name == "record"
                && let Some(r) = record.take()
            {
                records.push(r);
            }
        } else {
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
            "../../../migration/source-adapter-review-fixes-v2/ASSESSMENT-EXPECTED.json"
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
                assert_eq!(
                    result,
                    obj(&[
                        ("target", target.clone()),
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
