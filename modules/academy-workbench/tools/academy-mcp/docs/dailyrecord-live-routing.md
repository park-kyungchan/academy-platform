# Live DailyRecord lookup

For a request to read today's DailyRecord, choose `academy_dailyrecord_get`.
Supply the user's exact student name, current purpose and request reference.
Omit `lessonDate` for today in `Asia/Seoul`, or use an explicit real ISO date.
The caller cannot supply a native student key, teacher, namespace, admission,
facts, memo answer, proposed values or approval flag to this tool.

The route resolves the exact name with `academy_student_resolve` on the lesson
date, and passes its returned key and the backend's configured teacher to
`academy_dailyrecord_intake`. No date window and no student list gates the
route; the LMS limits every read to the configured teacher's students, and every
row must carry that teacher's key. Zero/ambiguous names, unavailable source,
unqualified or stale output and scope drift stop without automatic retry. Native current/prior record identity checks remain
in the existing intake/backend. The source fields and transmission state
remain observations. This lookup returns no review handle or patch.

The composed route passes the lesson date to the resolver. The owner client discovers the source's dated
groups and projects the exact principal-qualified DailyRecord occurrence into
a name/key pair. A missing or ambiguous dated row stays unavailable; it does
not fall back to a recent test page or prove student absence. The later intake
independently verifies the dated current and previous occurrences.

An undated resolve searches the DT/ZT result page `sort_date` `'1'` first,
with the same request and result as before. When the exact name is not on it,
the owner client reads the history selectors that `readDailyTestHistory`
reads (`11` to `16`), in that order, one request each, and stops at the first
page that holds the name. That result carries `fixedSourceFilter` naming the
selector; a result without it was found on `'1'`. The `'1'` page must offer
every history selector, or the search stops with `daily_history_selector_drift`.
A name on no page is `student_name_not_found` after at most seven requests;
it still covers only the pages read, not the roster.

`status: observed` and `sourceAvailability: observed` mean the read succeeded.
`preparation.interviews` records questions needed only if the user later asks
to author/review. No memo answer, attendance, assessment, fact or completion
is invented. Existing `academy_dailyrecord_intake` and review schemas retain
their complete current-context requirements and explicit source-option rules.

`academy_query` and `academy_source_status` concern preserved snapshots.
Their local retrieval time and historical access report neither refresh
the source nor establish current live availability. The status result now
states this scope and points to the live route.

## DailyRecord save (gated write)

`academy_dailyrecord_save` is the only write tool. It saves nothing unless
the upstream process was launched with both `--dailyrecord-save-admission
<path> <sha256>` (an `academy.dailyrecord-save-admission/1` file the runtime
owner issues from the actual current user save instruction, admitting exact
student/date/field/effect-record/after tuples) and `--dailyrecord-save-store
<owner-artifacts-dir>` (durable one-time consumption records). The caller
must present the `reviewDigest` of an `academy_dailyrecord_review` prepared
in the same process; the service recomputes that review against a fresh
before-state and holds without writing on any mismatch or drift. The native
backend then runs the owner client's `applyJournalAction` per field: fresh
before-state, one consumed admission, one write, authoritative readback,
no automatic retry. Without that launch configuration the tool returns
`dailyrecord_save_backend_not_configured` with `academyWrites: 0`. Exposing
the tool is not save authority; each save still needs its own issued
admission. No parent sending, printing or paper generation is included.

For an exact learning-render mismatch, a fresh qualified review may return
`learningSuggestedRendering` from the pinned owner catalog. This is a format
example, not an automatic rewrite or evidence of semantic equivalence: the
teacher must check that it preserves the intended facts before a new review.
Unknown book or section bindings remain blockers and do not produce a guess.

## Owner admission (retired)

The date windows, student lists and metadata operation lists of 2026-10-28 are
removed (TASK-GATES-1, 2026-11-17). An installed launch may still pass
`--dailyrecord-admission <path> <sha256>` and `--metadata-admission <path>
<sha256>`, and a release may still pin an admission in `routing.json`; they are
accepted and gate nothing. The native config's `students` are known name/key
pairs only: every other student is found by name through the source. The
`owner-admission-*.json` files in this directory are historical records.

## Neutral installation

`src/neutral-server.ts` is the maintained source for
`/root/.local/share/academy-mcp/server.ts` and its Bun bundle `server.mjs`.
`routing.json` may carry an admission path and its SHA256 of an earlier release;
the bridge does not read it. The neutral bridge resolves and intakes inside one
upstream process/account-lock batch.
`endpoints.json` retains its original native getter, image, lock and artifact
pins; no credential value is included in registration or source.

Build the neutral bundle with `bun build src/neutral-server.ts --target=bun`.
Back up the installed registry before `--discover` (which refuses overwrite).
Discovery includes the route once, suppressing duplicate upstream copies.
Only new client processes load changed registry/endpoint bytes. Existing
sessions keep their loaded tools. Review tickets retain their existing
request/scope/signature/expiry checks; failures never latch later requests.
Receipts contain tool names, statuses, phase metadata and hashes only.

Each call starts its endpoint in a new process group and ends the group when
the call ends (`src/upstream-process.ts`). The installed endpoint command is
`flock --wait 45 <account lock> python3 native-endpoint.py`, which execs
`setpriv` and Bun; `flock` keeps no handler for SIGTERM and its child holds
the locked descriptor. The SDK's stdio transport signalled only `flock`, so a
timed-out call left its Bun endpoint running with the lock, and the next
call's lock wait expired after 45 s. Now the bridge ends stdin, waits up to
2 s, then sends SIGTERM and, if needed, SIGKILL to the whole group and waits
until it is gone. The account lock and the bridge's one-call-at-a-time order
are unchanged. One upstream tool call is bounded by the SDK's 60 s, or by
`ACADEMY_MCP_UPSTREAM_TIMEOUT_MS` (1000 to 600000) when the launch sets it.

