# S11 Rust facade — immutable session and native transport

> Current fixture ownership: [E02 fixture ownership](../fixture-ownership.md)
> supersedes migration pathname and replay instructions below. The dated source
> observations and acceptance statements retain their original historical scope.
> Maintained tests read crate-owned copies; historical reproduction requires the
> complete pinned recovery revision, not a partially retired current tree.


Status at the session-source freeze on 2026-11-03: the first increment at f1e0554 was compiled by Root and its 15 bounded tests passed. Core intake at 05fd23a is independently accepted within its bounds: nine unique passing cases comprise eight unchanged 8f executions plus the one repaired exact-05 execution. The initial audit-fixture assertion failure is retained; no product behavior was changed to fit it. The lock at this base contains 157 package records / 147 distinct names, preserving all 156 old records / 146 names.

This session increment is source-only, uncompiled and unexecuted. Root owns dependency-cache/lock resolution, compilation and finite test/effect grants after this freeze. Neither earlier acceptance nor this source establishes full S11, protocol parity, native graph admission or activation. The accepted original-only qualifier at051 is unchanged.

`edu-academy-mcp` contains the retained legacy initialize/version preference, ping, tools/list, tools/call envelopes, typed IDs, ordered UTF-16 Js codec, source-metadata registry and a guarded JSON Schema subset. SDK2.3.1 source anchors are server dist/mcp-DIH4cS6P.mjs:1338, src-Cqbh3MYc.mjs:6486/6540/7159 and core dist/auth-BNDyLTqp.mjs:4. Exact SDK invalid-parameter issue serialization and all regexp/format edges still require qualification. No defaults or domain refinements are invented at the bridge.

The one embedded identity-tool metadata object is copied from the public original inventory stdout at migration/mcp-runtime-reference-v1/frozen/inventory/commands/neutral/stdout.bin (SHA25625ab9db16ecf5bcb4f4cdc2a8fee795a2616bc88c5a8e1428e046e7919774fd0). No business answers/captures are embedded in production code. Release registries supply the other tool metadata. Configured routes deliberately return isError/qualification gap because their actual Rust adapters are not in this grant; tools/list is metadata, not a claim of implemented operations.

## Versioned graph

V3 chooses terminal mismatch refusal for a whole MCP connection. Startup requires `--install-root /absolute/canonical/private/root`; no cwd or home defaults, legacy conversion or hot loading. The root has an owner-readable release.json (<=65536 bytes):

- schemaVersion=academy.mcp-release/3 and generation matching1..64 [A-Za-z0-9._-], excluding . and ...
- bridge, launcher, build, routing, endpoints, registry and owner pins, each {path:absolute,sha256:lowercase64hex}.
- build/owner/routing/endpoints/registry/endpoint manifests <=4MiB; immutable pinned artifacts <=64MiB. These are V3 artifact budgets, not domain or MCP response limits.

Owner manifest: strict {schemaVersion:academy.rust-owner/1,kind:fictional,uid,installRoot,evidenceRoot,stateRoot}. Each root must already exist, be distinct, private and owned by the effective UID, with canonical no-follow ancestry that is not writable by other users. Native/container kind is explicitly refused until actual owner binding is implemented and independently proven. Source/evidence and mutable state roots have distinct roles. Nothing creates, migrates or erases a store.

Build manifest: strict {schemaVersion:academy.rust-build/1,sourceRevision:40hex,semanticContractVersion,cargoLockSha256:64hex,artifacts:[{role,path,sha256}]}. It binds declared immutable artifacts but does not establish loaded dynamic-library closure, compiler provenance or native consumer loading by itself.

Routing retains {admissionPath,admissionSha256,paperScopes?}; its admission is included in pinned graph reads. Public registry entries retain strict {endpoint,upstreamName,tool}, where tool metadata remains source-owned. Endpoints map exact endpoint names to {executable:pin,identity:pin,args:[--install-root,root,--endpoint,name]}. Endpoint identity is strict {schemaVersion:academy.rust-endpoint/1,endpoint,semanticContractVersion,launchArgumentsSha256,artifacts};1..128 artifacts and exactly one matching entry. Roles are entry/source/policy/admission/config/dependency. All declared immutable pins are checked; no shell, Bun, caller command or credential environment is selected.

Every request and valid notification checks the frozen pointer/artifacts/root identity before handling or mutating session state. Requests also check before returning. Any observed change/removal latches refusal, even if original bytes are restored. New connections independently open a complete graph. Pin validation is bounded descriptor-relative file admission with before/after metadata and a same-descriptor second byte read; it is not an atomic multi-file snapshot or protection against every malicious same-UID mutation.

The launcher verifies its loaded executable inode/digest, verifies the bridge graph and replaces itself with the native bridge through exec. Stdio/native exit status are preserved. The bridge also verifies its loaded executable. A moved executable with a stale path pin refuses; a legitimate move requires a new separately qualified complete graph, with data/state provenance unchanged. No runtime installation/config publication is performed.

## Limit and effect scopes

The original stdio-diagnostics.ts input adapter alone establishes1MiB LF-inclusive input lines. It reports malformed lines/EOF fragments without payloads, accepts CRLF/split input, and caps its diagnostics at16; protocol diagnostics have their own cap16. Pinned SDK default child receive buffer is10MiB, a distinct stream contract. No generic1MiB output/domain cap is introduced. Original product `_meta` addition has a524288-byte SOFT omission boundary: a valid600000-byte result remains intact, including its dual text/structured representation.

