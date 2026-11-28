# Gated Academy writes: `academy_clinic_paper_{review,save}` and `academy_exam_paper_{review,save}`

## Purpose

Making a wrong-note or clinic paper for a student, and creating and assigning a formative
assessment (FA) or unit-exam paper, are Academy writes that the teacher does by hand today.
This contract lets the TypeScript server do them in two phases, `review` then `save`, in the
pattern of the CISM write (`apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts`): the review
reads the live page, pins its form by `wireSha256` and returns a `reviewToken`; the save
writes only that reviewed change, only with the teacher's explicit approval, records each
attempt before it writes, and reads the result back. After a crash the review reconciles the
open attempt by a read-only readback before any retry, so an exam paper created but not yet
assigned resumes at its assignment step. It is relied on by the teacher, by the skills that
call the tools ([tool-surface.md](tool-surface.md)), and by the units that build and check the
writes. Every refusal before a write sends no write request, of any HTTP method.

## Requirements

Review

- R1: `academy_clinic_paper_review` and `academy_exam_paper_review` send only requests whose route is classed read (Interface, "Request classes"), 0 write requests: they read the live page of the write, compute `wireSha256` over its write program (Interface, "Wire hash"), and refuse with `wire_hash_mismatch` when it differs from the value pinned for that page. Tests: `gated_writes_r1_review_sends_only_read_requests_and_reads_the_live_page`, `gated_writes_r1_a_drifted_or_missing_handler_is_a_wire_hash_mismatch_with_0_write_requests`, `gated_writes_pins_the_two_production_wire_hashes_exactly`, `gated_writes_without_the_test_seam_a_synthetic_page_is_a_wire_hash_mismatch`, `gated_writes_route_classes_read_only_without_a_create_flag_and_write_otherwise`, `gated_writes_route_classes_the_two_wire_pages_read`.
- R2: A review that passes answers `status: 'reviewed'`, the exact change it would make, and a `reviewToken` that is the SHA-256 of the canonical JSON of the change, the student and scope keys, the `wireSha256` and the review's expiry time, so the same review of the same page at the same expiry gives the same token; a token expires 15 minutes after its review. Tests: `gated_writes_r2_the_review_token_is_the_sha256_of_the_canonical_change_scope_wire_hash_and_expiry`.

Save

- R3: A `*_save` call without `userApproved: true` (absent, `false` or any other value) refuses with `user_approval_required`. Tests: `gated_writes_r3_save_without_user_approval_true_is_refused_with_0_http_requests`.
- R4: A `*_save` call whose `reviewToken` is unknown, already used, or was made by the other tool, or whose change differs from the reviewed change in any member, refuses with `review_token_invalid`. Tests: `gated_writes_r4_an_unknown_used_foreign_or_changed_token_is_review_token_invalid_with_0_http_requests`.
- R5: A `*_save` call whose `reviewToken` is more than 15 minutes old refuses with `review_token_expired`. Tests: `gated_writes_r5_a_token_more_than_15_minutes_old_is_review_token_expired_with_0_http_requests`.
- R6: Before each write a `*_save` call reads the live page again and refuses with `wire_hash_mismatch` when its `wireSha256` differs from the reviewed one. Tests: `gated_writes_r6_a_wire_hash_that_drifted_since_the_review_refuses_before_the_write`.
- R7: A change that would publish a paper school-wide (to every class or every student of the academy, or to the public pool) refuses with `public_scope_forbidden`, at review and at save; a paper is created and assigned only to the students and classes the change names. Tests: `gated_writes_r7_public_all_student_or_missing_scope_is_refused_at_review_and_save_with_0_http_requests`.
- R8: The requests of a save are only login requests, optionally one initial before-state read, and, for each step of the reviewed change (one step for a clinic paper; for an exam paper the creation step, then the assignment step), at most one pre-write read that yields the `wireSha256` (R6), one write request and one readback read (R14); every request is on the routes of the table "Tools", and a save sends no browse query. Tests: `gated_writes_r8_a_save_sends_one_pre_write_read_one_write_and_one_readback_read`, `gated_writes_clinic_posts_the_exact_condition_for_every_mode_and_flag_combination`, `gated_writes_clinic_refuses_inputs_that_do_not_fit_the_mode_before_any_request`, `gated_writes_book_incorrect_posts_the_single_or_multi_user_condition_of_the_frozen_testing_numbers`.

