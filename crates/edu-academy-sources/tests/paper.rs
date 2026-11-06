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
    pins_for_pages(reference, &catalog(), &pool())
}
fn pins_for_pages(reference: &str, catalog_page: &str, pool_page: &str) -> PaperPins {
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
            paper::contract_fingerprint(catalog_page).unwrap().into(),
        ),
        (
            "pool",
            paper::contract_fingerprint(pool_page).unwrap().into(),
        ),
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
                    .find(|f| {
                        f.get("nameSha256") == &hash(json(&Js::from("extra")).as_bytes()).into()
                    })
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

#[test]
fn review_definition_text_excludes_non_displayed_code() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for (kind, unit) in [
            ("middle1_definitions", "5-1"),
            ("elementary5_fraction_definitions", "2-2"),
        ] {
            for grid in [false, true] {
                let heading = if grid {
                    "<tr><th>숙제수행도DT<script>HEADER_SECRET</script></th><th>Count</th></tr>"
                } else {
                    ""
                };
                let table = format!(
                    "<table>{heading}<tr><td>{unit} Fictional definition<script>var page_session='SYNTHETIC_SECRET';</script><template>TEMPLATE_SECRET</template><noscript>NOSCRIPT_SECRET</noscript></td><td>12</td></tr></table>"
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
                let serialized = json(&output);
                assert!(
                    !serialized.contains("SECRET"),
                    "non-displayed source text exported"
                );
                let rows = output.get("observation").get("rows").array();
                assert_eq!(rows.len(), 1);
                assert_eq!(
                    string(&rows[0].get("cells").array()[0]),
                    format!("{unit} Fictional definition")
                );
                assert_eq!(count.get(), 2);
            }
        }
    }
}

#[test]
fn review_reader_context_pins_html_event_and_embedded_code() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for extra in [
            "<body onload=\"testInfoDialogManager.ready=function(){}\">",
            "<img src=missing onerror=\"testInfoDialogManager.ready=function(){}\">",
            "<svg onload=\"testInfoDialogManager.ready=function(){}\"></svg>",
            "<iframe srcdoc=\"<script>parent.testInfoDialogManager.ready=function(){}</script>\"></iframe>",
            "<a href=\"javascript:testInfoDialogManager.ready=function(){}\">link</a>",
        ] {
            let html = reader_shell(scope) + extra;
            let (mut src, count, _) = source(
                scope,
                vec![
                    ("metadata.paper.initial", html),
                    ("paper.definition", definition_xml()),
                ],
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
        let reference = reader_shell(scope) + "<button onclick=\"return false\">Fictional</button>";
        let admitted = owner_with_context(scope, &reference);
        let (port, count, _) = fake(
            scope,
            vec![
                ("metadata.paper.initial", reference),
                ("paper.definition", definition_xml()),
            ],
        );
        let mut src = PaperSource::new(port, admitted, admission(scope)).unwrap();
        assert!(
            call(
                &mut src,
                "academy_paper_definition_lookup",
                &paper_input(scope)
            )
            .is_ok()
        );
        assert_eq!(count.get(), 2);
    }
}

#[test]
fn review_contract_select_values_and_labels_are_opaque() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let shell = reader_shell(scope)
            + "<select name=timezone id=SYNTHETIC_SECRET><option value=SYNTHETIC_SECRET selected>SYNTHETIC_SECRET</option></select>";
        let admitted = owner_with_context(scope, &shell);
        let (port, count, _) = fake(
            scope,
            vec![("metadata.paper.initial", shell), ("paper.pool", pool())],
        );
        let mut src = PaperSource::new(port, admitted, admission(scope)).unwrap();
        let output = call(&mut src, "academy_paper_read_contracts", &input(scope)).unwrap();
        assert!(!json(&output).contains("SYNTHETIC_SECRET"));
        assert_eq!(count.get(), 2);
        let selects = output
            .get("observation")
            .get("studentPaperForm")
            .get("selects")
            .array();
        let select = selects
            .iter()
            .find(|s| s.get("nameSha256") == &hash(json(&Js::from("timezone")).as_bytes()).into())
            .unwrap();
        assert_eq!(select.get("optionCount"), &1.into());
        assert_eq!(string(select.get("contractSha256")).len(), 64);
    }
}

#[test]
fn review_borrowed_daily_refuses_non_displayed_selector_code() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for code in [
            "<script>var page_session='SYNTHETIC_SECRET';</script>",
            "<style>STYLE_SECRET</style>",
        ] {
            let html = daily(scope, vec![]).replace("Recent", &format!("Recent{code}"));
            let (mut src, count, _) = source(scope, vec![("metadata.daily", html)]);
            assert_code(
                call(&mut src, "academy_daily_test_inventory", &input(scope)),
                "contract_drift",
            );
            assert_eq!(count.get(), 1);
        }
    }
}

#[test]
fn review_inert_allowlist_never_exports_attribute_or_value_canaries() {
    // Each concrete slot is independently mutated. Unknown attributes also
    // exercise the default projection rather than a name-specific denylist.
    fn public_bytes(result: Result<Js>, canary: &str) -> Option<Js> {
        match result {
            Ok(output) => {
                assert!(!json(&output).contains(canary));
                Some(output)
            }
            Err(error) => {
                assert!(!format!("{error:?}").contains(canary));
                None
            }
        }
    }
    let mut admitted = [0usize; 5];
    let elements = [
        (
            "form",
            vec![
                "name",
                "id",
                "action",
                "method",
                "data-session",
                "style",
                "title",
                "onload",
            ],
        ),
        (
            "input",
            vec![
                "name",
                "id",
                "type",
                "value",
                "form",
                "checked",
                "disabled",
                "required",
                "data-token",
                "style",
                "onchange",
            ],
        ),
        (
            "select",
            vec![
                "name",
                "id",
                "value",
                "form",
                "multiple",
                "data-token",
                "style",
                "onchange",
            ],
        ),
        (
            "option",
            vec![
                "value",
                "label",
                "id",
                "selected",
                "disabled",
                "data-token",
                "style",
                "onclick",
            ],
        ),
        (
            "a",
            vec![
                "href",
                "target",
                "name",
                "id",
                "data-token",
                "style",
                "onclick",
            ],
        ),
        ("img", vec!["src", "srcset", "alt", "onerror", "data-token"]),
        ("iframe", vec!["src", "srcdoc", "name", "sandbox", "onload"]),
        (
            "div",
            vec![
                "id",
                "class",
                "data-token",
                "style",
                "title",
                "unknown-future-attribute",
            ],
        ),
        (
            "textarea",
            vec!["name", "value", "placeholder", "data-token"],
        ),
        ("button", vec!["name", "value", "formaction", "onclick"]),
    ];
    let mut cases = vec![];
    for (tag, attrs) in elements {
        for attr in attrs {
            let canary = format!("e09b_canary_{}", cases.len());
            let element = format!("<{tag} {attr}=\"{canary}\">Visible</{tag}>");
            cases.push((
                canary,
                if tag == "option" {
                    format!("<select>{element}</select>")
                } else {
                    element
                },
            ));
        }
        let canary = format!("e09b_canary_{}", cases.len());
        cases.push((
            canary.clone(),
            format!("<{tag} {canary}=opaque>Visible</{tag}>"),
        ));
    }
    for source in [
        "<script>function SLOT(){return 'Visible';}</script>",
        "<script>var opaqueValue='SLOT';</script>",
    ] {
        let canary = format!("e09b_canary_{}", cases.len());
        cases.push((canary.clone(), source.replace("SLOT", &canary)));
    }
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for (canary, extra) in &cases {
            // Current/history copies, all inert helper fields, and source script identifiers.
            let html = daily(scope, vec![]) + extra;
            let (mut src, _, _) = source(
                scope,
                vec![("metadata.daily", html.clone()), ("metadata.daily", html)],
            );
            let mut request = input(scope);
            request.set("periods", strings(&["11"]));
            if let Some(output) = public_bytes(
                call(&mut src, "academy_daily_test_inventory", &request),
                canary,
            ) {
                admitted[0] += 1;
                assert!(
                    !json(&output).contains(canary),
                    "daily slot leaked: {canary}"
                );
            }
            for grid in [false, true] {
                let heading = if grid {
                    "<tr><th>숙제수행도DT</th><th>Count</th></tr>"
                } else {
                    ""
                };
                let table = format!(
                    "<table>{heading}<tr><td>5-1 Visible{extra}</td><td>12</td></tr></table>"
                );
                let (mut src, _, _) = source(
                    scope,
                    vec![("paper.catalog", catalog()), ("paper.definitions", table)],
                );
                let mut request = input(scope);
                request.set("kind", "middle1_definitions".into());
                request.set("unitCodes", strings(&["5-1"]));
                if let Some(output) = public_bytes(
                    call(&mut src, "academy_source_read_contracts", &request),
                    canary,
                ) {
                    admitted[1] += 1;
                    assert!(
                        !json(&output).contains(canary),
                        "definition slot leaked: {canary}"
                    );
                }
            }
            let (mut src, _, _) = source(
                scope,
                vec![
                    ("metadata.paper.initial", reader_shell(scope)),
                    ("paper.header", header("3")),
                    ("paper.content", content_header() + extra),
                ],
            );
            let mut request = paper_input(scope);
            request.set("view", "header".into());
            if let Some(output) = public_bytes(
                call(&mut src, "academy_paper_definition_lookup", &request),
                canary,
            ) {
                admitted[2] += 1;
                assert!(
                    !json(&output).contains(canary),
                    "header slot leaked: {canary}"
                );
            }
            let shell = reader_shell(scope) + extra;
            let catalog_page = catalog() + extra;
            let pool_page = pool() + extra;
            if paper::reader_context_fingerprint(&shell).is_err()
                || paper::contract_fingerprint(&catalog_page).is_err()
                || paper::contract_fingerprint(&pool_page).is_err()
            {
                continue; // Explicit ownership/budget refusal; no public bytes.
            }
            let pins = pins_for_pages(&shell, &catalog_page, &pool_page);
            let admitted_owner = PaperOwner::parse(
                scope,
                &routes(),
                &owner_value(scope),
                &policy_value(scope),
                &remap_js(form_value(), scope),
                pins,
            )
            .unwrap();
            let (port, _, _) = fake(
                scope,
                vec![
                    ("metadata.paper.initial", shell),
                    ("paper.pool", pool_page),
                    ("paper.catalog", catalog_page),
                ],
            );
            let mut src = PaperSource::new(port, admitted_owner, admission(scope)).unwrap();
            if let Some(output) = public_bytes(
                call(&mut src, "academy_paper_read_contracts", &input(scope)),
                canary,
            ) {
                admitted[3] += 1;
                assert!(
                    !json(&output).contains(canary),
                    "contracts slot leaked: {canary}"
                );
                let mut request = input(scope);
                request.set("kind", "daily_test_catalog".into());
                let output = public_bytes(
                    call(&mut src, "academy_source_read_contracts", &request),
                    canary,
                )
                .unwrap();
                admitted[4] += 1;
                assert!(
                    !json(&output).contains(canary),
                    "catalog slot leaked: {canary}"
                );
            }
        }
    }
    assert!(
        admitted.iter().all(|count| *count >= cases.len()),
        "each public family must actually admit the canaries: {admitted:?}"
    );
}

