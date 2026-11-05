//! Invented owner/page fixtures only. No live calls or source/student originals.
use edu_academy_sources::{
    data::*,
    history::{decode_code, encode_code},
    http::{self, Response},
    owner_admission::{TransportAdmission, TransportScope},
    paper::{self, PaperOwner, PaperPins, PaperScope, PaperScopes, PaperSource},
    student_course::{self, ReadPort},
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
fn owner_value(scope: PaperScope) -> Js {
    let v = js(
        r#"{"ownerId":"fictional-paper1","teacherLabel":"Fictional Teacher","teacherSourceKey":"99001","collegeSourceKey":"88001","students":[{"studentKey":"77001","studentName":"Fictional One"}]}"#,
    );
    remap_js(v, scope)
}
fn remap(text: &str, scope: PaperScope) -> String {
    if scope == PaperScope::Paper1 {
        text.into()
    } else {
        text.replace("fictional-paper1", "fictional-paper2")
            .replace("99001", "99002")
            .replace("88001", "88002")
            .replace("77001", "77002")
            .replace("Fictional One", "Fictional Two")
    }
}
fn remap_js(v: Js, scope: PaperScope) -> Js {
    js(&remap(&json(&v), scope))
}
fn policy_value(scope: PaperScope) -> Js {
    remap_js(
        js(
            r#"{"schemaVersion":"academy.metadata-admission/1","sourceNamespace":"academy.lms.metadata","teacherId":"99001","allowedFrom":"2040-01-01","allowedThrough":"2040-01-31","studentScope":"configured","relativeSelectors":["1","11","12","13","14","15","16"],"operations":["student_papers","daily_metadata","daily_inventory","daily_history","printed_paper_metadata"]}"#,
        ),
        scope,
    )
}
fn routes() -> PaperScopes {
    PaperScopes::parse(&js(r##"[{"endpoint":"paper1","studentIds":["77001"]},{"endpoint":"paper2","studentIds":["77002"]}]"##)).unwrap()
}
fn admission(scope: PaperScope) -> TransportAdmission {
    let owner = owner_value(scope);
    let mut operations = student_course::read_contracts();
    operations.retain(|op| op.source_namespace == student_course::METADATA_NS);
    operations.extend(paper::read_contracts());
    TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: string(owner.get("ownerId")),
        teacher_id: string(owner.get("teacherSourceKey")),
        teacher_label: "Fictional Teacher".into(),
        college_id: string(owner.get("collegeSourceKey")),
        allowed_from: "2040-01-01".into(),
        allowed_through: "2040-01-31".into(),
        student_ids: vec![string(owner.get("students").array()[0].get("studentKey"))],
        source_namespaces: vec![student_course::METADATA_NS.into(), paper::PAPER_NS.into()],
        operations,
    }
}
fn reader_shell(scope: PaperScope) -> String {
    remap(
        &(paper_form()
            + r#"<script>var testInfoDialogManager={ready:function(){return 'fictional-ready';},viewDetailTest:function(){return 'fictional-view';}};</script>"#),
        scope,
    )
}
fn catalog() -> String {
    format!(
        r#"<form name=form1 id=catalog action="{}?reqCmd=DailyZeroTestSeach" method=post><input type=hidden name=evalgubun value=DT><input type=hidden name=frm_cl1 value=0><input type=hidden name=frm_cl2 value=-1><input type=hidden name=frm_cl3 value=-1></form><select id=cl1><option value=124>[22개정] 중1</option><option value=154>[22개정] 초5</option></select><script>function onSearchTestPaperList(){{return 'NEVER_EXECUTE';}}</script>"#,
        student_course::DAILY_PATH
    )
}
fn pool() -> String {
    "<form name=pool method=post action='/unused'><input name=category value=0></form><script>function loadCategory(){return 'NEVER_EXECUTE';}</script>".into()
}
fn converter_program() -> String {
    "const alphabet='23456789ABCDEFGHJKLMNPQRSTUVWXYZ';function keyToCode(n){return n;}function codeToKey(n){return n;}".into()
}
fn pins() -> PaperPins {
    pins_with_context(&reader_shell(PaperScope::Paper1))
}
fn pins_with_context(reference: &str) -> PaperPins {
    PaperPins::parse(&obj(&[
        ("schemaVersion", "academy.paper-pins/1".into()),
        (
            "readerContext",
            paper::reader_context_fingerprint(reference).unwrap().into(),
        ),
        (
            "ready",
            hash(b"function(){return 'fictional-ready';}").into(),
        ),
        ("view", hash(b"function(){return 'fictional-view';}").into()),
        (
            "catalog",
            paper::contract_fingerprint(&catalog()).unwrap().into(),
        ),
        ("pool", paper::contract_fingerprint(&pool()).unwrap().into()),
        ("converter", hash(converter_program().as_bytes()).into()),
    ]))
    .unwrap()
}
fn owner(scope: PaperScope) -> PaperOwner {
    owner_with_context(scope, &reader_shell(scope))
}
fn owner_with_context(scope: PaperScope, reference: &str) -> PaperOwner {
    PaperOwner::parse(
        scope,
        &routes(),
        &owner_value(scope),
        &policy_value(scope),
        &remap_js(form_value(), scope),
        pins_with_context(reference),
    )
    .unwrap()
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
type Capture = Rc<RefCell<Vec<(String, Vec<(String, String)>, String)>>>;
struct Fake {
    admission: TransportAdmission,
    replies: VecDeque<(String, Response)>,
    count: Rc<Cell<usize>>,
    seen: Capture,
    fault: Option<&'static str>,
}
impl ReadPort for Fake {
    fn transport_admission(&self) -> &TransportAdmission {
        &self.admission
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        id: &str,
        args: &[(String, String)],
    ) -> Result<Response> {
        assert_eq!(scope.owner_id, self.admission.owner_id);
        assert_eq!(scope.teacher_id, self.admission.teacher_id);
        assert!(self.admission.student_ids.contains(&scope.student_id));
        assert!(self.admission.operations.iter().any(|op| op.id == id));
        self.count.set(self.count.get() + 1);
        self.seen
            .borrow_mut()
            .push((id.into(), args.to_vec(), scope.student_id.clone()));
        if let Some(code) = self.fault {
            return Err(Error::refusal(code));
        }
        let (expected, body) = self
            .replies
            .pop_front()
            .expect("unexpected synthetic request");
        assert_eq!(id, expected);
        Ok(body)
    }
}
fn fake(scope: PaperScope, replies: Vec<(&str, String)>) -> (Fake, Rc<Cell<usize>>, Capture) {
    let count = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(vec![]));
    (
        Fake {
            admission: admission(scope),
            replies: replies
                .into_iter()
                .map(|(id, body)| (id.into(), response(&body)))
                .collect(),
            count: count.clone(),
            seen: seen.clone(),
            fault: None,
        },
        count,
        seen,
    )
}
fn source(
    scope: PaperScope,
    replies: Vec<(&str, String)>,
) -> (PaperSource<Fake>, Rc<Cell<usize>>, Capture) {
    let (port, count, seen) = fake(scope, replies);
    (
        PaperSource::new(port, owner(scope), admission(scope)).unwrap(),
        count,
        seen,
    )
}
fn input(scope: PaperScope) -> Js {
    remap_js(obj(&[("studentId", "77001".into())]), scope)
}
fn call(source: &mut PaperSource<Fake>, tool: &str, input: &Js) -> Result<Js> {
    let name = if source.endpoint() == PaperScope::Paper2 && !tool.starts_with("class2_") {
        format!("class2_{tool}")
    } else {
        tool.into()
    };
    source.read(&name, input, DATE, NOW)
}
fn assert_code<T>(r: Result<T>, code: &str) {
    let e = r.err().expect("must refuse");
    assert_eq!(e.message, code);
    assert_eq!(e.properties.get("code"), &code.into());
}
fn paper_input(scope: PaperScope) -> Js {
    let mut v = input(scope);
    v.set("paperCode", "#5".into());
    v
}
fn printed_input(scope: PaperScope) -> Js {
    let mut v = paper_input(scope);
    v.set(
        "period",
        js(r##"{"from":"2040-01-01","through":"2040-01-04"}"##),
    );
    v
}
fn daily(scope: PaperScope, rows: Vec<Js>) -> String {
    remap(&result(rows), scope)
}
fn definition_xml() -> String {
    "<?xml version='1.0'?><root><inforesult><![CDATA[999##item##Fictional shared title##item##HIDDEN_FILE_LOCATION##item##12##item##1180##item##2##item##1##item##Fictional major##item##Fictional category]]></inforesult></root>".into()
}
fn header(key: &str) -> String {
    format!(
        r#"<html><head><title>Fictional header</title></head><body><iframe src="{}?p_process=TestPageTabSwitch&amp;p_testno={key}&amp;seltab=1"></iframe></body></html>"#,
        "/servlet/controller.common.TestPageShowServlet"
    )
}
fn content_header() -> String {
    "<h1>Fictional paper</h1><table><tr><th>시험지 제목</th><td>Fictional title</td></tr><tr><th>총 문제수</th><td>12</td></tr><tr><th>Question secret</th><td>NEVER_EXPORT_QUESTION</td></tr></table>".into()
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

#[test]
fn code_conversion_preserves_exact_safe_integer_boundary() {
    for n in [1u64, 3, 31, 32, 33, 1000, 9_007_199_254_740_991] {
        let value: Js = n.to_string().into();
        let code = encode_code(&value).unwrap();
        assert_eq!(decode_code(&code).unwrap(), value);
    }
    assert_code(encode_code(&"0".into()), "invalid_printed_paper_key");
    assert_code(
        encode_code(&"9007199254740992".into()),
        "unsafe_printed_paper_key",
    );
    assert!(decode_code(&"#2".into()).is_err());
    assert!(decode_code(&"#ZZZZZZZZZZZ".into()).is_err());
}
#[test]
fn both_scopes_keep_aliases_students_and_original_row_order_separate() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let html = daily(
            scope,
            vec![
                row("77001", "Fictional One", "4", "81002"),
                row("77001", "Fictional One", "3", "81001"),
            ],
        );
        let (mut src, count, seen) = source(scope, vec![("metadata.daily", html)]);
        let tool = if scope == PaperScope::Paper2 {
            "class2_academy_daily_test_inventory"
        } else {
            "academy_daily_test_inventory"
        };
        let result = call(&mut src, tool, &input(scope)).unwrap();
        assert_eq!(result.get("endpoint"), &scope.as_str().into());
        let observation = result.get("observation");
        assert_eq!(
            observation.get("rows").array()[0].get("paperKey"),
            &"4".into()
        );
        assert_eq!(
            observation.get("rows").array()[1].get("paperKey"),
            &"3".into()
        );
        assert_eq!(observation.get("teacherJudgment"), &Js::Null);
        assert_eq!(
            observation.get("expectedPaperDefinitions"),
            &"not_observed".into()
        );
        assert!(observation.has("filterContracts"));
        assert!(!json(&result).contains("NEVER_EXPORT"));
        assert_eq!(count.get(), 1);
        assert_eq!(seen.borrow()[0].2, string(input(scope).get("studentId")));
    }
}
#[test]
fn history_all_six_selectors_are_explicit_and_empty_is_observed_only() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let empty = daily(scope, vec![]);
        let replies = (0..7).map(|_| ("metadata.daily", empty.clone())).collect();
        let (mut src, count, seen) = source(scope, replies);
        let mut request = input(scope);
        request.set("periods", strings(&["16", "15", "14", "13", "12", "11"]));
        let result = call(&mut src, "academy_daily_test_inventory", &request).unwrap();
        let pages = result.get("observation").get("observations").array();
        assert_eq!(pages.len(), 6);
        assert_eq!(pages[0].get("selector"), &"16".into());
        assert_eq!(pages[5].get("selector"), &"11".into());
        assert!(
            pages
                .iter()
                .all(|p| p.get("rows").array().is_empty() && p.get("teacherJudgment") == &Js::Null)
        );
        assert_eq!(count.get(), 7);
        let selectors = seen
            .borrow()
            .iter()
            .map(|(_, args, _)| {
                args.iter()
                    .find(|(k, _)| k == "sort_date")
                    .unwrap()
                    .1
                    .clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(selectors, vec!["1", "16", "15", "14", "13", "12", "11"]);
    }
}
#[test]
fn missing_relative_selector_refuses_before_history_dispatch_and_latches() {
    let html = result(vec![]).replace("<option value=16>Fictional 16</option>", "");
    let (mut src, count, _) = source(PaperScope::Paper1, vec![("metadata.daily", html)]);
    let mut request = input(PaperScope::Paper1);
    request.set("periods", strings(&["16"]));
    assert_code(
        call(&mut src, "academy_daily_test_inventory", &request),
        "daily_history_selector_drift",
    );
    assert_code(
        call(&mut src, "academy_daily_test_inventory", &request),
        "source_failure_latched_no_retry",
    );
    assert_eq!(count.get(), 1);
}
#[test]
fn printed_primary_miss_uses_only_qualified_daily_fallback() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let (mut src, count, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", remap(&paper_form(), scope)),
                (
                    "metadata.paper.search.1",
                    remap(&paper_result(vec![]), scope),
                ),
                (
                    "metadata.daily",
                    daily(scope, vec![row("77001", "Fictional One", "3", "81001")]),
                ),
            ],
        );
        let result = call(
            &mut src,
            "academy_printed_paper_lookup",
            &printed_input(scope),
        )
        .unwrap();
        let o = result.get("observation");
        assert_eq!(o.get("matchedSourceFamily"), &"daily_zero".into());
        assert_eq!(o.get("sourcePeriod"), &Js::Null);
        assert_eq!(o.get("searchedSources").array().len(), 2);
        assert_eq!(o.get("teacherJudgment"), &Js::Null);
        assert_eq!(count.get(), 3);
    }
}
#[test]
fn unqualified_primary_never_falls_back_and_latches_other_families() {
    let (mut src, count, _) = source(
        PaperScope::Paper1,
        vec![
            ("metadata.paper.initial", paper_form()),
            (
                "metadata.paper.search.1",
                principal() + "<script>const x=1;</script>",
            ),
        ],
    );
    assert_code(
        call(
            &mut src,
            "academy_printed_paper_lookup",
            &printed_input(PaperScope::Paper1),
        ),
        "result_list_missing_or_ambiguous",
    );
    assert_code(
        call(
            &mut src,
            "academy_paper_definition_lookup",
            &paper_input(PaperScope::Paper1),
        ),
        "source_failure_latched_no_retry",
    );
    assert_eq!(count.get(), 2);
}
#[test]
fn duplicate_attempt_and_conflicting_source_student_are_hard_refusals() {
    let cases = vec![
        (
            vec![
                row("77001", "Fictional One", "3", "81001"),
                row("77001", "Fictional One", "3", "81001"),
            ],
            "duplicate_student_search_row",
        ),
        (
            vec![row("77001", "Fictional Other", "3", "81001")],
            "student_search_identity_conflict",
        ),
        (
            vec![row("77002", "Fictional One", "3", "81001")],
            "student_search_identity_conflict",
        ),
    ];
    for (rows, code) in cases {
        let (mut src, count, _) =
            source(PaperScope::Paper1, vec![("metadata.daily", result(rows))]);
        assert_code(
            call(
                &mut src,
                "academy_daily_test_inventory",
                &input(PaperScope::Paper1),
            ),
            code,
        );
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn definition_and_header_views_work_in_both_scopes_without_attempt_inference() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for view in ["definition", "header"] {
            let mut replies = vec![("metadata.paper.initial", reader_shell(scope))];
            if view == "definition" {
                replies.push(("paper.definition", definition_xml()));
            } else {
                replies.extend([
                    ("paper.header", header("3")),
                    ("paper.content", content_header()),
                ]);
            }
            let (mut src, count, seen) = source(scope, replies);
            let mut request = paper_input(scope);
            request.set("view", view.into());
            let result = call(&mut src, "academy_paper_definition_lookup", &request).unwrap();
            let o = result.get("observation");
            assert_eq!(o.get("requestedPaperKey"), &"3".into());
            assert_eq!(o.get("studentAssignment"), &"unqualified".into());
            if view == "definition" {
                assert_eq!(o.get("information").get("definitionNo"), &"999".into());
                assert_eq!(o.get("information").get("questionCount"), &12.into());
                assert_eq!(
                    o.get("information").get("sourceDisplayLevelLabel"),
                    &"발전".into()
                );
                assert!(!json(&result).contains("HIDDEN_FILE_LOCATION"));
                assert_eq!(count.get(), 2);
            } else {
                assert!(!json(&result).contains("NEVER_EXPORT_QUESTION"));
                assert_eq!(count.get(), 3);
            }
            assert_eq!(
                seen.borrow().last().unwrap().2,
                string(input(scope).get("studentId"))
            );
        }
    }
}
#[test]
fn paper2_definition_batch_and_both_header_batches_retain_input_order() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for definition in [false, true] {
            if definition && scope == PaperScope::Paper1 {
                continue;
            }
            let mut replies = vec![("metadata.paper.initial", reader_shell(scope))];
            for key in ["4", "3"] {
                if definition {
                    replies.push(("paper.definition", definition_xml()));
                } else {
                    replies.extend([
                        ("paper.header", header(key)),
                        ("paper.content", content_header()),
                    ]);
                }
            }
            let (mut src, count, _) = source(scope, replies);
            let mut request = input(scope);
            request.set("paperCodes", strings(&["#6", "#5"]));
            if definition {
                request.set("view", "definition".into());
            }
            let result = call(&mut src, "academy_paper_headers", &request).unwrap();
            let rows = result.get("observation").get("rows").array();
            assert_eq!(rows[0].get("paperCode"), &"#6".into());
            assert_eq!(rows[1].get("paperCode"), &"#5".into());
            assert_eq!(count.get(), if definition { 3 } else { 5 });
        }
    }
}
#[test]
fn all_source_contract_kinds_work_on_both_scopes() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for kind in [
            "printed_code",
            "daily_test_catalog",
            "middle1_definitions",
            "elementary5_fraction_definitions",
        ] {
            let replies=match kind { "printed_code"=>vec![("paper.converter",converter_program())],
                "daily_test_catalog"=>vec![("paper.catalog",catalog())],
                _=>vec![("paper.catalog",catalog()),("paper.definitions", "<table><tr><td>5-1 Fictional</td><td>12</td></tr><tr><td>2-2 분수의 곱셈</td><td>24</td></tr><tr><td>9-9 unrelated</td><td>0</td></tr></table>".into())] };
            let (mut src, count, seen) = source(scope, replies);
            let mut request = input(scope);
            request.set("kind", kind.into());
            if kind == "middle1_definitions" {
                request.set("unitCodes", strings(&["5-1"]));
            }
            let result = call(&mut src, "academy_source_read_contracts", &request).unwrap();
            let o = result.get("observation");
            assert_eq!(o.get("sourceHandlersExecuted"), &false.into());
            assert_eq!(o.get("teacherJudgment"), &Js::Null);
            if kind.ends_with("definitions") {
                assert_eq!(o.get("rows").array().len(), 1);
                assert_eq!(count.get(), 2);
                let args = &seen.borrow()[1].1;
                assert_eq!(
                    args.iter().find(|(k, _)| k == "frm_cl1").unwrap().1,
                    if kind == "middle1_definitions" {
                        "124"
                    } else {
                        "154"
                    }
                );
            } else {
                assert_eq!(count.get(), 1);
            }
        }
    }
}
#[test]
fn paper_contract_inspection_checks_pins_and_exports_no_source_students_or_scripts() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let (mut src, count, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", reader_shell(scope)),
                ("paper.pool", pool()),
            ],
        );
        let result = call(&mut src, "academy_paper_read_contracts", &input(scope)).unwrap();
        assert_eq!(result.get("status"), &"inert_contracts_observed".into());
        assert_eq!(
            result.get("observation").get("sourceStudentRowsRetained"),
            &0.into()
        );
        assert!(!json(&result).contains("Fictional One"));
        assert!(!json(&result).contains("NEVER_EXECUTE"));
        assert_eq!(count.get(), 2);
    }
}

