//! E11a prestudy source tests. Every fixture is synthetic (Invented Student
//! 77001, Invented Teacher 99001, dates in 2040), ported from the TS vectors
//! in `apps/academy/dc-gang-a/tests/lms/prestudyRead.test.ts` and
//! `modules/academy-workbench/tools/academy-mcp/tests/prestudy-*.test.ts`.
use edu_academy_sources::{
    data::Error,
    http::Response,
    owner_admission::{TransportAdmission, TransportScope},
    prestudy::*,
    student_course::{ReadPort, read_contracts},
};

const WAITING: &str = include_str!("fixtures/prestudy/waiting.html");
const COMPLETED: &str = include_str!("fixtures/prestudy/completed.html");
const SUMMARY: &str = include_str!("fixtures/prestudy/summary.html");
const DETAIL: &str = include_str!("fixtures/prestudy/detail.html");
const DETAIL_NO_REPORT: &str = include_str!("fixtures/prestudy/detail-no-report.html");
const REPORT: &str = include_str!("fixtures/prestudy/report.html");
const NOW: &str = "2040-01-16T09:00:00.000Z";

fn code(error: Error) -> String {
    error.message
}

fn err_code(result: edu_academy_sources::data::Result<Response>) -> String {
    match result {
        Ok(_) => panic!("expected refusal"),
        Err(error) => error.message,
    }
}

fn response(html: &str) -> Response {
    Response {
        status: 200,
        headers: vec![],
        bytes: html.as_bytes().to_vec(),
        url: String::new(),
        stalled: false,
    }
}

#[derive(Default)]
struct FakePort {
    calls: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    detail: Option<String>,
    report: Option<String>,
    shell_response: Option<Response>,
}
impl PrestudyPort for FakePort {
    fn shell(
        &mut self,
        kind: ListKind,
        bound: usize,
    ) -> edu_academy_sources::data::Result<Response> {
        self.calls
            .borrow_mut()
            .push(format!("shell:{} bound={bound}", kind.cmd()));
        if let (ListKind::Waiting, Some(response)) = (kind, &self.shell_response) {
            return Ok(Response {
                status: response.status,
                headers: vec![],
                bytes: response.bytes.clone(),
                url: String::new(),
                stalled: response.stalled,
            });
        }
        Ok(response(match kind {
            ListKind::Waiting => WAITING,
            ListKind::Completed => COMPLETED,
        }))
    }
    fn search(
        &mut self,
        kind: ListKind,
        form: &PrestudySearchForm,
        student_name: &str,
        bound: usize,
    ) -> edu_academy_sources::data::Result<Response> {
        self.calls.borrow_mut().push(format!(
            "search:{} form={}/{}/{}/{} student={student_name} bound={bound}",
            kind.cmd(),
            form.req_cmd,
            form.college_key,
            form.member_type,
            form.teacher_key
        ));
        Ok(response(match kind {
            ListKind::Waiting => WAITING,
            ListKind::Completed => COMPLETED,
        }))
    }
    fn summary(&mut self, bound: usize) -> edu_academy_sources::data::Result<Response> {
        self.calls
            .borrow_mut()
            .push(format!("summary bound={bound}"));
        Ok(response(SUMMARY))
    }
    fn detail(
        &mut self,
        prestudy_key: &str,
        bound: usize,
    ) -> edu_academy_sources::data::Result<Response> {
        self.calls
            .borrow_mut()
            .push(format!("detail key={prestudy_key} bound={bound}"));
        Ok(response(self.detail.as_deref().unwrap_or(DETAIL)))
    }
    fn report(
        &mut self,
        analysis_key: &str,
        bound: usize,
    ) -> edu_academy_sources::data::Result<Response> {
        self.calls
            .borrow_mut()
            .push(format!("report key={analysis_key} bound={bound}"));
        Ok(response(self.report.as_deref().unwrap_or(REPORT)))
    }
}

fn source() -> PrestudySource<FakePort> {
    PrestudySource::new(FakePort::default(), "Invented Teacher", "99001")
}

// R08-4 (list half) + prestudyRead.test.ts:12-26: form readback, waiting
// rows, exact-name filter.
#[test]
fn search_form_is_read_back_from_the_live_shell_and_bound_to_the_principal() {
    let form = parse_search_form(WAITING, ListKind::Waiting, "99001").unwrap();
    assert_eq!(
        form,
        PrestudySearchForm {
            req_cmd: "WebUnPreStudy".into(),
            college_key: "88001".into(),
            member_type: "1".into(),
            teacher_key: "99001".into(),
        }
    );
    assert_eq!(
        code(parse_search_form(WAITING, ListKind::Completed, "99001").unwrap_err()),
        "prestudy_search_form_drift"
    );
    assert_eq!(
        code(parse_search_form(WAITING, ListKind::Waiting, "99002").unwrap_err()),
        "prestudy_search_form_drift"
    );
    assert_eq!(
        code(
            parse_search_form(
                &format!("<input type=password>{WAITING}"),
                ListKind::Waiting,
                "99001"
            )
            .unwrap_err()
        ),
        "prestudy_authentication_required"
    );
    assert_eq!(
        code(parse_search_form("<html></html>", ListKind::Waiting, "99001").unwrap_err()),
        "prestudy_search_form_missing"
    );
}