#[test]
fn review_reader_context_pins_svg_animation_and_unknown_attributes() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let reference = reader_shell(scope) + "<svg><a href='#safe'>Visible</a></svg>";
        let changed = reference.replace("Visible</a>", "Visible<animate attributeName='href' values='javascript:testInfoDialogManager.ready=function(){}'/></a>");
        assert_ne!(
            paper::reader_context_fingerprint(&reference).unwrap(),
            paper::reader_context_fingerprint(&changed).unwrap()
        );
        let admitted = owner_with_context(scope, &reference);
        let (port, count, _) = fake(
            scope,
            vec![
                ("metadata.paper.initial", changed),
                ("paper.definition", definition_xml()),
            ],
        );
        let mut src = PaperSource::new(port, admitted, admission(scope)).unwrap();
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
fn review_visible_text_excludes_hidden_and_unqualified_visibility_contexts() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for grid in [false, true] {
            for content in [
                "5-1 Visible<span hidden>SYNTHETIC_SECRET</span>",
                "5-1 Visible<span hidden=false>SYNTHETIC_SECRET</span>",
                "5-1 Visible<span hidden=until-found>SYNTHETIC_SECRET</span>",
                "5-1 Visible<span aria-hidden=TRUE>SYNTHETIC_SECRET</span>",
                "5-1 Visible<span inert>SYNTHETIC_SECRET</span>",
                "5-1 Visible<span style='display: none !important'>SYNTHETIC_SECRET</span>",
                "5-1 Visible<span class=unknown hidden>SYNTHETIC_SECRET</span>",
                "5-1 Visible<dialog>SYNTHETIC_SECRET</dialog>",
                "5-1 Visible<span popover>SYNTHETIC_SECRET</span>",
                "5-1 Visible<canvas>SYNTHETIC_SECRET</canvas>",
                "5-1 Visible<svg width=0 height=0><text>SYNTHETIC_SECRET</text></svg>",
                "5-1 Visible<object>SYNTHETIC_SECRET</object>",
                "5-1 Visible<x-unknown>SYNTHETIC_SECRET</x-unknown>",
                "5-1 Visible<details>SYNTHETIC_SECRET</details>",
                "5-1 Visible<select><option label=Public>SYNTHETIC_SECRET</option></select>",
                "5-1 Visible<select><option>Public</option><option>SYNTHETIC_SECRET</option></select>",
                "5-1 Visible<svg><text visibility=hidden>SYNTHETIC_SECRET</text></svg>",
            ] {
                let heading = if grid {
                    "<tr><th>숙제수행도DT</th><th>Count</th></tr>"
                } else {
                    ""
                };
                let table =
                    format!("<table>{heading}<tr><td>{content}</td><td>12</td></tr></table>");
                let (mut src, _, _) = source(
                    scope,
                    vec![("paper.catalog", catalog()), ("paper.definitions", table)],
                );
                let mut request = input(scope);
                request.set("kind", "middle1_definitions".into());
                request.set("unitCodes", strings(&["5-1"]));
                let output = call(&mut src, "academy_source_read_contracts", &request).unwrap();
                assert!(!json(&output).contains("SYNTHETIC_SECRET"));
                assert!(json(&output).contains("5-1 Visible"));
            }
        }
    }
}

#[test]
fn review_hidden_roots_ancestors_and_css_pages_never_export_text() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for table in [
            "<table><tr><td hidden>5-1 SYNTHETIC_SECRET</td><td>12</td></tr></table>",
            "<table><tr hidden><td>5-1 SYNTHETIC_SECRET</td><td>12</td></tr></table>",
            "<table hidden><tr><td>5-1 SYNTHETIC_SECRET</td><td>12</td></tr></table>",
        ] {
            let (mut src, _, _) = source(
                scope,
                vec![
                    ("paper.catalog", catalog()),
                    ("paper.definitions", table.into()),
                ],
            );
            let mut request = input(scope);
            request.set("kind", "middle1_definitions".into());
            request.set("unitCodes", strings(&["5-1"]));
            let output = call(&mut src, "academy_source_read_contracts", &request).unwrap();
            assert!(!json(&output).contains("SYNTHETIC_SECRET"));
        }
        for css in [
            "<style>td{display:none}</style>",
            "<link rel='alternate StyleSheet' href='/opaque.css'>",
        ] {
            let table =
                format!("{css}<table><tr><td>5-1 SYNTHETIC_SECRET</td><td>12</td></tr></table>");
            let (mut src, count, _) = source(
                scope,
                vec![("paper.catalog", catalog()), ("paper.definitions", table)],
            );
            let mut request = input(scope);
            request.set("kind", "middle1_definitions".into());
            request.set("unitCodes", strings(&["5-1"]));
            assert_code(
                call(&mut src, "academy_source_read_contracts", &request),
                "contract_drift",
            );
            assert_eq!(count.get(), 2);
            let (mut src, count, _) =
                source(scope, vec![("metadata.daily", daily(scope, vec![]) + css)]);
            assert_code(
                call(&mut src, "academy_daily_test_inventory", &input(scope)),
                "contract_drift",
            );
            assert_eq!(count.get(), 1);
        }
        let html = daily(scope, vec![]).replace(
            "<option value=1>Recent",
            "<option value=1 label=Recent>SYNTHETIC_SECRET",
        );
        let (mut src, count, _) = source(scope, vec![("metadata.daily", html)]);
        assert_code(
            call(&mut src, "academy_daily_test_inventory", &input(scope)),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
        let html = daily(scope, vec![]).replace("<option value=1>", "<option hidden value=1>");
        let (mut src, count, _) = source(scope, vec![("metadata.daily", html)]);
        assert_code(
            call(&mut src, "academy_daily_test_inventory", &input(scope)),
            "contract_drift",
        );
        assert_eq!(count.get(), 1);
        let child = content_header()
            + "<div class=test-title>Fictional class title</div><h1 hidden>SYNTHETIC_SECRET</h1><div hidden><h2>SYNTHETIC_SECRET</h2></div>";
        let (mut src, _, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", reader_shell(scope)),
                ("paper.header", header("3")),
                ("paper.content", child),
            ],
        );
        let mut request = paper_input(scope);
        request.set("view", "header".into());
        let output = call(&mut src, "academy_paper_definition_lookup", &request).unwrap();
        assert!(!json(&output).contains("SYNTHETIC_SECRET"));
        assert!(json(&output).contains("Fictional title"));
        assert!(json(&output).contains("Fictional class title"));
    }
}

#[test]
fn review_display_text_requires_safe_per_tag_attributes_by_default() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for (kind, unit) in [
            ("middle1_definitions", "5-1"),
            ("elementary5_fraction_definitions", "2-2"),
        ] {
            for (attrs, content) in [
                ("", "5-1 Visible<title>SYNTHETIC_SECRET</title>"),
                (
                    "bgcolor=white",
                    "5-1 <font color=white hidden>SYNTHETIC_SECRET</font>",
                ),
                (
                    "",
                    "5-1 Visible<font color=white hidden>SYNTHETIC_SECRET</font>",
                ),
                (
                    "",
                    "5-1 Visible<span background=opaque hidden>SYNTHETIC_SECRET</span>",
                ),
                (
                    "",
                    "5-1 Visible<span data-future-hint=opaque>SYNTHETIC_SECRET</span>",
                ),
                (
                    "",
                    "5-1 Visible<span unknown-presentation=opaque>SYNTHETIC_SECRET</span>",
                ),
            ] {
                for grid in [false, true] {
                    let heading = if grid {
                        "<tr><th>숙제수행도DT</th><th>Count</th></tr>"
                    } else {
                        ""
                    };
                    let content = content.replace("5-1", unit);
                    let table = format!(
                        "<table>{heading}<tr><td {attrs}>{content}</td><td>12</td></tr></table>"
                    );
                    let (mut src, _, _) = source(
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
                    if attrs.is_empty() {
                        assert!(json(&output).contains(&format!("{unit} Visible")));
                    }
                }
            }
        }
    }
}

