# S11 original reference and implementation contract

Reference preparation only. Base `00b239a2ae956816e47904d5a1001d6c4c147b61`
has exact parents S09 `d97d144e644b927d54bb0b4883152408e564a3a3` and bounded
S10 `748b47cae987a1099abbb84aa6c872bf17ed9a29`. No Rust implementation,
native activation, full S11 acceptance, real account/content/UID proof or P9
approval is supplied. Root owns integration and a NEW independent Wonder review.

## Independent origin

`observe.py` executes unchanged original TypeScript bytes in a disposable source
mirror. `SOURCE-MIRROR.json` maps every original path/hash to its executed path.
The mirror's sole dependency locator is a symlink to the existing installed MCP
node_modules; `--no-install`, `--no-env-file`, explicit minimal environment,
source-owned network denial, no Bun sockets/servers, and disposable write guards
apply. No production source, dependency or prior oracle is rewritten. The
preload observes stdio and confines writes; it does not implement tool bodies,
SDK normalization or protocol dispatch. Original relative imports and native
CJS/ESM resolution remain intact. No candidate Rust output was an input.

The mirror is needed because this worktree has no dependencies and Bun's
NODE_PATH did not resolve the SDK validator export. Earlier loader attempts are
retained failures, not semantic passes. `inventory-02` accidentally enabled Bun
automatic package resolution, materializing different package versions inside
its disposable directory. That attempt is excluded. Its file trace shows no
mutation path outside that directory, but network syscalls were not traced in
that early attempt; do not claim no fetch occurred. Final modes explicitly
disable installation and trace file, process and network syscalls. No package
installation command, fetch command, shared cache edit or pin substitution is
part of the final observer.

`PINS-before-v2.json` pins 51 source/contract files and 1,070 files in 14 installed
packages. SDK client/server/core are 2.3.1, Zod 4.4.3, Bun 1.4.3. Each final mode
retains equal before/after source and dependency snapshots. The earlier
`PINS-before.json` contains regex false positives and is retained as a failed
static attempt, superseded by v2. `GRAPH.json` is the first textual graph scan;
`GRAPH-v2.json` separates static imports from literal require candidates.

`FROZEN-MANIFEST.json` freezes ten original-only modes, 56 observations and 552
files at 2026-11-03T02:35:31.139371+00:00. Raw outputs precede any Rust candidate.
The freeze time is a local clock observation, not an independent immutable clock.
Raw streams, executed source pins, native process traces and original effect
files are the evidence; a completed observer label alone is insufficient.

## Exact protocol baseline

The authoritative tool descriptors are the actual `tools/list` responses in
`frozen/*/observations.json` and raw stdout, not a hand-transcribed schema list.
They retain full names, titles/descriptions, annotations, JSON schemas,
required/optional/default/bound/additionalProperties fields, order and omission.

| Observation | Exact qualification |
| --- | --- |
| inventory: 42 tools | Original neutral server lists a copied PUBLIC installed 41-entry registry plus its runtime identity tool. All configured endpoints are disposable, and list invokes none. This is actual original protocol output from metadata replay, NOT current installed endpoint wire or native freshness. |
| discovery: 29 tools | Original discovery over original fictional DailyRecord/saved/paper fixtures, including saved/native suppression and `class2_` naming. This topology is intentionally fictional. |
| dailyrecord source: 23 tools | Actual unchanged `dailyrecord-server.ts`, no native flags, no owner config, no backend. Actual endpoint/audit/SDK dispatch and unavailable responses. |
| protocol: 19 tools | Original neutral server with original fictional DailyRecord upstream. Real composition and wrapping, fictional source content. |
| bounds/graph | Original neutral server with explicitly fictional fault upstream. The fixture stimulates dispatch/delay/size; its output is NOT an independently observed real source result. |

`GRAPH-v2.json:selectedOriginalRoutes` is the exact selected public NAME →
endpoint → upstreamName mapping, including aliases and compositions. The copied
installed registry bytes and source-specific discovery registry are retained.
Changing one name at constant count fails. Keeping names while changing a route,
schema, annotation, description, default, omission or bound also fails. Original
`verifyEndpointInventory` checks required name presence, not full schema equality;
the migration comparator deliberately checks the whole observed descriptor.

SDK normalization is measured on wire. The fictional bounded probe omits `limit`
and actually receives default 2; its schema has min 1/max 3. `text` max 4 accepts
`four` and rejects `five!`. Original item-answer input accepts one label and
rejects 33 against the source-owned 32 bound. These probe values supplement,
not replace, the complete actual product schemas frozen from SDK output.

