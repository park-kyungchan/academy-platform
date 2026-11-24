#!/usr/bin/env python3
# Copied from /home/palantirkc/work/ws-program/runs/academy-mcp-endpoint-install.py (used for the PR 133 install) with four
# changes: this task's worktree; a pinned surface is copied and kept in the pointer; the previous pointer is kept under
# this task's private/. Restore: python3 -I harness/loop_install.py --restore <kept> --base <base> [--host].
# Replaces the dailyrecord endpoint's bundle with a build of a merged commit, as a new generation.
# Usage: python3 -I academy-mcp-endpoint-install.py <base> <commit> <generation> <artifact-name> [--probe] [--activate]
# Stage (only new paths are written): build src/dailyrecord-server.ts at <commit> from the repository root of a git archive
# (the deployed bundle's method), write it under ARTS/<artifact-name>/ owned by uid 10000, write releases/<generation> with
# the bridge, routing and registry of the pointer's generation and an endpoints.json whose dailyrecord spec names the new
# bundle and its pin. --probe: start the staged dailyrecord endpoint exactly as endpoints.json does and list its tools.
# --activate: keep the old pointer's bytes beside this script, swap the pointer by rename, read back through the launcher.
import hashlib, json, os, subprocess, sys, tempfile
a = [x for x in sys.argv[1:] if not x.startswith('--')]
base, commit, gen, art_name = a
REPO = '/home/palantirkc/work/edu-mcp-tier-parity/wt/education-platform'
MOD = 'modules/academy-workbench/tools/academy-mcp'
DEPS = '/home/palantirkc/platforms/education-platform/' + MOD + '/node_modules'
ARTS = '/opt/data/profiles/dc-gang-a-dev/artifacts/class-operations-20261023'
sha = lambda b: hashlib.sha256(b).hexdigest()
def wfile(path, data, owner=None):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as f: f.write(data); f.flush(); os.fsync(f.fileno())
    if owner: os.chown(path, owner, owner)
pointer_path = os.path.join(base, 'release.json')
old_bytes = open(pointer_path, 'rb').read(); old = json.loads(old_bytes); old_gen = old['generation']
rel = os.path.join(base, 'releases'); final = os.path.join(rel, gen)
for k in ('routing', 'registry', 'endpoints'):
    if old[k]['path'] != os.path.join(rel, old_gen, k + '.json'): sys.exit('pointer_path_outside_base@' + k)