#[test]
fn review_document_title_is_separate_unambiguous_head_metadata() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let (mut src, _, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", reader_shell(scope)),
                ("paper.header", header("3")),
                ("paper.content", content_header()),
            ],
        );
        let mut request = paper_input(scope);
        request.set("view", "header".into());
        let output = call(&mut src, "academy_paper_definition_lookup", &request).unwrap();
        assert_eq!(
            output
                .get("observation")
                .get("information")
                .get("documentTitle"),
            &"Fictional header".into()
        );
        assert_eq!(
            output
                .get("observation")
                .get("contentHeader")
                .get("documentTitle"),
            &"".into()
        );
        for child in [
            "<h1>Visible</h1><title>SYNTHETIC_SECRET</title>",
            "<title>Visible</title><title>SYNTHETIC_SECRET</title>",
            "<table><tr><td><title>SYNTHETIC_SECRET</title></td></tr></table>",
            "<svg><title>SYNTHETIC_SECRET</title></svg>",
        ] {
            let (mut src, count, _) = source(
                scope,
                vec![
                    ("metadata.paper.initial", reader_shell(scope)),
                    ("paper.header", header("3")),
                    ("paper.content", child.into()),
                ],
            );
            assert_code(
                call(&mut src, "academy_paper_definition_lookup", &request),
                "contract_drift",
            );
            assert_eq!(count.get(), 3);
        }
    }
}

// FIX-1: independently reconstructed review matrices; synthetic source only.
fn fix1_source(
    scope: PaperScope,
    replies: Vec<(&str, String)>,
    stylesheets: &[&str],
    catalog_page: &str,
    pool_page: &str,
    program: &str,
    shell: &str,
) -> (PaperSource<Fake>, Rc<Cell<usize>>) {
    let pins = PaperPins::parse(&obj(&[
        ("schemaVersion", "academy.paper-pins/1".into()),
        (
            "readerContext",
            paper::reader_context_fingerprint(shell).unwrap().into(),
        ),
        (
            "ready",
            hash(b"function(){return 'fictional-ready';}").into(),
        ),
        ("view", hash(b"function(){return 'fictional-view';}").into()),
        (
            "catalog",
            paper::contract_fingerprint(catalog_page).unwrap().into(),
        ),
        (
            "pool",
            paper::contract_fingerprint(pool_page).unwrap().into(),
        ),
        ("converter", hash(program.as_bytes()).into()),
        ("stylesheetUrls", strings(stylesheets)),
    ]))
    .unwrap();
    let owner = PaperOwner::parse(
        scope,
        &routes(),
        &owner_value(scope),
        &policy_value(scope),
        &remap_js(form_value(), scope),
        pins,
    )
    .unwrap();
    let (port, count, _) = fake(scope, replies);
    (
        PaperSource::new(port, owner, admission(scope)).unwrap(),
        count,
    )
}
fn fix1_page(scope: PaperScope, family: &str, body: &str, stylesheets: &[&str]) -> Result<Js> {
    let replies = match family {
        "header" | "batch" => vec![
            ("metadata.paper.initial", reader_shell(scope)),
            ("paper.header", header("3")),
            ("paper.content", body.into()),
        ],
        "inventory" => vec![("metadata.daily", body.into())],
        "definitions" => vec![
            ("paper.catalog", catalog()),
            ("paper.definitions", body.into()),
        ],
        _ => panic!("unknown synthetic family"),
    };
    let (mut src, count) = fix1_source(
        scope,
        replies,
        stylesheets,
        &catalog(),
        &pool(),
        &converter_program(),
        &reader_shell(scope),
    );
    let mut q = input(scope);
    let (tool, expected_requests) = match family {
        "header" => {
            q = paper_input(scope);
            q.set("view", "header".into());
            ("academy_paper_definition_lookup", 3)
        }
        "batch" => {
            q.set("paperCodes", strings(&["#5"]));
            ("academy_paper_headers", 3)
        }
        "inventory" => ("academy_daily_test_inventory", 1),
        _ => {
            q.set("kind", "middle1_definitions".into());
            q.set("unitCodes", strings(&["5-1"]));
            ("academy_source_read_contracts", 2)
        }
    };
    let result = call(&mut src, tool, &q);
    assert_eq!(count.get(), expected_requests);
    if result.is_err() {
        assert_code(
            call(&mut src, "academy_daily_test_inventory", &input(scope)),
            "source_failure_latched_no_retry",
        );
        assert_eq!(count.get(), expected_requests);
    }
    result
}
fn fix1_bytes(result: &Result<Js>) -> String {
    match result {
        Ok(v) => json(v),
        Err(e) => format!("{} {}", e.message, json(&e.properties)),
    }
}
fn fix1_evidence(name: &str, records: &[Js]) {
    if let Ok(root) = std::env::var("E09B_FIX1_EVIDENCE") {
        std::fs::write(
            std::path::Path::new(&root).join(format!("{name}.json")),
            json(&Js::Arr(records.to_vec())),
        )
        .unwrap();
    }
}
fn fix1_decorate(body: &str, variant: &str) -> String {
    let mut body = body.to_owned();
    let (html, head, attrs) = match variant {
        "plain" => ("", "", ""),
        "known-heading-class" => {
            body = body.replace("<h1>", "<h1 class=test-title>");
            ("", "", "")
        }
        "table-border" => {
            body = body.replace("<table>", "<table border=1>");
            ("", "", "")
        }
        "table-padding" => {
            body = body.replace("<table>", "<table cellpadding=2 cellspacing=0>");
            ("", "", "")
        }
        "table-width" => {
            body = body.replace("<table>", "<table width='100%'>");
            ("", "", "")
        }
        "cell-align" => {
            body = body.replace("<td>", "<td align=center>");
            ("", "", "")
        }
        "body-inline-style" => ("", "", "style='font-family:sans-serif'"),
        "body-class" => ("", "", "class=main-content"),
        "html-lang" => ("lang=ko", "", ""),
        "external-stylesheet" | "unpinned-link" => {
            ("", "<link rel=stylesheet href='/fictional.css'>", "")
        }
        "inline-stylesheet" => (
            "",
            "<style>table {border-collapse:collapse} th {font-weight:bold}</style>",
            "",
        ),
        "hiding-stylesheet" => ("", "<style>td{display:none}</style>", ""),
        "body-bgcolor" => ("", "", "bgcolor='#ffffff'"),
        _ => panic!("unknown synthetic presentation"),
    };
    format!(
        "<!doctype html><html {html}><head><title>NORMAL_DOCUMENT</title>{head}</head><body {attrs}>{body}</body></html>"
    )
}
#[test]
fn fix1_usefulness_72_cases_and_explicit_stylesheet_refusals() {
    let variants = [
        "plain",
        "known-heading-class",
        "table-border",
        "table-padding",
        "table-width",
        "cell-align",
        "body-inline-style",
        "body-class",
        "html-lang",
        "external-stylesheet",
        "inline-stylesheet",
        "body-bgcolor",
    ];
    let mut records = vec![];
    let mut totals = [0_usize; 12];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for (index, variant) in variants.iter().enumerate() {
            for family in ["header", "inventory", "definitions"] {
                let (body, wanted) = match family {
                    "header" => ("<h1>NORMAL_HEADING</h1><table><tr><th>시험지 제목</th><td>NORMAL_PAPER_TITLE</td></tr><tr><th>총 문제수</th><td>12 NORMAL_COUNT</td></tr></table>".into(), vec!["NORMAL_DOCUMENT", "NORMAL_HEADING", "NORMAL_PAPER_TITLE", "12 NORMAL_COUNT", "시험지 제목", "총 문제수"]),
                    "inventory" => (daily(scope, vec![row("77001", "Fictional One", "3", "81001")])+"<table><tr id=fictional-row><td>NORMAL_DT_TITLE</td><td>100 NORMAL_SCORE</td></tr></table>", vec!["NORMAL_DT_TITLE", "100 NORMAL_SCORE", if scope == PaperScope::Paper1 {"Fictional One"} else {"Fictional Two"}, "Fictional source title", "Recent", "Fictional 11", "Fictional 12", "Fictional 13", "Fictional 14", "Fictional 15", "Fictional 16"]),
                    _ => ("<table><tr><td>5-1 NORMAL_DEFINITION</td><td>12 NORMAL_ITEMS</td></tr></table>".into(), vec!["5-1 NORMAL_DEFINITION", "12 NORMAL_ITEMS", "[22개정] 중1"]),
                };
                let output = fix1_page(
                    scope,
                    family,
                    &fix1_decorate(&body, variant),
                    &["/assets/../fictional.css#owner-pin"],
                )
                .unwrap();
                let bytes = json(&output);
                let kept = wanted.iter().filter(|text| bytes.contains(**text)).count();
                assert_eq!(kept, wanted.len(), "{variant}/{family}");
                totals[index] += kept;
                records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("variant", (*variant).into()),
                    ("family", family.into()),
                    ("expected", wanted.len().into()),
                    ("kept", kept.into()),
                ]));
                if *variant == "plain" {
                    for refusing in ["unpinned-link", "hiding-stylesheet"] {
                        assert_code(
                            fix1_page(scope, family, &fix1_decorate(&body, refusing), &[]),
                            "contract_drift",
                        );
                    }
                }
            }
        }
    }
    assert_eq!(records.len(), 72);
    assert_eq!(totals, [40; 12]);
    fix1_evidence("usefulness-72", &records);
}
#[test]
fn fix1_css_hiding_declarations_and_conservative_grammar() {
    let hiding = [
        "display:none",
        "DISPLAY : n\\6f ne !important",
        "d\\69splay:n/**/one",
        "visibility:hidden",
        "visibility:collapse",
        "content-visibility:hidden",
        "opacity:0",
        "opacity:0.00%",
        "font-size:0px",
        "clip:rect(0px,0px,0px,0px)",
        "clip-path:circle(0px)",
        "clip-path:inset(50%)",
        "position:absolute;left:-10000px",
        "position:fixed;top:-9999em",
        "text-indent:-9999px",
        "color:transparent",
        "filter:opacity(0)",
        "-webkit-filter:opacity(0)",
        "mask-image:linear-gradient(transparent,transparent)",
        "position:absolute;clip:rect(0px,0px,100px,0px)",
        "font:0px serif",
        "font:100 0px serif",
        "color:rgba(255,0,0,0)",
        "color:#1230",
        "color:rgb(0 0 0 / 0)",
        "position:absolute;left:100000px",
        "position:fixed;top:100vh",
        "position:fixed;left:100vw",
        "position:fixed;top:0vh;left:12px",
    ];
    let invalid = [
        "not a declaration",
        "position:fixed;top:1vh",
        "left:12em",
        "clip-path:inset(100000px)",
        "margin-left:100vw",
        "padding-left:100vw",
        "margin:-100vmin 0",
        "display:var(--private)",
        "color:red;broken",
        "color:red;/*",
        "display:none;{",
        "opacity:calc(1 - 1)",
        "font-size:0px!important!important",
    ];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for css in hiding.iter().chain(invalid.iter()) {
            let body = format!(
                "<h1>Visible<span style='{css}'>CSS_PRIVATE_VALUE</span></h1><table><tr id=fictional-row><td>5-1 Visible<span style='{css}'>CSS_PRIVATE_VALUE</span></td><td>12</td></tr></table>"
            );
            for family in ["header", "inventory", "definitions"] {
                let body = if family == "inventory" {
                    daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
                } else {
                    body.clone()
                };
                let result = fix1_page(scope, family, &body, &[]);
                // FIX-3: cross-box effects cannot be isolated to this span;
                // refusal also protects text outside the offending subtree.
                let cross_box = css.starts_with("position:")
                    || css.starts_with("left:")
                    || css.starts_with("margin")
                    || css.starts_with("text-indent:")
                    || css.starts_with("filter:")
                    || css.starts_with("-webkit-filter:")
                    || css == &"d\\69splay:n/**/one"
                    || css.starts_with("display:var(")
                    || css == &"visibility:collapse";
                if cross_box {
                    assert_code(result, "contract_drift");
                } else {
                    let output = result.unwrap();
                    assert!(!json(&output).contains("CSS_PRIVATE_VALUE"), "{family}");
                    assert!(json(&output).contains("Visible"));
                }
                let blocked = format!("<style>h1{{{css}}}</style>{body}");
                assert_code(fix1_page(scope, family, &blocked, &[]), "contract_drift");
            }
        }
        for css in [
            "font-size:12px;color:#123456",
            "position:static;padding:0vh 12px",
            "margin:0;padding:2px",
            "clip:auto;clip-path:none",
            "font-family:sans-serif; border:1px solid black",
            "display:block;visibility:visible;opacity:1",
            "width:100%;height:auto",
            "/*safe*/ color: red !important",
        ] {
            let body = format!(
                "<style>table{{{css}}}</style><h1 style='{css}'>SAFE_STYLE_TEXT</h1><table><tr id=fictional-row><td style='{css}'>5-1 SAFE_STYLE_TEXT</td><td>12</td></tr></table>"
            );
            for family in ["header", "inventory", "definitions"] {
                let body = if family == "inventory" {
                    daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
                } else {
                    body.clone()
                };
                let output = fix1_page(scope, family, &body, &[]).unwrap();
                assert!(json(&output).contains("SAFE_STYLE_TEXT"));
                assert!(!json(&output).contains(css));
            }
        }
    }
}
#[test]
fn fix1_textarea_values_opaque_at_every_text_position_and_copy() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for value in ["TEXTAREA_PRIVATE_VALUE", "&#84;EXTAREA_PRIVATE_VALUE"] {
            for payload in [
                format!("<h1>Visible<textarea name=private>{value}</textarea></h1>"),
                format!("<textarea name=private><h1>{value}</h1></textarea>"),
                format!("<textarea name=private>{value}</textarea>"),
            ] {
                let body = format!(
                    "<h1>Visible</h1><table><tr id=fictional-row><th>시험지 제목</th><td>5-1 Visible{payload}</td></tr></table>"
                );
                for family in ["header", "inventory", "definitions", "batch"] {
                    let body = if family == "inventory" {
                        daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
                    } else {
                        body.clone()
                    };
                    let output = fix1_page(scope, family, &body, &[]).unwrap();
                    assert!(!json(&output).contains("TEXTAREA_PRIVATE_VALUE"));
                    assert!(json(&output).contains("Visible"));
                }
            }
        }
    }
}
#[test]
fn fix1_converter_semantic_names_are_exact_and_every_other_name_is_opaque() {
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let program = converter_program()
            + "function REVIEW_CANARY_key_32(n){return n;}function codeToKey_REVIEW_CANARY(n){return n;}const x={REVIEW_CANARY_key32:function(n){return n;}};const candidate='0123456789ABCDEFGHIJKLMNOPQRSTUV';";
        let (mut src, count) = fix1_source(
            scope,
            vec![("paper.converter", program.clone())],
            &[],
            &catalog(),
            &pool(),
            &program,
            &reader_shell(scope),
        );
        let mut q = input(scope);
        q.set("kind", "printed_code".into());
        let output = call(&mut src, "academy_source_read_contracts", &q).unwrap();
        assert_eq!(count.get(), 1);
        assert!(!json(&output).contains("REVIEW_CANARY"));
        assert!(!json(&output).contains("0123456789ABCDEFGHIJKLMNOPQRSTUV"));
        assert!(json(&output).contains("codeToKey"));
        assert!(json(&output).contains("keyToCode"));
        records.push(obj(&[
            ("scope", scope.as_str().into()),
            ("accepted", true.into()),
            ("disclosure", false.into()),
        ]));
    }
    fix1_evidence("converter-canary", &records);
}

