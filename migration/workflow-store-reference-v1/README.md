# Education workflow/store original-source reference

Original TS base: `39441f5d205b8a67e5c911da747d213a322d20da`.
Reference only; no Rust implementation, native business effects, live transport
or acceptance. Read [CONTRACT.md](CONTRACT.md), [CASES.json](CASES.json),
[SOURCE-PINS.json](SOURCE-PINS.json) and [OLD-STATES.json](OLD-STATES.json).

The normative source expectations are `EXPECTED-v2.json` (83 frames),
`SUPPLEMENT-EXPECTED.json` (12), `OLD-EXPECTED.json` (14) and
`ACCOUNT-EXPECTED.json`. `EXPECTED.json` preserves the initial unqualified v1
whose audit PIDs were not normalized. V2 is a documented nondeterminism fix;
no semantic expected values were changed to fit a candidate.

Run from the assigned worktree. Requires existing Bun1.4.3, Python3 and the
manifest/lock-matching Zod4.4.3 link recorded in SOURCE-PINS. No install command,
aggregate package test, Cargo, native helper, server, socket or Docker is needed.
Every evidence directory below must be NEW; substitute a fresh suffix for a
later review, always under the exact allocated S10 native note root.

```sh
python3 migration/workflow-store-reference-v1/verify_pins.py

python3 migration/workflow-store-reference-v1/run.py \
  --root /home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/native/reviewer-main-01

python3 migration/workflow-store-reference-v1/old_read.py \
  --root /home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/native/reviewer-old-01

python3 migration/workflow-store-reference-v1/supplement.py \
  --root /home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/native/reviewer-supplement-01

# Actually waits the source45-second account deadline, using a disposable file.
python3 migration/workflow-store-reference-v1/account_lock.py \
  --root /home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/native/reviewer-account-01

python3 migration/workflow-store-reference-v1/qualify.py \
  --root /home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/native/reviewer-mutants-01
```

For the strict outer process judge, `run.py --emit` emits one complete normalized
JSON transcript; its stdout is intentionally larger than the compact result.

```sh
python3 migration/workflow-store-reference-v1/judge.py \
  --command-json '["python3","/home/palantirkc/work/ws-replan/wt/education-platform-s10-reference/migration/workflow-store-reference-v1/run.py","--root","/home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/native/reviewer-main-02","--emit"]' \
  --evidence /home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/native/reviewer-judge-02
```

These source scripts consume the frozen expectations; `--freeze` is the initial
authoring route only and refuses to overwrite an existing expectation. The
old-state extractor likewise refuses an existing fixture directory. Do not
regenerate expectations during candidate qualification. No S08 Rust executable
or Rust-derived expected value is consulted.

The strict judge independently checks status/signal, stdout framing, stderr,
typed full transcript and denied/fake effect channels. Native drivers retain
raw channels, observe fake files independently, enforce streaming output/time
bounds and reap groups. Qualification rejects14 mutant processes and accepts
the unchanged positive control. It does not qualify every future Rust embedding;
a future mixed process must demonstrate the same original physical lock and
journal contract without an oracle fallback.

Actual author results, exact commands, failures, cleanup and source commit are
in the authorized absolute note root:
`/home/palantirkc/work/ws-replan/preparation/slices/S10/reference-v1/`.
All unresolved source/fence files there are deliberately retained fake states,
with no running writer. Root owns later retention/cleanup and acceptance;
independent Wonder review is still required.
