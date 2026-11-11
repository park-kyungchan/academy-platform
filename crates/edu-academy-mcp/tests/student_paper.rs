//! E09c synthetic adapter tests. Invented pages/owners only; no live call,
//! no real student name/id/record, no credential. Schema literals below are
//! derived from the checked-in registry
//! `tests/fixtures/mcp-runtime-reference-v1/frozen/inventory/registry.json`:
//! academy_student_metadata_get:73, academy_student_metadata:255,
//! paper1 printed:3853 source:3908 headers:3967 inventory:4006
//! definition:4051 read_contracts:4093; class2 printed:4122 source:4177
//! headers:4236 inventory:4283 definition:4328 read_contracts:4370.
use edu_academy_mcp::{
    Js, encode, obj,
    paper::{PaperDriver, class2_name, is_paper_tool, upstream_name},
    parse,
    protocol::RpcSession,
    registry::ToolRegistry,
    release::ReleaseGraph,
    student::{StudentCall, StudentDriver},
    validation,
};
use edu_academy_sources::{
    data::*,
    http::{self, Response},
    owner_admission::{TransportAdmission, TransportScope},
    paper::{self, PaperOwner, PaperPins, PaperScope, PaperScopes, PaperSource},
    student_course::{self, Owner, ReadPort, read_contracts},
    student_metadata::{MetadataPolicy, PaperFormPin, StudentSource},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    rc::Rc,
};
const DATE: &str = "2040-01-04";
fn js(s: &str) -> Js {
    parse(s.as_bytes()).unwrap()
}
fn registry() -> ToolRegistry {
    ToolRegistry::parse(include_bytes!(
        "fixtures/mcp-runtime-reference-v1/frozen/inventory/registry.json"
    ))
    .unwrap()
}
fn entry<'a>(
    r: &'a ToolRegistry,
    endpoint: &str,
    upstream: &str,
) -> &'a edu_academy_mcp::registry::Entry {
    r.entries()
        .iter()
        .find(|e| e.endpoint == endpoint && e.upstream_name == upstream)
        .unwrap()
}

