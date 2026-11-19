# Known skipped tests

`bun test tests` skips these tests by name with `test.skip`. Each reads `<repository>/.private/academy-teacher-fixture` through `tests/teacher-fixture.ts`. Only the integrated setup made that directory, and commit `28ac72e` retired that setup; the repository's `AGENTS.md` forbids restoring it. A test returns when it has a fixture that the repository owns.

| File | Test |
| --- | --- |
| `tests/teacher-response-contract.test.ts` | F1 actual SDK/wire invalid inputs stay fixed-size before either tool handler, without caller-key echo |
| `tests/teacher-response-contract.test.ts` | F2 canonical sibling problems and separate branches render only their actual parent paths |
| `tests/teacher-response-contract.test.ts` | F3 canonical print/distribution observations remain independent and issuance-bound |
| `tests/teacher-response-contract.test.ts` | F4 actual canonical report keeps teacher-reported, recorded, stage-observed and query-retrieved clocks separate across academy date boundary |
| `tests/teacher-response-contract.test.ts` | actual SDK/wire retains Unicode pages, canonical refusals/drift and source guards without stdio child startup |
| `tests/teacher-response-contract.test.ts` | near-cap accepted CallToolResult stays within512KiB while its separately measured JSONRPC frame may exceed it |
| `tests/teacher-report.test.ts` | qualified loader requires explicit exact source root; no default/auth discovery |
| `tests/teacher-report.test.ts` | canonical projections render original100, explicit similar assignment and unknowns without second reducer |
| `tests/teacher-report.test.ts` | scoped final survives old low score; cancellation/replacement and distinct attempts remain separate |
| `tests/teacher-report.test.ts` | class occurrence scope preserves raw fields and multiple occurrences; no inferred attendance or full roster |
| `tests/teacher-report.test.ts` | full report paging is bounded and cursor binds snapshot, selector, format and caller |
| `tests/teacher-report.test.ts` | a single oversized report line is refused without skipping; later report caller change invalidates continuation |
| `tests/teacher-tools-mcp.test.ts` | real SDK production discovery/schema and canonical Unicode pages reconstruct without loss |
| `tests/teacher-tools-mcp.test.ts` | real SDK Korean report pages are individually bounded and include explicit continuation to final section |
| `tests/teacher-tools-mcp.test.ts` | real SDK reports refuse malformed, snapshot/caller drift, auth refusal and transport errors without echoing source errors |

## Conditional skips

These skip by condition, not by name, and are unchanged: `tests/release-base-diagnostics.test.ts:36` (when not uid 0), `:82` and `:118` (when a process cannot run as `nobody` through `runuser`), and `tests/release-graph.test.ts:77` (without `node`).
