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
echo "test-academy-mcp-install: ok"
