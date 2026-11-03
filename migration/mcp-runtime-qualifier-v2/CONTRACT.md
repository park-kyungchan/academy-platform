# S11 reference qualifier v2

This is a read-only comparator for the finite captured-original S11 boundary.
It supersedes the **comparison policy** of v1 without changing any v1 artifact.
Its baseline is original reference commit
`807db88af175b4dc60b843584e2cc62101e1ac31`, sole parent
`00b239a2ae956816e47904d5a1001d6c4c147b61`, joining accepted S09
`d97d144e644b927d54bb0b4883152408e564a3a3` and bounded S10
`748b47cae987a1099abbb84aa6c872bf17ed9a29`.

No Rust output exists in this qualifier's derivation. New independent Wonder
review and Root acceptance of the exact immutable candidate are required before
any Rust facade. The author does not accept this qualifier.

## Oracle custody

`BASELINE-MANIFEST.json` pins all 565 v1 source/document/frozen artifacts, the
51 original repository source/contract pins, POLICY.json, original before/after
source/dependency snapshots and retained original receipts. The v1 frozen
manifest remains SHA256
`6824b9d0fa3492c2f463cd5c32de1445d6335978bb1a1252e75c04cca057a16c`.
The qualifier verifies these hashes before comparison. Its source embeds the
v2 manifest hash. A candidate fixture, expected.json, COMPLETE flag, policy file
or source-before/source-after pair cannot redefine the expectation. Updating a
policy/oracle requires a separately reviewed forward version. There is no
baseline generation/update command in qualify.py.

Expected observations come directly from `frozen/<mode>/observations.json`,
never v1's normalized expected.json and never candidate observations. Each
response binding was derived from its unique original top-command raw stdout
frame and the exact original stdin request ID. It records observation index,
command role, response frame index, request frame index and, when retained,
the exact occurrence and receipt role. Candidate discovery/first-seen matching
does not select those bindings. Identity responses that return no occurrence
have separately pinned identity-result receipts; no new correlation is inferred.

## Literal semantics and the one narrow capture atom

Omissions, scalar types (including bool/int/float), array order, tool names,
descriptions, schema/defaults/bounds, routing, error codes and digests are exact.
All semantic timestamps, process identity, UUIDs, occurrence roles and root/path
identity remain literal. There is no global string substitution, root prefix
replacement, ignored leaf key, UUID renumbering or JSON-in-text re-encoding.
`semanticVolatileAtoms` and `rootRelocationRules` are deliberately empty.

The only variable atom is each enumerated observer command's
`RESULT.json.elapsedSeconds`. `observe.py:30-34,90` measures it from monotonic
time under a 90-second command deadline. POLICY enumerates its finite command
roles, accepts only a finite JSON float, bounds it by 90 seconds and by the latest
retained same-PGID EXIT.at minus that literal command's START.startedAt. Missing
EXIT evidence does not manufacture a timing proof. This atom is instrumentation,
never payload `elapsedMs`. Clock synchronization and non-adversarial local capture
remain assumptions; this check cannot attest a new process remotely.

Fresh occurrence substitution is deliberately unsupported. Actual source
`neutral-server.ts:119,126,132,241-254` associates randomUUID occurrences with
request dispatch, response metadata/failure content and receipt filenames/body.
The recorded literal roles and bytes must agree; swaps, changed description UUIDs,
inconsistent text/structured/receipt occurrences and changed elapsedMs are refused.
No success for new runtime UUID values is claimed. Positive volatility tests
cover only the justified capture-duration atom.

Source freshness is semantic: `tests/neutral-upstream-fixture.ts:39` really assigns
`new Date().toISOString()` to a DailyRecord observation. `src/dailyrecord.ts:244`
rejects negative ages or ages over 300000ms; lines 423 and 451 use/propagate the
source observedAt. Therefore it is **not** an ignored timestamp. POLICY records a
future proposal only: bind the source read to its exact request/namespace/revision/
wireDigest and establish observedAt <= read completion <= service evaluation,
with evaluation-observedAt in [0,300000]ms and exact cross-channel linkage. v1
does not retain the required evaluation clock/read-completion proof. v2 refuses
that substitution. This is a precise fresh-replay limit, not semantic equivalence.

## Candidate capture directory

Run `python3 -B migration/mcp-runtime-qualifier-v2/qualify.py --mode <mode>
--candidate <directory>`. The directory must contain exactly the finite
`candidateFiles` set in the selected manifest entry:

- Original-format observations, COMPLETE, OBSERVER-PINS, SOURCE-MIRROR and registry
  when present in that mode; no candidate expected.json or policy input.
- Every top command's START, RESULT and raw stdin/stdout/stderr.
- Every recorded inner process's START, EXIT when originally present, loads,
  CHANNEL-ORIGINS and three frozen raw channels.
- Original-format PINS-before/PINS-after and all retained receipt files listed
  under that mode's `nativeSnapshots`. These are additional original provenance
  preserved outside the v1 Git freeze and independently hash-pinned by v2.