if not os.path.exists(final):
    full = subprocess.run(['git', '-C', REPO, 'rev-parse', commit + '^{commit}'], capture_output=True, text=True, check=True).stdout.strip()
    tmp = tempfile.mkdtemp(prefix='acad-ep-')
    arch = subprocess.run(['git', '-C', REPO, 'archive', full, MOD, 'apps/academy/dc-gang-a/src/lms'], capture_output=True, check=True).stdout
    subprocess.run(['tar', '-x', '-C', tmp], input=arch, check=True)
    os.symlink(DEPS, os.path.join(tmp, MOD, 'node_modules'))
    out = os.path.join(tmp, 'out.mjs')
    subprocess.run(['/usr/local/bin/bun', 'build', MOD + '/src/dailyrecord-server.ts', '--target=bun', '--outfile=' + out], cwd=tmp, check=True, stdout=subprocess.DEVNULL)
    bundle = open(out, 'rb').read(); subprocess.run(['rm', '-rf', tmp], check=True)
    art = os.path.join(ARTS, art_name); new_path = os.path.join(art, 'dailyrecord-server.mjs')
    os.mkdir(art, 0o700); os.chown(art, 10000, 10000); wfile(new_path, bundle, 10000)
    src = os.path.join(rel, old_gen); ep = json.loads(open(os.path.join(src, 'endpoints.json'), 'rb').read())
    if sha(open(os.path.join(src, 'endpoints.json'), 'rb').read()) != old['endpoints']['sha256']: sys.exit('copy_hash_mismatch@endpoints')
    args = ep['dailyrecord']['args']; idx = [i for i, x in enumerate(args) if x.startswith('{')]
    if len(idx) != 1: sys.exit('endpoint_spec_unqualified')
    spec = json.loads(args[idx[0]])
    olds = [p for p in spec['argv'] if p.endswith('/dailyrecord-server.mjs')]
    if len(olds) != 1 or olds[0] not in spec['pins']: sys.exit('endpoint_bundle_unqualified')
    spec['argv'] = [new_path if p == olds[0] else p for p in spec['argv']]
    del spec['pins'][olds[0]]; spec['pins'][new_path] = sha(bundle)
    args[idx[0]] = json.dumps(spec)
    ep_bytes = json.dumps(ep, indent=1).encode()
    stage = os.path.join(rel, '.' + gen + '.staging'); os.mkdir(stage, 0o700)
    for k in ('routing', 'registry') + (('surface',) if 'surface' in old else ()):
        b = open(os.path.join(src, k + '.json'), 'rb').read()
        if sha(b) != old[k]['sha256']: sys.exit('copy_hash_mismatch@' + k)
        wfile(os.path.join(stage, k + '.json'), b)
    bridge = open(os.path.join(src, 'server.mjs'), 'rb').read()
    if sha(bridge) != json.load(open(os.path.join(src, 'bridge-pin.json')))['sha256']: sys.exit('bridge_hash_mismatch')
    wfile(os.path.join(stage, 'server.mjs'), bridge)
    wfile(os.path.join(stage, 'endpoints.json'), ep_bytes)
    wfile(os.path.join(stage, 'bridge-pin.json'), json.dumps({'path': os.path.join(final, 'server.mjs'), 'sha256': sha(bridge)}).encode())
    wfile(os.path.join(stage, 'provenance.json'), json.dumps({'generation': gen, 'sourceCommit': full, 'sourceBranch': 'main', 'serverSha256': sha(bridge),
        'dailyrecordServer': {'path': new_path, 'sha256': sha(bundle), 'replaces': olds[0]}, 'priorGeneration': old_gen, 'accountReads': 0, 'academyWrites': 0}).encode())
    nb = old_bytes.replace(('/' + old_gen + '/').encode(), ('/' + gen + '/').encode()).replace(('"' + old_gen + '"').encode(), ('"' + gen + '"').encode())
    if nb.count(old['endpoints']['sha256'].encode()) != 1: sys.exit('pointer_rewrite_unqualified')
    nb = nb.replace(old['endpoints']['sha256'].encode(), sha(ep_bytes).encode())
    if json.loads(nb)['generation'] != gen or nb.count(gen.encode()) != (5 if 'surface' in old else 4): sys.exit('pointer_rewrite_unqualified')
    wfile(os.path.join(stage, 'release.json'), nb)
    os.rename(stage, final)
    print('staged', gen, 'from', full[:12], 'dailyrecord', sha(bundle)[:12], 'at', new_path)
def mcp_tools(cmd, env=None):
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    msgs = [{'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2025-06-18', 'capabilities': {}, 'clientInfo': {'name': 'install-readback', 'version': '1'}}},
            {'jsonrpc': '2.0', 'method': 'notifications/initialized'}, {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'}]
    p.stdin.write(''.join(json.dumps(m) + '\n' for m in msgs).encode()); p.stdin.flush()
    names = None
    for line in p.stdout:
        try: m = json.loads(line)
        except ValueError: continue
        if m.get('id') == 2: names = sorted(t['name'] for t in m['result']['tools']); break
    p.stdin.close(); p.terminate(); err = p.stderr.read().decode()[-200:]; p.wait(timeout=30)
    if names is None: sys.exit('readback_failed: ' + err.replace('\n', ' '))
    return names
if '--probe' in sys.argv:
    for g in (old_gen, gen):
        e = json.load(open(os.path.join(rel, g, 'endpoints.json')))['dailyrecord']
        n = mcp_tools([e['command'], *e['args']])
        print('probe dailyrecord of', g, 'tools', len(n), 'names_sha', sha('\n'.join(n).encode())[:12])
if '--activate' in sys.argv and old_gen != gen:
    keep = os.path.join('/home/palantirkc/work/edu-mcp-tier-parity/private', 'pointer.before-' + gen + '.' + os.path.basename(base) + '.json')
    if not os.path.exists(keep): wfile(keep, old_bytes)
    nb = open(os.path.join(final, 'release.json'), 'rb').read()
    t = os.path.join(base, '.release.json.' + str(os.getpid())); wfile(t, nb); os.rename(t, pointer_path)
    print('activated', gen, 'previous', old_gen, 'kept', keep)
    n = mcp_tools(['/usr/bin/env', 'ACADEMY_MCP_INSTALL_DIR=' + base, '/usr/local/bin/bun', '--no-env-file', os.path.join(base, 'launcher.mjs')])
    print('pointer', json.load(open(pointer_path))['generation'], 'tools', len(n), 'names_sha', sha('\n'.join(n).encode())[:12])
