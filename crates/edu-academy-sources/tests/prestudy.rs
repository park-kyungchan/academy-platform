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

#[derive(Clone, Default)]
struct FakePort {
    calls: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    detail: Option<String>,
    report: Option<String>,
}
impl PrestudyPort for FakePort {
    fn shell(&mut self, kind: ListKind) -> edu_academy_sources::data::Result<Response> {
        self.calls
            .borrow_mut()
            .push(format!("shell:{}", kind.cmd()));
        Ok(response(match kind {
            ListKind::Waiting => WAITING,
            ListKind::Completed => COMPLETED,
        }))
    }
    fn search(
        &mut self,
        kind: ListKind,
        _form: &PrestudySearchForm,
        _student_name: &str,
    ) -> edu_academy_sources::data::Result<Response> {
        self.calls
            .borrow_mut()
            .push(format!("search:{}", kind.cmd()));
        Ok(response(match kind {
            ListKind::Waiting => WAITING,
            ListKind::Completed => COMPLETED,
        }))
    }
    fn summary(&mut self) -> edu_academy_sources::data::Result<Response> {
        self.calls.borrow_mut().push("summary".into());
        Ok(response(SUMMARY))
    }
    fn detail(&mut self, _prestudy_key: &str) -> edu_academy_sources::data::Result<Response> {
        self.calls.borrow_mut().push("detail".into());
        Ok(response(self.detail.as_deref().unwrap_or(DETAIL)))
    }
    fn report(&mut self, _analysis_key: &str) -> edu_academy_sources::data::Result<Response> {
        self.calls.borrow_mut().push("report".into());
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
    assert_eq!(
        rows_for_student(&rows, "Invented Student"),
        vec![PrestudyListRow {
            student_label: "Invented Student".into(),
            unit_label: "9-1 Fictional Unit (기본)(ver. 1)".into(),
            date_literal: "40.01.02 03:04".into(),
            status_literal: "OK".into(),
            prestudy_key: "12345".into(),
        }]
    );
    assert!(rows_for_student(&rows, "Invented").is_empty());
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
    assert_eq!(rows_for_student(&rows, "Invented Student").len(), 1);
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
        .read_report("Invented Teacher", "77001", "12345", NOW)
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
        err_code(port.shell(ListKind::Waiting)),
        "prestudy_source_port_unavailable"
    );
    drop(scope);
}

#[test]
fn error_prestudy_report_source_port_unavailable() {
    let (mut port, _scope) = session_port(false);
    assert_eq!(
        err_code(port.report("55501")),
        "prestudy_report_source_port_unavailable"
    );
    assert_eq!(
        err_code(port.detail("12345")),
        "prestudy_report_source_port_unavailable"
    );
}

#[test]
fn error_prestudy_report_principal_rejected() {
    let mut source = source();
    assert_eq!(
        code(
            source
                .read_report("Other Teacher", "77001", "12345", NOW)
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
                .read_report("Invented Teacher", "77001", "12345", NOW)
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

struct FakeReadPort {
    admission: TransportAdmission,
}
impl ReadPort for FakeReadPort {
    fn transport_admission(&self) -> &TransportAdmission {
        &self.admission
    }
    fn read(
        &mut self,
        _scope: &TransportScope,
        operation: &str,
        _arguments: &[(String, String)],
    ) -> edu_academy_sources::data::Result<Response> {
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
        .read_report("Invented Teacher", "77001", "12345", NOW)
        .unwrap();
    assert_eq!(observation.source_record.prestudy_key, "12345");
}
