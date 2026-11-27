# Gated Academy writes: `academy_clinic_paper_{review,save}` and `academy_exam_paper_{review,save}`

## Purpose

Making a wrong-note or clinic paper for a student, and creating and assigning a formative
assessment (FA) or unit-exam paper, are Academy writes that the teacher does by hand today.
This contract lets the TypeScript server do them in two phases, `review` then `save`, in the
pattern of the CISM write (`apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts`): the review
reads the live page, pins its form by `wireSha256` and returns a `reviewToken`; the save
writes only that reviewed change, only with the teacher's explicit approval, records each
attempt before it posts, and reads the result back. It is relied on by the teacher, by the
skills that call the tools ([tool-surface.md](tool-surface.md)), and by the units that build
and check the writes. Every refusal sends no POST.

## Requirements

Review

- R1: `academy_clinic_paper_review` and `academy_exam_paper_review` send GET requests only (0 POSTs): they read the live page of the write, compute `wireSha256` over its write program (Interface, "Wire hash"), and refuse with `wire_drift` when it differs from the value this contract pins for that page. (no test yet; observed by the unit that builds the gated writes)
- R2: A review that passes answers `status: 'reviewed'`, the exact change it would make, and a `reviewToken` that is the SHA-256 of the canonical JSON of the change, the student and scope keys, the `wireSha256` and the review's expiry time, so the same review of the same page at the same expiry gives the same token; a token expires 15 minutes after its review. (no test yet; observed by the unit that builds the gated writes)

Save

- R3: A `*_save` call without `userApproved: true` (absent, `false` or any other value) refuses with `approval_required`. (no test yet; observed by the unit that builds the gated writes)
- R4: A `*_save` call whose `reviewToken` is unknown, expired, already used, or was made by the other tool, or whose change differs from the reviewed change in any member, refuses with `review_token_invalid`. (no test yet; observed by the unit that builds the gated writes)
- R5: Before its POST a `*_save` call reads the live page again and refuses with `wire_drift` when its `wireSha256` differs from the reviewed one. (no test yet; observed by the unit that builds the gated writes)
- R6: A change that would publish a paper school-wide (to every class or every student of the academy, or to the public pool) refuses with `public_publish_refused`, at review and at save; a paper is created and assigned only to the students and classes the change names. (no test yet; observed by the unit that builds the gated writes)
- R7: The requests of a save are only: the pre-write GET of R5, the single write POST of each step of the reviewed change (one step for a clinic paper; for an exam paper the creation step, then the assignment step), and the post-write readback GET of R12, all on the routes of the table "Tools". (no test yet; observed by the unit that builds the gated writes)

Lock and ledger

- R8: Before its first LMS request a review or save takes the shared account lock of `docs/academy-account-lock.md` with `/usr/bin/flock --nonblock` and holds it until its last; when the lock is busy the call ends with a tool error carrying `owner_account_busy`, and 0 HTTP requests are sent. (no test yet; observed by the unit that builds the gated writes)
- R9: A save records an attempt in the attempt ledger (state `open`, the `reviewToken`, the change's hash, the time) and makes that record durable (written and synced to disk) before it sends a POST, and records each step's POST before it is sent; a save that cannot record the attempt refuses with `ledger_unavailable`. (no test yet; observed by the unit that builds the gated writes)
- R10: A save of a change for which the ledger holds an `open` attempt of the same student and scope refuses with `attempt_open`, whether the attempt is of a running call or of one that crashed. (no test yet; observed by the unit that builds the gated writes)
- R11: An `open` attempt left by a crash is resolved only by a read-only readback of the server: the reconcile sends GETs only, marks the attempt `persisted` when the readback shows the change, `absent` when it shows the page without it, and leaves it `open` with `readback_unresolved` otherwise; no retry of the write is accepted before the attempt is `absent`, and a retry is a new review. (no test yet; observed by the unit that builds the gated writes)
- R12: After its POST a save reads the page back and answers `status: 'saved'` only when the readback shows the reviewed change; otherwise it answers `status: 'unverified'`, keeps the attempt `open`, and sends no further POST (an exam paper whose creation is not read back is not assigned). (no test yet; observed by the unit that builds the gated writes)

Refusals and privacy

- R13: Every refusal of R1, R3 to R6, R8, R10 and of an invalid input sends 0 HTTP POSTs; the refusals of R3, R4, R6 when found from the input alone, and R8, send 0 HTTP requests at all. (no test yet; observed by the unit that builds the gated writes)
- R14: No answer, error, ledger record or log line holds a student name, record text, cookie or password; the ledger holds keys and hashes only. (no test yet; observed by the unit that builds the gated writes)

## Interface

### Tools

| Tool | Write | Routes |
| --- | --- | --- |
| `academy_clinic_paper_review` / `_save` | wrong-note and clinic paper of a book section | the LMS commands `createBookIncorrectPapers` and `CreateClinic`; their full paths are pinned by the unit that builds them |
| `academy_exam_paper_review` / `_save` | FA or unit-exam paper: creation and assignment | `/servlet/controller.tutor.fa.TestPageRegServlet` (creation) and `/servlet/controller.tutor.fa.TestPageListGrpServlet` (assignment); the unit-exam family's paths are pinned by the unit that builds it |

`*_save` takes exactly `reviewToken`, `userApproved` and the reviewed change; `*_review` takes
the change and its scope (student or class keys, book and section keys, or paper keys).

### Wire hash

`wireSha256` is the SHA-256 of the page's write program: the source text of the functions
the write calls, line ends normalized to `\n`, joined by `\n`, as the CISM write computes it.
The pinned value of each page is part of the tool's source and of its test, and changes only
with this specification.

### Attempt ledger

One private file of the server's owner (mode 0600), one JSON line per state change, holding
`attemptId`, `tool`, `reviewToken`, `changeSha256`, the student and scope keys, `state`
(`open`, `persisted`, `absent`) and the time. It is outside the repository.

### Codes

| Code | When |
| --- | --- |
| `wire_drift` | R1, R5 |
| `approval_required` | R3 |
| `review_token_invalid` | R4 |
| `public_publish_refused` | R6 |
| `owner_account_busy` | R8 |
| `ledger_unavailable` | R9 |
| `attempt_open` | R10 |
| `readback_unresolved` | R11 |

### Limits

| Limit | Value |
| --- | --- |
| Life of a `reviewToken` | 15 minutes |
| A page read | 2 MiB |

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/gated-writes.test.ts`, with a mock
transport that counts requests by method, a scratch ledger directory, synthetic pages and
invented keys only. No test contacts the LMS.

## Not included

- Deleting or changing a paper once created, and unassigning it.
- Printing the paper; that is `academy_paper_print_pdf_get`.
- Choosing which sections or items a clinic should hold; the review takes the change as given.
- The DailyRecord and CISM writes, which keep their own contracts.
