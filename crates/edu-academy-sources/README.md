# Opt-in one-field driver

`live_driver::execute` provides library modes `plan`, `save`, and `status` through
an injected core `Service`. Supported fields are `attendance`,
`dailyTestAssessment`, and `previousHomeworkAssessment`; other fields return
`field_not_supported` before any source operation.

Each request supplies `mode`, `scope`, `field`, and `requestRef`. `plan` and `save`
also require `value`; `save` requires `approval`, the preceding `planDigest`.
`planDigest` binds scope, field, physical target, record key, and before/after
hashes. A fresh held review or changed digest refuses with
`approval_digest_mismatch`. A prepared review without a patch returns
`prepared_patch_required`. Nothing retries a write automatically.

`status` reads stored execution metadata. With `reconcile: true`, the core reads
the source only at or after the settlement due time. `EvidencePort` retains the
immediate HTTP receipt and six-field comparison without retaining field values.
`Owner::leased` shares one client and account lease across source/writer views.

The stdin binary, real `Runtime`, and executable input contract are unfinished
under the packet's product-line ceiling. This library is not a live-run command.
