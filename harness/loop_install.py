#!/usr/bin/env python3
# Installs one loop generation of academy-mcp into the loop base (never the host base the user's sessions use).
# Usage: python3 -I harness/loop_install.py <commit> <generation> <surface.json|none> [--base <dir>] [--host]
#        python3 -I harness/loop_install.py --restore <saved pointer> [--base <dir>] [--host]
# --host (the user allowed host installs, 2026-11-17 ~10:30Z): install into the host base the user's sessions use. Every
# activation keeps the previous pointer bytes under private/ and prints the restore command. --restore writes kept
# pointer bytes back by rename after checking every pinned file's sha256 and base, then reads tools/list back.
# Build src/neutral-server.ts at <commit> of the task worktree; copy routing/registry/endpoints of the HOST's current
# generation (checked against the host pointer's sha256); pin the surface file when given; write bridge-pin and
# release.json; activate the loop pointer by rename; read tools/list back through the launcher.
import hashlib, json, os, shutil, subprocess, sys, tempfile
HOST = '/root/.local/share/academy-mcp-host-20261106'
BASE = '/root/.local/share/academy-mcp-parity-loop'
REPO = '/home/palantirkc/work/edu-mcp-tier-parity/wt/education-platform'
MOD = 'modules/academy-workbench/tools/academy-mcp'
DEPS = '/home/palantirkc/platforms/education-platform/' + MOD + '/node_modules'
sha = lambda b: hashlib.sha256(b).hexdigest()
def wfile(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as f: f.write(data); f.flush(); os.fsync(f.fileno())
KEEP = '/home/palantirkc/work/edu-mcp-tier-parity/private'
if '--base' in sys.argv: BASE = sys.argv[sys.argv.index('--base') + 1]
if '--host' in sys.argv: BASE = HOST
elif BASE == HOST: sys.exit('base_is_host')
def readback():
    p = subprocess.Popen(['/usr/bin/env', 'ACADEMY_MCP_INSTALL_DIR=' + BASE, '/usr/local/bin/bun', '--no-env-file', os.path.join(BASE, 'launcher.mjs')], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    msgs = [{'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2025-06-18', 'capabilities': {}, 'clientInfo': {'name': 'loop-readback', 'version': '1'}}},
            {'jsonrpc': '2.0', 'method': 'notifications/initialized'}, {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'}]
    p.stdin.write(''.join(json.dumps(m) + '\n' for m in msgs).encode()); p.stdin.flush()
    result = None
    for line in p.stdout:
        try: m = json.loads(line)
        except ValueError: continue
        if m.get('id') == 2: result = m.get('result'); break
    p.stdin.close(); p.terminate(); err = p.stderr.read().decode()[-300:]; p.wait(timeout=20)
    if not result: sys.exit('readback_failed: ' + err.replace('\n', ' ')[-160:])
    names = [t['name'] for t in result['tools']]
    compact = json.dumps(result, ensure_ascii=False, separators=(',', ':')).encode()
    print('tools', len(names), 'bytes', len(compact), 'names_sha', sha('\n'.join(sorted(names)).encode())[:12])
if '--restore' in sys.argv:
    b = open(sys.argv[sys.argv.index('--restore') + 1], 'rb').read(); ptr = json.loads(b)
    if ptr.get('schemaVersion') != 'academy.mcp-release/1': sys.exit('restore_pointer_schema_unexpected')
    for k in ('routing', 'registry', 'endpoints', 'surface'):
        if k not in ptr: continue
        if not ptr[k]['path'].startswith(os.path.join(BASE, 'releases') + '/'): sys.exit('restore_outside_base@' + k)
        if sha(open(ptr[k]['path'], 'rb').read()) != ptr[k]['sha256']: sys.exit('restore_hash_mismatch@' + k)
    t = os.path.join(BASE, '.release.json.' + str(os.getpid())); wfile(t, b); os.rename(t, os.path.join(BASE, 'release.json'))
    print('restored', ptr['generation']); readback(); sys.exit(0)
commit, gen, surface = sys.argv[1], sys.argv[2], sys.argv[3]
rel = os.path.join(BASE, 'releases'); final = os.path.join(rel, gen)
if not os.path.isdir(BASE):
    os.mkdir(BASE, 0o700); os.mkdir(rel, 0o700); os.mkdir(os.path.join(BASE, 'receipts'), 0o700)
    wfile(os.path.join(BASE, 'launcher.mjs'), open(os.path.join(HOST, 'launcher.mjs'), 'rb').read())
if os.path.exists(final): sys.exit('generation_exists@' + final)
host = json.load(open(os.path.join(HOST, 'release.json')))
if host['schemaVersion'] != 'academy.mcp-release/1': sys.exit('host_pointer_schema_unexpected')
tmp = tempfile.mkdtemp(prefix='acad-loop-build-')
full = subprocess.run(['git', '-C', REPO, 'rev-parse', commit + '^{commit}'], capture_output=True, text=True, check=True).stdout.strip()
arch = subprocess.run(['git', '-C', REPO, 'archive', full, MOD, 'apps/academy/dc-gang-a/src/lms'], capture_output=True, check=True).stdout
subprocess.run(['tar', '-x', '-C', tmp], input=arch, check=True)
mdir = os.path.join(tmp, MOD); os.symlink(DEPS, os.path.join(mdir, 'node_modules'))
out = os.path.join(tmp, 'server.mjs')
subprocess.run(['/usr/local/bin/bun', 'build', 'src/neutral-server.ts', '--target=bun', '--outfile=' + out], cwd=mdir, check=True, stdout=subprocess.DEVNULL)
server = open(out, 'rb').read(); shutil.rmtree(tmp)
stage = os.path.join(rel, '.' + gen + '.staging'); os.mkdir(stage, 0o700)
pointer = {'schemaVersion': 'academy.mcp-release/1', 'generation': gen}
for k in ('routing', 'registry', 'endpoints'):
    b = open(host[k]['path'], 'rb').read()
    if sha(b) != host[k]['sha256']: sys.exit('copy_hash_mismatch@' + k)
    wfile(os.path.join(stage, k + '.json'), b)
    pointer[k] = {'path': os.path.join(final, k + '.json'), 'sha256': sha(b)}
if surface != 'none':
    b = open(surface, 'rb').read(); json.loads(b)
    wfile(os.path.join(stage, 'surface.json'), b)
    pointer['surface'] = {'path': os.path.join(final, 'surface.json'), 'sha256': sha(b)}
wfile(os.path.join(stage, 'server.mjs'), server)
wfile(os.path.join(stage, 'bridge-pin.json'), json.dumps({'path': os.path.join(final, 'server.mjs'), 'sha256': sha(server)}).encode())
wfile(os.path.join(stage, 'provenance.json'), json.dumps({'generation': gen, 'sourceCommit': full, 'surface': surface, 'hostGeneration': host['generation'], 'serverSha256': sha(server)}).encode())
pbytes = json.dumps(pointer, indent=2).encode() + b'\n'
wfile(os.path.join(stage, 'release.json'), pbytes)
os.rename(stage, final)
kept = os.path.join(KEEP, 'pointer.before-%s.%s.json' % (gen, os.path.basename(BASE)))
wfile(kept, open(os.path.join(BASE, 'release.json'), 'rb').read())
t = os.path.join(BASE, '.release.json.' + str(os.getpid())); wfile(t, pbytes); os.rename(t, os.path.join(BASE, 'release.json'))
print('restore with: python3 -I harness/loop_install.py --restore %s --base %s%s' % (kept, BASE, ' --host' if BASE == HOST else ''))
print('activated', gen, 'from', full[:12], 'server', sha(server)[:12], 'surface', pointer.get('surface', {}).get('sha256', 'none')[:12])
readback()