#[test]
fn waiting_rows_keep_upload_literal_and_key_without_deriving_teacher_review() {
    let rows = parse_list(WAITING, ListKind::Waiting, "99001").unwrap();
    assert_eq!(rows.len(), 2);
    // The exact-name filter itself is crate-private; its effect is asserted
    // through read_lists (one of the two rows survives, below).
    assert_eq!(rows[0].student_label, "Invented Student");
    assert_eq!(rows[0].prestudy_key, "12345");
}

// prestudyRead.test.ts:28-34.
#[test]
fn summary_rows_retain_ai_star_literals_and_flags_but_never_the_video_url() {
    let rows = parse_summary(SUMMARY).unwrap();
    assert_eq!(
        rows,
        vec![PrestudySummaryRow {
            student_label: "Invented Student(초5)".into(),
            unit_label: "9-1 Fictional Unit(ver. 1)".into(),
            date_literal: "40.01.02 03:04".into(),
            ai_score_literal: "77점 보고서".into(),
            rating_literal: "-".into(),
            report_available: true,
            video_available: true,
        }]
    );
    assert!(!format!("{rows:?}").contains("cdn.example"));
    // The summary label carries the parenthesised suffix and still matches
    // the exact student through read_lists (asserted there).
}

// R08-4: an observed empty surface is observed, not unavailable.
#[test]
fn read_lists_projects_all_surfaces_with_teacher_judgment_separate() {
    let mut source = source();
    let lists = source
        .read_lists("Invented Teacher", "Invented Student", NOW)
        .unwrap();
    assert_eq!(lists.waiting.len(), 1);
    assert_eq!(lists.waiting[0].prestudy_key, "12345");
    assert!(lists.completed.is_empty());
    assert_eq!(lists.summary.len(), 1);
    assert_eq!(lists.teacher_judgment, None);
    assert_eq!(source.requests(), 5);
}

// R08-1 + prestudyRead.test.ts:88-94: detail identity binding.
#[test]
fn detail_binds_exact_student_and_record_and_preserves_source_literals() {
    assert_eq!(
        parse_detail(DETAIL, "77001", "12345").unwrap(),
        PrestudyDetail {
            prestudy_key: "12345".into(),
            unit_label: Some("9-1 Fictional Unit(ver. 1)".into()),
            date_literal: Some("2040-01-02 03:04:05".into()),
            analysis_key: Some("55501".into()),
            encoding_result: Some("success".into()),
        }
    );
    let bare = parse_detail(DETAIL_NO_REPORT, "77001", "12345").unwrap();
    assert_eq!(bare.unit_label, None);
    assert_eq!(bare.date_literal, None);
    assert_eq!(bare.analysis_key, None);
}

// prestudyRead.test.ts:96-104.
#[test]
fn duplicate_detail_identity_fields_and_conflicting_report_links_fail_closed() {
    for html in [
        DETAIL.replace(
            "</form>",
            "<input name=\"prestudy_key\" value=\"12346\"></form>",
        ),
        DETAIL.replace(
            "</form>",
            "<input name=\"student_pri_no\" value=\"77002\"></form>",
        ),
        format!(
            "{DETAIL}<form name=\"form1\"><input name=\"prestudy_key\" value=\"12345\"></form>"
        ),
        format!("{DETAIL}<button onclick=\"viewAiReport('55502')\">other report</button>"),
        DETAIL.replace("name=\"prestudy_key\"", "name=\"unrelated_key\""),
    ] {
        assert_eq!(
            code(parse_detail(&html, "77001", "12345").unwrap_err()),
            "prestudy_detail_unqualified",
            "{html}"
        );
    }
}

// R08-2 (source half) + prestudyRead.test.ts:112-121.
#[test]
fn report_parses_server_rendered_cards_and_scores_without_a_transcript() {
    let report = parse_report(REPORT).unwrap();
    assert_eq!(report.scores, vec![77.0, 80.0, 90.0]);
    assert_eq!(report.sections.len(), 2);
    assert_eq!(
        report.sections[1],
        PrestudyReportSection {
            title: "개념 이해".into(),
            subtitle: Some("개념을 정확히 이해하고 설명하는가?".into()),
            blocks: vec![
                PrestudyReportBlock {
                    heading: Some("포함된 핵심 개념".into()),
                    text: "첫째 줄\n둘째 줄".into(),
                },
                PrestudyReportBlock {
                    heading: Some("advice".into()),
                    text: "조언 본문".into(),
                },
            ],
        }
    );
    assert!(!format!("{report:?}").contains("transcript"));
}

// R08-2 selection (source half of prestudy-report-precision.test.ts:27-48):
// source order, exact partial, ambiguous refusal.
#[test]
fn section_selection_preserves_source_order_and_reports_missing_titles() {
    let report = PrestudyReport {
        sections: vec![
            section("개념 이해", "narrative one"),
            section("논리적 설명", "narrative two"),
            section("총평", "narrative three"),
        ],
        scores: vec![77.0],
    };
    let requested = vec!["총평".to_string(), "개념 이해".to_string()];
    let (sections, view) = select_report_view(&report, Some(&requested)).unwrap();
    assert_eq!(
        sections
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        ["개념 이해", "총평"]
    );
    assert!(view.complete_requested_view);
    assert!(!view.complete_source_report);
    let requested = vec!["概念理解".to_string(), "개념 이해".to_string()];
    let (sections, view) = select_report_view(&report, Some(&requested)).unwrap();
    assert_eq!(sections.len(), 1);
    assert_eq!(view.missing_titles, ["概念理解"]);
    assert!(!view.complete_requested_view);
    let (sections, view) = select_report_view(&report, None).unwrap();
    assert_eq!(sections.len(), 3);
    assert!(view.complete_source_report);
}

