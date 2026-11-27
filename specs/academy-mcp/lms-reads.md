# Read-only LMS schedule, attendance and prestudy reads: `academy_lms_schedule_get` and `academy_prestudy_get`

## Purpose

Four LMS pages that the teacher uses each lesson day have no tool today: attendance, the study
schedule, the course study manager, and the prestudy lists and teacher summary as one read.
This contract fixes their read-only reads through the new tool `academy_lms_schedule_get` and
the consolidated `academy_prestudy_get` ([tool-surface.md](tool-surface.md)). It is relied on
by the lesson-day and prestudy-review skills and by the units that build and check the reads.

## Requirements

- R1: `academy_lms_schedule_get` takes `view` `attendance`, `study_schedule` or `course_study_manager` and sends only the GET of that view in the table "Routes"; `academy_prestudy_get` sends only the requests of its rows of that table: those of its `view` (`videos` when no `view` and no `reportId` is given), and, with `reportId`, those of the report row once for that key. (no test yet; observed by the unit that builds the LMS reads)
- R2: The reads send no POST, other than the prestudy list search form that the table names, which changes no Academy data; no request leaves the table. (no test yet; observed by the unit that builds the LMS reads)
- R3: Before its first LMS request a call takes the shared account lock of `docs/academy-account-lock.md` with `/usr/bin/flock --nonblock` and holds it until its last; when the lock is busy the call ends with a tool error carrying `owner_account_busy`, and 0 HTTP requests are sent. (no test yet; observed by the unit that builds the LMS reads)
- R4: A redirect to the login page, the login page itself, a session-error page (`normal_session_error.jsp`, `popup_session_error.jsp`), or a page with a password input refuses the call with `authentication_required`; no row of such a page is answered and no login is retried within the call (fail closed). (no test yet; observed by the unit that builds the LMS reads)
- R5: A page whose scope fields (the teacher key and, where the page has them, the date or period) differ from those requested refuses with `scope_conflict`; a page without the form or table the read expects refuses with `source_drift`. (no test yet; observed by the unit that builds the LMS reads)
- R6: I1 `write_forbidden`: under a mock transport that answers `write_forbidden` to every request not in the table "Routes", every view of both tools on the synthetic fixtures completes, and the transport records 0 `write_forbidden` requests. (no test yet; observed by the unit that builds the LMS reads)
- R7: A date that is not a calendar date written `YYYY-MM-DD`, or a period over 31 days, refuses with an invalid input before any request. (no test yet; observed by the unit that builds the LMS reads)
- R8: A page larger than 2 MiB, or one that yields more than 1000 rows, refuses with `source_rejected`. (no test yet; observed by the unit that builds the LMS reads)
- R9: No video URL and no cookie is answered or logged; an answer holds the page's labels, dates, keys and status literals only. (no test yet; observed by the unit that builds the LMS reads)

## Interface

### Routes

| Tool and view | Request |
| --- | --- |
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

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/lms-reads.test.ts`, with synthetic pages
of invented values only.

## Not included

- Marking attendance, changing a schedule, or checking a prestudy video: these are writes.
- The AI report of one prestudy video beyond what `includeReports` and `reportId` return.
- Coverage beyond the one page each view reads.
