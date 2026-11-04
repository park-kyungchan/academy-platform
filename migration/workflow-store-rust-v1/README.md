# S10 offline Rust comparison tools

Read `docs/s10/IMPLEMENTATION.md` and immutable original
`migration/workflow-store-reference-v1/CONTRACT.md` first. The candidate library
never reads those expected files. These tools separate actual Rust decisions from
fictional test ports, process observation and unchanged independent expectations.

Use a NEW private absolute evidence root beneath the assigned S10 native notes,
Rust1.95 and the sole compiler grant. Set `PYTHONDONTWRITEBYTECODE=1` and invoke
Python with `-B`; tools also disable bytecode before original imports. Do not run
original freeze modes or modify any reference. Existing verified Zod4.4.3 is used
only by unchanged TS reader/lock participants; no installation is needed.

Build with `cargo build -j1 --locked --offline --bins --target-dir OWN_TARGET`.
Use `run_check.py --notes OWN_NOTES --label UNIQUE --seconds BOUND -- COMMAND`
for raw stdout/stderr/status/duration/hash/process-group evidence. It creates a
process-local subreaper and never changes global runtime configuration.

Commands (substitute explicit absolute `BINARY`, `RESULT` and `MAIN_RESULT`):

```text
python3 -B verify.py --suite main --binary BINARY --root NEW_RESULT
python3 -B verify.py --suite supplement --binary BINARY --root NEW_RESULT
python3 -B old_read.py --binary BINARY --root NEW_RESULT --main-results MAIN_RESULT
python3 -B mixed_locks.py --binary BINARY --root NEW_RESULT
python3 -B qualification.py --root NEW_RESULT --main-results MAIN_RESULT
python3 -B s08_regression.py --binary S08_BINARY --root NEW_RESULT
```

`mixed_locks.py` actually waits45 seconds once on the frozen selected flock prefix,
with only a new disposable account pathname and vetted fake child substituted.
Both languages then exercise the same open objects, cancellation and holder death.
Do not substitute a real account lock or execute the native child tail.

`qualification.py` runs a positive control and16 actual process/file/effect mutants,
including extra files beyond snapshot evidence, target/effect tampering, journal
corruption/removal, false results, denied effects, nonzero/signal/hang/overflow.
These qualify the observer, not an independently reviewed source mutation score.
The5 Rust integration tests exercise publication/identity, index/field fencing,
lock lifetime, CISM custody guards and ordered UTF16/deep JSON.

All test faults, fixture identities and fake policies live here or in workflow
integration tests. `participant.rs` is a migration adapter, not a production
source/policy/auth implementation. Captured uncertain fake states and failed
channels are retained; no uncertain effect is retried as a fresh business action.
Root owns acceptance and later pane closure after native process readback.
