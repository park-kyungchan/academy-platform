# Read-only LMS schedule, attendance, test result and prestudy reads: `academy_lms_schedule_get` and `academy_prestudy_get`

## Purpose

Four LMS pages that the teacher uses each lesson day have no tool today: attendance, the study
schedule, the course study manager, and the prestudy lists and teacher summary as one read.
This contract fixes their read-only reads through the new tool `academy_lms_schedule_get` and
the consolidated `academy_prestudy_get` ([tool-surface.md](tool-surface.md)). It also fixes a fourth
view of `academy_lms_schedule_get`, `test_results`: the lists of a student's DA, FA and NA attempts and of the
new-student diagnosis results, whose rows carry the keys (`paperNo`, `testingNo`, `classNo`, `applyCode`) that the batch reports of
`academy_official_report_pdf_get` take ([direct-key-lookup.md](direct-key-lookup.md) R6, R8) and that no other read lists. It is relied on
by the lesson-day and prestudy-review skills and by the units that build and check the reads.

## Requirements

- R1: `academy_lms_schedule_get` takes `view` `attendance`, `study_schedule`, `course_study_manager` or `test_results` and sends only the requests of that view's rows in the table "Routes": `test_results` takes `family` (`da`, `fa`, `na` or `new_diag`) and sends, for `da`, `fa` and `na`, the page's own search POST of the family (`displayName` and `period` required, no `sortDate`, the name resolved by the backend's existing student-by-name rule, that is the launch's own name pair, else the name search row of the table, sent before the search) and, for `new_diag`, the one GET of the diagnosis result list (`sortDate` one of the page's values `1`, `2`, `3`, `11` to `16`, or none; no `period`; a `displayName` is only a literal filter on the applicant's name: no name search and no student key); it answers the first page only; `study_schedule` with all of `gradeNo`, `groupKey`, `studentKey` and `courseKey` sends the page's own search POST, with any of them missing it sends the start page GET with the given filters in its query and answers status `filters_required`, the options of the four selects and rows `[]`; `course_study_manager` always sends the `search=1` GET (a filter not given is `grade_no=0`, `semester_no=0`, `level_no=0`, `status=-1`); `academy_prestudy_get` sends only the requests of its rows of that table: those of its `view` (`videos` when no `view` and no `reportId` is given), and, with `reportId`, those of the report row once for that key. Tests: `lms_reads_r1_schedule_views_send_one_get_of_their_own_page`, `lms_reads_r1_schedule_filters_use_the_pages_own_parameter_names`, `lms_reads_r1_schedule_rows_and_filter_options_are_parsed_from_the_pinned_shapes`, `lms_reads_r1_prestudy_views_send_only_the_requests_of_their_own_rows`, `lms_reads_r1_report_id_reads_the_report_row_once_for_that_key`, `lms_reads_r1_the_bridge_forwards_the_view_and_pairs_include_reports_with_report_id`, `lms_reads_study_schedule_search_posts_user_search_with_the_four_filters`, `lms_reads_study_schedule_without_all_filters_answers_filters_required_with_options`, `lms_reads_course_study_manager_search_get_parses_course_rows`, `lms_reads_test_results_fa_posts_the_pages_search_and_reads_inline_rows`, `lms_reads_test_results_new_diag_reads_paper_and_apply_code`.
- R2: The reads send 0 write requests: every request is a GET or a POST search form whose route is classed read by its named route and operation, never by its method alone (as [tool-surface.md](tool-surface.md) "Request classes" states); the only POSTs are those the table names (the login exchange, the student name search, the study schedule `UserSearch` search, the test results search of `TestPageListServlet` `UserBySearchTestResult` for `da`, `fa` and `na`, and the prestudy list search forms), and no request leaves the table; no `onclick` handler of a result page (`editDate`, `updateScore`, `editState`, `testListView`) is requested. The thirteen routes of the registry `lmsRouteRegistry.ts` for these reads (the nine of the other views and the four of `test_results`) are classed read. Tests: `lms_reads_r2_all_views_send_only_gets_and_the_named_search_posts_of_read_routes`, `lms_reads_r2_the_nine_read_routes_are_classed_read_and_a_neighbouring_route_is_write`, `lms_reads_test_results_routes_are_classed_read_and_a_neighbouring_route_is_write`, `lms_reads_study_schedule_rowspan_cells_fill_every_column`.
- R3: Before its first LMS request a call takes the shared account lock of `docs/academy-account-lock.md`, waiting up to 45 seconds, and holds it until its last request; when the lock is still held after 45 seconds the call ends with a tool error carrying `owner_account_busy`, and 0 HTTP requests are sent. Tests: `lms_reads_r3_a_held_account_lock_ends_each_call_with_owner_account_busy_and_sends_no_request`.
- R4: A redirect to the login page, the login page itself, a session-error page (`normal_session_error.jsp`, `popup_session_error.jsp`), or a page with a password input refuses the call with `authentication_required`; no row of such a page is answered and no login is retried within the call (fail closed). Tests: `lms_reads_r4_schedule_views_refuse_with_authentication_required_and_0_login_retries`, `lms_reads_r4_prestudy_views_refuse_with_authentication_required_and_0_login_retries`, `lms_reads_r4_a_session_page_after_the_search_form_post_refuses_too`, `lms_reads_r4_the_parser_keeps_its_own_authentication_throw`.
- R5: A page whose scope fields (the teacher key and, where the page has them, the date or period) differ from those requested refuses with `scope_conflict`; a page without the form the read expects refuses with `source_drift`. For `test_results` the `userForm` `pri_no` of the answered page must be the teacher's key (the page of another teacher refuses with `scope_conflict`, a page without it with `source_drift`), and the rows are the inline literal `var EXTERN_DIALOG = { testResultList: JSON.parse('<JSON array>') }`: the text between the quotes is decoded as a JavaScript single-quoted string literal (the escapes `\'`, `\"`, `\\`, `\n` and `\uXXXX` turned back into characters, nothing evaluated) and then parsed as JSON, and a missing or malformed literal, or JSON that is not an array of objects, refuses with `source_drift`. For `study_schedule` and `course_study_manager` the `reqCmd` input of `form#form1` must exist (the start page holds it without a value; its value is not compared), and the list `div.content-list` holds either `table.content-table` or only the image `firstpage.png` (the start page) or `no_result.png` (a search without a result): the two images answer rows `[]` and the options of the page's selects (status `observed`, or `filters_required` as R1 says); a list with neither image nor table refuses with `source_drift`. Header `colspan` and body `rowspan` and `colspan` cells are expanded so that each row has every column (a full `study_schedule` row has 13). Tests: `lms_reads_r5_attendance_scope_fields_and_shape`, `lms_reads_r5_study_schedule_and_course_study_manager_scope_fields_and_shape`, `lms_reads_r5_prestudy_search_form_scope_and_shape`, `lms_reads_r5_prestudy_summary_page_without_its_table_is_source_drift`, `lms_reads_schedule_start_and_no_result_pages_answer_zero_rows`, `lms_reads_test_results_decodes_the_js_string_literal_before_json`, `lms_reads_test_results_omits_rows_of_another_student`.
- R6: I1 `write_forbidden`: under a mock transport that answers `write_forbidden` to every request whose route is classed write, or that is not in the table "Routes", every view of both tools (the four views of `academy_lms_schedule_get` included) on the synthetic fixtures completes, and the transport records 0 `write_forbidden` requests. Tests: `lms_reads_r6_every_view_of_both_tools_completes_with_0_forbidden_requests`, `lms_reads_r6_the_transport_answers_write_forbidden_outside_the_table`.
- R7: A date that is not a calendar date written `YYYY-MM-DD`, or a period over 31 days (over 92 days for `test_results` only, whose search is by period and which has no month rule), refuses with an invalid input before any request; so does a `test_results` call with no `family`, a `da`, `fa` or `na` call without `displayName` or without `period` or with a `sortDate`, and a `new_diag` call with a `period` or a `sortDate` that is not one of the page's values. Tests: `lms_reads_r7_invalid_dates_and_periods_refuse_before_any_request`, `lms_reads_r7_a_real_calendar_date_and_a_period_of_31_days_in_one_month_are_accepted`, `lms_reads_r7_each_view_refuses_the_members_of_the_other_views`, `lms_reads_r7_the_client_refuses_the_same_input_before_a_request`, `lms_reads_test_results_fa_posts_the_pages_search_and_reads_inline_rows`.
- R8: A page larger than 2 MiB, or one that yields more than 1000 rows (for `test_results`, a list of more than 1000 inline rows), refuses with `source_rejected`. Tests: `lms_reads_test_results_decodes_the_js_string_literal_before_json`, `lms_reads_r8_schedule_pages_over_2_MiB_are_source_rejected`, `lms_reads_r8_schedule_pages_over_1000_rows_are_source_rejected`, `lms_reads_r8_prestudy_pages_over_2_MiB_or_1000_rows_are_source_rejected`.
- R9: No video URL and no cookie is answered or logged; an answer holds the page's labels, dates, keys and status literals only. Tests: `lms_reads_r9_answers_and_failures_hold_no_video_url_and_no_cookie`.

