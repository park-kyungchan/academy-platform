# Read-only LMS schedule, attendance and prestudy reads: `academy_lms_schedule_get` and `academy_prestudy_get`

## Purpose

Four LMS pages that the teacher uses each lesson day have no tool today: attendance, the study
schedule, the course study manager, and the prestudy lists and teacher summary as one read.
This contract fixes their read-only reads through the new tool `academy_lms_schedule_get` and
the consolidated `academy_prestudy_get` ([tool-surface.md](tool-surface.md)). It is relied on
by the lesson-day and prestudy-review skills and by the units that build and check the reads.

## Requirements

- R1: `academy_lms_schedule_get` takes `view` `attendance`, `study_schedule` or `course_study_manager` and sends only the requests of that view's rows in the table "Routes"; `academy_prestudy_get` sends only the requests of its rows of that table: those of its `view` (`videos` when no `view` and no `reportId` is given), and, with `reportId`, those of the report row once for that key. Tests: `lms_reads_r1_schedule_views_send_one_get_of_their_own_page`, `lms_reads_r1_schedule_filters_use_the_pages_own_parameter_names`, `lms_reads_r1_schedule_rows_and_filter_options_are_parsed_from_the_pinned_shapes`, `lms_reads_r1_prestudy_views_send_only_the_requests_of_their_own_rows`, `lms_reads_r1_report_id_reads_the_report_row_once_for_that_key`, `lms_reads_r1_the_bridge_forwards_the_view_and_pairs_include_reports_with_report_id`.
- R2: The reads send 0 write requests: every request is a GET or a POST search form whose route is classed read by its named route and operation, never by its method alone (as [tool-surface.md](tool-surface.md) "Request classes" states); the only POSTs are those the table names (the login exchange, the student name search and the prestudy list search forms), and no request leaves the table. Tests: `lms_reads_r2_all_views_send_only_gets_and_the_named_search_posts_of_read_routes`, `lms_reads_r2_the_eight_read_routes_are_classed_read_and_a_neighbouring_route_is_write`.
- R3: Before its first LMS request a call takes the shared account lock of `docs/academy-account-lock.md`, waiting up to 45 seconds, and holds it until its last request; when the lock is still held after 45 seconds the call ends with a tool error carrying `owner_account_busy`, and 0 HTTP requests are sent. Tests: `lms_reads_r3_a_held_account_lock_ends_each_call_with_owner_account_busy_and_sends_no_request`.
- R4: A redirect to the login page, the login page itself, a session-error page (`normal_session_error.jsp`, `popup_session_error.jsp`), or a page with a password input refuses the call with `authentication_required`; no row of such a page is answered and no login is retried within the call (fail closed). Tests: `lms_reads_r4_schedule_views_refuse_with_authentication_required_and_0_login_retries`, `lms_reads_r4_prestudy_views_refuse_with_authentication_required_and_0_login_retries`, `lms_reads_r4_a_session_page_after_the_search_form_post_refuses_too`, `lms_reads_r4_the_parser_keeps_its_own_authentication_throw`.
- R5: A page whose scope fields (the teacher key and, where the page has them, the date or period) differ from those requested refuses with `scope_conflict`; a page without the form or table the read expects refuses with `source_drift`. Tests: `lms_reads_r5_attendance_scope_fields_and_shape`, `lms_reads_r5_study_schedule_and_course_study_manager_scope_fields_and_shape`, `lms_reads_r5_prestudy_search_form_scope_and_shape`, `lms_reads_r5_prestudy_summary_page_without_its_table_is_source_drift`.
- R6: I1 `write_forbidden`: under a mock transport that answers `write_forbidden` to every request whose route is classed write, or that is not in the table "Routes", every view of both tools on the synthetic fixtures completes, and the transport records 0 `write_forbidden` requests. Tests: `lms_reads_r6_every_view_of_both_tools_completes_with_0_forbidden_requests`, `lms_reads_r6_the_transport_answers_write_forbidden_outside_the_table`.
- R7: A date that is not a calendar date written `YYYY-MM-DD`, or a period over 31 days, refuses with an invalid input before any request. Tests: `lms_reads_r7_invalid_dates_and_periods_refuse_before_any_request`, `lms_reads_r7_a_real_calendar_date_and_a_period_of_31_days_in_one_month_are_accepted`, `lms_reads_r7_each_view_refuses_the_members_of_the_other_views`, `lms_reads_r7_the_client_refuses_the_same_input_before_a_request`.
- R8: A page larger than 2 MiB, or one that yields more than 1000 rows, refuses with `source_rejected`. Tests: `lms_reads_r8_schedule_pages_over_2_MiB_are_source_rejected`, `lms_reads_r8_schedule_pages_over_1000_rows_are_source_rejected`, `lms_reads_r8_prestudy_pages_over_2_MiB_or_1000_rows_are_source_rejected`.
- R9: No video URL and no cookie is answered or logged; an answer holds the page's labels, dates, keys and status literals only. Tests: `lms_reads_r9_answers_and_failures_hold_no_video_url_and_no_cookie`.

