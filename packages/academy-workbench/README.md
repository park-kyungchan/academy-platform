# Synthetic academy workbench

Isolated TypeScript/Bun prototype for date-first class overview, student detail, book/unit/section progress and a separate date-based history. It uses fabricated students and evidence only. Existing repository applications, source collectors and production stores are unchanged.

## Continue on another PC

Install **Bun 1.4.2** from the official Bun distribution, clone this repository using your own authorized GitHub access, and run from this directory:

```sh
cd packages/academy-workbench
bun install --frozen-lockfile --ignore-scripts --registry=https://registry.npmjs.org
bun run check
bun run preview
```

Open http://127.0.0.1:4173. `check` runs local TypeScript 7.0.2, the synthetic tests, build, fixture validation and demo. It writes an ignored local command report to `docs/book-flow-validation-results.json`. `preview` serves only the static build on loopback. Teacher decisions are in memory and reset on reload. No credentials or real student data are needed. `node_modules`, Bun binaries and caches are deliberately not carried in Git; pinned dependencies and `bun.lock` reproduce the installation. `package-lock.json` preserves the original npm dependency record; Bun is the active workflow.

If an existing package cache produces `Executable not found` for the official TypeScript platform package, restore the same locked packages from the official registry with `bun install --frozen-lockfile --ignore-scripts --force --registry=https://registry.npmjs.org --cache-dir .bun-cache`, then rerun `bun run check`. Do not copy compiler binaries between PCs or disable integrity checks.

## Contracts and evidence

`src/contracts/book-flow.ts` re-exports the immutable r2 TypeScript contract in `imports/academy-contract-r2/`. The schema version is 1.0.0, revision 2. Four payloads retain their original bytes and verified hashes in `docs/r2-import-manifest.json`. The delivery README is retained only in the original local pack because it contains a machine-specific path.

Stable domain IDs and versioned typed records are canonical. Space is a prospective review/feedback projection, not a transactional database. The existing `StudentBookMap` and assignment timeline remain explicitly synthetic UI projections; importing r2 does not claim that the full `BookFlowStore`, append-only `HistoryEvent` store, ActionAttempt persistence or WebMCP registration is implemented.

`deriveAttention` independently preserves applicability, assignment, requiredness, completion, effort, mapping, coverage, deadline and evidence presence. Confirmed incomplete includes work already started; confirmed never-started is a narrower fact. Empty queries, overdue deadlines and old scores never imply nonperformance. Source observation time is not actual event time. Completion, query coverage and known denominators remain separate; incomplete structure suppresses whole-book percentages.

The permission-checked application service is shared by UI and agent ports. Teacher effects require explicit rationale/evidence and revision checks. No LMS writes, external deployment, official record finalization or parent sending are included. The repository's `src/academy/classroomCyclePolicy.ts` remains the classroom authority: synthetic path/grade observations are not teacher classroom acceptance. See `docs/r2-review.json` for exact changes and verification limits.

## Focused checks

```sh
bun test tests/r2-contract.test.ts
bun run typecheck
bun run build
```

The ten supplied semantic cases are executed separately from the original 55-test foundation. Additional tests check imported bytes, the supplied schema, negative schema cases, simultaneous UI badges, event-time preservation, structure completeness and repository policy. The offline schema checker supports only the keywords used by this pinned schema and rejects unsupported keywords/references; it is not a general JSON Schema validator.

GitHub Actions runs this package on Linux and Windows with frozen installs. Existing repository CI is retained. Live academy access, production persistence, actual WebMCP and mobile-device acceptance remain unverified.
