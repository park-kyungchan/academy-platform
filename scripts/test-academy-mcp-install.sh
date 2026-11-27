#!/usr/bin/env bash
# Tests the install scripts of academy-mcp (modules/academy-workbench/tools/academy-mcp/install) and the read-back
# script scripts/academy-mcp-installed.sh on a temporary base with invented generations. It starts no build and reads
# nothing outside its temporary directory: the read-back after a pointer is written or restored finds no launcher in
# the temporary base and fails, which the test expects. Run by ./check.
set -euo pipefail
root=$(cd "$(dirname "$(realpath "$0")")/.." && pwd)
installed=$root/scripts/academy-mcp-installed.sh
inst=$root/modules/academy-workbench/tools/academy-mcp/install
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT
cd "$tmp"
# Keep any process the scripts start (the launcher read-back) inside the temporary directory.
export HOME=$tmp XDG_CONFIG_HOME=$tmp/config XDG_CACHE_HOME=$tmp/cache
fail() { echo "test-academy-mcp-install: $*" >&2; exit 1; }
# run <expected exit> <out file> <command...>: stdout to the file, stderr to <out file>.err
run() {
  local want=$1 out=$2 got=0
  shift 2
  "$@" >"$out" 2>"$out.err" || got=$?
  [ "$got" = "$want" ] || { cat "$out" "$out.err" >&2; fail "exit $got, wanted $want: ${*##*/}"; }
}
sha() { sha256sum "$1" | cut -d' ' -f1; }
A=1111111111111111111111111111111111111111
B=2222222222222222222222222222222222222222

# 1. The read-back script: the three exits and the prior line.
base=$tmp/readback
mkdir -p "$base/releases/g-own" "$base/releases/g-next" "$base/releases/g-bare"
printf '{"generation":"g-own","sourceCommit":"%s"}' "$A" >"$base/releases/g-own/provenance.json"
printf '{"generation":"g-next","priorGeneration":"g-own"}' >"$base/releases/g-next/provenance.json"
printf '{"generation":"g-bare"}' >"$base/releases/g-bare/provenance.json"
run 2 o "$installed" --base "$base" 1111111
grep -q '^no_release_record@' o.err || fail "no release record not reported"
printf '{"generation":"g-own"}' >"$base/release.json"
run 0 o "$installed" --base "$base" 1111111
[ "$(cat o)" = "$(printf 'generation g-own\ncommit %s' "$A")" ] || fail "own commit lines: $(cat o)"
run 0 o "$installed" --base "$base" "$A"
run 1 o "$installed" --base "$base" 2222222
[ "$(cat o)" = "$(printf 'generation g-own\ncommit %s' "$A")" ] || fail "other commit lines: $(cat o)"
printf '{"generation":"g-next"}' >"$base/release.json"
run 1 o "$installed" --base "$base" 1111111
[ "$(cat o)" = "$(printf 'generation g-next\ncommit %s (prior g-own)' "$A")" ] || fail "prior lines: $(cat o)"
printf '{"generation":"g-bare"}' >"$base/release.json"
run 2 o "$installed" --base "$base" 1111111
[ "$(cat o)" = "generation g-bare" ] || fail "no commit lines: $(cat o)"
# A chain longer than five steps finds no commit.
prev=g-own
for i in 1 2 3 4 5 6; do
  mkdir -p "$base/releases/g-chain$i"
  printf '{"generation":"g-chain%s","priorGeneration":"%s"}' "$i" "$prev" >"$base/releases/g-chain$i/provenance.json"
  prev=g-chain$i
done
printf '{"generation":"g-chain5"}' >"$base/release.json"
run 1 o "$installed" --base "$base" 1111111
[ "$(sed -n 2p o)" = "commit $A (prior g-own)" ] || fail "five steps: $(cat o)"
printf '{"generation":"g-chain6"}' >"$base/release.json"
run 2 o "$installed" --base "$base" 1111111
run 2 o "$installed" --base "$base"
run 2 o "$installed" --base "$base" not-hex
run 2 o "$installed" --unknown 1111111

