# Existing Education workflow/store reference v1

This is a migration-only ORIGINAL TypeScript reference at source
`39441f5d205b8a67e5c911da747d213a322d20da`, before an S10 Rust implementation.
It introduces no runtime journal, production authority, receipt protocol or
controller. Source acceptance, real Academy behavior, mixed Rust/TS compatibility,
independent review and full S10 acceptance are not claimed.

## Callable closure and caller ownership

`dailyrecord-server.ts:15–24` creates the owner workflow after native launch
verification, then passes the same native backend to `DailyRecordService`.
`dailyrecord-tools.ts:51–95` exposes intake, review, save, reconcile and execution
status. `neutral-server.ts` supplies bridge-observed attribution and routes to
the selected endpoint. These servers and the bridge are **read, not started**.
The reference calls the original service methods and original native write
backend directly. CISM calls the original registered handlers, their original
Zod schemas and original dual text/structured-content result wrapper through
an inert registration map; no MCP client, service or socket is started.

DailyRecord exercised closure:

1. `DailyRecordWorkflowStore.publishOnce('observation', …)` publishes a literal,
   source-type-compatible old observation with a fixed handle. This deliberately
   exercises the durable old-handle reader, rather than manufacturing an intake
   success. The observation, its scope, subject and request remain separate.
2. `DailyRecordService.review` recomputes the original review and persists the
   original immutable review. The input is then saved as an input fixture.
3. A **different native Bun process** calls `DailyRecordService.save` with that
   input. It uses original `NativeDailyRecordBackend.saveDailyRecord`, original
   `applyJournalHttpAction`, original `requestBoundApproval` and original store.
4. Further processes call the original duplicate/save, reconcile, status and
   old-state readers. No source-specific refusal is rewritten into success.

`fixture.ts` supplies fictional numeric identities, dates, current and previous
records, exact field strings, a file-backed fake source, an explicit fake
current-user event and fake policy ports. The read port returns an explicitly
synthetic `DailyRecordObservation`; it is not a source-parser oracle. The native
write backend still performs its actual namespace/account/student/date,
independent row and journal-state checks. `applyJournalHttpAction` performs its
actual target, before-state, effect-record and digest checks.

The fake parent/learning policy implementations satisfy the original injected
interfaces but do **not** prove canonical Education policy, content truth or
production authorization. In particular `learning.review => []` is a fixture
port, not a new admission rule. All fixture students, account IDs, text and
instructions are invented locally, independent of Hindsight/student records,
S08 Rust outputs and production data. CISM's `sourceMode: live_source` is the
original type's literal in an explicitly fake report; it does not describe an
actual live read.

Selected runtime exports: `DailyRecordWorkflowStore`,
`createDailyRecordWorkflow`, `workflowLockKey`, `workflowRequestKey`,
`executionKey`, `ledgerGroup`, `fieldExecutionState`, `studentExecutions`,
`unresolvedOnField`, `requestBoundApproval`, `canonicalJson`,
`canonicalActionDigest`, `createDailyRecordSaveAdmission`,
`liveSaveRequestEvidence`, `durableSaveConsumeOnce`, `DailyRecordService`,
`NativeDailyRecordBackend`, `applyJournalHttpAction`,
`journalHttpActionDigest`, `registerCismWeeklyTools`, `cismHash` and the handler
schemas are consumed directly or transitively. The predicates
`needsReconciliation`, `settledEffect` and `consumedWithoutEffect` execute on
the actual service path.

Scope is finite, ordinary source-typed JSON inputs plus the fixture ports and
fault hooks listed below. It does not cover arbitrary JavaScript values,
malicious proxies, all schema bounds or every module export. Intake handle UUID
creation, real DOM/HTTP parsing, network deadlines/cancellation, owner launch
verification, native credential retrieval, native CISM acknowledgement custody,
full MCP/bridge graph and retained observing JS ports remain their existing
owners. S09/S11 and later Rust participation must qualify their actual boundaries.
No production server/configuration loader or aggregate package test is imported
as an executable entrypoint.

## Existing journal representation

All paths below are relative to the caller-supplied owner workflow root. The
native default is `<nativeOwnerArtifactsRoot>/dailyrecord-workflow`; the native
constant is `/opt/data/profiles/dc-gang-a-dev/artifacts/`. The reference substitutes
only a NEW disposable explicit root and calls no native configuration loader.

