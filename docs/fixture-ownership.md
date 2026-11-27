# Current Rust fixture ownership (E02)

The maintained checks use crate-owned fixtures and the offline participant listed
below. Each copy has the exact original bytes; no expected result was regenerated
from Rust. The parent guard uses immutable `parent-guard-vectors-v2.json`, preserving
all 529 original decisions and five historical source hash records. Historical
TypeScript paths are provenance only, not runtime oracle inputs. The original
`parent-guard-vectors.json` remains unchanged.

## Current inputs

| Historical path | Current crate-owned path | SHA-256 |
| --- | --- | --- |
| `migration/mcp-runtime-reference-v1/frozen/inventory/registry.json` | `crates/edu-academy-mcp/tests/fixtures/mcp-runtime-reference-v1/frozen/inventory/registry.json` | `2b30fcdd18fce4e82e79e74406a58e580d594381a94164fc1e3631d2f4e2a5d5` |
| `migration/mcp-runtime-reference-v1/frozen/protocol/commands/neutral/stdin.bin` | `crates/edu-academy-mcp/tests/fixtures/mcp-runtime-reference-v1/frozen/protocol/commands/neutral/stdin.bin` | `31969c2844a5b02118f6f60fdd9984202bcb587f2705629ecb2ec3817e8299d7` |
| `migration/mcp-runtime-reference-v1/frozen/protocol/commands/neutral/stdout.bin` | `crates/edu-academy-mcp/tests/fixtures/mcp-runtime-reference-v1/frozen/protocol/commands/neutral/stdout.bin` | `8998846f023154baf767f5d88748e0f86cee17268b4da98f1ebe026bc69813ec` |
| `migration/source-adapter-reference-v1/corpus/login.html` | `crates/edu-academy-sources/tests/fixtures/source-adapter-reference-v1/corpus/login.html` | `3ff129a91380d0b9c11238ccf7d76c76347ead0672579c1ad9c944c947e3a5fa` |
| `migration/source-adapter-reference-v1/corpus/principal.html` | `crates/edu-academy-sources/tests/fixtures/source-adapter-reference-v1/corpus/principal.html` | `b43a4c7d9c70bcdacdfcf84e482b630fdaadb5fbde7d4c0c157dc15616bde943` |
| `migration/source-adapter-review-fixes-v2/ASSESSMENT-EXPECTED.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/ASSESSMENT-EXPECTED.json` | `8a841fc71e497fe685adea60ff20d1c223b2c2873401a9a7d9fc5d135677b866` |
| `migration/source-adapter-review-fixes-v2/AST-EXPECTED.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/AST-EXPECTED.json` | `39a113de7563452838c0f025ffa78babcea5fedcfdfadcaa47b9f3a4d679572f` |
| `migration/source-adapter-review-fixes-v2/AST-STIMULI.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/AST-STIMULI.json` | `f342b56bd9007d562f243186346456f0c0b67a5bd9e0580ae614e8c594715e26` |
| `migration/source-adapter-review-fixes-v2/BUDGET-EXPECTED.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/BUDGET-EXPECTED.json` | `8127f4bc7a188f4cde0d14c5fbaa9935e21738815c7c4b542f8cee7b3155b3ca` |
| `migration/source-adapter-review-fixes-v2/BUDGET-STIMULI.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/BUDGET-STIMULI.json` | `d7b65cf1dadc70ddecaebc4c191a25eba282d2be9f8b41f76723242fc6f68dd4` |
| `migration/source-adapter-review-fixes-v2/CONTAINERS-EXPECTED.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/CONTAINERS-EXPECTED.json` | `ac96bb2e10c5d47ce7de345ccc2a9193e7df1d0ee00db53810d0665c70d2f092` |
| `migration/source-adapter-review-fixes-v2/CONTAINERS-STIMULI.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/CONTAINERS-STIMULI.json` | `1043a050180cb0a3aec5cfba213559055a90786ef4f48c8de30350b8857606ae` |
| `migration/source-adapter-review-fixes-v2/ORIGINAL-EXPECTED.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/ORIGINAL-EXPECTED.json` | `9d63fe58653eb3e01e86108919391b6f4cac0895ffc2280954e81ba24c6dcd2c` |
| `migration/source-adapter-review-fixes-v2/REGEXP-EXPECTED.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/REGEXP-EXPECTED.json` | `11bf58e280d4006d6e3d123752d540829090f1ddc8bb7d03a86e14a41842a0ba` |
| `migration/source-adapter-review-fixes-v2/REGEXP-STIMULI.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/REGEXP-STIMULI.json` | `40c3c965dc64841786de8d9037505342757905f4508ef112454d6ffe6ab4549b` |
| `migration/source-adapter-review-fixes-v2/STIMULI.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/STIMULI.json` | `2781dbb26bd18f7279d2a22bebd11bf0af88bf342222502098a6cf33d558015e` |
| `migration/source-adapter-review-fixes-v2/UNICODE-EXPECTED.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/UNICODE-EXPECTED.json` | `7dce885fe8c2a73c47a506460b6d08e5cef07765bbdd75aeb39709e1ed61047f` |
| `migration/source-adapter-review-fixes-v2/UNICODE-STIMULI.json` | `crates/edu-academy-sources/tests/fixtures/source-adapter-review-fixes-v2/UNICODE-STIMULI.json` | `abe75c20dbce407cbdaf4f75a667ede59500f2c955f1e6b9c20b1d9e23d8d5d2` |
| `migration/workflow-store-review-fixes-v2/CASES.json` | `crates/edu-academy-core/tests/fixtures/workflow-store-review-fixes-v2/CASES.json` | `bc6fb875677bfaeba6aec7fc212242b144c3c54de111a724dc77dde0de59eebf` |
| `migration/workflow-store-review-fixes-v2/EXPECTED.json` | `crates/edu-academy-core/tests/fixtures/workflow-store-review-fixes-v2/EXPECTED.json` | `9692f743495050c7fd1190f3c4155b665fd6f074f3734db4d2c71fdc18a40381` |
| `migration/workflow-store-rust-v1/participant.rs` | `crates/edu-academy-core/tests/fixtures/workflow-store-rust-v1/participant.rs` | `2714354c58cfff60bc877b6df0d9e7ee5698c899b7ca2e564ad53c295623f3be` |

