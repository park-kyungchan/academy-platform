# The PC executor of `academy_official_report_pdf_get`

## Purpose

The eight official StudyQ reports (`course_book`, `cism`, `diag`, `new_diag`, `formative`, `ganga`,
`na_score`, `smart_book`) are made by the original Windows program `StudyQReport.exe` on the
user's Windows PC, reached over SSH from the host of the TypeScript Academy server. The data is
read directly from the Academy server, read-only, and passed to the program verbatim, so the PDF
is the one the Academy's own client makes; the Linux re-implementation of the report layouts,
whose pages were clipped and misplaced, is retired. The paper PDFs of the same install are
[pc-paper-executor.md](pc-paper-executor.md).
The contract is relied on by the teacher who prints the reports, by the agent that calls the tool
and uploads the PDF, by the host step that installs the launch option, and by every later change
of the tool.

## Requirements

Launch and connection

- R1: The server takes the launch option `--pc-executor <user@host> <identityFile> <knownHostsFile> <exeDir>` at most once: `<user@host>` of letters, digits, `.`, `_` and `-` (beginning with a letter or digit), `<identityFile>` and `<knownHostsFile>` absolute paths of regular files, `<exeDir>` matching `^[A-Za-z]:\\[A-Za-z0-9 ()._\\-]+$`; any other form refuses the launch with `pc_executor_arguments_invalid`. Without the option every report type answers `status` `unavailable` with `reasonCode` `pc_executor_not_configured`, before any other check and any request. Tests: pc_executor_launch_option, official_report_without_pc_executor_is_pc_executor_not_configured.
- R2: `ssh` and `scp` (`/usr/bin/ssh`, `/usr/bin/scp`) are started with a fixed argv and no shell: the option words `-i <identityFile> -o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile=<knownHostsFile> -o IdentitiesOnly=yes -o ConnectTimeout=10 -F none` (so the MCP user's `~/.ssh/config` never applies and the host key is always checked, never accepted anew), then for ssh the target and one `powershell -NoProfile -NonInteractive -EncodedCommand <base64>` of a fixed program, for scp the two paths. No value of a caller reaches an argv except the fixed file names `report.info`, `./report.info`, a temporary name `./.pc-executor-<16 hex>.tmp` (scp runs in the directory of the output path) and a PDF name that passed both checks of R7. The option words come from the one builder that the native pool bridge also uses, whose argv stays `-o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=8 <target> <command>`. Tests: pc_executor_ssh_and_scp_take_fixed_words_only, native_pool_ssh_argv_is_byte_identical.
- R3: Calls are serialized host-wide: a call holds the lock file `<owner artifacts root>/official-reports/pc-executor.lock`, made by exclusive create and holding the process id, the process start time read from `/proc/<pid>/stat` and a nonce, from before its first remote step to after its last, and removes it at the end; a call that finds a lock of a running process with that start time waits; a lock whose process does not run or started at another time is taken over by writing a new lock file under a temporary name and renaming it over the stale one, never by removing it first. So two `academy_operations` processes never interleave their steps. Tests: pc_executor_calls_are_serialized_host_wide, pc_executor_lock_of_a_live_process_waits_and_a_stale_lock_is_taken_over.
- R4: Each ssh or scp step other than the run of the exe ends after 30 s, and the whole call after 300 s; a step stopped by its own limit, or a connection that fails, answers `pc_unreachable`; host key verification that fails answers `pc_host_key_mismatch`; a call that passes its 300 s (waiting for the lock included) answers `pc_executor_timeout`. Tests: pc_executor_unreachable_and_host_key_mismatch, pc_executor_timeout_deletes_only_the_pdf_the_result_names, pc_executor_lock_of_a_live_process_waits_and_a_stale_lock_is_taken_over.

One call

- R5: `report.info` is one JSON object with exactly the members `testing_no`, `book_code`, `p_no`, `pri_no`, `cls_no`, `apply_code`, `report_type` and `block_data`, in that order, every member but `block_data` a JSON number (table "report.info"), `block_data` the data of the report type (R12 to R14) as it is, written as `JSON.stringify` gives it; it is encoded in CP949, and each character outside CP949 (by the table of the encoder) is written as its JSON escape `\uxxxx` (four lowercase hex digits; a code point above U+FFFF as its two UTF-16 escapes; a lone surrogate stays as `JSON.stringify` already escapes it), so `JSON.parse` of `report.info` gives the server's answer back as `block_data` and a `block_data` wholly inside CP949 gives the same bytes as before. `StudyQReport.exe` reads such escapes: measured on the PC with a live CISM week whose `block3` held U+207A twice, the escaped `report.info` gave a 1-page PDF of Producer `Haru Free PDF Library`. The answer of a call that succeeds counts these characters in `escapedCharacters` (0 when none) and holds no character value or position. Tests: pc_executor_report_info_members_are_numbers_and_block_data_verbatim, pc_executor_report_info_is_cp949, pc_executor_character_outside_cp949_is_escaped, pc_executor_renders_one_pdf_and_leaves_the_pc_as_it_was, official_report_rendered_answer_counts_escaped_characters.
- R6: A call runs these steps on the PC's report directory `AppData/Roaming/otwoline/studyq/report` under the user's home: (1) list the names of its `*.pdf` files and delete a stale `report_result.info`; (2) copy `report.info` there; (3) start `<exeDir>\StudyQReport.exe` with `Start-Process -PassThru -WorkingDirectory <exeDir>` and wait up to 120 s for that one process; past that, stop only that process id and answer `pc_executor_timeout` (R10); (4) delete a `report.info` the exe left and read `report_result.info` (CP949); (5) copy the PDF it names (R8); (6) delete that PDF and `report_result.info` (R9). No process is stopped by its image name. Tests: pc_executor_ssh_and_scp_take_fixed_words_only, pc_executor_renders_one_pdf_and_leaves_the_pc_as_it_was.
- R7: `report_result.info` is `{"result":"ok","pdf_list":[{"pdf_type":"report","pdf_file":"<17 digits>.pdf","pri_no":0,"page_cnt":<n>}]}`. A `result` other than `ok`, no result file, a result that is not JSON, a `page_cnt` that is not an integer of at least 1, or a `pdf_file` that does not match `^\d{17}\.pdf$` or was listed in step (1) answers `pc_executor_failed`; a `pdf_list` of other than exactly one entry, or an entry whose `pdf_type` is not `report`, answers `pc_output_invalid`. Tests: pc_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf.
- R8: The PDF is copied to a temporary name beside the output path, must begin with `%PDF-` and carry the Producer `Haru Free PDF Library` (compared after the PDF string escapes are decoded; a file stores `Haru\040Free\040PDF\040Library`), else `pc_output_invalid`; it is then linked to the output path and the temporary name unlinked, so an output that appeared meanwhile answers `output_exists` and is kept. The answer's `sha256` is that of the file, its `pages` the `page_cnt`. Tests: pc_executor_renders_one_pdf_and_leaves_the_pc_as_it_was, pc_executor_pdf_producer_decodes_string_escapes, pc_executor_output_that_appeared_meanwhile_is_output_exists, pc_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf.
- R9: The PDF and `report_result.info` are deleted on the PC, after a failure too, but only once a `pdf_file` that matched `^\d{17}\.pdf$` and was not listed in step (1) is known (a single entry of `pdf_list`); a name that fails either check is never deleted and never part of an argv. Nothing else on the PC is created or removed but `report.info`, `report_result.info` and that one PDF. A failure after `report.info` was sent states `pcCleanup`: `done` when the PC holds no file of the call, else `incomplete`. Tests: pc_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf, pc_executor_renders_one_pdf_and_leaves_the_pc_as_it_was, pc_executor_unreachable_and_host_key_mismatch.
- R10: After `pc_executor_timeout` of the exe the call reads `report_result.info` once more after 5 s, without deleting anything in that read; it deletes a PDF only when that file names it and the name passes both checks of R9, together with `report_result.info` and a left `report.info` (`pcCleanup` `done`); otherwise it leaves the PC as it is (`pcCleanup` `incomplete`). Another new PDF that appeared meanwhile is not deleted. Tests: pc_executor_timeout_deletes_only_the_pdf_the_result_names.
- R11: A failure answers `status` `unavailable` with one `reasonCode` of the table "Codes" and the `failure` of the read-failure catalog; no member of a failure holds a student value or a remote program's output. Tests: official_report_pc_failures_are_answered_with_their_code, pc_executor_unreachable_and_host_key_mismatch.

Data, server-direct and verbatim

- R12: `course_book` (`report_type` 22): `block_data` is the JSON object of `getCourseBookReport` (`TextBookManageServlet`, `course_key`, `book_no`, `student_pri_no`) exactly as the server answered it, after the key checks of [direct-key-lookup.md](direct-key-lookup.md) R4; nothing is projected, renamed or cut, and a report without a test is passed like any other (the refusal `course_book_no_tests` is retired). Tests: official_report_course_book_passes_the_raw_answer_verbatim, official_report_course_book_without_tests_renders_what_the_server_returns, direct_key_course_book_report_is_kept_as_the_server_sent_it.
- R13: `diag` (20), `new_diag` (19), `formative` (17), `ganga` (16), `na_score` (18) and `smart_book` (21): `block_data` is the JSON object of the `ReportViewServlet` view (`DiagReportView`, `NewStudentReportView`, `FaView`, `GaReportView`, `NaScoreResultView`, `SmartBookReportView`, the forms of [direct-key-lookup.md](direct-key-lookup.md) R8, unchanged) exactly as answered, and the members of `report.info` carry the keys that request used. Tests: official_report_report_views_pass_the_raw_answer_with_their_keys, official_report_class_no_zero_is_accepted, official_report_report_views_read_only_read_class_routes_i1_write_forbidden, official_report_ganga_na_score_smart_book_read_only_read_class_routes_i1_write_forbidden.
- R14: `cism` (200), `params.scope` `week`: `block_data` is the object the page function `makeCISMJSonData` of `CismChartMain` builds, read from the same read-only page (table "cism block data"); the fields, the wire hash, the report hash and the writes of the CISM tools are unchanged, and the CISM tools do not answer the block data. Tests: official_report_cism_block_data_is_read_as_the_page_function_builds_it, official_report_cism_week_passes_the_page_block_data, official_report_cism_reads_only_read_class_routes_i1_write_forbidden.
- R15: `cism` with `params.scope` `month` refuses with `cism_month_unavailable` before any request (the executor has no month form). Tests: official_report_cism_month_is_cism_month_unavailable.
- R16: A call that succeeds answers `status` `rendered` with `outputPath`, `sha256`, `pages`, `pcCleanup`, `escapedCharacters` (R5), `execution` `read_only_lms_pc_executor` and `academyWrites` 0, and no parity member (`parityOk`, `parityMismatches`, `parityRawRn`) and no `truncation`. The tool does not upload: the calling agent sends the PDF on. Tests: official_report_course_book_passes_the_raw_answer_verbatim, official_report_cism_week_passes_the_page_block_data.
- R17: No Academy write: every server request of a call is one of the existing read routes, under the mock transport that answers `write_forbidden` to every write. Tests: official_report_cism_reads_only_read_class_routes_i1_write_forbidden, official_report_report_views_read_only_read_class_routes_i1_write_forbidden, official_report_ganga_na_score_smart_book_read_only_read_class_routes_i1_write_forbidden.
- R18: (removed)

## Interface

### report.info

| Member | `course_book`, `cism` | `diag`, `formative`, `ganga`, `na_score` | `new_diag` | `smart_book` |
| --- | --- | --- | --- | --- |
| `testing_no` | 0 | `testingNo` | 0 | 0 |
| `book_code` | 0 | 0 | 0 | `bookCode` when all digits, else 0 |
| `p_no` | 0 | 0 | `paperNo` | 0 |
| `pri_no` | 0 | the student key of the request | 0 | the student key of the request |
| `cls_no` | 0 | `classNo` | 0 | 0 |
| `apply_code` | 0 | 0 | `applyCode` when all digits, else 0 | 0 |
| `report_type` | 22, 200 | 20, 17, 16, 18 | 19 | 21 |

The codes are those of the dispatcher of `StudyQReport.exe`; a key is written as its digits
without leading zeros. An example, with invented values:
`{"testing_no":901,"book_code":0,"p_no":0,"pri_no":770011,"cls_no":0,"apply_code":0,"report_type":17,"block_data":{...}}`.

### cism block data

| Block | Rows |
| --- | --- |
| `block1` | `[{"1":"1"}]` when the numbers of `label[name=label_think_note]` sum to other than 0, else `[{"1":"0"}]` |
| `block2` | `[{"1":<the_date>,"2":<teacher_name>,"3":"<M>월<D>일~<M>월<D>일"}]`, the week of the page's week label |
| `block3` | one row per `div[id^=root]` `i`: `"1"` the text of `grp_name_i`, `"2"` `하`, `중` or `상` for the value 1, 2 or 3 of the select in `class_mode_tag_i` (else empty), `"3"` the text of `submit_sum_i`, `"4"` of `homework_rate_i`, `"5"` the labels of `think_note_i` and `%`, `"6"` the labels of `week_test_i` and `점` |
| `block4` | `{"1":"Ｉ-1","2":<i_1>}`, `{"1":"Ｉ-2","2":<i_2>}` |
| `block5` | `Ｓ-1` to `Ｓ-4` with `s_1` to `s_4` |
| `block6` | `Ｍ-1`, `Ｍ-2` with `m_1`, `m_2` |

Texts are read as jQuery reads them (`.text()`, `.val()` without carriage returns); `Ｉ`, `Ｓ` and
`Ｍ` are full-width.

### The lock

The lock holds `{"pid":<n>,"start":"<field 22 of /proc/<pid>/stat>","nonce":"<32 hex>"}`. The
start time identifies a process only within one host and one pid namespace: every
`academy_operations` process of the host runs in the host's namespace.

### Codes

| Code | When |
| --- | --- |
| `pc_executor_not_configured` | R1 |
| `pc_executor_arguments_invalid` | R1, at launch |
| `pc_unreachable` | R4: a connection that fails or a step past its 30 s |
| `pc_host_key_mismatch` | R4 |
| `pc_executor_failed` | R6, R7: a remote program or a copy that fails, or a result of R7 |
| `pc_executor_timeout` | R4, R6, R10 |
| `pc_output_invalid` | R7, R8 |
| `pc_input_unencodable` | An internal guard of the CP949 encoder: no tool input reaches it for `report.info`, whose characters outside CP949 are escaped (R5) |
| `cism_month_unavailable` | R15 |
| `output_exists`, `output_path_invalid`, `output_root_unqualified` | the output path, as before, and R8 |

### On the host

The SSH target, the identity copy readable by the MCP user, the known-hosts file with the PC's
host key, and `<exeDir>` (`C:\Program Files (x86)\otwoline\studyq\report` on the PC) are
installed by the host step that writes the launch option; none is in this repository. When the PC
is off or unreachable the tool answers `pc_unreachable` and no other renderer is used.

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/pc-executor.test.ts` (fake `ssh` and `scp`
programs that keep an invented PC directory on the test host), `tests/official-report-pdf.test.ts`
and `tests/direct-key-lookup.test.ts`, with invented values only.

## Not included

- Installing the identity, the known-hosts file or the launch option: the host step.
- Uploading the PDF: the calling agent sends it to Google Drive.
- The exam, answer, score and clinic papers of the PC: [pc-paper-executor.md](pc-paper-executor.md).
- A month of CISM: the executor has no month form.
- Checking the report's content against the student: the request's keys name the student.