fn section(title: &str, text: &str) -> PrestudyReportSection {
    PrestudyReportSection {
        title: title.into(),
        subtitle: None,
        blocks: vec![PrestudyReportBlock {
            heading: None,
            text: text.into(),
        }],
    }
}

// Full source read: detail first, report second, sourceRecord preserved.
#[test]
fn read_report_binds_detail_before_following_the_analysis_key() {
    let mut source = source();
    let observation = source
        .read_report(
            "Invented Teacher",
            "Invented Student",
            "77001",
            "12345",
            NOW,
        )
        .unwrap();
    assert_eq!(observation.prestudy_key, "12345");
    assert_eq!(
        observation.source_record,
        PrestudySourceRecord {
            prestudy_key: "12345".into(),
            unit_label: Some("9-1 Fictional Unit(ver. 1)".into()),
            date_literal: Some("2040-01-02 03:04:05".into()),
        }
    );
    assert_eq!(observation.sections.len(), 2);
    assert_eq!(observation.scores, vec![77.0, 80.0, 90.0]);
    assert_eq!(source.requests(), 2);
}

// The twelve prestudy_report_*/prestudy_detail_* cases (GAP-E11 item 1b
// step 10), one test each with its exact code.

#[test]
fn error_prestudy_source_port_unavailable() {
    let (mut port, scope) = session_port(false);
    assert_eq!(
        err_code(port.shell(ListKind::Waiting, MAX_SOURCE_BYTES)),
        "prestudy_source_port_unavailable"
    );
    drop(scope);
}

#[test]
fn error_prestudy_report_source_port_unavailable() {
    let (mut port, _scope) = session_port(false);
    assert_eq!(
        err_code(port.report("55501", MAX_SOURCE_BYTES)),
        "prestudy_report_source_port_unavailable"
    );
    assert_eq!(
        err_code(port.detail("12345", MAX_SOURCE_BYTES)),
        "prestudy_report_source_port_unavailable"
    );
}

#[test]
fn error_prestudy_report_principal_rejected() {
    let mut source = source();
    assert_eq!(
        code(
            source
                .read_report("Other Teacher", "Invented Student", "77001", "12345", NOW)
                .unwrap_err()
        ),
        "prestudy_report_principal_rejected"
    );
    assert_eq!(source.requests(), 0);
}

#[test]
fn error_prestudy_detail_student_mismatch() {
    assert_eq!(
        code(parse_detail(DETAIL, "77002", "12345").unwrap_err()),
        "prestudy_detail_student_mismatch"
    );
}

#[test]
fn error_prestudy_detail_record_mismatch() {
    assert_eq!(
        code(parse_detail(DETAIL, "77001", "12346").unwrap_err()),
        "prestudy_detail_record_mismatch"
    );
}

#[test]
fn error_prestudy_detail_unqualified() {
    assert_eq!(
        code(parse_detail("<html><body>gone</body></html>", "77001", "12345").unwrap_err()),
        "prestudy_detail_unqualified"
    );
}

#[test]
fn error_prestudy_report_scope_conflict() {
    let detail = parse_detail(DETAIL, "77001", "12345").unwrap();
    assert_eq!(
        code(bind_source_record(&detail, "12346").unwrap_err()),
        "prestudy_report_scope_conflict"
    );
}

#[test]
fn error_prestudy_report_result_unqualified() {
    assert_eq!(
        code(qualify_report_key("12346", "12345").unwrap_err()),
        "prestudy_report_result_unqualified"
    );
    assert!(qualify_report_key("12345", "12345").is_ok());
}

#[test]
fn error_prestudy_report_unqualified() {
    assert_eq!(
        code(parse_report("<html><body>empty</body></html>").unwrap_err()),
        "prestudy_report_unqualified"
    );
}

#[test]
fn error_prestudy_report_source_rejected() {
    let big = format!(
        "<div class=\"max-w-5xl rounded-2xl\"><h1>개념 이해</h1><div class=\"text-sm\">{}</div></div>",
        "가".repeat(60_001)
    );
    assert_eq!(
        code(parse_report(&big).unwrap_err()),
        "prestudy_report_source_rejected"
    );
}

#[test]
fn error_prestudy_report_not_ready() {
    assert_eq!(
        code(
            parse_report("<html><body><h1>리포트를 생성할 수 없습니다.</h1></body></html>")
                .unwrap_err()
        ),
        "prestudy_report_not_ready"
    );
    // No viewAiReport handler: the detail is observed, the report is not.
    let port = FakePort {
        detail: Some(DETAIL_NO_REPORT.to_string()),
        ..Default::default()
    };
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    assert_eq!(
        code(
            source
                .read_report(
                    "Invented Teacher",
                    "Invented Student",
                    "77001",
                    "12345",
                    NOW
                )
                .unwrap_err()
        ),
        "prestudy_report_not_ready"
    );
    assert_eq!(source.requests(), 1);
}