The fictional neutral SDK test runs with outbound networking denied and
proves a single upstream lifetime, strict caller schema and content-free
receipts. Fresh native E2E evidence is separate from source/synthetic tests;
the work record cites session/receipt metadata without student originals.

## Versioned read releases

A student found by an exact unique live name/key match is bound to the same
request for five minutes in the process. The DailyRecord row and previous
occurrence still require exact principal, student, course and membership
qualification. Saves still need their per-request approval and readback.

An upgraded bridge optionally recognizes an owner-controlled `release.json`
under its existing install directory. The pointer references immutable,
SHA-256-pinned `routing`, `endpoints`, and `registry` files and is replaced
atomically only after those files are ready. It is read for every request, so already-running upgraded bridge clients
can converge on the new release without being restarted. No per-call admission
handshake runs; `academy_runtime_read_admission` is retired, and a registry or
surface of an earlier release that names it drops it. Invalid releases, and removal of an adopted
pointer, fail closed. Existing pre-upgrade bridge processes cannot acquire this
behavior without a separately authorized upgrade. The release schema is
`academy.mcp-release/1`; see `src/runtime-release.ts` and synthetic release
tests for exact fields and checks. Do not mutate or overwrite pinned releases.

An unqualified release base keeps the existing `release_base_unqualified` code.
An optional finite `reason` distinguishes path form, directory type, write mode,
and owner mismatch without exposing a path or numeric identity. It is available
in tool failures, discovery error data, and a fresh bridge's startup diagnostic.
It identifies a failed check only; it does not repair ownership or authorize a retry.

## Tool surface file

A release pointer (`academy.mcp-release/1` or `/2`) may pin an optional
`surface` member, `{path, sha256}`, read and verified like `registry`. The file
decides what `tools/list` shows and the server's `instructions`; it never
changes what a call accepts.

```json
{"schemaVersion": "academy.mcp-surface/1",
 "instructions": "Resolve the student first. Admitted dates: {{admittedDates}}.",
 "tools": [{"name": "academy_dailyrecord_get",
            "title": "Read a DailyRecord",
            "description": "Read one DailyRecord. Admitted dates: {{admittedDates}}.",
            "inputSchema": {"type": "object", "properties": {}}}]}
```

- `instructions` is optional: a string of at most 16,384 bytes in UTF-8. A
  value that is not a string or is longer fails the release with
  `release_surface_unqualified`. The bridge answers `initialize` with the
  `instructions` of the release it loads at start, with `{{admittedDates}}`
  rendered as in `tools/list`; the text stays fixed for the life of the process,
  so a pointer change under a running bridge does not change it. Without the
  member (or without a surface), `initialize` is unchanged
  (`tests/golden/surface-absent-initialize.json`). The surface hash in
  `academy_runtime_identity` covers the text. An empty string adds no
  `instructions` member, as the SDK omits an empty text.
- In `tools`, `name` and `description` are required; `title` and
  `inputSchema` are optional; no other member is accepted, here or at the top
  level. Names are unique, and each is a tool
  of the release's registry or `academy_runtime_identity`. An `inputSchema` is
  an object whose `type` is `"object"`. A file that breaks any of these fails
  the release with `release_surface_unqualified`; bytes that do not match the
  pin fail as a registry pin does.
- `tools/list` returns the surface's tools in its order and no other. The
  description is shown verbatim: no routing context, scope sentence or date
  hint is added. The only substitution is `{{admittedDates}}` of an earlier
  surface, which renders as `any date`: no date window gates a read. `inputSchema` and `title` are the surface's when given;
  otherwise, and for `annotations` and `outputSchema`, the registry tool's.
- `tools/call` refuses a name that the surface does not list with
  `unknown_tool` before any endpoint starts. A listed name is validated against
  the registry's `inputSchema`, not the displayed one, so a displayed schema
  may be narrower than what an endpoint accepts.
- `academy_runtime_identity` reports `surfaceSha256` (`null` without a
  surface); it is callable only if the surface lists it.
- Without `surface`, `tools/list` is unchanged
  (`tests/golden/surface-absent-tools-list.json`).

With or without a surface, a `tool_input_invalid` result carries at most five
`issues` of the form `{kind, path}` (plus `missingKey` or `expectedType` when
the schema names them); no argument value or caller key appears. Tests:
`tests/surface.test.ts` and `tests/instructions.test.ts`.

## Item answers

`academy_item_answer_lookup` reads a bounded set of exact numeric item labels
through the authenticated owner client. It returns the source's official
answer literal and explicit rendering/printed-paper limitations. For observed delimiter patterns, each answer also carries
`answerPrefixCandidate` and `answerPayloadCandidate`. These are syntactic
parts only; their rendering meaning is unqualified. Escape pipes when placing
the original literal in a Markdown table. It does not
interpret an unqualified literal as a rendered answer, infer a student
response, grade, save, print, or send. The existing source endpoint may return
an external solution reference instead of a literal answer. Production
exposure requires the owner bundle and neutral registry to be published as a
consistent verified release; local tests do not prove live availability.

`academy_item_section_trace` without `bookLabel`, for a student bound to more
than one book, answers `interview_required` with `bindings[].bookLabel` and
`bookNo` right after the binding read, as
`academy_textbook_section_results_get` does but without its per-book hints.
It never scans every book in one call; repeat it with the chosen `bookLabel`.
With one bound book it proceeds with that book.
