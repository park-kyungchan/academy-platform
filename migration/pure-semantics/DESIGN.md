# S08 independent source oracle and deliberately unimplemented foundation

Authority: current user assignment and `/home/palantirkc/work/ws-replan/{STATE,ORCHESTRATION}.md`, then the selected final-review ZIP (`964845e260a25350823fb42d38a60e7cba973f8af607e10b505727d3bf78915c`). S08 minimum vectors are EDU-03/04/05/06/17/18/JUDGE-01. Source base is `cc6e81f1be2fcb56f011041e3d606e21edf76da3`; prerequisite is accepted S05 `049f7c3da0af6c320c818d2afdab3da34bccd37a`. S05 supplies no template, calendar or native authorization evidence here. Existing repository harness instructions do not add retired gates.

This is author-frozen foundation, pending independent Wonder review. It is not S08 parity acceptance. No Rust semantic function is implemented or compiled. Lead owns interfaces, integration, review acceptance and the sole `-j1` build slot. Do not start S09/S10 or dispatch Terra before independent freeze acceptance.

## Public finite leaf boundary

The new crate's library has exactly these semantic entrypoints, all in `crates/edu-academy-core/src/leaf.rs` and all returning `Err(LeafError::Unimplemented)` in this revision:

```rust
pub fn journal_wire_width(units: &[u16]) -> Result<u64, LeafError>;
pub fn cism_week(date: &str) -> Result<WeekPeriod, LeafError>;
pub fn cism_month_dates(month: &str) -> Result<Vec<String>, LeafError>;
```

`WeekPeriod` serializes `period_start` as `periodStart` and `period_end` as `periodEnd`. `LeafError` is exactly `CismDateRejected`, `CismMonthRejected`, `Unimplemented`; source messages are `cism_date_rejected`, `cism_month_rejected`, and foundation message `s08_leaf_unimplemented`. These are new interfaces, not a claim that the source exposes these Rust types.

One binary request is one UTF-8 JSON value read to EOF, with an adapter-only 1,048,576-byte limit. The strict tagged request enum is exactly:

```json
{"op":"width","units":[65,55357,56832]}
{"op":"week","value":"2026-01-01"}
{"op":"month","value":"2026-11"}
```

`units` are unsigned integers 0..65535, including unpaired surrogates. This representation preserves all source code units without replacing malformed Unicode with U+FFFD. The original width function takes a JS string; the reference adapter constructs it with `String.fromCharCode`. No normalization, trimming, scalar conversion, line-ending conversion or truncation occurs. Non-string calendar inputs, unknown fields/operations and invalid JSON are outside the leaf protocol; the CLI refuses them with status64 and `s08_protocol_input_rejected` on stderr. This is adapter admission, not a source refusal. Input/output I/O failure is status64/74. Unimplemented leaves exit78 with `s08_leaf_unimplemented`, no plausible success frame.

An implemented source return has status0, empty stderr, and exactly one compact JSON line:

```json
{"outcome":{"kind":"return","value":5},"events":[],"effects":{"deniedTransportAttempts":0,"forbiddenFakeDispatches":0}}
```

A source calendar refusal also has child status0, with `outcome` equal to `{"kind":"throw","error":{"name":"Error","message":"cism_date_rejected"}}` (or month error). This protocol carries the original thrown refusal; child status is an additional independent comparator input. Actual nonzero status always fails comparison.

## Exact leaf algorithms

### Width

Initialize a u64 sum to0. Iterate the original UTF-16 units in order; add1 for units0..127 and2 for128..65535. Do not interpret surrogate pairs. Return the sum. Empty input returns0, A returns1, 가 returns2, 😀 (D83D DE00) returns4, a lone D800 returns2, composed é returns2, decomposed e+0301 returns3, CR/LF each contribute1. ASCII399 / Hangul200 / ASCII1+Hangul200 return399/400/401. Width does not enforce the renderer's400 policy. Values cannot overflow u64 under the CLI input bound.

### CISM date and week

Admit only exactly ten ASCII bytes `20YY-MM-DD`, years2000..2099 inclusive. Require hyphens at offsets4/7, all other bytes digits, month1..12, day1..month length. Gregorian leap rule: divisible by4 except centuries not divisible by400. All other strings produce `CismDateRejected`. Compute the weekday for the admitted UTC date, Sunday0 through Saturday6; subtract that many calendar days to obtain Sunday, then add6 for Saturday. Format both as zero-padded YYYY-MM-DD. No local timezone conversion or clock reads. Output arithmetic may cross outside the input year range: 2000-01-01 returns1999-12-26..2000-01-01; 2099-12-31 returns2099-12-27..2100-01-02. Do not re-admit those derived dates with the input year rule.

An implementer may use the following completely specified small arithmetic approach without a date dependency: count days from2000-01-01 using month lengths; its weekday is Saturday6. For day ordinal n, weekday is `(n+6)%7`. Convert a derived ordinal back with Gregorian month lengths, including1999 and2100. Internal output ordinals range from-6 to36530; i32 is sufficient. This arithmetic is an implementation instruction; the oracle calls the original Date-based functions.

