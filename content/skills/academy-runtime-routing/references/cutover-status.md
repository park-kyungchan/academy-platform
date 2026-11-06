# Academy cutover status

## Current state — 2026-11-06

User decision 2026-11-06: the last TS release is restored host-native and
`academy_operations` is ENABLED in **Muse only** (generation
`host-restore-20261106-v1`, base `/root/.local/share/academy-mcp-host-20261106`,
record `/home/palantirkc/work/edu-mcp-host-restore/STATE.md`). Live reads go
through those registered Muse tools only. Save remains unconfigured. Claude Code,
Codex and Antigravity CLI stay DISABLED until the full-surface Rust cutover.
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
