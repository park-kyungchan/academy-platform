#!/usr/bin/env bash
# LEAD ONLY, after a sole compiler-slot grant. The author did not run this file.
set -euo pipefail
repo=/home/palantirkc/work/ws-replan/wt/education-platform
results=${1:?Supply a NEW absolute result directory outside the checkout}
case "$results" in /*) ;; *) echo absolute_results_required >&2; exit 64;; esac
case "$results" in "$repo"|"$repo"/*) echo results_must_be_outside_checkout >&2; exit 64;; esac
mkdir "$results"
cd "$repo"
source /home/palantirkc/work/ws-replan/preparation/final-review/execution-env.sh
export CARGO_TARGET_DIR="$results/cargo-target"
git rev-parse HEAD > "$results/revision.txt"
git status --porcelain > "$results/git-status.txt"
if [ -s "$results/git-status.txt" ]; then echo clean_committed_candidate_required >&2; exit 64; fi
run() {
  local label=$1 status
  shift
  printf '%q ' "$@" > "$results/$label.command"
  set +e
  "$@" > "$results/$label.stdout" 2> "$results/$label.stderr"
  status=$?
  set -e
  printf '%s\n' "$status" > "$results/$label.exit"
  if [ "$status" -ne 0 ]; then return "$status"; fi
}
run test cargo test --locked --offline -j1 -p edu-academy-core
run clippy cargo clippy --locked --offline -j1 -p edu-academy-core --all-targets -- -D warnings
run build cargo build --locked --offline -j1 -p edu-academy-core --bin edu-academy-core
cp "$CARGO_TARGET_DIR/debug/edu-academy-core" "$results/edu-academy-core"
sha256sum "$results/edu-academy-core" > "$results/binary.sha256"
run all202 node migration/pure-semantics/candidate-v3/compare.mjs --candidate "$results/edu-academy-core" --results-root "$results/all202"
run backward54 node migration/pure-semantics/compare-candidate.mjs --candidate "$results/edu-academy-core" --results-root "$results/backward54"
run original15 node migration/pure-semantics/candidate-v3/admission.mjs --candidate "$results/edu-academy-core" --cases /home/palantirkc/work/ws-replan/preparation/slices/S08/PROTOCOL-ADMISSION-CASES.json --results-root "$results/original15"
run proposed13 node migration/pure-semantics/candidate-v3/admission.mjs --candidate "$results/edu-academy-core" --cases migration/pure-semantics/candidate-v3/admission-v3.json --results-root "$results/proposed13"
# Existing source-reference/17-control evidence is independently frozen. Optional
# new verification, always a new directory, never --freeze-reference:
# node migration/pure-semantics/verify.mjs --deps-root /home/palantirkc/projects/education-platform/modules/academy-workbench/tools/academy-mcp/node_modules --results-root "$results/reference-rerun"
