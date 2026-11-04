# Explicit DailyRecord MCP business connection

Source-only increment from c2a34811fb590bf5f9ba356a16ac121f9bfe3def. Compiler NOT RUN; every new test UNEXECUTED. This is not S11/native/runtime acceptance.

`RpcSession::handle_business(request, &mut driver)` lends a concrete `business::DailyRecord` for one call. The driver borrows an already-built actual core `Service`; it never accepts an arbitrary business-result callback. The default CLI, `handle` and `handle_supervised` retain their existing explicit qualification gaps. There is no transport/credential/environment/root factory or activation change.

| Endpoint-local tool | Actual implementation |
| --- | --- |
| academy_dailyrecord_intake | Service::intake |
| academy_dailyrecord_review | Service::review(input, true), durable review |
| academy_dailyrecord_save | Service::save, existing approval/fence/lock/journal |
| academy_dailyrecord_reconcile | Service::reconcile, existing authoritative readback |
| academy_dailyrecord_execution_status | service::status on the same Store |
| academy_student_resolve | Owner SourceView typed search, native::resolve_name |
| academy_runtime_read_admission | Owner SourceView routing admission; SHA256(JSON.stringify(admission)) |

The actual owner path is assembled outside RpcSession:

```rust,ignore
let mut reader = owner.source();
let mut writer = owner.writer();
let (admission, owner_admission, managed_names) = owner.write_configuration()?;
let mut backend = NativeBackend::new(
    &mut writer, admission, owner_admission, managed_names, runtime,
);
let service = Service { store, source: &mut reader, policy, backend: &mut backend, runtime };
let mut driver = DailyRecord::from_borrowed_service(service, &owner);
let response = session.handle_business(&request, &mut driver)?;
```

The caller owns source/backend/runtime consistency. Reader, resolver and writer must use distinct views of the same actual Owner, sharing its one Client and success/failure authentication cache, admission, managed names and clock. The supplied Policy must be the owner-approved policy for this Store. No payload field establishes these obligations. They are explicit typed construction obligations, not a claim that a trait object can be inspected to authenticate its owner. Root remains responsible for a qualified native owner/build graph and actual HTTP transport.

`from_borrowed_service_without_read_owner` explicitly represents an absent routing/search capability, with the same real Service requirement. It binds only to a graph whose pinned routing admission is JSON null. The probe returns exactly `{status:"unavailable", admissionSha256:null}`; resolution returns `student_resolve_backend_not_configured`. This constructor discovers no owner or credentials. A present Owner returns the configured digest without exposing admission contents or making a source request.

Each business call requires a dailyrecord endpoint session, a unique matching registered upstream name (or the existing endpoint-local admission probe), the pinned input schema and the existing core semantic schema. The actual canonical Store root must be under this graph's state root. The owner routing admission's stringify bytes/digest must match the graph's pinned admission. ReleaseGraph's fictional-only owner admission is unchanged; injecting a driver cannot promote JSON `kind:"fictional"` to native. Unknown/cross-endpoint calls never reach the driver. Registered retained routes outside these seven preserve their qualification refusal. No inventory/schema/annotation/pin is widened.

Name resolution preserves original purpose/request/display-name context, UTF16/JS trim rules, optional lessonDate, exact source rows, scope and ambiguity. `StageError::StudentNameNotAdmitted` is matched before generic Fault conversion. Other failures use existing `public_read_failure`; trusted authentication context is supplied only by the actual native stage, never inferred from serialized JSON, names or messages. No resolver/service/save/fence/journal algorithm was copied into the adapter.

Success uses the existing dual text/structuredContent result. Installed public SDK 2.3.1 `mcp-DIH4cS6P.mjs:1775–1826` gives unknown tools RPC -32602 and catches handler/input failures into text-only `isError`; `src-Cqbh3MYc.mjs:6534–6565` gives encoding failure RPC -32603. The source uses those result/error families. Handler Fault messages remain text-only, with additive `_meta["academy/businessEffect"]` after business entry. Typed source errors caught by Service/resolver remain structured public failure parts. Exact Zod validation/issue diagnostic text, opaque JS error-class identity and original TS stack origins are not established by the Rust validators. Existing default-session refusal/error behavior is preserved.

Graph readback runs before entry and after both successful and failed business calls, retaining the existing frozen-graph latch. Post-entry graph/RPC-output failure includes effect evidence and `automaticRetryAllowed:false`. Once save is entered, its failure cannot be reported as zero writes: it remains possible/unknown even if the graph or serialization fails. For other operations, zero Academy writes does not erase local Store effects; `localWorkflowEffects` directs reconciliation of the owner Store. Business Faults do not install a session-wide failure latch. The original queue-tail catch (`dailyrecord.ts:260–267`) permits a new judgment on the next request; the existing retained raw authentication-failure cache is separate. No automatic retry is introduced.

The new test target `business_endpoint` requires Root's explicit, existing, canonical absolute `S11_BUSINESS_TEST_ROOT`. Each case creates a unique finite fictional child and preserves all graph/Store output for readback. No fallback, original/private root, real network, child process or account-lock path exists in these cases. Positive durable observation/review/status/fence/reconciliation proof uses fictional Source/Backend ports while executing the actual Service/Store/approval algorithms. Separate cases use actual Owner/SourceView/Client with finite in-memory HTTP and literal fictional credentials to test shared authentication, typed source failures, name ambiguity and the public immutable 6508 refusal. The two evidence boundaries must not be conflated.

The missing immutable 650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1 handler preimage remains absent. No positive public-Client observation/write or fabricated handler success is claimed. `Client::journal_state` retains its gate before approval. An explicit fictional uncertainty backend consumes real request approval and throws; it never reports a fabricated persisted HTTP write.

Parent academy_dailyrecord_get resolve+intake in one child/client lifetime, history child/lock sequencing, metadata/learning/CISM/textbook/paper routes, actual HTTP/native owner/input/build graph, old-session/unknown-write/process/lock recovery, and S13/S14 remain required next increments. Individual P9 read/write/recovery/stability/deletion, weekday 14–22 KST and all private/live/native holds remain. No production completion, deployment, source recapture or runtime grant follows from this source freeze.
