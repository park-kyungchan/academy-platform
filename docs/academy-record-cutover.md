# Academy record store cutover

The body and draft directory is `<root>`, and attempts live in
`<root>.ledger/academy.sqlite`. Both directories stay under the owner root.
Directories have mode 0700 and the store file has mode 0600.

One root is served by one form. The live driver opens the engine form.
`open_store` refuses file-form attempts with `workflow_ledger_legacy_root`.
`Store::open` refuses an engine sibling or engine bodies with that same code.
`DraftStore::open` refuses an engine sibling with `record_store_not_synthetic`.
`DraftStore::open_with_core` requires a ledgered core for the same directory,
compared by device and inode; otherwise it returns `record_store_not_synthetic`.
A process opens the core with `open_store` first and passes it to
`DraftStore::open_with_core`, including for a directory that does not yet exist.
Without an owner marker, this accepts only the private, empty directories
initialized by the supplied core; unrelated entries or nonempty directories
are refused. Core identity is checked before writing the owner marker.
It never holds `DraftStore::open` on a root that
may later be given to `open_store`: the sibling check happens only at open.
The synthetic backend stays on the file form. The example
`fictional_dailyrecord_endpoint` also stays on the file form with invented data.

Erasure removes the whole store, its bodies and its sibling ledger together,
and accounts for backups. Deleting one student cannot erase its identifier:
it remains in observations, the action log, `edits.jsonl` and backups.

For execution, outcome and reconciliation on the engine form, the complete
serialized body wrapper (kind, id and record) is bounded at 65,536 bytes.
`Store::publish` refuses a larger wrapper with `workflow_attempt_body_too_large`
before writing a body or calling the ledger, leaving no new attempt open;
a wrapper exactly at the bound is accepted and readable.

When an effect target moved, reconciliation leaves that field uncertain with
`workflow_effect_target_drift`, keeps its fence, and reconciles the other fields.

The engine stores the original execution key, bounded at 8,192 bytes.
The body bound is checked first by `Store::publish`. A key over 8,192 bytes
whose serialized body still fits is refused with `workflow_execution_key_too_large`.
A key so long that its serialized body exceeds 65,536 bytes is refused with
`workflow_attempt_body_too_large`. Both refusals leave nothing written and no
attempt open, occur before any dispatch, and are final and not retried.
The engine holds no student body text, but the execution key carries
unsalted SHA-256 digests of before and after text. Anyone able to read the
mode-0600 owner-only store or a backup can recognize short or guessable text
by comparing its digest.

`open_store` installs the embedded schema when it differs from the installed
schema. Before a second version of `academy` reaches a store with attempts,
install it on a copy and measure the basis of one `begin_attempt` there.
That basis must not grow with the number of closed attempts.
The ledger grows by about 47.6 kB per attempt. Every schema install or backup
keeps a full copy under `backup/` of the sibling. While an attempt of a field
is open, a second save of that field is refused with `workflow_attempt_fenced`.
The store is not registered with an agent tool: the engine has one principal,
and the schema grants that principal all five actions.

A process opens its store once at startup and keeps that `Store` for its lifetime.
The first open creates and initializes the engine. A killed first open may leave
`.academy.sqlite.new-*` in the sibling directory. Never open these files:
a name can be a second hard link of the live store. Cleanup may only unlink
that name, and backups skip it.

Never move, copy or delete `<root>.ledger` apart from `<root>`. Deleting the
sibling makes `open_store` start an empty ledger over the existing bodies:
every attempt is forgotten. Pass roots without `..`; both forms refuse it.

Until the engine revision with item G3 is pinned, one `begin` costs about
0.2 to 0.33 ms per closed attempt (measured: 597 ms at 2,700 attempts).
The full-bound control `preflight_limit_on_engine` is run once after the engine revision with G3 is pinned.
The first production save waits for that pin.
Before a production store exists, rollback consists of reverting packet LC.