# 2. A base with an active generation g-old and a staged generation g-new (no build, no launcher).
base=$tmp/base
for g in g-old g-new; do
  d=$base/releases/$g
  mkdir -p "$d"
  printf '{"admissionPath":"/nonexistent/%s"}' "$g" >"$d/routing.json"
  printf '[]\n' >"$d/registry.json"
  printf '{"dailyrecord":{"command":"/bin/false","args":[]}}' >"$d/endpoints.json"
  printf 'export {};\n' >"$d/server.mjs"
  printf '{"path":"%s","sha256":"%s"}' "$d/server.mjs" "$(sha "$d/server.mjs")" >"$d/bridge-pin.json"
  printf '{"schemaVersion":"academy.mcp-release/1","generation":"%s"' "$g" >"$d/release.json"
  for k in routing registry endpoints; do
    printf ',"%s":{"path":"%s","sha256":"%s"}' "$k" "$d/$k.json" "$(sha "$d/$k.json")" >>"$d/release.json"
  done
  printf '}\n' >>"$d/release.json"
done
cp "$base/releases/g-old/release.json" "$base/release.json"
mkdir -p "$tmp/arts"
snapshot() { (cd "$tmp" && find base arts -printf '%p %s %m\n' | LC_ALL=C sort && cat base/release.json); }
snapshot >before

# 3. Each install script refuses arguments it lacks or does not know, before any effect.
run 2 o python3 -I "$inst/loop_install.py"
grep -q '^usage:' o.err || fail "loop_install usage"
run 2 o python3 -I "$inst/loop_install.py" "$A" g-x none
grep -q -- '--base' o.err || fail "loop_install without --base"
run 2 o python3 -I "$inst/loop_install.py" "$A" g-x none --base "$base" --host
run 2 o python3 -I "$inst/loop_install.py" --restore "$tmp/none.json" g-x --base "$base"
run 2 o python3 -I "$inst/endpoint_install.py"
grep -q '^usage:' o.err || fail "endpoint_install usage"
run 2 o python3 -I "$inst/endpoint_install.py" "$A" g-x art --base "$base"
grep -q -- '--artifacts' o.err || fail "endpoint_install without --artifacts"
run 2 o python3 -I "$inst/endpoint_install.py" "$A" g-x art --base "$base" --artifacts "$tmp/arts" --owner-uid me
run 2 o python3 -I "$inst/endpoint_install.py" "$A" ../g-x art --base "$base" --artifacts "$tmp/arts"
run 2 o python3 -I "$inst/endpoint_install.py" "$A" g-x art --base "$base" --artifacts "$tmp/arts" --unknown
run 2 o python3 -I "$inst/registry_surface_install.py"
grep -q '^usage:' o.err || fail "registry_surface_install usage"
run 2 o python3 -I "$inst/registry_surface_install.py" g-x surface.json dailyrecord - --base "$base" --activate
grep -q -- '--commit' o.err || fail "registry_surface_install without --commit"
run 2 o python3 -I "$inst/registry_surface_install.py" g-x surface.json dailyrecord - --base "$base" --commit 1111111
run 2 o python3 -I "$inst/registry_surface_install.py" g-x surface.json dailyrecord - --commit "$A"
run 2 o python3 -I "$inst/registry_surface_install.py" g-x surface.json dailyrecord - --base "$base" --commit "$A" --host
snapshot >after
cmp -s before after || fail "a refused call changed the base"

# 4. A pointer written and restored: endpoint_install.py activates the staged g-new (its generation exists, so it
# builds nothing) and keeps the old pointer; loop_install.py --restore writes the kept pointer back.
run 1 o python3 -I "$inst/endpoint_install.py" "$A" g-new art --base "$base" --artifacts "$tmp/arts" --kept "$tmp/kept" --activate
kept=$tmp/kept/pointer.before-g-new.base.json
[ "$(head -n 1 o)" = "activated g-new previous g-old kept $kept" ] || fail "activate line: $(cat o)"
cmp -s "$base/release.json" "$base/releases/g-new/release.json" || fail "pointer not written"
cmp -s "$kept" "$base/releases/g-old/release.json" || fail "old pointer not kept"
[ -z "$(ls -A "$tmp/arts")" ] || fail "an artifact was written"
run 1 o python3 -I "$inst/loop_install.py" --restore "$kept" --base "$base"
[ "$(head -n 1 o)" = "restored g-old" ] || fail "restore line: $(cat o)"
cmp -s "$base/release.json" "$base/releases/g-old/release.json" || fail "pointer not restored"
# The default kept directory is <base>/pointers.
run 1 o python3 -I "$inst/endpoint_install.py" "$A" g-new art --base "$base" --artifacts "$tmp/arts" --activate
[ -f "$base/pointers/pointer.before-g-new.base.json" ] || fail "default kept directory"
# A restore refuses a pointer whose pins do not match.
printf 'changed\n' >>"$base/releases/g-old/registry.json"
run 1 o python3 -I "$inst/loop_install.py" --restore "$kept" --base "$base"
grep -q '^restore_hash_mismatch@registry' o.err || fail "restore of a changed pin"
cmp -s "$base/release.json" "$base/releases/g-new/release.json" || fail "a refused restore changed the pointer"
# The build archive holds apps/academy/dc-gang-a/src whole, not only its lms part.
! grep -q "'apps/academy/dc-gang-a/src/lms'" "$inst/loop_install.py" "$inst/endpoint_install.py" || fail "an install script archives only src/lms"

