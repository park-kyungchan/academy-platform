# Academy workbench reference

This directory retains Academy source, context and the TypeScript MCP reference while the education-platform Rust replacement is qualified. The installed Academy bundle and its configuration are separate, unchanged runtime owners. No native cutover or Academy calls are authorized by these files.

The former standalone UI, integration harness, CI snapshots and engineering evidence have been retired after the Cargo CI cutover. Original bytes remain recoverable from `archive/education-platform-pre-delw1-B-20261105` and Git history.

Retained context and viewer sources still use a small dependency closure under `architecture/`, `scripts/`, `harness/`, `migration/` and `app/standalone/`. The protected MCP analysis utility also reads `preview/v21/app.js`; this is a source-analysis artifact. Keep these dependencies until their readers are retired. Their presence does not restore the retired harness as an active platform runtime.

[Academy MCP reference](tools/academy-mcp/README.md) keeps its own source, pins and tests. Workspace Rust verification is described in the [platform README](../../README.md). Source checks, independent review, teacher acceptance and native activation remain separate.

The month-end `apps/academy/month-end-workbench/docs/source-context.md` locator in retained MCP documents `assessment-research-binding.md:3`, `candidate-explanation-framework.md:5` and `pool-readonly-workflow.md:40` is `unavailable_at_head`. Resolve it only as an archived locator at tag `archive/education-platform-pre-delw1-C-20261105`, commit `a81006644762446839970ff6d6e0069efb17f1d4`: [archived source context](https://github.com/park-kyungchan/education-platform/blob/archive/education-platform-pre-delw1-C-20261105/apps/academy/month-end-workbench/docs/source-context.md). This notice supersedes their description of that path as a current checkout entrypoint; the protected MCP documents retain their original bytes. Consumption remains `locator_only_requalify_before_use`: qualify the exact archived source and current task before use. An archive locator grants no operational authority.

Preserve dated source identities, original observations, user decisions, unknowns and privacy boundaries. Private context must not become synthetic fixtures or public preview assets. Credentials, sessions, databases and operational effects retain their existing owners.
