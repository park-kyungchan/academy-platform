# Textbook capture (explicit owner launch; reads only)

`academy-capture textbook` uses one NativeSession, sequential typed reads and the
existing account lease. It does not activate the installed server or grant login.
The Lead publishes these three private owner files (root 0700, files 0600):
- `account-lock.json`: `academy.account-lock/1`, `path` of the existing shared
  canonical lock; same file as TypeScript. See `academy-account-lock.md`.
- `credential.json`: `academy.owner-credential/1`, `username`, `password`,
  `expiresAtMs` (integer ms, in the future and no later than the authorized window).
- `transport-admission.json`: `academy.transport-admission/1`, camelCase fields
  of `owner_admission::TransportAdmission`: ownerId, teacherId, teacherLabel,
  collegeId, allowedFrom/allowedThrough (YYYY-MM-DD), studentIds,
  sourceNamespaces and operations. The owner's publisher uses
  `capture::contracts()` unchanged: student.books, textbook.wrongnote,
  textbook.detail, metadata.paper.initial, metadata.paper.search.7; all effect=false.
  Namespaces: academy.lms.student_course and academy.lms.metadata.
Admission and credential CLI hashes pin the exact published bytes.

The Lead also writes a private roster JSON file (0600) with `students`:
`[{"student_id":"77001","student_name":"Invented Learner"}]` (invented example),
and `paper_form`: the owner's `academy.paper-search-form/1` structural pin
(`student_metadata::PaperFormPin`), containing action, method, fields with
name/tag/type/value/checked/options. This pin cannot be inferred from a response.
Literal names and this pin are required by the existing typed code. No payload is
printed: reports identify students by one-based roster_ordinal, with status/counts.
The Lead must provide the correct form pin before the first live run.

First authorized run, one student in ROSTER (variables are explicit private paths
and lowercase SHA256 hashes supplied by the Lead; no environment discovery):
```sh
academy-capture textbook --owner-root "$OWNER_ROOT" \
  --admission transport-admission.json --admission-sha256 "$ADMISSION_SHA256" \
  --credential credential.json --credential-sha256 "$CREDENTIAL_SHA256" \
  --roster "$ROSTER" --spool "$SPOOL" --date 2026-11-13 \
  --max-requests 5 --no-detail
```
This budget covers ONE book: bindings 1 + index 2, plus 2 paper-search requests
when a title pattern exists. Two source requests cannot run the existing index.
More books need an explicit larger budget. Login exchanges are additional (once).
Details add 4 + title count per batch of at most 12, per book. Budget exhaustion,
any failed request or degraded paper search preserves both old student partitions.
Without detail, counts/score are null and wrong_ordinals is absent; graded=false.
Source attempt_key gives stable section keys student|book|attempt, rather than row.
Ordinals are one-based; hint is the first index title (null for an empty index).
Both files are staged/parsed before rename; a crash or I/O error between the two
renames can expose different generations. This is not a multi-file transaction.

Optional `--raw-dir "$RAW_DIR"` saves source response bytes before parsing,
including malformed responses. The Lead pre-creates this private directory outside
all Git worktrees. OwnerRoot validates its canonical path, private root and trusted
ancestors (no symlinks or group/other writable ancestors). Per student, body files
are `000001-student.books.body` etc.; index.jsonl appends sequence, operation,
rendered query/body parameters, http_status, byte_count, sha256 and observed_at.
Bodies/index are 0600, directories 0700; login bodies/headers/cookies are excluded.
Sequence continues across runs; retain a failed parse's bodies for offline repair.
Run only one capture writer per raw directory; the existing account lease serializes
production sessions. No raw directory or body is created without the flag.
