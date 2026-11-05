//! Invented raw pages and owner contracts only. No source/student originals.
use edu_academy_sources::{
    data::*,
    http::{self, Response},
    owner_admission::{TransportAdmission, TransportScope},
    student_course::{self, Owner, ReadPort, Student, read_contracts},
    student_metadata::{MetadataPolicy, PaperFormPin, StudentSource, metadata_rows},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};
const DATE: &str = "2040-01-04";
const NOW: &str = "2040-01-04T09:00:00.000Z";
fn js(s: &str) -> Js {
    parse(s).unwrap()
}
fn owner_value() -> Js {
    js(
        r#"{"ownerId":"fictional-owner","teacherLabel":"Fictional Teacher","teacherSourceKey":"99001","collegeSourceKey":"88001","students":[{"studentKey":"77001","studentName":"Fictional One"},{"studentKey":"77002","studentName":"Fictional Two"}]}"#,
    )
}
fn owner() -> Owner {
    Owner::parse(&owner_value()).unwrap()
}
fn policy_value() -> Js {
    js(
        r#"{"schemaVersion":"academy.metadata-admission/1","sourceNamespace":"academy.lms.metadata","teacherId":"99001","allowedFrom":"2040-01-01","allowedThrough":"2040-01-31","studentScope":"configured","relativeSelectors":["1","11","12","13","14","15","16"],"operations":["student_papers","daily_metadata","daily_inventory","daily_history","printed_paper_metadata"]}"#,
    )
}
fn admission() -> TransportAdmission {
    TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "fictional-owner".into(),
        teacher_id: "99001".into(),
        teacher_label: "Fictional Teacher".into(),
        college_id: "88001".into(),
        allowed_from: "2040-01-01".into(),
        allowed_through: "2040-01-31".into(),
        student_ids: vec!["77001".into(), "77002".into()],
        source_namespaces: vec![
            student_course::COURSE_NS.into(),
            student_course::METADATA_NS.into(),
            student_course::DAY_NS.into(),
        ],
        operations: read_contracts(),
    }
}