#[test]
fn fix1_canary_matrix_8484_attributes_and_hidden_values() {
    let attrs = [
        "id",
        "class",
        "title",
        "lang",
        "dir",
        "hidden",
        "inert",
        "aria-label",
        "aria-hidden",
        "data-secret",
        "style",
        "color",
        "bgcolor",
        "background",
        "width",
        "height",
        "align",
        "valign",
        "border",
        "cellpadding",
        "cellspacing",
        "nowrap",
        "name",
        "value",
        "href",
        "src",
        "srcdoc",
        "onclick",
        "onload",
        "action",
        "method",
        "form",
        "type",
        "placeholder",
        "label",
        "selected",
        "checked",
        "disabled",
        "readonly",
        "contenteditable",
        "slot",
        "part",
        "popover",
        "data-x",
        "xmlns",
    ];
    let tags = [
        "h1", "div", "span", "font", "table", "tr", "td", "th", "a", "form", "select", "option",
        "textarea", "button", "details", "dialog", "svg", "math", "template", "iframe", "input",
        "link", "script", "img", "object", "embed", "audio", "video", "canvas", "noscript",
        "title",
    ];
    let mut records = vec![];
    let mut admitted = 0;
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for tag in tags {
            for attr in attrs {
                let canary = format!("FIX1_PRIVATE_{}_{}", tag, attr.replace('-', "_"));
                let element = format!("<{tag} {attr}='{canary}'>SAFE_VISIBLE</{tag}>");
                let payload = match tag {
                    "td" | "th" => format!("<table><tr>{element}</tr></table>"),
                    "tr" => {
                        format!("<table><tr {attr}='{canary}'><td>SAFE_VISIBLE</td></tr></table>")
                    }
                    _ => element,
                };
                for family in ["header", "inventory", "definitions"] {
                    let body = format!(
                        "<h1>VISIBLE_CONTROL</h1><table><tr><th>시험지 제목</th><td>5-1 VISIBLE_CONTROL {payload}</td></tr></table>"
                    );
                    let body = if family == "inventory" {
                        daily(scope, vec![]) + &body
                    } else {
                        body
                    };
                    let output = fix1_page(scope, family, &body, &[]);
                    admitted += usize::from(output.is_ok());
                    assert!(
                        !fix1_bytes(&output).contains(&canary),
                        "{tag}/{attr}/{family}"
                    );
                    records.push(obj(&[
                        ("scope", scope.as_str().into()),
                        ("case", format!("{tag}/{attr}").into()),
                        ("family", family.into()),
                        ("accepted", output.is_ok().into()),
                        ("confirmedDisclosure", false.into()),
                    ]));
                }
            }
        }
        let values = [
            ("hidden-root", "<h1 hidden>FIX1_TEXT_VALUE</h1>"),
            (
                "hidden-ancestor",
                "<div hidden><h1>FIX1_TEXT_VALUE</h1></div>",
            ),
            (
                "aria-hidden",
                "<div aria-hidden=true><h1>FIX1_TEXT_VALUE</h1></div>",
            ),
            ("inert", "<div inert><h1>FIX1_TEXT_VALUE</h1></div>"),
            ("script", "<script>const a='FIX1_TEXT_VALUE';</script>"),
            ("style", "<style>/*FIX1_TEXT_VALUE*/</style>"),
            ("template", "<template><h1>FIX1_TEXT_VALUE</h1></template>"),
            ("comment", "<!--FIX1_TEXT_VALUE-->"),
            ("cdata", "<![CDATA[FIX1_TEXT_VALUE]]>"),
            ("svg-title", "<svg><title>FIX1_TEXT_VALUE</title></svg>"),
            (
                "svg-foreign",
                "<svg><foreignObject><h1>FIX1_TEXT_VALUE</h1></foreignObject></svg>",
            ),
            // HTML5 reparents this H1 into the ordinary visible HTML TD.
            (
                "math-repair-visible-control",
                "<math><annotation-xml encoding='text/html'><h1>FIX1_TEXT_VALUE</h1></annotation-xml></math>",
            ),
            (
                "encoded-hidden",
                "<h1 aria-hidden='tr&#117;e'>FIX1_TEXT_VALUE</h1>",
            ),
            // FIX-1(a) explicitly makes this presentation-only text exportable.
            (
                "inert-presentation-visible-control",
                "<h1 color=white>FIX1_TEXT_VALUE</h1>",
            ),
            (
                "textarea-value",
                "<h1><textarea name=private_note>FIX1_TEXT_VALUE</textarea></h1>",
            ),
            (
                "textarea-encoded",
                "<h1><textarea name=private_note>&#70;IX1_TEXT_VALUE</textarea></h1>",
            ),
            (
                "malformed-hidden",
                "<div hidden><table><h1>FIX1_TEXT_VALUE</table></div>",
            ),
            ("unknown-tag", "<unknown><h1>FIX1_TEXT_VALUE</h1></unknown>"),
            (
                "hidden-details",
                "<details><summary>FIX1_TEXT_VALUE</summary></details>",
            ),
        ];
        for (case, payload) in values {
            for family in ["header", "inventory", "definitions"] {
                let body = format!(
                    "<h1>VISIBLE_CONTROL</h1><table><tr id=fictional-row><th>시험지 제목</th><td>5-1 VISIBLE_CONTROL {payload}</td></tr></table>"
                );
                let body = if family == "inventory" {
                    daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
                } else {
                    body
                };
                let output = fix1_page(scope, family, &body, &[]);
                admitted += usize::from(output.is_ok());
                let visible_control = case.ends_with("visible-control");
                let present = fix1_bytes(&output).contains("FIX1_TEXT_VALUE");
                assert_eq!(present, visible_control, "{case}/{family}");
                records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("case", case.into()),
                    ("family", family.into()),
                    ("accepted", output.is_ok().into()),
                    ("visibleControl", visible_control.into()),
                    ("confirmedDisclosure", false.into()),
                ]));
            }
        }
    }
    assert_eq!(records.len(), 8484);
    assert!(
        admitted >= 8200,
        "matrix must actually admit ordinary pages"
    );
    fix1_evidence("canary-8484", &records);
}
#[test]
fn fix1_inert_canary_matrix_120_cases() {
    let attrs = [
        "id",
        "class",
        "title",
        "style",
        "color",
        "bgcolor",
        "background",
        "data-secret",
        "name",
        "value",
        "href",
        "src",
        "onclick",
        "onload",
        "action",
        "method",
        "type",
        "placeholder",
        "label",
        "form",
    ];
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for attr in attrs {
            let canary = format!("FIX1_INERT_{}", attr.replace('-', "_"));
            let payload = format!(
                "<div {attr}='{canary}'><form name='{canary}' action='/{canary}' method=post><input name='{canary}' value='{canary}'><textarea name='{canary}'>{canary}</textarea><select name='{canary}'><option value='{canary}'>{canary}</option></select><a href='/{canary}' onclick='{canary}()'>SAFE</a></form></div><script>function {canary}(){{return '{canary}';}}</script>"
            );
            for family in ["catalog", "pool", "shell"] {
                let shell = reader_shell(scope) + if family == "shell" { &payload } else { "" };
                let cat = catalog() + if family == "catalog" { &payload } else { "" };
                let pool_page = pool() + if family == "pool" { &payload } else { "" };
                let replies = if family == "catalog" {
                    vec![("paper.catalog", cat.clone())]
                } else {
                    vec![
                        ("metadata.paper.initial", shell.clone()),
                        ("paper.pool", pool_page.clone()),
                    ]
                };
                let (mut src, count) = fix1_source(
                    scope,
                    replies,
                    &[],
                    &cat,
                    &pool_page,
                    &converter_program(),
                    &shell,
                );
                let mut q = input(scope);
                let tool = if family == "catalog" {
                    q.set("kind", "daily_test_catalog".into());
                    "academy_source_read_contracts"
                } else {
                    "academy_paper_read_contracts"
                };
                let output = call(&mut src, tool, &q).unwrap();
                assert_eq!(count.get(), if family == "catalog" { 1 } else { 2 });
                assert!(!json(&output).contains(&canary));
                records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("case", attr.into()),
                    ("family", family.into()),
                    ("accepted", true.into()),
                    ("confirmedDisclosure", false.into()),
                ]));
            }
        }
    }
    assert_eq!(records.len(), 120);
    fix1_evidence("inert-canary-120", &records);
}
#[test]
fn fix1_copy_canary_matrix_8_cases() {
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let c = "FIX1_COPIED_PRIVATE";
        let mut record = row("77001", "Fictional One", "3", "81001");
        record.set("_id", c.into());
        record.set("privatePayload", c.into());
        let (mut src, count, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", remap(&paper_form(), scope)),
                (
                    "metadata.paper.search.1",
                    remap(&(principal() + &result(vec![record.clone()])), scope),
                ),
            ],
        );
        let printed = call(
            &mut src,
            "academy_printed_paper_lookup",
            &printed_input(scope),
        )
        .unwrap();
        assert_eq!(count.get(), 2);
        assert!(!json(&printed).contains(c));
        let page = daily(scope, vec![record])
            + &format!(
                "<table><tr id='{c}'><td>SAFE_DISPLAY</td><td><span hidden>{c}</span></td></tr></table>"
            );
        let (mut src, count, _) = source(
            scope,
            vec![("metadata.daily", page.clone()), ("metadata.daily", page)],
        );
        let mut q = input(scope);
        q.set("periods", strings(&["11"]));
        let history = call(&mut src, "academy_daily_test_inventory", &q).unwrap();
        assert_eq!(count.get(), 2);
        assert!(!json(&history).contains(c));
        let (mut src, count, _) = source(
            scope,
            vec![
                ("metadata.paper.initial", reader_shell(scope)),
                (
                    "paper.definition",
                    definition_xml().replace("HIDDEN_FILE_LOCATION", c),
                ),
            ],
        );
        let definition = call(
            &mut src,
            "academy_paper_definition_lookup",
            &paper_input(scope),
        )
        .unwrap();
        assert_eq!(count.get(), 2);
        assert!(!json(&definition).contains(c));
        let page = format!(
            "<h1>SAFE_HEADING</h1><table><tr><th>시험지 제목</th><td><textarea name=private_note>{c}</textarea></td></tr></table>"
        );
        let batch = fix1_page(scope, "batch", &page, &[]).unwrap();
        assert!(!json(&batch).contains(c));
        for family in [
            "printed",
            "daily-history",
            "definition-xml",
            "header-batch-textarea",
        ] {
            records.push(obj(&[
                ("scope", scope.as_str().into()),
                ("family", family.into()),
                ("accepted", true.into()),
                ("confirmedDisclosure", false.into()),
            ]));
        }
    }
    assert_eq!(records.len(), 8);
    fix1_evidence("copy-canary-8", &records);
}
#[test]
fn fix1_native_history_accepts_harmless_styles_and_preserves_child_hash() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for presentation in [
            "<style>h1{font-weight:bold}</style>",
            "<link rel=stylesheet href='/fictional.css'>",
        ] {
            let child = presentation.to_owned() + &content_header();
            let (src, count) = fix1_source(
                scope,
                vec![
                    ("metadata.paper.initial", reader_shell(scope)),
                    ("paper.header", header("3")),
                    ("paper.content", child.clone()),
                ],
                &["/fictional.css"],
                &catalog(),
                &pool(),
                &converter_program(),
                &reader_shell(scope),
            );
            let id = input(scope).get("studentId").clone();
            let name: Js = if scope == PaperScope::Paper1 {
                "Fictional One"
            } else {
                "Fictional Two"
            }
            .into();
            let mut history = edu_academy_sources::history::History::new(
                NativeHistoryLoan {
                    source: src,
                    fail: false,
                },
                obj(&[("teacherId", admission(scope).teacher_id.into())]),
                policy_value(scope),
                vec![obj(&[
                    ("studentId", id.clone()),
                    ("displayName", name.clone()),
                ])],
                vec![name],
                vec![id.clone()],
            )
            .unwrap();
            let output = history
                .headers(&obj(&[("studentId", id), ("paperCodes", strings(&["#5"]))]))
                .unwrap();
            assert_eq!(count.get(), 3);
            assert_eq!(
                output.get("observation").get("rows").array()[0]
                    .get("contentHeader")
                    .get("sourceSha256"),
                &hash(child.as_bytes()).into()
            );
            assert!(!json(&output).contains("NEVER_EXPORT_QUESTION"));
            assert!(json(&output).contains("Fictional title"));
        }
    }
}

