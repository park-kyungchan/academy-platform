# S11 source error parts prerequisite

Status: SOURCE_READY_FOR_ROOT_ERROR_PARTS_BUILD_READBACK. Source authored only; compiler, new tests and independent review remain Root-owned and unperformed by this worker.

## Implemented boundary

`core::js::Fault` owns its dynamic name, message, legacy optional schema issues and ordered `Js` properties. Legacy constructors and `value()` output remain exact when extra parts are absent. Explicit null/undefined issues remain distinct from absence. When retained properties contain `issues`, that property owns the representation; the legacy `issues` vector supplies it only when absent. No leaked strings or serialized error JSON in messages.

`sources::data::{fault_from_error,error_from_fault}` centralize semantic parts conversion. A SourceError → Fault → SourceError roundtrip retains name, message, ordered properties and issues. The allocated native schema conversion and HTTP journal bridge use these helpers. The HTTP bridge still retains its original before-read error; after-read/write uncertainty and the existing approved entry, private wire mapping and immutable 6508 gate remain unchanged.

`core::workflow::read_failure::public_read_failure` uses the original finite catalog and explicit caller fallback. Its read envelope retains the old fallback. Diagnostics hash actual owned name + NUL + message. Error kind preserves the existing native name-based Error/TypeError/RangeError/SyntaxError classification, including legacy Zod/Syntax malformed classification. Catalog errors alone may project an integer HTTP status 400..599 and exactly these source operations:

- login_document
- login_submission
- owned_source_request
- cism_weekly_read
- cism_field_save
- textbook_answer_catalog
- getTextBookProgress
- day_record_group_selector

`Main` remains unlisted and is not renamed. Unknown properties, code, category, evidenceLocator, cause, reason and origin are not class authority. Unknown errors publish only the selected fallback and diagnostics, never the upstream message or arbitrary properties.

`Fault::trusted_authentication` is explicit native context for the selected original AcademyReadFailure authentication branch. Only that typed context permits recursive cause projection with `source_cause_unclassified` fallback and depth <2. Generic parts conversion never infers it from a name, code, properties or serialized marker. `Fault::value` and SourceError conversion retain the ordinary wrapper/cause properties but do not encode the native context. The future owner adapter must explicitly construct it after a known authentication stage; that adapter is not implemented here. Existing sources native authentication/cache algorithms are unchanged.

Backend-level save exceptions now derive only `dispatch_failed.reasonCode` through the finite projector with the original `dailyrecord_save_unqualified_no_automatic_retry` fallback. Reconciliation hold, unknown write count, durable execution/outcome algorithms and no-retry semantics remain exact.

## Precise unsupported semantics

Semantic error parts are not opaque JavaScript prototype identity. In particular, the original `ReleaseBaseUnqualified` class-specific `reason` branch is unsupported: a native Fault named ReleaseBaseUnqualified or containing reason does not establish that class. Its catalog message still classifies normally without a fabricated reason. Likewise, dynamic names do not establish original instanceof TypeError/ZodError identity; the pre-existing native name classification is retained. Non-Error JavaScript thrown primitives are outside this typed Fault API.

No actual allowlisted TS stack/native origin evidence is supplied; origin is omitted. This leaf does not add owner workflow ports, shared authentication ownership, v2 admission, trait-bound changes or MCP business joins. It does not prove the qualified public before → approval → write → after sequence, genuine HTTPS, native owner scope, activation or P9 acceptance.

## Authored checks and their effects (not executed)

Core `tests/source_error_parts.rs`:

- `legacy_fault_values_and_absent_null_issues_stay_distinct`: pure legacy values and issues presence.
- `finite_http_status_and_exact_eight_operations_are_projected_only_for_catalog_errors`: pure finite allowlists, malformed and unknown suppression.
- `dynamic_name_hash_uses_actual_name_nul_message`: pure independent digest constant.
- `trusted_authentication_cause_is_bounded_and_data_cannot_forge_it`: pure typed cause depth, unknown fallback and forged metadata rejection.
- `backend_catalog_exception_keeps_original_audit_reason_and_reconciliation_fence`: requires a separately selected existing canonical absolute `S11_ERROR_TEST_ROOT`. Creates exactly three fresh `source-error-parts-<pid>-<index>` fictional children, refusing reuse. Runs actual intake, review, request publication, workflow lock, approval consumption/execution fence, backend exception, outcome/audit readback and attempted replay refusal. Only fictional core Source/Policy/Backend are injected; no SourcePort success or real HTTP is claimed. Leaves all store files/locks/results in place. Uses runtime process/environment attribution fallback; no account lock, private root or network.

Sources `tests/source_error_parts.rs`:

- `source_core_source_roundtrip_retains_dynamic_name_order_absence_null_and_issues`: pure parts roundtrips and authority stripping.
- `actual_client_before_read_error_retains_typed_properties_through_core_roundtrip`: in-memory fictional Transport returns one 503 to the actual Client before-read. Approval panics if reached. Checks internal Main/503 retention and deliberate public Main omission. No socket, file or write request.

Root must grant and perform the fresh compiler and selected finite checks. No old test assertion, oracle, original TypeScript, manifest or lockfile was changed. Scope/pins/commit/protected record evidence is in the task preparation handoff, not a runtime approval.
