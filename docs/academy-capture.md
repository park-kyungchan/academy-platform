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

## DailyTest history (family `dailytest`)

`prepare-owner` lists the family's operation only when asked: `--families textbook,dailytest`
(comma list of `textbook`, `dailytest` and `dailyrecord`, no repeat; default `textbook`). The document written
for `textbook` alone is byte-identical to the one written before the family existed; `dailytest`
adds the one read `metadata.daily`. A list that names an unknown or repeated family refuses with
`prepare_input_invalid` before any file exists.

```sh
academy-capture dailytest --owner-root "$OWNER_ROOT" --admission transport-admission.json --admission-sha256 "$ADM_SHA" --credential credential.json --credential-sha256 "$CRED_SHA" --roster "$OWNER_ROOT/roster.json" --spool "$SPOOL" --date 2026-11-13 --max-requests 8 --raw-dir "$RAW_DIR"
```
The mode takes the arguments of `textbook` except `--no-detail`. It has no form pin: the
roster's `paper_form` is read and ignored. One login, sequential requests, `--max-requests` per
student, the admission checked before any request, a partition replaced only after the whole
read of that student parsed back, the recorder writing each answer before any parser: all by the
same code as `textbook` (`capture::run_students`). Per student: one request for the selector
`1` (its page lists the periods and is also the period `1` page), then one request per other
offered selector among 11 to 16: at most 7. Enabled options outside {1, 11 to 16} are not read;
a page without the selector `1` among its options, with no single `select[name=sort_date]`, or
with a form-owned or duplicated option refuses (`daily_history_selector_drift`, `contract_drift`).
Rows come from the one whole-page `testResultList: JSON.parse('...')` expression (TypeScript
`inspectAssessmentSearchMetadata`); other script, regular-expression or template literals and
token counts never decide. A page the parser refuses keeps its saved answer for `replay`.

Publishes `<spool>/paper_attempt/<student_id>.jsonl`, keys sorted: `type`, `key`, `observed_at`,
`student_id`, `student`, `attempt_key`, `paper_key`, `period` (the selector literal),
`period_ordinal` (position among the enabled options, from 1), `row_ordinal` (position among the
student's rows of that page, from 1), `source_row_id` (string or null), `source_score` (the
number printed as the JavaScript engine prints it, as a string, or null). `key` is
`<student_id>|<attempt_key>`; an attempt key that occurs more than once in one read (another
period or another paper) is `<student_id>|<attempt_key>|<paper_key>|<period>` on every
occurrence. No name, paper body or question text. The report line of a student adds
`paper_attempts`; its `registered_books` and `section_attempts` are 0. Headers (title,
category, question count) are not read: they need the six owner pins of the paper route.

Replay: `replay --family dailytest --raw-dir ... --students ... --identity ... --spool ...
--max-requests N` answers the same reads from the raw directory (no `--paper-form`, no
`--no-detail`); a missing answer ends the student `replay_body_missing` with the operation id.
A raw directory may hold the answers of both families for a student.

From the spool to a read store: `ontology/academy-read.schema.json` (domain
`academy_read`, `edu_academy_ontology::ACADEMY_READ_SCHEMA`) declares one `records`
source per partition directory under the spool root `/var/lib/academy-read/spool`.
After a capture the spool is indexed with `ont index` into a store of its own;
`ont render` then answers, for one student id, the views `registered_books` and
`section_results` (each row with the time the source was read, `$observed_at`).
`observed_at` is not a stored property, so an unchanged record captured again
appends nothing and only moves that time. Only a member the row key needs fails
a source: a `registered_book` line without `student_id` or `book_no` fails its
source (`key_unresolved`, nothing of that run is appended, earlier rows stay); a
`section_attempt` line without `key` fails the same way. A line missing any
other member indexes with the source `ok`: its cell renders `(missing)`, except
`student_id` on a `section_attempt` line, which both views filter on, so that
row appears in neither view and nothing marks it (the capture always writes
`student_id`). An ungraded section has no counts, no wrong list and may have no
date literal, and renders `(missing)` there. The record's `score` is not mapped
(the engine has no type for a fractional JSON number) and `section_results` has no
score column. Rows turn stale 36 hours after the last index run, not after the
capture's age: the `Observed at` column shows the capture's time and is never
marked stale, so lesson preparation reads that date. Each type is ONE source for
every student, so one record the schema refuses (a value over its `max_bytes`, a
changed field with an earlier `observed_at` than the stored one, an unreadable
line) fails that type's whole run for all students; on a first run the view then
shows a header with no rows. The engine has no override of the glob root: a test rewrites the root
string in the schema text before installing it into a temporary store. The schema
textbook types carry no student name: a student is identified by id only.