#[test]
fn fix1_stylesheet_owner_publication_is_bounded_canonical_and_not_response_learned() {
    fn pin_value(urls: Js) -> Js {
        obj(&[
            ("schemaVersion", "academy.paper-pins/1".into()),
            ("readerContext", hash(b"shell").into()),
            ("ready", hash(b"ready").into()),
            ("view", hash(b"view").into()),
            ("catalog", hash(b"catalog").into()),
            ("pool", hash(b"pool").into()),
            ("converter", hash(b"converter").into()),
            ("stylesheetUrls", urls),
        ])
    }
    for invalid in [
        Js::Null,
        "url".into(),
        Js::Arr(vec![Js::Null]),
        strings(&["/fictional.css", "/assets/../fictional.css#same"]),
        strings(&["javascript:opaque"]),
        strings(&["https://user:pass@fictional.invalid/style.css"]),
        strings(&[&"x".repeat(4097)]),
        Js::Arr(vec!["/fictional.css".into(); 65]),
    ] {
        assert!(PaperPins::parse(&pin_value(invalid)).is_err());
    }
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for href in [
            "/fictional.css",
            "/assets/../fictional.css#page",
            &format!("{}/fictional.css#page", http::ORIGIN),
        ] {
            let body =
                format!("<link rel='alternate StyleSheet' href='{href}'><h1>VISIBLE_PINNED</h1>");
            assert!(
                json(&fix1_page(scope, "header", &body, &["/fictional.css"]).unwrap())
                    .contains("VISIBLE_PINNED")
            );
            assert_code(fix1_page(scope, "header", &body, &[]), "contract_drift");
        }
        for body in [
            "<link rel=stylesheet><h1>Visible</h1>",
            "<link rel=stylesheet href=fictional.css><h1>Visible</h1>",
            "<link rel=stylesheet href='/other.css'><h1>Visible</h1>",
            "<base href='https://fictional.invalid/'><link rel=stylesheet href='/fictional.css'><h1>Visible</h1>",
        ] {
            assert_code(
                fix1_page(
                    scope,
                    "definitions",
                    &format!("{body}<table><tr><td>5-1 Visible</td><td>12</td></tr></table>"),
                    &["/fictional.css"],
                ),
                "contract_drift",
            );
        }
    }
}
#[test]
fn fix1_presentation_and_hidden_style_apply_to_roots_ancestors_and_descendants() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for attribute in [
            "border",
            "cellpadding",
            "cellspacing",
            "width",
            "height",
            "align",
            "valign",
            "bgcolor",
            "color",
            "face",
            "size",
            "lang",
            "dir",
            "class",
            "id",
            "nowrap",
            "background",
        ] {
            let body = format!(
                "<div {attribute}=ATTR_PRIVATE><h1 {attribute}=ATTR_PRIVATE>Visible</h1><table {attribute}=ATTR_PRIVATE><tr id=fictional-row><td {attribute}=ATTR_PRIVATE>5-1 Visible<span {attribute}=ATTR_PRIVATE> Content</span></td><td>12</td></tr></table></div>"
            );
            for family in ["header", "inventory", "definitions"] {
                let body = if family == "inventory" {
                    daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
                } else {
                    body.clone()
                };
                let bytes = json(&fix1_page(scope, family, &body, &[]).unwrap());
                assert!(bytes.contains("Visible"));
                assert!(!bytes.contains("ATTR_PRIVATE"));
            }
        }
        for body in [
            "<h1 style='display:none'>HIDDEN_CSS_VALUE</h1><h1>Visible</h1>",
            "<div style='opacity:0'><h1>HIDDEN_CSS_VALUE</h1></div><h1>Visible</h1>",
            "<h1>Visible<span style='visibility:hidden'>HIDDEN_CSS_VALUE</span></h1>",
        ] {
            let output = fix1_page(scope, "header", body, &[]).unwrap();
            assert!(!json(&output).contains("HIDDEN_CSS_VALUE"));
            assert!(json(&output).contains("Visible"));
        }
    }
}

