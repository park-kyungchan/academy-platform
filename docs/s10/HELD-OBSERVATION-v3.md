# Held observation review parity

Successor to candidate `3321a3a1f8e84a4c5fe817497dcd124263dee172`.
Independent Wonder observation `u_held_unqualified_stale` showed that the original
service schema-parses an existing held observation without running fresh intake
qualification again. A stale held observation returns a held review and digest
without a source callback, including when its embedded target differs from its
own scope. The previous Rust candidate instead returned a hard qualification
error. This successor removes only that extra qualification from held review.

Held observations still pass the observation schema. Service handles still bind
to the exact stored scope and request. Fresh authoritative reads still undergo
schema and source qualification; approval, write, uncertainty and replay checks
are unchanged. This supersedes the v2 statement that supplied held observations
retain additional target/value qualification. It creates no source admission or
live effect authority. Existing oracles and candidates remain immutable.

The independent original-backed unseen input and output remain in
`preparation/implementation/reviews/wonder/S10-fixes-3321a3/` of the task workspace.
Root owns a new private build target and records actual commands, channels,
hashes and process cleanup in `preparation/slices/S10/held-legacy-v3/native/`.
Acceptance requires independent same-finding readback of the successor binary,
unchanged 34-case correction oracle and preserved fresh-read rejection fences.