# 5. The option --paper-admit (specs/academy-mcp/paper-admit.md), on a scratch base with invented ids. Its refusals come
# before the build, so they run here; a success is read through --paper-admit-preview, which builds and writes nothing.
pbase=$tmp/pbase
mkdir -p "$pbase/releases/g-p"
printf '{"admissionSha256":"%s","paperScopes":[{"endpoint":"paper1","studentIds":["90001","90002"]},{"endpoint":"paper2","studentIds":["90003"]}]}' \
  "$(printf 'x' | sha256sum | cut -d' ' -f1)" >"$pbase/releases/g-p/routing.json"
printf '[]\n' >"$pbase/releases/g-p/registry.json"
printf '{}' >"$pbase/releases/g-p/endpoints.json"
printf '{"schemaVersion":"academy.mcp-release/1","generation":"g-p"' >"$pbase/releases/g-p/release.json"
for k in routing registry endpoints; do
  printf ',"%s":{"path":"%s","sha256":"%s"}' "$k" "$pbase/releases/g-p/$k.json" "$(sha "$pbase/releases/g-p/$k.json")" >>"$pbase/releases/g-p/release.json"
done
printf '}\n' >>"$pbase/releases/g-p/release.json"
cp "$pbase/releases/g-p/release.json" "$pbase/release.json"
psnap() { (cd "$tmp" && find pbase -printf '%p %s %m\n' | LC_ALL=C sort && cat pbase/release.json pbase/releases/g-p/routing.json); }
psnap >pbefore
# padmit_refused <code line> <args...>: an install refused with that last line of standard error, exit 1, base unchanged, no id on either stream.
padmit_refused() {
  local code=$1; shift
  run 1 po python3 -I "$inst/loop_install.py" "$A" g-q none --base "$pbase" "$@"
  [ "$(tail -n 1 po.err)" = "$code" ] || fail "paper-admit refusal: $(cat po.err), wanted $code"
  [ ! -s po ] || fail "paper-admit refusal wrote to standard output"
  psnap >pafter
  cmp -s pbefore pafter || fail "a refused paper-admit changed the base ($code)"
  ! grep -q '[0-9]\{5\}' po po.err || fail "an id in a paper-admit refusal ($code)"
}
padmit_preview() { run "$1" pv python3 -I "$inst/loop_install.py" --paper-admit-preview --base "$pbase" "${@:2}"; }
# The sha256 of the routing.json the admission of the arguments yields, computed apart from the script.
padmit_expect() {
  python3 -I - "$pbase/releases/g-p/routing.json" "$@" <<'PY'
import hashlib, json, sys
r = json.load(open(sys.argv[1]))
for a in sys.argv[2:]:
    ep, ids = a.split('=')
    for e in r['paperScopes']:
        if e['endpoint'] == ep: e['studentIds'] += ids.split(',')
print(hashlib.sha256(json.dumps(r, separators=(',', ':')).encode()).hexdigest())
PY
}

