#!/usr/bin/env bash
# Prints which source commit the active generation of an academy-mcp install base was built from. Changes nothing.
# Usage: scripts/academy-mcp-installed.sh [--base <dir>] <commit>
#   <dir>     the install base (default /root/.local/share/academy-mcp-host-20261106)
#   <commit>  4 to 40 hexadecimal characters: the commit the caller expects
# Reads <base>/release.json, takes its member generation, reads releases/<generation>/provenance.json and prints
#   generation <name>
#   commit <sourceCommit>
# A provenance without sourceCommit is followed through its member priorGeneration, at most five steps; the second
# line then ends " (prior <generation>)", naming the generation whose provenance held the commit.
# Exit 0: the commit is the active generation's own and begins with <commit>.
# Exit 1: the commit is another one, or was found only through a prior generation.
# Exit 2: no release record or no commit is found (and for a usage error).
set -euo pipefail
usage() { echo "usage: academy-mcp-installed.sh [--base <dir>] <commit>" >&2; exit 2; }
base=/root/.local/share/academy-mcp-host-20261106
commit=
while [ "$#" -gt 0 ]; do
  case $1 in
    --base) [ "$#" -ge 2 ] || usage; base=$2; shift 2 ;;
    -*) usage ;;
    *) [ -z "$commit" ] || usage; commit=$1; shift ;;
  esac
done
[ -n "$commit" ] || usage
[[ $commit =~ ^[0-9a-fA-F]{4,40}$ ]] || usage
exec python3 -I - "$base" "${commit,,}" <<'PY'
import json, os, re, sys
base, want = sys.argv[1], sys.argv[2]
def missing(reason):
    print(reason, file=sys.stderr)
    sys.exit(2)
def generation(v):
    return isinstance(v, str) and re.fullmatch(r'[A-Za-z0-9._-]{1,64}', v) is not None and v not in ('.', '..')
def load(path):
    try:
        with open(path, 'rb') as f: data = f.read(1 << 20)
        v = json.loads(data)
    except (OSError, ValueError):
        return None
    return v if isinstance(v, dict) else None
pointer = load(os.path.join(base, 'release.json'))
if pointer is None or not generation(pointer.get('generation')):
    missing('no_release_record@' + os.path.join(base, 'release.json'))
active = pointer['generation']
print('generation', active)
current = active
for step in range(6):
    provenance = load(os.path.join(base, 'releases', current, 'provenance.json'))
    if provenance is None:
        missing('no_provenance@' + current)
    commit = provenance.get('sourceCommit')
    if commit is not None:
        if not isinstance(commit, str) or re.fullmatch(r'[0-9a-f]{4,40}', commit) is None:
            missing('source_commit_unreadable@' + current)
        print('commit ' + commit + ('' if step == 0 else ' (prior %s)' % current))
        sys.exit(0 if step == 0 and commit.startswith(want) else 1)
    prior = provenance.get('priorGeneration')
    if not generation(prior):
        break
    current = prior
missing('no_source_commit@' + active)
PY