Lock and ledger

- R9: Before its first LMS request a review or save takes the shared account lock of `docs/academy-account-lock.md`, waiting up to 45 seconds, and holds it until its last request; when the lock is still held after 45 seconds the call ends with a tool error carrying `owner_account_busy`, and 0 HTTP requests are sent. Tests: `gated_writes_r9_a_held_account_lock_ends_each_call_with_owner_account_busy_and_sends_no_request`.
- R10: A save records an attempt in the attempt ledger (state `open`, the `reviewToken`, the change's hash, the time) and records each step before that step's write request, each record made durable (written and synced to disk) before the request is sent; when a record cannot be made the save refuses with `ledger_unavailable` and sends no further write request. Tests: `gated_writes_r10_an_unset_relative_symlinked_or_loose_ledger_is_ledger_unavailable_before_any_request`, `gated_writes_r10_the_ledger_is_created_0600_and_each_record_is_durable_before_the_request_it_guards`, `gated_writes_r10_a_record_that_cannot_be_made_refuses_ledger_unavailable_and_sends_no_write`, `gated_writes_r10_records_survive_a_separate_process_and_a_corrupt_line_makes_the_ledger_unavailable`, `gated_writes_bridge_a_saved_paper_records_academy_writes_1_and_an_unconfirmed_post_records_an_unknown_effect`.
- R11: A save of a change for which the ledger holds an `open` attempt of the same student and scope whose call is still running refuses with `attempt_already_open`; one whose call has ended without resolving it (a crash, or R14's `unverified`) refuses with `reconcile_blocks_retry` until a review has reconciled it (R12). Tests: `gated_writes_r11_a_live_lock_refuses_attempt_already_open_with_0_http_requests`, `gated_writes_r11_a_stale_lock_is_removed_and_the_unresolved_attempt_blocks_with_reconcile_blocks_retry`, `gated_writes_r11_a_stale_lock_with_no_unresolved_attempt_is_cleaned_and_the_save_proceeds`, `gated_writes_r11_an_unresolved_attempt_of_the_same_student_and_scope_blocks_a_save_other_scopes_are_not_blocked`.
- R12: A review of a change with an unresolved `open` attempt first reconciles it by a read-only readback of the server, with requests classed read only: it marks the attempt `persisted` when the readback shows every step, `partially_persisted` when it shows the exam paper created but not assigned, and `absent` when it shows no step; a readback that fits none of these leaves the attempt `open` and the review refuses with `reconcile_blocks_retry`. Tests: `gated_writes_r12_r13_a_before_write_attempt_is_absent_and_the_review_answers_a_fresh_token`, `gated_writes_r12_r13_a_response_received_attempt_is_persisted_and_the_review_offers_nothing_to_save`, `gated_writes_r12_a_write_dispatched_attempt_or_a_failed_readback_stays_open_with_reconcile_blocks_retry`, `gated_writes_r12_r13_a_crashed_save_is_reconciled_end_to_end_through_the_review`.
- R13: After reconciliation, a `persisted` attempt is answered as done and offers nothing to save; a `partially_persisted` exam attempt is answered with a review of the assignment step alone on the paper already created, whose save sends the requests of that step only and never creates a second paper; an `absent` attempt is answered with a review of the whole change, under a new `reviewToken`. Tests: `gated_writes_r12_r13_a_before_write_attempt_is_absent_and_the_review_answers_a_fresh_token`, `gated_writes_r12_r13_a_response_received_attempt_is_persisted_and_the_review_offers_nothing_to_save`, `gated_writes_r12_r13_a_crashed_save_is_reconciled_end_to_end_through_the_review`.
- R14: After each write a save reads the page back and answers `status: 'saved'` only when the readback shows every step of the reviewed change; otherwise it answers `status: 'unverified'`, keeps the attempt `open`, and sends no further write request (an exam paper whose creation is not read back is not assigned). Tests: `gated_writes_r14_an_unconfirmed_write_answers_unverified_and_keeps_the_attempt_open`, `gated_writes_r14_a_failed_write_request_leaves_the_attempt_open_at_write_dispatched_and_rejects`, `gated_writes_r8_a_save_sends_one_pre_write_read_one_write_and_one_readback_read`.

Refusals and privacy

- R15: Every refusal before a write (R1, R3 to R7, R9, R11, R12, an invalid input, and `ledger_unavailable` of a step not yet written) sends 0 write requests, of any HTTP method; the refusals found from the input alone (R3, R4, R5, R7) and R9 send 0 HTTP requests at all. Tests: `gated_writes_r15_every_refusal_sends_0_write_requests_of_any_method`.
- R16: No answer, error, ledger record or log line holds a student name, record text, cookie or password; the ledger holds keys and hashes only. Tests: `gated_writes_r16_no_answer_or_record_holds_a_name_cookie_or_password`.

## Interface

### Tools

| Tool | Write | Routes |
| --- | --- | --- |
| `academy_clinic_paper_review` / `_save` | wrong-note paper of a book (`kind: 'book_incorrect'`), or the similar and advance clinic papers of one attempt (`kind: 'clinic'`); one step | `GET /servlet/controller.coursemanage.TextBookManageServlet?reqCmd=getTextBookMain` and `GET /servlet/controller.tutor.fa.TestPageListGrpServlet?p_process=Main` (the pages whose inline handlers are pinned; read), and `POST /servlet/controller.common.TestpageSelectExServlet`: `getWrongAnswerNoteResult` (read), `getStudyResultSingleTestingSingleUser` or `getStudyResultMultiTestingMultiUser` with a `create:1` condition (the write), and `getStudyResultSingleTestingSingleUser` with `score:{create:1}` and no other create flag (the clinic readback; read) |
| `academy_exam_paper_review` / `_save` | FA or unit-exam paper, two steps: creation, then assignment | `/servlet/controller.tutor.fa.TestPageRegServlet` (creation) and `/servlet/controller.tutor.fa.TestPageListGrpServlet` (assignment); the unit-exam family's paths are pinned by the unit that builds it |

`*_save` takes exactly `reviewToken`, `userApproved` and the reviewed change; `*_review` takes
the change and its scope (student or class keys, book and section keys, or paper keys).

### Request classes

A request is classed read or write by its named route and operation, as
`apps/academy/dc-gang-a/src/lms/lmsRouteRegistry.ts` and
`apps/academy/dc-gang-a/src/lms/endpointCandidates.ts` class them, never by its HTTP method: a
GET can write and a POST can read. A route that neither classes is classed write until the
unit that adds it classes it.

`POST /servlet/controller.common.TestpageSelectExServlet` is classed by its operation (`p_process`) and, for the three
score and clinic operations, by its `condition` form member
(`testpageSelectExRequestEffect` in `lmsRouteRegistry.ts`, mirrored in the fixture classifier
`tests/network-deny.ts`): `getWrongAnswerNoteResult` and `getTestExamList` are read;
`getStudyResultSingleTestingSingleUser`, `getStudyResultSingleTestingMultiUser` and
`getStudyResultMultiTestingMultiUser` are read only when `condition` is present, parses to a JSON
object and none of `incorrect.create`, `similar.create`, `advance.create` equals 1 (the score
views send `score:{create:1}` and the other flags 0), and write when any of them equals 1 or when
`condition` is absent or malformed. The two pages `GET …TextBookManageServlet?reqCmd=getTextBookMain`
and `GET …tutor.fa.TestPageListGrpServlet?p_process=Main` are read.

### Wire hash

`wireSha256` is the SHA-256 of the page's write program: the source text of the functions
the write calls, line ends normalized to `\n`, joined by `\n`, as the CISM write computes it.
The value for each page is pinned by the unit that implements its tool, in the tool's source
and its test; this contract pins the computation, not the values.

For the clinic tools the program hashed is the page's own inline handler, selected with
`selectSourceFunction` from the page text (`\r\n` normalized to `\n`): `createBookIncorrectPapers` on
the `getTextBookMain` page (`4794a5e8353e93f481d3f5cb4bc448c42e0b314413e55068900d51165893bf8c`) and
`externOnCreateOneClinicPaper` on the tutor.fa `TestPageListGrpServlet` `Main` page
(`3f726b828586a15d3ed7c2071e63207a54cec231f168e1f838ffdae0f75447fa`). Both call
`MODULE.Typeset.createResultPaper` and `CLINIC_MODE` of the static `/jsutil/module.js`, which is
not fetched again on each call, so that R8's one pre-write read holds: a change of that static file is
not seen by the wire hash. This gap is accepted. A test may replace the two pinned values through the
`expectedWireHashes` option of the service so that synthetic pages exercise every path; a test pins
the two production values themselves.

### Attempt ledger

One private file of the server's owner (mode 0600), outside the repository, set by the launch
argument `--gated-write-ledger <absolute path>` or else the variable `ACADEMY_GATED_WRITE_LEDGER_PATH`;
there is no default path. A path that is unset, empty, relative, not normalized, inside the repository,
a symlink, readable or writable by group or others, in a directory others can write, or that cannot be
written is `ledger_unavailable`, before any HTTP request. Every record is one JSON line, appended and
synced to disk before the request it guards, and holds keys and hashes only:

- `reviewed` (written by a review): `tool`, `reviewToken`, `changeSha256`, `studentKey`, `scopeKey`, `wireSha256`, `reviewedAt`, `expiresAt`.
- `attempt` (written by a save at each step, and by a review that reconciles): `attemptId`, `tool`, `reviewToken`, `changeSha256`, `studentKey`, `scopeKey`, `step` (`before_write`, `write_dispatched`, `response_received`, `reconciled`), `responseSha256` and `paperKeys` where known, `state` (`open`, `persisted`, `partially_persisted`, `absent`), `at` and `runId`.

The `tool` of both tools of a pair is one family name (`academy_clinic_paper`), so a token of the
other family is `review_token_invalid`. A token is used once: any `attempt` record that carries it
uses it. The attempt of a call is the records of its `attemptId`; its state is that of its last record.

A call takes `<ledger path>.lock` by an exclusive create (`wx`, mode 0600) holding `{pid, runId, at}`
and removes it when it ends. A lock whose process is alive (`kill(pid, 0)` succeeds or is refused for
permission) is a call still running: `attempt_already_open`, 0 HTTP requests. A lock of a dead process, or one that
cannot be read and is older than 5 seconds, is stale: it is removed, and a save then refuses with
`reconcile_blocks_retry` when the ledger holds an unresolved attempt of that lock's `runId` (of any
attempt when the lock cannot be read) or of its own student and scope, else it goes on; a review takes the lock itself.

### Codes

| Code | When |
| --- | --- |
| `wire_hash_mismatch` | R1, R6 |
| `user_approval_required` | R3 |
| `review_token_invalid` | R4 |
| `review_token_expired` | R5 |
| `public_scope_forbidden` | R7 |
| `owner_account_busy` | R9 |
| `ledger_unavailable` | R10 |
| `attempt_already_open` | R11 |
| `reconcile_blocks_retry` | R11, R12 |
| `clinic_input_invalid` | a review input that fits no mode or kind (a count out of range, `patList` on `multiply` or `divide`, a member of the other kind), R15 |
| `clinic_scope_unresolved` | the wrong-note list of a book is empty, holds another student or course, holds more than 50 testing numbers, or lacks a requested testing number |
| `clinic_response_unverified` | `unverified` after a write whose answer lacks `result: 'OK'` and a valid non-empty `print_list` |
| `clinic_readback_unverified` | `unverified` after a write whose read-only scope readback failed |

### Clinic and wrong-note papers

`academy_clinic_paper_review` takes `kind` (`book_incorrect` or `clinic`) and `studentKey`, and by kind:

- `book_incorrect`: `courseKey`, `bookNo`, and optionally `testingNos` (positive numeric keys; default every testing number of the student's wrong-note list for that course and book, read by `getWrongAnswerNoteResult` with `answerNoteResult={courseKeyList:[courseKey],bookNoList:[bookNo],userKeyList:[studentKey]}`). `scopeKey` is `book_incorrect:<courseKey>:<bookNo>`. The change is `{kind, studentKey, courseKey, bookNo, testingNos}` with the testing numbers sorted and frozen. The write is one `POST` of `getStudyResultSingleTestingSingleUser` with `condition = {testing_no, pri_no, student_name: '', score:{create:0}, incorrect:{create:1}, similar:{create:0}, advance:{create:0}, report:{create:0}}` for one testing number, or of `getStudyResultMultiTestingMultiUser` with the same flags and `user_list:[{pri_no, student_name: '', testing_list:[{testing_no}…]}]` for several. The readback is `getWrongAnswerNoteResult` again, which must answer `result: 'ok'` and still hold every reviewed testing number.
- `clinic`: `testingNo`, `clinicMode` (`multiply`, `divide` or `custom`, the values of `CLINIC_MODE` in `/jsutil/module.js`) and at least one of `createSimilar: true` and `createAdvance: true`. `scopeKey` is `clinic:<testingNo>`. A flag that is false or omitted is `{create:0}` with no other member. `multiply`: similar `{create:1, mode:'multiply', exam_cnt: similarCount ?? 1}` (1 to 20), advance `{create:1, mode:'multiply', exam_cnt:1}` (`advanceCount`, when given, must be 1). `divide`: similar `exam_cnt: similarCount ?? 10` and advance `exam_cnt: advanceCount ?? 10` (each 1 to 100). `custom`: similar `{create:1, mode:'custom', exam_cnt:0, pat_list}` with `patList` of 1 to 100 items `{pat_no ≥ 1, d4_cnt, d3_cnt, d2_cnt, d1_cnt}` (each count an integer from 0 to 100, their sum at least 1, `pat_no` unique), advance `{create:1, mode:'custom', exam_cnt:1}`. `patList` on another mode, or a count for a flag that is off, is refused. The write is one `POST` of `getStudyResultSingleTestingSingleUser` with `condition = {testing_no, pri_no, student_name: '', score:{create:0}, incorrect:{create:0}, similar, advance, report:{create:0}}`; the readback is the same operation with `score:{create:1}` and every other flag `{create:0}`, which must answer `result: 'OK'` and a non-empty `print_list` of the student.

`student_name` is `''` in every request: no name is sent or kept. R7 refuses, with 0 HTTP requests, a `studentKey` that is absent, not a positive number, `0` or `all`, a target key (`courseKey`, `bookNo`, `testingNo`) that is absent or not a positive number, and any member whose name says public, all, everyone, academy or school wide, or pool.

The write answers `{result: 'OK', print_list: [...]}`. R14 for these tools: a save is `saved` (with `paperKeys` where the papers carry `p_no` or `paper_no`, and `printListSha256`) only when that answer holds a non-empty `print_list` whose papers each hold `exam_list` items with an `exam_no`, and the read-only scope readback succeeded; otherwise it is `unverified` and the attempt stays `open`.

The LMS lists no previously created clinic or wrong-note papers (the printed-check state of the browser is local to it), so R12 can decide an unresolved attempt only from its last recorded step and a read-only scope readback, never from the papers themselves. When the readback succeeds: a last step `before_write` is `absent` (no write request was sent) and the review answers a fresh `reviewToken`; a last step `response_received` is `persisted` (the write was answered with a valid `print_list`) and the review answers done with nothing to save; a last step `write_dispatched` is indeterminate (the write request may or may not have made a paper). When the readback fails, every case is indeterminate. An indeterminate attempt stays `open` and the review refuses with `reconcile_blocks_retry`, the same code that a save gives. There is no automatic way out of it: the teacher inspects the LMS by hand, and the operator resolves the attempt in the private ledger file; no call retries a write blindly.

### Limits

| Limit | Value |
| --- | --- |
| Life of a `reviewToken` | 15 minutes |
| Wait for the account lock | 45 seconds |
| A page read | 2 MiB |

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/gated-writes.test.ts`, with a mock
transport that counts requests by route class (read or write) and by method, a scratch ledger
directory, synthetic pages and invented keys only. No test contacts the LMS. The synthetic LMS is
`tests/gated-write-fixture-lms.ts`; `tests/gated-write-fixture.ts` serves the audited clinic tools from it
as a dailyrecord endpoint, for the tests through the bridge (R9, and the write receipts of R10).

## Not included

- Deleting or changing a paper once created, and unassigning it.
- Printing the paper; that is `academy_paper_print_pdf_get`.
- Choosing which sections or items a clinic should hold; the review takes the change as given.
- The DailyRecord and CISM writes, which keep their own contracts.