paper_admit_r2_form() {
  local bad
  for bad in 'paper1' '=90010' 'paper1=' 'paper1=90010,,90011' 'paper1=90010,' 'paper1= 90010' 'Paper1=90010' '1paper=90010'; do
    padmit_refused paper_admit_form --paper-admit "$bad"
  done
  padmit_preview 1 --paper-admit paper1
  [ "$(tail -n 1 pv.err)" = paper_admit_form ] || fail "r2 preview: $(cat pv.err)"
}
paper_admit_r3_endpoint_twice() {
  padmit_refused paper_admit_form --paper-admit paper1=90010 --paper-admit paper1=90011
}
paper_admit_r4_id_twice() {
  padmit_refused paper_admit_form --paper-admit paper1=90010,90010
  padmit_refused paper_admit_form --paper-admit paper1=90010 --paper-admit paper2=90010
}
paper_admit_r5_restore() {
  run 1 po python3 -I "$inst/loop_install.py" --restore "$pbase/release.json" --base "$pbase" --paper-admit paper1=90010
  [ "$(tail -n 1 po.err)" = paper_admit_form ] || fail "r5: $(cat po.err)"
  psnap >pafter; cmp -s pbefore pafter || fail "a refused restore with the option changed the base"
}
paper_admit_r6_id_invalid() {
  padmit_refused paper_admit_id_invalid@1 --paper-admit paper1=0123
  padmit_refused paper_admit_id_invalid@2 --paper-admit paper1=90010,12345678901234567
  padmit_refused paper_admit_id_invalid@3 --paper-admit paper1=90010,90011,9001x
}
paper_admit_r7_position_not_id() {
  padmit_refused paper_admit_id_invalid@2 --paper-admit paper1=90010,-5 --paper-admit paper2=90011
  padmit_refused paper_admit_id_invalid@1 --paper-admit paper1=90010 --paper-admit paper2=00007
  padmit_refused paper_admit_id_present@2 --paper-admit paper1=90010,90003
}
paper_admit_r8_no_id_printed() {
  padmit_preview 0 --paper-admit paper1=90010,90011 --paper-admit paper2=90012
  ! cat pv pv.err | sed 's/[0-9a-f]\{64\}//' | grep -q '9001' || fail "an id on standard output or error of a preview"
  padmit_preview 1 --paper-admit paper1=90001
  ! cat pv pv.err | sed 's/[0-9a-f]\{64\}//' | grep -q '9000' || fail "an id in a refused preview"
}
paper_admit_r9_endpoint_unknown() {
  padmit_refused paper_admit_endpoint_unknown@paper3 --paper-admit paper3=90010
  padmit_refused paper_admit_endpoint_unknown@paper3 --paper-admit paper1=90010 --paper-admit paper3=90011
  # A routing.json without paperScopes has no entry either.
  local nb=$tmp/pbase-none
  mkdir -p "$nb/releases/g-n"
  printf '{}' >"$nb/releases/g-n/routing.json"
  printf '{"schemaVersion":"academy.mcp-release/1","generation":"g-n","routing":{"path":"%s","sha256":"%s"}}\n' "$nb/releases/g-n/routing.json" "$(sha "$nb/releases/g-n/routing.json")" >"$nb/release.json"
  run 1 po python3 -I "$inst/loop_install.py" --paper-admit-preview --base "$nb" --paper-admit paper1=90010
  [ "$(tail -n 1 po.err)" = paper_admit_endpoint_unknown@paper1 ] || fail "r9 no paperScopes: $(cat po.err)"
  [ -z "$(cd "$nb" && find . -newer release.json -type f)" ] || fail "r9 wrote in the base"
}
paper_admit_r10_id_present() {
  padmit_refused paper_admit_id_present@1 --paper-admit paper1=90001
  padmit_refused paper_admit_id_present@2 --paper-admit paper1=90010,90003
  padmit_refused paper_admit_id_present@1 --paper-admit paper2=90002
}
paper_admit_r11_limit() {
  local lb=$tmp/pbase-full ids
  mkdir -p "$lb/releases/g-l"
  python3 -I - "$lb/releases/g-l/routing.json" <<'PY'
import json, sys
json.dump({'paperScopes': [{'endpoint': 'paper1', 'studentIds': [str(100000 + i) for i in range(999)]}]}, open(sys.argv[1], 'w'), separators=(',', ':'))
PY
  printf '{"schemaVersion":"academy.mcp-release/1","generation":"g-l","routing":{"path":"%s","sha256":"%s"}}\n' "$lb/releases/g-l/routing.json" "$(sha "$lb/releases/g-l/routing.json")" >"$lb/release.json"
  run 1 po python3 -I "$inst/loop_install.py" --paper-admit-preview --base "$lb" --paper-admit paper1=90010,90011
  [ "$(tail -n 1 po.err)" = paper_admit_limit ] || fail "r11: $(cat po.err)"
  run 0 po python3 -I "$inst/loop_install.py" --paper-admit-preview --base "$lb" --paper-admit paper1=90010
  grep -q ' admitted 1$' po || fail "r11 at the limit: $(cat po)"
}
paper_admit_r12_order() {
  # form before an invalid id; an invalid id before an unknown endpoint; unknown endpoint before a present id; present id before the limit.
  padmit_refused paper_admit_form --paper-admit paper1=00001 --paper-admit paper1=90010
  padmit_refused paper_admit_id_invalid@1 --paper-admit paper3=00001
  padmit_refused paper_admit_endpoint_unknown@paper3 --paper-admit paper1=90001 --paper-admit paper3=90010
  padmit_refused paper_admit_id_present@1 --paper-admit paper2=90001 --paper-admit paper1=90003
}
paper_admit_r13_refusal_writes_nothing() {
  local kept=$tmp/pkept
  run 1 po python3 -I "$inst/loop_install.py" "$A" g-q none --base "$pbase" --kept "$kept" --paper-admit paper1=90001
  [ "$(tail -n 1 po.err)" = paper_admit_id_present@1 ] || fail "r13: $(cat po.err)"
  [ ! -e "$kept" ] || fail "r13 made the kept directory"
  [ ! -e "$pbase/releases/g-q" ] && [ -z "$(ls -A "$pbase" | grep '^\.')" ] || fail "r13 left a directory in the base"
  psnap >pafter; cmp -s pbefore pafter || fail "r13 changed the base"
}
paper_admit_r14_ids_appended() {
  padmit_preview 0 --paper-admit paper1=90010,90011 --paper-admit paper2=90012
  [ "$(cat pv)" = "routing sha256 $(padmit_expect paper1=90010,90011 paper2=90012) admitted 3" ] || fail "r14: $(cat pv)"
  padmit_preview 0 --paper-admit paper2=90012
  [ "$(cat pv)" = "routing sha256 $(padmit_expect paper2=90012) admitted 1" ] || fail "r14 one endpoint: $(cat pv)"
}
paper_admit_r15_other_members_kept() {
  # admissionSha256 is kept and no member is added: the expected file is the active one with only the ids added.
  padmit_preview 0 --paper-admit paper1=90010
  [ "$(cat pv)" = "routing sha256 $(padmit_expect paper1=90010) admitted 1" ] || fail "r15: $(cat pv)"
}
paper_admit_r19_hash_checked() {
  printf ' ' >>"$pbase/releases/g-p/routing.json"
  padmit_preview 1 --paper-admit paper1=90010
  [ "$(tail -n 1 pv.err)" = copy_hash_mismatch@routing ] || fail "r19: $(cat pv.err)"
  # Restore the file for the cases after this one.
  python3 -I - "$pbase/releases/g-p/routing.json" <<'PY'
import sys
b = open(sys.argv[1], 'rb').read(); open(sys.argv[1], 'wb').write(b[:-1])
PY
  psnap >pafter; cmp -s pbefore pafter || fail "r19 restored file differs"
}
paper_admit_r20_preview_writes_nothing() {
  padmit_preview 0 --paper-admit paper1=90010
  [ "$(wc -l <pv)" = 1 ] && grep -qE '^routing sha256 [0-9a-f]{64} admitted 1$' pv || fail "r20 line: $(cat pv)"
  [ ! -s pv.err ] || fail "r20 standard error: $(cat pv.err)"
  psnap >pafter; cmp -s pbefore pafter || fail "r20 preview changed the base"
  # No positional word, and no install, with the preview.
  run 2 po python3 -I "$inst/loop_install.py" --paper-admit-preview "$A" g-q none --base "$pbase"
}
paper_admit_help() {
  python3 -I "$inst/loop_install.py" --help 2>&1 | grep -q -- '--paper-admit' || fail "help does not name --paper-admit"
}
paper_admit_r2_form
paper_admit_r3_endpoint_twice
paper_admit_r4_id_twice
paper_admit_r5_restore
paper_admit_r6_id_invalid
paper_admit_r7_position_not_id
paper_admit_r8_no_id_printed
paper_admit_r9_endpoint_unknown
paper_admit_r10_id_present
paper_admit_r11_limit
paper_admit_r12_order
paper_admit_r13_refusal_writes_nothing
paper_admit_r14_ids_appended
paper_admit_r15_other_members_kept
paper_admit_r19_hash_checked
paper_admit_r20_preview_writes_nothing
paper_admit_help
echo "test-academy-mcp-install: ok"