fn fix2_body(scope: PaperScope, family: &str, payload: &str) -> String {
    let body = format!(
        "<h1>VISIBLE_CONTROL{payload}</h1><table><tr id=fictional-row><th>시험지 제목</th><td>5-1 VISIBLE_CONTROL{payload}</td><td>12</td></tr></table>"
    );
    if family == "inventory" {
        daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
    } else {
        body
    }
}
#[test]
fn fix2_exact_reviewer_32_crlf_and_48_border_cases() {
    let mut crlf_records = vec![];
    for encoded in [
        "display:\\6e&#13;&#10;one",
        "display:\\6e&#xD;&#xA;one",
        "display:n\\6f&#13;&#10;ne",
        "display:no\\6e&#13;&#10;e",
    ] {
        for scope in [PaperScope::Paper1, PaperScope::Paper2] {
            for family in ["header", "inventory", "definitions", "batch"] {
                let payload = format!("<span style=\"{encoded}\">FIX2_CRLF_CANARY</span>");
                let output =
                    fix1_page(scope, family, &fix2_body(scope, family, &payload), &[]).unwrap();
                assert!(!json(&output).contains("FIX2_CRLF_CANARY"));
                assert!(json(&output).contains("VISIBLE_CONTROL"));
                crlf_records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("family", family.into()),
                    ("case", encoded.into()),
                    ("disclosure", false.into()),
                ]));
            }
        }
    }
    assert_eq!(crlf_records.len(), 32);
    fix1_evidence("fix2-reviewer-crlf-32", &crlf_records);
    let mut border_records = vec![];
    for css in [
        "border-top:10000px solid transparent",
        "border:10000px solid transparent",
        "border-width:10000px;border-style:solid",
    ] {
        for scope in [PaperScope::Paper1, PaperScope::Paper2] {
            for family in ["header", "inventory", "definitions", "batch"] {
                for mode in ["inline", "block"] {
                    if mode == "inline" {
                        let payload = format!("<div style='{css}'>FIX2_BORDER_CANARY</div>");
                        let output =
                            fix1_page(scope, family, &fix2_body(scope, family, &payload), &[])
                                .unwrap();
                        assert!(!json(&output).contains("FIX2_BORDER_CANARY"));
                        assert!(json(&output).contains("VISIBLE_CONTROL"));
                    } else {
                        let body = format!("<style>div{{{css}}}</style>")
                            + &fix2_body(scope, family, "<div>FIX2_BORDER_CANARY</div>");
                        assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
                    }
                    border_records.push(obj(&[
                        ("scope", scope.as_str().into()),
                        ("family", family.into()),
                        ("mode", mode.into()),
                        ("css", css.into()),
                        ("disclosure", false.into()),
                    ]));
                }
            }
        }
    }
    assert_eq!(border_records.len(), 48);
    fix1_evidence("fix2-reviewer-border-48", &border_records);
}
#[test]
fn fix2_css_encodings_roots_ancestors_descendants_and_title() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for css in [
            "display:\\6e&#10;one",
            "display:\\6e&#13;one",
            "display:\\6e&#12;one",
            "display:\\6e\r\none",
            "display:\\6e\rone",
            "display:\\6e\u{c}one",
            "d\\69&#13;&#10;splay:none",
            "border:1e4px solid transparent",
            "border:10000\\70&#13;&#10;x solid black",
            "line-height:10000px",
            "letter-spacing:10000px",
            "word-spacing:10000px",
            "width:10000px",
            "outline:10000px solid black",
            "font-size:10000px",
            "position:unqualified",
        ] {
            for family in ["header", "inventory", "definitions", "batch"] {
                for payload in [
                    format!("<div style=\"{css}\"><h1>FIX2_PRIVATE</h1></div>"),
                    format!("<h1 style=\"{css}\">FIX2_PRIVATE</h1>"),
                    format!("<span style=\"{css}\">FIX2_PRIVATE</span>"),
                ] {
                    let result = fix1_page(scope, family, &fix2_body(scope, family, &payload), &[]);
                    // Inert structural inspection has an earlier control-byte
                    // guard. FF is also tested directly at the CSS boundary;
                    // this independent guard must stay a sanitized refusal.
                    if result.is_err() && (css.contains('\u{c}') || css.contains("&#12;")) {
                        assert_code(result, "paper_output_budget_exceeded");
                    } else if matches!(css, "outline:10000px solid black" | "position:unqualified")
                    {
                        assert_code(result, "contract_drift");
                    } else {
                        let output = result.unwrap();
                        assert!(!json(&output).contains("FIX2_PRIVATE"));
                        assert!(json(&output).contains("VISIBLE_CONTROL"));
                    }
                }
            }
            let title =
                format!("<title style=\"{css}\">FIX2_TITLE_PRIVATE</title><h1>Visible</h1>");
            assert_code(fix1_page(scope, "header", &title, &[]), "contract_drift");
        }
        for css in [
            "border-top:1px solid red",
            "border:0 solid transparent",
            "border-width:1px;border-style:solid",
            "font-size:12px",
            "padding:2px",
            "line-height:1.5",
            "letter-spacing:1px",
            "width:100%;height:auto",
            "color:r\\65&#13;&#10;d!important",
        ] {
            let payload = format!("<div style=\"{css}\">FIX2_VISIBLE</div>");
            for family in ["header", "inventory", "definitions", "batch"] {
                let body = format!(
                    "<style>div{{{}}}</style>",
                    css.replace("&#13;&#10;", "\r\n")
                ) + &fix2_body(scope, family, &payload);
                assert!(
                    json(&fix1_page(scope, family, &body, &[]).unwrap()).contains("FIX2_VISIBLE")
                );
            }
        }
    }
}

#[test]
fn fix2_geometry_composition_obeys_disclosure_model_v2() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for (css, depth) in [
                ("border-top:128px solid transparent", 16),
                ("border-top:128px solid transparent;padding-top:128px", 40),
                ("left:128px;position:relative", 16),
            ] {
                let payload = format!(
                    "{}FIX2_COMPOSED_PRIVATE{}",
                    format!("<div style='{css}'>").repeat(depth),
                    "</div>".repeat(depth)
                );
                let result = fix1_page(scope, family, &fix2_body(scope, family, &payload), &[]);
                // DECISION-DISCLOSURE-2 supersedes the old cumulative sum:
                // only the positioned case is hidden; in-flow nested borders
                // and padding remain visible by ordinary document scrolling.
                let visible = !css.contains("position");
                if visible {
                    let output = result.unwrap();
                    assert!(json(&output).contains("FIX2_COMPOSED_PRIVATE"));
                    assert!(json(&output).contains("VISIBLE_CONTROL"));
                } else {
                    assert_code(result, "contract_drift");
                }
                let payload = format!(
                    "{}FIX2_COMPOSED_PRIVATE{}",
                    "<div>".repeat(depth),
                    "</div>".repeat(depth)
                );
                let body =
                    format!("<style>div{{{css}}}</style>") + &fix2_body(scope, family, &payload);
                if visible {
                    assert!(
                        json(&fix1_page(scope, family, &body, &[]).unwrap())
                            .contains("FIX2_COMPOSED_PRIVATE")
                    );
                } else {
                    assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
                }
            }
            let payload = "<div style='border-top:1px solid black'><div style='padding-top:2px'>FIX2_COMPOSED_VISIBLE</div></div>";
            let body = "<style>div{margin-top:1px}</style>".to_owned()
                + &fix2_body(scope, family, payload);
            assert!(
                json(&fix1_page(scope, family, &body, &[]).unwrap())
                    .contains("FIX2_COMPOSED_VISIBLE")
            );
        }
    }
}