### CISM month

Admit exactly seven ASCII bytes `20YY-MM`, year2000..2099 and month01..12, else `CismMonthRejected`. Start the result with that month's first date. Let w be its Sunday-based weekday; advance by `7-w` days (always1..7). While the resulting date remains in the requested month, append it, then advance7 days. Preserve this order and the literal reference dates, not just derived intervals. When the first is Sunday, include it once and advance7. Otherwise the first entry denotes the intersecting week beginning in the preceding month, followed by the month's Sundays. Do not add a Sunday after the month ends. Examples:2026-10 gives01,04,11,18,25;2026-11 gives01,08,15,22,29; leap2024-02 gives01,04,11,18,25. All bytes are in `expected-v2.json`.

## Reference callable contracts beyond the first leaf

Every case in `corpus/cases.json` calls an actual unchanged function, not a transcription of its algorithm. All full values, exact string bytes, source throws (name/message and Zod issues), effect counters and meaningful event ordering are in `expected-v2.json`. JSON protocol object member order is not significant to deep equality; arrays, missing properties and all string bytes are significant. JSON digest member ordering is inside the `bytes` string and is compared exactly.

| Boundary | Frozen selected behavior | Disposition outside this finite boundary |
| --- | --- | --- |
| `lessonJournalTemplates.journalWireWidth` | All original code units;16 cases, including surrogates and399/400/401 | First Terra leaf; CLI bound is explicit adapter limit |
| `cismWeek`, `cismMonthDates` | 22 dates,16 months; UTC Sunday–Saturday, invalid/year/month/leap transitions | First Terra leaf; does not prove CISM parsing/writing |
| `renderLessonJournal` | 18 structured synthetic cases; full progress/homework/memo/blockers/evidenceRefs/revision/approval values | Needs a later Sol finite decomposition/review before Rust implementation; do not infer all schema bounds covered |
| learning `parseCalendarDate` | 7 cases; source's four-digit-year range differs from CISM, returns input string or `invalid calendar date` | Keep separate; no call to it from a CISM Rust leaf |
| learning `assertKnowledge`, `known`, `unknown` | Missing/null invalid; known blank/zero/not_performed preserved; unknown reason and exact keys/refs;16 cases | Constructors' deep-freeze identity/property descriptors not characterized by serialized frames; generic knowledge admission does not impose assessment enums |
| learning `canonicalJson`/`digest` | Strict recursion, finite number JSON, UTF-16 key sort, property presence, exact bytes/SHA256 and adversarial JS structures | Judgment-heavy: not assigned to Terra. Sparse/decorated arrays, symbols, accessors, unsupported prototypes, cycles/depth refusals retained in selected cases |
| admission `canonicalJson`/`canonicalActionDigest` | Separate recursive sort, omits undefined object members; sparse arrays can emit invalid JSON bytes; symbols ignored; accessors may run; finite-number handling | Do not substitute the stricter learning encoder. Unbounded cycles/depth are not executed; explicit obligation remains to choose admission before porting it |
| CISM `cismHash` | Ordinary JSON.stringify insertion/index-property order, negative zero/exponential/escaping/toJSON behavior; exact bytes and SHA256 | Preserve distinct family. Top-level unsupported values can yield Node TypeError; no universal generic-JSON promise |
| `DailyRecordService.intake/review` | 24 synthetic scenarios; full original return values, requests, targets, holds, patch, digests and policy callback events | Reference composition only, not a production port. Fake learning/parent policies are explicitly injected; no canonical catalog/bridge or general parent-language qualification proved |
| `applyJournalHttpAction` | 4 in-memory fake-target cases; current DT vs prior homework, mismatched effectRecordKey refusal, exact approval/read/write/read event order | No HTTP, real owner admission, lock, journal or durable uncertainty proof. Broader transport/readback/conflict cases remain S10/S11 |
| template and CISM constants | Actual `constants` result preserves every label/order/revision/path/wire hash/narrative enum | Constants are reference outputs; not a replacement for runtime authority |

There are78 JSON-family cases:28 learning,25 admission,25 CISM. The declarative `$js` construction supports undefined, -0, NaN, Infinity, bigint, function, symbol, sparse/decorated array, Date, null-prototype object, symbol-key, accessor, cycle and depth64/65 as selected per family. It does not transform ordinary business strings or objects. Numeric source values use binary64, not arbitrary-precision decimal; 9007199254740992 is retained as the actual representable JS number. No Rust canonicalizer generated these expectations.