// ---------- shared invented fixtures (mirrors the E09a/E09b source tests) ----------
fn owner_value() -> Js {
    js(
        r#"{"ownerId":"fictional-owner","teacherLabel":"Fictional Teacher","teacherSourceKey":"99001","collegeSourceKey":"88001","students":[{"studentKey":"77001","studentName":"Fictional One"},{"studentKey":"77002","studentName":"Fictional Two"}]}"#,
    )
}
fn policy_value() -> Js {
    js(
        r#"{"schemaVersion":"academy.metadata-admission/1","sourceNamespace":"academy.lms.metadata","teacherId":"99001","allowedFrom":"2040-01-01","allowedThrough":"2040-01-31","studentScope":"configured","relativeSelectors":["1","11","12","13","14","15","16"],"operations":["student_papers","daily_metadata","daily_inventory","daily_history","printed_paper_metadata"]}"#,
    )
}
fn student_admission() -> TransportAdmission {
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
    policy: TransportAdmission,
    replies: VecDeque<(String, String)>,
    count: Rc<Cell<usize>>,
    seen: Capture,
    fault: Option<&'static str>,
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
        if let Some(code) = self.fault {
            return Err(Error::refusal(code));
        }
        let (expected, body) = self.replies.pop_front().expect("unexpected synthetic read");
        assert_eq!(id, expected, "operation order");
        Ok(response(&body))
    }
}
fn fake(
    admission: TransportAdmission,
    replies: Vec<(&str, String)>,
) -> (Fake, Rc<Cell<usize>>, Capture) {
    let count = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(vec![]));
    (
        Fake {
            policy: admission,
            replies: replies.into_iter().map(|(a, b)| (a.into(), b)).collect(),
            count: count.clone(),
            seen: seen.clone(),
            fault: None,
        },
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
fn day_main(name: &str, id: &str) -> String {
    format!(
        r#"<input name=std_date value="{DATE}"><input name=teacher_pri_no value=99001><select name=grp_seq><option value=3 selected>Fictional group</option></select>
<table><thead><tr><th rowspan=2>이름</th><th rowspan=2>과정</th><th rowspan=2>지속사항</th><th colspan=4>지난진도</th><th colspan=6>일괄 발송 금일진도</th></tr><tr><th>지난수업일</th><th>진도</th><th>숙제</th><th>메모</th><th></th><th>금일출석</th><th>일일테스트</th><th>진도</th><th>숙제</th><th>메모</th></tr></thead><tbody><tr><td><a onclick="onOpenDaysMain('{id}')">{name}</a></td><td>Fictional course</td><td></td><td></td><td></td><td></td><td></td><td></td><td><select onchange="onAttn('0','5','{id}','13','8')"><option value=Y>Fictional</option></select></td><td></td><td><textarea onfocusout="onFoPrg('0','5','{id}','13','8')">100%</textarea></td><td><textarea onfocusout="onFoHw('0','5','{id}','13','8')">Fictional</textarea></td><td><input onfocusout="onFoMm('0','5','{id}','13','8')" value="Fictional memo"></td></tr></tbody></table>
<script>function onAttn(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoPrg(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoHw(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onFoMm(i,course_seq,stu_pri_no,record_seq,cm_seq){{}}function onOpenDaysMain(student_pri_no){{}}</script>"#
    )
}
fn selected_rec(name: &str, id: &str) -> String {
    let fields = ["", DATE, "5", "", name, "", "", "", "", "3", "13", id, "8"];
    format!("<root><record>{}</record></root>", fields.join("!"))
}
fn routing_admission() -> Js {
    obj(&[
        ("sourceNamespace", "academy.lms.dayrecord".into()),
        ("teacherId", "99001".into()),
        ("allowedFrom", "2040-01-04".into()),
        ("allowedThrough", "2040-01-04".into()),
    ])
}
fn student_source(replies: Vec<(&str, String)>) -> StudentSource<Fake> {
    let (port, _, _) = fake(student_admission(), replies);
    StudentSource::new(
        port,
        Owner::parse(&owner_value()).unwrap(),
        student_admission(),
        MetadataPolicy::parse(&policy_value()).unwrap(),
        Some(PaperFormPin::parse(&form_value()).unwrap()),
    )
    .unwrap()
}
fn student_driver(replies: Vec<(&str, String)>) -> (StudentDriver<Fake>, Rc<Cell<usize>>) {
    let (port, count, _) = fake(student_admission(), replies);
    let source = StudentSource::new(
        port,
        Owner::parse(&owner_value()).unwrap(),
        student_admission(),
        MetadataPolicy::parse(&policy_value()).unwrap(),
        Some(PaperFormPin::parse(&form_value()).unwrap()),
    )
    .unwrap();
    (StudentDriver::new(source, routing_admission()), count)
}
/// Serialized length exactly as the adapters measure it (tool_result + encode).
fn serialized_len(value: &Js) -> usize {
    encode(&edu_academy_mcp::audit::tool_result(value.clone()).unwrap())
        .unwrap()
        .len()
}
const CAP_BYTES: usize = 512 * 1024;
const FIXED_NOW: &str = "2040-01-04T09:00:00.000Z";
fn student_request(queries: Vec<Js>, named: bool) -> Js {
    let mut v = obj(&[
        ("purpose", "Fictional metadata read".into()),
        ("currentRequestRef", "fictional-request".into()),
        ("queries", Js::Arr(queries)),
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

// ---------- R05: schema literals derived from registry.json (lines cited above) ----------
#[test]
fn schema_literals_match_registry_for_all_fourteen_tools() {
    let r = registry();
    // Paper schemas are small enough to pin as full literals.
    let student_id = r##"{"type":"string","pattern":"^[1-9][0-9]{0,15}$"}"##;
    for (endpoint, upstream, required) in [
        ("paper1", "academy_daily_test_inventory", vec!["studentId"]),
        ("paper1", "academy_paper_read_contracts", vec!["studentId"]),
        ("paper2", "academy_daily_test_inventory", vec!["studentId"]),
        ("paper2", "academy_paper_read_contracts", vec!["studentId"]),
    ] {
        let schema = entry(&r, endpoint, upstream).tool.get("inputSchema");
        assert_eq!(
            schema.get("additionalProperties"),
            &Js::Bool(false),
            "{upstream}"
        );
        for k in required {
            assert!(
                schema.get("required").array().iter().any(|x| x.is(k)),
                "{upstream} {k}"
            );
        }
        assert!(
            schema
                .get("properties")
                .get("studentId")
                .get("pattern")
                .is("^[1-9][0-9]{0,15}$")
        );
        let _ = student_id;
    }
    for upstream in [
        "academy_paper_headers",
        "academy_paper_definition_lookup",
        "academy_printed_paper_lookup",
        "academy_source_read_contracts",
    ] {
        for endpoint in ["paper1", "paper2"] {
            let schema = entry(&r, endpoint, upstream).tool.get("inputSchema");
            assert_eq!(
                schema.get("additionalProperties"),
                &Js::Bool(false),
                "{endpoint} {upstream}"
            );
            assert!(schema.get("properties").has("studentId"), "{upstream}");
        }
    }
    // class2 tool names are the bridge spellings of the paper2 entries.
    for e in r.entries().iter().filter(|e| e.endpoint == "paper2") {
        assert_eq!(
            e.tool.get("name").code_units(),
            format!("class2_{}", e.upstream_name),
            "paper2 tool name"
        );
        assert_eq!(
            upstream_name(&e.tool.get("name").code_units()),
            Some(e.upstream_name.as_str())
        );
        assert_eq!(
            class2_name(&e.upstream_name),
            e.tool.get("name").code_units()
        );
    }
    // Student query kinds: the oneOf const set, as a literal.
    for upstream in ["academy_student_metadata_get", "academy_student_metadata"] {
        let schema = entry(&r, "dailyrecord", upstream).tool.get("inputSchema");
        let items = schema
            .get("properties")
            .get("queries")
            .get("items")
            .get("oneOf")
            .array()
            .to_vec();
        let kinds: Vec<String> = items
            .iter()
            .map(|i| i.get("properties").get("kind").get("const").code_units())
            .collect();
        assert_eq!(
            kinds,
            [
                "student_papers",
                "daily_metadata",
                "daily_inventory",
                "daily_history",
                "printed_paper_metadata"
            ],
            "{upstream}"
        );
        assert_eq!(
            schema.get("properties").get("queries").get("maxItems"),
            &Js::Num(8.0)
        );
    }
    assert!(is_paper_tool("class2_academy_paper_headers"));
    assert!(!is_paper_tool("academy_student_metadata"));
}

#[test]
fn registry_schema_accepts_valid_and_refuses_invalid_for_every_tool() {
    let r = registry();
    let valid: Vec<(&str, &str, Js)> = vec![
        (
            "dailyrecord",
            "academy_student_metadata_get",
            student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], true),
        ),
        (
            "dailyrecord",
            "academy_student_metadata",
            student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false),
        ),
        (
            "paper1",
            "academy_daily_test_inventory",
            js(r#"{"studentId":"77001"}"#),
        ),
        (
            "paper1",
            "academy_paper_definition_lookup",
            js(r##"{"studentId":"77001","paperCode":"#5"}"##),
        ),
        (
            "paper1",
            "academy_paper_headers",
            js(r##"{"studentId":"77001","paperCodes":["#5"]}"##),
        ),
        (
            "paper1",
            "academy_paper_read_contracts",
            js(r#"{"studentId":"77001"}"#),
        ),
        (
            "paper1",
            "academy_printed_paper_lookup",
            js(
                r##"{"studentId":"77001","paperCode":"#5","period":{"from":"2040-01-01","through":"2040-01-04"}}"##,
            ),
        ),
        (
            "paper1",
            "academy_source_read_contracts",
            js(r#"{"studentId":"77001","kind":"printed_code"}"#),
        ),
        (
            "paper2",
            "academy_daily_test_inventory",
            js(r#"{"studentId":"77002"}"#),
        ),
        (
            "paper2",
            "academy_paper_definition_lookup",
            js(r##"{"studentId":"77002","paperCode":"#5"}"##),
        ),
        (
            "paper2",
            "academy_paper_headers",
            js(r##"{"studentId":"77002","paperCodes":["#5"]}"##),
        ),
        (
            "paper2",
            "academy_paper_read_contracts",
            js(r#"{"studentId":"77002"}"#),
        ),
        (
            "paper2",
            "academy_printed_paper_lookup",
            js(
                r##"{"studentId":"77002","paperCode":"#5","period":{"from":"2040-01-01","through":"2040-01-04"}}"##,
            ),
        ),
        (
            "paper2",
            "academy_source_read_contracts",
            js(r#"{"studentId":"77002","kind":"printed_code"}"#),
        ),
    ];
    for (endpoint, upstream, input) in &valid {
        let e = entry(&r, endpoint, upstream);
        validation::validate(e.tool.get("inputSchema"), input)
            .unwrap_or_else(|err| panic!("{upstream} valid refused: {}", err.code));
        let mut bad = input.clone();
        bad.set("unexpected", true.into());
        assert!(
            validation::validate(e.tool.get("inputSchema"), &bad).is_err(),
            "{upstream} extra key admitted"
        );
    }
}

// ---------- R05 minimum tests 1-3 through the student adapter ----------
#[test]
fn student_full_query_set_both_aliases() {
    for named in [false, true] {
        let mut replies: Vec<(&str, String)> = vec![];
        if named {
            let main = day_main("Fictional One", "77001");
            replies.extend([
                ("student.day.groups", main.clone()),
                ("student.day.main", main),
                (
                    "student.day.selected",
                    selected_rec("Fictional One", "77001"),
                ),
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
            ("metadata.paper.initial", paper_form()),
            (
                "metadata.paper.search.1",
                paper_result(vec![row("77001", "Fictional One", "3", "81001")]),
            ),
        ]);
        let (mut driver, count) = student_driver(replies);
        let mut printed = period_query("printed_paper_metadata");
        printed.set("paperCode", "#5".into());
        let queries = vec![
            period_query("student_papers"),
            js(r#"{"kind":"daily_metadata"}"#),
            js(r#"{"kind":"daily_inventory"}"#),
            js(r#"{"kind":"daily_history","periods":["11"]}"#),
            printed,
        ];
        let tool = if named {
            "academy_student_metadata_get"
        } else {
            "academy_student_metadata"
        };
        let out = driver.call(tool, &student_request(queries, named)).unwrap();
        assert_eq!(out.get("status").code_units(), "observed");
        assert_eq!(out.get("results").array().len(), 5);
        for res in out.get("results").array() {
            assert_eq!(res.get("observation").get("teacherJudgment"), &Js::Null);
        }
        assert!(
            !String::from_utf8(encode(&out).unwrap())
                .unwrap()
                .contains("NEVER_EXPORT")
        );
        assert_eq!(count.get(), if named { 11 } else { 8 });
        assert_eq!(driver.student_source_requests(), count.get());
    }
}

#[test]
fn student_invalid_and_identity_refusals_make_zero_requests() {
    // Unsupported query kind: validation in the source layer, zero reads.
    let (mut driver, count) = student_driver(vec![]);
    let out = driver
        .call(
            "academy_student_metadata",
            &student_request(vec![js(r#"{"kind":"missing"}"#)], false),
        )
        .unwrap();
    assert_eq!(out.get("status").code_units(), "unavailable");
    assert_eq!(out.get("failedSource").code_units(), "metadata");
    assert_eq!(count.get(), 0);
    // Wrong teacher / student / namespace in the keyed scope.
    for (key, value) in [
        ("teacherId", "99002"),
        ("studentId", "77003"),
        ("sourceNamespace", "academy.lms.dayrecord"),
    ] {
        let (mut d2, c2) = student_driver(vec![]);
        let mut input = student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false);
        let mut scope = input.get("scope").clone();
        scope.set(key, value.into());
        input.set("scope", scope);
        let out = d2.call("academy_student_metadata", &input).unwrap();
        assert_eq!(
            out.get("reasonCode").code_units(),
            "native_metadata_scope_refused",
            "{key}"
        );
        assert_eq!(c2.get(), 0, "{key}");
    }
    // Admission mismatch at construction: mismatched teacher refuses, zero reads.
    let (port, c3, _) = fake(student_admission(), vec![]);
    let mut bad = student_admission();
    bad.teacher_id = "99002".into();
    assert!(
        StudentSource::new(
            port,
            Owner::parse(&owner_value()).unwrap(),
            bad,
            MetadataPolicy::parse(&policy_value()).unwrap(),
            Some(PaperFormPin::parse(&form_value()).unwrap())
        )
        .is_err()
    );
    assert_eq!(c3.get(), 0);
}

#[test]
fn student_no_fallback_after_unqualified_primary_and_malformed_maps() {
    // Printed lookup: primary paper page drifts (renamed field) -> refusal,
    // and the daily fallback is never read.
    let drifted = paper_form().replace("sort_date1", "renamed_field");
    let (mut driver, count) = student_driver(vec![("metadata.paper.initial", drifted)]);
    let mut q = period_query("printed_paper_metadata");
    q.set("paperCode", "#5".into());
    let out = driver
        .call("academy_student_metadata", &student_request(vec![q], false))
        .unwrap();
    assert_eq!(out.get("status").code_units(), "unavailable");
    assert_eq!(count.get(), 1, "no fallback read after drift");
    // A latched failure stays latched with no further requests.
    let out2 = driver
        .call(
            "academy_student_metadata",
            &student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false),
        )
        .unwrap();
    assert_eq!(
        out2.get("reasonCode").code_units(),
        "source_failure_latched_no_retry"
    );
    assert_eq!(count.get(), 1);
}

// ---------- paper fixtures ----------
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
fn paper_owner_value(scope: PaperScope) -> Js {
    remap_js(
        js(
            r#"{"ownerId":"fictional-paper1","teacherLabel":"Fictional Teacher","teacherSourceKey":"99001","collegeSourceKey":"88001","students":[{"studentKey":"77001","studentName":"Fictional One"}]}"#,
        ),
        scope,
    )
}
fn paper_policy(scope: PaperScope) -> Js {
    remap_js(policy_value(), scope)
}
fn paper_routes() -> PaperScopes {
    PaperScopes::parse(&js(r##"[{"endpoint":"paper1","studentIds":["77001"]},{"endpoint":"paper2","studentIds":["77002"]}]"##)).unwrap()
}
fn paper_admission(scope: PaperScope) -> TransportAdmission {
    let owner = paper_owner_value(scope);
    let mut operations = read_contracts();
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
fn catalog_page() -> String {
    format!(
        r#"<form name=form1 id=catalog action="{}?reqCmd=DailyZeroTestSeach" method=post><input type=hidden name=evalgubun value=DT><input type=hidden name=frm_cl1 value=0><input type=hidden name=frm_cl2 value=-1><input type=hidden name=frm_cl3 value=-1></form><select id=cl1><option value=124>[22개정] 중1</option><option value=154>[22개정] 초5</option></select><script>function onSearchTestPaperList(){{return 'NEVER_EXECUTE';}}</script>"#,
        student_course::DAILY_PATH
    )
}
fn pool_page() -> String {
    "<form name=pool method=post action='/unused'><input name=category value=0></form><script>function loadCategory(){return 'NEVER_EXECUTE';}</script>".into()
}
fn converter_program() -> String {
    "const alphabet='23456789ABCDEFGHJKLMNPQRSTUVWXYZ';function keyToCode(n){return n;}function codeToKey(n){return n;}".into()
}
fn paper_pins(reference: &str) -> PaperPins {
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
            paper::contract_fingerprint(&catalog_page()).unwrap().into(),
        ),
        (
            "pool",
            paper::contract_fingerprint(&pool_page()).unwrap().into(),
        ),
        ("converter", hash(converter_program().as_bytes()).into()),
    ]))
    .unwrap()
}
fn paper_owner(scope: PaperScope) -> PaperOwner {
    PaperOwner::parse(
        scope,
        &paper_routes(),
        &paper_owner_value(scope),
        &paper_policy(scope),
        &remap_js(form_value(), scope),
        paper_pins(&reader_shell(scope)),
    )
    .unwrap()
}
fn paper_source(scope: PaperScope, replies: Vec<(&str, String)>) -> PaperSource<Fake> {
    let (port, _, _) = fake(paper_admission(scope), replies);
    PaperSource::new(port, paper_owner(scope), paper_admission(scope)).unwrap()
}
fn paper_driver(
    scope: PaperScope,
    replies: Vec<(&str, String)>,
) -> (PaperDriver<Fake>, Rc<Cell<usize>>) {
    let (port, count, _) = fake(paper_admission(scope), replies);
    let source = PaperSource::new(port, paper_owner(scope), paper_admission(scope)).unwrap();
    (
        PaperDriver::new(source, routing_admission(), DATE.into()),
        count,
    )
}
fn paper_input(scope: PaperScope) -> Js {
    remap_js(obj(&[("studentId", "77001".into())]), scope)
}
fn definition_xml() -> String {
    "<?xml version='1.0'?><root><inforesult><![CDATA[999##item##Fictional shared title##item##HIDDEN_FILE_LOCATION##item##12##item##1180##item##2##item##1##item##Fictional major##item##Fictional category]]></inforesult></root>".into()
}
fn header_page(key: &str) -> String {
    format!(
        r#"<html><head><title>Fictional header</title></head><body><iframe src="{}?p_process=TestPageTabSwitch&amp;p_testno={key}&amp;seltab=1"></iframe></body></html>"#,
        "/servlet/controller.common.TestPageShowServlet"
    )
}
fn content_header() -> String {
    "<h1>Fictional paper</h1><table><tr><th>시험지 제목</th><td>Fictional title</td></tr><tr><th>총 문제수</th><td>12</td></tr><tr><th>Question secret</th><td>NEVER_EXPORT_QUESTION</td></tr></table>".into()
}

// ---------- R06 minimum tests 1-4 through the paper adapters ----------
#[test]
fn paper_both_scopes_no_fallthrough() {
    for scope in [PaperScope::Paper1, PaperScope::Paper2] {
        let html = remap(&daily(), scope);
        let (mut driver, count) = paper_driver(scope, vec![("metadata.daily", html)]);
        let tool = if scope == PaperScope::Paper2 {
            "class2_academy_daily_test_inventory"
        } else {
            "academy_daily_test_inventory"
        };
        let out = driver.call(tool, &paper_input(scope)).unwrap();
        assert_eq!(out.get("endpoint").code_units(), scope.as_str());
        assert_eq!(count.get(), 1);
    }
    // class2 spelling on a paper1 driver refuses before any source request.
    let (mut driver, count) = paper_driver(PaperScope::Paper1, vec![]);
    let out = driver
        .call(
            "class2_academy_daily_test_inventory",
            &paper_input(PaperScope::Paper1),
        )
        .unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
    assert_eq!(out.get("code").code_units(), "paper_scope_unresolved");
    assert_eq!(out.get("failedSource").code_units(), "paper");
    assert_eq!(count.get(), 0);
    // A paper1 student on a paper2 driver is not admitted either.
    let (mut driver, count) = paper_driver(PaperScope::Paper2, vec![]);
    let out = driver
        .call(
            "academy_daily_test_inventory",
            &paper_input(PaperScope::Paper1),
        )
        .unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
    assert_eq!(count.get(), 0);
}

#[test]
fn paper_every_tool_valid_and_invalid() {
    // definition (default view)
    let (mut d, _) = paper_driver(
        PaperScope::Paper1,
        vec![
            ("metadata.paper.initial", reader_shell(PaperScope::Paper1)),
            ("paper.definition", definition_xml()),
        ],
    );
    let mut input = paper_input(PaperScope::Paper1);
    input.set("paperCode", "#5".into());
    let out = d.call("academy_paper_definition_lookup", &input).unwrap();
    assert_eq!(out.get("status").code_units(), "observed");
    assert!(
        !String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("HIDDEN_FILE_LOCATION")
    );
    // headers (header view, one code)
    let (mut d, _) = paper_driver(
        PaperScope::Paper1,
        vec![
            ("metadata.paper.initial", reader_shell(PaperScope::Paper1)),
            ("paper.header", header_page("3")),
            ("paper.content", content_header()),
        ],
    );
    let mut input = paper_input(PaperScope::Paper1);
    input.set("paperCodes", Js::Arr(vec!["#5".into()]));
    let out = d.call("academy_paper_headers", &input).unwrap();
    assert_eq!(out.get("status").code_units(), "observed");
    assert!(
        !String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("NEVER_EXPORT_QUESTION")
    );
    // read contracts
    let (mut d, _) = paper_driver(
        PaperScope::Paper1,
        vec![
            ("metadata.paper.initial", reader_shell(PaperScope::Paper1)),
            ("paper.pool", pool_page()),
        ],
    );
    let out = d
        .call(
            "academy_paper_read_contracts",
            &paper_input(PaperScope::Paper1),
        )
        .unwrap();
    assert_eq!(
        out.get("observation").get("status").code_units(),
        "inert_contracts_observed"
    );
    // source contracts: converter, catalog, middle1 definitions
    let (mut d, _) = paper_driver(
        PaperScope::Paper1,
        vec![("paper.converter", converter_program())],
    );
    let mut input = paper_input(PaperScope::Paper1);
    input.set("kind", "printed_code".into());
    assert_eq!(
        d.call("academy_source_read_contracts", &input)
            .unwrap()
            .get("status")
            .code_units(),
        "observed"
    );
    let (mut d, _) = paper_driver(PaperScope::Paper1, vec![("paper.catalog", catalog_page())]);
    let mut input = paper_input(PaperScope::Paper1);
    input.set("kind", "daily_test_catalog".into());
    assert_eq!(
        d.call("academy_source_read_contracts", &input)
            .unwrap()
            .get("status")
            .code_units(),
        "observed"
    );
    let (mut d, _) = paper_driver(
        PaperScope::Paper1,
        vec![
            ("paper.catalog", catalog_page()),
            (
                "paper.definitions",
                "<table><tr><td>5-1 Fictional</td><td>12</td></tr></table>".into(),
            ),
        ],
    );
    let mut input = paper_input(PaperScope::Paper1);
    input.set("kind", "middle1_definitions".into());
    input.set("unitCodes", Js::Arr(vec!["5-1".into()]));
    assert_eq!(
        d.call("academy_source_read_contracts", &input)
            .unwrap()
            .get("status")
            .code_units(),
        "observed"
    );
    // printed lookup through the paper family
    let (mut d, _) = paper_driver(
        PaperScope::Paper1,
        vec![
            ("metadata.paper.initial", paper_form()),
            (
                "metadata.paper.search.1",
                paper_result(vec![row("77001", "Fictional One", "3", "81001")]),
            ),
        ],
    );
    let mut input = paper_input(PaperScope::Paper1);
    input.set("paperCode", "#5".into());
    input.set(
        "period",
        obj(&[
            ("from", "2040-01-01".into()),
            ("through", "2040-01-04".into()),
        ]),
    );
    let out = d.call("academy_printed_paper_lookup", &input).unwrap();
    assert_eq!(out.get("status").code_units(), "observed");
    // invalid inputs refuse with zero extra requests beyond any shell read
    let (mut d, count) = paper_driver(PaperScope::Paper1, vec![]);
    let mut input = paper_input(PaperScope::Paper1);
    input.set("paperCode", "#0".into());
    let out = d.call("academy_paper_definition_lookup", &input).unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
    assert_eq!(count.get(), 0);
    let (mut d, count) = paper_driver(PaperScope::Paper1, vec![]);
    let mut input = paper_input(PaperScope::Paper1);
    input.set("kind", "unknown_kind".into());
    let out = d.call("academy_source_read_contracts", &input).unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
    assert_eq!(count.get(), 0);
    // duplicate header codes refuse (source duplicate guard)
    let (mut d, _) = paper_driver(PaperScope::Paper1, vec![]);
    let mut input = paper_input(PaperScope::Paper1);
    input.set("paperCodes", Js::Arr(vec!["#5".into(), "#5".into()]));
    let out = d.call("academy_paper_headers", &input).unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
}

#[test]
fn paper_identity_drift_and_budget_refusals() {
    // Student not in the owner scope: identity refusal, zero requests.
    let (mut d, count) = paper_driver(PaperScope::Paper1, vec![]);
    let out = d
        .call(
            "academy_paper_read_contracts",
            &obj(&[("studentId", "77009".into())]),
        )
        .unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
    assert_eq!(count.get(), 0);
    // Contract drift on the shell (renamed reader function) refuses typed.
    let drifted = reader_shell(PaperScope::Paper1).replace("viewDetailTest", "renamedFn");
    let (mut d, count) = paper_driver(
        PaperScope::Paper1,
        vec![("metadata.paper.initial", drifted)],
    );
    let out = d
        .call(
            "academy_paper_read_contracts",
            &paper_input(PaperScope::Paper1),
        )
        .unwrap();
    // R1: the public code is the catalogued projection, not the raw source
    // string; contract_drift is uncatalogued and is masked to the fallback.
    assert_eq!(out.get("code").code_units(), "source_read_unavailable");
    assert_eq!(
        out.get("failure").get("code").code_units(),
        out.get("code").code_units()
    );
    assert_eq!(out.get("failedSource").code_units(), "paper");
    assert_eq!(count.get(), 1);
    // Output budget: a definitions page with very many rows exceeds the cap
    // only through the source budget; the adapter envelope stays bounded.
    let big = format!(
        "<table>{}</table>",
        "<tr><td>5-1 Fictional</td><td>12</td></tr>".repeat(4000)
    );
    let (mut d, _) = paper_driver(
        PaperScope::Paper1,
        vec![
            ("paper.catalog", catalog_page()),
            ("paper.definitions", big),
        ],
    );
    let mut input = paper_input(PaperScope::Paper1);
    input.set("kind", "middle1_definitions".into());
    input.set("unitCodes", Js::Arr(vec!["5-1".into()]));
    let out = d.call("academy_source_read_contracts", &input).unwrap();
    assert!(
        encode(&out).unwrap().len() <= 512 * 1024 + 4096,
        "adapter result must stay bounded"
    );
}

// ---------- protocol error mapping over a fictional release graph ----------
fn put(path: &PathBuf, bytes: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
fn pin_json(path: &PathBuf) -> Js {
    obj(&[
        ("path", path.to_str().unwrap().into()),
        (
            "sha256",
            edu_academy_mcp::digest(&fs::read(path).unwrap()).into(),
        ),
    ])
}
fn doc(path: &PathBuf, value: &Js) -> Js {
    put(path, &encode(value).unwrap());
    pin_json(path)
}
struct Scene {
    install: PathBuf,
    base: PathBuf,
}
impl Drop for Scene {
    // R7: the fixture root is removed at the end of each test.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
impl Scene {
    fn new(label: &str) -> Self {
        Self::new_with(label, "fictional", None)
    }
    fn new_with(label: &str, kind: &str, scopes: Option<Js>) -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let base = home.join(format!(
            ".e09c-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        for name in ["install", "evidence", "state"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(name))
                .unwrap();
        }
        let install = base.join("install");
        let bridge = install.join("bridge");
        put(
            &bridge,
            b"fictional executable metadata only; never launched",
        );
        let launcher = install.join("launcher");
        put(&launcher, b"fictional launcher, never executed");
        let admission_path = install.join("admission.json");
        put(&admission_path, &encode(&routing_admission()).unwrap());
        let mut routing = obj(&[
            ("admissionPath", admission_path.to_str().unwrap().into()),
            (
                "admissionSha256",
                edu_academy_mcp::digest(&fs::read(&admission_path).unwrap()).into(),
            ),
        ]);
        routing.set(
            "paperScopes",
            scopes.unwrap_or_else(|| {
                Js::Arr(vec![
                    obj(&[
                        ("endpoint", "paper1".into()),
                        ("studentIds", Js::Arr(vec!["77001".into()])),
                    ]),
                    obj(&[
                        ("endpoint", "paper2".into()),
                        ("studentIds", Js::Arr(vec!["77002".into()])),
                    ]),
                ])
            }),
        );
        let routing_pin = doc(&install.join("routing.json"), &routing);
        let owner = doc(
            &install.join("owner.json"),
            &obj(&[
                ("schemaVersion", "academy.rust-owner/1".into()),
                ("kind", kind.into()),
                ("uid", Js::Num(rustix::process::geteuid().as_raw() as f64)),
                ("installRoot", install.to_str().unwrap().into()),
                (
                    "evidenceRoot",
                    base.join("evidence").to_str().unwrap().into(),
                ),
                ("stateRoot", base.join("state").to_str().unwrap().into()),
            ]),
        );
        let mut endpoints = obj(&[]);
        for endpoint in ["dailyrecord", "paper1", "paper2"] {
            let args = vec![
                "--install-root".to_string(),
                install.to_str().unwrap().into(),
                "--endpoint".into(),
                endpoint.into(),
            ];
            let mut artifact = pin_json(&bridge);
            artifact.set("role", "entry".into());
            let identity = doc(
                &install.join(format!("identity-{endpoint}.json")),
                &obj(&[
                    ("schemaVersion", "academy.rust-endpoint/1".into()),
                    ("endpoint", endpoint.into()),
                    ("semanticContractVersion", "s11-fictional".into()),
                    (
                        "launchArgumentsSha256",
                        edu_academy_mcp::digest(&serde_json::to_vec(&args).unwrap()).into(),
                    ),
                    ("artifacts", Js::Arr(vec![artifact])),
                ]),
            );
            let mut ep = obj(&[]);
            ep.set("executable", pin_json(&bridge));
            ep.set("identity", identity);
            ep.set("args", Js::Arr(args.into_iter().map(Js::from).collect()));
            endpoints.set(endpoint, ep);
        }
        let endpoints_pin = doc(&install.join("endpoints.json"), &endpoints);
        let original = registry();
        let entries: Vec<Js> = original
            .entries()
            .iter()
            .filter(|e| {
                (e.endpoint == "dailyrecord"
                    && edu_academy_mcp::student::TOOLS.contains(&e.upstream_name.as_str()))
                    || (["paper1", "paper2"].contains(&e.endpoint.as_str())
                        && edu_academy_mcp::paper::TOOLS.contains(&e.upstream_name.as_str()))
            })
            .map(|e| {
                obj(&[
                    ("endpoint", e.endpoint.clone().into()),
                    ("upstreamName", e.upstream_name.clone().into()),
                    ("tool", e.tool.clone()),
                ])
            })
            .collect();
        assert_eq!(entries.len(), 14);
        let registry_pin = doc(&install.join("registry.json"), &Js::Arr(entries));
        let source = install.join("source.bin");
        put(&source, b"fictional source version one");
        let mut source_pin = pin_json(&source);
        source_pin.set("role", "source".into());
        let build = doc(
            &install.join("build.json"),
            &obj(&[
                ("schemaVersion", "academy.rust-build/1".into()),
                ("sourceRevision", "0".repeat(40).into()),
                ("semanticContractVersion", "s11-fictional".into()),
                ("cargoLockSha256", "0".repeat(64).into()),
                ("artifacts", Js::Arr(vec![source_pin])),
            ]),
        );
        let pointer = obj(&[
            ("schemaVersion", "academy.mcp-release/3".into()),
            ("generation", "fictional-v1".into()),
            ("bridge", pin_json(&bridge)),
            ("launcher", pin_json(&launcher)),
            ("build", build),
            ("routing", routing_pin),
            ("owner", owner),
            ("registry", registry_pin),
            ("endpoints", endpoints_pin),
        ]);
        put(&install.join("release.json"), &encode(&pointer).unwrap());
        Self { install, base }
    }
    fn graph(&self) -> ReleaseGraph {
        ReleaseGraph::open(&self.install).unwrap()
    }
}
fn rpc(id: &str, tool: &str, args: &Js) -> Js {
    obj(&[
        ("jsonrpc", "2.0".into()),
        ("id", id.into()),
        ("method", "tools/call".into()),
        (
            "params",
            obj(&[("name", tool.into()), ("arguments", args.clone())]),
        ),
    ])
}

#[test]
fn protocol_maps_student_and_paper_calls_and_errors() {
    let scene = Scene::new("protocol");
    // student success + schema error mapping
    let (mut student, _) = student_driver(vec![("metadata.daily", daily())]);
    let (mut p1, _) = paper_driver(
        PaperScope::Paper1,
        vec![("metadata.daily", remap(&daily(), PaperScope::Paper1))],
    );
    let (mut p2, _) = paper_driver(
        PaperScope::Paper2,
        vec![("metadata.daily", remap(&daily(), PaperScope::Paper2))],
    );
    let mut session = RpcSession::endpoint(scene.graph(), "dailyrecord".into()).unwrap();
    let req = rpc(
        "1",
        "academy_student_metadata",
        &student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false),
    );
    let out = session
        .handle_student_paper(&req, &mut student, &mut [&mut p1, &mut p2])
        .unwrap()
        .unwrap();
    assert_eq!(
        out.get("result")
            .get("structuredContent")
            .get("status")
            .code_units(),
        "observed"
    );
    // invalid input -> text tool error, no source call needed
    let (mut student2, c2) = student_driver(vec![]);
    let (mut q1, _) = paper_driver(PaperScope::Paper1, vec![]);
    let (mut q2, _) = paper_driver(PaperScope::Paper2, vec![]);
    let mut bad = student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false);
    bad.set("unexpected", true.into());
    let out = session
        .handle_student_paper(
            &rpc("2", "academy_student_metadata", &bad),
            &mut student2,
            &mut [&mut q1, &mut q2],
        )
        .unwrap()
        .unwrap();
    assert!(out.get("result").get("isError").truthy());
    assert_eq!(c2.get(), 0);
    // paper success on its own endpoint session
    let mut session = RpcSession::endpoint(scene.graph(), "paper1".into()).unwrap();
    let (mut student3, _) = student_driver(vec![]);
    let (mut r1, _) = paper_driver(
        PaperScope::Paper1,
        vec![("metadata.daily", remap(&daily(), PaperScope::Paper1))],
    );
    let (mut r2, _) = paper_driver(PaperScope::Paper2, vec![]);
    let out = session
        .handle_student_paper(
            &rpc(
                "3",
                "academy_daily_test_inventory",
                &paper_input(PaperScope::Paper1),
            ),
            &mut student3,
            &mut [&mut r1, &mut r2],
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        out.get("result")
            .get("structuredContent")
            .get("status")
            .code_units(),
        "observed"
    );
    // class2 spelling inside the paper2 session maps to the paper2 driver
    let mut session = RpcSession::endpoint(scene.graph(), "paper2".into()).unwrap();
    let (mut student4, _) = student_driver(vec![]);
    let (mut s1, _) = paper_driver(PaperScope::Paper1, vec![]);
    let (mut s2, _) = paper_driver(
        PaperScope::Paper2,
        vec![("metadata.daily", remap(&daily(), PaperScope::Paper2))],
    );
    let out = session
        .handle_student_paper(
            &rpc(
                "4",
                "class2_academy_daily_test_inventory",
                &paper_input(PaperScope::Paper2),
            ),
            &mut student4,
            &mut [&mut s1, &mut s2],
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        out.get("result")
            .get("structuredContent")
            .get("endpoint")
            .code_units(),
        "paper2"
    );
    // admission mismatch: a driver bound to another admission refuses in-protocol
    let (port, _, _) = fake(student_admission(), vec![]);
    let source = StudentSource::new(
        port,
        Owner::parse(&owner_value()).unwrap(),
        student_admission(),
        MetadataPolicy::parse(&policy_value()).unwrap(),
        Some(PaperFormPin::parse(&form_value()).unwrap()),
    )
    .unwrap();
    let mut wrong = StudentDriver::new(
        source,
        obj(&[("sourceNamespace", "academy.lms.dayrecord".into())]),
    );
    let (mut t1, _) = paper_driver(PaperScope::Paper1, vec![]);
    let (mut t2, _) = paper_driver(PaperScope::Paper2, vec![]);
    let mut session = RpcSession::endpoint(scene.graph(), "dailyrecord".into()).unwrap();
    let out = session
        .handle_student_paper(
            &rpc(
                "5",
                "academy_student_metadata",
                &student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false),
            ),
            &mut wrong,
            &mut [&mut t1, &mut t2],
        )
        .unwrap()
        .unwrap();
    assert!(
        out.has("error"),
        "admission mismatch must be an RPC error: {out:?}"
    );
}

// ---------- repair 1: R1 catalogued envelopes, R2 over-cap, R3/R4 protocol ----------
fn big_daily(row_count: usize, title_len: usize) -> String {
    let title = "x".repeat(title_len);
    let rows: Vec<Js> = (0..row_count)
        .map(|i| {
            obj(&[
                ("priNo", "77001".into()),
                ("userName", "Fictional One".into()),
                ("pNo", format!("{}", i + 1).into()),
                ("testingNo", format!("{}", 81001 + i).into()),
                ("score", 100.into()),
                ("_id", "fictional-row".into()),
                ("title", title.as_str().into()),
            ])
        })
        .collect();
    result(rows)
}
fn student_driver_fault(code: &'static str) -> (StudentDriver<Fake>, Rc<Cell<usize>>) {
    let count = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(vec![]));
    let port = Fake {
        policy: student_admission(),
        replies: VecDeque::new(),
        count: count.clone(),
        seen,
        fault: Some(code),
    };
    let source = StudentSource::new(
        port,
        Owner::parse(&owner_value()).unwrap(),
        student_admission(),
        MetadataPolicy::parse(&policy_value()).unwrap(),
        Some(PaperFormPin::parse(&form_value()).unwrap()),
    )
    .unwrap();
    (StudentDriver::new(source, routing_admission()), count)
}
fn paper_driver_fault(
    scope: PaperScope,
    code: &'static str,
) -> (PaperDriver<Fake>, Rc<Cell<usize>>) {
    let count = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(vec![]));
    let port = Fake {
        policy: paper_admission(scope),
        replies: VecDeque::new(),
        count: count.clone(),
        seen,
        fault: Some(code),
    };
    let source = PaperSource::new(port, paper_owner(scope), paper_admission(scope)).unwrap();
    (
        PaperDriver::new(source, routing_admission(), DATE.into()),
        count,
    )
}
fn paper_driver_on(scope: PaperScope, lesson_date: &str) -> (PaperDriver<Fake>, Rc<Cell<usize>>) {
    let (port, count, _) = fake(paper_admission(scope), vec![]);
    let source = PaperSource::new(port, paper_owner(scope), paper_admission(scope)).unwrap();
    (
        PaperDriver::new(source, routing_admission(), lesson_date.into()),
        count,
    )
}

#[test]
fn r1_envelope_code_is_catalogued_and_uncatalogued_is_masked() {
    // Student: an uncatalogued upstream code never reaches the envelope.
    let (mut driver, count) = student_driver_fault("uncatalogued_upstream_xyz");
    let out = driver
        .call(
            "academy_student_metadata",
            &student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false),
        )
        .unwrap();
    assert_eq!(out.get("status").code_units(), "unavailable");
    assert_eq!(
        out.get("code").code_units(),
        "student_metadata_unqualified_no_automatic_retry"
    );
    assert_eq!(
        out.get("failure").get("code").code_units(),
        out.get("code").code_units()
    );
    assert_eq!(
        out.get("reasonCode").code_units(),
        out.get("code").code_units()
    );
    assert!(
        !String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("uncatalogued_upstream_xyz")
    );
    assert_eq!(count.get(), 1);
    // Student: a catalogued code passes through unchanged, still consistent.
    let (mut driver, _) = student_driver(vec![]);
    let mut input = student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false);
    let mut scope = input.get("scope").clone();
    scope.set("teacherId", "99002".into());
    input.set("scope", scope);
    let out = driver.call("academy_student_metadata", &input).unwrap();
    assert_eq!(
        out.get("code").code_units(),
        "native_metadata_scope_refused"
    );
    assert_eq!(
        out.get("failure").get("code").code_units(),
        out.get("code").code_units()
    );
    // Paper: same masking on the blocked envelope.
    let (mut driver, count) = paper_driver_fault(PaperScope::Paper1, "uncatalogued_upstream_xyz");
    let out = driver
        .call(
            "academy_daily_test_inventory",
            &paper_input(PaperScope::Paper1),
        )
        .unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
    assert_eq!(out.get("code").code_units(), "source_read_unavailable");
    assert_eq!(
        out.get("failure").get("code").code_units(),
        out.get("code").code_units()
    );
    assert!(
        !String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("uncatalogued_upstream_xyz")
    );
    assert_eq!(count.get(), 1);
}

#[test]
fn r2_over_cap_branches_for_metadata_and_paper() {
    // By construction: the same fixture is run through the source directly
    // to measure its serialized length, then through the adapter. Only the
    // title length changes, so only the 512 KiB cap branch can differ.
    let input = student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false);
    let under_value = student_source(vec![("metadata.daily", big_daily(200, 1133))])
        .metadata("academy_student_metadata", &input, FIXED_NOW)
        .unwrap();
    let over_value = student_source(vec![("metadata.daily", big_daily(200, 1134))])
        .metadata("academy_student_metadata", &input, FIXED_NOW)
        .unwrap();
    let under_len = serialized_len(&under_value);
    let over_len = serialized_len(&over_value);
    assert!(
        under_len <= CAP_BYTES && over_len > CAP_BYTES && over_len - under_len <= 16384,
        "metadata lengths must straddle the cap: under={under_len} over={over_len} cap={CAP_BYTES}"
    );
    let (mut driver, _) = student_driver(vec![("metadata.daily", big_daily(200, 1133))]);
    let small = driver.call("academy_student_metadata", &input).unwrap();
    assert_eq!(small.get("status").code_units(), "observed");
    let (mut driver, count) = student_driver(vec![("metadata.daily", big_daily(200, 1134))]);
    let out = driver.call("academy_student_metadata", &input).unwrap();
    assert_eq!(out.get("status").code_units(), "unavailable");
    assert_eq!(out.get("failedSource").code_units(), "metadata");
    assert_eq!(out.get("sourceRequests"), &Js::Num(1.0));
    assert_eq!(
        out.get("failure").get("code").code_units(),
        out.get("code").code_units()
    );
    assert_eq!(count.get(), 1);
    // Paper inventory: same construction through the shared cap code.
    let p_input = paper_input(PaperScope::Paper1);
    let under_value = paper_source(
        PaperScope::Paper1,
        vec![("metadata.daily", big_daily(200, 1073))],
    )
    .read("academy_daily_test_inventory", &p_input, DATE, FIXED_NOW)
    .unwrap();
    let over_value = paper_source(
        PaperScope::Paper1,
        vec![("metadata.daily", big_daily(200, 1074))],
    )
    .read("academy_daily_test_inventory", &p_input, DATE, FIXED_NOW)
    .unwrap();
    let under_len = serialized_len(&under_value);
    let over_len = serialized_len(&over_value);
    assert!(
        under_len <= CAP_BYTES && over_len > CAP_BYTES && over_len - under_len <= 16384,
        "paper lengths must straddle the cap: under={under_len} over={over_len} cap={CAP_BYTES}"
    );
    let (mut driver, _) = paper_driver(
        PaperScope::Paper1,
        vec![("metadata.daily", big_daily(200, 1073))],
    );
    let small = driver
        .call("academy_daily_test_inventory", &p_input)
        .unwrap();
    assert_eq!(small.get("status").code_units(), "observed");
    let (mut driver, count) = paper_driver(
        PaperScope::Paper1,
        vec![("metadata.daily", big_daily(200, 1074))],
    );
    let out = driver
        .call("academy_daily_test_inventory", &p_input)
        .unwrap();
    assert_eq!(out.get("status").code_units(), "blocked");
    assert_eq!(out.get("failedSource").code_units(), "paper");
    assert_eq!(
        out.get("failure").get("code").code_units(),
        out.get("code").code_units()
    );
    assert_eq!(count.get(), 1);
}

#[test]
fn r3_protocol_scope_crossing_refusals() {
    let scene = Scene::new("crossing");
    let (mut student, _) = student_driver(vec![]);
    let (mut p1, _) = paper_driver(PaperScope::Paper1, vec![]);
    let (mut p2, _) = paper_driver(PaperScope::Paper2, vec![]);
    // class2_* inside a paper1 session -> paper_scope_unresolved RPC error.
    let mut session = RpcSession::endpoint(scene.graph(), "paper1".into()).unwrap();
    let out = session
        .handle_student_paper(
            &rpc(
                "c1",
                "class2_academy_daily_test_inventory",
                &paper_input(PaperScope::Paper1),
            ),
            &mut student,
            &mut [&mut p1, &mut p2],
        )
        .unwrap()
        .unwrap();
    assert!(out.has("error"), "{out:?}");
    assert!(
        String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("paper_scope_unresolved")
    );
    // A paper tool name on the dailyrecord endpoint is not a student/paper
    // target and falls through to the unchanged unknown-tool refusal.
    let (mut student, _) = student_driver(vec![]);
    let (mut p1, _) = paper_driver(PaperScope::Paper1, vec![]);
    let (mut p2, _) = paper_driver(PaperScope::Paper2, vec![]);
    let mut session = RpcSession::endpoint(scene.graph(), "dailyrecord".into()).unwrap();
    let out = session
        .handle_student_paper(
            &rpc(
                "c2",
                "academy_paper_headers",
                &paper_input(PaperScope::Paper1),
            ),
            &mut student,
            &mut [&mut p1, &mut p2],
        )
        .unwrap()
        .unwrap();
    assert!(
        String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("unknown_tool"),
        "{out:?}"
    );
    // A student tool name on a paper endpoint likewise falls through.
    let (mut student, _) = student_driver(vec![]);
    let (mut p1, _) = paper_driver(PaperScope::Paper1, vec![]);
    let (mut p2, _) = paper_driver(PaperScope::Paper2, vec![]);
    let mut session = RpcSession::endpoint(scene.graph(), "paper1".into()).unwrap();
    let out = session
        .handle_student_paper(
            &rpc(
                "c3",
                "academy_student_metadata",
                &student_request(vec![js(r#"{"kind":"daily_metadata"}"#)], false),
            ),
            &mut student,
            &mut [&mut p1, &mut p2],
        )
        .unwrap()
        .unwrap();
    assert!(
        String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("unknown_tool"),
        "{out:?}"
    );
    // Missing driver for the session endpoint in the papers slice.
    let (mut student, _) = student_driver(vec![]);
    let (mut only_p2, _) = paper_driver(PaperScope::Paper2, vec![]);
    let mut session = RpcSession::endpoint(scene.graph(), "paper1".into()).unwrap();
    let out = session
        .handle_student_paper(
            &rpc(
                "c4",
                "academy_daily_test_inventory",
                &paper_input(PaperScope::Paper1),
            ),
            &mut student,
            &mut [&mut only_p2],
        )
        .unwrap()
        .unwrap();
    assert!(out.has("error"), "{out:?}");
    assert!(
        String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("paper_scope_unresolved")
    );
}

#[test]
fn r4_protocol_binding_and_lesson_date_refusals() {
    // A non-fictional owner graph is refused at graph open, before any
    // session or driver exists.
    let scene = Scene::new_with("nonfictional", "native", None);
    let err = match ReleaseGraph::open(&scene.install) {
        Ok(_) => panic!("non-fictional owner graph must refuse"),
        Err(e) => e,
    };
    assert_eq!(err.code, "native_owner_binding_unqualified");
    drop(scene);
    // paperScopes mismatch: declared students do not cover the driver scope.
    let scopes = Js::Arr(vec![
        obj(&[
            ("endpoint", "paper1".into()),
            ("studentIds", Js::Arr(vec!["77009".into()])),
        ]),
        obj(&[
            ("endpoint", "paper2".into()),
            ("studentIds", Js::Arr(vec!["77002".into()])),
        ]),
    ]);
    let scene = Scene::new_with("scopes", "fictional", Some(scopes));
    let (mut student, _) = student_driver(vec![]);
    let (mut p1, _) = paper_driver(PaperScope::Paper1, vec![]);
    let (mut p2, _) = paper_driver(PaperScope::Paper2, vec![]);
    let mut session = RpcSession::endpoint(scene.graph(), "paper1".into()).unwrap();
    let out = session
        .handle_student_paper(
            &rpc(
                "b1",
                "academy_daily_test_inventory",
                &paper_input(PaperScope::Paper1),
            ),
            &mut student,
            &mut [&mut p1, &mut p2],
        )
        .unwrap()
        .unwrap();
    assert!(out.has("error"), "{out:?}");
    assert!(
        String::from_utf8(encode(&out).unwrap())
            .unwrap()
            .contains("paper_scope_unresolved")
    );
    drop(session);
    drop(scene);
    // Empty and invalid lesson_date refuse before any source request.
    for bad_date in ["", "2040-02-30"] {
        let (mut driver, count) = paper_driver_on(PaperScope::Paper1, bad_date);
        let out = driver
            .call(
                "academy_daily_test_inventory",
                &paper_input(PaperScope::Paper1),
            )
            .unwrap();
        assert_eq!(out.get("status").code_units(), "blocked", "{bad_date}");
        assert_eq!(
            out.get("failure").get("code").code_units(),
            out.get("code").code_units(),
            "{bad_date}"
        );
        assert_eq!(count.get(), 0, "{bad_date}");
    }
}
