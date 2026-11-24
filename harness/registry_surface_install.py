#!/usr/bin/env python3
# Adds tools that an installed endpoint lists to the release registry and pins a surface file, as a new generation.
# Usage: python3 -I harness/registry_surface_install.py <base> <generation> <surface.json> <endpoint> <tool>[,<tool>...]|- [--refresh] [--activate]
# --refresh: every registry entry of <endpoint> takes the endpoint's listed definition (input schema, description), keeping its registry name;
# an entry the endpoint does not list (a bridge composition) is kept unchanged and printed.
# --compositions <entries.json>: replace the registry entries of those names (from harness/composition_entries.ts).
# Stage (new paths only): copy routing, endpoints, server.mjs and bridge-pin of the active generation (checked by sha256);
# start <endpoint> exactly as endpoints.json does and take each named tool's listed definition into a registry entry
# {endpoint, upstreamName, tool}; refuse a name already in the registry; check that every surface tool is a registry tool
# or academy_runtime_identity; write releases/<generation>. --activate: keep the old pointer under private/, swap the
# pointer by rename, read tools/list back through the launcher. Restore: harness/loop_install.py --restore <kept> --base <base> [--host].
import hashlib, json, os, subprocess, sys
a = [x for i, x in enumerate(sys.argv[1:], 1) if not x.startswith('--') and sys.argv[i - 1] != '--compositions']
base, gen, surface_path, endpoint, names = a[0], a[1], a[2], a[3], a[4].split(',')
KEEP = '/home/palantirkc/work/edu-mcp-tier-parity/private'
sha = lambda b: hashlib.sha256(b).hexdigest()
def wfile(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as f: f.write(data); f.flush(); os.fsync(f.fileno())
def tools_list(cmd):
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    msgs = [{'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2025-06-18', 'capabilities': {}, 'clientInfo': {'name': 'registry-install', 'version': '1'}}},
            {'jsonrpc': '2.0', 'method': 'notifications/initialized'}, {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'}]
    p.stdin.write(''.join(json.dumps(m) + '\n' for m in msgs).encode()); p.stdin.flush()
    tools = None
    for line in p.stdout:
        try: m = json.loads(line)
        except ValueError: continue
        if m.get('id') == 2: tools = m['result']['tools']; break
    p.stdin.close(); p.terminate(); err = p.stderr.read().decode()[-200:]; p.wait(timeout=30)
    if tools is None: sys.exit('tools_list_failed: ' + err.replace('\n', ' '))
    return tools
pointer_path = os.path.join(base, 'release.json')
old_bytes = open(pointer_path, 'rb').read(); old = json.loads(old_bytes); old_gen = old['generation']
rel = os.path.join(base, 'releases'); src = os.path.join(rel, old_gen); final = os.path.join(rel, gen)
if os.path.exists(final): sys.exit('generation_exists')
copies = {}
for k in ('routing', 'registry', 'endpoints'):
    if old[k]['path'] != os.path.join(src, k + '.json'): sys.exit('pointer_path_outside_base@' + k)
    b = open(old[k]['path'], 'rb').read()
    if sha(b) != old[k]['sha256']: sys.exit('copy_hash_mismatch@' + k)
    copies[k] = b
bridge = open(os.path.join(src, 'server.mjs'), 'rb').read()
if sha(bridge) != json.load(open(os.path.join(src, 'bridge-pin.json')))['sha256']: sys.exit('bridge_hash_mismatch')
registry = json.loads(copies['registry']); have = {r['tool']['name'] for r in registry}
spec = json.loads(copies['endpoints'])[endpoint]
listed = {t['name']: t for t in tools_list([spec['command'], *spec['args']])}
names = [n for n in names if n != '-']
refreshed = []
if '--refresh' in sys.argv:
    for r in registry:
        if r['endpoint'] != endpoint: continue
        if r['upstreamName'] in ('academy_dailyrecord_get', 'academy_endpoint_identity', 'academy_runtime_identity'): continue  # bridge-composed, as --discover skips them
        if r['upstreamName'] not in listed: print('kept_not_listed', r['tool']['name']); continue
        t = dict(listed[r['upstreamName']]); t['name'] = r['tool']['name']
        if t != r['tool']: r['tool'] = t; refreshed.append(t['name'])
if '--compositions' in sys.argv:
    comp = json.load(open(sys.argv[sys.argv.index('--compositions') + 1], encoding='utf-8'))
    for e in comp:
        i = next((i for i, r in enumerate(registry) if r['tool']['name'] == e['tool']['name']), None)
        if i is None or registry[i]['endpoint'] != e['endpoint'] or registry[i]['upstreamName'] != e['upstreamName']: sys.exit('composition_entry_unmatched@' + e['tool']['name'])
        if registry[i] != e: registry[i] = e; refreshed.append(e['tool']['name'])
for n in names:
    if n in have: sys.exit('tool_already_registered@' + n)
    if n not in listed: sys.exit('tool_not_listed_by_endpoint@' + n)
    registry.append({'endpoint': endpoint, 'upstreamName': n, 'tool': listed[n]})
surface_bytes = open(surface_path, 'rb').read(); surface = json.loads(surface_bytes)
known = {r['tool']['name'] for r in registry} | {'academy_runtime_identity'}
unknown = [t['name'] for t in surface['tools'] if t['name'] not in known]
if unknown: sys.exit('surface_names_unknown_tool@' + ','.join(unknown))
reg_bytes = json.dumps(registry, indent=2, ensure_ascii=False).encode() + b'\n'
stage = os.path.join(rel, '.' + gen + '.staging'); os.mkdir(stage, 0o700)
files = {'routing.json': copies['routing'], 'registry.json': reg_bytes, 'endpoints.json': copies['endpoints'], 'surface.json': surface_bytes, 'server.mjs': bridge,
         'bridge-pin.json': json.dumps({'path': os.path.join(final, 'server.mjs'), 'sha256': sha(bridge)}).encode()}
for f, b in files.items(): wfile(os.path.join(stage, f), b)
pointer = {'schemaVersion': old['schemaVersion'], 'generation': gen}
for k, f in (('routing', 'routing.json'), ('registry', 'registry.json'), ('endpoints', 'endpoints.json'), ('surface', 'surface.json')):
    pointer[k] = {'path': os.path.join(final, f), 'sha256': sha(files[f])}
nb = json.dumps(pointer).encode()
wfile(os.path.join(stage, 'release.json'), nb)
wfile(os.path.join(stage, 'provenance.json'), json.dumps({'generation': gen, 'priorGeneration': old_gen, 'registryAdded': names, 'registryRefreshed': refreshed, 'fromEndpoint': endpoint,
      'surface': os.path.abspath(surface_path), 'surfaceSha256': sha(surface_bytes), 'accountReads': 0, 'academyWrites': 0}).encode())
os.rename(stage, final)
print('staged', gen, 'registry', len(registry), 'added', names, 'refreshed', len(refreshed), 'surface', sha(surface_bytes)[:12])
if '--activate' in sys.argv:
    keep = os.path.join(KEEP, 'pointer.before-' + gen + '.' + os.path.basename(base) + '.json')
    if not os.path.exists(keep): wfile(keep, old_bytes)
    t = os.path.join(base, '.release.json.' + str(os.getpid())); wfile(t, nb); os.rename(t, pointer_path)
    print('activated', gen, 'previous', old_gen, 'kept', keep)
    tl = tools_list(['/usr/bin/env', 'ACADEMY_MCP_INSTALL_DIR=' + base, '/usr/local/bin/bun', '--no-env-file', os.path.join(base, 'launcher.mjs')])
    ns = sorted(t['name'] for t in tl)
    print('pointer', json.load(open(pointer_path))['generation'], 'tools', len(ns), 'bytes', len(json.dumps({'tools': tl}, ensure_ascii=False, separators=(',', ':')).encode()), 'names_sha', sha('\n'.join(ns).encode())[:12])
