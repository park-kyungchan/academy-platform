# The PC paper executor of `academy_paper_print_pdf_get` and of `printPdf` of `academy_clinic_paper_save`

## Purpose

Exam papers, answer papers, score (result) papers and clinic papers are made as PDF files by the
native StudyQ programs on the user's Windows PC: `StudyQPdfManager.exe` prepares the images, the QR
codes and `examset.info`, and `ETestMaker.exe` renders the PDF file. The print list comes from the
Academy server directly, read-only, or, for clinic papers, from the write answer of an approved
clinic save; it is passed as the LMS page passes it, so the PDF is the one the Academy's own client
makes. Nothing on this host draws a page: the Linux assembly of `print.zip` archives is retired.
The contract is relied on by the teacher who prints papers on lesson days, by the agent that calls
the tools, by the host step that pins and installs the programs, and by every later change of the
tools.

## Requirements

Launch and connection

- R1: The executor takes no launch option of its own: it uses the configuration of `--pc-executor` ([pc-executor.md](pc-executor.md) R1). The StudyQ install root on the PC is the parent folder of `<exeDir>`; `StudyQPdfManager.exe` is started in `<root>\studylink` and `ETestMaker.exe` in `<root>\ipm`. Without `--pc-executor`, or when `<exeDir>` has no parent folder of the form of R1 there, every call answers `status` `unavailable` with `reasonCode` `pc_executor_not_configured` before any request. Tests: pc_paper_executor_install_root_is_the_parent_of_exe_dir, pc_paper_without_pc_executor_is_pc_executor_not_configured.
- R2: `ssh` and `scp` are started with the fixed argv of [pc-executor.md](pc-executor.md) R2 (the same builder); no value of a caller reaches an argv except the fixed name `pdf.info`, a temporary name `./.pc-executor-<16 hex>.tmp` and a PDF name that passed the checks of R11. Each print item holds the host-wide lock of [pc-executor.md](pc-executor.md) R3 (the same lock file, so report and paper calls never interleave) from before its first remote step to after its last, and has a deadline of 300 s (waiting for the lock included); each ssh or scp step other than the two program runs ends after 30 s. A step past its own limit or a connection that fails answers `pc_unreachable`, a host key that fails `pc_host_key_mismatch`, an item past its 300 s `pc_executor_timeout`. Tests: pc_paper_executor_prints_one_item_and_leaves_only_the_native_cache, pc_paper_executor_unreachable_and_host_key_mismatch, pc_paper_executor_shares_the_host_wide_lock_of_the_report_executor.

Input and print lists

