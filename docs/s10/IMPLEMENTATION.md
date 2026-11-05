# Existing Education workflow/store Rust candidate

> Current fixture ownership: [E02 fixture ownership](../fixture-ownership.md)
> supersedes migration pathname and replay instructions below. The dated source
> observations and acceptance statements retain their original historical scope.
> Maintained tests read crate-owned copies; historical reproduction requires the
> complete pinned recovery revision, not a partially retired current tree.


This implements the adopted S10 EDU-07–16/26 boundary on accepted S08
`39441f5d205b8a67e5c911da747d213a322d20da`, qualified original S10 reference
`977613f0b70159280ad1af8a54b17c6efd3bf30d`, and allocated scaffold
`0572c70c7ea1b36c38d08627a3f72bdd1c8cf681`. It is an implementation submission;
Root owns independent semantic review and acceptance. Nothing activates a native
endpoint, selects a real account, saves to Academy or sends to a parent.

## Native APIs

`workflow::store::Store::open(root, owner_root)` requires an explicit canonical
owner root, preserves existing directories, and refuses symlink components.
Publication is the existing mode0600 temporary file, complete write, file fsync,
close, hard link, unlink and directory fsync. Existing records are returned, never
replaced. Kinds, record/ledger/audit names and construction-order JSON bytes are
the original representation. `codec` parses JSON with an explicit container
stack, insertion-order properties, duplicate-key replacement and UTF-16 escape
units, including lone surrogates. Shared accepted `Js` encoders/digests retain
JavaScript number, integer-key enumeration and string encoding semantics.

`state` implements the original request/student/execution keys, state precedence,
ledger-only missingness and physical `(effectRecordKey, field)` fence across
requests and dates. Reconciliation takes precedence over outcome, then execution.
An index without its execution is available; it is not proof of completion.

`approval::{RequestApproval, LaunchApproval, FileCurrent, DurableConsume}` retain
reviewed action bindings and first-instruction binding, ledger-before-execution,
real current-user-file reads with the original16KiB bound, captured admission
identity, exclusive consumption and the older consume writer's original lack of
directory fsync. There is no issuer-key or actor-credential prerequisite.

`service::Service` takes explicit source, policy, backend and clock/entropy ports.
It implements prior-review checks, fresh recomputation under the student lock,
consumed/settled/uncertain decisions, per-field outcomes, status and reconciliation
of each execution's own lesson/physical record. Old values after uncertain
transport remain unresolved regardless of elapsed time. Authoritative new values
record observed persistence without causal proof. Exact scoped user resolution
never replays; a successor requires its own review/request. `native::NativeBackend`
retains independent namespace/date/student, row/target and batch-before checks,
then uses actual `http::apply` approval/write/readback logic over owner ports.

The review translation uses existing pure validators/JSON/digests/templates and
explicit parent/learning policy traits. Fictional policy values and permissive
callbacks exist only under migration tests. Original TypeScript/pure-core files
and Cargo manifests/lock are unchanged. The public runtime has no fake identity,
fault selector, expected-file lookup or TypeScript fallback.

`cism::{CismStore, Service}` retain private current-owner directory and
O_NOFOLLOW/regular-file/nlink1/owner/mode/size guards, direct exclusive0600 writes,
file+directory fsync, immutable review/request bindings and legacy plus
operation-specific dispatch/persisted filenames. Existing matching receipts or
current authoritative after-values settle according to the original CISM policy;
unresolved predecessors fence successors. Numeric/existing-value protections
remain. Save, PDF, print and parent transmission are separate; no downstream
facility is called by these save paths.

`Store::lock` uses Rust1.95 `File::try_lock` on the actual append-opened `.flock`
file, default20ms polling/default30s wait and `shared_writer_lock_busy` on expiry.
The open file description owns the lock; no PID heuristic or unlink/recreate is
used. Account serialization remains the existing outer `flock --wait 45` envelope,
which must precede workflow locking. The Rust code neither loads nor executes the
installed native child tail.

## Migration executable and verification

`edu-academy-workflow` is an offline migration participant, with its callback
adapter under `crates/edu-academy-core/tests/fixtures/workflow-store-rust-v1/participant.rs`. A first bounded
JSON line supplies operation, explicit root/ownerRoot, input, clock and owner-port
bindings. Further `{port,input}` / `{value}` frames invoke the typed ports; the
last frame is `{result}` or a finite error. Named operations cover store, ledger,
approval, action, review/save/reconcile/status and CISM get/review/save. The binary
has no source transport or native account default. Only the migration adapter
understands test abort responses; durable runtime hooks themselves have no fault
switches. This is not an MCP/native deployment.

The unchanged original scenarios and v2 normalization compare83 main frames,
12 supplements and14 frozen old-state reads. Fourteen actual Rust-emitted states
are additionally read by unchanged original TS and compared directly, retaining
all existing journal-byte hashes. Independent observers read actual target,
all journal files, effect/denied logs and downstream counts after every child,
including crashes; they also reject undeclared files elsewhere in the suite root.
The old stdout-only judge is not the effect authority.

| Cases | Actual comparison boundary |
| --- | --- |
| EDU-07 | Separate-process duplicate, settled decision and immutable reviewed input |
| EDU-08–11 | New request/date fences; old-value uncertainty/time; desired readback; backfill target |
| EDU-12–13 | TS→Rust and Rust→TS workflow/account contention, cancellation, holder death, retained FD/inode |
| EDU-14–15 | Real Rust exits70/71/72/73/74 after index/fence/before write/after acceptance/before readback |
| EDU-16 | Legacy and operation CISM readers, uncertainty, observed settlement and reviewed successor |
| EDU-26 | Actual fake PDF/print/parent-send counters remain zero on successful saves |

Only original declared participant-PID, UUID/action-digest, wall-clock, audit
multiset and filesystem-observation normalization is used. Actual inner Rust PIDs
are mapped to their recorded logical participant; source identities are not
aliased. Raw process/file/effect bytes remain in the absolute task evidence.
Audit multiset equality is not causal ordering.

The finite original contract remains the comparison domain: ordinary source-typed
JSON and explicit owner ports. It does not qualify arbitrary host objects/proxies,
all malformed JSON/Zod error diagnostics, every schema-invalid input, live HTTP,
credential/native acknowledgement custody, production root races against
nonparticipants, S09 source parsing, S11 MCP routing, P8 user verdict or P9
activation/rollback. Additional workflow schemas enforce source shape/value guards;
full newly extended Zod error-array equivalence is not claimed. These limits do
not replace or narrow the adopted S10 cases, and no new expected contract was
created. Root must review the complete source and actual evidence.
