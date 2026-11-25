# Working in education-platform

Shared rules of this host: `/root/.agents/AGENTS.md`. This page adds only what is particular to this repository. What the repository is: `README.md`. For Academy classroom practice use the [Hindsight knowledge entry](docs/academy-practical-entry.md); it holds locators only.

## What this is and who uses it

Tooling for a teacher's Academy work. The route in use is the installed TypeScript Academy server (source `modules/academy-workbench/tools/academy-mcp`, registered as `academy_operations`). The Rust Cargo workspace is the replacement under construction: its crates are not installed and are not qualified for native activation. Users: the teacher, through the TypeScript server, and every agent that changes this repository.

## The check

`./check` at the root, until it prints `check: ok`; it is the file the CI job `rust` runs. It covers the default members. `./check all` also builds and tests the frozen crate. On the host it takes the build lock and makes its fixture directory by itself (do not wrap it in `flock`; do not point `CHECK_FIXTURE_BASE` under `/tmp` or inside a work tree). Builds go to `target/` in your worktree. Do not push before it passes.

## Contracts you can break

| Contract | Where | Relied on by |
| --- | --- | --- |
| Record store, ledger, bounds and refusal codes | `docs/academy-record-cutover.md` | `edu-academy-core` workflow, `edu-academy-teacher` draft store, the live driver |
| The `academy` schema | `ontology/academy.schema.json`, embedded by `edu-academy-ontology` | Every store file under an owner root |
| Engine pin (`ont*` `rev` in `Cargo.toml`) | `ontology-platform` | The ledger tests of `edu-academy-ontology` |
| Account lock: every Rust path that authenticates to the LMS holds the shared account lock | `docs/academy-account-lock.md` | The installed TypeScript server and the lesson sessions |
| One-field live driver input and output | `crates/edu-academy-sources/README.md` | Callers of `academy-live-driver` |
| The `edu-academy-memory` subcommands, their output and refusal codes | `crates/edu-academy-memory/README.md` | The lesson-day session and its memory skill |
| Crate-owned fixtures and their hashes | `docs/fixture-ownership.md` | Tests of every crate |

A change of one of these says so in the pull request and comes with the test that fails on the old behaviour.

## Frozen

`crates/edu-academy-mcp` (not a default member): parity work stopped; retired at the first cutover. Retained TypeScript/Bun reference source (`modules/`, `apps/`, `migration/`) and every file with a current Rust, live runtime or host consumer stay until their retirement conditions are met. Do not restore the retired Bun integration harness, context gates, custom receipts or automated review workflow. Historical pins and records describe their original observations, not current acceptance.

## Imported

`imported/` holds two former repositories' trees and histories ([imported/README.md](imported/README.md)). Do not change, format or build a file there: the Cargo workspace excludes it, `./check` and the CI job do not read it, and no test reads it as a fixture. The Academy's entry points use `apps/academy/dc-gang-a/`, not `imported/dc-gang-a/`; the Rust migration decides what of `imported/` survives. It holds the Academy's records as they are: never copy a file from there into a fixture or another path.

## Privacy and effects

Preserve private context and original dated student, book and activity identities. Never read or publish credentials, use private student payloads as synthetic fixtures, contact Academy endpoints or infer operational effects from a repository check. Do not change the installed Academy bundle or host configuration as a side effect of source maintenance. Native profiles, sessions, databases, stores and credential consumers keep their owners. DailyRecord work follows [the policy](apps/academy/dc-gang-a/docs/daily-record-authoring-policy.md); drafts, review, save and transmission stay separate.

## Pull request

One pull request per unit of work, able to merge alone; its description is your plan. Never force-push. The required check is `rust`. Review policy: `REVIEW.md`. Current user instructions and the task's `STATE.md` govern effects.
