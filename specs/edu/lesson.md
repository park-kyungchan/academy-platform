# The lesson command `edu lesson`

## Purpose

`edu lesson` answers a lesson session's DailyRecord reads from the Ontology read store of
the domain `academy_read`, and captures a date from the LMS with the family `dailyrecord`
when the store does not hold it yet or when asked to. It is a one-shot command: one run, one
answer, nothing on standard error. It is meant to replace the TypeScript server's
`academy_dailyrecord_get` for these reads. This specification describes the command as
built at commit 857caff0 (crate `crates/edu`). It is relied on by the lesson sessions that
call it, by the tests in `crates/edu/tests/lesson.rs`, and by every change of the crate.

## Requirements

Output

- R1: Line 1 of every answer on standard output is one JSON object, and an answer whose `status` is `ok` ends with exit code 0. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`, `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`.
- R2: A refusal is the single line `{"status":"<code>"}` with exit code 2 and no further line; only `lesson_student_ambiguous` adds a member, `count`. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`, `i0_a_malformed_or_future_date_is_refused_before_any_request_or_engine_call`, `the_configuration_file_is_private_and_exact_and_a_failed_index_shows_no_table`.
- R3: The command writes nothing on standard error, whether it answers or refuses. Tests: `the_configuration_file_is_private_and_exact_and_a_failed_index_shows_no_table`.
- R4: No session cookie and no password appear on standard output, in the run log or in the raw answers kept under `rawDir`. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`, `a3_day_twice_then_record_by_a_korean_name_then_history`, `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`.
- R5: Line 1 holds no student name, no record text and no LMS user name. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`, `i3_refusals_carry_no_cookie_password_name_or_text`.
- R6: The run log holds no student name, record text, student id or lesson date. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`.

Words

- R7: The words are one of the four forms of the Interface; an unknown verb, a missing or extra positional word, a flag the verb does not take, a flag given twice, any other word beginning with `--`, `--since` without a value, a date given twice, more than 31 dates, or words without `lesson` refuse with `lesson_usage_invalid`. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`.
- R8: A date, positional or the value of `--since`, that is not a calendar date that exists, written `YYYY-MM-DD` with its zeros, refuses with `lesson_date_invalid`. Tests: `i0_a_malformed_or_future_date_is_refused_before_any_request_or_engine_call`.
- R9: A date of `day`, `record` or `refresh` after today refuses with `lesson_date_in_future`; the value of `--since` is not compared with today. Tests: `i0_a_malformed_or_future_date_is_refused_before_any_request_or_engine_call`.
- R10: Today is the calendar date in Asia/Seoul (UTC+9, no daylight saving time). Tests: `seoul_dates_follow_the_calendar`.
- R11: A refusal of the words (R7, R8, R9, R12, R13) is the answer even when the configuration or the store would refuse, and such a run makes no engine call and no LMS request. Tests: `i0_a_malformed_or_future_date_is_refused_before_any_request_or_engine_call`.
- R12: A student word of ASCII digits only is a student id and has 1 to 12 digits; any other student word is a name of 1 to 200 bytes without a control character; any other word refuses with `lesson_student_invalid`. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`.
- R13: `history` takes a student id only: a name refuses with `lesson_student_invalid`. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`.

Configuration and run log