Omitted call arguments become `{}` before original dispatch hashing. Strict
unknown keys fail before upstream invocation. The original neutral unknown tool
returns `isError:true`, dual text/structured blocked data and `unknown_tool`;
the audited source endpoint uses `tool_not_registered` data instead. Unknown
method is JSON-RPC -32601. Do not merge these boundaries or invent one error
representation for both. `isError` absence differs from false; absent differs
from null; JSON scalar types and array order are significant.

The original tool-result wrappers preserve separate `content[].text` and
`structuredContent`; audit metadata can be omitted when it would exceed its
existing cap. The 600,000-character fictional upstream result actually returns
through the bridge. This reference does NOT invent a universal 512-KiB response
rejection: the source's metadata cap is not a result cap. The observer bounds
its own channels independently.

Actual newline stdio accepts CRLF and split frames, reports malformed JSON while
subsequent valid requests work, and emits finite metadata-only diagnostics on an
over-1-MiB frame. Original source has a 1-MiB framing bound and diagnostic cap 16;
this run observes three diagnostics, not saturation of the cap. No HTTP/vendor
MCP session transport, native authentication or permission system is replaced.

## Routing, identity and effects

Actual original name-based DailyRecord resolution and intake run inside one
upstream lifetime. DailyTest composition closes metadata before opening paper,
preserving independent namespace admission and account-lock order. The raw
events show metadata then paper/header dispatch. Safe authentication failure
propagates; arbitrary fictional error text is reduced to a finite public code.
Neither missing data nor a backend refusal creates current progress/save success.

The original owner-controlled `paperScopes` routes the canonical paper name to
the single admitted endpoint. A foreign `class2_` alias or unmatched student
returns `paper_scope_unresolved`/interview data before source dispatch. Caller
class/book hints never widen scope. Events and receipts stay separate from tool
payloads and show these executed paths.

`academy_runtime_identity` is artifact metadata. V1 says
`legacy_graph_incomplete`; V2 says `artifact_pinned` after original bridge and
endpoint-manifest checks. The original endpoint identity tool, with no identity
configuration, actually returns `unqualified`; its backend admission is
`unavailable`. With the fictional pinned endpoint manifest it returns
`qualified`, while the bridge STILL refuses missing backend admission. Neither
identity status grants authorization/content freshness. Tampering a pinned
disposable source artifact returns `release_artifact_mismatch` before dispatch.

When an upstream save has been sent, failure is conservatively
`academyWrites:null`, `effectStatus:"unknown"`, `automaticRetryAllowed:false`.
The pending-switch observation has exactly this result at `release_readback`.
The hung-save observation verifies the fictional handler was entered before
deadline/cleanup; it retains unknown settlement and performs no retry. A new
nonblocking flock child verifies the OWN disposable account lock was released.
No real account lock was opened. Output/exit status alone never proves an effect.

## Chosen candidate contract: fail closed on mandatory graph mismatch

This is an explicit scoped redesign required by adopted EDU-23, not a claim
that the original V1 already enforces it. Preserve the raw original observations:

* One existing original V1 process reads g1 then g2: its loaded bridge is unchanged
  while generation changes, and qualification remains legacy/incomplete.
* A pending fictional save at g2 followed by g3 refuses with
  `release_pointer_changed` and unknown write effect.
* V2 bridge mismatch refuses before upstream; a later valid original pointer
  resumes on that same original process. Pointer removal after a release was
  seen refuses rather than falling back to unpinned files.
* Original launcher validates bridge pin before launching. A tampered bridge
  exits 1 with `launcher_bridge_hash_mismatch`. Launcher relocation, using the
  same bytes and explicit fake install/source roots, still lists the source
  endpoint; it never infers evidence roots from cwd/executable location.

The future facade must require a COMPLETE mandatory graph identity before domain
dispatch. V1 incompleteness cannot become success from a tool count or a fresh
launcher. Bind per-process loaded bridge/binary, endpoint executables/arguments,
schema/contract/parser assets, explicit source mappings, policy/config/admission
digests, external modules/runtime and host/container edge identity. Source
account, authorization, lock/journal ownership and current pointer identity are
separate facts. A bundle digest alone does not pin an externalized dependency.

Any mandatory mismatch stops NEW domain dispatch in that candidate connection.
A pointer change during a possibly effectful pending request preserves unknown
settlement and cannot trigger another dispatch/retry. Do not silently resume
that candidate connection just because a pointer is restored; recovery uses a
fresh owner-qualified native connection and authoritative reconciliation under
existing field fences. Original same-process recovery above remains raw baseline
evidence of the intentional difference; it is not an oracle to weaken.