Learning canonicalization first rejects depth>64, then allows null/bool/string/finite number via JSON.stringify. For compound objects it rejects own symbol keys, any accessor descriptor and ancestor cycles; arrays need Object.keys length equal to length; non-arrays need Object.prototype/null prototype. Recursively encode arrays in order, objects with `Object.keys(...).sort()` using UTF-16 code units, no spaces. SHA256 hashes the UTF-8 bytes of that string. Source permits lone surrogates by JSON escaping. Shared references without cycles are not rejected, although this corpus does not separately characterize them. Non-enumerable properties are not a universal ban; do not invent one.

Admission uses the same primitive/finite branch but a distinct `dailyrecord_save_action_not_json` failure; arrays map/join without sparse checks. Objects select enumerable string keys whose accessed values are not undefined, sort them by UTF-16, then recursively encode; it has no prototype/accessor/symbol/cycle/depth protection. CISM uses ordinary JSON.stringify (array index keys first in numeric order; other string keys by insertion order), then hashes bytes. Its top-level undefined/function/symbol fail in hash.update; array equivalents become null. Exact source TypeError text is runtime-pinned, not a new semantic error category. No family performs Unicode normalization.

## Required Education dispositions

EDU-03: learning known blank and numeric0 remain distinct; unknown has reason, missing/null throw. Source assessment code `-1` is unknown option, `0` is explicit not-performed (not numeric score0); blank source assessment is unqualified. Service intake and review preserve those distinctions and never derive a code from a narrative. Serialized evidence is not a general learning-completion derivation or student-account entitlement proof.

EDU-04: `DailyRecordService` selects `previousTarget` only for `previousHomeworkAssessment`; all other rows use current `target`. Previous target must share namespace, teacher, student, course and membership, have a different record and earlier lessonDate. A known previous assessment with no previous target refuses source qualification; unknown previous assessment with absent target yields field-local `unknown_before`/`independent_previous_record_required`. The fake HTTP action still carries today's target/date but must use the independent previous `effectRecordKey`. Wrong record refuses before fake approval/write. No recorded HTML occurrence parser or native writable binding is proved.

EDU-05: proposal rows gather global plus own field blockers, deduplicate in insertion order and emit only `proposed` rows to patch. An overall `held` review can still contain an independent valid attendance patch. A learning hold is not a global block in the frozen review result; that does not authorize a partial real save. Fake policy composition proves the local separation, not actual policy-bundle judgments.

EDU-06: changed nonempty before with `set` gets `existing_value_requires_explicit_replace` except assessment sentinel-1. Exact unchanged set is noop. `replace` is eligible subject to all remaining policy/fact checks; explicit `clear` requires empty value. Empty narrative without clear holds. Memo `no_addition` preserves existing memo and blocks even a clear proposal. Unsupported completion language holds independently. Teacher-reported progress can preserve the exact before prefix plus mapped text and exact fact lines; the fixture covers one such case. No automatic replacement or source write is performed by review.

EDU-17/18: exact width/text/digest/calendar expectations are frozen as bounded above, with no Rust candidate yet. Canonical JSON and renderer implementation remain separate obligations. EDU-18's full CISM date/year output boundary cannot be inferred from S05.

JUDGE-01: actual-child status7 with plausible JSON; stdout/stderr junk; denied network/fake dispatch; wrong width/error/digest bytes/digest hash; missing field; reordered array/events; null-to-missing; zero-to-blank; wrong physical target and removed independent patch are rejected. Raw status/signal/spawn error/stdout/stderr remain separate. See `qualification.json`; no wrapper exit or self-reported success substitutes for child evidence.

## Source functions deliberately not ported here

`learning/model.ts` catalog/scope/record/context validation, stable IDs, contextFromLessonKey, resolveProblemPath, studentOf, assertTime/assertInstant and immutableCopy's runtime freeze require bounded source-backed specifications before implementation. Source type unions for catalog, plans, observations, provenance, scope and assignments remain the authoritative meanings; no new Rust union approximates them in this foundation. `requireText`, `assertExactKeys` and refs validators execute transitively in selected cases but are not generally exhausted. `validateRecord`'s teacher-inspection authority is not inferred from a known-state constructor.

`parseCismReport`, handler/source selector, inert DOM qualification and CISM report/field identities remain source/parser work for S09 and action integration for S10. `cismWriteParameters` requires a separate pure parameter corpus (including source tuple membership and exact URLSearchParams encoding) before migration. It is not exercised in this first leaf. `liveSaveRequestEvidence`, save-admission schemas, createDailyRecordSaveAdmission's exact tuple/current-event/one-time consumption and durable owner behavior are not invoked. They remain S10/S11 obligations, with no real current-request file, consume key or journal touched. The imported read/workflow modules are byte-pinned dependencies; their I/O exports are never called in review, and no workflow/write backend is instantiated.

The legacy source's complete lock, durable journal, cross-request/date physical-field uncertainty, actual native account authorization, root ownership, source HTTP policy, history/budget/HTML and complete release-graph recovery remain unproved here and carry forward to S09/S10/S11 as applicable. User P8 lookup verdict is absent. S08 foundation does not close S05's gaps, grant P9 activation/deletion, or claim production completeness.
