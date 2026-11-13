# Explicit textbook capture and owner preparation

No command discovers credentials or grants a login window. The Lead supplies
explicit private paths; preparation does not log in or open the shared lock.

Prepare a NEW canonical owner root (must not already exist):
```sh
academy-capture prepare-owner --identity "$IDENTITY" --secret-env "$SECRET_ENV" --secret-key DC_ACADEMY_HTTP_PASSWORD --lock "$SHARED_LOCK" --students "$STUDENTS" --paper-form "$PAPER_FORM" --allowed-from 2026-11-13 --allowed-through 2026-11-15 --expires-at 2026-11-15T14:10:00Z --owner-root "$OWNER_ROOT" > "$PREP_RESULT"
```
Identity JSON supplies teacherSourceKey, teacherLabel, collegeSourceKey and
sourceUsername (other keys are ignored). Students JSON is an array of
`{"student_id":"77001","student_name":"Invented Learner"}` (invented example).
Paper-form JSON is the independent owner-published academy.paper-search-form/1
pin: action, method and fields (name/tag/type/value/checked/options).
The env-style input selects exactly one KEY=value line. One matching outer quote
pair is removed; no shell expansion occurs; missing/duplicate/empty keys refuse.
Inputs are bounded regular single-link files owned by the caller, at absolute
explicit paths; symlink leaves refuse. No input payload is echoed on failure.
Expiry must be future RFC3339 and at most UTC midnight after allowedThrough.
An empty/duplicate roster, invalid pin/date scope or noncanonical/missing/nonregular
lock refuses. The shared lock is never created, changed or acquired by preparation.

The command writes four 0600 files inside a new 0700 root with trusted ancestors:
- account-lock.json: academy.account-lock/1, path to the existing shared lock;
  it must be the TypeScript launcher's SAME file (academy-account-lock.md).
- credential.json: academy.owner-credential/1, username/password/expiresAtMs.
- transport-admission.json: academy.transport-admission/1, teacher/college/date/
  student scope; operations are exactly capture::contracts(), all reads.
- roster.json: students plus paper_form, consumed unchanged by textbook capture.
Files use exclusive no-follow publication and typed readback. Any failure removes
only the newly created files/root; an existing root is retained without changes.
Output contains paths, file hashes/byte counts, student/operation counts and expiry.

First capture (one book; explicitly authorized by the Lead):
```sh
academy-capture textbook --owner-root "$OWNER_ROOT" --admission transport-admission.json --admission-sha256 "$(jq -r '.documents["transport-admission.json"].sha256' "$PREP_RESULT")" --credential credential.json --credential-sha256 "$(jq -r '.documents["credential.json"].sha256' "$PREP_RESULT")" --roster "$OWNER_ROOT/roster.json" --spool "$SPOOL" --date 2026-11-13 --max-requests 5 --no-detail
```
One NativeSession retains the account lease; all source reads are sequential.
One book without detail costs 1 binding + 2 index + optionally 2 paper-search
requests. More books need an explicit larger budget. Login exchanges are extra,
once per run. Detail costs 4 + title count per batch of at most 12, per book.
The budget excludes login. Reports identify students by one-based roster_ordinal;
stdout contains only fixed codes and counts. Ungraded counts/score are null,
wrong_ordinals is absent, graded=false. Stable keys use student|book|attempt.
Ordinals follow source order (one-based); hint is the first index title or null.
Read/batch failure, dropped paper rows (capture_paper_row_dropped) or a source
request/recorder mismatch (capture_raw_request_mismatch) prevents publication.
Both partitions are staged/parsed before either rename. A crash/I/O failure
between renames can expose different generations; no multi-file transaction.

Optional --raw-dir "$RAW_DIR" saves source response bytes before parsing.
The Lead pre-creates it privately outside Git; OwnerRoot checks path/ancestors.
Private per-student body files and index.jsonl retain operation, rendered query/
body parameters, status, size, hash and read second. Sequences continue across runs.
Authentication finishes before the recorder arms; login/header/cookie/credential
bytes are excluded. No raw directory/body is created without this flag.
Run only one capture writer per raw directory. Keep malformed bodies for repair.
The real structural form pin is NOT supplied by repository fixtures; see the
owner unit's report for the retained TypeScript source and confirmation boundary.