fn request(q: Js, named: bool) -> Js {
    let mut v = obj(&[
        ("purpose", "Fictional metadata read".into()),
        ("currentRequestRef", "fictional-request".into()),
        ("queries", Js::Arr(vec![q])),
    ]);
    if named {
        v.set("displayName", "Fictional One".into());
        v.set("lessonDate", DATE.into());
    } else {
        v.set(
            "scope",
            obj(&[
                ("sourceNamespace", student_course::METADATA_NS.into()),
                ("teacherId", "99001".into()),
                ("studentId", "77001".into()),
                ("lessonDate", DATE.into()),
            ]),
        );
    }
    v
}
fn invoke<P: ReadPort>(source: &mut StudentSource<P>, q: Js) -> Result<Js> {
    source.metadata("academy_student_metadata", &request(q, false), NOW)
}
fn response(body: &str) -> Response {
    Response {
        status: 200,
        headers: vec![],
        bytes: body.as_bytes().to_vec(),
        url: http::ORIGIN.into(),
        stalled: false,
    }
}
struct Fake {
    policy: TransportAdmission,
    replies: VecDeque<(&'static str, String)>,
    count: Rc<Cell<usize>>,
    seen: Capture,
}
impl ReadPort for Fake {
    fn transport_admission(&self) -> &TransportAdmission {
        &self.policy
    }
    fn read(
        &mut self,
        s: &TransportScope,
        id: &str,
        args: &[(String, String)],
    ) -> Result<Response> {
        self.count.set(self.count.get() + 1);
        self.seen
            .borrow_mut()
            .push((id.into(), args.to_vec(), s.student_id.clone()));
        let (expected, body) = self.replies.pop_front().expect("unexpected synthetic read");
        assert_eq!(id, expected);
        Ok(response(&body))
    }
}
type Capture = Rc<RefCell<Vec<(String, Vec<(String, String)>, String)>>>;
fn fake(replies: Vec<(&'static str, String)>) -> (Fake, Rc<Cell<usize>>, Capture) {
    let count = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(vec![]));
    (
        Fake {
            policy: admission(),
            replies: replies.into(),
            count: count.clone(),
            seen: seen.clone(),
        },
        count,
        seen,
    )
}
fn source(replies: Vec<(&'static str, String)>) -> (StudentSource<Fake>, Rc<Cell<usize>>, Capture) {
    let (port, count, seen) = fake(replies);
    (
        StudentSource::new(
            port,
            owner(),
            admission(),
            MetadataPolicy::parse(&policy_value()).unwrap(),
            Some(form_pin()),
        )
        .unwrap(),
        count,
        seen,
    )
}
fn row(student: &str, name: &str, paper: &str, attempt: &str) -> Js {
    obj(&[
        ("priNo", student.into()),
        ("userName", name.into()),
        ("pNo", paper.into()),
        ("testingNo", attempt.into()),
        ("score", 100.into()),
        ("_id", "fictional-row".into()),
        ("title", "Fictional source title".into()),
        ("privatePayload", "NEVER_EXPORT".into()),
    ])
}
fn result(rows: Vec<Js>) -> String {
    let raw = json(&Js::Arr(rows))
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    format!(
        "<select name=sort_date><option value=1>Recent</option>{}</select><script>const results={{testResultList:JSON.parse('{raw}')}};throw Error('NEVER_EXECUTE');</script>",
        (11..=16)
            .map(|n| format!("<option value={n}>Fictional {n}</option>"))
            .collect::<String>()
    )
}
fn daily() -> String {
    result(vec![
        row("77001", "Fictional One", "3", "81001"),
        row("77002", "Fictional Two", "4", "81002"),
    ])
}
fn principal() -> String {
    "<form name=userForm><input name=pri_no value=99001></form><form name=form1><input name=g_pri_no value=99001><input name=g_clg_no value=88001><input name=g_mem_type value=teacher></form>".into()
}
fn form_value() -> Js {
    let fields = [
        ("clg_no", "input", Some("hidden"), Some("88001")),
        ("cls_no", "input", Some("hidden"), None),
        ("p_pageno", "input", Some("hidden"), None),
        ("check_da_test", "input", Some("checkbox"), Some("D")),
        ("check_fa_test", "input", Some("checkbox"), Some("F")),
        ("check_na_test", "input", Some("checkbox"), Some("N")),
        ("stu_name", "input", Some("text"), None),
        ("chioce_cls", "select", None, Some("0")),
        ("sort_date1", "input", Some("text"), None),
        ("sort_date2", "input", Some("text"), None),
    ]
    .iter()
    .map(|(n, t, ty, v)| {
        obj(&[
            ("name", (*n).into()),
            ("tag", (*t).into()),
            ("type", ty.map_or(Js::Null, Js::from)),
            ("value", v.map_or(Js::Null, Js::from)),
            (
                "checked",
                if *ty == Some("checkbox") {
                    (*n == "check_da_test").into()
                } else {
                    Js::Null
                },
            ),
            (
                "options",
                if *t == "select" {
                    Js::Arr(vec![obj(&[
                        ("value", "0".into()),
                        ("label", "All fictional classes".into()),
                    ])])
                } else {
                    Js::Arr(vec![])
                },
            ),
        ])
    })
    .collect();
    obj(&[
        ("schemaVersion", "academy.paper-search-form/1".into()),
        (
            "action",
            format!(
                "{}?p_process=UserBySearchTestResult&ass_no=1001",
                student_course::PAPER_PATH
            )
            .into(),
        ),
        ("method", "post".into()),
        ("fields", Js::Arr(fields)),
    ])
}
fn form_pin() -> PaperFormPin {
    PaperFormPin::parse(&form_value()).unwrap()
}
fn paper_form() -> String {
    format!(
        r#"{}<form name="search" action="{}?p_process=UserBySearchTestResult&amp;ass_no=1001" method="post"><input name="clg_no" type="hidden" value="88001"><input name="cls_no" type="hidden" value="0"><input name="p_pageno" type="hidden" value="4"><input name="check_da_test" type="checkbox" value="D" checked><input name="check_fa_test" type="checkbox" value="F"><input name="check_na_test" type="checkbox" value="N"><input name="stu_name" type="text"><select name="chioce_cls"><option value="0">All fictional classes</option></select><input name="sort_date1" type="text" value="2000-01-01"><input name="sort_date2" type="text" value="2000-01-02"></form><script>function onSearchPage(){{throw Error('NEVER_EXECUTE');}}</script>"#,
        principal(),
        student_course::PAPER_PATH
    )
}
fn paper_result(rows: Vec<Js>) -> String {
    principal() + &result(rows)
}
fn period_query(kind: &str) -> Js {
    obj(&[
        ("kind", kind.into()),
        (
            "period",
            obj(&[
                ("from", "2040-01-01".into()),
                ("through", "2040-01-04".into()),
            ]),
        ),
    ])
}
fn course_html(name: &str, id: &str) -> String {
    format!("{}<table>{}</table>",principal(),["5","6"].iter().map(|c|format!(r#"<tr><td>Fictional course {c}</td><td>Fictional Teacher</td><td>{name}</td><td>100%</td><td>Fictional</td><td>시작됨</td><td><a onclick="onChangeStatus(0,88001,{id},{c},9{c})"></a><a onclick="onDeleteCourse(88001,{id},{c},9{c})"></a></td></tr>"#)).collect::<String>())
}
fn book_html(id: &str) -> String {
    ["5","6"].iter().map(|c|format!(r#"<table><tr><td>Fictional</td><td>Fictional book {c}</td><td><a onclick="createBookIncorrectPapers('{c}','8{c}','{id}')"></a></td></tr></table>"#)).collect()
}
fn day_main(name: &str, id: &str) -> String {
    format!(
        r#"<input name=std_date value="{DATE}"><input name=teacher_pri_no value=99001><select name=grp_seq><option value=3 selected>Fictional group</option></select>
<table><thead><tr><th rowspan=2>이름</th><th rowspan=2>과정</th><th rowspan=2>지속사항</th><th colspan=4>지난진도</th><th colspan=6>일괄 발송 금일진도</th></tr><tr><th>지난수업일</th><th>진도</th><th>숙제</th><th>메모</th><th></th><th>금일출석</th><th>일일테스트</th><th>진도</th><th>숙제</th><th>메모</th></tr></thead><tbody><tr><td><a onclick="onOpenDaysMain('{id}')">{name}</a></td><td>Fictional course</td><td></td><td></td><td></td><td></td><td></td><td></td><td><select onchange="onAttn('0','5','{id}','13','8')"><option value=Y>Fictional</option></select></td><td></td><td><textarea onfocusout="onFoPrg('0','5','{id}','13','8')">100%</textarea></td><td><textarea onfocusout="onFoHw('0','5','{id}','13','8')">Fictional</textarea></td><td><input onfocusout="onFoMm('0','5','{id}','13','8')" value="Fictional memo"></td></tr></tbody></table>
<script>function onAttn(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoPrg(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoHw(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoMm(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onOpenDaysMain(student_pri_no){{}}</script>"#
    )
}
fn selected(name: &str, id: &str) -> String {
    let fields = ["", DATE, "5", "", name, "", "", "", "", "3", "13", id, "8"];
    format!("<root><record>{}</record></root>", fields.join("!"))
}
fn assert_code<T>(result: Result<T>, code: &str) {
    let e = result.err().expect("must refuse");
    assert_eq!(e.message, code);
    assert_eq!(e.properties.get("code"), &code.into());
}

#[test]
fn all_five_queries_project_exact_rows_and_keep_teacher_judgment_separate() {
    for named in [false, true] {
        let mut replies = vec![];
        if named {
            let main = day_main("Fictional One", "77001");
            replies.extend([
                ("student.day.groups", main.clone()),
                ("student.day.main", main),
                ("student.day.selected", selected("Fictional One", "77001")),
            ]);
        }
        replies.extend(vec![
            ("metadata.paper.initial", paper_form()),
            (
                "metadata.paper.search.1",
                paper_result(vec![row("77001", "Fictional One", "3", "81001")]),
            ),
            ("metadata.daily", daily()),
            ("metadata.daily", daily()),
            ("metadata.daily", daily()),
            ("metadata.daily", daily()),
            ("metadata.daily", daily()),
            ("metadata.paper.initial", paper_form()),
            (
                "metadata.paper.search.1",
                paper_result(vec![row("77001", "Fictional One", "3", "81001")]),
            ),
        ]);
        let (mut src, count, seen) = source(replies);
        let mut printed = period_query("printed_paper_metadata");
        printed.set("paperCode", "#5".into()); // alphabet index 3
        let queries = vec![
            period_query("student_papers"),
            js(r#"{"kind":"daily_metadata"}"#),
            js(r#"{"kind":"daily_inventory"}"#),
            js(r#"{"kind":"daily_history","periods":["11","12"]}"#),
            printed,
        ];
        let mut input = request(queries[0].clone(), named);
        input.set("queries", Js::Arr(queries));
        let result = src
            .metadata(
                if named {
                    "academy_student_metadata_get"
                } else {
                    "academy_student_metadata"
                },
                &input,
                NOW,
            )
            .unwrap();
        assert_eq!(result.get("results").array().len(), 5);
        assert_eq!(count.get(), if named { 12 } else { 9 });
        for r in result.get("results").array() {
            assert_eq!(r.get("observation").get("teacherJudgment"), &Js::Null);
        }
        let json = json(&result);
        for excluded in [
            "NEVER_EXPORT",
            "NEVER_EXECUTE",
            "handlers",
            "filterContracts",
            "form1",
            "privatePayload",
        ] {
            assert!(!json.contains(excluded));
        }
        assert!(json.contains("not_complete_history"));
        let capture = seen.borrow();
        assert!(capture.iter().all(|(_, _, id)| id == "77001"));
        let paper = &capture[if named { 4 } else { 1 }].1;
        assert!(paper.contains(&("sort_date1".into(), "2040-01-01".into())));
        assert!(
            !paper
                .iter()
                .any(|(k, _)| k == "check_fa_test" || k == "check_na_test")
        );
    }
}

#[test]
fn named_alias_checks_dated_occurrence_before_metadata_in_one_port() {
    let main = day_main("Fictional One", "77001");
    let (mut src, count, seen) = source(vec![
        ("student.day.groups", main.clone()),
        ("student.day.main", main),
        ("student.day.selected", selected("Fictional One", "77001")),
        ("metadata.daily", daily()),
    ]);
    let result = src
        .metadata(
            "academy_student_metadata_get",
            &request(js(r#"{"kind":"daily_metadata"}"#), true),
            NOW,
        )
        .unwrap();
    assert_eq!(result.get("scope").get("studentId"), &"77001".into());
    assert_eq!(count.get(), 4);
    assert_eq!(seen.borrow()[0].0, "student.day.groups");
}
#[test]
fn all_relative_selectors_preserve_order_and_never_infer_calendar_bounds() {
    let mut replies = vec![("metadata.daily", daily())];
    replies.extend((0..6).map(|_| ("metadata.daily", daily())));
    let (mut src, count, _) = source(replies);
    let out = invoke(
        &mut src,
        js(r#"{"kind":"daily_history","periods":["16","15","14","13","12","11"]}"#),
    )
    .unwrap();
    let obs = out.get("results").array()[0].get("observation");
    assert_eq!(count.get(), 7);
    assert_eq!(
        obs.get("observations").array()[0].get("selector"),
        &"16".into()
    );
    assert!(string(obs.get("calendarBounds")).contains("not_exact"));
}
#[test]
fn printed_lookup_explicit_empty_primary_can_read_daily_fallback() {
    let (mut src, count, _) = source(vec![
        ("metadata.paper.initial", paper_form()),
        ("metadata.paper.search.1", paper_result(vec![])),
        ("metadata.daily", daily()),
    ]);
    let mut q = period_query("printed_paper_metadata");
    q.set("paperCode", "#5".into());
    let out = invoke(&mut src, q).unwrap();
    let o = out.get("results").array()[0].get("observation");
    assert_eq!(o.get("matchedSourceFamily"), &"daily_zero".into());
    assert_eq!(o.get("sourcePeriod"), &Js::Null);
    assert_eq!(o.get("searchedSources").array().len(), 2);
    assert_eq!(count.get(), 3);
}
#[test]
fn printed_miss_is_only_not_found_in_scope() {
    let (mut src, _, _) = source(vec![
        ("metadata.paper.initial", paper_form()),
        ("metadata.paper.search.1", paper_result(vec![])),
        ("metadata.daily", result(vec![])),
    ]);
    let mut q = period_query("printed_paper_metadata");
    q.set("paperCode", "#5".into());
    let out = invoke(&mut src, q).unwrap();
    let o = out.get("results").array()[0].get("observation");
    assert_eq!(o.get("status"), &"not_found_in_scope".into());
    assert_eq!(o.get("matchedSourceFamily"), &Js::Null);
}
#[test]
fn complete_input_validation_precedes_any_batch_read() {
    for q in [
        r#"{"kind":"missing"}"#,
        r#"{"kind":"daily_metadata","periods":["11"]}"#,
        r#"{"kind":"daily_history","periods":[]}"#,
        r#"{"kind":"daily_history","periods":["11","11"]}"#,
        r#"{"kind":"daily_history","periods":["1"]}"#,
        r#"{"kind":"student_papers","period":{"from":"2040-01-04","through":"2040-01-01"}}"#,
    ] {
        let (mut src, count, _) = source(vec![]);
        let mut input = request(js(r#"{"kind":"daily_metadata"}"#), false);
        input.set(
            "queries",
            Js::Arr(vec![js(r#"{"kind":"daily_metadata"}"#), js(q)]),
        );
        assert_code(
            src.metadata("academy_student_metadata", &input, NOW),
            "tool_input_invalid",
        );
        assert_eq!(count.get(), 0);
    }
}
#[test]
fn namespace_teacher_student_date_and_tool_refuse_before_io() {
    let mut cases = vec![];
    for (k, v) in [
        ("sourceNamespace", "academy.lms.dayrecord"),
        ("teacherId", "99002"),
        ("studentId", "77003"),
    ] {
        let mut input = request(js(r#"{"kind":"daily_metadata"}"#), false);
        let mut sc = input.get("scope").clone();
        sc.set(k, v.into());
        input.set("scope", sc);
        cases.push(input);
    }
    for input in cases {
        let (mut src, count, _) = source(vec![]);
        assert_code(
            src.metadata("academy_student_metadata", &input, NOW),
            "native_metadata_scope_refused",
        );
        assert_eq!(count.get(), 0);
    }
    let (mut src, count, _) = source(vec![]);
    assert_code(
        src.metadata(
            "unsupported",
            &request(js(r#"{"kind":"daily_metadata"}"#), false),
            NOW,
        ),
        "student_metadata_tool_not_supported",
    );
    assert_eq!(count.get(), 0);
}
#[test]
fn missing_relative_selector_admission_prevents_even_current_page() {
    let p = policy_value();
    let mut p = p;
    p.set("relativeSelectors", Js::Arr(vec!["11".into()]));
    let (f, count, _) = fake(vec![]);
    let mut src = StudentSource::new(
        f,
        owner(),
        admission(),
        MetadataPolicy::parse(&p).unwrap(),
        Some(form_pin()),
    )
    .unwrap();
    assert_code(
        invoke(&mut src, js(r#"{"kind":"daily_history","periods":["11"]}"#)),
        "metadata_namespace_not_admitted",
    );
    assert_eq!(count.get(), 0);
}
#[test]
fn period_date_must_fit_both_metadata_and_transport_admission() {
    let (mut src, count, _) = source(vec![]);
    let q =
        js(r#"{"kind":"student_papers","period":{"from":"2039-12-31","through":"2040-01-04"}}"#);
    assert_code(invoke(&mut src, q), "metadata_namespace_not_admitted");
    assert_eq!(count.get(), 0);
    let mut p = policy_value();
    p.set("allowedFrom", Js::Null);
    let (f, count, _) = fake(vec![]);
    let mut src = StudentSource::new(
        f,
        owner(),
        admission(),
        MetadataPolicy::parse(&p).unwrap(),
        Some(form_pin()),
    )
    .unwrap();
    assert_eq!(
        invoke(
            &mut src,
            js(
                r#"{"kind":"student_papers","period":{"from":"2039-12-31","through":"2040-01-04"}}"#
            )
        )
        .unwrap_err()
        .message,
        "native_transport_scope_refused"
    );
    assert_eq!(count.get(), 0);
}
#[test]
fn owner_scope_is_pure_configuration_and_zero_requests() {
    let (mut src, count, _) = source(vec![]);
    let out = src
        .observe(
            &js(r#"{"operation":"owner_scope"}"#),
            "invalid-unused",
            "invalid-unused",
        )
        .unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(out.get("requests"), &0.into());
    assert_eq!(
        out.get("page").get("authentication"),
        &"not_attempted".into()
    );
    assert!(string(out.get("page").get("coverage")).contains("not_verified"));
}
#[test]
fn multiple_courses_and_books_are_exact_bound_observations() {
    for (name, id) in [("Fictional One", "77001"), ("Fictional Two", "77002")] {
        let (mut src, count, _) = source(vec![
            ("student.course.initial", principal()),
            ("student.course.search", course_html(name, id)),
            ("student.books", book_html(id)),
        ]);
        let out = src
            .observe(
                &obj(&[
                    ("operation", "discover_books".into()),
                    ("studentName", name.into()),
                ]),
                DATE,
                NOW,
            )
            .unwrap();
        let page = out.get("page");
        assert_eq!(page.get("courses").array().len(), 2);
        assert_eq!(page.get("books").array().len(), 2);
        assert_eq!(page.get("teacherJudgment"), &Js::Null);
        assert_eq!(count.get(), 3);
        assert!(!json(page).contains("onclick"));
    }
}
#[test]
fn course_only_does_not_fetch_books() {
    let (mut src, count, _) = source(vec![
        ("student.course.initial", principal()),
        (
            "student.course.search",
            course_html("Fictional One", "77001"),
        ),
    ]);
    let out = src
        .observe(
            &js(r#"{"operation":"student_course","studentName":"Fictional One"}"#),
            DATE,
            NOW,
        )
        .unwrap();
    assert_eq!(count.get(), 2);
    assert_eq!(out.get("page").get("books"), &Js::Undefined);
}
#[test]
fn wrong_course_teacher_college_student_and_unsafe_handler_refuse() {
    for (from, to, code) in [
        (
            "Fictional Teacher",
            "Other Teacher",
            "course_source_identity_conflict",
        ),
        ("88001", "88002", "student_read_discovery_scope_conflict"),
        ("77001", "77002", "course_source_identity_conflict"),
        (
            "onDeleteCourse(88001,77001,5,95)",
            "onDeleteCourse(88001,77001,5,95);evil()",
            "unsafe_or_duplicate_course_handler",
        ),
    ] {
        let html = course_html("Fictional One", "77001").replace(from, to);
        let (mut src, _, _) = source(vec![
            ("student.course.initial", principal()),
            ("student.course.search", html),
        ]);
        assert_code(
            src.observe(
                &js(r#"{"operation":"student_course","studentName":"Fictional One"}"#),
                DATE,
                NOW,
            ),
            code,
        );
    }
}
#[test]
fn book_wrong_student_unknown_course_and_duplicate_binding_refuse() {
    for html in [
        book_html("77002"),
        book_html("77001").replace("'5'", "'7'"),
        book_html("77001") + &book_html("77001"),
    ] {
        let (mut src, _, _) = source(vec![
            ("student.course.initial", principal()),
            (
                "student.course.search",
                course_html("Fictional One", "77001"),
            ),
            ("student.books", html),
        ]);
        assert_code(
            src.observe(
                &js(r#"{"operation":"discover_books","studentName":"Fictional One"}"#),
                DATE,
                NOW,
            ),
            "textbook_binding_source_identity_conflict",
        );
    }
}
#[test]
fn non_started_three_handler_course_checks_displayed_identity() {
    let row = js(
        r#"{"cells":["course","Fictional Teacher","Fictional One","","","completed",""],"handlers":["onChangeStatus(1,88001,77001,5,95)","onResetCourse(5,77001,88001)","onDeleteCourse(88001,77001,5,95)"]}"#,
    );
    let s = Student {
        student_key: "77001".into(),
        student_name: "Fictional One".into(),
    };
    assert!(student_course::bind_course_row(&row, &owner(), &s).is_ok());
    let mut wrong = row;
    let mut cells = wrong.get("cells").array().to_vec();
    cells[1] = "Other Teacher".into();
    wrong.set("cells", Js::Arr(cells));
    assert_code(
        student_course::bind_course_row(&wrong, &owner(), &s),
        "course_source_identity_conflict",
    );
}
#[test]
fn observation_outside_owner_unknown_operation_and_over_scope_refuse_without_io() {
    for input in [
        js(r#"{"operation":"student_course","studentName":"Other"}"#),
        js(r#"{"operation":"discover_books","studentName":"Other"}"#),
    ] {
        let (mut src, count, _) = source(vec![]);
        assert_code(
            src.observe(&input, DATE, NOW),
            "student_read_outside_owner_scope",
        );
        assert_eq!(count.get(), 0);
    }
    let (mut src, _, _) = source(vec![]);
    assert_code(
        src.observe(&js(r#"{"operation":"write"}"#), DATE, NOW),
        "student_read_operation_not_supported",
    );
}
#[test]
fn roster_200_is_hard_bound_and_201_never_truncates() {
    let mut v = owner_value();
    let roster = (0..200)
        .map(|i| {
            obj(&[
                ("studentKey", (77001 + i).to_string().into()),
                ("studentName", format!("Fictional {i}").into()),
            ])
        })
        .collect::<Vec<_>>();
    v.set("students", Js::Arr(roster.clone()));
    assert_eq!(
        Owner::parse(&v)
            .unwrap()
            .scope_page()
            .get("studentNames")
            .array()
            .len(),
        200
    );
    let mut over = roster;
    over.push(obj(&[
        ("studentKey", "77999".into()),
        ("studentName", "Fictional overbound".into()),
    ]));
    v.set("students", Js::Arr(over));
    assert_code(Owner::parse(&v), "student_read_owner_scope_invalid");
}
#[test]
fn duplicate_names_ids_and_unsafe_numeric_source_keys_refuse() {
    for students in [
        r#"[{"studentKey":"77001","studentName":"Fictional One"},{"studentKey":"77002","studentName":"Fictional One"}]"#,
        r#"[{"studentKey":"77001","studentName":"Fictional One"},{"studentKey":"77001","studentName":"Fictional Two"}]"#,
    ] {
        let mut v = owner_value();
        v.set("students", js(students));
        assert_code(Owner::parse(&v), "student_read_owner_scope_invalid");
    }
    let mut v = owner_value();
    v.set("teacherSourceKey", "9007199254740992".into());
    assert_code(Owner::parse(&v), "student_read_source_key_invalid");
}
#[test]
fn transport_owner_teacher_college_and_roster_must_match_exactly() {
    for field in ["owner", "teacher", "college", "roster"] {
        let mut p = admission();
        match field {
            "owner" => p.owner_id = "other".into(),
            "teacher" => p.teacher_id = "99002".into(),
            "college" => p.college_id = "88002".into(),
            _ => p.student_ids.pop().map(|_| ()).unwrap(),
        };
        let (f, count, _) = fake(vec![]);
        assert_code(
            StudentSource::new(
                f,
                owner(),
                p,
                MetadataPolicy::parse(&policy_value()).unwrap(),
                Some(form_pin()),
            ),
            "student_read_owner_transport_conflict",
        );
        assert_eq!(count.get(), 0);
    }
}
#[test]
fn metadata_wrong_name_duplicate_attempt_and_source_record_refuse() {
    let s = Student {
        student_key: "77001".into(),
        student_name: "Fictional One".into(),
    };
    assert_code(
        metadata_rows(&result(vec![row("77001", "Other", "3", "81001")]), &s),
        "student_search_identity_conflict",
    );
    let r = row("77001", "Fictional One", "3", "81001");
    assert_code(
        metadata_rows(&result(vec![r.clone(), r]), &s),
        "duplicate_student_search_row",
    );
    let mut bad = row("77001", "Fictional One", "3", "81001");
    bad.set("testingNo", Js::Null);
    assert_code(metadata_rows(&result(vec![bad]), &s), "invalid_source_key");
}
#[test]
fn comments_strings_external_and_duplicate_lists_cannot_spoof_raw_metadata() {
    let s = Student {
        student_key: "77001".into(),
        student_name: "Fictional One".into(),
    };
    for html in [
        "<script>// testResultList:JSON.parse('[]')</script>",
        "<script>const s=\"testResultList:JSON.parse('[]')\";</script>",
        "<script src=foreign>const x={testResultList:JSON.parse('[]')}</script>",
    ] {
        assert_code(metadata_rows(html, &s), "result_list_missing_or_ambiguous");
    }
    assert_code(
        metadata_rows(&(result(vec![]) + &result(vec![])), &s),
        "result_list_missing_or_ambiguous",
    );
}
#[test]
fn source_failure_latches_all_later_reads_and_no_fallback() {
    let (mut src, count, _) = source(vec![
        ("metadata.paper.initial", paper_form()),
        (
            "metadata.paper.search.1",
            "<script>const invalid=1;</script>".into(),
        ),
    ]);
    let err = invoke(&mut src, period_query("student_papers")).unwrap_err();
    assert!(matches!(
        err.message.as_str(),
        "student_read_source_identity_conflict" | "student_paper_owner_drift"
    ));
    assert_code(
        invoke(&mut src, js(r#"{"kind":"daily_metadata"}"#)),
        "source_failure_latched_no_retry",
    );
    assert_eq!(count.get(), 2);
}
#[test]
fn paper_form_drift_has_typed_failure_before_search_or_fallback() {
    let base = paper_form();
    for html in [
        base.replace("name=\"stu_name\"", "name=\"renamed\""),
        base.replace("p_process=UserBySearchTestResult", "p_process=Write"),
        base.replace(
            "</form><script>",
            "<input name=extra_required required></form><script>",
        ),
        base.replace("<option value=\"0\">All fictional classes</option>", ""),
        base.replace("method=\"post\"", "method=\"get\""),
        base.replace("value=\"D\"", "value=\"Changed\""),
        base.replace("value=\"D\" checked", "value=\"D\""),
    ] {
        let (mut src, count, _) = source(vec![("metadata.paper.initial", html)]);
        assert_code(
            invoke(&mut src, period_query("student_papers")),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn missing_owner_paper_pin_and_changed_template_refuse_before_any_io() {
    let (f, count, _) = fake(vec![]);
    let mut src = StudentSource::new(
        f,
        owner(),
        admission(),
        MetadataPolicy::parse(&policy_value()).unwrap(),
        None,
    )
    .unwrap();
    assert_code(
        invoke(&mut src, period_query("student_papers")),
        "contract_drift",
    );
    assert_eq!(count.get(), 0);
    let mut p = admission();
    p.operations
        .iter_mut()
        .find(|o| o.id == "metadata.daily")
        .unwrap()
        .path = "/unexpected".into();
    let (mut f, count, _) = fake(vec![]);
    f.policy = TransportAdmission::from_bytes(&serde_json::to_vec(&p).unwrap()).unwrap();
    let mut src = StudentSource::new(
        f,
        owner(),
        p,
        MetadataPolicy::parse(&policy_value()).unwrap(),
        Some(form_pin()),
    )
    .unwrap();
    assert_code(
        invoke(&mut src, js(r#"{"kind":"daily_metadata"}"#)),
        "student_read_contract_not_admitted",
    );
    assert_eq!(count.get(), 0);
}
#[test]
fn mirrored_parameters_are_explicit_equal_and_unique_per_placement() {
    let p = admission();
    p.validate().unwrap();
    let op = p
        .operations
        .iter()
        .find(|o| o.id == "student.course.search")
        .unwrap();
    assert_eq!(
        op.mirrored_parameters,
        ["reqCmd", "student_name", "cls_no", "grade_no"]
    );
    for mode in [
        "different",
        "undeclared",
        "same_query",
        "same_body",
        "wrong_template",
    ] {
        let mut p = admission();
        let op = p
            .operations
            .iter_mut()
            .find(|o| o.id == "student.course.search")
            .unwrap();
        match mode {
            "different" => {
                op.body
                    .iter_mut()
                    .find(|v| v.name == "reqCmd")
                    .unwrap()
                    .value = edu_academy_sources::owner_admission::ParameterValue::Literal {
                    value: "Other".into(),
                }
            }
            "undeclared" => op.mirrored_parameters.clear(),
            "same_query" => op
                .query
                .push(serde_json::from_value(serde_json::to_value(&op.query[0]).unwrap()).unwrap()),
            "same_body" => op
                .body
                .push(serde_json::from_value(serde_json::to_value(&op.body[0]).unwrap()).unwrap()),
            _ => op.path = "/another".into(),
        };
        assert_eq!(
            p.validate().unwrap_err().message,
            "native_transport_scope_refused"
        );
    }
}
#[test]
fn old_transport_templates_serialize_without_mirror_field() {
    let p = admission();
    for o in p
        .operations
        .iter()
        .filter(|o| o.id != "student.course.search")
    {
        assert!(
            serde_json::to_value(o)
                .unwrap()
                .get("mirroredParameters")
                .is_none()
        );
    }
}
#[test]
fn day_record_uses_existing_parser_and_independent_record_identity() {
    let main = day_main("Fictional One", "77001");
    let (mut src, count, _) = source(vec![
        ("student.day.main", main.clone()),
        ("student.day.selected", selected("Fictional One", "77001")),
    ]);
    let input = js(
        r#"{"operation":"day_record","date":"2040-01-04","groupIds":["3"],"studentNames":["Fictional One"]}"#,
    );
    let out = src.observe(&input, DATE, NOW).unwrap();
    assert_eq!(
        out.get("page")
            .get("source")
            .get("dayRecords")
            .array()
            .len(),
        1
    );
    assert_eq!(count.get(), 2);
    let (mut src, count, _) = source(vec![
        ("student.day.main", main),
        ("student.day.selected", selected("Fictional One", "77002")),
    ]);
    assert_eq!(
        src.observe(&input, DATE, NOW).unwrap_err().message,
        "independent_occurrence_conflict"
    );
    assert_eq!(count.get(), 2);
}
#[test]
fn empty_day_scope_preserves_partial_coverage_without_inventing_students() {
    let empty = format!(
        "<input name=std_date value={DATE}><input name=teacher_pri_no value=99001><select name=grp_seq><option value=3 selected>Fictional</option></select>"
    );
    let (mut src, count, _) = source(vec![("student.day.main", empty)]);
    let input = js(
        r#"{"operation":"day_record","date":"2040-01-04","groupIds":["3"],"studentNames":["Fictional One"]}"#,
    );
    let out = src.observe(&input, DATE, NOW).unwrap();
    assert_eq!(
        out.get("page")
            .get("source")
            .get("dayRecords")
            .array()
            .len(),
        0
    );
    assert_eq!(count.get(), 1);
}

#[test]
fn native_session_uses_exact_mirrored_wire_one_authentication_and_releases_lease() {
    use edu_academy_core::workflow::Runtime;
    use edu_academy_sources::{
        http::{Cancellation, NativeTransport, TransportLimits},
        owner_root::OwnerRoot,
        owner_workflow::{Credentials, NativeSession},
    };
    use std::{
        fs,
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        os::unix::fs::PermissionsExt,
        thread,
        time::{Duration, Instant},
    };
    struct Clock;
    impl Runtime for Clock {
        fn now(&self) -> String {
            NOW.into()
        }
        fn uuid(&self) -> edu_academy_core::js::Result<String> {
            Ok("fictional-e09a".into())
        }
        fn epoch_ms(&self) -> u64 {
            1000
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
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
    let login = format!(
        r#"<form id="loginForm" method="post">{fields}</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>"#
    );
    let replies = vec![
        login,
        "accepted".into(),
        principal() + "<a>로그아웃</a>",
        principal(),
        course_html("Fictional One", "77001"),
        book_html("77001"),
        daily(),
    ];
    let server = thread::spawn(move || {
        let mut captures = vec![];
        let deadline = Instant::now() + Duration::from_secs(20);
        for reply in replies {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(v) => break v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "synthetic accept deadline");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("synthetic accept {e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':')
                    && k.eq_ignore_ascii_case("content-length")
                {
                    length = v.trim().parse::<usize>().unwrap();
                }
            }
            let mut bytes = vec![0; length];
            reader.read_exact(&mut bytes).unwrap();
            drop(reader);
            let target = first.split_whitespace().nth(1).unwrap();
            let url = url::Url::parse(&format!("http://{address}{target}")).unwrap();
            if !url.path().contains("login") && !url.path().contains("LoginServlet") {
                captures.push((
                    url.path().to_string(),
                    url.query_pairs()
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect::<Vec<_>>(),
                    http::parse_form(std::str::from_utf8(&bytes).unwrap()),
                ));
            }
            drop(bytes);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            )
            .unwrap();
        }
        captures
    });
    let parent = std::path::PathBuf::from(
        std::env::var("S09_NATIVE_FIXTURE_ROOT").expect("explicit synthetic root"),
    );
    let path = parent.join(format!("e09a-native-{}", std::process::id()));
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let root = Rc::new(OwnerRoot::open(&path).unwrap());
    let mut random = [0u8; 32];
    fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut random)
        .unwrap();
    let username = hash(&random);
    random.reverse();
    let password = hash(&random);
    let account = hash(format!("{}\0{}", http::ORIGIN, username).as_bytes());
    let clock = Clock;
    let cancellation = Cancellation::default();
    let native = NativeTransport::synthetic_loopback(
        address,
        TransportLimits::default(),
        cancellation.clone(),
    )
    .unwrap();
    let session = NativeSession::open(
        root.clone(),
        admission(),
        native,
        || Ok(Credentials::new(username.clone(), password.clone())),
        cancellation,
        &clock,
    )
    .unwrap();
    let mut src = StudentSource::new(
        session,
        owner(),
        admission(),
        MetadataPolicy::parse(&policy_value()).unwrap(),
        Some(form_pin()),
    )
    .unwrap();
    let pure = src
        .observe(&js(r#"{"operation":"owner_scope"}"#), DATE, NOW)
        .unwrap();
    assert_eq!(pure.get("requests"), &0.into());
    let books = src
        .observe(
            &js(r#"{"operation":"discover_books","studentName":"Fictional One"}"#),
            DATE,
            NOW,
        )
        .unwrap();
    assert_eq!(books.get("page").get("books").array().len(), 2);
    invoke(&mut src, js(r#"{"kind":"daily_inventory"}"#)).unwrap();
    assert_eq!(src.requests(), 4);
    drop(src);
    drop(
        root.account(&account)
            .expect("native session released the exact account"),
    );
    let captured = server.join().unwrap();
    let mirrored = captured
        .iter()
        .find(|(_, q, _)| q.iter().any(|(k, _)| k == "student_name"))
        .unwrap();
    for key in ["reqCmd", "student_name", "cls_no", "grade_no"] {
        assert_eq!(
            mirrored
                .1
                .iter()
                .find(|(k, _)| k == key)
                .unwrap()
                .1
                .as_bytes(),
            mirrored
                .2
                .iter()
                .find(|(k, _)| k == key)
                .unwrap()
                .1
                .as_bytes()
        );
    }
    assert_eq!(captured.len(), 5);
    drop(root);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn day_record_multiple_names_exact_scopes_and_explicit_budget_refusal() {
    let (mut src, count, seen) = source(vec![
        ("student.day.main", day_main("Fictional One", "77001")),
        ("student.day.selected", selected("Fictional One", "77001")),
        ("student.day.main", day_main("Fictional Two", "77002")),
        ("student.day.selected", selected("Fictional Two", "77002")),
    ]);
    let input = js(
        r#"{"operation":"day_record","date":"2040-01-04","groupIds":["3"],"studentNames":["Fictional One","Fictional Two"]}"#,
    );
    let out = src.observe(&input, DATE, NOW).unwrap();
    assert_eq!(
        out.get("page")
            .get("source")
            .get("dayRecords")
            .array()
            .len(),
        2
    );
    assert_eq!(count.get(), 4);
    assert_eq!(seen.borrow()[2].2, "77002");
    for (field, values, code) in [
        (
            "groupIds",
            Js::Arr(vec!["3".into(), "3".into()]),
            "student_read_duplicate_scope",
        ),
        (
            "studentNames",
            Js::Arr(vec!["Fictional One".into(); 65]),
            "student_read_scope_size_invalid",
        ),
        (
            "groupIds",
            Js::Arr((1..=13).map(|n| n.to_string().into()).collect()),
            "student_read_scope_size_invalid",
        ),
    ] {
        let (mut src, count, _) = source(vec![]);
        let mut bad = input.clone();
        bad.set(field, values);
        assert_code(src.observe(&bad, DATE, NOW), code);
        assert_eq!(count.get(), 0);
    }
}
#[test]
fn failed_read_is_latched_with_no_automatic_retry_or_cross_family_fallthrough() {
    struct Cancelled {
        count: Rc<Cell<usize>>,
        policy: TransportAdmission,
    }
    impl ReadPort for Cancelled {
        fn transport_admission(&self) -> &TransportAdmission {
            &self.policy
        }
        fn read(
            &mut self,
            _: &TransportScope,
            _: &str,
            _: &[(String, String)],
        ) -> Result<Response> {
            self.count.set(self.count.get() + 1);
            Err(Error::refusal("native_transport_cancelled"))
        }
    }
    let count = Rc::new(Cell::new(0));
    let mut src = StudentSource::new(
        Cancelled {
            count: count.clone(),
            policy: admission(),
        },
        owner(),
        admission(),
        MetadataPolicy::parse(&policy_value()).unwrap(),
        Some(form_pin()),
    )
    .unwrap();
    assert_code(
        invoke(&mut src, js(r#"{"kind":"daily_metadata"}"#)),
        "native_transport_cancelled",
    );
    assert_code(
        src.observe(
            &js(r#"{"operation":"student_course","studentName":"Fictional One"}"#),
            DATE,
            NOW,
        ),
        "source_failure_latched_no_retry",
    );
    assert_eq!(count.get(), 1);
}
#[test]
fn named_occurrence_missing_or_wrong_source_record_cannot_read_metadata() {
    for record in [
        selected("Fictional Two", "77001"),
        selected("Fictional One", "77002"),
        selected("Fictional One", "77001").replace("!13!", "!14!"),
    ] {
        let main = day_main("Fictional One", "77001");
        let (mut src, count, _) = source(vec![
            ("student.day.groups", main.clone()),
            ("student.day.main", main),
            ("student.day.selected", record),
        ]);
        assert_eq!(
            src.metadata(
                "academy_student_metadata_get",
                &request(js(r#"{"kind":"daily_metadata"}"#), true),
                NOW
            )
            .unwrap_err()
            .message,
            "independent_occurrence_conflict"
        );
        assert_eq!(count.get(), 3);
    }
    let empty = format!(
        "<input name=std_date value={DATE}><input name=teacher_pri_no value=99001><select name=grp_seq><option value=3>Fictional</option></select>"
    );
    let (mut src, count, _) = source(vec![
        ("student.day.groups", empty.clone()),
        ("student.day.main", empty),
    ]);
    assert_code(
        src.metadata(
            "academy_student_metadata_get",
            &request(js(r#"{"kind":"daily_metadata"}"#), true),
            NOW,
        ),
        "native_dailyrecord_occurrence_unverified",
    );
    assert_eq!(count.get(), 2);
}
#[test]
fn daily_selector_drift_and_row_bound_are_hard_refusals() {
    let (mut src, count, _) = source(vec![(
        "metadata.daily",
        daily().replace("value=11", "value=17"),
    )]);
    assert_code(
        invoke(&mut src, js(r#"{"kind":"daily_history","periods":["11"]}"#)),
        "daily_history_selector_drift",
    );
    assert_eq!(count.get(), 1);
    let student = Student {
        student_key: "77001".into(),
        student_name: "Fictional One".into(),
    };
    assert_code(
        metadata_rows(
            &result(
                (0..2001)
                    .map(|i| row("77001", "Fictional One", "3", &(81001 + i).to_string()))
                    .collect(),
            ),
            &student,
        ),
        "student_search_list_rejected",
    );
    assert_code(
        metadata_rows(
            &result(vec![row("77002", "Fictional One", "3", "81001")]),
            &student,
        ),
        "student_search_identity_conflict",
    );
}

#[test]
fn metadata_request_reference_keeps_live_safe_text_and_256_unit_bound() {
    for value in [
        "a".repeat(257),
        "fictional\u{0}reference".into(),
        "fictional\u{7f}reference".into(),
    ] {
        let (mut src, count, _) = source(vec![]);
        let mut input = request(js(r#"{"kind":"daily_metadata"}"#), false);
        input.set("currentRequestRef", value.into());
        assert_code(
            src.metadata("academy_student_metadata", &input, NOW),
            "tool_input_invalid",
        );
        assert_eq!(count.get(), 0);
    }
}

#[test]
fn native_owner_path_and_operation_set_mismatch_refuse_with_zero_loopback_requests() {
    use edu_academy_core::workflow::Runtime;
    use edu_academy_sources::{
        http::{Cancellation, NativeTransport, TransportLimits},
        owner_root::OwnerRoot,
        owner_workflow::{Credentials, NativeSession},
    };
    use std::{fs, io::Read, net::TcpListener, os::unix::fs::PermissionsExt};
    struct Clock;
    impl Runtime for Clock {
        fn now(&self) -> String {
            NOW.into()
        }
        fn uuid(&self) -> edu_academy_core::js::Result<String> {
            Ok("fictional-binding".into())
        }
        fn epoch_ms(&self) -> u64 {
            1000
        }
    }
    for mode in ["owner", "path", "operations"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let parent = std::path::PathBuf::from(
            std::env::var("S09_NATIVE_FIXTURE_ROOT").expect("explicit synthetic root"),
        );
        let path = parent.join(format!("e09a-mismatch-{}-{mode}", std::process::id()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let root = Rc::new(OwnerRoot::open(&path).unwrap());
        let mut actual = admission();
        match mode {
            "owner" => actual.owner_id = "another-owner".into(),
            "path" => {
                actual
                    .operations
                    .iter_mut()
                    .find(|o| o.id == "metadata.daily")
                    .unwrap()
                    .path = "/another-read".into()
            }
            _ => {
                actual.operations.pop();
            }
        }
        let mut random = [0u8; 32];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut random)
            .unwrap();
        let username = hash(&random);
        random.reverse();
        let password = hash(&random);
        let clock = Clock;
        let cancellation = Cancellation::default();
        let native = NativeTransport::synthetic_loopback(
            listener.local_addr().unwrap(),
            TransportLimits::default(),
            cancellation.clone(),
        )
        .unwrap();
        let session = NativeSession::open(
            root.clone(),
            actual,
            native,
            || Ok(Credentials::new(username.clone(), password.clone())),
            cancellation,
            &clock,
        )
        .unwrap();
        assert_code(
            StudentSource::new(
                session,
                owner(),
                admission(),
                MetadataPolicy::parse(&policy_value()).unwrap(),
                Some(form_pin()),
            ),
            "student_read_owner_transport_conflict",
        );
        let mut loopback_request_counter = 0;
        loop {
            match listener.accept() {
                Ok(_) => loopback_request_counter += 1,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("synthetic listener {e}"),
            }
        }
        assert_eq!(
            loopback_request_counter, 0,
            "mismatched admission must never authenticate or read"
        );
        drop(root);
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn fix1_static_metadata_property_keys_and_unresolved_keys() {
    let student = Student {
        student_key: "77001".into(),
        student_name: "Fictional One".into(),
    };
    for key in ["testResultList", "'testResultList'", "['testResultList']"] {
        let html = format!("<script>const data={{{key}:JSON.parse('[]')}};</script>");
        assert!(metadata_rows(&html, &student).unwrap().is_empty());
    }
    for key in [
        "[testResultList]",
        "[unknown]",
        "[1]",
        "[pick()]",
        "['test'+'ResultList']",
    ] {
        let script = format!(
            "<script>const testResultList='unrelated';const data={{{key}:JSON.parse('[]')}};</script>"
        );
        assert_code(
            metadata_rows(&script, &student),
            "result_list_missing_or_ambiguous",
        );
        let (mut src, count, _) = source(vec![
            ("metadata.paper.initial", paper_form()),
            ("metadata.paper.search.1", principal() + &script),
        ]);
        let mut q = period_query("printed_paper_metadata");
        q.set("paperCode", "#5".into());
        assert_code(invoke(&mut src, q), "result_list_missing_or_ambiguous");
        assert_eq!(
            count.get(),
            2,
            "unresolved key must not trigger daily fallback"
        );
        assert_code(
            invoke(&mut src, js(r#"{"kind":"daily_metadata"}"#)),
            "source_failure_latched_no_retry",
        );
        assert_eq!(count.get(), 2);
    }
    // An unresolved property cannot be ignored merely because a valid one exists.
    assert_code(
        metadata_rows(
            "<script>const data={testResultList:JSON.parse('[]'),[unknown]:0};</script>",
            &student,
        ),
        "result_list_missing_or_ambiguous",
    );
}

#[test]
fn fix1_form_ownership_overrides_refuse_before_search_and_latch() {
    let base = paper_form();
    let identified = base.replace("name=\"search\"", "id=\"reviewer-search\" name=\"search\"");
    for html in [
        identified.clone()
            + "<input form=reviewer-search name=extra_required required value=required-filter>",
        identified.clone()
            + "<select form=reviewer-search name=extra><option>extra</option></select>",
        base.replace("name=\"clg_no\"", "form=foreign-form name=\"clg_no\""),
        identified.replace("name=\"clg_no\"", "form=reviewer-search name=\"clg_no\""),
        base.replace("name=pri_no", "form=foreign-form name=pri_no"),
    ] {
        let (mut src, count, _) = source(vec![("metadata.paper.initial", html)]);
        let mut q = period_query("printed_paper_metadata");
        q.set("paperCode", "#5".into());
        assert_code(invoke(&mut src, q), "contract_drift");
        assert_eq!(count.get(), 1);
        assert_code(
            invoke(&mut src, js(r#"{"kind":"daily_metadata"}"#)),
            "source_failure_latched_no_retry",
        );
        assert_eq!(count.get(), 1);
    }
}

#[test]
fn fix1_course_principal_form_ownership_is_also_checked() {
    for html in [
        principal().replace("name=g_pri_no", "form=foreign-form name=g_pri_no"),
        principal().replace("name=form1", "id=course-owner name=form1")
            + "<input form=course-owner name=extra_required required>",
    ] {
        // Ensure the intended fixture mutation happened.
        assert!(html.contains("form=foreign-form") || html.contains("id=course-owner"));
        let (mut src, count, _) = source(vec![("student.course.initial", html)]);
        assert_code(
            src.observe(
                &js(r#"{"operation":"student_course","studentName":"Fictional One"}"#),
                DATE,
                NOW,
            ),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
}

#[test]
fn fix1_metadata_parser_depth_and_resource_bounds_in_isolated_processes() {
    for case in [
        "json4097",
        "js8192",
        "unary8192",
        "members8192",
        "regexp8192",
        "template8192",
        "tokens",
        "json_values",
        "json_positive",
        "js_positive",
        "unary_positive",
        "binary_positive",
        "members_positive",
        "html_positive",
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fix1_metadata_parser_child", "--nocapture"])
            .env("E09A_FIX1_PARSER_CASE", case)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated case {case} failed: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn fix1_metadata_parser_child() {
    let Ok(case) = std::env::var("E09A_FIX1_PARSER_CASE") else {
        return;
    };
    let (script, positive) = match case.as_str() {
        "json4097" => (
            format!(
                "const x={{testResultList:JSON.parse('{}0{}')}};",
                "[".repeat(4097),
                "]".repeat(4097)
            ),
            false,
        ),
        "js8192" => (
            format!(
                "const unused={}0{};const x={{testResultList:JSON.parse('[]')}};",
                "[".repeat(8192),
                "]".repeat(8192)
            ),
            false,
        ),
        "unary8192" => (
            format!(
                "const unused={}0;const x={{testResultList:JSON.parse('[]')}};",
                "!".repeat(8192)
            ),
            false,
        ),
        "members8192" => (
            format!(
                "const unused=x{};const x={{testResultList:JSON.parse('[]')}};",
                ".x".repeat(8192)
            ),
            false,
        ),
        "regexp8192" => (
            format!(
                "const unused=/{}0{}/;const x={{testResultList:JSON.parse('[]')}};",
                "(".repeat(8192),
                ")".repeat(8192)
            ),
            false,
        ),
        "template8192" => (
            format!(
                "const unused={}0{};const x={{testResultList:JSON.parse('[]')}};",
                "`${".repeat(8192),
                "}`".repeat(8192)
            ),
            false,
        ),
        "tokens" => (
            format!(
                "{}const x={{testResultList:JSON.parse('[]')}};",
                ";".repeat(257)
            ),
            false,
        ),
        "json_values" => (
            format!(
                "const x={{testResultList:JSON.parse('[{}0]')}};",
                "0,".repeat(100_001)
            ),
            false,
        ),
        "json_positive" => {
            // Outer array + row object + 29 arrays = 31, one below limit 32.
            let mut r = row("77001", "Fictional One", "3", "81001");
            let mut nested = 0.into();
            for _ in 0..29 {
                nested = Js::Arr(vec![nested]);
            }
            r.set("unselected", nested);
            let text = json(&Js::Arr(vec![r]));
            (
                format!(
                    "const x={{testResultList:JSON.parse({})}};",
                    json(&text.as_str().into())
                ),
                true,
            )
        }
        "js_positive" => (
            format!(
                "const unused={}0{};const x={{testResultList:JSON.parse('[]')}};",
                "[".repeat(31),
                "]".repeat(31)
            ),
            true,
        ),
        "unary_positive" => (
            format!(
                "const unused={}0;const x={{testResultList:JSON.parse('[]')}};",
                "!".repeat(220)
            ),
            true,
        ),
        "binary_positive" => (
            format!(
                "const unused={}0;const x={{testResultList:JSON.parse('[]')}};",
                "0+".repeat(110)
            ),
            true,
        ),
        "members_positive" => (
            format!(
                "const unused=x{};const x={{testResultList:JSON.parse('[]')}};",
                ".x".repeat(110)
            ),
            true,
        ),
        "html_positive" => ("const x={testResultList:JSON.parse('[]')};".into(), true),
        _ => panic!("unknown isolated parser case"),
    };
    let script = format!("<script>{script}</script>");
    let html = if case == "html_positive" {
        format!("{}{script}{}", "<div>".repeat(4096), "</div>".repeat(4096))
    } else {
        script
    };
    let student = Student {
        student_key: "77001".into(),
        student_name: "Fictional One".into(),
    };
    if positive {
        let rows = metadata_rows(&html, &student).unwrap();
        assert_eq!(rows.len(), usize::from(case == "json_positive"));
    } else {
        let err = metadata_rows(&html, &student)
            .err()
            .expect("bounded refusal");
        assert_eq!(err.message, "metadata_parser_resource_limit");
        assert_eq!(
            err.properties.get("code"),
            &"metadata_parser_resource_limit".into()
        );
        for value in ["Fictional One", "77001", "testResultList"] {
            assert!(!format!("{err:?}").contains(value));
        }
        let (mut src, count, _) = source(vec![
            ("metadata.paper.initial", paper_form()),
            ("metadata.paper.search.1", principal() + &html),
        ]);
        let mut q = period_query("printed_paper_metadata");
        q.set("paperCode", "#5".into());
        assert_code(invoke(&mut src, q), "metadata_parser_resource_limit");
        assert_eq!(count.get(), 2);
        assert_code(
            invoke(&mut src, js(r#"{"kind":"daily_metadata"}"#)),
            "source_failure_latched_no_retry",
        );
        assert_eq!(count.get(), 2);
    }
    eprintln!("isolated parser case {case}: PASS");
}

#[test]
fn fix1_daily_selector_explicit_form_owner_refuses() {
    let html = daily().replace("name=sort_date", "form=foreign-form name=sort_date");
    let (mut src, count, _) = source(vec![("metadata.daily", html)]);
    assert_code(
        invoke(&mut src, js(r#"{"kind":"daily_history","periods":["11"]}"#)),
        "contract_drift",
    );
    assert_eq!(count.get(), 1);
}