#[test]
fn unsupported_queries_shapes_and_scope_aliases_make_zero_requests() {
    let scope = PaperScope::Paper1;
    let mut cases = vec![
        ("academy_unknown", input(scope), "paper_tool_not_supported"),
        (
            "class2_academy_daily_test_inventory",
            input(scope),
            "paper_scope_unresolved",
        ),
        (
            "academy_daily_test_inventory",
            js(r##"{"studentId":"77001","periods":[]}"##),
            "tool_input_invalid",
        ),
        (
            "academy_daily_test_inventory",
            js(r##"{"studentId":"77001","periods":["11","11"]}"##),
            "tool_input_invalid",
        ),
        (
            "academy_daily_test_inventory",
            js(r##"{"studentId":"77001","periods":["1"]}"##),
            "tool_input_invalid",
        ),
        (
            "academy_daily_test_inventory",
            js(r##"{"studentId":"77002"}"##),
            "read_scope_not_admitted",
        ),
        (
            "academy_paper_definition_lookup",
            js(r##"{"studentId":"77001","paperCode":"#5","view":"result"}"##),
            "tool_input_invalid",
        ),
        (
            "academy_paper_headers",
            js(r##"{"studentId":"77001","paperCodes":["#5"],"view":"definition"}"##),
            "tool_input_invalid",
        ),
        (
            "academy_paper_headers",
            js(r##"{"studentId":"77001","paperCodes":["#5","#5"]}"##),
            "read_scope_not_admitted",
        ),
        (
            "academy_paper_headers",
            js(r##"{"studentId":"77001","paperCodes":[]}"##),
            "tool_input_invalid",
        ),
        (
            "academy_source_read_contracts",
            js(r##"{"studentId":"77001","kind":"middle1_definitions"}"##),
            "daily_definition_unit_scope_rejected",
        ),
        (
            "academy_source_read_contracts",
            js(r##"{"studentId":"77001","kind":"middle1_definitions","unitCodes":["5-8"]}"##),
            "daily_definition_unit_scope_rejected",
        ),
        (
            "academy_source_read_contracts",
            js(r##"{"studentId":"77001","kind":"middle1_definitions","unitCodes":["5-1","5-1"]}"##),
            "daily_definition_unit_scope_rejected",
        ),
        (
            "academy_source_read_contracts",
            js(
                r##"{"studentId":"77001","kind":"middle1_definitions","unitCodes":["5-1"],"unitNames":{"other":"Fictional"}}"##,
            ),
            "tool_input_invalid",
        ),
    ];
    for field in ["studentId", "view"] {
        let mut request = paper_input(scope);
        request.set("view", "header".into());
        if let Js::Obj(props) = &mut request {
            props.push((
                text(field),
                if field == "view" {
                    "definition".into()
                } else {
                    "77001".into()
                },
            ));
        }
        cases.push((
            "academy_paper_definition_lookup",
            request,
            "tool_input_invalid",
        ));
    }
    for kind in [
        "printed_code",
        "daily_test_catalog",
        "elementary5_fraction_definitions",
    ] {
        cases.push((
            "academy_source_read_contracts",
            obj(&[
                ("studentId", "77001".into()),
                ("kind", kind.into()),
                ("unitCodes", strings(&["5-1"])),
            ]),
            "paper_query_combination_not_supported",
        ));
    }
    for (tool, request, code) in cases {
        let (mut src, count, _) = source(scope, vec![]);
        assert_code(call(&mut src, tool, &request), code);
        assert_eq!(count.get(), 0);
    }
}
#[test]
fn actual_admission_owner_path_namespace_and_operation_mismatches_make_zero_requests() {
    for change in 0..7 {
        let scope = PaperScope::Paper1;
        let (mut port, count, _) = fake(scope, vec![]);
        match change {
            0 => port.admission.owner_id = "fictional-other".into(),
            1 => port.admission.teacher_id = "99002".into(),
            2 => port.admission.college_id = "88002".into(),
            3 => port.admission.student_ids = vec!["77002".into()],
            4 => port.admission.operations[0].path = "/different".into(),
            5 => {
                port.admission.operations.pop();
            }
            _ => {
                port.admission.source_namespaces.pop();
            }
        }
        assert_code(
            PaperSource::new(port, owner(scope), admission(scope)),
            "student_read_owner_transport_conflict",
        );
        assert_eq!(count.get(), 0);
    }
}
#[test]
fn missing_contract_or_date_admission_refuses_before_initial_page() {
    for change in 0..3 {
        let scope = PaperScope::Paper1;
        let (mut port, count, _) = fake(scope, vec![]);
        let mut expected = admission(scope);
        match change {
            0 => {
                port.admission
                    .operations
                    .retain(|o| o.id != "paper.content");
                expected.operations.retain(|o| o.id != "paper.content");
            }
            1 => {
                port.admission
                    .operations
                    .iter_mut()
                    .find(|o| o.id == "paper.header")
                    .unwrap()
                    .path = "/different".into();
                expected
                    .operations
                    .iter_mut()
                    .find(|o| o.id == "paper.header")
                    .unwrap()
                    .path = "/different".into();
            }
            _ => {
                port.admission.allowed_through = "2040-01-03".into();
                expected.allowed_through = "2040-01-03".into();
            }
        }
        let mut src = PaperSource::new(port, owner(scope), expected).unwrap();
        let mut request = paper_input(scope);
        request.set("view", "header".into());
        let result = call(&mut src, "academy_paper_definition_lookup", &request);
        if change < 2 {
            assert_code(result, "paper_contract_not_admitted");
        } else {
            assert!(result.is_err());
        }
        assert_eq!(count.get(), 0);
    }
}
#[test]
fn route_configuration_conflicts_and_unknown_scope_refuse() {
    for v in [
        js("[]"),
        js(
            r#"[{"endpoint":"paper1","studentIds":["77001"]},{"endpoint":"paper2","studentIds":["77001"]}]"#,
        ),
        js(
            r#"[{"endpoint":"paper1","studentIds":["77001"]},{"endpoint":"paper1","studentIds":["77002"]}]"#,
        ),
    ] {
        assert!(PaperScopes::parse(&v).is_err());
    }
    assert_code(PaperScope::parse("paper3"), "paper_scope_unresolved");
    assert_code(routes().resolve("77003"), "paper_scope_unresolved");
    assert_code(
        PaperOwner::parse(
            PaperScope::Paper2,
            &routes(),
            &owner_value(PaperScope::Paper1),
            &policy_value(PaperScope::Paper1),
            &form_value(),
            pins(),
        ),
        "paper_scope_unresolved",
    );
}
#[test]
fn every_pinned_search_element_and_effective_form_owner_drift_stops_after_initial() {
    let original = paper_form();
    let mut changed = vec![
        original.replace("name=\"clg_no\"", "name=\"different\""),
        original.replace("method=\"post\"", "method=\"get\""),
        original.replace(
            "UserBySearchTestResult&amp;ass_no=1001",
            "OtherCommand&amp;ass_no=1001",
        ),
        original.replace(
            "type=\"hidden\" value=\"88001\"",
            "type=\"text\" value=\"88001\"",
        ),
        original.replace("value=\"88001\"", "value=\"88002\""),
        original.replace("value=\"D\" checked", "value=\"D\""),
        original.replace("value=\"D\" checked", "value=\"D\" checked disabled"),
        original.replace("All fictional classes", "Changed class label"),
        original.replace("<option value=\"0\">All fictional classes</option>", ""),
        original.replace("<form name=\"search\"", "<form name=\"search\" id=bound")
            + "<input form=bound name=external_required required>",
        original.replace("name=\"clg_no\"", "name=\"clg_no\" form=foreign"),
        original.replace("<form name=\"search\"", "<form name=\"search\" id=bound")
            + "<div id=bound></div>",
        original.replace(
            "<input name=\"stu_name\"",
            "<input required name=extra><input name=\"stu_name\"",
        ),
    ];
    for field in [
        "clg_no",
        "cls_no",
        "p_pageno",
        "check_da_test",
        "check_fa_test",
        "check_na_test",
        "stu_name",
        "chioce_cls",
        "sort_date1",
        "sort_date2",
    ] {
        changed.push(original.replace(
            &format!("name=\"{field}\""),
            &format!("name=\"{field}\" form=foreign"),
        ));
    }
    for html in changed {
        let (mut src, count, _) =
            source(PaperScope::Paper1, vec![("metadata.paper.initial", html)]);
        assert_code(
            call(
                &mut src,
                "academy_printed_paper_lookup",
                &printed_input(PaperScope::Paper1),
            ),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
        assert_code(
            call(
                &mut src,
                "academy_daily_test_inventory",
                &input(PaperScope::Paper1),
            ),
            "source_failure_latched_no_retry",
        );
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn reader_methods_require_static_unique_unmodified_owner_pinned_bindings() {
    let original = reader_shell(PaperScope::Paper1);
    let cases = vec![
        original.replace("ready:function", "[ready]:function"),
        original.replace("ready:function", "['ready']:function"),
        original.replace("ready:function", "[sideEffect()]:function"),
        original.replace("fictional-ready", "changed-ready"),
        original.clone() + "<script>testInfoDialogManager.ready=function(){};</script>",
        original.clone() + "<script>var testInfoDialogManager={};</script>",
        original.replace("ready:function", "ready:call(),x:function"),
    ];
    for html in cases {
        let (mut src, count, _) =
            source(PaperScope::Paper1, vec![("metadata.paper.initial", html)]);
        assert_code(
            call(
                &mut src,
                "academy_paper_definition_lookup",
                &paper_input(PaperScope::Paper1),
            ),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn computed_metadata_keys_are_refused_even_for_literal_string_forms() {
    for key in [
        "[testResultList]",
        "['testResultList']",
        "[sideEffect()]",
        "['test'+'ResultList']",
    ] {
        let html = result(vec![]).replace("testResultList:", &format!("{key}:"));
        let (mut src, count, _) = source(PaperScope::Paper1, vec![("metadata.daily", html)]);
        assert_code(
            call(
                &mut src,
                "academy_daily_test_inventory",
                &input(PaperScope::Paper1),
            ),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn catalog_form_action_options_defaults_and_ownership_are_pinned_before_search() {
    let original = catalog();
    for html in [
        original.replace("method=post", "method=get"),
        original.replace("DailyZeroTestSeach", "OtherCommand"),
        original.replace("name=evalgubun", "name=other"),
        original.replace("value=DT", "value=ZT"),
        original.replace("[22개정] 중1", "Changed grade"),
        original.replace("value=124", "value=125"),
        original.replace("name=evalgubun", "name=evalgubun form=foreign"),
        original.clone() + "<input form=catalog name=extra required>",
        original.clone() + "<div id=catalog></div>",
        original.replace(
            "<input type=hidden name=frm_cl1",
            "<input required name=extra><input type=hidden name=frm_cl1",
        ),
    ] {
        let (mut src, count, _) = source(PaperScope::Paper1, vec![("paper.catalog", html)]);
        let mut request = input(PaperScope::Paper1);
        request.set("kind", "middle1_definitions".into());
        request.set("unitCodes", strings(&["5-1"]));
        assert_code(
            call(&mut src, "academy_source_read_contracts", &request),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn header_frame_identity_and_duplicates_refuse_without_following_any_source_url() {
    let original = header("3");
    for html in [
        header("4"),
        original.replace("seltab=1", "seltab=2"),
        original.replace("seltab=1", "seltab=1&amp;extra=1"),
        original.replace("seltab=1", "seltab=1&amp;seltab=1"),
        original.replace("/servlet/", "https://foreign.invalid/servlet/"),
        original.clone() + &original,
        original.clone() + "<input name=p_testno value=3><input name=p_testno value=3>",
        original.clone() + "<input name=p_testno value=3 form=foreign>",
        original + "<input name=p_testno value=4>",
    ] {
        let (mut src, count, _) = source(
            PaperScope::Paper1,
            vec![
                ("metadata.paper.initial", reader_shell(PaperScope::Paper1)),
                ("paper.header", html),
            ],
        );
        let mut request = paper_input(PaperScope::Paper1);
        request.set("view", "header".into());
        assert!(call(&mut src, "academy_paper_definition_lookup", &request).is_err());
        assert_eq!(count.get(), 2);
        assert_code(
            call(
                &mut src,
                "academy_daily_test_inventory",
                &input(PaperScope::Paper1),
            ),
            "source_failure_latched_no_retry",
        );
    }
}
#[test]
fn definition_xml_is_strict_and_keeps_source_definition_identity_distinct() {
    let original = definition_xml();
    for text in [
        original.replace("</root>", ""),
        original.clone() + "<extra/>",
        original.replace("<root>", "<!DOCTYPE root><root>"),
        original.replace("<inforesult>", "<inforesult x='y'>"),
        original.replace("12##item##1180", "0##item##1180"),
        original.replace("12##item##1180", "10001##item##1180"),
        original.replace("<root>", "<root><parsererror/>"),
        original.replace("<root>", "<root><record>spoof</record>"),
        original.replace(
            "</inforesult>",
            "</inforesult><inforesult>spoof</inforesult>",
        ),
    ] {
        assert!(paper::parse_definition(&text).is_err());
    }
    let info = paper::parse_definition(&original).unwrap();
    assert_eq!(info.get("definitionNo"), &"999".into());
    assert!(!json(&info).contains("HIDDEN_FILE_LOCATION"));
    assert_eq!(info.get("studentScore"), &"unobserved".into());
}
#[test]
fn source_http_failure_has_no_fallback_or_second_attempt() {
    let scope = PaperScope::Paper1;
    let (mut port, count, _) = fake(scope, vec![("metadata.daily", result(vec![]))]);
    port.replies[0].1.status = 500;
    let mut src = PaperSource::new(port, owner(scope), admission(scope)).unwrap();
    assert_code(
        call(&mut src, "academy_daily_test_inventory", &input(scope)),
        "student_read_source_rejected",
    );
    assert_code(
        call(&mut src, "academy_daily_test_inventory", &input(scope)),
        "source_failure_latched_no_retry",
    );
    assert_eq!(count.get(), 1);
}

#[test]
fn recursive_parser_limits_are_checked_in_isolated_processes() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    for case in [
        "js31",
        "js32",
        "js33",
        "js8192",
        "json31",
        "json32",
        "json33",
        "json4097",
        "tokens255",
        "tokens260",
        "xml31",
        "xml32",
        "xml33",
        "size2000000",
        "size2000001",
        "nested_input65536",
        "history_input65536",
    ] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "paper_parser_child", "--nocapture"])
            .env("E09B_PARSER_CASE", case)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if start.elapsed() > Duration::from_secs(300) {
                child.kill().unwrap();
                panic!("isolated case {case} exceeded timeout");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "isolated case {case} failed: {} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn paper_parser_child() {
    let Ok(case) = std::env::var("E09B_PARSER_CASE") else {
        return;
    };
    if let Some(depth) = case.strip_prefix("xml") {
        let depth = depth.parse::<usize>().unwrap();
        let xml = definition_xml().replace("<?xml version='1.0'?>", "");
        let xml = "<a>".repeat(depth - 2) + &xml + &"</a>".repeat(depth - 2);
        let r = paper::parse_definition(&xml);
        if depth <= 32 {
            assert!(r.is_ok());
        } else {
            assert_code(r, "paper_parser_resource_limit");
        }
        return;
    }
    let scope = PaperScope::Paper1;
    if case == "history_input65536" {
        let mut nested = Js::Null;
        for _ in 0..65_536 {
            nested = Js::Arr(vec![nested]);
        }
        let (mut src, count, _) = source(scope, vec![]);
        assert_code(
            src.history_header(&nested, &"#5".into(), DATE, NOW),
            "student_read_source_key_invalid",
        );
        assert_code(
            src.history_header(&"77001".into(), &nested, DATE, NOW),
            "invalid_printed_paper_code",
        );
        assert_eq!(count.get(), 0);
        return;
    }
    if case == "nested_input65536" {
        let mut nested = Js::Null;
        for _ in 0..65_536 {
            nested = Js::Arr(vec![nested]);
        }
        let (mut src, count, _) = source(scope, vec![]);
        let mut request = input(scope);
        request.set("periods", Js::Arr(vec![nested]));
        assert_code(
            call(&mut src, "academy_daily_test_inventory", &request),
            "tool_input_invalid",
        );
        assert_eq!(count.get(), 0);
        return;
    }
    let mut expected = None;
    let html = if let Some(depth) = case
        .strip_prefix("js")
        .filter(|rest| rest.bytes().all(|b| b.is_ascii_digit()))
    {
        let depth = depth.parse::<usize>().unwrap();
        if depth > 32 {
            expected = Some("metadata_parser_resource_limit");
        }
        result(vec![])
            + &format!(
                "<script>const inert={}0{};</script>",
                "[".repeat(depth),
                "]".repeat(depth)
            )
    } else if let Some(depth) = case.strip_prefix("json") {
        let depth = depth.parse::<usize>().unwrap();
        if depth > 32 {
            expected = Some("metadata_parser_resource_limit");
        }
        let raw = format!(
            "[{{\"priNo\":\"77001\",\"userName\":\"Fictional One\",\"pNo\":\"3\",\"testingNo\":\"81001\",\"inert\":{}0{}}}]",
            "[".repeat(depth - 2),
            "]".repeat(depth - 2)
        );
        format!(
            "<script>const rows={{testResultList:JSON.parse('{}')}};</script>",
            raw.replace('\\', "\\\\").replace('\'', "\\'")
        )
    } else if let Some(tokens) = case.strip_prefix("tokens") {
        let tokens = tokens.parse::<usize>().unwrap();
        if tokens > 256 {
            expected = Some("metadata_parser_resource_limit");
        }
        result(vec![]) + &format!("<script>{}</script>", "var inert=0;".repeat(tokens / 5))
    } else if let Some(size) = case.strip_prefix("size") {
        let size = size.parse::<usize>().unwrap();
        if size > 2_000_000 {
            expected = Some("paper_source_size_limit");
        }
        let base = result(vec![]);
        format!("{base}<!--{}-->", " ".repeat(size - base.len() - 7))
    } else {
        panic!("unknown case");
    };
    let (mut src, count, _) = source(scope, vec![("metadata.daily", html)]);
    let r = call(&mut src, "academy_daily_test_inventory", &input(scope));
    if let Some(code) = expected {
        assert_code(r, code);
        assert_code(
            call(&mut src, "academy_daily_test_inventory", &input(scope)),
            "source_failure_latched_no_retry",
        );
    } else {
        assert!(r.is_ok(), "positive boundary failed");
    }
    assert_eq!(count.get(), 1);
}
#[test]
fn definition_table_spans_unit_names_and_source_order_are_preserved() {
    let table = "<table><tr><th>숙제수행도DT</th><th>수업이해도DT</th></tr><tr><td rowspan=2>Fictional Unit</td><td>first</td></tr><tr><td>second</td></tr><tr><td>15-1 not a unit match</td><td>other</td></tr></table>";
    let (mut src, count, _) = source(
        PaperScope::Paper1,
        vec![
            ("paper.catalog", catalog()),
            ("paper.definitions", table.into()),
        ],
    );
    let mut request = input(PaperScope::Paper1);
    request.set("kind", "middle1_definitions".into());
    request.set("unitCodes", strings(&["5-1"]));
    request.set("unitNames", obj(&[("5-1", "Fictional Unit".into())]));
    let r = call(&mut src, "academy_source_read_contracts", &request).unwrap();
    let rows = r.get("observation").get("rows").array();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get("cells").array()[1], "first".into());
    assert_eq!(rows[1].get("cells").array()[1], "second".into());
    assert_eq!(count.get(), 2);
}
#[test]
fn table_overlap_unterminated_span_and_overbound_rows_refuse() {
    for table in ["<table><tr><th>숙제수행도DT</th><th>DT</th></tr><tr><td rowspan=2>5-1</td><td>x</td></tr></table>".into(),
        "<table><tr><th>숙제수행도DT</th><th>DT</th></tr><tr><td>5-1</td><td rowspan=2>x</td></tr><tr><td colspan=2>x</td></tr></table>".into(),
        format!("<table>{}</table>","<tr><td>5-1</td><td>x</td></tr>".repeat(2001))] {
        let (mut src,count,_)=source(PaperScope::Paper1,vec![("paper.catalog",catalog()),("paper.definitions",table)]);
        let mut request=input(PaperScope::Paper1);request.set("kind","middle1_definitions".into());request.set("unitCodes",strings(&["5-1"]));
        assert!(call(&mut src,"academy_source_read_contracts",&request).is_err());assert_eq!(count.get(),2);
    }
}

#[test]
fn cancellation_deadline_and_stalled_response_fail_once_and_latch() {
    for code in ["upstream_cancelled", "upstream_deadline_exceeded"] {
        let scope = PaperScope::Paper1;
        let (mut port, count, _) = fake(scope, vec![]);
        port.fault = Some(code);
        let mut src = PaperSource::new(port, owner(scope), admission(scope)).unwrap();
        assert_code(
            call(&mut src, "academy_daily_test_inventory", &input(scope)),
            code,
        );
        assert_code(
            call(&mut src, "academy_daily_test_inventory", &input(scope)),
            "source_failure_latched_no_retry",
        );
        assert_eq!(count.get(), 1);
    }
    let scope = PaperScope::Paper1;
    let (mut port, count, _) = fake(scope, vec![("metadata.daily", result(vec![]))]);
    port.replies[0].1.stalled = true;
    let mut src = PaperSource::new(port, owner(scope), admission(scope)).unwrap();
    assert_code(
        call(&mut src, "academy_daily_test_inventory", &input(scope)),
        "student_read_source_rejected",
    );
    assert_eq!(count.get(), 1);
}
#[test]
fn every_middle1_unit_code_and_explicit_class2_contract_alias_are_supported() {
    let units = vec![
        "5-1", "5-2", "5-3", "5-4", "5-5", "5-6", "5-7", "6-1", "6-2", "6-3", "6-4", "6-5", "7-1",
        "7-2",
    ];
    let rows = units
        .iter()
        .map(|u| format!("<tr><td>{u} Fictional unit</td><td>12</td></tr>"))
        .collect::<String>();
    let scope = PaperScope::Paper2;
    let (mut src, count, _) = source(
        scope,
        vec![
            ("paper.catalog", catalog()),
            ("paper.definitions", format!("<table>{rows}</table>")),
        ],
    );
    let mut request = input(scope);
    request.set("kind", "middle1_definitions".into());
    request.set("unitCodes", strings(&units));
    let value = call(&mut src, "class2_academy_source_read_contracts", &request).unwrap();
    assert_eq!(value.get("observation").get("rows").array().len(), 14);
    assert_eq!(count.get(), 2);
}
#[test]
fn reader_static_quoted_keys_work_and_computed_member_writes_refuse() {
    let scope = PaperScope::Paper1;
    let shell = reader_shell(scope).replace("ready:function", "'ready':function");
    let admitted_owner = owner_with_context(scope, &shell);
    let (port, count, _) = fake(
        scope,
        vec![
            ("metadata.paper.initial", shell),
            ("paper.definition", definition_xml()),
        ],
    );
    let mut src = PaperSource::new(port, admitted_owner, admission(scope)).unwrap();
    assert!(
        call(
            &mut src,
            "academy_paper_definition_lookup",
            &paper_input(scope)
        )
        .is_ok()
    );
    assert_eq!(count.get(), 2);
    for write in [
        "testInfoDialogManager[key]=function(){};",
        "testInfoDialogManager['ready']=function(){};",
        "testInfoDialogManager[key]++;",
        "delete testInfoDialogManager[key];",
    ] {
        let (mut src, count, _) = source(
            scope,
            vec![(
                "metadata.paper.initial",
                reader_shell(scope) + &format!("<script>{write}</script>"),
            )],
        );
        assert_code(
            call(
                &mut src,
                "academy_paper_definition_lookup",
                &paper_input(scope),
            ),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn batch_failure_stops_before_following_codes_or_other_source_reads() {
    let scope = PaperScope::Paper2;
    let (mut src, count, _) = source(
        scope,
        vec![
            ("metadata.paper.initial", reader_shell(scope)),
            ("paper.definition", "<root/>".into()),
        ],
    );
    let mut request = input(scope);
    request.set("paperCodes", strings(&["#5", "#6"]));
    request.set("view", "definition".into());
    assert!(call(&mut src, "class2_academy_paper_headers", &request).is_err());
    assert_code(
        call(&mut src, "academy_daily_test_inventory", &input(scope)),
        "source_failure_latched_no_retry",
    );
    assert_eq!(count.get(), 2);
}
#[test]
fn display_rows_require_an_unambiguous_exact_id_join() {
    let scope = PaperScope::Paper1;
    for duplicate in [false, true] {
        let display =
            "<table><tr id=fictional-row><td>Fictional target</td><td>100</td></tr></table>";
        let mut html = result(vec![row("77001", "Fictional One", "3", "81001")]) + display;
        if duplicate {
            html += display;
        }
        let (mut src, count, _) = source(scope, vec![("metadata.daily", html)]);
        let value = call(&mut src, "academy_daily_test_inventory", &input(scope));
        if duplicate {
            assert_code(value, "contract_drift");
        } else {
            assert_eq!(
                value
                    .unwrap()
                    .get("observation")
                    .get("displayRows")
                    .array()
                    .len(),
                1
            );
        }
        assert_eq!(count.get(), 1);
    }
}
#[test]
fn span_expansion_has_an_aggregate_output_budget() {
    let first = format!(
        "<tr><td rowspan=2000 colspan=20>5-1 {}</td></tr>",
        "x".repeat(3000)
    );
    let table = format!(
        "<table><tr><th>숙제수행도DT</th></tr>{first}{}</table>",
        "<tr></tr>".repeat(1998)
    );
    let scope = PaperScope::Paper1;
    let (mut src, count, _) = source(
        scope,
        vec![("paper.catalog", catalog()), ("paper.definitions", table)],
    );
    let mut request = input(scope);
    request.set("kind", "middle1_definitions".into());
    request.set("unitCodes", strings(&["5-1"]));
    assert_code(
        call(&mut src, "academy_source_read_contracts", &request),
        "paper_output_budget_exceeded",
    );
    assert_eq!(count.get(), 2);
}

struct NativeHistoryLoan {
    source: PaperSource<Fake>,
    fail: bool,
}
impl edu_academy_sources::history::Port for NativeHistoryLoan {
    fn dated_students(&mut self, _: &Js) -> Result<Js> {
        panic!("not used by header loan")
    }
    fn daily_page(&mut self, _: &Js, _: &Js) -> Result<Js> {
        panic!("not used by header loan")
    }
    fn paper_header(&mut self, _: &Js, _: &Js) -> Result<Js> {
        panic!("native failure must never fall back to raw port")
    }
    fn paper_admission(&self) -> Option<&TransportAdmission> {
        Some(self.source.transport_admission())
    }
    fn qualified_paper_header(
        &mut self,
        student: &Js,
        code: &Js,
    ) -> Option<Result<edu_academy_sources::history::QualifiedHeader>> {
        Some(if self.fail {
            Err(Error::refusal("fictional_native_header_failed"))
        } else {
            self.source.history_header(student, code, DATE, NOW)
        })
    }
}
fn history(
    loan: NativeHistoryLoan,
    teacher: &str,
) -> Result<edu_academy_sources::history::History<NativeHistoryLoan>> {
    let mut metadata = policy_value(PaperScope::Paper1);
    metadata.set("teacherId", teacher.into());
    edu_academy_sources::history::History::new(
        loan,
        obj(&[("teacherId", teacher.into())]),
        metadata,
        vec![obj(&[
            ("studentId", "77001".into()),
            ("displayName", "Fictional One".into()),
        ])],
        vec!["Fictional One".into()],
        vec!["77001".into()],
    )
}
#[test]
fn history_native_header_loan_preserves_the_exact_raw_hash_without_question_exports() {
    let scope = PaperScope::Paper1;
    let (src, count, _) = source(
        scope,
        vec![
            ("metadata.paper.initial", reader_shell(scope)),
            ("paper.header", header("3")),
            ("paper.content", content_header()),
        ],
    );
    let mut history = history(
        NativeHistoryLoan {
            source: src,
            fail: false,
        },
        "99001",
    )
    .unwrap();
    let result = history
        .headers(&obj(&[
            ("studentId", "77001".into()),
            ("paperCodes", strings(&["#5"])),
        ]))
        .unwrap();
    let rows = result.get("observation").get("rows").array();
    assert_eq!(
        rows[0].get("contentHeader").get("sourceSha256"),
        &hash(content_header().as_bytes()).into()
    );
    assert!(!json(&result).contains("NEVER_EXPORT_QUESTION"));
    assert_eq!(count.get(), 3);
}
#[test]
fn native_history_admission_mismatch_is_zero_io_and_failed_native_read_never_falls_back() {
    let scope = PaperScope::Paper1;
    let (src, count, _) = source(scope, vec![]);
    assert_code(
        history(
            NativeHistoryLoan {
                source: src,
                fail: false,
            },
            "99002",
        ),
        "native_metadata_scope_refused",
    );
    assert_eq!(count.get(), 0);
    let (src, count, _) = source(scope, vec![]);
    let mut history = history(
        NativeHistoryLoan {
            source: src,
            fail: true,
        },
        "99001",
    )
    .unwrap();
    let request = obj(&[
        ("studentId", "77001".into()),
        ("paperCodes", strings(&["#5"])),
    ]);
    assert_code(history.headers(&request), "fictional_native_header_failed");
    assert!(history.headers(&request).is_err());
    assert_eq!(count.get(), 0);
}
#[test]
fn source_teacher_and_college_conflicts_refuse_after_one_initial_page() {
    let scope = PaperScope::Paper1;
    let (mut src, count, _) = source(
        scope,
        vec![(
            "metadata.paper.initial",
            paper_form().replace("name=pri_no value=99001", "name=pri_no value=99002"),
        )],
    );
    assert_code(
        call(
            &mut src,
            "academy_printed_paper_lookup",
            &printed_input(scope),
        ),
        "student_paper_owner_drift",
    );
    assert_eq!(count.get(), 1);
    let (mut port, count, _) = fake(scope, vec![("metadata.paper.initial", paper_form())]);
    let mut expected = admission(scope);
    expected.college_id = "88002".into();
    port.admission.college_id = "88002".into();
    let mut value = owner_value(scope);
    value.set("collegeSourceKey", "88002".into());
    let configured = PaperOwner::parse(
        scope,
        &routes(),
        &value,
        &policy_value(scope),
        &form_value(),
        pins(),
    )
    .unwrap();
    let mut src = PaperSource::new(port, configured, expected).unwrap();
    assert_code(
        call(
            &mut src,
            "academy_printed_paper_lookup",
            &printed_input(scope),
        ),
        "student_paper_college_scope_conflict",
    );
    assert_eq!(count.get(), 1);
}

#[test]
fn paper2_accepts_both_public_aliases_without_changing_its_admitted_owner() {
    let scope = PaperScope::Paper2;
    let (mut src, count, _) = source(
        scope,
        vec![
            ("metadata.daily", result(vec![])),
            ("metadata.daily", result(vec![])),
        ],
    );
    let generic = src
        .read("academy_daily_test_inventory", &input(scope), DATE, NOW)
        .unwrap();
    let explicit = src
        .read(
            "class2_academy_daily_test_inventory",
            &input(scope),
            DATE,
            NOW,
        )
        .unwrap();
    assert_eq!(generic.get("endpoint"), &"paper2".into());
    assert_eq!(generic.get("observation"), explicit.get("observation"));
    assert_eq!(count.get(), 2);
}
#[test]
fn printed_code_validation_at_the_paper_boundary_is_a_typed_zero_io_refusal() {
    let scope = PaperScope::Paper1;
    let (mut src, count, _) = source(scope, vec![]);
    let mut request = paper_input(scope);
    request.set("paperCode", "#2".into());
    assert_code(
        call(&mut src, "academy_paper_definition_lookup", &request),
        "invalid_printed_paper_code",
    );
    assert_eq!(count.get(), 0);
}

#[test]
fn review_inventory_exports_only_qualified_selector_values_on_both_scopes() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for history in [false, true] {
            let html = daily(scope, vec![])
                .replace(
                    "<select name=sort_date>",
                    "<select name=sort_date id=SYNTHETIC_SECRET value=SYNTHETIC_SECRET>",
                )
                .replace(
                    "</select>",
                    "<option value=SYNTHETIC_SECRET>Unqualified option</option></select>",
                )
                + "<input type=hidden name=page_session value=SYNTHETIC_SECRET><input type=hidden name=updated_nonce value=SYNTHETIC_SECRET><input type=hidden name=sort_date value=SYNTHETIC_SECRET><select name=page_token><option value=SYNTHETIC_SECRET>Extra</option></select><form name=extra method=SYNTHETIC_SECRET action='/unqualified?token=SYNTHETIC_SECRET'><input name=other value=SYNTHETIC_SECRET></form><script src='/unqualified.js?token=SYNTHETIC_SECRET'></script>";
            let mut replies = vec![("metadata.daily", html.clone())];
            if history {
                replies.push(("metadata.daily", html));
            }
            let (mut src, count, _) = source(scope, replies);
            let mut request = input(scope);
            if history {
                request.set("periods", strings(&["11"]));
            }
            let output = call(&mut src, "academy_daily_test_inventory", &request).unwrap();
            assert!(!json(&output).contains("SYNTHETIC_SECRET"));
            let observation = output.get("observation");
            let pages = if history {
                vec![
                    observation.get("currentFilterQualification"),
                    &observation.get("observations").array()[0],
                ]
            } else {
                vec![observation]
            };
            for page in pages {
                let contract = page.get("filterContracts");
                assert!(!contract.has("scriptSources"));
                assert_eq!(
                    contract.get("scriptSourceHashes").array(),
                    &[hash(b"/unqualified.js?token=SYNTHETIC_SECRET").into()]
                );
                let form = contract
                    .get("forms")
                    .array()
                    .iter()
                    .find(|f| f.get("name").is("extra"))
                    .unwrap();
                assert!(!form.has("action"));
                assert_eq!(
                    form.get("actionSha256"),
                    &hash(b"/unqualified?token=SYNTHETIC_SECRET").into()
                );
                assert_eq!(form.get("method"), &"unqualified".into());
                let controls = contract.get("controls").array();
                assert_eq!(controls.len(), 1);
                assert_eq!(controls[0].get("name"), &"sort_date".into());
                assert_eq!(controls[0].get("options").array().len(), 7);
                assert_eq!(controls[0].get("value"), &"1".into());
                assert!(!controls[0].has("id"));
            }
            if history {
                assert_eq!(
                    observation.get("observations").array()[0].get("label"),
                    &"Fictional 11".into()
                );
            }
            assert_eq!(count.get(), if history { 2 } else { 1 });
        }
    }
}

#[test]
fn review_reader_requires_an_unconditional_active_program_binding() {
    let scope = PaperScope::Paper1;
    let original = reader_shell(scope);
    let declaration = original
        .rsplit("<script>")
        .next()
        .unwrap()
        .strip_suffix("</script>")
        .unwrap();
    for wrapped in [
        format!("function dormant(){{{declaration}}}"),
        format!("if(false){{{declaration}}}"),
        format!("(()=>{{{declaration}}})();"),
        format!("{{{}}}", declaration.replace("var ", "let ")),
    ] {
        let html = original.replace(declaration, &wrapped);
        let admitted_owner = owner_with_context(scope, &html);
        let (port, count, _) = fake(
            scope,
            vec![
                ("metadata.paper.initial", html),
                ("paper.definition", definition_xml()),
            ],
        );
        let mut src = PaperSource::new(port, admitted_owner, admission(scope)).unwrap();
        assert_code(
            call(
                &mut src,
                "academy_paper_definition_lookup",
                &paper_input(scope),
            ),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
    for attr in [
        "type=module",
        "type=text/plain",
        "nomodule",
        "type=application/json",
    ] {
        let html = original.replace(
            &format!("<script>{declaration}"),
            &format!("<script {attr}>{declaration}"),
        );
        let admitted_owner = owner_with_context(scope, &html);
        let (port, count, _) = fake(
            scope,
            vec![
                ("metadata.paper.initial", html),
                ("paper.definition", definition_xml()),
            ],
        );
        let mut src = PaperSource::new(port, admitted_owner, admission(scope)).unwrap();
        assert_code(
            call(
                &mut src,
                "academy_paper_definition_lookup",
                &paper_input(scope),
            ),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
    }
}

#[test]
fn review_srcdoc_overrides_refuse_before_child_read_and_latch_native_history() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for attr in ["srcdoc=''", "SrCdOc='<h1>Unqualified</h1>'"] {
            let (mut src, count, _) = source(
                scope,
                vec![
                    ("metadata.paper.initial", reader_shell(scope)),
                    (
                        "paper.header",
                        header("3").replace("<iframe ", &format!("<iframe {attr} ")),
                    ),
                    ("paper.content", content_header()),
                ],
            );
            assert_code(
                src.history_header(input(scope).get("studentId"), &"#5".into(), DATE, NOW),
                "printed_paper_frame_unqualified",
            );
            assert_eq!(count.get(), 2);
            assert_code(
                call(&mut src, "academy_daily_test_inventory", &input(scope)),
                "source_failure_latched_no_retry",
            );
            assert_eq!(count.get(), 2);
        }
    }
}

#[test]
fn review_reader_context_refuses_abrupt_prefix_and_indirect_mutation() {
    let scope = PaperScope::Paper1;
    let original = reader_shell(scope);
    let declaration = original
        .rsplit("<script>")
        .next()
        .unwrap()
        .strip_suffix("</script>")
        .unwrap();
    let mut violations = vec![];
    for (case, html) in [
        (
            "abrupt_prefix",
            original.replace(declaration, &format!("throw 0;{declaration}")),
        ),
        (
            "indirect_mutation",
            original.clone()
                + "<script>var alias=testInfoDialogManager;alias.ready=function(){};</script>",
        ),
    ] {
        let (mut src, count, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", html),
                ("paper.definition", definition_xml()),
            ],
        );
        let result = call(
            &mut src,
            "academy_paper_definition_lookup",
            &paper_input(scope),
        );
        if result.is_ok() || count.get() != 1 {
            violations.push(case);
        }
    }
    assert!(
        violations.is_empty(),
        "unqualified reader contexts: {violations:?}"
    );
}

#[test]
fn review_document_base_overrides_refuse_before_child_read_on_both_scopes() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for base in [
            "<base href='https://unqualified.example/'>",
            "<base HREF='//unqualified.example/'>",
            "<base href=''><base href='https://unqualified.example/'>",
            "<base href='/servlet/'>",
        ] {
            let (mut src, count, _) = source(
                scope,
                vec![
                    ("metadata.paper.initial", reader_shell(scope)),
                    ("paper.header", format!("{base}{}", header("3"))),
                    ("paper.content", content_header()),
                ],
            );
            assert_code(
                src.history_header(input(scope).get("studentId"), &"#5".into(), DATE, NOW),
                "printed_paper_frame_unqualified",
            );
            assert_eq!(count.get(), 2);
            assert_code(
                call(&mut src, "academy_daily_test_inventory", &input(scope)),
                "source_failure_latched_no_retry",
            );
            assert_eq!(count.get(), 2);
        }
        let (mut src, count, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", reader_shell(scope)),
                (
                    "paper.header",
                    format!("<base target='_self'>{}", header("3")),
                ),
                ("paper.content", content_header()),
            ],
        );
        assert!(
            src.history_header(input(scope).get("studentId"), &"#5".into(), DATE, NOW)
                .is_ok()
        );
        assert_eq!(count.get(), 3);
    }
}

#[test]
fn review_definition_attributes_are_hashed_across_grid_fallback_and_row_copies() {
    let handler = "viewPaper('SYNTHETIC_SECRET')";
    let child_handler = "child('SYNTHETIC_SECRET')";
    let href = "/definition?session=SYNTHETIC_SECRET";
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for (kind, unit) in [
            ("middle1_definitions", "5-1"),
            ("elementary5_fraction_definitions", "2-2"),
        ] {
            for grid in [false, true] {
                let heading = if grid {
                    "<tr><th>숙제수행도DT</th><th>Level</th><th>Count</th></tr>"
                } else {
                    ""
                };
                let span = if grid { "rowspan=2 colspan=2" } else { "" };
                let tail = if grid { "<tr><td>13</td></tr>" } else { "" };
                let table = format!(
                    r#"<table>{heading}<tr><td {span} onclick="{handler}">{unit} Fictional definition<a href="{href}" onclick="{child_handler}">Fictional link</a></td><td>12</td></tr>{tail}</table>"#
                );
                let (mut src, count, _) = source(
                    scope,
                    vec![("paper.catalog", catalog()), ("paper.definitions", table)],
                );
                let mut request = input(scope);
                request.set("kind", kind.into());
                if kind == "middle1_definitions" {
                    request.set("unitCodes", strings(&[unit]));
                }
                let output = call(&mut src, "academy_source_read_contracts", &request).unwrap();
                assert!(!json(&output).contains("SYNTHETIC_SECRET"));
                let rows = output.get("observation").get("rows").array();
                assert_eq!(rows.len(), if grid { 2 } else { 1 });
                for row in rows {
                    assert_eq!(row.get("matchedUnitCodes").array(), &[unit.into()]);
                    assert!(string(&row.get("cells").array()[0]).contains("Fictional definition"));
                    let column = &row.get("columns").array()[0];
                    assert!(!column.has("handlers") && !column.has("links"));
                    assert_eq!(
                        column.get("handlerHashes").array(),
                        &[
                            hash(handler.as_bytes()).into(),
                            hash(child_handler.as_bytes()).into()
                        ]
                    );
                    assert_eq!(
                        column.get("linkHashes").array(),
                        &[hash(href.as_bytes()).into()]
                    );
                    assert!(!row.has("handlers") && !row.has("links"));
                    assert_eq!(
                        row.get("handlerHashes").array().len(),
                        if grid { 4 } else { 2 }
                    );
                    assert_eq!(
                        row.get("linkHashes").array().len(),
                        if grid { 2 } else { 1 }
                    );
                }
                assert_eq!(count.get(), 2);
            }
        }
    }
}