## Interface

### Routes

| Tool and view | Request |
| --- | --- |
| every call of both tools, before its other rows | the login exchange of the server's session, as the tools send it today |
| every call of `academy_prestudy_get`, after the login | the student name search of the old `academy_prestudy_get`: `POST /servlet/controller.dailyzerotest.DailyZeroTestServlet?reqCmd=DtResultSearch` with body `sort_date` and `stu_name`, sent again for each period selector of the page while no student is found |
| `academy_lms_schedule_get(view:'attendance')` | `GET /servlet/controller.cct.tutor.AttendanceServlet?p_process=Main` |
| `academy_lms_schedule_get(view:'study_schedule')` | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=StudySchedule` |
| `academy_lms_schedule_get(view:'course_study_manager')` | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=CourseStudyManager` |
| `academy_prestudy_get(view:'videos')` | the requests of the old `academy_prestudy_get`, in its order: `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=WebUnPreStudy` and the `POST` of that page's `form1`; `GET ...CourseManageServlet?reqCmd=WebCtPreStudy` and the `POST` of that page's `form1`; `GET ...CourseManageServlet?reqCmd=TeacherPrestudySummary` |
| `academy_prestudy_get(view:'incomplete')` | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=WebUnPreStudy` and the `POST` of that page's `form1` |
| `academy_prestudy_get(view:'teacher_summary')` | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=TeacherPrestudySummary` |
| `academy_prestudy_get(includeReports:true, reportId)` | the requests of the old `academy_prestudy_report_get`: `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=WebPreStudy&prestudy_key=<reportId>`, then `GET /app-ui/solution_common/prestudy/prestudy_ai_report.jsp?analysis_key=<key>` with the analysis key that the first page gives |

A `form1` POST is the list's search form: it changes no Academy data. `includeReports:true` without `reportId`, or `reportId` without `includeReports:true`, refuses with an invalid input before any request. The `view` rows answer what the old tool of the same requests answered, as [tool-surface.md](tool-surface.md) R4 requires.

The query parameters of a view beyond the command (a date, a period, a class) are those the
page's own form carries; a hidden scope field is read back from the live page, never built
from the caller's input.

### Codes

| Code | When |
| --- | --- |
| `owner_account_busy` | R3 |
| `authentication_required` | R4 |
| `scope_conflict`, `source_drift` | R5 |
| `source_rejected` | R8 |

### Limits

| Limit | Value |
| --- | --- |
| One page | 2 MiB |
| Rows of one page | 1000 |
| Period of one call | 31 days |
| Wait for the account lock | 45 seconds |

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/lms-reads.test.ts`, with synthetic pages
of invented values only.

## Not included

- Marking attendance, changing a schedule, or checking a prestudy video: these are writes.
- The AI report of one prestudy video beyond what `includeReports` and `reportId` return.
- Coverage beyond the one page each view reads.
