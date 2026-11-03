# Source-only owner DailyRecord bridge

This increment implements the pinned original DailyRecord owner v1/v2 read,
name-resolution cache and admission behavior over one actual `http::Client<T>`.
It is source ready for Root integration, not compiled, executed or activated by
the author. F must supply the fixed `Client::search_students_by_name(&mut self,
input: &Js) -> data::Result<Js>` before Root compiles the combined source.

`Owner::new(client, admission, owner_record, managed_names, credential, runtime)`
validates the owner record, teacher and launch date before credentials or source
transport. No owner record confines native reads to the selected launch date;
the independent `is_date_admitted(date, None)` function instead retains the
original `2026-10-23` default. The v1/v2 schema retains strict keys, real calendar
dates, user decider, namespace, teacher key, fixed default, 500 UTF-16-unit note,
nullable v2 bounds, inverted-range refusal and optional configured/source_verified
student scope. `OwnerAdmission` cannot be constructed from unchecked fields.
Date validation and strict name-row validation reuse existing schema helpers;
owner issues use the existing Error::zod/ordered-part representation. The owner
name-pair maximum is 200, distinct from the existing resolver helper's 201.

`owner.source()` implements the existing core `service::Source`.
`owner.writer()` implements core `native::SourcePort`, without a raw HttpPort
implementation. Obtain `(admission, owner_record, managed_names)` using
`write_configuration()` and pass these copies plus the writer and the same
Runtime to the existing `NativeBackend::new`. Supply that backend and the source
as the separate mutable references required by `Service`. The new compile-only
`real_service_interface_fit` function demonstrates this exact composition.
The core native change removes only the unused HttpPort supertrait/import and
corrects its apply comment; every save-algorithm byte remains unchanged.

Both views share one Rc/RefCell state, one Client, one credential provider, and
one retained raw authentication result in sources::native::Backend. That result
includes synchronous credential failure, Client login failure and success.
There is no second login, independent auth cache or client clone in either view.
Every state access uses `try_borrow_mut`. A conflicting borrow returns the
native BorrowMutError.to_string() in generic Error/Fault parts before nested
transport. The reentrant-provider test bounds this behavior with a finite
callback. Credential values have private fields, no Debug/Clone implementation
and no environment/default lookup; providers must keep their own failure data
appropriate for the existing error boundary.

`native::StageError` carries Source(Error), Authentication(raw cause), or typed
StudentNameNotAdmitted. Only a known authentication stage is converted directly
to `Fault::trusted_authentication`. Other source errors use central lossless
parts conversion. Neither a class-looking name, a code nor arbitrary properties
grant native context. Converting a trusted Fault back through data intentionally
does not reconstruct its authority. StudentNameNotAdmitted remains a native
enum case for the future MCP resolver; it does not assert opaque JS class identity.

The future resolver must call
`source.search_students_by_name(exact_name, request_ref, lesson_date)` and retain
the typed result. Match StudentNameNotAdmitted to the original
`student_name_not_admitted` unavailable result, pass successful pairs to the
existing exact-name resolver, and use the existing failure projection for other
errors. Do not serialize the enum and try to recognize it by error name.
The source implementation passes only teacherLabel/studentName to F's actual
Client method. Legacy SourcePorts declare absence with the default optional
name capability; only that absence uses the configured-student fallback. A
failed available lookup never falls back or probes student keys. Implementers
of an available legacy SourcePort must override both has_name_search and the
optional search method; the concrete owner port always forwards F's method.

In source_verified mode the managed-name guard runs before authentication.
Dated resolution discovers/executes only the selected date and verifies exact
name, date, teacher and nonambiguity before caching. Every search first deletes
its request's prior resolution. Returned pairs preserve order; cache insertion
requires all exact-name pairs and one distinct key. The 32-entry cache uses
insertion order, delete/reinsert request replacement, optional date binding and
age 0..300000ms inclusive. Clock rollback, expiry, ID/date/request mismatch do
not admit a read. Configured student, fresh request cache and managed recorded
subject are tried in that order. Source::read passes both the original request
reference and recorded subject through; neither is silently dropped.
`read_routing_admission` preserves owner bounds (including null) and supplies
configured as the omitted v2 studentScope default without widening launch scope.

Owner-aware and legacy reads call the same existing observation body: current
occurrence, exact identity, target state, previous occurrence, six fields,
previous-homework evidence, tone and source revision are not independently
reimplemented. The actual writer calls Client::apply_journal_action with the
actual Approval and owner time. Service's concrete RequestApproval uses its
store/runtime and does not reenter owner state; it retains the existing ledger
and execution-fence ordering. Arbitrary injected callbacks must respect the
fallible borrow boundary. No code calls a raw write or core http::apply directly.

The immutable 6508 gate and accepted HTTP implementation are unchanged. A new
actual-Client check performs source-faithful synthetic login, then observes
`journal_mutation_wire_not_qualified` before approval with a deliberately inert
fictional document. Its DOM helper reuses the existing public-refusal test's
fictional shape with this owner's identity. No qualified handler preimage,
approved business success, real HTTPS or real owner activation is claimed.

All new cases are authored only in tests/owner_workflow.rs and
 tests/owner_admission.rs. Actual Client checks use finite in-memory login,
submission and principal HTML, never authenticated=true. Pure resolver/cache
and complete observation checks are explicitly fictional SourcePort tests.
There are no filesystem tests, store opens, sockets, environment reads, private
credentials, native account locks or service changes. The interface-fit helper
takes a Store supplied by a future caller and is not executed by any test.
Root owns combined compilation, selected case execution, independent review,
runtime effects, any future integration and verified pane closure.