- R14: Without `--config` the configuration file is `/etc/education-platform/lesson.json`. (no test yet)
- R15: The configuration file is refused with `lesson_config_refused` when it is absent, a symbolic link or not a regular file, not owned by the caller's effective user, writable by group or others, larger than 64 KiB, not one JSON object of the members of the Interface (a member unknown, or a required one missing or of another type), of a `schemaVersion` other than `edu.lesson-config/1`, holds a path that is not absolute, or a `lockWaitSeconds` over 1 hour. Tests: `the_configuration_file_is_private_and_exact_and_a_failed_index_shows_no_table`.
- R16: `rawDir` may be absent, and then no raw answer is kept; `lockWaitSeconds` may be absent and is then 30 seconds. Tests: `the_configuration_file_is_private_and_exact_and_a_failed_index_shows_no_table`.
- R17: Every run whose configuration was accepted, refused or not, appends one line to `lesson-runs.jsonl` in the directory of the configured store, a file of mode 0600, with exactly the members `time`, `verb`, `dates`, `login`, `requests`, `lock_wait_seconds` and `status`. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`, `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`, `i3_refusals_carry_no_cookie_password_name_or_text`, `a_caller_that_is_not_root_is_refused_by_the_engine_rule`, `a_busy_account_lock_is_retried_for_the_configured_seconds`.
- R18: When that line cannot be appended (also when the run log is not a regular file of the caller, is readable or writable by group or others, or has a second link), the answer is `{"status":"lesson_run_log_failed"}` with exit code 2. (no test yet)

Store

- R19: A caller whose real or effective user id is not 0 is refused with `principal_not_trusted` before the owner root, the secret file or the spool is touched and before any LMS request. Tests: `a_caller_that_is_not_root_is_refused_by_the_engine_rule`.
- R20: A store path that does not name a regular file, an engine answer that is not one JSON object, a `describe` refused without a code, or a `search` or `render` the engine does not answer, refuses with `lesson_store_unavailable`. (no test yet)
- R21: When the store's installed `academy_read` domain is absent or its digest differs from that of the read schema the command embeds, with the schema's spool root replaced by the configured spool, every verb refuses with `lesson_schema_mismatch` before any LMS request. Tests: `i5_a_store_with_another_schema_is_refused_before_any_request`, `i0_a_malformed_or_future_date_is_refused_before_any_request_or_engine_call`.
- R22: The command never installs or changes a schema in the store. (no test yet)
- R23: Every run that passes R21 indexes the sources `lesson_record` and `lesson_day` before it reads, and again after a capture; when an index does not end `ok` for both sources the run refuses with `lesson_index_failed` and prints no table. Tests: `the_configuration_file_is_private_and_exact_and_a_failed_index_shows_no_table`.

Capture and the account lock

- R24: `day` and `record` capture their date only when the store holds no `lesson_day` of that date, or with `--refresh`; a date the store holds is answered without any LMS request, with `login` `none` and `requests` 0. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`, `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`.
- R25: `history` never contacts the LMS. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`.
- R26: A capture logs in once for all its dates and sends one page request per date. Tests: `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`, `a3_day_twice_then_record_by_a_korean_name_then_history`.
- R27: `login` is `new` when the run sent an LMS request, else `none`, and `requests` counts every LMS request of the run, the login exchanges included. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`, `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`.
- R28: A refused capture ends with the code of its first refused date, as the family, the session or the owner root gives it (for example `capture_day_date_conflict`, `native_owner_authentication_failed`), and the dates before it stay published. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`.
- R29: A capture holds the shared account lock of `docs/academy-account-lock.md` from its login to its end; while another holds it the run tries again every quarter of a second until `lockWaitSeconds` have passed, then refuses with `owner_account_busy` without an LMS request. Tests: `a_busy_account_lock_is_retried_for_the_configured_seconds`.
- R30: A run that waited for the account lock reports the whole seconds it waited, at least 1, as `lock_wait_seconds` in the run log, and also in line 1 when it answers `ok`; a refused run that waited (for example `owner_account_busy`) has them in the run log only, because its line 1 is the code alone (R2); a run that did not wait reports 0. Tests: `a_busy_account_lock_is_retried_for_the_configured_seconds`, `a3_day_twice_then_record_by_a_korean_name_then_history`.
- R31: The password is the value of the configured secret file's line `<secretKey>=<value>` and the user name is the identity file's `sourceUsername`, both read when the session opens; neither is written to the spool, the store, the raw answers or the run log. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`, `a3_day_twice_then_record_by_a_korean_name_then_history`.
- R32: A captured date is written to the spool by the family `dailyrecord` as `docs/academy-capture.md` (section "The lesson record's lines") states: one `lesson_day` line per date with the members of that section, and, with `rawDir`, the raw answers under it. Tests: `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`.
- R33: An error of the capture whose code is not a word of 1 to 64 characters of `a-z`, `0-9` and `_` refuses with `lesson_capture_failed`. (no test yet)
- R34: A `roster.json` of the owner root that matches its digest but is not a roster refuses with `lesson_roster_refused`. (no test yet)
- R35: When the store holds no `lesson_day` of a date after its capture and index, the run refuses with `lesson_coverage_missing`. (no test yet)