Keep existing old sessions immutable under their actual native owners. Their
original V1 behavior is not qualified as a candidate graph. Root must determine
the authorized old-client drain/refusal and new-connection proof before P9;
this reference neither reconfigures nor closes those sessions. Do not implement
a second lifecycle controller, agent loop, MCP transport, permission/session
manager, Hindsight DB or Work framework.

`judge.py:candidate_graph_qualified` is only a negative-control contract predicate.
It is NOT an artifact verifier or authorization decision: boolean status fields
cannot supply the actual byte, ownership and native-edge evidence above.

## State compatibility and retained TypeScript

Keep the accepted S09/S10 source cases, APIs, old readers and qualifications.
The original workflow record kinds remain observation/review/request/execution/
outcome/reconciliation; paths and bytes remain those in the immutable S10
reference CONTRACT's “Existing journal representation”. Student lock/ledger
groups omit lesson date; physical field fences span request/date changes.
Unresolved executions cannot be bypassed by a new requestRef. Preserve original
record keys, canonicalization, publication/fsync/lock protocols and uncertainty.
No new journal format or Rust-created replacement state is authorized here.

The journal static wire digest gate
`650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1`
in `lessonJournalHttp.ts:9` is unchanged. Bounded recorded/fake S09/S10 ports do
not prove actual HTTP authentication/acquisition, account content, native UID,
current/prior source linkage or trusted Ontology ingestion. Unqualified source
availability must remain unavailable/refused. Accepted S12 user scenario verdicts
do not change these facts or authorize P9.

ALL product implementation remains TypeScript in this reference commit. The
native JS/DOM parser, source-owner bridge/config loaders, DailyRecord/CISM/
textbook modules, Hermes environment lookup, SDK stdio and installed launcher
are retained owners, not claimed Rust ports. A future Rust facade translates
domain tools/protocol only; Root must explicitly retain or independently qualify
each still-required TS/native module, external JS import and ABI.

## Verification and limits

Per independent mode: wall ceiling 90 seconds, inherited per-process CPU 90s,
file ceiling 64 MiB, combined captured output ceiling 12 MiB. No aggregate
complete-modes timeout, blanket cap increase or expensive Rust slot. Linux
process/file/network traces retain exec argv, actual exit/signal and file paths;
per-process raw channels/start/exit metadata are copied into the corpus. Top
child pipes include Bun-native writes that bypass JS stream taps. Inner SDK
stdio uses stream taps. Thread/process statuses and signal exits are in the
native traces, including abrupt children that cannot write EXIT.json. Missing
early setup-child stderr is a disclosed failed-attempt provenance gap, not an
empty semantic result. No adversarial daemonization claim is made.

Normalization replaces ONLY UUIDv4 identifiers in first-seen order, actual
November-3 runtime clocks under named clock keys (equality retained), nonnegative
integer elapsedMs, and the declared disposable mode-root prefix. Fictional
2040 dates, source strings, descriptions, annotations, schemas, omissions,
arrays, scalar types, digests and error codes remain. JSON member order outside
text is immaterial; JSON in text is re-encoded preserving its original member
order. Raw framing bytes remain separate. Clocks on another day require a new
explicit reviewed normalization version, not silently broader masking.

Identity hashes are deliberately strict. A replay at a different absolute root
changes pointer/endpoint hashes and must report that difference; do NOT blanket
mask digests to make it green. A future candidate's new binary/graph identity
requires a separately reviewed, source-backed identity correspondence and actual
graph verification. This preparation supplies no permissive identity remapper.

The judge detects ten controls: same-count rename, schema, annotation,
description omission, maximum/default drift, same-name route drift, unqualified
graph, lying wrapper and changed source after green. `negative.py` also records
an actual child exit 23 inside an outer exit 0 and an actual fictional source
change after a green hash. These are disposable controls, not real builds.

Remaining native prerequisites are enumerated in `GRAPH-v2.json`. Actual
container bytes/binaries, UID, cwd, mounts, selected Hermes config/relay/unit,
credential retrieval and source account/content remain unperformed under this
grant. No installation, service/socket, container/daemon, SSH/relay, native
profile/config, source account, live GET/save/send, print/PDF/parent send,
Ontology append, original deletion or remote Git action occurred as intended
reference work. Early automatic dependency-resolution failure is disclosed above.