- R3: `academy_paper_print_pdf_get` takes `source` (`paper` or `result`), for `paper` `paperNo` and optionally `printCase` (`teacher_paper`, the default, `student_paper` or `answer_paper`), for `result` `testingNo` and `studentKey`, and optionally `outputPath`; any other member, a missing key of the source or a key of the other source refuses as invalid input. The members of the retired Linux assembly (`mode`, `paperKeys`, `selType`, `includeAnswers`, `includeExplain`) are refused like any unknown member. Tests: pc_paper_input_takes_source_and_its_keys_only.
- R4: `source` `paper`: the one print item is built member for member as `MODULE.Typeset.createTestPaper` and `addTypesetObject` (print type `each`) of the LMS page build it from the answer of `getTestExamList` (table "Paper item"); `paper_type` is `test` for `teacher_paper` and `student_paper` and `answer` for `answer_paper`; `logo` is `https://storage.studyq.net/school_data/<sk>/<sk>-logo.png` with `<sk>` the principal's school key; `student_name` is empty; a member whose source member is absent is absent. Tests: pc_paper_item_is_built_as_the_page_builds_it, pc_paper_answers_one_entry_per_item_with_numbered_paths.
- R5: The requests of a call are reads only: `getTestExamList` (`TestpageSelectExServlet`, `p_no` and `p_option_type` the print case) and, for `source` `result`, `getStudyResultSingleTestingSingleUser` with exactly the condition `{testing_no, pri_no, student_name: "", score: {create: 1}, incorrect: {create: 0}, similar: {create: 0}, advance: {create: 0}, report: {create: 0}}` (the clinic readback's form), under the mock transport that answers `write_forbidden` to every write. Tests: pc_paper_reads_only_read_class_routes_i1_write_forbidden.
- R6: `source` `result`: the print list is the `print_list` of that answer as the server sent it; an answer whose `result` is not `OK` or whose list is not an array of objects, or an item whose `pri_no` names another student, refuses with `paper_print_source_unverified`; an empty list refuses with `paper_print_list_empty`. Tests: pc_paper_reads_only_read_class_routes_i1_write_forbidden, pc_paper_item_completeness_is_checked_for_each_source.
- R7: Before any PC step every item of the list is checked: it holds `add_answer`, `add_code`, `add_difficulty`, `add_explain`, `add_pat`, `exam_dispose_type` and `exam_spacer` (not null), and an item built from `getTestExamList` also `add_incorrect_comment` (from its `add_comment`), and a non-empty `exam_list` of objects; else the call refuses with `paper_print_item_incomplete` (an `examset.info` without these members crashed `ETestMaker.exe`). Every `exam_size_list` that is a string is parsed into its JSON array; one that is not a JSON array refuses with `paper_print_item_incomplete`. Tests: pc_paper_item_completeness_is_checked_for_each_source, pc_paper_executor_item_incomplete_and_unencodable_refuse_before_any_remote_step.
- R8: `pdf.info` is `{"app_ver":"8.6","cmd":"Print","pid":"<milliseconds>","print_list":[<one item>],"sk":<school key>,"uk":<teacher key>}`, the keys of the signed-in principal as JSON numbers (without `sk` the native program writes `clg_no` 0), encoded in CP949; a character outside CP949 refuses with `pc_input_unencodable` before the lock and before any remote step. Tests: pc_paper_executor_prints_one_item_and_leaves_only_the_native_cache, pc_paper_executor_item_incomplete_and_unencodable_refuse_before_any_remote_step.

One item on the PC

- R9: Step 1: when a `StudyQPdfManager.exe`, `ETestMaker.exe` or `PDFMerge.exe` runs on the PC (the teacher is printing), the item refuses with `pc_busy` before anything is sent. Tests: pc_paper_executor_refuses_pc_busy_before_sending_anything.
- R10: Step 2: `pdf.info` is copied to `%APPDATA%\otwoline\studyq`, `StudyQPdfManager.exe` is started by one fixed PowerShell program (`Start-Process -PassThru` in its folder), and the program waits up to 120 s until `examset\examset.info` is newer than the start and readable; then it stops that `StudyQPdfManager.exe` and every process whose parent it is, by process id. No other process is stopped and no process is stopped by its name. Past 120 s the item answers `pc_executor_timeout`; a manager that ends without writing the file answers `pc_executor_failed`. Tests: pc_paper_executor_prints_one_item_and_leaves_only_the_native_cache, pc_paper_executor_manager_timeout_stops_only_its_own_processes_and_cleans_up.
- R11: Step 3: a stale `examset\examset_result.info` is deleted and the names of the PDFs in `examset\` are listed; `ETestMaker.exe` is started the same way, waited for up to 120 s and past that only its own process id is stopped (`pc_executor_timeout`); then `examset_result.info` is read (CP949). A result other than `result` `ok` with exactly one `pdf_list` entry whose `pdf_file` matches `^\d{17}\.pdf$` and was not listed before, with a `page_cnt` integer of at least 1, answers `pc_output_invalid`. Tests: pc_paper_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf.
- R12: Step 4: that PDF is copied to a temporary name beside the output path, must begin with `%PDF-` and carry the Producer `Haru Free PDF Library` (string escapes decoded), else `pc_output_invalid`; it is linked to the output path and the temporary name unlinked, so an output that appeared meanwhile answers `output_exists` and is kept. Then the PDF, `examset_result.info`, `examset.info` and `pdf.info` are deleted on the PC. Tests: pc_paper_executor_prints_one_item_and_leaves_only_the_native_cache, pc_paper_executor_output_that_appeared_meanwhile_is_output_exists, pc_paper_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf.
- R13: After a failure of steps 2 to 4 the item deletes `pdf.info`, `examset.info` and `examset_result.info` (a `pdf.info` or `examset.info` left by the teacher's last print is overwritten and deleted the same way), and a PDF only when `examset_result.info` names it under the name rules of R11 (after an `ETestMaker.exe` timeout it is read once more 5 s later). Nothing else on the PC is created or removed, except the native cache of the programs: `exam\`, `examtext\` (images and the QR images in each exam folder) and `examset\theme\`. A failure after `pdf.info` was sent states `pcCleanup` (`done` or `incomplete`). Tests: pc_paper_executor_manager_timeout_stops_only_its_own_processes_and_cleans_up, pc_paper_executor_maker_timeout_deletes_only_the_pdf_the_result_names, pc_paper_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf.

Answers

- R14: A call answers one entry per print item, in order: `{paperType, pageCount, outputPath, sha256}` in `pdfs`, with `status` `rendered`, `pcCleanup`, `academyWrites` 0 and `execution` `read_only_lms_pc_executor`. With one item the PDF is written at `outputPath` (a `.pdf` path inside the owner `print-pdfs` root, never overwritten; default `<root>/print-<case>-<key>-<time>.pdf`, which holds no personal value); with more than one the n-th is that path with `-<n>` before `.pdf`. Every path is checked for `output_exists` before the first PC step. When an item fails the PDFs before it stay and are answered in `pdfs` with the error (`status` `unavailable`, `reasonCode`, `failure`). Tests: pc_paper_answers_one_entry_per_item_with_numbered_paths, pc_paper_failed_item_keeps_the_items_before_it.
- R15: A failure answers one `reasonCode` of the table "Codes" and the `failure` of the read-failure catalog; no member of a failure holds a student value or a remote program's output. Tests: pc_paper_failed_item_keeps_the_items_before_it, pc_paper_executor_unreachable_and_host_key_mismatch.

Clinic papers

- R16: `academy_clinic_paper_save` takes `printPdf` (boolean). When it is `true` and the save answers `saved`, the `print_list` of the write answer of that same call is handed to the paper executor (R7 to R14, without `add_incorrect_comment`) after the attempt was recorded `reconciled` `persisted` in the ledger, outside the save's serialized section and the ledger lock, and in its own try/catch: a PC error only adds a `pdf` member (`status`, `reasonCode`, `pdfs`, `nextStep`) to the saved answer and never turns the save into an unknown effect. The list is used in that call only and never stored. Without `printPdf` nothing is printed and the answer has no `pdf` member. Tests: clinic_save_print_pdf_prints_the_write_answer_after_the_persisted_record, clinic_save_print_pdf_error_only_adds_a_pdf_member.
- R17: An unverified or refused save does not print and has no `pdf` member. A failed PDF part says in `nextStep` that the papers can be printed again with `source` `paper` and each `paperKey` of the save, when `getTestExamList` answers for that key, and that otherwise the PDF part cannot be repeated; never by calling the save again. Tests: clinic_save_unverified_or_refused_does_not_print, clinic_save_print_pdf_error_only_adds_a_pdf_member.

Privacy and the surface

- R18: `./check` fails when a file under `specs/`, `modules/academy-workbench/tools/academy-mcp/src/` or `apps/academy/dc-gang-a/src/` holds a secret value: a private key block, a token of a known form, a bearer credential, or a quoted value assigned to a password, secret, key or token name (privacy check I6). When `CHECK_STUDYQ_ROSTER` names a file of student names, one per line, it also fails when one of them appears. It prints file and line only, never the text it matched, and does not read `imported/`, `private/` or `private-students/` trees. Tests: i6_check_passes_on_specs_and_the_server_sources, i6_check_fails_on_secret_values_without_printing_them, i6_check_skips_private_trees_and_roster_names_fail_without_being_printed, i6_check_wiring.
- R19: The entry of `academy_paper_print_pdf_get` in `release/surface.json` is not larger in compact JSON than before this contract (795 B), and the whole surface stays within [tool-surface.md](tool-surface.md) R2. The launch option `--studyq-runner` of the retired Linux executor and its five words are dropped unread, so that a launch written for an earlier generation still starts. Tests: pc_paper_tool_surface_is_not_larger_than_before, pc_paper_retired_studyq_runner_option_is_dropped_unread, tool_surface_r2_running_cap.
- R20: A save settled by a read-only readback has no write answer: with `printPdf` it answers `clinic_print_list_unavailable` for the PDF part, and the save stands. (no test yet: in this version a save always holds its write answer; an attempt settled by readback is answered by `academy_clinic_paper_review` as `persisted`, which prints nothing.)

## Interface

### Input

| Member | `source` `paper` | `source` `result` |
| --- | --- | --- |
| `paperNo` | required, a key of 1 to 999999999 | refused |
| `printCase` | `teacher_paper` (default), `student_paper`, `answer_paper` | refused |
| `testingNo`, `studentKey` | refused | required |
| `outputPath` | optional | optional |

`academy_clinic_paper_save` takes `reviewToken`, `userApproved`, `change` ([gated-writes.md](gated-writes.md)) and `printPdf`.

### Paper item

The members in this order; each from the `getTestExamList` answer unless the table says otherwise.

| Member | Value |
| --- | --- |
| `print_type` | `each` |
| `paper_type` | `test` or `answer` (R4) |
| `logo` | the school logo URL (R4) |
| `examset_title`, `sub_title`, `examset_type` | as answered |
| `stage_id` | `stage_id` of the first row of `exam_list` |
| `add_incorrect_comment` | `add_comment` |
| `add_answer`, `add_explain`, `add_code`, `add_difficulty`, `add_pat` | as answered |
| `theme_type`, `theme_color`, `exam_dispose_type`, `exam_spacer` | as answered |
| `student_name` | empty |
| `test_code`, `exam_list` | as answered |

`create_answer` is not passed (the page does not pass it). An example `pdf.info`, with invented
values: `{"app_ver":"8.6","cmd":"Print","pid":"2208988800000","print_list":[{"print_type":"each","paper_type":"test",...}],"sk":88001,"uk":90001}`.

### On the PC

| Path under `%APPDATA%\otwoline\studyq` | Written by | Removed by the call |
| --- | --- | --- |
| `pdf.info` | the call (scp) | yes |
| `examset\examset.info` | `StudyQPdfManager.exe` | yes |
| `examset\examset_result.info` | `ETestMaker.exe` | yes |
| `examset\<17 digits>.pdf` | `ETestMaker.exe` | yes, only the one the result names (R11, R13) |
| `exam\`, `examtext\`, `examset\theme\` | the native programs (cache) | no |

`ETestMaker.exe`, started with no argument and no shared mapping, reads `examset.info` and writes
`examset_result.info` = `{"pdf_list":[{"pdf_type":"test","pdf_file":"<17 digits>.pdf","pri_no":0,"page_cnt":<n>}],"result":"ok"}`
and the PDF (Haru Free PDF Library). The programs and their libraries are pinned by SHA-256 and
installed by the host step that installs this generation; none is in this repository.

### Codes

| Code | When |
| --- | --- |
| `pc_executor_not_configured` | R1 |
| `pc_unreachable`, `pc_host_key_mismatch`, `pc_executor_timeout` | R2, R10, R11 |
| `pc_executor_failed` | a remote program or a copy that fails, R10 |
| `pc_output_invalid` | R11, R12 |
| `pc_input_unencodable` | R8 |
| `pc_busy` | R9 |
| `paper_print_list_empty` | R6, an empty `exam_list` or clinic list |
| `paper_print_item_incomplete` | R7 |
| `paper_print_source_unverified` | R6, an answer of `getTestExamList` that is not `result` `OK` with an `exam_list` of objects |
| `clinic_print_list_unavailable` | R20 |
| `output_exists`, `output_path_invalid`, `output_root_unqualified` | R12, R14 |

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/pc-paper-executor.test.ts` (fake `ssh` and `scp`
programs that keep an invented PC StudyQ folder on the test host, fake backends, the recorded
synthetic LMS of the gated writes) and `tests/tool-surface.test.ts`, with invented values only.

## Not included

- Wrong-note (`incorrect` `create: 1`), similar and advance papers of a result: their creation is a
  server write, offered only through `academy_clinic_paper_save`.
- Printing on paper, merging several PDFs into one, and the viewer of `StudyQPdfManager.exe`.
- Installing or pinning the programs, the identity or the known-hosts file: the host step.
- Uploading the PDFs: the calling agent sends them on.