## The lesson record's lines (contract of the family `dailyrecord`)

`academy-capture dailyrecord` writes these lines. The read schema (version 2) declares
their two types and four views; the capture tests build the two lines pinned by `b7`
from an invented page.

Prepare with `--families dailyrecord` (or include it in the comma list). Preparation
adds `academy.lms.dayrecord` to the admission namespaces and the exact read operation
`student.day.groups`; it keeps the same four private documents and roster. The roster
limits storage, not the page read. Its `paper_form` is ignored by this family.

```sh
academy-capture dailyrecord --owner-root "$OWNER_ROOT" --admission transport-admission.json --admission-sha256 "$ADM_SHA" --credential credential.json --credential-sha256 "$CRED_SHA" --roster roster.json --roster-sha256 "$ROSTER_SHA" --spool "$SPOOL" --dates 2031-03-03,2031-03-05 --raw-dir "$RAW_DIR"
```

`--dates` is a comma list of 1 to 31 distinct real, zero-padded dates. Document
hashes and date syntax are checked before login. The roster path is relative to the
owner root, or an absolute path within it. One NativeSession holds the existing
account lease for the run. Each date makes one `student.day.groups` request with
`std_ymd` and no group parameter, scoped to the roster's first student and that date.
Both admission authorization and exact operation-contract checks precede every
request. `--max-requests` is a run-wide budget, defaulting to the number of dates.
The existing optional raw recorder stores each answer, including malformed pages,
under that first student's raw partition before parsing.

Stdout is one JSON object with `dates`: each entry has `date`, `status` (`published`,
`refused`, `not_run`), `code` (null unless refused), `rows`, `stored`, `outside_roster`,
`cut_fields` and `rows_before`. A refused date stops the run with exit code 2;
earlier dates stay published. No names, record texts or credential values are reported.
Both private type directories are created before reading the first date, including
when its page or date scope refuses. Complete empty pages publish zero coverage.
The parser verifies the date, teacher, all-groups selection, closing document, table
columns and contiguous zero-based handler ordinals. It reads previous texts from
readonly textareas and the previous key from `openHomeWorkRate`, never a second request.
A row with neither an `openHomeWorkRate` handler nor an `h_homework_rate` control, whose previous date
cell and three previous texts are empty, is a student's first lesson: it is stored with those four
and `previous_homework_assessment` as empty strings and without `previous_record_key`. A row with only
one of the two, or with neither and a previous text, refuses its date with `capture_day_structure_invalid`.

Offline replay: `academy-capture replay --family dailyrecord --raw-dir "$RAW_DIR"
--students "$STUDENTS" --identity "$IDENTITY" --spool "$NEW_SPOOL" --dates 2031-03-03,2031-03-05`.
It needs neither a paper form nor `--no-detail`, uses the first roster student's raw
partition, verifies each matched body's size and digest, and preserves its recorded
read second. The parser and date publisher are the same as capture's.

The publisher holds `<spool>/.lesson-record.lock` from reading existing partitions
through both renames. It validates old types, keys and observation times; stages and
reads back both candidates; checks the four source capacities for each type; and
renames rows before coverage. Existing lines of other dates retain their bytes.
An I/O failure between renames can leave older coverage, as rule 6 describes.
There is no multi-file transaction or automatic retention policy.

One line is one JSON object with its members sorted and no space, as the other families write.

`<spool>/lesson_record/<yyyy-mm>.jsonl`, type `lesson_record`: one row of the LMS day-record
page of all groups, which is one course membership of one student on one lesson date. Key:
`student_id`, `membership_key` and `lesson_date`. The page of all groups does not show
a row's group, so a line holds none. The record key is not part of the key: it is a
small number that names a record only together with the membership key, and a line may be
without one (whether the LMS shows a row before the row has a record key is not known, and
the key does not depend on it). Every member is on every line except `record_key` and
`previous_record_key`; a text the page shows empty is `""`. The numbers (`observed_at`,
`row_ordinal`, `cut_fields`, the counts of `lesson_day`) are JSON numbers and every other
member is a string. The engine fails the whole source on a line without one of the three key
members (`key_unresolved`) and on a member of the wrong JSON type (`wrong_value_type`). It
accepts a line without another member and renders that cell `(missing)`: a writer never
leaves one out.

