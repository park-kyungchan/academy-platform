#!/usr/bin/env python3
# Installs one bridge generation of academy-mcp into an install base.
# Usage: python3 -I <checkout>/modules/academy-workbench/tools/academy-mcp/install/loop_install.py <commit> <generation> <surface.json|none> --base <dir> [--kept <dir>]
#        python3 -I <checkout>/modules/academy-workbench/tools/academy-mcp/install/loop_install.py --restore <kept pointer> --base <dir>
# The source checkout is the top of the Git work tree this script lies in; node_modules is its module's. Every activation
# keeps the previous pointer bytes under --kept (default <base>/pointers) and prints the restore command. --restore writes
# kept pointer bytes back by rename after checking every pinned file's sha256 and base, then reads tools/list back.
# Build src/neutral-server.ts at <commit> of the checkout; copy routing/registry/endpoints of the base's active generation
# (checked against the pointer's sha256); pin the surface file when given; write bridge-pin and release.json; activate the
# pointer by rename; read tools/list back through the base's launcher.
import argparse, hashlib, json, os, re, shutil, subprocess, sys, tempfile
MOD = 'modules/academy-workbench/tools/academy-mcp'
SELF = os.path.realpath(__file__)
def generation(v):
    if not re.fullmatch(r'[A-Za-z0-9._-]{1,64}', v) or v in ('.', '..'): raise argparse.ArgumentTypeError('generation must match [A-Za-z0-9._-]{1,64}')
    return v
ap = argparse.ArgumentParser(prog='loop_install.py', allow_abbrev=False,
    usage='python3 -I loop_install.py <commit> <generation> <surface.json|none> --base <dir> [--kept <dir>]\n'
          '       python3 -I loop_install.py --restore <kept pointer> --base <dir>')
ap.add_argument('install', nargs='*', help='<commit> <generation> <surface.json|none>')
ap.add_argument('--restore', metavar='<kept pointer>')
ap.add_argument('--base', required=True, metavar='<dir>')
ap.add_argument('--kept', metavar='<dir>')
o = ap.parse_args()
if (o.restore is None and len(o.install) != 3) or (o.restore is not None and o.install): ap.error('give <commit> <generation> <surface.json|none>, or --restore <kept pointer>')
if o.restore is None:
    try: generation(o.install[1])
    except argparse.ArgumentTypeError as e: ap.error(str(e))
BASE = os.path.abspath(o.base)
KEEP = os.path.abspath(o.kept) if o.kept else os.path.join(BASE, 'pointers')
sha = lambda b: hashlib.sha256(b).hexdigest()
def wfile(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as f: f.write(data); f.flush(); os.fsync(f.fileno())
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
if o.restore is not None:
    b = open(o.restore, 'rb').read(); ptr = json.loads(b)
    if ptr.get('schemaVersion') != 'academy.mcp-release/1': sys.exit('restore_pointer_schema_unexpected')
    for k in ('routing', 'registry', 'endpoints', 'surface'):
        if k not in ptr: continue
        if not ptr[k]['path'].startswith(os.path.join(BASE, 'releases') + '/'): sys.exit('restore_outside_base@' + k)
        if sha(open(ptr[k]['path'], 'rb').read()) != ptr[k]['sha256']: sys.exit('restore_hash_mismatch@' + k)
    t = os.path.join(BASE, '.release.json.' + str(os.getpid())); wfile(t, b); os.rename(t, os.path.join(BASE, 'release.json'))
    print('restored', ptr['generation']); readback(); sys.exit(0)
commit, gen, surface = o.install
rel = os.path.join(BASE, 'releases'); final = os.path.join(rel, gen)
if not os.path.isfile(os.path.join(BASE, 'release.json')): sys.exit('base_pointer_missing@' + BASE)
if os.path.exists(final): sys.exit('generation_exists@' + final)
host = json.load(open(os.path.join(BASE, 'release.json')))
if host['schemaVersion'] != 'academy.mcp-release/1': sys.exit('host_pointer_schema_unexpected')
REPO = subprocess.run(['git', '-C', os.path.dirname(SELF), 'rev-parse', '--show-toplevel'], capture_output=True, text=True, check=True).stdout.strip()
DEPS = os.path.join(REPO, MOD, 'node_modules')
if not os.path.isdir(DEPS): sys.exit('module_node_modules_missing@' + DEPS)
full = subprocess.run(['git', '-C', REPO, 'rev-parse', commit + '^{commit}'], capture_output=True, text=True, check=True).stdout.strip()
tmp = tempfile.mkdtemp(prefix='.build-', dir=BASE)
try:
    arch = subprocess.run(['git', '-C', REPO, 'archive', full, MOD, 'apps/academy/dc-gang-a/src'], capture_output=True, check=True).stdout
    subprocess.run(['tar', '-x', '-C', tmp], input=arch, check=True)
    mdir = os.path.join(tmp, MOD); os.symlink(DEPS, os.path.join(mdir, 'node_modules'))
    out = os.path.join(tmp, 'server.mjs')
    subprocess.run(['/usr/local/bin/bun', 'build', 'src/neutral-server.ts', '--target=bun', '--outfile=' + out], cwd=mdir, check=True, stdout=subprocess.DEVNULL)
    server = open(out, 'rb').read()
finally:
    shutil.rmtree(tmp)
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
os.makedirs(KEEP, 0o700, exist_ok=True)
kept = os.path.join(KEEP, 'pointer.before-%s.%s.json' % (gen, os.path.basename(BASE)))
wfile(kept, open(os.path.join(BASE, 'release.json'), 'rb').read())
t = os.path.join(BASE, '.release.json.' + str(os.getpid())); wfile(t, pbytes); os.rename(t, os.path.join(BASE, 'release.json'))
print('restore with: python3 -I %s --restore %s --base %s' % (SELF, kept, BASE))
print('activated', gen, 'from', full[:12], 'server', sha(server)[:12], 'surface', pointer.get('surface', {}).get('sha256', 'none')[:12])
readback()