| Item | Exact original representation |
| --- | --- |
| Record kinds | `observation`, `review`, `request`, `execution`, `outcome`, `reconciliation` |
| Record path | `<kind>/<sha256(kind + '\n' + id)>.json` |
| Record bytes | `JSON.stringify({kind,id,record}) + '\n'`, in construction order |
| Publication | New mode0600 temp file, write, file fsync, close, hard-link to final name, unlink temp, directory fsync. Existing name returns original `get` result; never replaces it. |
| Reader | ENOENT returns null; otherwise JSON parse and exact `kind`/`id` check, with `workflow_store_record_identity_conflict` on mismatch. No new schema is inferred. |
| Ledger path | `ledger/<sha256(group)>/<sha256(executionKey)>.json` |
| Ledger bytes | `JSON.stringify({group,key}) + '\n'`, same immutable publication |
| Ledger read | Visible `.json` names only, matching group, returned keys sorted. An entry without its execution record is skipped by `studentExecutions`. |
| Audit | `audit/<sha256(requestKey)>/<15-digit-Date.now>-<UUID>.json`; `{requestKey,at,pid,...event}` + newline. Metadata only; field/instruction text is not an audit event. |
| Directories | Created recursively mode0700; source does not retroactively chmod existing roots. |

`canonicalJson` here is the ORIGINAL save-admission implementation: recursively
sorted JS object keys, undefined object properties omitted, array order retained,
no whitespace. SHA256 is UTF-8. CISM uses ordinary `JSON.stringify`, not that
canonicalizer. The native HTTP action owner independently checks its own action
digest. No accepted S08 Rust function supplies any expected bytes.

Keys are original function outputs, frozen as exact strings in expected data:

- Request key: canonical `{scope,requestRef}` including the full dated scope.
- Student lock and ledger group: canonical
  `{sourceNamespace,teacherId,studentId}`; lesson date is absent.
- Execution key: canonical `{requestRef,student:workflowLockKey(scope),field,
  effectRecordKey,beforeSha256,afterSha256}`; the physical record is included,
  lesson date is absent. The original field remains part of the key.
- A physical fence search reads all student executions and filters the same
  **field and effectRecordKey**, independent of request and lesson date.
  A different field or different physical record is not the same fence.

`OLD-STATES.json` pins 14 snapshots with the exact on-disk JSON bytes emitted by
these original functions (plus the literal legacy CISM record from its original
reader contract). Snapshots retain observation/review/request and action lineage.
The compact fixtures omit audit and lock files; complete original audit bytes,
PIDs, lock metadata and all channels remain in task-owned native evidence.
Copying synthetic journal bytes into a fresh offline reader is **not** a live
lock/root migration or a business rollback procedure.

## State and authority semantics

`fieldExecutionState` uses this exact precedence:

1. Reconciliation classification -> `reconciled_persisted`,
   `reconciled_not_persisted` or `reconciled_conflict`.
2. Outcome status `persisted`/`noop`/`not_persisted` ->
   `settled_persisted`/`settled_noop`/`settled_not_persisted`; other outcome statuses
   -> `reconciliation_required`.
3. Execution only -> `dispatch_outcome_unknown`; no execution -> `available`.

Request publication binds `requestRef` to the first instruction text digest.
The actual stored instruction, optional session/received time, observed or
declared attribution and policy pins remain separate. Caller actor attribution
is audit evidence, not a credential. The synthetic fixed attribution is explicitly
supplied through the original interface and is not a claim of bridge observation.

Save checks the prior prepared review's exact scope, request, handle, policy pins
and canonical input digest before locking. An edited resubmission is refused,
even when the original execution settled. Inside the student lock it first
checks unresolved physical-field predecessors, then consumed-without-effect and
settled executions, then a fresh original review. Durable approval consumption
adds the ledger **before** publishing the execution, before transport.

`requestBoundApproval` checks reviewed field, student, teacher, lesson date,
physical effect record and exact before/after strings. It is not a standalone
replacement for the backend's independently verified source target. Native
target/class/course/membership/wire checks remain in the original backend and
HTTP action owner. The supplemental refusals distinguish those boundaries.

The legacy launch-pinned admission is separately exercised: exact actual-current
`runtime`, `requestRef`, `requestTextSha256`, `currentUserEventRef` must match a
fresh `liveSaveRequestEvidence` read. Admitted tuple keys use their ORIGINAL
ordinary JSON digest, and `durableSaveConsumeOnce` uses exclusive `wx`, write,
file fsync and close. It does not add a directory fsync; the reference does not
silently strengthen that older implementation. Its on-disk consumed bytes are
frozen, and a fresh approval object cannot reconsume the same tuple.

An old-value readback after uncertain transport remains unresolved. A returned
transport with old value produces a consumed not-persisted transition, which
still cannot replay under the same request. Elapsed time does not settle an
uncertain dispatch. Reconciliation reads the execution's **own** lesson scope
and checks the current physical target before classifying the field. A backfill
binding a different previous record leaves the old target unresolved/unobserved.