| Member | Value | Bound |
| --- | --- | --- |
| `type`, `observed_at` | `lesson_record`; the time the page was read, in seconds | |
| `source_namespace` | `academy.lms.dayrecord`, as the ledger's `save_attempt` spells it | 64 |
| `student_id` | The student's id | 1 to 32 digits |
| `student_name` | The name the row showed | 200 |
| `lesson_date` | `YYYY-MM-DD` | |
| `row_ordinal` | The row's place on the page, from 0 | |
| `membership_key` | The row's course membership | 1 to 32 digits |
| `course_id` | The row's course key, as text | 32 |
| `course_label` | The course's name as the row showed it | 200 |
| `record_key` | The row's record key; left out while the LMS shows none | 32 |
| `attendance`, `attendance_label` | The selected option's value and its text | 16, 64 |
| `progress`, `homework`, `lesson_memo` | The row's three texts | 6000 each |
| `daily_test_assessment` | The value of the row's hidden control | 16 |
| `student_memo` | The memo the LMS keeps per student and course, shown on the row | 6000 |
| `previous_record_key` | Record key of the previous lesson of the membership; left out when the row shows none | 32 |
| `previous_lesson_date_literal` | The previous lesson's date as the row showed it; `""` when it showed none | 64 |
| `previous_progress`, `previous_homework`, `previous_lesson_memo` | The previous lesson's texts as the row showed them | 6000 each |
| `previous_homework_assessment` | The value of the row's hidden control: it assesses the homework of the record `previous_record_key`, and this line is its only home in the store | 16 |
| `cut_fields` | How many of the row's members were cut (rule 4) | |

`<spool>/lesson_day/<yyyy>.jsonl`, type `lesson_day`: one captured date. Key: `lesson_date`.
Members: `type`, `observed_at`, `source_namespace`, `lesson_date`, `row_count`
(rows the page held), `stored_count` (rows written as `lesson_record`),
`outside_roster_count` (rows of students the roster does not hold: counted, not stored),
`cut_fields`. A line that counts no row is a captured date that held none. A date with
neither a line nor a row was never captured. Rows without a line, or rows with a later
`observed_at` than their date's line, are a capture that stopped between the two files
(rule 6): the rows are the page's rows, and the counts are missing or those of the capture
before. In the store a third state exists: an index run whose `lesson_record` source failed
while `lesson_day` was indexed (the run's answer names the failed source and its exit code
is 2) leaves a coverage NEWER than its rows until a run indexes both. A capture that finds
a date unchanged still moves `Observed at` of its rows and of its line to the time of that
capture, so the coverage tells how old a date's rows are (`b11`).

Both directories exist, mode 0700, before an index run under version 2: a source whose
directory is missing fails with `glob_root_missing`, the run ends `source_failed` with exit
code 2, and the other sources are still indexed. The writer makes them. The engine does
not check a directory's mode: 0700 is the writer's duty, because the files hold students'
records.

Rules a writer of these two files keeps. The engine fails the WHOLE source, for every date,
on one refused line, and it retracts every row of a file that is gone, so each rule is
checked before a file is replaced:

1. A file is replaced by rename, and the read of the old file, the build of the new one and
   the rename happen under one lock of the spool. A capture of a date replaces that date's
   lines and keeps every other line of the file byte for byte. A file is never removed and
   never left empty while it held a line of another date: the engine retracts the rows of
   an emptied or absent file with status `ok`. A file holds lines of its own type only
   (`record_type_mismatch`).
2. No line replaces a line of the same key with a smaller `observed_at`
   (`record_older_than_head`), and no line carries a time later than the clock
   (`record_time_in_future`).
3. No key is on two lines (`duplicate_key`). Two rows of one key on a page are one line when
   every member but `row_ordinal` is equal, and a refusal of the date when they differ.
   `student_id` and `membership_key` are 1 to 32 ASCII digits each, so the key's separator
   `|` is never inside a part; a row whose id or membership is anything else refuses the
   date. The writer writes an id as the page holds it: `007` and `7` are two keys. The
   pattern of `lesson_date` checks the shape only (`2040-13-45` is indexed), so the writer
   writes only a date that exists.