// R08-3 at the source boundary: the budget helper E11b applies. At/below
// passes, above refuses the whole payload; nothing is clipped.
#[test]
fn error_prestudy_report_result_too_large() {
    assert!(check_report_budget(1024, 1024).is_ok());
    assert_eq!(
        code(check_report_budget(1025, 1024).unwrap_err()),
        "prestudy_report_result_too_large"
    );
    // UTF-8 length is bytes, not characters (Hangul/astral vectors).
    assert_eq!(utf8_json_len("가"), 3);
    assert_eq!(utf8_json_len("😀"), 4);
    assert_eq!(DEFAULT_SELECTED_MAX_BYTES, 32 * 1024);
    assert_eq!(DEFAULT_FULL_MAX_BYTES, 512 * 1024);
}

#[test]
fn error_prestudy_report_section_ambiguous() {
    let report = PrestudyReport {
        sections: vec![section("개념 이해", "one"), section("개념 이해", "other")],
        scores: vec![],
    };
    let requested = vec!["개념 이해".to_string()];
    assert_eq!(
        code(select_report_view(&report, Some(&requested)).unwrap_err()),
        "prestudy_report_section_ambiguous"
    );
}

// R08-4: malformed list/summary rows refuse; empty stays distinct (above).
#[test]
fn malformed_rows_fail_closed() {
    let bad_list = WAITING.replace(
        "<td>Invented Student</td><td>9-1 Fictional Unit (기본)(ver. 1)</td>",
        "<td></td><td></td>",
    );
    assert_eq!(
        code(parse_list(&bad_list, ListKind::Waiting, "99001").unwrap_err()),
        "prestudy_row_unqualified"
    );
    let bad_summary = SUMMARY.replace(
        "<td>Invented Student(초5)</td><td>9-1 Fictional Unit(ver. 1)</td>",
        "<td></td><td></td>",
    );
    assert_eq!(
        code(parse_summary(&bad_summary).unwrap_err()),
        "prestudy_summary_row_unqualified"
    );
}

thread_local! {
    // Each test runs on its own thread, so the record is per test.
    static RECORDED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}