The dedicated bridge joins RpcSession → dispatch → endpoint plan → one owned native child. The same child negotiates initialize, receives initialized, lists tools, satisfies the original required-name inventory check, and returns its pinned endpoint identity before a configured call. A dailyrecord endpoint also needs the exact projected read-admission hash. The product endpoint currently refuses that admission and every unimplemented domain route explicitly. A healthy protocol or identity handshake cannot turn those gaps into successful business results.

The original stdio SDK accepts preinitialize calls and repeated initialize. The latest successfully parsed clientInfo replaces the prior client; no stricter lifecycle gate is added. Invalid envelopes are dropped with capped payload-free diagnostics, while valid notifications traverse the V3 terminal graph latch. Validation uses the existing core JS whitespace set for AJV date-time separators and an escape-aware limited regexp translation, preserving literal backslash+d. Unsupported regexp semantics fail closed. This is not full ECMAScript regexp or SDK issue-serialization parity; domain Zod defaults/refinements remain endpoint responsibilities.

For workflow calls, dispatch discards caller attribution and injects observed bridge/client/process metadata; the declared actor is evidence, not authentication. Exclusive descriptor-relative dispatch/result/failure/cleanup receipts are written beneath the admitted evidence root, using fresh occurrences and hashes rather than arguments/results in diagnostics. The success branch preserves upstream _meta and applies the existing soft metadata omission. Identity results keep their original branch. Inventory failures retain missing names; bounded upstream occurrences remain attributable. Receipt failure returns a reconciliation error and retains possible-write uncertainty.

Possible-write is marked immediately before a save request can leave, after serialization and graph/control checks. Cleanup and a final graph readback occur for both outcomes. A disconnect or graph change never rewrites a possible effect to zero or retries it. Transport return alone cannot prove persisted state. Durable workflow fences, owner locks, reconciliation and actual business adapters remain joins to the existing core APIs; these product receipts do not replace workflow journals.

## Owned native process lifecycle

Only the dedicated binary installs safe SIGTERM receipt and subreaper status, before starting its I/O threads. Library construction and test hosts do neither. signal-hook is pinned to 0.4.4 with only iterator enabled; existing rustix fs/process features supply safe parent-death, process-group, pidfd and reap primitives. Each request launches the exact pinned executable/argv in a fresh group with an empty environment except verified parent metadata. The child verifies its loaded executable and parent before domain work.

Work defaults to an absolute 30-second deadline. Owned nonblocking child pipes and bounded input/output handoffs poll cancellation, EOF, parent TERM and graph mismatch. Graph verification repeats after each verification plus at most 100 ms of polling and immediately before every upstream request; it is not a filesystem watch. Cleanup has a separate absolute two-second budget: close input, 50 ms grace, TERM, then KILL after another 150 ms, discover only positive owned ancestry/adoptions, reap known identities and drain both pipes. Root identity/start ticks, actual exit/signal, reaped IDs, pipe EOF and qualification failures are retained. No whole-host process scan, inherited CLI descriptor-flag change or unsafe pre-exec callback is used.

The isolated binary may adopt descendants that escape the initial group; pidfds and PID/start checks retain ownership. A process limit of 256 fails closed. Partial inventory/setup/reap/drain failures remain cleanup gaps. The pre-exec hard-kill window, malicious same-UID mutation, and uninterruptible host filesystem/kernel stalls are not solved by these cooperative deadlines. Native owner admission remains refused.

Inherited stdio workers can remain blocked; their actual join booleans are recorded rather than inferred from dropped handles. The binary exits through the safe low-level exit after owned cleanup, avoiding at-exit stdio-lock deadlock, and restores/re-raises received TERM through the pinned safe public API. A failed re-raise exits 1 rather than fabricating signal status. Native process/pipe/lock evidence still requires Root's separately allocated execution.

## Open implementation and proof gaps

- New supervision/session code and the new signal dependency are not compiled or executed by this source author. Root must resolve the minimal native lock delta, compile, then inspect actual finite test/effect artifacts. Safe API selection is source evidence only.
- Core intake/durable observation exists with bounded independent acceptance but has no MCP business adapter yet. Source/SourcePort/HttpPort/Policy joins, saved capabilities/status/labels, paper and composition routes, metadata/item/prestudy/textbook/CISM ports remain named gaps. The disjoint owner HTTP-port source leaf is separately owned and is not silently integrated here.
- Genuine HTTPS, owner read scope, cookies/authentication, native owner/profile/UID graph, compiler/dynamic-library provenance and complete executable inventory/consumer loading remain unqualified. Artifact pins do not prove them.
- Pending writes and recovery against actual workflow journals, genuine reference/candidate comparisons and same-inode native lock release still need their own evidence. No re-recording or replay of accepted originals is implied.
- Full SDK schema issue serialization, server-initiated requests, paginated inventory, arbitrary regexp parity and all domain validation remain outside this bounded transport join.

New source tests are session_join (four functions), validation_edges (two), and process_lifecycle (six). They exercise actual native dispatch against a test-only fictional stdio peer: first inventory refusal, observed attribution, pre/reinitialization, diagnostic/drop/latching, a 600000-byte payload, cancellation, EOF, parent TERM, ignored TERM, escaped descendants, blocked output, graph switches before/after possible writes, and reacquisition of the same fictional lock inode. They retain fresh fixture files and native receipts. The harness=false stdio peer provides only fictional wire faults/results, never a production domain port.

Those tests require explicit absolute S11_TEST_ROOT, S11_MCP_BINARY and S11_STDIO_PEER paths selected by Root. Existing 15 MCP tests, old core tests and qualifier sources are unchanged. No test was run or passing-case count added by this source increment.