#[test]
fn fix2_security_foreground_and_background_combinations_are_unqualified() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for css in [
                "color:white;background:white",
                "color:red;background-color:red",
                "color:#fff",
                "color:#ffffff",
                "color:#fefefe",
                "color:yellow",
                "opacity:0.1",
                "opacity:1e-300",
                "background:black;color:black",
            ] {
                let payload = format!("<div style='{css}'>FIX2_CONTRAST_PRIVATE</div>");
                let output =
                    fix1_page(scope, family, &fix2_body(scope, family, &payload), &[]).unwrap();
                assert!(!json(&output).contains("FIX2_CONTRAST_PRIVATE"));
                assert!(json(&output).contains("VISIBLE_CONTROL"));
                let body = format!("<style>div{{{css}}}</style>")
                    + &fix2_body(scope, family, "<div>FIX2_CONTRAST_PRIVATE</div>");
                assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
            }
            for css in ["color:red", "color:black", "color:#123456"] {
                let payload = format!("<div style='{css}'>FIX2_CONTRAST_VISIBLE</div>");
                let body =
                    format!("<style>div{{{css}}}</style>") + &fix2_body(scope, family, &payload);
                assert!(
                    json(&fix1_page(scope, family, &body, &[]).unwrap())
                        .contains("FIX2_CONTRAST_VISIBLE")
                );
            }
        }
    }
}
#[test]
fn fix2_security_stylesheet_work_is_bounded_across_the_document() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            let normal = fix2_body(scope, family, "STYLE_BOUND_VISIBLE");
            for (prefix, admitted) in [
                ("<style>div{font-weight:bold}</style>".repeat(64), true),
                ("<style>div{font-weight:bold}</style>".repeat(65), false),
                (
                    format!(
                        "<style>{}</style><style>{}</style>",
                        "div{}".repeat(512),
                        "div{}".repeat(512)
                    ),
                    true,
                ),
                (
                    format!(
                        "<style>{}</style><style>{}</style>",
                        "div{}".repeat(512),
                        "div{}".repeat(513)
                    ),
                    false,
                ),
                (format!("<style>/*{}*/</style>", "a".repeat(32764)), true),
                (
                    format!(
                        "<style>/*{}*/</style><style>/**/</style>",
                        "a".repeat(32764)
                    ),
                    false,
                ),
            ] {
                let output = fix1_page(scope, family, &(prefix + &normal), &[]);
                if admitted {
                    assert!(json(&output.unwrap()).contains("STYLE_BOUND_VISIBLE"));
                } else {
                    assert_code(output, "contract_drift");
                }
            }
        }
    }
}

#[test]
fn fix2_security_raw_heading_occurrences_and_projection_work_are_bounded() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "batch"] {
            for (payload, admitted) in [
                (
                    "<div class=test-title>OCCURRENCE_VISIBLE</div>".repeat(64),
                    true,
                ),
                (
                    "<div class=test-title>OCCURRENCE_VISIBLE</div>".repeat(65),
                    false,
                ),
                ("<div class=test-title></div>".repeat(65), false),
                (
                    "<div class=test-title hidden>OPAQUE</div>".repeat(65),
                    false,
                ),
                (
                    format!(
                        "{}X{}",
                        "<div class=test-title>".repeat(65),
                        "</div>".repeat(65)
                    ),
                    false,
                ),
            ] {
                let output = fix1_page(scope, family, &payload, &[]);
                if admitted {
                    assert!(json(&output.unwrap()).contains("OCCURRENCE_VISIBLE"));
                } else {
                    assert_code(output, "printed_paper_view_header_limit");
                }
            }
        }
    }
    // The reported shape stays below the original byte/element caps. It must
    // fail at the shared work preflight before any overlapping text walk.
    let payload = format!(
        "{}X{}",
        "<div class=test-title border=1 cellpadding=2 cellspacing=0 width=100%>".repeat(19_990),
        "</div>".repeat(19_990)
    );
    let scope = PaperScope::Paper1;
    let (mut src, count, _) = source(
        scope,
        vec![
            ("metadata.paper.initial", reader_shell(scope)),
            ("paper.header", payload),
        ],
    );
    let mut q = paper_input(scope);
    q.set("view", "header".into());
    assert_code(
        call(&mut src, "academy_paper_definition_lookup", &q),
        "paper_document_limit",
    );
    assert_eq!(count.get(), 2);
    assert_code(
        call(&mut src, "academy_daily_test_inventory", &input(scope)),
        "source_failure_latched_no_retry",
    );
    assert_eq!(count.get(), 2);
}

// FIX-3 / DECISION-DISCLOSURE-2. These reproduce the reviewer's exact
// invented v4 bodies; below-fold normal flow is now explicitly visible.
fn fix3_body(scope: PaperScope, family: &str, payload: &str) -> String {
    let body = format!(
        "<h1>V3_VISIBLE{payload}</h1><table><tr id=fictional-row><th>시험지 제목</th><td>5-1 V3_VISIBLE{payload}</td><td>12 V3_ITEMS</td></tr></table>"
    );
    if family == "inventory" {
        daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
    } else {
        body
    }
}
#[test]
fn fix3_exact_32_ordinary_styled_pages_preserve_selected_fields_and_rows() {
    let mut records = vec![];
    let mut core_values = 0;
    let mut core_bytes = 0;
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for mode in ["plain", "inline-padding", "style-padding", "style-font"] {
                let css = if mode == "style-font" {
                    "font-size:12px"
                } else {
                    "padding:2px"
                };
                let attr = if mode == "inline-padding" {
                    " style='padding:2px'"
                } else {
                    ""
                };
                let prefix = if mode.starts_with("style-") {
                    format!("<style>td{{{css}}}</style>")
                } else {
                    String::new()
                };
                let rows = (0..40).map(|i| format!("<tr><td{attr}>5-1 ORDINARY_ROW_{i}</td><td{attr}>12 ORDINARY_ITEMS_{i}</td></tr>")).collect::<String>();
                let body = prefix + &fix3_body(scope, family, "") + "<table>" + &rows + "</table>";
                let result = fix1_page(scope, family, &body, &[]);
                let bytes = json(&result.unwrap());
                for marker in ["V3_VISIBLE", "V3_ITEMS"] {
                    assert!(bytes.contains(marker), "{family}/{mode}");
                    core_values += 1;
                    core_bytes += marker.len();
                }
                let mut row_values = 0;
                if family == "definitions" {
                    for i in 0..40 {
                        for marker in [
                            format!("5-1 ORDINARY_ROW_{i}"),
                            format!("12 ORDINARY_ITEMS_{i}"),
                        ] {
                            assert!(bytes.contains(&marker), "{mode}/{marker}");
                            row_values += 1;
                        }
                    }
                    assert_eq!(row_values, 80);
                }
                records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("family", family.into()),
                    ("variant", mode.into()),
                    ("coreValues", 2usize.into()),
                    ("coreBytes", 18usize.into()),
                    ("rowValues", row_values.into()),
                ]));
            }
        }
    }
    assert_eq!(records.len(), 32);
    assert_eq!((core_values, core_bytes), (64, 576));
    fix1_evidence("fix3-ordinary-32", &records);
}
#[test]
fn fix3_exact_16_overlays_do_not_export_covered_text() {
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for mode in ["inline", "block"] {
                let css = "position:fixed;top:0;left:0;width:100%;border-top:128px solid black";
                let overlay = if mode == "inline" {
                    format!("<div style='{css}'></div>")
                } else {
                    format!("<style>div{{{css}}}</style><div id=cover></div>")
                };
                let body = fix3_body(scope, family, "<span>V4_COVERED_CANARY</span>") + &overlay;
                let result = fix1_page(scope, family, &body, &[]);
                match result {
                    Ok(output) => assert!(
                        !json(&output).contains("V4_COVERED_CANARY"),
                        "{family}/{mode}"
                    ),
                    Err(error) => assert_eq!(error.message, "contract_drift"),
                }
                records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("family", family.into()),
                    ("mode", mode.into()),
                    ("confirmedDisclosure", false.into()),
                ]));
            }
        }
    }
    assert_eq!(records.len(), 16);
    fix1_evidence("fix3-overlay-16", &records);
}
#[test]
fn fix3_exact_24_sibling_spacers_remain_visible_by_scrolling() {
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for css in [
                "height:128px",
                "padding-top:128px",
                "border-top:128px solid transparent",
            ] {
                let payload = format!(
                    "{}<span>V4_SIBLING_CANARY</span>",
                    format!("<div style='{css}'></div>").repeat(20)
                );
                let body = fix3_body(scope, family, &payload);
                let output = fix1_page(scope, family, &body, &[]).unwrap();
                assert!(json(&output).contains("V4_SIBLING_CANARY"));
                assert!(json(&output).contains("V3_VISIBLE"));
                records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("family", family.into()),
                    ("css", css.into()),
                    ("expectedVisible", true.into()),
                ]));
            }
        }
    }
    assert_eq!(records.len(), 24);
    fix1_evidence("fix3-spacer-24", &records);
}