struct FakeReadPort {
    admission: TransportAdmission,
}
impl ReadPort for FakeReadPort {
    fn transport_admission(&self) -> &TransportAdmission {
        &self.admission
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> edu_academy_sources::data::Result<Response> {
        RECORDED.with(|r| {
            r.borrow_mut().push(format!(
                "{operation} teacher={} {arguments:?}",
                scope.teacher_id
            ))
        });
        Ok(response(match operation {
            "prestudy.waiting.shell" | "prestudy.waiting.search" => WAITING,
            "prestudy.completed.shell" | "prestudy.completed.search" => COMPLETED,
            "prestudy.summary" => SUMMARY,
            "prestudy.detail" => DETAIL,
            _ => REPORT,
        }))
    }
}

fn session_port(admitted: bool) -> (SessionPrestudyPort<'static, FakeReadPort>, TransportScope) {
    let operations = if admitted {
        serde_json::to_value(read_contracts()).unwrap()
    } else {
        serde_json::json!([])
    };
    let admission: TransportAdmission = serde_json::from_value(serde_json::json!({
        "schemaVersion": "academy.transport-admission/1",
        "ownerId": "invented-owner",
        "teacherId": "99001",
        "teacherLabel": "Invented Teacher",
        "collegeId": "88001",
        "allowedFrom": "2040-01-01",
        "allowedThrough": "2040-12-31",
        "studentIds": ["77001"],
        "sourceNamespaces": ["academy.lms.student_course"],
        "operations": operations,
    }))
    .unwrap();
    let scope = TransportScope {
        owner_id: "invented-owner".into(),
        source_namespace: "academy.lms.student_course".into(),
        teacher_id: "99001".into(),
        student_id: "77001".into(),
        date: "2040-01-02".into(),
    };
    // Test-only leak: the session port borrows the fake port for 'static.
    let port = Box::leak(Box::new(FakeReadPort { admission }));
    (SessionPrestudyPort::new(port, scope.clone()), scope)
}

// F1: blockText recursion — nested p/div, br, script/style skipped.
// Before the repair the Rust walk returned lines A / BC for the first
// vector and leaked script/style text; TS returns AB / C and no leak.
#[test]
fn block_text_recurses_like_ts_and_skips_script_and_style() {
    // With an h3 the nested `p` is skipped by the card loop (TS :177), so
    // only the recursive text of the div remains.
    let html = r#"<div class="max-w-5xl rounded-2xl"><h1>개념 이해</h1><h3>H</h3><div class="text-sm">A<p>B</p>C</div></div>"#;
    let report = parse_report(html).unwrap();
    assert_eq!(report.sections[0].blocks[0].text, "AB\nC");
    // Without an h3 the nested `p` is also matched by the card loop and its
    // text is appended as a second line, in TS and here (:175-181).
    let html = r#"<div class="max-w-5xl rounded-2xl"><h1>개념 이해</h1><div class="text-sm">A<p>B</p>C</div></div>"#;
    let report = parse_report(html).unwrap();
    assert_eq!(report.sections[0].blocks[0].text, "AB\nC\nB");
    let html = r#"<div class="max-w-5xl rounded-2xl"><h1>개념 이해</h1><h3>H</h3><div class="text-sm">첫째 줄<br>둘째 줄<script>var secret=1;</script><style>.x{color:red}</style></div></div>"#;
    let report = parse_report(html).unwrap();
    assert_eq!(report.sections[0].blocks[0].text, "첫째 줄\n둘째 줄");
    assert!(!format!("{:?}", report.sections[0].blocks[0].text).contains("secret"));
}

// F2/F9: detail values are raw; a padded id/key refuses as TS does, and an
// over-safe-integer source key is a record mismatch, not unqualified.
#[test]
fn detail_whitespace_and_key_pattern_vectors() {
    let padded = DETAIL.replace("value=\"77001\"", "value=\" 77001\"");
    assert_eq!(
        code(parse_detail(&padded, "77001", "12345").unwrap_err()),
        "prestudy_detail_student_mismatch"
    );
    let padded = DETAIL.replace("value=\"12345\"", "value=\" 12345\"");
    assert_eq!(
        code(parse_detail(&padded, "77001", "12345").unwrap_err()),
        "prestudy_detail_unqualified"
    );
    // 9007199254740993 fits the parser pattern (16 digits) but not the
    // entry bound: TS parser gives record mismatch against the request.
    let big = DETAIL.replace("value=\"12345\"", "value=\"9007199254740993\"");
    assert_eq!(
        code(parse_detail(&big, "77001", "12345").unwrap_err()),
        "prestudy_detail_record_mismatch"
    );
    // Unit/date literals are preserved raw, including padding.
    let spaced = DETAIL.replace(
        "value=\"9-1 Fictional Unit(ver. 1)\"",
        "value=\" 9-1 Fictional Unit (ver. 1) \"",
    );
    assert_eq!(
        parse_detail(&spaced, "77001", "12345")
            .unwrap()
            .unit_label
            .as_deref(),
        Some(" 9-1 Fictional Unit (ver. 1) ")
    );
}

// F5/F6/F12: list principal code, entry guards, scope teacher binding.
#[test]
fn list_principal_and_entry_guards() {
    let mut first = source();
    assert_eq!(
        code(
            first
                .read_lists("Other Teacher", "Invented Student", NOW)
                .unwrap_err()
        ),
        "prestudy_principal_rejected"
    );
    assert_eq!(
        code(first.read_lists("Invented Teacher", "", NOW).unwrap_err()),
        "invalid_read_name"
    );
    assert_eq!(
        code(
            first
                .read_lists("Invented Teacher", "   ", NOW)
                .unwrap_err()
        ),
        "invalid_read_name"
    );
    let mut source = source();
    assert_eq!(
        code(
            source
                .read_report(
                    "Invented Teacher",
                    "Invented Student",
                    "77001",
                    "9007199254740993",
                    NOW
                )
                .unwrap_err()
        ),
        "invalid_read_key"
    );
    assert_eq!(
        code(
            source
                .read_report("Invented Teacher", "Invented Student", "abc", "12345", NOW)
                .unwrap_err()
        ),
        "invalid_read_key"
    );
    // Session scope teacher id must equal the source teacher key.
    let (port, _scope) = session_port(true);
    let mut source = PrestudySource::new(port, "Invented Teacher", "99002");
    assert_eq!(
        code(
            source
                .read_lists("Invented Teacher", "Invented Student", NOW)
                .unwrap_err()
        ),
        "prestudy_principal_rejected"
    );
}

// F8: the search form value may live on a select element.
#[test]
fn search_form_reads_select_element_values() {
    let html = WAITING.replace(
        "<input type=\"hidden\" name=\"g_mem_type\" value=\"1\">",
        "<select name=\"g_mem_type\"><option value=\"1\" selected>1</option></select>",
    );
    let form = parse_search_form(&html, ListKind::Waiting, "99001").unwrap();
    assert_eq!(form.member_type, "1");
}

// F13: real-bound vectors.
#[test]
fn bounds_at_their_real_values() {
    // Exactly 60,000 units accepted; 60,001 refused (existing test).
    let exact = format!(
        "<div class=\"max-w-5xl rounded-2xl\"><h1>개념 이해</h1><div class=\"text-sm\">{}</div></div>",
        "가".repeat(60_000)
    );
    assert!(parse_report(&exact).is_ok());
    // Row cap: 1,000 rows accepted, 1,001 refused.
    let row = "<tr><td>Invented Student</td><td>Unit</td><td>40.01.02</td><td>OK</td><td><button onclick=\"prestudy_main(12345);\">x</button></td></tr>";
    let form = "<form name=\"form1\"><input name=\"reqCmd\" value=\"WebUnPreStudy\"><input name=\"g_clg_no\" value=\"88001\"><input name=\"g_mem_type\" value=\"1\"><input name=\"g_pri_no\" value=\"99001\"><input name=\"student_name\" value=\"\"></form>";
    let ok = format!("<html>{form}<table>{}</table></html>", row.repeat(1000));
    assert_eq!(
        parse_list(&ok, ListKind::Waiting, "99001").unwrap().len(),
        1000
    );
    let over = format!("<html>{form}<table>{}</table></html>", row.repeat(1001));
    assert_eq!(
        code(parse_list(&over, ListKind::Waiting, "99001").unwrap_err()),
        "prestudy_row_unqualified"
    );
}

#[test]
fn port_bounds_status_stall_bom_and_invalid_utf8() {
    // A body above 2,000,000 bytes but below 2,000,000 UTF-16 units is
    // accepted at the port (F3): 700,000 Hangul chars = 2,100,000 bytes,
    // 700,000 units. The check is made through the source, not a parser.
    let form = "<form name=\"form1\"><input name=\"reqCmd\" value=\"WebUnPreStudy\"><input name=\"g_clg_no\" value=\"88001\"><input name=\"g_mem_type\" value=\"1\"><input name=\"g_pri_no\" value=\"99001\"><input name=\"student_name\" value=\"\"></form>";
    let multibyte = format!("<html>{form}<p>{}</p></html>", "가".repeat(700_000));
    assert!(multibyte.len() > 2_000_000 && multibyte.len() < 8_000_000);
    let port = FakePort {
        shell_response: Some(response(&multibyte)),
        ..Default::default()
    };
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    assert!(
        source
            .read_lists("Invented Teacher", "Invented Student", NOW)
            .is_ok()
    );
    // Over 8,000,000 bytes, and over 2,000,000 UTF-16 units under 8,000,000
    // bytes (astral characters: 4 bytes, 2 units), refuse.
    let over_bytes = format!("<html>{form}<p>{}</p></html>", "a".repeat(8_000_000));
    let over_units = format!("<html>{form}<p>{}</p></html>", "😀".repeat(1_000_001));
    assert!(over_units.len() < 8_000_000);
    for body in [over_bytes, over_units] {
        let port = FakePort {
            shell_response: Some(response(&body)),
            ..Default::default()
        };
        let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
        assert_eq!(
            code(
                source
                    .read_lists("Invented Teacher", "Invented Student", NOW)
                    .unwrap_err()
            ),
            "prestudy_source_rejected"
        );
    }
    for (status, stalled, bytes) in [
        (500u16, false, WAITING.as_bytes().to_vec()),
        (200, true, WAITING.as_bytes().to_vec()),
        (200, false, vec![0xFF, 0xFE, 0x3C]),
    ] {
        let port = FakePort {
            shell_response: Some(Response {
                status,
                headers: vec![],
                bytes,
                url: String::new(),
                stalled,
            }),
            ..Default::default()
        };
        let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
        assert_eq!(
            code(
                source
                    .read_lists("Invented Teacher", "Invented Student", NOW)
                    .unwrap_err()
            ),
            "prestudy_source_rejected"
        );
    }
    // A leading BOM is stripped, then the page parses.
    let port = FakePort {
        shell_response: Some(response(&format!("\u{FEFF}{WAITING}"))),
        ..Default::default()
    };
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    assert!(
        source
            .read_lists("Invented Teacher", "Invented Student", NOW)
            .is_ok()
    );
}

// F13: the fake port records its arguments; every read's parameters and
// the bound are asserted.
#[test]
fn fake_port_records_request_parameters() {
    let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let port = FakePort {
        calls: calls.clone(),
        ..Default::default()
    };
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    source
        .read_lists("Invented Teacher", "Invented Student", NOW)
        .unwrap();
    source
        .read_report(
            "Invented Teacher",
            "Invented Student",
            "77001",
            "12345",
            NOW,
        )
        .unwrap();
    let calls = calls.borrow();
    assert!(
        calls[0].starts_with("shell:WebUnPreStudy bound=8000000"),
        "{calls:?}"
    );
    assert!(
        calls[1].contains("form=WebUnPreStudy/88001/1/99001 student=Invented Student"),
        "{calls:?}"
    );
    assert!(
        calls.iter().any(|c| c == "detail key=12345 bound=8000000"),
        "{calls:?}"
    );
    assert!(
        calls.iter().any(|c| c == "report key=55501 bound=8000000"),
        "{calls:?}"
    );
}

// F4: backend shape limits at and over the bound, through read_report.
#[test]
fn report_shape_limits_are_enforced_in_read_report() {
    let long_unit = DETAIL.replace(
        "value=\"9-1 Fictional Unit(ver. 1)\"",
        &format!("value=\"{}\"", "u".repeat(501)),
    );
    let port = FakePort {
        detail: Some(long_unit),
        ..Default::default()
    };
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    assert_eq!(
        code(
            source
                .read_report(
                    "Invented Teacher",
                    "Invented Student",
                    "77001",
                    "12345",
                    NOW
                )
                .unwrap_err()
        ),
        "source_response_malformed"
    );
    let many_sections = (0..17)
        .map(|i| format!("<div class=\"max-w-5xl rounded-2xl\"><h1>Section {i}</h1><div class=\"text-sm\">text</div></div>"))
        .collect::<Vec<_>>()
        .join("");
    let port = FakePort {
        report: Some(format!("<html><body>{many_sections}</body></html>")),
        ..Default::default()
    };
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    assert_eq!(
        code(
            source
                .read_report(
                    "Invented Teacher",
                    "Invented Student",
                    "77001",
                    "12345",
                    NOW
                )
                .unwrap_err()
        ),
        "source_response_malformed"
    );
}

// F7/F10: the expired-session test is the TS regular expression over the raw
// text; `\b` is ASCII and `\s` is the ECMAScript set.
#[test]
fn expired_session_expression_matches_javascript_over_raw_text() {
    for html in [
        format!("<!-- <input type=password> -->{WAITING}"),
        format!("<script>x='<input data-type=password>'</script>{WAITING}"),
        format!("<input type=passwordx>{WAITING}"),
        format!("<INPUT\u{FEFF}TYPE\u{00A0}=\u{3000}'password'>{WAITING}"),
        format!("{WAITING}normal_session_error.jsp"),
        format!("{WAITING}POPUP_SESSION_ERROR.JSP"),
        format!("{WAITING}세션이 만료"),
    ] {
        assert_eq!(
            code(parse_search_form(&html, ListKind::Waiting, "99001").unwrap_err()),
            "prestudy_authentication_required"
        );
    }
    // Not matched by JavaScript: U+0085 is not `\s`, and `<inputx` has no `\b`.
    for html in [
        format!("<input type\u{0085}=password>{WAITING}"),
        format!("<inputx type=password>{WAITING}"),
    ] {
        assert!(
            parse_search_form(&html, ListKind::Waiting, "99001").is_ok(),
            "{html}"
        );
    }
}

#[test]
fn score_script_and_handler_expressions_use_ascii_word_boundary_and_js_space() {
    let card = |script: &str| {
        format!(
            "<div class=\"max-w-5xl rounded-2xl\"><h1>개념 이해</h1><div class=\"text-sm\">x</div></div><script>{script}</script>"
        )
    };
    // `\b` before `scores`: JavaScript sees a boundary after a Hangul letter.
    let report = parse_report(&card("var 한scores = [1, 2.5];")).unwrap();
    assert_eq!(report.scores, vec![1.0, 2.5]);
    // ASCII word character before `scores`: no boundary.
    assert!(
        parse_report(&card("var xscores = [1];"))
            .unwrap()
            .scores
            .is_empty()
    );
    // U+FEFF and U+00A0 are `\s`; U+0085 is not.
    assert_eq!(
        parse_report(&card("scores\u{FEFF}=\u{00A0}[3]"))
            .unwrap()
            .scores,
        vec![3.0]
    );
    assert!(
        parse_report(&card("scores\u{0085}=[3]"))
            .unwrap()
            .scores
            .is_empty()
    );
    assert_eq!(
        parse_report(&card("scores = {a:\u{00A0}4, b:5}"))
            .unwrap()
            .scores,
        vec![4.0, 5.0]
    );
    // The list handler accepts the JavaScript whitespace set around the key.
    let handler = WAITING.replace(
        "prestudy_main(12345)",
        "prestudy_main(\u{00A0}12345\u{FEFF})",
    );
    assert_eq!(
        parse_list(&handler, ListKind::Waiting, "99001").unwrap()[0].prestudy_key,
        "12345"
    );
    // Cell text collapses with the same set.
    let cell = WAITING.replace(
        "<td>Invented Student</td>",
        "<td>\u{FEFF} Invented\u{00A0}\u{3000}Student </td>",
    );
    assert_eq!(
        parse_list(&cell, ListKind::Waiting, "99001").unwrap()[0].student_label,
        "Invented Student"
    );
}

// F1: more blockText vectors (li, tr, nested script in a nested element).
#[test]
fn block_text_more_vectors() {
    let wrap = |inner: &str| {
        format!(
            "<div class=\"max-w-5xl rounded-2xl\"><h1>T</h1><h3>H</h3><div class=\"text-sm\">{inner}</div></div>"
        )
    };
    let text = |inner: &str| {
        parse_report(&wrap(inner)).unwrap().sections[0].blocks[0]
            .text
            .clone()
    };
    assert_eq!(text("<ul><li>a</li><li>b<span>c</span></li></ul>"), "a\nbc");
    assert_eq!(text("x<p>y<script>bad()</script><b>z</b></p>w"), "xyz\nw");
    assert_eq!(text("a<br>b<br><br>c"), "a\nb\nc");
    assert_eq!(text("<style>.a{}</style>only"), "only");
}

// F14: every public narrative type prints counts and lengths only.
#[test]
fn debug_of_every_public_type_leaks_no_text() {
    let secret = "SECRET-NARRATIVE";
    let row = PrestudyListRow {
        student_label: secret.into(),
        unit_label: secret.into(),
        date_literal: secret.into(),
        status_literal: secret.into(),
        prestudy_key: "12345".into(),
    };
    let summary = PrestudySummaryRow {
        student_label: secret.into(),
        unit_label: secret.into(),
        date_literal: secret.into(),
        ai_score_literal: secret.into(),
        rating_literal: secret.into(),
        report_available: true,
        video_available: false,
    };
    let lists = PrestudyLists {
        waiting: vec![row.clone()],
        completed: vec![],
        summary: vec![summary.clone()],
        teacher_judgment: None,
        observed_at: secret.into(),
        coverage: secret.into(),
    };
    let detail = PrestudyDetail {
        prestudy_key: "12345".into(),
        unit_label: Some(secret.into()),
        date_literal: Some(secret.into()),
        analysis_key: Some("55501".into()),
        encoding_result: Some(secret.into()),
    };
    let record = PrestudySourceRecord {
        prestudy_key: "12345".into(),
        unit_label: Some(secret.into()),
        date_literal: Some(secret.into()),
    };
    let block = PrestudyReportBlock {
        heading: Some(secret.into()),
        text: secret.into(),
    };
    let section = PrestudyReportSection {
        title: secret.into(),
        subtitle: Some(secret.into()),
        blocks: vec![block.clone()],
    };
    let report = PrestudyReport {
        sections: vec![section.clone()],
        scores: vec![1.0],
    };
    let observation = PrestudyReportObservation {
        prestudy_key: "12345".into(),
        source_record: record.clone(),
        sections: vec![section.clone()],
        scores: vec![1.0],
        encoding_result: Some(secret.into()),
        observed_at: secret.into(),
        coverage: secret.into(),
    };
    let view = PrestudyReportView {
        selected: true,
        requested_titles: Some(vec![secret.into()]),
        available_titles: vec![secret.into()],
        returned_titles: vec![secret.into()],
        missing_titles: vec![secret.into()],
        complete_requested_view: true,
        complete_source_report: true,
    };
    let form = PrestudySearchForm {
        req_cmd: "WebUnPreStudy".into(),
        college_key: secret.into(),
        member_type: secret.into(),
        teacher_key: secret.into(),
    };
    for printed in [
        format!("{row:?}"),
        format!("{summary:?}"),
        format!("{lists:?}"),
        format!("{detail:?}"),
        format!("{record:?}"),
        format!("{block:?}"),
        format!("{section:?}"),
        format!("{report:?}"),
        format!("{observation:?}"),
        format!("{view:?}"),
        format!("{form:?}"),
    ] {
        assert!(!printed.contains("SECRET"), "{printed}");
        assert!(!printed.contains("12345"), "{printed}");
    }
    assert!(format!("{block:?}").contains("16 units"));
}

// F6: the report read has the name guard too.
#[test]
fn report_read_refuses_blank_name_before_any_request() {
    let mut source = source();
    assert_eq!(
        code(
            source
                .read_report("Invented Teacher", "", "77001", "12345", NOW)
                .unwrap_err()
        ),
        "invalid_read_name"
    );
    assert_eq!(source.requests(), 0);
}

// F14: scores print as JavaScript prints numbers.
#[test]
fn scores_json_matches_json_stringify() {
    assert_eq!(scores_json(&[]), "[]");
    assert_eq!(
        scores_json(&[77.0, 77.5, 0.0, -0.0, -3.0, 100.0]),
        "[77,77.5,0,0,-3,100]"
    );
}

// F14: Debug prints counts and lengths only; scores print as JS numbers.
#[test]
fn debug_redacts_narrative_and_scores_serialise_as_js_numbers() {
    let observation = PrestudyReportObservation {
        prestudy_key: "12345".into(),
        source_record: PrestudySourceRecord {
            prestudy_key: "12345".into(),
            unit_label: Some("Invented Unit".into()),
            date_literal: None,
        },
        sections: vec![section("개념 이해", "teacher-only narrative")],
        scores: vec![77.0, 77.5],
        encoding_result: None,
        observed_at: NOW.into(),
        coverage: "x".into(),
    };
    let debug = format!("{observation:?}");
    assert!(!debug.contains("teacher-only narrative"), "{debug}");
    assert!(!debug.contains("개념 이해"), "{debug}");
    assert!(debug.contains("sections: 1"), "{debug}");
    assert_eq!(
        format!("{{\"scores\":{}}}", scores_json(&observation.scores)),
        "{\"scores\":[77,77.5]}"
    );
    // JSON.stringify bytes for the same payload.
    assert_eq!(scores_json(&[77.0, 80.0, 90.25]), "[77,80,90.25]");
}

// Session loan: admitted contracts serve the same reads end to end.
#[test]
fn session_port_serves_lists_and_report_through_admitted_contracts() {
    let (port, _scope) = session_port(true);
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    let lists = source
        .read_lists("Invented Teacher", "Invented Student", NOW)
        .unwrap();
    assert_eq!(lists.waiting.len(), 1);
    let observation = source
        .read_report(
            "Invented Teacher",
            "Invented Student",
            "77001",
            "12345",
            NOW,
        )
        .unwrap();
    assert_eq!(observation.source_record.prestudy_key, "12345");
}

// F12/F13: the session port sends exactly the admitted arguments, and the
// admitted POST body keeps the TS order `reqCmd, g_clg_no, g_mem_type,
// g_pri_no, student_name` (`academyHttpClient.ts:126`).
#[test]
fn session_requests_and_contract_body_order_follow_ts() {
    let (port, _scope) = session_port(true);
    let mut source = PrestudySource::new(port, "Invented Teacher", "99001");
    source
        .read_lists("Invented Teacher", "Invented Student", NOW)
        .unwrap();
    source
        .read_report(
            "Invented Teacher",
            "Invented Student",
            "77001",
            "12345",
            NOW,
        )
        .unwrap();
    let recorded = RECORDED.with(|r| r.borrow().clone());
    assert_eq!(
        recorded,
        [
            "prestudy.waiting.shell teacher=99001 []",
            "prestudy.waiting.search teacher=99001 [(\"g_clg_no\", \"88001\"), (\"g_mem_type\", \"1\"), (\"student_name\", \"Invented Student\")]",
            "prestudy.completed.shell teacher=99001 []",
            "prestudy.completed.search teacher=99001 [(\"g_clg_no\", \"88001\"), (\"g_mem_type\", \"1\"), (\"student_name\", \"Invented Student\")]",
            "prestudy.summary teacher=99001 []",
            "prestudy.detail teacher=99001 [(\"prestudy_key\", \"12345\")]",
            "prestudy.report teacher=99001 [(\"analysis_key\", \"55501\")]",
        ]
    );
    let contracts = serde_json::to_value(read_contracts()).unwrap();
    for id in ["prestudy.waiting.search", "prestudy.completed.search"] {
        let contract = contracts
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        let names: Vec<&str> = contract["body"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "reqCmd",
                "g_clg_no",
                "g_mem_type",
                "g_pri_no",
                "student_name"
            ]
        );
    }
}