Capture **storage** may be in a fictional test directory. Logical cwd/argv/locator
fields must still match their literal original roles. v2 does not authorize
relocation or execute any embedded path. It never opens installed endpoint,
configuration, pointer or package-root locators from a candidate. A new launch
with new PIDs/cwd/paths is currently refused; this is a strict captured-reference
qualifier, not a universal fresh-run comparator or native harness.

Read limits are 64MiB per file and 12MiB per raw channel. Traversal rejects
symlinks, special entries, more than 2048 entries and missing/extra/unbound files.
RESULT bindings must name the exact original logical channel path and hash the
actual supplied raw bytes. The hash need not equal the original stdout hash if
the permitted outer-member order changed; comparison still enforces wire tokens.
Per-process files and channel origins are independently pinned to original roles.
Source-before/source-after must each match the original pins as well as each
other; changing both cannot hide changed source after an outer-green report.

## Raw comparison

Input is exact aggregate stdin bytes, including malformed JSON, CRLF and the
accepted split-input request. Stream read/write chunk boundaries are not wire
frames and are not compared. Stdout is the exact LF-delimited frame sequence,
including line endings, final newline, diagnostics/non-JSON lines and order.
Valid JSON frames reject duplicate keys/nonfinite numbers and compare lexical
trees: only object member order is immaterial; scalar lexemes, string escaping,
whitespace, arrays and literal content.text tokens remain exact. Tool text is
never parsed as nested JSON. Original non-JSON lines compare byte-for-byte.
Stderr compares byte-for-byte, retaining framing error count/order/content.

Every expected response must also equal the object reconstructed from its exact
raw frame, and its original request ID/name binding must agree. Extra, truncated
or replaced frames fail even when observations.json is unchanged. Candidate
channel hashes alone, observer COMPLETE and outer exit zero cannot establish
correspondence. All command/inner exits remain exact; there is no generic
`child_success` or graph-trust predicate.

The bounds neutral command intentionally has `returncode:-15` and
`cleanupRequested:true` after the fictional hung save. This is an expected
SIGTERM cleanup fixture, not a successful child. Other signals or exit zero are
different outcomes. Its missing inner EXIT remains an explicit qualification
limit; fabricated/omitted EXIT evidence cannot be silently substituted.

## Qualification and known original defects

`comparison:MATCH` means exact correspondence within the above recorded boundary.
All ten modes return `qualification:LIMITED`, `fullParity:false`,
`graphAdmission:false`, exit **2**. A mismatch or missing required evidence is
`REFUSED / UNQUALIFIED`, exit **1**. `--verify-baseline` returns exit **0** for
integrity only, with qualification NOT_EVALUATED. Neither label accepts S11.

The v1 freezer's direct-channel selection (`freeze.py:25-32`) checks whether a
process entry occurs in originalArgv, without a process-group/complete-argv
association. Several discover/endpoint/launcher process copies were consequently
overwritten by another command's channels. v2 detects the actual argv/PGID
mismatch and lists each affected process/channel in its immutable mode limits.
It does not repair v1, infer new inner bytes or relabel those copies as valid
per-process capture. Top-command raw pipes and their response bindings remain
independently usable. Correct direct copies, JS taps tagged
`inner_original_stream_emit_or_write_tap`, and explicitly tagged
`no_stream_data_or_write_observed` stay distinct. A no-event tap is not syscall
silence. An actually missing required candidate file fails closed.

Some graph release failures have returned occurrence IDs but no retained receipt;
those observation indices are LIMITED, not invented receipt passes. The four
non-wire bounds/launcher/relocation records remain pinned observer-only outcomes.
Early failed discovery/protocol stderr is genuinely unavailable and is outside
these final ten modes; it is not an empty channel. Raw files, local hashes and
correlations detect substitution within this finite corpus but are not
cryptographic remote attestation or proof that a candidate freshly executed.

F3 remains a negative-control issue: bare graph booleans never admit anything.
Actual artifact/owner/UID/mount/endpoint/account/lock/journal and fresh native
connection prerequisites remain external. v1 same-process graph recovery is
recorded behavior, not permission to adopt it in a future facade.

F4: static 51 source/1070 dependency pins are not executed dynamic bundle/jsdom
closure. F5: the original observer actively confines writes/network and uses JS
inner taps; behavior at those edges is scoped, not universal native parity.
F6: fictional inventory/routing/backend topology is not genuine endpoint wire.
F7: eight failed setup attempts (including unintended inventory-02 automatic
resolution) and early trace/bootstrap gaps remain preserved; no exhaustive
no-fetch/no-outside-effect claim follows from v2. No observer was invoked here.

## Coverage

All ten v1 modes are pinned: inventory, discovery, protocol, composition, graph,
identity, framing, bounds, launcher, relocation. All 56 original observations are
literal inputs; 52 returned responses have exact raw request/response bindings.
The remaining four records are intentional observer outcomes (bounds two;
launcher one; relocation one). See each mode's precise immutable limits.
Partial support never produces a full-parity claim. Tests use only stdlib and
fictional copies under the allocated native notes directory. No product source,
Cargo/Rust/Bun build, dependency resolution, native profile/service or live
Academy operation is part of this contract.