An authoritative desired value records `classification: persisted`,
`basis: readback_new_value`; this is observed persistence, not request causation.
The implementation makes no stronger causal assertion. A user-instruction
resolution names exact executions on the exact scope, records instruction and
digest with `basis: user_instruction`, and does not replay them. Only a separately
reviewed new request is subsequently dispatched in the supplemental scenario.

## Kernel locks, order and lifecycle

Original workflow lock path is
`locks/<sha256(workflowLockKey(scope))>.flock`, opened append/mode0600. Bun FFI
calls libc `flock(fd, LOCK_EX|LOCK_NB)`; poll default20ms, deadline default30000ms,
and refusal is `shared_writer_lock_busy` when `Date.now() > deadline`. Unlock is
`LOCK_UN` in finally and close in the outer finally. No unlink, PID ownership
heuristic or stale-file deletion exists. Tests override only the original
method's supported wait/poll options for short workflow deadlines.

`ACCOUNT-SOURCE.json` pins the selected original installed endpoint metadata.
The three account-participating endpoint entries use exactly
`flock --wait 45 <shared-account-lock> <native-tail…>`. The outer OS lock wraps
the complete native process, which can then acquire the workflow lock:
**account before workflow**. The reference never runs the native tail, opens the
real account lock, loads credentials, or invokes Docker. It executes the exact
selected flock prefix with only the lock pathname replaced by a disposable
`native-academy-account.lock` and a vetted original-TS fake child as the command.

The account test actually waits for the original 45-second deadline and observes
exit1/no stdout/no stderr; the second TS participant has not started, hence it
cannot have taken the workflow lock first. A Python `fcntl.flock` participant
opens that same disposable file while held, observes conflict, retains its file
descriptor across holder death, then acquires the same object. No rename/unlink
or copied lock is used. Both account and workflow device/inode relationships
must remain unchanged. The holder and its TS descendant are SIGKILLed and reaped;
a process-local Linux subreaper handles adopted descendants. A new source-prefix
participant acquires account then the original TS workflow lock.

These tests establish TS/TS and native-OS/Python flock participation only. A
future Rust participant must use these same files and demonstrate contention,
ordering, deadlines, cancellation and old-state readers; that mixed claim is
still unperformed. Participating single-host filesystem writers are covered;
nonparticipating UI/remote writers can race between comparison and write.

## Fault boundaries and actual effects

| Fault | Injection after original operation | Actual child exit | Subsequent source observation |
| --- | --- | --- | --- |
| Index only | Actual `addToLedger` returns, before execution publication | 70 | No dispatch/execution; available, not complete. A fresh allowed same save can dispatch. |
| Durable fence | Actual execution publication completes, before approval returns | 71 | Unknown dispatch, old value, conflicting save blocked; uncertainty retained despite fake dispatch count0. |
| Before dispatch | Approval returns to original HTTP action; fake write entered before effect | 72 | Same conservative fence and no replay, count0. |
| After acceptance | Fake target persisted value, write has not returned | 73 | One fake dispatch; no outcome. Second save blocks; authoritative new value reconciles persisted. |
| Before readback | Fake write returned; original second read entered before observation | 74 | One fake dispatch; no outcome. Same no-replay/observed-persisted behavior. |

Fault hooks wrap and call the original instance methods before exit or act in
the explicitly injected fake HTTP port; original source bytes are untouched.
Processes are real Bun children, not in-memory restart simulations. Per-case
raw stdout/stderr, exit/signal, PIDs and reaping are retained. Parent drivers use
monotonic total deadlines, 4MiB streaming output caps and process-group cleanup.
No failure is retried against an uncertain fake effect; each scenario has its
own NEW root and deliberately retained source/journal state.

Fake source logs distinguish authoritative reads, fake authentication, exact
action and source digests, approval decision, fake dispatch and fake acceptance.
The driver independently reads this log after each child and requires it to
equal the child's snapshot; denied transport is a separate fatal channel.
PDF, print and parent-send facilities are present as fake port methods with
operation counters. Successful save leaves each actual downstream count0.
The original result also retains `parentSends:0`, separate parent instruction
and `automaticRetryAllowed:false`; no implicit downstream action is added.

## CISM legacy compatibility and rollback readers

Owner root is native `<artifacts>/cism-weekly`, replaced here by a NEW fake root.
Original `directory` requires a real private0700 current-owner directory.
`readSafe` uses O_NOFOLLOW and requires regular file, nlink1, current UID,
mode without group/other access and size<=1000000, then JSON parsing.
`exclusive` uses `wx`/0600, direct JSON+newline, file fsync and directory fsync.
Unlike workflow publication it is not temp+hard-link; a mid-write crash can
leave a partial file that the old reader refuses. No format redesign is made.

