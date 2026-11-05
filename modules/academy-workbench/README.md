# Academy workbench reference

This directory retains Academy source, context and the TypeScript MCP reference while the education-platform Rust replacement is qualified. The installed Academy bundle and its configuration are separate, unchanged runtime owners. No native cutover or Academy calls are authorized by these files.

The former standalone UI, integration harness, CI snapshots and engineering evidence have been retired after the Cargo CI cutover. Original bytes remain recoverable from `archive/education-platform-pre-delw1-B-20261105` and Git history.

Retained context and viewer sources still use a small dependency closure under `architecture/`, `scripts/`, `harness/`, `migration/` and `app/standalone/`. The protected MCP analysis utility also reads `preview/v21/app.js`; this is a source-analysis artifact. Keep these dependencies until their readers are retired. Their presence does not restore the retired harness as an active platform runtime.

[Academy MCP reference](tools/academy-mcp/README.md) keeps its own source, pins and tests. Workspace Rust verification is described in the [platform README](../../README.md). Source checks, independent review, teacher acceptance and native activation remain separate.

Preserve dated source identities, original observations, user decisions, unknowns and privacy boundaries. Private context must not become synthetic fixtures or public preview assets. Credentials, sessions, databases and operational effects retain their existing owners.