4. Every string member has the bound of the table, in bytes, and the writer measures a
   string in the form the line holds it: the quote mark, the backslash and the control
   characters escaped, every other character as UTF-8. The engine checks the unescaped
   value against the same number, and a line and the observation the engine makes of it
   against 65,536 bytes, so 6000 quote marks pass the first check and fail the second.
   - The seven texts, the two names (`student_name`, `course_label`) and the two labels
     (`attendance_label`, `previous_lesson_date_literal`): a longer value is cut at a
     character boundary so that it ends with ` [cut: <n> bytes in the LMS]` (`n`: the bytes
     of the value as the page gave it) and fits its bound, and is counted in `cut_fields`.
   - The keys and the codes (`student_id`, `membership_key`, `course_id`,
     `record_key`, `previous_record_key`, `attendance`, `daily_test_assessment`,
     `previous_homework_assessment`) are never cut: a value over its bound refuses the date.
   With every string at its bound a line has about 43,300 bytes (`b8`).
5. An unreadable old file, a page that is not the asked date's page, or a page that is cut
   leaves both files as they were and ends with a refusal code. A page that is the asked
   date's page with fewer rows than before, or with none, is published as it is.
6. The `lesson_day` file is replaced after the `lesson_record` file, so a run that dies
   between the two leaves a coverage that is older than the rows, never newer.
7. The engine reads, in one run of one source, at most 200 files, 50,000 lines and 64 MiB of
   text, and no file over 16 MiB (`too_many_files`, `too_many_records`,
   `source_text_too_large`, `file_too_large`). Before the rename the writer counts the
   type's files as they will stand: over 190 files, 45,000 lines, 48 MiB in all or 12 MiB
   in the file it replaces, it refuses the date. What leaves the spool then is the owner's
   decision: a month that leaves is retracted in the store, where its history stays.

Views. `daily_records` (one date): the rows in the page's order WITHOUT their texts (id,
name, course, membership, attendance, daily test, the previous homework's rate and date),
at most 200; it is where a reader takes a student's id from, because `lesson_record` carries
the name its row showed and a view takes no name with Korean letters as an argument.
`daily_record` (one date and one student id): every member of that student's rows.
`record_history` (one student id and a date `since`): the rows from that date on, the
newest date first and the page's order inside a date, at most 24. `day_coverage` (one date):
its `lesson_day`.

Arguments of a view. The engine refuses an argument that breaks the value constraints of
the property it is compared with: a date that is not `YYYY-MM-DD` with its zeros (`2040-6-1`,
`20400102`, `2040`) for `lesson_date` or `since` (`b2`, `b11`), and a student id that is longer than
32 bytes (every view that takes one) or holds a letter (`daily_record` and `record_history`,
whose `student_id` is digits only by the schema's `key_digits`) (`b11`). The answer is exit code 2 with `status` `refused`,
`code` `invalid_argument`, and the members `field` (`/args/<name>`), `argument` (its name) and
`property` (the property it is compared with; for `since` it is `lesson_date`) (`b2`, `b11`).
So a date spelled without its zeros no longer reads as a date that was never captured, and
`since` no longer leaves out rows without a mark. The pattern checks a shape and not a
calendar: `2040-13-45` meets it and is answered (`b11`). Among well-formed dates `since` is
still compared with `lesson_date` as TEXT (`b11`). A view asked without one of its arguments
is refused (`invalid_argument`) as well (`b11`).

Bounds and sizes (`b11`). A view that reaches its bound says so in its last row
(`(+N more; limit L)`). A date with more than 200 stored rows shows 200; a row the table
does not show is still read by its student id through `daily_record`. With seven texts of
300 bytes on every row, `daily_record` of one row is 2.8 kB, `daily_records` of 20 rows
2.0 kB and `record_history` of 24 rows 24 kB; one row with its seven texts at their bound
is 43 kB.
A table is for reading: a line break in a text renders as a space, and a text equal to one
of the engine's marks (`(missing)`, a trailing ` (stale)`) cannot be told from the mark.
The exact bytes of a member are read with the engine's `get` or `search`.