- Field key: `cismHash([teacherKey, periodStart, fieldRef])`.
- Legacy dispatch: `<fieldKey>.dispatch.json`; done file replaces suffix with
  `.persisted.json`.
- New operation: `<fieldKey>.<reviewDigest>.dispatch.json` and corresponding
  `.persisted.json`. Both legacy and operation-specific names are read.
- Review: `<reviewDigest>.review.json`, exact parsed input without attribution,
  period/account/wire/policy bindings and separate attribution.
- Request: `<cismHash([teacherKey,requestRef])>.request.json`, immutable exact
  request. Rebinding is refused.

An unresolved legacy or new operation fences all later requests on that field.
`settle` accepts an existing matching persisted receipt, or writes one only when
the current authoritative field equals that dispatch's intended after-value.
Observed equality is the **specific original CISM policy**, not a generic
no-effect proof or permission to replay. The legacy dispatch bytes remain
unchanged. Once settled, a separately reviewed, separately requested explicit
narrative completion is allowed and gets its own operation receipt. The same
request cannot become a successor. Numeric values and existing nonempty fields
retain their original protections. `weekly_get` can create an observed-persisted
receipt; “read” here is not a promise of zero local journal writes.

The 14 old snapshots are copied only into NEW synthetic reader roots. Every
original frozen file is hashed before and after the actual old reader. Rollback
compatibility here means the unchanged original TS reader still understands
the existing journal states and leaves retained bytes intact. It is not a
tested Rust binary rollback or an assertion that restoring an older release
pointer reverses a source write.

## Expectations and normalization

`EXPECTED-v2.json` is the normative 83-frame source transcript.
`SUPPLEMENT-EXPECTED.json` freezes12 additional source frames;
`OLD-EXPECTED.json` freezes14 actual old-reader results;
`ACCOUNT-EXPECTED.json` freezes account/order/deadline/kernel-object observations.
`EXPECTED.json` is preserved initial v1, unqualified because it kept volatile
audit emitter PIDs. V2 changes only their documented logical aliases, derived
from original raw channels, without changing source values, digests, errors,
effects or decisions. It was never adjusted to a Rust candidate.

Only these nondeterministic observations are normalized:

- Original random native action attempt IDs and their **linked** action digests
  become per-scenario encounter-order aliases. Both independent original digest
  functions must agree; action, consumption and execution references share the
  mapping. Raw original action/digest bytes remain in channels/frozen snapshots.
- Actual wall-clock `recordedAt`, `reconciledAt` and audit `at` must be ISO
  millisecond UTC strings before substitution. Fixed fixture times remain in
  source input/observation/policy fields. Audit emitter PIDs map only to the
  actual recorded source participant, never to teacher/student/account identity.
- Audit filename order within a clock millisecond is UUID order. Audit records
  are compared as an explicit multiset; all other arrays remain ordered. All
  raw names, bytes and timestamps are retained. Meaningful dispatch/consume
  ordering is compared in the independent append-only fake operation log.
- Disposable lock device/inode and filesystem timestamps become aliases only
  after same-object checks. Elapsed lock times must satisfy the measured bounds;
  full actual times and identities remain in native observations.

Required store JSON text retains original serialization/key order/whitespace
and newline after only those substitutions. Top-level transcript object-key
order is not a semantic check. JSON duplicate keys/nonfinite values, bool/integer
substitutions, extra fields, array changes and unexpected stdout are refused by
the strict judge. All raw process channels stay separate from normalized data.

Fourteen actual mutant children qualify the comparator: wrong value, missing
field, changed identity, changed effect, changed refusal, bool/integer,
plausible output+exit17, junk stdout, stderr, independently observed denied
dispatch, independently observed fake dispatch, extra field, overbound stdout
and timeout. An unchanged positive control must pass. No mutant uses a real
socket or a real business effect.

## Retention and remaining authority

All unresolved fake fences, receipts and source states are intentionally retained
under the exact S10 `reference-v1/native/` note root. They are not pending live
actions. No running child or lock owner is required to preserve uncertainty.
Root may dispose of these wholly synthetic roots after independent review and
evidence retention; no real owner original/configuration/lock is a cleanup target.
The worktree's ignored Zod link is task-owned and recorded in SOURCE-PINS; the
existing installation's pinned entries are rehashed after execution.

This reference needs independent Wonder review and Lead disposition. Rust
implementation, actual mixed-language participation, real transport/policy,
activation, P8 user verdict, P9 cutover/deletion and production completion remain
outside this stage. S07's fake receipts/absence seals are not Academy authority,
and its schema is not substituted for these Education journals.
