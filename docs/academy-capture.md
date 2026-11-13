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
The secret file is not shell input: `#` after an unquoted value stays in the
value, backslashes are not unescaped, outer whitespace (including a trailing CR)
is trimmed before removing one matching outer quote pair, a quote-prefixed value
without the same final quote is refused, and any CR remaining inside the value
is refused.
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
After a kill between writes, or when only final stdout fails, the owner root
remains; the Lead removes it before a new run.

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

Propose a candidate form pin from an explicitly saved UTF-8 page, offline:
```sh
academy-capture propose-paper-form --body "$SAVED_BODY" --out "$NEW_CANDIDATE"
```
This command opens no credential, account lock or owner root and makes no request.
It uses the crate DOM to pin action (null means the attribute is absent), method,
and every named input/select (10 required names, at most 64 unique names).
Tag/type, college value, checkbox values/states, select values and all option
values/labels are pinned; other field values are null. The same parser and
verifier must accept the candidate against that page before a new 0600 file is
created exclusively; existing output is never overwritten. A page with a
missing/extra control or a difference in any pinned property is refused before
search. Additional pinned fields are verified but never sent by search.
Stdout/stderr expose only fixed codes, counts and booleans; candidate content
stays in the explicit output file. Proposal is not publication: the owner decides
whether to publish the candidate as the independent comparison pin.

Replay recorded answers offline (the spool must not exist):
```sh
academy-capture replay --raw-dir "$RAW_DIR" --students "$STUDENTS" --identity "$IDENTITY" --paper-form "$PAPER_FORM" --spool "$NEW_SPOOL" --no-detail --max-requests 13
```
The identity supplies the same four keys as prepare-owner. No credential,
account lock, owner root or network is opened. Raw files are read only.
Optional --date selects the local admission date (default 2000-01-01); it does
not filter saved responses. Each dispatched operation is rendered with the
same contracts and answered by the latest sequence with identical operation,
query and body parameters. The saved status is used; size and sha256 must match.
No match ends that student as replay_body_missing with operation and the count
of answered requests. Existing spool, missing index/body and bad digests refuse
with replay_* codes. The private new spool remains for inspection on refusal.
Recorder indexes do not retain response headers or redirect destinations;
replay reconstructs no such evidence and cannot verify header-dependent failures.
Both modes run the same student parser and partition publisher. Capture-path
paper metadata uses the TypeScript whole-page result-list expression; unrelated
script syntax and token counts cannot reject a matched literal. Extracted JSON,
row, identity, scope and form-pin checks remain in force.
Failed student reports carry cause: up to four safe fixed refusal codes from
inner to outer. Success has no cause. Neither cause nor missing-operation output
contains private values, labels, keys or paths.

From the spool to a read store: `ontology/academy-read.schema.json` (domain
`academy_read`, `edu_academy_ontology::ACADEMY_READ_SCHEMA`) declares one `records`
source per partition directory under the spool root `/var/lib/academy-read/spool`.
After a capture the spool is indexed with `ont index` into a store of its own;
`ont render` then answers, for one student id, the views `registered_books` and
`section_results` (each row with the time the source was read, `$observed_at`).
`observed_at` is not a stored property, so an unchanged record captured again
appends nothing and only moves that time; a record without a required member fails
its source and leaves the earlier rows. Rows turn stale 36 hours after the last
index run. The engine has no override of the glob root: a test rewrites the root
string in the schema text before installing it into a temporary store. The schema
carries no student name; a student is identified by id only.
