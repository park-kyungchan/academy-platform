#!/usr/bin/env bash
set -euo pipefail
repo=/home/palantirkc/work/ws-replan/wt/education-platform
out=${1:?NEW absolute task-owned native directory required}
case "$out" in /home/palantirkc/work/ws-replan/preparation/slices/S08/core-fix-v4/native/*) ;; *) exit 64;; esac
cd "$repo"
source /home/palantirkc/work/ws-replan/preparation/final-review/execution-env.sh
export CARGO_TARGET_DIR=/home/palantirkc/work/ws-replan/preparation/slices/S08/core-fix-v4/native/target
mkdir "$out"
git rev-parse HEAD > "$out/revision.txt"
git status --porcelain > "$out/git-status.txt"
if [ -s "$out/git-status.txt" ]; then echo clean_committed_candidate_required >&2; exit 64; fi
run() { python3 migration/pure-semantics/candidate-v4/command.py "$(basename "$out")-$1" "${@:2}"; }
run fmt cargo fmt --all --check
run test cargo test --locked --offline -j1
run clippy cargo clippy --locked --offline -j1 --all-targets -- -D warnings
run build cargo build --locked --offline -j1
cp "$CARGO_TARGET_DIR/debug/edu-academy-core" "$out/edu-academy-core"
sha256sum "$out/edu-academy-core" > "$out/binary.sha256"
run pins node migration/pure-semantics/candidate-v4/verify-pins.mjs "$out/pins.json"
run old202 node migration/pure-semantics/candidate-v4/old202.mjs --candidate "$out/edu-academy-core" --results-root "$out/old202"
run leaf54 node migration/pure-semantics/candidate-v4/leaf54.mjs --candidate "$out/edu-academy-core" --results-root "$out/leaf54"
run original15 node migration/pure-semantics/candidate-v4/protocol-admission.mjs --candidate "$out/edu-academy-core" --cases /home/palantirkc/work/ws-replan/preparation/slices/S08/PROTOCOL-ADMISSION-CASES.json --results-root "$out/original15"
run proposed13 node migration/pure-semantics/candidate-v4/protocol-admission.mjs --candidate "$out/edu-academy-core" --cases migration/pure-semantics/candidate-v3/admission-v3.json --results-root "$out/proposed13"
run new603 node migration/pure-semantics/candidate-v4/compare.mjs --candidate "$out/edu-academy-core" --results "$out/new603"
run controls node migration/pure-semantics/candidate-v4/controls.mjs --actual "$out/new603" --old-actual "$out/old202" --results "$out/controls"
git status --porcelain > "$out/final-git-status.txt"
git rev-parse HEAD > "$out/final-revision.txt"
