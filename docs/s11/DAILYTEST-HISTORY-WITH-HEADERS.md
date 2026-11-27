# DailyTest history with headers (source handoff)

`academy_dailytest_history_with_headers` ports the original
`modules/academy-workbench/tools/academy-mcp/src/dailytest-header-route.ts`
consumer and the `neutral-server.ts` two-source lifetime composition.

Input is strict. Display names use ECMAScript trim and a 200 UTF-16 unit bound;
lesson dates are literal valid dates. Purpose/request references retain the
DailyRecord ID contract. Selectors are unique, ordered values 11–16 (1–6 values).
Header pages default to 32, permit 1–32 headers and offsets 0–12000, and require
a lower-hex selection SHA256 when offset is nonzero.

The projected metadata scope and ordered current-selector/history rows determine
the selection SHA256. Unknown source properties are stripped before hashing.
First-seen observed paper keys are deduplicated, pinned and paginated before code
conversion. Printed code encoding retains the original alphabet and safe integer
ceiling. Returned headers must match exact student/key/code identity and count;
duplicate requested fields fail. Titles, categories and counts stay raw strings;
empty/absent values become null. Relative-period coverage, unqualified calendar
bounds, book edition, attempt dates and teacher completion stay explicit. Output
field order and the original dual MCP representation's 512 KiB cap are retained.
In particular the original `partial` status compares total papers to this page's
selected count, including a terminal empty page at a nonzero offset.

The parent selects metadata at dailyrecord, prefers the paper1 registration and
falls back to paper2, then uses the existing student-specific paper router. It
uses the existing supervisor/OwnedChild/Client for each source, with one absolute
work deadline. Metadata supervision and its cleanup receipt finish before paper
opens. Each child independently initializes, verifies inventory and endpoint
identity; dailyrecord additionally probes the admission digest. Failed cleanup,
control cancellation, graph change or deadline failure prevents the next launch.
There are no retries. Metadata unavailable and paper blocked failures retain only
the original bounded measured sourceRequests value and public failure projection.

The original seven-route DailyRecord driver, its protocol entry and endpoint
binding are untouched. Additive `business::History`, `RpcSession::handle_history`
and the exact two-route endpoint binding lend `edu_academy_sources::history::History`
only to fictional owner graphs. Metadata queries are limited to this task's
single daily_history query. The source algorithms enforce namespace/date/selector
and managed-name policy, dated identity, configured identity consistency, current
selector discovery, ordered raw row projection and duplicate attempts. Paper
source algorithms enforce explicit student admission and returned identity,
retain raw metadata strings, and hash raw source text. Each source loan latches
source failures. The fictional executable uses real session/driver/source code
and a finite raw-observation Port. A shared fictional kernel file lock and event
log make overlap observable; they never touch an Academy account lock.

This does not implement an HTTP adapter, authentication/native owner factory,
source HTML/header extraction, broader metadata families, learning/CISM/textbook
routes, operational activation or recovery. The Port inputs are explicit raw
source observations, not proof that a live source produced them. These limits
are material: source handoff and fictional tests do not qualify production or
live/native Academy behavior.

## Lead verification (UNEXECUTED)

All tests and examples are UNEXECUTED by the source author. `rustfmt` and static
Git/hash/readback checks are the only source-turn verification.

The crate `edu-academy-mcp` that held these tests and examples was removed in pull request #167; the commands that built and ran them no longer apply.

The tests retain outputs, source call events, complete child RPC streams and
supervisor cleanup receipts under the selected root. They cover strict input
before calls/spawn, UTF-16/date edges, ordered selection pins and pagination,
scope and identity conflicts, raw/absent fields, the dual-result cap, bounded
failure counters and source latches, real child route selection and independently
qualified cleanup before the next child acquires the fictional lock.


## Fixture executable packaging repair

Cargo debug executables can exceed the existing release artifact limit of
64 MiB (`release::ARTIFACT_BYTES`). The history fixture verifies the Lead-supplied
executable hash, copies it into its fresh install root, and uses the host
`/usr/bin/strip --strip-debug` on that copy before creating any release pins.
The original Cargo executable is unchanged and its hash is rechecked. The
packaged copy must fit the unchanged limit. Each scene retains strip stdout and
stderr and `state/child-packaging.json` with input/package/tool hashes and sizes.
This is fixture packaging only; release.rs, production code, limits, profiles,
Cargo manifests and lockfiles remain unchanged.


## Release-child timing repair

The two child integration tests require the normal release-profile fictional
child (opt-level 3, debug-assertions false, overflow-checks false), as explicitly
selected by the Lead after debug-child handshake timeouts. The parent test
binary remains in Cargo's test profile. The existing 10-second work budget,
all supervisor/graph checks and every cap remain unchanged. Fixture packaging
now invokes strip only when the supplied executable exceeds ARTIFACT_BYTES;
a release executable already fitting the cap is copied and pinned byte-for-byte.
The packaging receipt records whether strip was invoked. This qualifies only
the explicitly selected release child, not default debug-child performance.

## Upstream failure contract repair

The history child returns its bounded unavailable/blocked source envelope even
when a valid source result exceeds the existing dual MCP 512 KiB limit. It
retains measured sourceRequests; the parent therefore keeps failedSource and
the original source-specific code/category/locator. An oversized metadata result
maps to student_metadata_unqualified_no_automatic_retry; an oversized paper
result maps to source_headers_read_failed. Both are upstream_unavailable.

Malformed metadata/header response shapes map at the two upstream parse
boundaries to source_response_malformed / malformed_response /
academy-mcp:response-contract. These are ordinary response-contract failures:
Composite-only failedSource/sourceRequests fields are absent. Caller schema
failures still map to tool_input_invalid / input_validation, with zero child
calls. Explicit semantic identity/scope conflicts retain their original codes.
The fictional endpoint can inject a negative response mutation only after real
session/driver/source execution, to verify malformed-response propagation.
No cap, deadline, admission or existing seven-route behavior is changed.