#[test]
fn fix3_cross_box_hazards_are_rejected_even_in_empty_unselected_siblings() {
    let hazards = [
        "position:relative",
        "position:absolute",
        "position:fixed",
        "position:sticky",
        "top:0",
        "right:0",
        "bottom:0",
        "left:0",
        "inset:0",
        "inset-inline-start:0",
        "z-index:0",
        "float:left",
        "margin:-1px",
        "margin-left:-1px",
        "text-indent:-1px",
        "text-indent:129px",
        "transform:translateX(1px)",
        "translate:1px",
        "rotate:1deg",
        "scale:2",
        "outline:1px solid black",
        "outline-width:1px",
        "outline-offset:1px",
        "outline-style:solid",
        "box-shadow:0 0 1px black",
        "text-shadow:0 0 1px black",
        "filter:blur(1px)",
        "backdrop-filter:blur(1px)",
        "mix-blend-mode:multiply",
        "-webkit-transform:translateX(1px)",
        "animation:cover 1s",
        "all:initial",
        // Same lexer rules apply even if an earlier/later value is unsupported.
        "width:calc(1px);position:fixed",
        "position:fixed;width:var(--x)",
        "/*;*/p\\6f&#13;&#10;sition/**/:/**/fixed!important",
        "color:'inert;';position:fixed",
        "position:fixed;/*",
    ];
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for css in hazards {
                let body = fix3_body(scope, family, "FLOW_CANARY")
                    + &format!("<div style=\"{css}\"></div>");
                assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
                let body = format!("<style>div{{{css}}}</style>")
                    + &fix3_body(scope, family, "FLOW_CANARY");
                assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
                records.push(obj(&[
                    ("scope", scope.as_str().into()),
                    ("family", family.into()),
                    ("css", css.into()),
                    ("confirmedDisclosure", false.into()),
                ]));
            }
            for css in [
                "position:static",
                "padding:128px 128px 128px 128px;border:128px solid black",
                "margin:128px;text-indent:128px",
                "outline:none;outline-width:0;outline-offset:0px;outline-style:none",
                "box-shadow:none;text-shadow:none;transform:none;translate:none;rotate:none;scale:none",
                "clip:auto;clip-path:none;mask:none;mask-image:none;filter:none;backdrop-filter:none;mix-blend-mode:normal",
                "overflow:visible;overflow-x:visible;overflow-y:visible;height:128px;max-height:128px",
            ] {
                let body = format!("<style>div{{{css}}}</style>")
                    + &fix3_body(
                        scope,
                        family,
                        &format!("<div style='{css}'>FLOW_VISIBLE</div>"),
                    );
                assert!(
                    json(&fix1_page(scope, family, &body, &[]).unwrap()).contains("FLOW_VISIBLE")
                );
            }
            // Self-hidden CSS still omits only its own subtree.
            for css in [
                "display:none",
                "opacity:0",
                "height:1px;overflow:hidden",
                "height:128px;max-height:128px;overflow-y:hidden",
                "clip-path:inset(50%)",
                "mask-image:linear-gradient(transparent,transparent)",
            ] {
                let body = fix3_body(
                    scope,
                    family,
                    &format!("<span style='{css}'>SELF_HIDDEN</span>"),
                );
                let output = json(&fix1_page(scope, family, &body, &[]).unwrap());
                assert!(output.contains("V3_VISIBLE"));
                assert!(!output.contains("SELF_HIDDEN"));
            }
        }
    }
    assert_eq!(records.len(), hazards.len() * 8);
    fix1_evidence("fix3-flow-hazards", &records);
}

#[test]
fn fix3_column_and_column_group_rejections_protect_non_descendant_cells() {
    let mut records = vec![];
    // Exact connector trigger: the collapsed first column is the canary cell.
    let exact = "<table><colgroup><col style=\"visibility:collapse\"></colgroup><tr><td>5-1 COLUMN_PRIVATE</td><td>12</td></tr></table>";
    let result = fix1_page(PaperScope::Paper1, "definitions", exact, &[]);
    let exported = result
        .as_ref()
        .is_ok_and(|v| json(v).contains("COLUMN_PRIVATE"));
    fix1_evidence(
        "fix3-column-exact",
        &[obj(&[
            ("accepted", result.is_ok().into()),
            ("exported", exported.into()),
        ])],
    );
    assert!(
        !exported,
        "column-hidden text exported outside the col subtree"
    );
    assert_code(result, "contract_drift");
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for tag in ["col", "colgroup"] {
                for css in [
                    "visibility:collapse",
                    "VISIBILITY:COLLAPSE!important",
                    "visibility:coll\\61&#13;&#10;pse",
                    "visibility:inherit",
                    "display:none",
                ] {
                    let cols = if tag == "col" {
                        format!("<colgroup><col style='{css}'></colgroup>")
                    } else {
                        format!("<colgroup style='{css}'><col></colgroup>")
                    };
                    let body = format!(
                        "<h1>VISIBLE_CONTROL</h1><table>{cols}<tr id=fictional-row><td>5-1 COLUMN_PRIVATE</td><td>12</td></tr></table>"
                    );
                    let body = if family == "inventory" {
                        daily(scope, vec![row("77001", "Fictional One", "3", "81001")]) + &body
                    } else {
                        body
                    };
                    assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
                    records.push(obj(&[
                        ("scope", scope.as_str().into()),
                        ("family", family.into()),
                        ("tag", tag.into()),
                        ("css", css.into()),
                        ("confirmedDisclosure", false.into()),
                    ]));
                }
                let cols = if tag == "col" {
                    "<colgroup><col style='visibility:visible'></colgroup>"
                } else {
                    "<colgroup style='visibility:visible'><col></colgroup>"
                };
                let body = fix3_body(scope, family, "COLUMN_VISIBLE")
                    .replace("<table>", &format!("<table>{cols}"));
                assert!(
                    json(&fix1_page(scope, family, &body, &[]).unwrap()).contains("COLUMN_VISIBLE")
                );
            }
        }
    }
    assert_eq!(records.len(), 80);
    fix1_evidence("fix3-column-80", &records);
}

#[test]
fn fix3_css_roles_are_qualified_independently_of_native_tag_names() {
    let exact = "<table><tbody style='display:table-column;visibility:collapse'></tbody><tbody><tr><td>5-1 ROLE_PRIVATE</td><td>12</td></tr></tbody></table>";
    let result = fix1_page(PaperScope::Paper1, "definitions", exact, &[]);
    let exported = result
        .as_ref()
        .is_ok_and(|v| json(v).contains("ROLE_PRIVATE"));
    fix1_evidence(
        "fix3-role-exact",
        &[obj(&[
            ("accepted", result.is_ok().into()),
            ("exported", exported.into()),
        ])],
    );
    assert!(!exported, "CSS-created column hides sibling cells");
    assert_code(result, "contract_drift");
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for tag in ["tbody", "thead", "tfoot", "tr", "div", "span"] {
                for css in [
                    "display:table-column;visibility:collapse",
                    "display:table-column-group;visibility:collapse",
                    "DISPLAY:TABLE-COLUMN!important;visibility:collapse",
                    "d\\69splay:table-col\\75&#13;&#10;mn;visibility:collapse",
                    "display:table-column;display:none",
                    "display:none;display:table-column",
                    "display:var(--role);visibility:collapse",
                    "display:inherit;visibility:collapse",
                    "display:n/**/one",
                    "display:contents",
                    "display:ruby",
                    "visibility:collapse",
                    "visibility:inherit",
                    "visibility:var(--visibility)",
                    "visibility:coll\\61&#13;&#10;pse",
                ] {
                    let role = format!("<{tag} style='{css}'></{tag}>");
                    let body = fix3_body(scope, family, "ROLE_PRIVATE")
                        .replace("<table>", &format!("<table>{role}"));
                    assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
                    records.push(obj(&[
                        ("scope", scope.as_str().into()),
                        ("family", family.into()),
                        ("tag", tag.into()),
                        ("css", css.into()),
                        ("confirmedDisclosure", false.into()),
                    ]));
                }
            }
            for css in [
                "visibility:hidden",
                "display:none",
                "display:n\\6f ne!important",
                "font-size:0px;display:block",
                "color:transparent;display:table-row",
            ] {
                let body = fix3_body(
                    scope,
                    family,
                    &format!("<span style='{css}'>LOCAL_ROLE_PRIVATE</span>"),
                );
                let output = json(&fix1_page(scope, family, &body, &[]).unwrap());
                assert!(output.contains("V3_VISIBLE"));
                assert!(!output.contains("LOCAL_ROLE_PRIVATE"));
            }
        }
    }
    assert_eq!(records.len(), 720);
    fix1_evidence("fix3-css-role-720", &records);
}

#[test]
fn fix3_declarative_shadow_roots_refuse_unslotted_light_dom_text() {
    let exact = "<table><tr><td>5-1 <div><template shadowrootmode=open><span>visible</span></template>SHADOW_PRIVATE</div></td><td>12</td></tr></table>";
    let result = fix1_page(PaperScope::Paper1, "definitions", exact, &[]);
    let exported = result
        .as_ref()
        .is_ok_and(|v| json(v).contains("SHADOW_PRIVATE"));
    fix1_evidence(
        "fix3-shadow-exact",
        &[obj(&[
            ("accepted", result.is_ok().into()),
            ("exported", exported.into()),
        ])],
    );
    assert!(!exported, "unslotted light-DOM text exported");
    assert_code(result, "contract_drift");
    let mut records = vec![];
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        for family in ["header", "inventory", "definitions", "batch"] {
            for mode in ["open", "closed", "OPEN", "", "unsupported"] {
                for content in ["<span>VISIBLE</span>", "<slot></slot>"] {
                    let body = fix3_body(
                        scope,
                        family,
                        &format!(
                            "<div><template shadowrootmode='{mode}'>{content}</template>SHADOW_PRIVATE</div>"
                        ),
                    );
                    assert_code(fix1_page(scope, family, &body, &[]), "contract_drift");
                    records.push(obj(&[
                        ("scope", scope.as_str().into()),
                        ("family", family.into()),
                        ("mode", mode.into()),
                    ]));
                }
            }
            let body = fix3_body(
                scope,
                family,
                "<div><template>TEMPLATE_PRIVATE</template>LIGHT_VISIBLE</div>",
            );
            let output = json(&fix1_page(scope, family, &body, &[]).unwrap());
            assert!(output.contains("LIGHT_VISIBLE"));
            assert!(!output.contains("TEMPLATE_PRIVATE"));
        }
    }
    assert_eq!(records.len(), 80);
    fix1_evidence("fix3-shadow-80", &records);
}