Answers

- R36: The line 1 of `day` has exactly the members `status`, `date`, `captured_at`, `age_seconds`, `rows`, `stored`, `outside_roster`, `cut_fields`, `login`, `requests` and `lock_wait_seconds`, and the lines after it are the view `daily_records` of the date as the engine renders it. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`.
- R37: `captured_at` is the time of the date's `lesson_day` in seconds since the epoch and `age_seconds` the seconds from it to the run's time; a capture moves `captured_at` forward and a read without capture leaves it as it was. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`, `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`.
- R38: The line 1 of `record` has the members of `day` and `student_id`, and the lines after it are the view `daily_record` of the date and that student id, with no row of another student. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`.
- R39: A name is matched with the `student_name` of the date's rows; when no row has it the run refuses with `lesson_student_not_found`. Tests: `i3_refusals_carry_no_cookie_password_name_or_text`.
- R40: A name on rows of two or more student ids refuses with `{"status":"lesson_student_ambiguous","count":<n>}`, `<n>` the number of student ids; rows of one student in two courses are one student. Tests: `a_name_on_two_students_is_refused_with_their_count_and_exact_prints_members`.
- R41: With `--exact`, the lines after line 1 are the student's rows of the date in the page's order, each one JSON object of the row's members that hold a value, with their exact text, and `observed_at`; a member the row lacks is left out. Tests: `a_name_on_two_students_is_refused_with_their_count_and_exact_prints_members`.
- R42: `history` answers the student id's rows whose `lesson_date` is on or after `since` (without `--since`, 56 days before today), at most 24: line 1 has exactly `status`, `since`, `rows` (their number), `captured_at` (the oldest time of those rows) and `age_seconds` (both null without rows), `login`, `requests` and `lock_wait_seconds`, and the lines after it are the view `record_history`. Tests: `a3_day_twice_then_record_by_a_korean_name_then_history`.
- R43: The line 1 of `refresh` has exactly `status`, `dates` (their number), `login`, `requests` and `lock_wait_seconds`, and is followed by one line per date in the order given, one JSON object with exactly `date`, `captured_at`, `age_seconds`, `rows`, `stored`, `outside_roster` and `cut_fields`. Tests: `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date`.

A lesson session

- R44: After a refusal whose code the Interface marks `falls back: yes`, a lesson session reads the DailyRecord it asked for through `academy_dailyrecord_get`. (no test yet; ars-lesson-replay and the cutover's lesson day observe it)
- R45: After a refusal whose code the Interface marks `falls back: no`, a lesson session does not call `academy_dailyrecord_get` for that read and reports the code. (no test yet; ars-lesson-replay and the cutover's lesson day observe it)
- R46: After a refusal, a lesson session never runs `edu` again with the same arguments. (no test yet; ars-lesson-replay and the cutover's lesson day observe it)

## Interface

### Forms

```
edu [--config <path>] lesson day <date> [--refresh]
edu [--config <path>] lesson record <date> <student> [--refresh] [--exact]
edu [--config <path>] lesson history <student-id> [--since <date>]
edu [--config <path>] lesson refresh <date>...
```

- `--config <path>` is accepted only as the first two words; no other word names a path.
- `<date>` is `YYYY-MM-DD`. `refresh` takes 1 to 31 distinct dates.
- `<student>` is a student id (1 to 12 ASCII digits) or a name as the date's row shows it
  (1 to 200 bytes, no control character). An id is used as it is, whether or not the date
  has a row of it. `<student-id>` is an id only.
- Flags may stand anywhere after the verb, each at most once. A word that is not UTF-8 is
  read as an empty word, which every form refuses.

### Order of a run

1. The words (R7 to R13).
2. The configuration file (R15).
3. The engine's `describe` and the schema digest (R19 to R21).
4. The two spool directories of the family `dailyrecord`, then the index of `lesson_record`
   and `lesson_day` (R23).
5. `day`, `record` and `refresh`: the capture when R24 asks for it, then the index again.
6. The reads and the answer; then the run log line (R17), when the configuration was accepted.

### Answers

Line 1 is one JSON object. After it come the engine's table unchanged (`day`, `record`,
`history`), the rows of `--exact`, or the coverage lines of `refresh`.

| Verb | Members of line 1 |
| --- | --- |
| `day` | `status`, `date`, `captured_at`, `age_seconds`, `rows`, `stored`, `outside_roster`, `cut_fields`, `login`, `requests`, `lock_wait_seconds` |
| `record` | those of `day` and `student_id` |
| `history` | `status`, `since`, `rows`, `captured_at`, `age_seconds`, `login`, `requests`, `lock_wait_seconds` |
| `refresh` | `status`, `dates`, `login`, `requests`, `lock_wait_seconds` |

`rows`, `stored`, `outside_roster` and `cut_fields` are the date's `lesson_day` counts
(`row_count`, `stored_count`, `outside_roster_count`, `cut_fields`). `login` is `new` or
`none`. As built, the members of a JSON line stand in the order of their names; a reader
takes them by name.

An example of line 1 of `record`, with invented values:

```json
{"age_seconds":12,"captured_at":1930000000,"cut_fields":0,"date":"2031-03-03","lock_wait_seconds":0,"login":"none","outside_roster":0,"requests":0,"rows":2,"status":"ok","stored":2,"student_id":"90001"}
```

Exit codes: 0 with `status` `ok`; 2 with a refusal, or when standard output cannot be written.

### Refusal codes

edu's own codes, each with what a lesson session does after it (R44, R45):

| Code | When | Falls back |
| --- | --- | --- |
| `lesson_usage_invalid` | R7 | falls back: no |
| `lesson_date_invalid` | R8 | falls back: no |
| `lesson_date_in_future` | R9 | falls back: no |
| `lesson_student_invalid` | R12, R13 | falls back: no |
| `lesson_config_refused` | R15 | falls back: yes |
| `lesson_run_log_failed` | R18 | falls back: yes |
| `lesson_store_unavailable` | R20 | falls back: yes |
| `lesson_schema_mismatch` | R21 | falls back: yes |
| `lesson_index_failed` | R23 | falls back: yes |
| `lesson_capture_failed` | R33 | falls back: no |
| `lesson_roster_refused` | R34 | falls back: yes |
| `lesson_coverage_missing` | R35 | falls back: yes |
| `lesson_student_not_found` | R39 | falls back: no |
| `lesson_student_ambiguous` | R40 | falls back: no |

Any other code: falls back: no. These are the codes edu passes through as it receives them:
from the sources crate (the owner root, the transport admission, the account lock, the
session and its login, for example `owner_account_busy` and
`native_owner_authentication_failed`), from a refused date of the capture (for example
`capture_day_date_conflict`, and `capture_input_invalid` for a refused date without a code),
and from the engine (for example `principal_not_trusted`).

The reason of the two classes: `yes` is a fault of edu's own configuration, store, index or
run log, which `academy_dailyrecord_get` does not use. `no` is a word the server's tool
would be given as wrongly, an answer about the date's rows, or a refusal of the LMS, the
account or the session, which the server's tool reaches through the same account.

### Configuration

A JSON object, owned by the caller, mode without write for group or others, at most 64 KiB.
An example with invented values:

```json
{"schemaVersion":"edu.lesson-config/1","ownerRoot":"/srv/example/owner",
 "admissionSha256":"<64 hex>","rosterSha256":"<64 hex>","spool":"/srv/example/spool",
 "store":"/srv/example/store/store.sqlite","secretFile":"/srv/example/secret.env",
 "secretKey":"EXAMPLE_KEY","identityFile":"/srv/example/identity.json",
 "rawDir":"/srv/example/raw","lockWaitSeconds":30}
