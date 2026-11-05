# S10 independent review corrections v2

> Current fixture ownership: [E02 fixture ownership](../fixture-ownership.md)
> supersedes migration pathname and replay instructions below. The dated source
> observations and acceptance statements retain their original historical scope.
> Maintained tests read crate-owned copies; historical reproduction requires the
> complete pinned recovery revision, not a partially retired current tree.


Base: `1245660d0899c0e644d1fbec6ae1d3d59b877207`. This candidate corrects
F1/F2 and the source-backed learning suggestion omission. Root owns fresh
independent review and acceptance; these changes grant no native authority.

`Service::review` distinguishes an omitted optional observation handle from a
supplied unknown/unbound handle. Omission returns the original held rows,
guidance and digest without reading a source or persisting a review. The public
`review::review(input, held_observation, fresh_observation, policy, runtime)`
signature remains available. An absent held observation is not parsed as a full
observation when the submitted handle is absent. Supplied observations and all
fresh observations keep their schema, scope, target and value checks.

Shared pre-read conditions preserve lazy reads for missing, stale, future and
globally blocked reviews. Duplicate fields fail before a fresh source read.
The service passes a complete read result to the private review composition.
Unavailable reasons use the finite 188-entry original `read-failure.ts` catalog;
unknown returned reasons become `source_read_unavailable`, while unknown thrown
messages become `dailyrecord_source_unqualified_no_automatic_retry`. Schema and
syntax errors retain `source_response_malformed`. Raw exception messages do not
become public blockers. Rust `Fault` retains its typed name/message/schema data;
it does not invent JavaScript stack frames, HTTP metadata or nested causes that
its owner port has not supplied. Review digests contain the exact selected safe
reason, as in the original class.

The existing typed policy port now supplies the full suggestion row. Its exact
condition is a held observation, no global blockers and learning blockers that
**include** `learning_render_mismatch`. Other learning or row blockers do not
disable it. Learning review may use `teacherReported.mappedText`; suggestion
receives the full original `p.value` and original optional `learningEntries`.
Any non-null suggestion, including an empty string, has meaning
`format_example_only_teacher_must_review_meaning`. Missing/null suggestions
have null rendering and meaning. Suggestions neither approve meaning nor create
an approval, write, dispatch or replay.

## Regression custody and execution

`migration/workflow-store-review-fixes-v2/CASES.json` contains 34 fictional cases.
`original.ts` invokes the actual original `DailyRecordService` with explicit
fictional unavailable, throwing and suggest-capable source/policy ports.
`verify.py freeze` creates a new independent `EXPECTED.json` exactly once; it
never reads a Rust result or changes an old oracle. `verify.py compare` executes
the actual Rust service through its typed RPC participant. It compares full
results, digests, rows, holds, nulls and callback values without normalization.
The only old fixture changes bind the optional suggestion RPC and default null.
Old expected values, cases and comparators are unchanged.

`workflow_review_fixes.rs` exercises the same expectations through direct typed
Rust service ports, checks every retained journal byte before/after held review,
forbids backend saves/publication, and covers absent held observations through
the unchanged public pure review API. Separate typed-error assertions cover
safe catalog/fallback and malformed classification.

Compiler/check execution is initially **HOLD under S09**. Only after an explicit
Root grant, source `preparation/final-review/execution-env.sh`, use Rust1.95,
`CARGO_BUILD_JOBS=1`, `-j1`, `--locked --offline`, and an own private target/evidence
root under `preparation/slices/S10/review-fixes-v2/native`. Run the independent
original freeze before Rust comparisons; then changed-path tests, unchanged
83-main/12-supplement once and necessary strict checks. Raw commands, PIDs,
durations, hashes and group cleanup belong in that task's native notes.

Store/state/codec/lock/approval/CISM/HTTP/native-backend bytes, Cargo files,
accepted S08, original TS, original reference `977613f0b70159280ad1af8a54b17c6efd3bf30d`
and EXPECTED-v2 SHA256
`4cbb93d60824f878b712b22174d0689c2990965f77bcd9ce61354e6e0a09a150`
remain unchanged. Their original 14+14 old-reader/mixed-lock/45-second evidence
may be reused where those pinned boundaries are unaffected. New failures or
source concerns require targeted follow-up, not automatic evidence reuse.

Live source/policy admission, native ownership, MCP/S11, P8 lookup reevaluation
and P9 activation/deletion remain unproved or pending. Finite offline results
do not complete those obligations or narrow the full adopted contract.

## Observed correction checks

Root's explicit compiler grant at 2026-11-02T23:20:33Z superseded the initial
HOLD. Actual original-only freeze produced 34 expectations with SHA256
`9692f743495050c7fd1190f3c4155b665fd6f074f3734db4d2c71fdc18a40381`.
All 34 full Rust result/digest/callback comparisons match. The three direct
Rust integration tests pass, including absent held observation through the
unchanged public API and typed known/unknown/malformed failures. Unchanged main
83 and supplementary 12 comparisons each ran once and match exactly.

Scoped Clippy with `-D warnings`, rustfmt check and source custody checks pass.
The source observer confirms all 188 catalog entries exactly match original
category/locator pairs, 44 original source pins, three existing dependency pins
and 152 original reference artifacts. One source-observer parse failure was
retained: its regex initially missed rustfmt's braced tuple match arms. The
observer parser was corrected; runtime source, frozen expectations and old
comparators were unchanged by that repair.

Binary SHA256:
`af185c6e91dddf9d75b1f2a04c755fecbce63dc17d4560869b5abf5fcf3baad4`.
Compiler/check custody was explicitly released after actual readback found all
334 recorded process IDs and 105 groups gone, with no compiler/product process
remaining. Raw evidence is in the task's allocated absolute native notes.
Root's fresh targeted Wonder review and acceptance remain pending.
