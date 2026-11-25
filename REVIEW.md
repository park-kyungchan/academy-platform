# Review policy of academy-platform

For the one reviewer session of a pull request (and for the automatic reviewer). Review the head whose required check `rust` passed.

## A finding blocks only when it is one of these, shown by a command, a test or two quoted sentences

1. Wrong behaviour of the route: a save is dispatched twice for one field, or on a field whose attempt is unresolved; a fence is lost or never released; the ledger and the bodies disagree after a crash or a refusal; an answer contradicts the contract that states it.
2. Loss or exposure of data: a body or a ledger row is lost or damaged; student text is kept where the contract says none is kept; a credential is in the tree, a commit or the pull request, except a value under `imported/` that a finding of the Lead records as a test value or as rotated by its owner; a real name or a student record is in the tree, a commit or the pull request anywhere but under `imported/`, where the user's decision act-rq-e679a963 admits the Academy's records as they are.
3. A broken contract: a statement of a document under `docs/` (or of `ontology/academy.schema.json`) that the code contradicts; the engine pin in `Cargo.toml` moves without the ledger tests of `edu-academy-ontology` passing on the new revision and the pull request saying which answer shapes changed.
4. An irreversible effect on a real target without a guard: a call to the Academy LMS, a change of the installed server or its registration, a write under an owner root.

## Not blocking (at most five notes)

A test gap, a surviving mutant, a sentence in a document, a name, a preference for another structure, hardening against a same-user adversary. Do not write mutants by hand. A change that deletes the only tests of code that stays says so in the pull request; when it does not, that is a finding of kind 3.

## Form

First line exactly `Verdict: accept` or `Verdict: repair`; second line the head revision; then the blocking findings (each with file, line and the command, test or quotation that shows it), then the notes.
