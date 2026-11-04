# Finite future leaf handoff — WAIT for independent freeze and Lead dispatch

Goal: implement only UTF-16 unit width, CISM week and CISM month selection against the independently source-frozen oracle. No judgment-heavy digest/template/review or runtime code.

Checkout `/home/palantirkc/work/ws-replan/wt/education-platform`, common Git `/home/palantirkc/projects/education-platform/.git`, branch `rust/s08-pure-semantics-20261102`. Lead must supply the exact independently accepted foundation commit before dispatch. Base prerequisite is accepted S05 `049f7c3da0af6c320c818d2afdab3da34bccd37a`, original source `cc6e81f1be2fcb56f011041e3d606e21edf76da3`. Frozen contract: `migration/pure-semantics/{DESIGN.md,freeze.json,expected-v2.json,corpus/cases.json}`. Current author result is not independent review acceptance.

Allowed write: **only `crates/edu-academy-core/src/leaf.rs`**. Public interfaces and CLI remain Lead-owned. All original source/tests/config and migration oracle (including invalid v1 evidence) are read-only. No manifest/lock/dependency/toolchain, .gitignore, STATE/ORCHESTRATION, expected/corpus/driver/comparator edits. No children, source requests, real accounts, locks/journals, source saves, print/send, global configuration, push/merge/deploy. Return an ambiguity to Lead; do not broaden behavior or copy expected outputs into implementation.

Exact algorithms, return types, enums, refusal messages, year boundaries, malformed Unicode representation and edge examples are in DESIGN.md. Follow its day-ordinal algorithm; standard integer operations and month tables suffice. The leaf protocol supports all u16 code units, so no surrogate rejection/substitution is needed. Width never enforces400. Calendar input years2000..2099 can return derived dates in1999/2100. Month first-day Sunday is not duplicated. No date/JSON/hash crate is needed in the leaf.

Before any effect, read canonical `/root/.agents/AGENTS.md`, current `/home/palantirkc/work/ws-replan/{STATE,ORCHESTRATION,CURRENT-PLAN}.md`, accepted foundation and frozen files. Current user/STATE/ZIP outrank dated Hindsight/retired harness rules. Hindsight uses `/docker/hindsight/HINDSIGHT_OFFICIAL_PER_CALL.md`, installed coding-agents0.8.2/Hindsight1.1.0 revisiona5a619b5ca59c5c39e0f96be6f7a31fabd136bd5 official native features. No child delegation or model substitution.

Allowed static command before a build grant:

```sh
cargo fmt --all -- --check
cargo metadata --offline --locked --no-deps --format-version 1
git diff --check
```

Only after the Lead expressly grants its sole build slot, the prescribed future commands are:

```sh
cargo build --offline --locked -j1 -p edu-academy-core
node migration/pure-semantics/compare-candidate.mjs --candidate /home/palantirkc/work/ws-replan/wt/education-platform/target/debug/edu-academy-core --results-root /tmp/s08-candidate-NEW
```

The second command runs only54 leaf cases and preserves actual child statuses/bytes. It never regenerates expectations. Lead will run/review broader evidence and decide later test/clippy/release commands; none is implicitly authorized by this handoff. First compiled success is future evidence, absent at foundation.

Return a concise terminal result with exact commit/diff, actual commands/checks and raw result references, unresolved gaps and running children/builds. No file-output request. Lead preserves evidence/closes the bounded task pane and starts the next task in a fresh session. Do not start S09/S10. Any inseparable judgment-heavy issue returns to Lead without candidate implementation.
