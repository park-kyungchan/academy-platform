# Stable Academy stdio launcher

Register the same owner-controlled launcher in host CLI configurations. It reads the
current release pointer on every new process, verifies the selected bridge before
starting it, and leaves the existing bridge's routing and endpoint checks intact.
Existing processes keep their already-loaded bridge until they reconnect.

For a legacy `academy.mcp-release/1` generation, stage a regular private
`bridge-pin.json` beside `server.mjs` before activating the pointer. Its exact
JSON keys are `path` (the canonical absolute `server.mjs` path) and `sha256`
(the lowercase SHA-256 of its bytes). Create it as part of the new immutable
generation; do not alter an existing retained release. The pointer is activated
only after all release artifacts and this pin are verified. A missing, changed,
or mismatched pin fails closed. A release/2 pointer uses its own bridge pin.
This pin does not upgrade a legacy endpoint graph to release/2 qualification.

The launcher does not own credentials, change admissions, or create an account
lock. Its execution host must already be able to run the owner bridge and its
existing endpoint transports. Container paths and identities are separate; a
host path cannot be treated as a container path without a supported transport.
