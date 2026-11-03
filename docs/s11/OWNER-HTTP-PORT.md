# S11 owner-qualified HTTP journal port

This source leaf adds `Client<T>::apply_journal_action(action, approval, now)`
over the existing injected `Transport`. The owner supplies the real
`workflow::approval::Approval` implementation and the current runtime timestamp.
Neither caller flags nor the timestamp grant authority. This leaf implements
client glue, not an HTTPS transport or operational activation.

The source reference is `AcademyHttpClient.applyJournalAction` in
`apps/academy/dc-gang-a/src/lms/academyHttpClient.ts` (base lines 803–819), plus
`JOURNAL_HTTP_SOURCE_CONTRACT` in `lessonJournalHttp.ts`. The immutable Rust
`edu_academy_core::workflow::http::{validate_action, apply}` owns action validation,
state qualification, approval consumption and outcome classification.

The public method validates the action before checking exact teacher-principal
binding and authentication. A private `HttpPort` adapter then calls the existing
`Client::journal_state` for every read and the existing private `Client::request`
for mutation. The read allowlist is unchanged; mutation never uses
`read_request`. There is no new public raw-write entrypoint or qualification
escape hatch. Every pre-existing method, helper and test remains unchanged.

Core apply reads fresh before-state, checks the qualified wire/target/effect
record and exact before value, and returns a no-op without consuming approval or
writing when before equals after. For a non-no-op it consumes the real approval
once and checks its exact action digest before dispatching one write. It then
reads authoritative after-state even if the write fails. Returned transport
status alone does not establish persistence. Results retain `parentSends: 0`
and `automaticRetryAllowed: false`; uncertain writes are never automatically
retried or rolled back.

The six original bindings are:

| Field | Method | Servlet | Selector | Value parameter | Effect record |
| --- | --- | --- | --- | --- | --- |
| progress | GET | DayRecordServlet | udtPrg | prg_txt | current |
| homework | GET | DayRecordServlet | udtHw | hw_txt | current |
| lessonMemo | GET | DayRecordServlet | udtMemo | memo_txt | current |
| attendance | GET | DayRecordServlet | udtAttn | attn_yn | current |
| dailyTestAssessment | POST | CourseCommonServlet | SetDailyTest | daily_test_no | current |
| previousHomeworkAssessment | POST | CourseCommonServlet | SetHomeWorkRate | homework_rate_no | independently selected previous |

GET query order is `reqCmd`, the value parameter, `course_seq`, `stu_pri_no`,
`record_seq`, `cm_seq`, `dummy`. POST form order is `reqCmd`, `cm_seq`,
`record_seq`, `tutor_pri_no`, the value parameter. The POST servlet is
`/servlet/controller.cct.common.CourseCommonServlet`; the GET servlet is
`/servlet/controller.cct.tutor.DayRecordServlet`. `effectRecordKey` is used
verbatim after core qualification, retaining the distinct previous-homework
occurrence. `dummy` is the injected runtime timestamp.

The existing request boundary carries ordered query/body pairs, cookies,
20-second request timeout, Origin/Referer, form content type, redirect refusal
and HTTP failure behavior. JS string values become URLSearchParams-compatible
USVString through the existing source string conversion, including replacement
of lone surrogates. Actual pair serialization belongs to the injected transport;
unit assertions of literal encoded strings do not prove genuine HTTPS bytes.
No response-body echo is interpreted as successful persistence.

Source errors during the before-read retain their original name, message and
properties, including typed `AcademyHttpFailure` status/operation properties.
The private bridge retains that error while satisfying the narrower core Fault
interface. Core validation/approval faults retain name, message and optional
`issues` on conversion back to source Error. After-read failures retain the
existing core `unresolved` result, and write failures retain `uncertain`.

## Authored verification and remaining proof

No compilation or test execution occurred in this source leaf. Four new
integration tests in `crates/edu-academy-sources/tests/owner_http_apply.rs` exercise
the actual public entrypoint and real before-read parsing/refusal:

- `public_validation_principal_and_authentication_refuse_before_transport`
- `public_before_read_preserves_typed_http_error_properties`
- `public_before_read_preserves_cookie_and_authentication_refusals`
- `public_synthetic_wire_cannot_authorize_any_of_the_six_fields_or_noop`

Five new private unit tests in `http.rs::owner_http_write_tests` exercise the
real private adapter and existing request method over fictional transport:

- `six_private_field_bindings_and_literal_form_encoding`
- `private_value_uses_usvstring_and_keeps_reserved_data_in_one_parameter`
- `private_request_failures_dispatch_once_and_never_follow_redirects`
- `private_request_reuses_cookie_scope_and_close_clears_it`
- `core_fault_conversion_preserves_name_message_and_issues`

These are two distinct test scopes. Private wire tests deliberately invoke only
the private write seam; they do not prove approval consumption or successful
public qualification. The public synthetic HTML/XML has fictional identities
and inert handler bodies. It must fail the immutable
`650813891d9428cb03eedb071f5c94bc369fd9641993de6d02ce4e07a5945bd1` gate
before approval or mutation, including for a requested no-op. No test invents a
qualified wire preimage, changes an oracle or stubs a qualified success.

The full public join through qualified before-state, approval, one write and
qualified authoritative after-state remains **PENDING**. The allowed public
source has no original handler bodies needed for that qualification. Root's
aggregate-only selected saved-input metadata also does not establish a raw HTTP
fixture. Private originals were not opened by this leaf. Root's separate source
integration/build/test readback must distinguish these gaps from private wire
unit results. Genuine HTTPS, real authentication, owner read scope, native
approval/recovery, original/candidate comparison, full S09/S11 acceptance and
service activation remain separate evidence boundaries.