```

| Member | Required | Value |
| --- | --- | --- |
| `schemaVersion` | yes | `edu.lesson-config/1` |
| `ownerRoot` | yes | Absolute path of a standing owner root (`docs/academy-capture.md`): `account-lock.json`, `transport-admission.json`, `roster.json` |
| `admissionSha256`, `rosterSha256` | yes | The sha256 of `transport-admission.json` (read up to 256 KiB) and of `roster.json` (read up to 1 MiB) |
| `spool` | yes | Absolute path of the spool the read schema's sources name |
| `store` | yes | Absolute path of the store file; the run log is in its directory |
| `secretFile`, `secretKey` | yes | Absolute path of the secret file and the key of its password line |
| `identityFile` | yes | Absolute path of the identity file (`sourceUsername`) |
| `rawDir` | no | Absolute path of the raw answers' directory |
| `lockWaitSeconds` | no | 0 to 1 hour, in seconds; 30 when absent |

### The run log

`<directory of store>/lesson-runs.jsonl`, mode 0600, one JSON object per run: `time` (seconds
since the epoch), `verb` (`day`, `record`, `history`, `refresh` or `unknown`), `dates` (the number of
dates of `refresh`; 1 for every other verb whose words were accepted, `history` included,
which is given no date; 0 when the words are refused), `login`, `requests`, `lock_wait_seconds`,
`status` (`ok` or the code).

### The store it reads

The configured store, an Ontology store with the domain `academy_read` of
`ontology/academy-read.schema.json` installed for the configured spool. The command calls the
engine in process with the run's clock and the caller's two user ids: `describe`, `index`
of the sources `lesson_record` and `lesson_day`, `search` of the object types `lesson_record`
(by `lesson_date` and `student_name` to resolve a name; by `lesson_date` and `student_id` for
`--exact`; by `student_id` and `lesson_date` for `history`) and `lesson_day` (by
`lesson_date`, the coverage), and `render` of the views `daily_records` (`lesson_date`),
`daily_record` (`lesson_date`, `student_id`) and `record_history` (`student_id`, `since`).

### Tests

`crates/edu/tests/lesson.rs`, on an invented LMS, invented credentials, identity and roster
and a disposable store:

| Test | Requirements |
| --- | --- |
| `a3_day_twice_then_record_by_a_korean_name_then_history` | R1, R4, R5, R6, R17, R24, R25, R26, R27, R30, R31, R36, R37, R38, R42 |
| `i0_a_malformed_or_future_date_is_refused_before_any_request_or_engine_call` | R2, R8, R9, R11, R21 |
| `i2_refresh_of_three_dates_logs_in_once_with_one_request_per_date` | R1, R4, R17, R24, R26, R27, R32, R37, R43 |
| `i3_refusals_carry_no_cookie_password_name_or_text` | R2, R4, R5, R7, R12, R13, R17, R28, R31, R39 |
| `a_name_on_two_students_is_refused_with_their_count_and_exact_prints_members` | R40, R41 |
| `a_caller_that_is_not_root_is_refused_by_the_engine_rule` | R17, R19 |
| `i5_a_store_with_another_schema_is_refused_before_any_request` | R21 |
| `a_busy_account_lock_is_retried_for_the_configured_seconds` | R17, R29, R30 |
| `the_configuration_file_is_private_and_exact_and_a_failed_index_shows_no_table` | R2, R3, R15, R16, R23 |
| `seoul_dates_follow_the_calendar` | R10 |

## Fields against academy_dailyrecord_get

Each member of the observed answer of `academy_dailyrecord_get`
(`modules/academy-workbench/tools/academy-mcp/src/dailyrecord-read.ts`, `status` `observed`)
and the member of `edu lesson record` that carries it. "Line 1" is the answer's first line;
"row" is a column of the view `daily_record` and a member of a row of `--exact`.

| Member of `academy_dailyrecord_get` | Member of `edu lesson record` |
| --- | --- |
| `status` (`observed`) | line 1 `status` (`ok`) |
| `sourceAvailability` (`observed`) | not carried |
| `displayName` | row `student_name` (the name the row showed) |
| `lessonDate` | line 1 `date`; row `lesson_date` with `--exact` |
| `timeZone` (`Asia/Seoul`) | not carried (R10 fixes the zone) |
| `purpose`, `currentRequestRef` | not carried |
| `academyWrites`, `parentSends`, `execution` | not carried (edu never writes to the LMS) |
| `automaticRetryAllowed` | not carried (R46 is the rule for a session) |
| `sourceContentIsUntrustedData` | not carried |
| `scope.sourceNamespace` | row `source_namespace` with `--exact` |
| `scope.teacherId` | not carried |
| `scope.studentId` | line 1 `student_id`; row `student_id` |
| `scope.lessonDate` | line 1 `date` |
| `route` | not carried |
| `fields[]` of `progress`: `before.value` | row `progress` |
| `fields[]` of `homework`: `before.value` | row `homework` |
| `fields[]` of `lessonMemo`: `before.value` | row `lesson_memo` |
| `fields[]` of `attendance`: `before.value` | row `attendance` (column `Attendance value`) |
| `fields[]` of `dailyTestAssessment`: `before.value` | row `daily_test_assessment` |
| `fields[]` of `previousHomeworkAssessment`: `before.value` | row `previous_homework_assessment` |
| `fields[].field` | the row member's name |
| `fields[].before.label` | not carried (row `attendance_label` is the page's option text, not the server's code label) |
| `fields[].before.state`, `before.evidenceRefs`, `before.reasonCode` | not carried (a value the row lacks is left out with `--exact` and renders `(missing)` in the table) |
| `fields[].selected`, `fields[].disposition` | not carried |
| `fields[].target.sourceNamespace`, `target.studentId`, `target.lessonDate` | as `scope` above |
| `fields[].target.teacherId` | not carried |
| `fields[].target.courseId` | row `course_id` |
| `fields[].target.membershipId` | row `membership_key` |
| `fields[].target.recordId` | row `record_key` |
| `source.observedAt` | line 1 `captured_at` with `age_seconds`; row `observed_at` (column `Observed at`) |
| `source.sourceRevision`, `source.wireDigest` | not carried |
| `source.transmissionState`, `transmissionState` | not carried |
| `preparation.status`, `preparation.interviews`, `preparation.requiredOnlyForAuthoring` | not carried |
| `assessmentCodeTables`, `assessmentCodeLabels` | not carried |
| `authoringHandle` | not carried |

The server's tool refuses a source older than 5 minutes (`stale_source`); edu refuses no
age and reports it in `age_seconds`. Row members edu carries that the server's answer does
not: `student_memo`, `course_label`, `attendance_label`, `previous_record_key`,
`previous_lesson_date_literal`, `previous_progress`, `previous_homework`,
`previous_lesson_memo`, `row_ordinal`, `cut_fields`.

## Not included

- Saves, drafts, review and transmission of a DailyRecord: the command reads only.
- The views `registered_books`, `section_results` and `day_coverage`, and the families
  other than `dailyrecord`.
- The format of the engine's tables, which is the engine's; this contract passes them
  unchanged.
- How the configuration file, the standing owner root, the secret file and the store are
  installed, and who owns them.
- Limits of the LMS transport (request and answer sizes, timeouts).
