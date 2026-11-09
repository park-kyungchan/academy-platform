# Academy cutover status

## Current client routing — 2026-11-07

User requirement 2026-11-07: custom MCPs are CLI-agnostic. `academy_operations` is directly registered in **Muse, Claude Code, Codex, Antigravity CLI and Grok Build**. This supersedes the earlier Muse/Claude-only client-registration gate, while preserving task permissions and release qualification. The current backend remains the TS host-restore route at `/root/.local/share/academy-mcp-host-20261106`; its `release.json` selects the actual forward generation. The owning record is `/home/palantirkc/work/edu-mcp-host-restore/STATE.md`. Live reads go
through those registered tools only, from one lesson session at a time (one shared
account). Save readiness must be determined from the current native result; the earlier unconfigured-save observation is historical, not a permanent hold. Rust activation still requires the full-surface cutover gate below; enabling a native client is not Rust parity.
No browser access, direct HTTP probe, shell client or substitute MCP client.
Do not modify the install base, `/opt/data` layout or release pointer; report
failures with their code. Rust work continues separately; its later cutover
replaces this route.

Superseded hold (2026-11-05): all four CLIs disabled; retained for history.

## Future route release

Require full parity for all 44 live tools (or an explicit user-approved tool
disposition), synthetic tests, independent review, green CI and verified rollback
before a separately authorized cutover. A registered tool, bounded handler or
successful build is not full parity. No tool drop or partial cutover is approved.
Report Rust parity from the complete gate, never from registration/handler counts.
After release, current task authority still determines reads, review, save,
parent transmission and other separate effects. Preserve original values,
existing account-lock ownership and native readback/reconciliation semantics;
do not recreate a shared runtime gate or probe a held target after failure.

## Historical TS deployment — navigation only

The retained common server was under
`/root/.local/share/academy-mcp-scoped-20261030/`. Its `launcher.mjs` verified a
selected immutable bridge pin and `release.json` selected endpoints. Fresh
connections could load new bridge code while running clients retained older
code/schemas; the directory name did not identify the active endpoint generation.
The retired Muse-specific `/root/.local/share/muse/academy-runtime-20261023/`
was not the common runtime authority. These are historical facts, not a live
inspection procedure or fallback. Keep the bundle, service unit/socket, release
state and pointing host configurations untouched until their authorized owner
performs cutover and retirement.