## Interface

### Routes

| Tool and view | Request |
| --- | --- |
| every call of both tools, before its other rows | the login exchange of the server's session, as the tools send it today |
| every call of `academy_prestudy_get`, after the login | the student name search of the old `academy_prestudy_get`: `POST /servlet/controller.dailyzerotest.DailyZeroTestServlet?reqCmd=DtResultSearch` with body `sort_date` and `stu_name`, sent again for each period selector of the page while no student is found |
| `academy_lms_schedule_get(view:'attendance')` | `GET /servlet/controller.cct.tutor.AttendanceServlet?p_process=Main` |
| `academy_lms_schedule_get(view:'study_schedule')` with `gradeNo`, `groupKey`, `studentKey` and `courseKey` | `POST /servlet/controller.coursemanage.CourseManageServlet`, form body `reqCmd=UserSearch&grade_no=<gradeNo>&cls_no=<groupKey>&student_no=<studentKey>&course=<courseKey>` (the page's own `onSearch`; the existing roster route `UserSearchServlet` `p_process=Main` is another route) |
| `academy_lms_schedule_get(view:'study_schedule')` with any of the four missing | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=StudySchedule` with the given filters in the query; answers `filters_required` |
| `academy_lms_schedule_get(view:'course_study_manager')` | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=CourseStudyManager&grade_no=<g>&semester_no=<s>&level_no=<l>&status=<t>&search=1`, an unset filter being `0`, `0`, `0`, `-1` |
| `academy_lms_schedule_get(view:'test_results', family:'da'\|'fa'\|'na')`, after the login and, when `displayName` is not a pair of the launch, after the student name search (the `DtResultSearch` row above, sent as for `academy_prestudy_get`) | `POST /servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&ass_no=<a>`, form body `clg_no=<the owner's college key>&cls_no=0&p_pageno=1&<flag>=<a>&stu_name=<displayName>&chioce_cls=0&sort_date1=<from>&sort_date2=<through>`, `(a, flag)` being `(1000, check_da_test)` for `da`, `(1001, check_fa_test)` for `fa`, `(1002, check_na_test)` for `na`; the page's own search form, read as the POST of `UserSearch` is |
| `academy_lms_schedule_get(view:'test_results', family:'new_diag')` | `GET /servlet/controller.tutor.diag.DiagnostManageServlet?reqCmd=TestResultList`, with `&sortDate=<v>` when `sortDate` is given (the `sortDate` select of the page's `form1`: `1`, `2`, `3`, `11` to `16`) |
| `academy_prestudy_get(view:'videos')` | the requests of the old `academy_prestudy_get`, in its order: `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=WebUnPreStudy` and the `POST` of that page's `form1`; `GET ...CourseManageServlet?reqCmd=WebCtPreStudy` and the `POST` of that page's `form1`; `GET ...CourseManageServlet?reqCmd=TeacherPrestudySummary` |
| `academy_prestudy_get(view:'incomplete')` | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=WebUnPreStudy` and the `POST` of that page's `form1` |
| `academy_prestudy_get(view:'teacher_summary')` | `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=TeacherPrestudySummary` |
| `academy_prestudy_get(includeReports:true, reportId)` | the requests of the old `academy_prestudy_report_get`: `GET /servlet/controller.coursemanage.CourseManageServlet?reqCmd=WebPreStudy&prestudy_key=<reportId>`, then `GET /app-ui/solution_common/prestudy/prestudy_ai_report.jsp?analysis_key=<key>` with the analysis key that the first page gives |

A `form1` POST, the `UserSearch` POST and the `UserBySearchTestResult` POST are the lists' search forms: they change no Academy data. `includeReports:true` without `reportId`, or `reportId` without `includeReports:true`, refuses with an invalid input before any request. The `view` rows answer what the old tool of the same requests answered, as [tool-surface.md](tool-surface.md) R4 requires.

The query parameters of a view beyond the command (a date, a period, a class) are those the
page's own form carries; a hidden scope field is read back from the live page, never built
from the caller's input.

### Answer of view `test_results`

The observation holds `view` `test_results`, `family`, `page` 1, `pageRowCount` (the rows of the page), `rows`, `observedAt`, and `coverage` `first_page_of_the_search_only_not_all_history`; there is no paging beyond the first page.

| Family | Members of the answer | A row |
| --- | --- | --- |
| `da`, `fa`, `na` | `student` `{studentId, displayName}`, `period`, `otherStudentRowsOmitted` | `{paperNo, testingNo, studentKey, classNo, title, applyDate, score}` from the page's `pNo`, `testingNo`, `priNo`, `clsNo`, `testingName`, `applyDate` (`MM-DD HH:MM`) and `score`; `classNo` is 0 on every live row |
| `new_diag` | `sortDate` when given | `{paperNo, applyCode, studentName, applyDate, title, score}` from the page's `pNo`, `applyCode`, `studentName`, `applyDate` (`YYYY.MM.DD`), `testName` and `score` |

For `da`, `fa` and `na` only the rows whose `priNo` is the resolved student's key are answered: the page's `stu_name` is a name search that can also list a student with a longer name, so each other row is left out and counted in `otherStudentRowsOmitted`, never returned. For `new_diag` the rows whose `studentName` equals the given `displayName` are kept (a name that no row carries answers rows `[]`, no `displayName` keeps every row of the page). A row's keys (`pNo`, `testingNo`, `priNo`, `clsNo`) are integers written as digits (`clsNo` may be 0), and an `applyCode` is 1 to 64 characters with no control character; a row of the answered student or applicant that does not keep that rule refuses the read with `source_drift`. The keys feed `academy_official_report_pdf_get`: `params.testingNo` and `params.classNo` (0 accepted) of `formative`, `diag`, `ganga` and `na_score`, and `params.paperNo` and `params.applyCode` of `new_diag` ([direct-key-lookup.md](direct-key-lookup.md) R6, R8).

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
| Period of one call | 31 days (92 days for `test_results` only, which has no month rule) |
| Wait for the account lock | 45 seconds |

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/lms-reads.test.ts`, with synthetic pages
of invented values only.

## Not included

- Marking attendance, changing a schedule, or checking a prestudy video: these are writes.
- The AI report of one prestudy video beyond what `includeReports` and `reportId` return.
- Coverage beyond the one page each view reads; `test_results` reads page 1 of its search only.
- The `SmartBookResultSearch` list (the `bookCode` of a SmartBook report is not discovered by this read).
