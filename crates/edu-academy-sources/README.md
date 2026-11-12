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
`academy-live-driver` reads one JSON object from stdin. It additionally requires
absolute `ownerRoot` and `store` paths and `admission`, `credential`, and
`launchAdmission` pins, each `{relativePath, sha256}`. The owner root validates
private files; the store must be outside Git and have mode 0700. Invalid input,
unsupported fields, and unsafe stores fail before login. Success exits 0;
failure exits 2 before a service call or 3 after one, with one JSON object and
typed codes only on stderr. Its real clock is SystemTime; UUIDs come from Linux.
