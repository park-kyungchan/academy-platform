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

- R1: `academy_clinic_paper_review` and `academy_exam_paper_review` send only requests whose route is classed read (Interface, "Request classes"), 0 write requests: they read the live page of the write, compute `wireSha256` over its write program (Interface, "Wire hash"), and refuse with `wire_hash_mismatch` when it differs from the value pinned for that page. (no test yet; observed by the unit that builds the gated writes)
- R2: A review that passes answers `status: 'reviewed'`, the exact change it would make, and a `reviewToken` that is the SHA-256 of the canonical JSON of the change, the student and scope keys, the `wireSha256` and the review's expiry time, so the same review of the same page at the same expiry gives the same token; a token expires 15 minutes after its review. (no test yet; observed by the unit that builds the gated writes)

Save

- R3: A `*_save` call without `userApproved: true` (absent, `false` or any other value) refuses with `user_approval_required`. (no test yet; observed by the unit that builds the gated writes)
- R4: A `*_save` call whose `reviewToken` is unknown, already used, or was made by the other tool, or whose change differs from the reviewed change in any member, refuses with `review_token_invalid`. (no test yet; observed by the unit that builds the gated writes)
- R5: A `*_save` call whose `reviewToken` is more than 15 minutes old refuses with `review_token_expired`. (no test yet; observed by the unit that builds the gated writes)
- R6: Before each write a `*_save` call reads the live page again and refuses with `wire_hash_mismatch` when its `wireSha256` differs from the reviewed one. (no test yet; observed by the unit that builds the gated writes)
- R7: A change that would publish a paper school-wide (to every class or every student of the academy, or to the public pool) refuses with `public_scope_forbidden`, at review and at save; a paper is created and assigned only to the students and classes the change names. (no test yet; observed by the unit that builds the gated writes)
- R8: The requests of a save are only login requests, optionally one initial before-state read, and, for each step of the reviewed change (one step for a clinic paper; for an exam paper the creation step, then the assignment step), at most one pre-write read that yields the `wireSha256` (R6), one write request and one readback read (R14); every request is on the routes of the table "Tools", and a save sends no browse query. (no test yet; observed by the unit that builds the gated writes)

Lock and ledger

- R9: Before its first LMS request a review or save takes the shared account lock of `docs/academy-account-lock.md`, waiting up to 45 seconds, and holds it until its last request; when the lock is still held after 45 seconds the call ends with a tool error carrying `owner_account_busy`, and 0 HTTP requests are sent. (no test yet; observed by the unit that builds the gated writes)
- R10: A save records an attempt in the attempt ledger (state `open`, the `reviewToken`, the change's hash, the time) and records each step before that step's write request, each record made durable (written and synced to disk) before the request is sent; when a record cannot be made the save refuses with `ledger_unavailable` and sends no further write request. (no test yet; observed by the unit that builds the gated writes)
- R11: A save of a change for which the ledger holds an `open` attempt of the same student and scope whose call is still running refuses with `attempt_already_open`; one whose call has ended without resolving it (a crash, or R14's `unverified`) refuses with `reconcile_blocks_retry` until a review has reconciled it (R12). (no test yet; observed by the unit that builds the gated writes)
- R12: A review of a change with an unresolved `open` attempt first reconciles it by a read-only readback of the server, with requests classed read only: it marks the attempt `persisted` when the readback shows every step, `partially_persisted` when it shows the exam paper created but not assigned, and `absent` when it shows no step; a readback that fits none of these leaves the attempt `open` and the review refuses with `reconcile_blocks_retry`. (no test yet; observed by the unit that builds the gated writes)
- R13: After reconciliation, a `persisted` attempt is answered as done and offers nothing to save; a `partially_persisted` exam attempt is answered with a review of the assignment step alone on the paper already created, whose save sends the requests of that step only and never creates a second paper; an `absent` attempt is answered with a review of the whole change, under a new `reviewToken`. (no test yet; observed by the unit that builds the gated writes)
- R14: After each write a save reads the page back and answers `status: 'saved'` only when the readback shows every step of the reviewed change; otherwise it answers `status: 'unverified'`, keeps the attempt `open`, and sends no further write request (an exam paper whose creation is not read back is not assigned). (no test yet; observed by the unit that builds the gated writes)

Refusals and privacy

- R15: Every refusal before a write (R1, R3 to R7, R9, R11, R12, an invalid input, and `ledger_unavailable` of a step not yet written) sends 0 write requests, of any HTTP method; the refusals found from the input alone (R3, R4, R5, R7) and R9 send 0 HTTP requests at all. (no test yet; observed by the unit that builds the gated writes)
- R16: No answer, error, ledger record or log line holds a student name, record text, cookie or password; the ledger holds keys and hashes only. (no test yet; observed by the unit that builds the gated writes)

## Interface

### Tools

| Tool | Write | Routes |
| --- | --- | --- |
| `academy_clinic_paper_review` / `_save` | wrong-note and clinic paper of a book section, one step | the LMS commands `createBookIncorrectPapers` and `CreateClinic`; their full paths are pinned by the unit that builds them |
| `academy_exam_paper_review` / `_save` | FA or unit-exam paper, two steps: creation, then assignment | `/servlet/controller.tutor.fa.TestPageRegServlet` (creation) and `/servlet/controller.tutor.fa.TestPageListGrpServlet` (assignment); the unit-exam family's paths are pinned by the unit that builds it |

`*_save` takes exactly `reviewToken`, `userApproved` and the reviewed change; `*_review` takes
the change and its scope (student or class keys, book and section keys, or paper keys).

### Request classes

A request is classed read or write by its named route and operation, as
`apps/academy/dc-gang-a/src/lms/lmsRouteRegistry.ts` and
`apps/academy/dc-gang-a/src/lms/endpointCandidates.ts` class them, never by its HTTP method: a
GET can write and a POST can read. A route that neither classes is classed write until the
unit that adds it classes it.

### Wire hash

`wireSha256` is the SHA-256 of the page's write program: the source text of the functions
the write calls, line ends normalized to `\n`, joined by `\n`, as the CISM write computes it.
The value for each page is pinned by the unit that implements its tool, in the tool's source
and its test; this contract pins the computation, not the values.

### Attempt ledger

One private file of the server's owner (mode 0600), one JSON line per state change, holding
`attemptId`, `tool`, `reviewToken`, `changeSha256`, the student and scope keys, the steps
recorded, `state` (`open`, `persisted`, `partially_persisted`, `absent`) and the time. It is
outside the repository.

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

### Limits

| Limit | Value |
| --- | --- |
| Life of a `reviewToken` | 15 minutes |
| Wait for the account lock | 45 seconds |
| A page read | 2 MiB |

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/gated-writes.test.ts`, with a mock
transport that counts requests by route class (read or write) and by method, a scratch ledger
directory, synthetic pages and invented keys only. No test contacts the LMS.

## Not included

- Deleting or changing a paper once created, and unassigning it.
- Printing the paper; that is `academy_paper_print_pdf_get`.
- Choosing which sections or items a clinic should hold; the review takes the change as given.
- The DailyRecord and CISM writes, which keep their own contracts.
