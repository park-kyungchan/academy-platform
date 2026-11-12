# Opt-in one-field driver

`academy-live-driver` reads one JSON object on stdin and emits one JSON object.
Supported fields: `attendance`, `dailyTestAssessment`, `previousHomeworkAssessment`.
Inputs: `mode`, `scope`, `requestRef`, `field`, absolute `ownerRoot` and `store`,
and three `{relativePath, sha256}` pins: `admission`, `credential`, `launchAdmission`.
`plan` and `save` also require `value`; `save` requires the preceding `planDigest`.
The digest binds scope, field, target, record key and before/after hashes.
A held plan keeps its real blocker; only a prepared digest mismatch reports
`approval_digest_mismatch`. No automatic write retry occurs.
`status` returns stored execution metadata and nullable `reconcileStatus`/`reconcileCode`.
With `reconcile: true`, not-yet-due `save_confirmation_pending` waits exit 0;
a failed settling read exits nonzero. Every mode still logs in once.
`save` failures retain HTTP status/body hash/six-field comparison when collected.
Values and identities are hashes only; stderr contains typed codes only.
Success exits 0; failure exits 2 before any service call, 3 afterwards.
The whole existing store tree is checked before login: no symlinks/special files,
no directory with group/other mode bits; a new store is created with mode 0700.
The launch object is unversioned; extra keys are ignored as the core ignores them.
Launch shape and identity, transport student/date/namespace scope are checked
before login. The transport `operations` list is parsed and validated and
authorizes nothing here; the product requests are fixed.
The fourth owner-root file is the generated account lock, never deleted. It
excludes only programs using the same canonical owner root, not the server in use.
Runtime uses SystemTime and Linux kernel UUIDs; keep the wall clock stable.