## Historical reproduction and recovery

The retained `migration/` Python/Bun observers, comparators, freeze tools, pins,
contracts and recorded results describe completed migration allocations. They are
historical reproduction tools, not current Cargo, package-script, CI or installed
Academy entrypoints. They must be used with their complete original revision in
an isolated recovery checkout, not run against a partially retired source tree.
Their original bytes and assertions remain preserved. This notice supersedes
instructions elsewhere to run those historical tools against the current tree;
it does not disable any maintained Rust test or authorize a new original freeze.

The separate E-DEL-FIXTURE-001..005 deletion PR may remove the 21 historical paths
after checking their consumers and verifying archive recovery. Recovery uses
`archive/education-platform-pre-e-del-fixture-20261105` at that PR's base;
verify the remote tag before deleting. Use `git show TAG:PATH` for original bytes,
or `git worktree add --detach RECOVERY TAG` for the complete reproduction graph.
Do not redirect original comparators to candidate outputs or edit frozen pins.

Current verification is the pinned Cargo workflow in the root README. Fixture
ownership does not qualify native activation, live Academy effects or full parity.

## Invented day-record capture page

`crates/edu-academy-sources/tests/fixtures/dailyrecord/day.html` is authored from
scratch. All identities, dates and record contents are invented; its one row uses
the invented values of the read-schema contract tests. Its control layout follows
the supplied structural description: two header rows without `thead`, nested
homework/report cells, readonly previous texts, and hidden assessments in their
respective columns. Inert handler declarations use the crate's existing signatures.
`dailyrecord_tests.rs` derives adversarial pages by changing only these invented
bytes, and constructs disposable owner documents, raw indexes and spool files.
No private page, credential, student record or endpoint supplies fixture values.

## Invented lesson-command page

`crates/edu/tests/fixtures/day.html` is authored from scratch in the layout of the page above:
two rows, the first of a student whose invented name has Korean letters, the second a student's
first lesson (no previous-homework handler or control, empty previous cells), as the family
stores it. Every text carries an invented marker, so `crates/edu/tests/lesson.rs` can show that
no text reaches line 1, a refusal or the run log. Its secret file, identity, roster, cookie and
password are invented in the test.
