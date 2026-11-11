//! Owner-bound passive course/book observations and reusable E07 READ primitives.
//! No credentials, runtime discovery, handlers, effects or device observations.
use crate::{
    data::*,
    dom::{self, Document},
    http::{self, Request, Response, Transport},
    js_text::{DecodeRefusal, decode_bounded},
    owner_admission::{
        OperationContract, ParameterValue, ScopeField, TransportAdmission, TransportParameter,
        TransportScope,
    },
    owner_workflow::NativeSession,
};
use std::collections::HashSet;

pub const COURSE_NS: &str = "academy.lms.student_course";
pub const METADATA_NS: &str = "academy.lms.metadata";
pub const DAY_NS: &str = "academy.lms.dayrecord";
pub const COURSE_PATH: &str = "/servlet/controller.coursemanage.CourseManageServlet";
pub const BOOK_PATH: &str = "/servlet/controller.coursemanage.TextBookManageServlet";
pub const DAILY_PATH: &str = "/servlet/controller.dailyzerotest.DailyZeroTestServlet";
pub const PAPER_PATH: &str = "/servlet/controller.tutor.base.TestPageListServlet";
pub(crate) fn require(ok: bool, code: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::refusal(code))
    }
}
pub(crate) fn exact(v: &Js, keys: &[&str]) -> Result<()> {
    require(
        object(v) && v.props().len() == keys.len() && keys.iter().all(|k| v.has(k)),
        "tool_input_invalid",
    )
}
pub(crate) fn key(v: &Js) -> Result<String> {
    let s = String::from_utf16(v.units())
        .map_err(|_| Error::refusal("student_read_source_key_invalid"))?;
    require(
        v.string()
            && !s.is_empty()
            && s.len() <= 16
            && !s.starts_with('0')
            && s.bytes().all(|b| b.is_ascii_digit())
            && s.parse::<u64>().is_ok_and(|n| n <= 9_007_199_254_740_991),
        "student_read_source_key_invalid",
    )?;
    Ok(s)
}
pub(crate) fn name(v: &Js) -> Result<String> {
    require(
        v.string()
            && !trim(v.units()).is_empty()
            && v.units().len() <= 200
            && !v.units().iter().any(|u| *u < 32),
        "student_read_name_invalid",
    )?;
    String::from_utf16(v.units()).map_err(|_| Error::refusal("student_read_name_invalid"))
}
pub(crate) fn date(v: &Js) -> Result<String> {
    require(valid_date(v), "student_read_date_invalid")?;
    Ok(string(v))
}
pub(crate) fn instant(v: &str) -> Result<()> {
    require(
        edu_academy_core::instant::epoch_ms(
            &v.into(),
            edu_academy_core::instant::Grammar::DailyRecord,
        )
        .is_some(),
        "student_read_observation_time_invalid",
    )
}
#[derive(Clone)]
pub struct Student {
    pub student_key: String,
    pub student_name: String,
}
/// Exact configured identities; never a claim that the current teacher roster is complete.
pub struct Owner {
    pub(crate) owner_id: String,
    pub(crate) teacher_label: String,
    pub(crate) teacher_key: String,
    pub(crate) college_key: String,
    pub(crate) students: Vec<Student>,
}
impl Owner {
    pub fn parse(v: &Js) -> Result<Self> {
        exact(
            v,
            &[
                "ownerId",
                "teacherLabel",
                "teacherSourceKey",
                "collegeSourceKey",
                "students",
            ],
        )?;
        let owner_id = string(v.get("ownerId"));
        require(
            v.get("ownerId").string()
                && !owner_id.is_empty()
                && owner_id.len() <= 100
                && owner_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
            "student_read_owner_scope_invalid",
        )?;
        let rows = v.get("students");
        // E07 admission binds at most 200 exact student IDs, so this facade uses the same bound.
        require(
            array(rows) && (1..=200).contains(&rows.array().len()),
            "student_read_owner_scope_invalid",
        )?;
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        let mut students = vec![];
        for row in rows.array() {
            exact(row, &["studentKey", "studentName"])?;
            let student = Student {
                student_key: key(row.get("studentKey"))?,
                student_name: name(row.get("studentName"))?,
            };
            require(
                ids.insert(student.student_key.clone())
                    && names.insert(student.student_name.clone()),
                "student_read_owner_scope_invalid",
            )?;
            students.push(student);
        }
        Ok(Self {
            owner_id,
            teacher_label: name(v.get("teacherLabel"))?,
            teacher_key: key(v.get("teacherSourceKey"))?,
            college_key: key(v.get("collegeSourceKey"))?,
            students,
        })
    }
    pub fn scope_page(&self) -> Js {
        obj(&[
            ("schemaVersion", 1.into()),
            ("sourceOperation", "configured_teacher_scope".into()),
            ("sourceKind", "native_owner_configuration".into()),
            ("teacherLabel", self.teacher_label.as_str().into()),
            ("teacherKey", self.teacher_key.as_str().into()),
            ("collegeKey", self.college_key.as_str().into()),
            (
                "studentNames",
                Js::Arr(
                    self.students
                        .iter()
                        .map(|s| s.student_name.as_str().into())
                        .collect(),
                ),
            ),
            ("authentication", "not_attempted".into()),
            (
                "coverage",
                "configured_students_not_verified_current_complete_teacher_roster".into(),
            ),
            (
                "currentScreen",
                screen("configuration_is_not_device_observation"),
            ),
        ])
    }
    pub(crate) fn student(&self, name: &str) -> Result<&Student> {
        self.students
            .iter()
            .find(|s| s.student_name == name)
            .ok_or_else(|| Error::refusal("student_read_outside_owner_scope"))
    }
    pub(crate) fn scope(&self, student: &Student, date: &str, ns: &str) -> TransportScope {
        TransportScope {
            owner_id: self.owner_id.clone(),
            source_namespace: ns.into(),
            teacher_id: self.teacher_key.clone(),
            student_id: student.student_key.clone(),
            date: date.into(),
        }
    }
    pub(crate) fn validate_transport(&self, p: &TransportAdmission) -> Result<()> {
        p.validate()?;
        require(
            p.owner_id == self.owner_id
                && p.teacher_id == self.teacher_key
                && p.teacher_label == self.teacher_label
                && p.college_id == self.college_key
                && p.student_ids.len() == self.students.len()
                && self
                    .students
                    .iter()
                    .all(|s| p.student_ids.contains(&s.student_key)),
            "student_read_owner_transport_conflict",
        )
    }
    pub(crate) fn principal(&self) -> Js {
        obj(&[
            ("teacherLabel", self.teacher_label.as_str().into()),
            ("teacherKey", self.teacher_key.as_str().into()),
            ("collegeKey", self.college_key.as_str().into()),
        ])
    }
}
pub(crate) fn screen(reason: &str) -> Js {
    obj(&[("status", "unknown".into()), ("reason", reason.into())])
}
/// Injected READ only. NativeSession retains E07 authentication, expiry, lease,
/// cancellation and abnormal-response invalidation. No raw Transport is accepted by the facade.
pub trait ReadPort {
    fn transport_admission(&self) -> &TransportAdmission;
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<Response>;
}
impl<T: Transport> ReadPort for NativeSession<'_, T> {
    fn transport_admission(&self) -> &TransportAdmission {
        NativeSession::transport_admission(self)
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<Response> {
        NativeSession::read(self, scope, operation, arguments)
    }
}
fn lit(k: &str, v: &str) -> TransportParameter {
    TransportParameter {
        name: k.into(),
        value: ParameterValue::Literal { value: v.into() },
    }
}
fn arg(k: &str, max: usize) -> TransportParameter {
    TransportParameter {
        name: k.into(),
        value: ParameterValue::Argument {
            name: k.into(),
            max_bytes: max,
        },
    }
}
fn scoped(k: &str, f: ScopeField) -> TransportParameter {
    TransportParameter {
        name: k.into(),
        value: ParameterValue::Scope { field: f },
    }
}
fn op(
    id: &str,
    ns: &str,
    method: &str,
    path: &str,
    query: Vec<TransportParameter>,
    body: Vec<TransportParameter>,
) -> OperationContract {
    OperationContract {
        id: id.into(),
        source_namespace: ns.into(),
        method: method.into(),
        path: path.into(),
        effect: false,
        mirrored_parameters: if id == "student.course.search" {
            vec![
                "reqCmd".into(),
                "student_name".into(),
                "cls_no".into(),
                "grade_no".into(),
            ]
        } else {
            vec![]
        },
        query,
        body,
    }
}
/// Exact owner-published wire contracts. Installing them grants no admission.
/// Every operation is an explicitly selected read, including POST search routes.
pub fn read_contracts() -> Vec<OperationContract> {
    let mut contracts = vec![
        op(
            "student.course.initial",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "StudyCourse")],
            vec![],
        ),
        op(
            "student.course.search",
            COURSE_NS,
            "POST",
            COURSE_PATH,
            vec![
                lit("reqCmd", "StudyCourse"),
                arg("clg_no", 16),
                lit("cls_no", "0"),
                lit("grade_no", "0"),
                arg("student_name", 800),
            ],
            vec![
                lit("reqCmd", "StudyCourse"),
                arg("g_clg_no", 16),
                scoped("g_pri_no", ScopeField::TeacherId),
                arg("g_mem_type", 200),
                lit("cls_no", "0"),
                lit("grade_no", "0"),
                arg("student_name", 800),
            ],
        ),
        op(
            "student.books",
            COURSE_NS,
            "POST",
            BOOK_PATH,
            vec![lit("reqCmd", "getTextBookProgress")],
            vec![arg("student_name", 800)],
        ),
        op(
            "prestudy.waiting.shell",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "WebUnPreStudy")],
            vec![],
        ),
        op(
            "prestudy.waiting.search",
            COURSE_NS,
            "POST",
            COURSE_PATH,
            vec![],
            vec![
                lit("reqCmd", "WebUnPreStudy"),
                arg("g_clg_no", 16),
                arg("g_mem_type", 200),
                scoped("g_pri_no", ScopeField::TeacherId),
                arg("student_name", 800),
            ],
        ),
        op(
            "prestudy.completed.shell",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "WebCtPreStudy")],
            vec![],
        ),
        op(
            "prestudy.completed.search",
            COURSE_NS,
            "POST",
            COURSE_PATH,
            vec![],
            vec![
                lit("reqCmd", "WebCtPreStudy"),
                arg("g_clg_no", 16),
                arg("g_mem_type", 200),
                scoped("g_pri_no", ScopeField::TeacherId),
                arg("student_name", 800),
            ],
        ),
        op(
            "prestudy.summary",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "TeacherPrestudySummary")],
            vec![],
        ),
        op(
            "prestudy.detail",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "WebPreStudy"), arg("prestudy_key", 16)],
            vec![],
        ),
        op(
            "prestudy.report",
            COURSE_NS,
            "GET",
            "/app-ui/solution_common/prestudy/prestudy_ai_report.jsp",
            vec![arg("analysis_key", 16)],
            vec![],
        ),
        op(
            "textbook.catalog",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "GaStudyAnswer")],
            vec![],
        ),
        op(
            "textbook.catalog.elementary",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "GaStudyAnswer"), lit("grade", "elementary")],
            vec![],
        ),
        op(
            "textbook.catalog.high",
            COURSE_NS,
            "GET",
            COURSE_PATH,
            vec![lit("reqCmd", "GaStudyAnswer"), lit("grade", "high")],
            vec![],
        ),
        op(
            "metadata.daily",
            METADATA_NS,
            "POST",
            DAILY_PATH,
            vec![lit("reqCmd", "DtResultSearch")],
            vec![arg("sort_date", 2), arg("stu_name", 800)],
        ),
        op(
            "metadata.paper.initial",
            METADATA_NS,
            "GET",
            PAPER_PATH,
            vec![lit("p_process", "UserByMain"), lit("ass_no", "1001")],
            vec![],
        ),
        op(
            "student.day.groups",
            DAY_NS,
            "POST",
            http::MAIN,
            vec![],
            vec![lit("p_process", "Main"), arg("std_ymd", 8)],
        ),
        op(
            "student.day.main",
            DAY_NS,
            "POST",
            http::MAIN,
            vec![],
            vec![
                lit("p_process", "Main"),
                arg("std_ymd", 8),
                arg("grp_seq", 16),
            ],
        ),
        op(
            "student.day.selected",
            DAY_NS,
            "GET",
            http::SELECTED,
            vec![
                lit("reqCmd", "SelSglRecord"),
                arg("course_seq", 16),
                scoped("stu_pri_no", ScopeField::StudentId),
                arg("record_seq", 16),
                arg("sel_grp_seq", 16),
                scoped("tut_pri_no", ScopeField::TeacherId),
                arg("cm_seq", 16),
            ],
            vec![],
        ),
    ];
    // Omit unchecked paper-type fields exactly as the observed form does;
    // never widen a default type selection or send a guessed empty value.
    for mask in 0..8 {
        let mut body = vec![arg("clg_no", 16), arg("cls_no", 64), lit("p_pageno", "1")];
        for (bit, k) in ["check_da_test", "check_fa_test", "check_na_test"]
            .iter()
            .enumerate()
        {
            if mask & (1 << bit) != 0 {
                body.push(arg(k, 64));
            }
        }
        body.extend([
            arg("stu_name", 800),
            arg("chioce_cls", 64),
            arg("sort_date1", 10),
            arg("sort_date2", 10),
        ]);
        contracts.push(op(
            &format!("metadata.paper.search.{mask}"),
            METADATA_NS,
            "POST",
            PAPER_PATH,
            vec![
                lit("p_process", "UserBySearchTestResult"),
                lit("ass_no", "1001"),
            ],
            body,
        ));
    }
    contracts
}
pub(crate) fn check_contract(p: &TransportAdmission, id: &str) -> Result<()> {
    let expected = read_contracts()
        .into_iter()
        .find(|o| o.id == id)
        .ok_or_else(|| Error::refusal("student_read_operation_not_supported"))?;
    require(
        p.operations.iter().any(|o| {
            o.id == id && serde_json::to_value(o).ok() == serde_json::to_value(&expected).ok()
        }),
        "student_read_contract_not_admitted",
    )
}
pub(crate) fn response_text(r: Response) -> Result<String> {
    require(
        r.status == 200 && !r.stalled,
        "student_read_source_rejected",
    )?;
    let html = decode_bounded(&r.bytes, 8_000_000, 2_000_000).map_err(|refusal| {
        Error::refusal(match refusal {
            DecodeRefusal::TooLarge => "student_read_source_rejected",
            DecodeRefusal::NotUtf8 => "student_read_encoding_rejected",
        })
    })?;
    let d = Document::parse(&html);
    require(
        !d.select("input").iter().any(|e| {
            e.value()
                .attr("type")
                .is_some_and(|t| t.eq_ignore_ascii_case("password"))
        }) && !html.contains("webUserPass")
            && !html.contains("세션이 만료"),
        "authentication_required",
    )?;
    Ok(html)
}
pub(crate) fn field(d: &Document, selector: &str) -> Result<String> {
    let nodes = d.select(selector);
    require(nodes.len() == 1, "student_read_source_identity_conflict")?;
    if let Some(form) = dom::closest(nodes[0], "form") {
        verify_form_ownership(d, form)?;
    }
    Ok(dom::value(nodes[0]))
}
/// The source contracts qualify ancestor-owned controls only. Explicit HTML
/// ownership overrides (including external associated controls) need a new pin.
pub(crate) fn verify_form_ownership(d: &Document, form: scraper::ElementRef<'_>) -> Result<()> {
    let id = form.value().attr("id");
    if let Some(id) = id {
        require(
            d.select("[id]")
                .iter()
                .filter(|e| e.value().attr("id") == Some(id))
                .count()
                == 1,
            "contract_drift",
        )?;
    }
    require(
        d.select("[form]").iter().all(|e| {
            !e.ancestors().any(|a| a.id() == form.id())
                && id.is_none_or(|id| e.value().attr("form") != Some(id))
        }),
        "contract_drift",
    )
}
pub(crate) fn course_principal(d: &Document, owner: &Owner) -> Result<String> {
    require(
        field(d, "form[name=form1] [name=g_pri_no]")? == owner.teacher_key
            && field(d, "form[name=form1] [name=g_clg_no]")? == owner.college_key,
        "student_read_discovery_scope_conflict",
    )?;
    let member = field(d, "form[name=form1] [name=g_mem_type]")?;
    require(
        !member.is_empty() && member.len() <= 200,
        "student_read_discovery_scope_conflict",
    )?;
    Ok(member)
}
/// Passive tuple parsing. Mutation handler strings are never executed or returned.
pub fn bind_course_row(row: &Js, owner: &Owner, student: &Student) -> Result<Js> {
    require(
        owner.student(&student.student_name)?.student_key == student.student_key,
        "course_source_identity_conflict",
    )?;
    let cells = row.get("cells");
    let handlers = row.get("handlers");
    require(
        array(cells)
            && cells.array().len() == 7
            && cells
                .array()
                .iter()
                .all(|c| c.string() && c.units().len() <= 2000)
            && array(handlers)
            && (2..=3).contains(&handlers.array().len()),
        "course_row_structure_drift",
    )?;
    // Both two- and three-control variants must bind the exact displayed teacher/name.
    require(
        cells.array()[1].is(&owner.teacher_label) && cells.array()[2].is(&student.student_name),
        "course_source_identity_conflict",
    )?;
    let re=regex::Regex::new(r"^\s*(onChangeStatus|onResetCourse|onDeleteCourse)\(\s*([0-9]+(?:\s*,\s*[0-9]+)*)\s*\)\s*;?\s*$").expect("constant");
    let mut calls = std::collections::HashMap::new();
    for h in handlers.array() {
        require(
            h.string() && h.units().len() <= 500,
            "unsafe_or_duplicate_course_handler",
        )?;
        let s = string(h);
        let c = re
            .captures(&s)
            .ok_or_else(|| Error::refusal("unsafe_or_duplicate_course_handler"))?;
        let tuple = c[2]
            .split(',')
            .map(str::trim)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        require(
            calls.insert(c[1].to_string(), tuple).is_none(),
            "unsafe_or_duplicate_course_handler",
        )?;
    }
    let status = calls
        .get("onChangeStatus")
        .ok_or_else(|| Error::refusal("course_handler_signature_drift"))?;
    let del = calls
        .get("onDeleteCourse")
        .ok_or_else(|| Error::refusal("course_handler_signature_drift"))?;
    let reset = calls.get("onResetCourse");
    require(
        status.len() == 5 && del.len() == 4 && reset.is_none_or(|r| r.len() == 3),
        "course_handler_signature_drift",
    )?;
    require(
        reset.is_some() || (status[0] == "0" && cells.array()[5].is("시작됨")),
        "course_handler_signature_drift",
    )?;
    for id in &status[1..] {
        key(&id.as_str().into())?;
    }
    require(
        status[1] == owner.college_key
            && status[2] == student.student_key
            && del == &status[1..]
            && reset
                .is_none_or(|r| r == &[status[3].clone(), status[2].clone(), status[1].clone()]),
        "course_source_identity_conflict",
    )?;
    Ok(obj(&[
        (
            "identity",
            obj(&[
                ("sourceNamespace", "academy_student_course".into()),
                ("studentKey", status[2].as_str().into()),
                ("collegeKey", status[1].as_str().into()),
                ("courseKey", status[3].as_str().into()),
                ("userCourseKey", status[4].as_str().into()),
            ]),
        ),
        ("sourceCells", cells.clone()),
        (
            "coverage",
            "observed_row_not_all_assignments_or_pagination".into(),
        ),
        ("handlersInvoked", false.into()),
        ("teacherJudgment", Js::Null),
    ]))
}
pub(crate) fn courses(html: &str, owner: &Owner, student: &Student) -> Result<Vec<Js>> {
    let d = Document::parse(html);
    course_principal(&d, owner)?;
    let mut out = vec![];
    let mut seen = HashSet::new();
    for row in d.select("tr") {
        let handlers = dom::select(row, "[onclick]")
            .iter()
            .filter_map(|e| e.value().attr("onclick"))
            .filter(|s| {
                s.contains("onChangeStatus")
                    || s.contains("onResetCourse")
                    || s.contains("onDeleteCourse")
            })
            .map(Js::from)
            .collect::<Vec<_>>();
        if handlers.is_empty() {
            continue;
        }
        require(out.len() < 1000, "student_read_discovery_row_limit")?;
        let cells = dom::select(row, ":scope > td")
            .iter()
            .map(|e| dom::content(*e).trim().to_string().into())
            .collect();
        let course = bind_course_row(
            &obj(&[("cells", Js::Arr(cells)), ("handlers", Js::Arr(handlers))]),
            owner,
            student,
        )?;
        require(
            seen.insert(json(course.get("identity"))),
            "student_read_discovery_identity_conflict",
        )?;
        out.push(course);
    }
    require(
        !out.is_empty(),
        "student_read_discovery_binding_unavailable",
    )?;
    Ok(out)
}
pub(crate) fn books(html: &str, student: &Student, courses: &[Js]) -> Result<Vec<Js>> {
    let d = Document::parse(html);
    let mut out = vec![];
    let mut seen = HashSet::new();
    let re=regex::Regex::new(r#"^\s*createBookIncorrectPapers\(\s*['"]?([0-9]+)['"]?\s*,\s*['"]?([0-9]+)['"]?\s*,\s*['"]?([0-9]+)['"]?\s*\)\s*;?\s*$"#).expect("constant");
    for node in d.select("[onclick]") {
        let call = node.value().attr("onclick").unwrap_or("");
        if !call.contains("createBookIncorrectPapers") {
            continue;
        }
        let c = re
            .captures(call)
            .ok_or_else(|| Error::refusal("textbook_binding_source_unqualified"))?;
        for i in 1..=3 {
            key(&c[i].into())?;
        }
        require(
            c[3] == student.student_key
                && courses
                    .iter()
                    .any(|r| r.get("identity").get("courseKey").is(&c[1])),
            "textbook_binding_source_identity_conflict",
        )?;
        let row = dom::closest(node, "tr")
            .ok_or_else(|| Error::refusal("textbook_binding_source_unqualified"))?;
        let cells = dom::select(row, ":scope > td");
        require(
            cells.len() >= 2 && out.len() < 1000,
            "textbook_binding_source_unqualified",
        )?;
        let label = dom::content(cells[1])
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        name(&label.as_str().into())?;
        require(
            seen.insert((c[1].to_string(), c[2].to_string())),
            "textbook_binding_source_identity_conflict",
        )?;
        out.push(obj(&[
            ("courseKey", c[1].into()),
            ("bookNo", c[2].into()),
            ("studentKey", c[3].into()),
            ("bookLabel", label.into()),
            ("teacherJudgment", Js::Null),
        ]));
    }
    require(!out.is_empty(), "textbook_binding_source_unqualified")?;
    Ok(out)
}
/// Reuses the accepted day-record parser through an internal Client view. Its
/// apparent authentication only selects parsing: every wire call still enters
/// NativeSession::read and authenticates there. No second login/cookie owner.
pub(crate) struct DayTransport<'a, P: ReadPort> {
    pub port: &'a mut P,
    pub admission: &'a TransportAdmission,
    pub scope: TransportScope,
    pub requests: &'a mut usize,
}
impl<P: ReadPort> Transport for DayTransport<'_, P> {
    fn dispatch(&mut self, r: &Request) -> Result<Response> {
        for id in [
            "student.day.groups",
            "student.day.main",
            "student.day.selected",
        ] {
            check_contract(self.admission, id)?;
            let contract = self
                .admission
                .operations
                .iter()
                .find(|o| o.id == id)
                .expect("checked");
            if contract.method != r.method || contract.path != r.path {
                continue;
            }
            let mut args = vec![];
            for p in contract.query.iter().chain(&contract.body) {
                if let ParameterValue::Argument { name, .. } = &p.value
                    && let Some((_, v)) = r.query.iter().chain(&r.body).find(|(k, _)| k == &p.name)
                {
                    args.push((name.clone(), v.clone()));
                }
            }
            let Ok(expected) = self.admission.request(&self.scope, id, &args, false) else {
                continue;
            };
            if expected.query != r.query || expected.body != r.body {
                continue;
            }
            require(*self.requests < 256, "student_read_request_budget_exceeded")?;
            *self.requests += 1;
            return self.port.read(&self.scope, id, &args);
        }
        Err(Error::refusal("student_read_wire_scope_conflict"))
    }
}
