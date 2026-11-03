#!/usr/bin/env python3
"""Bounded unchanged-original recorder. Importing this module starts no process.

Execution is a separately released task phase. --preflight reads pins only.
This is not a controller, service launcher, Rust gate or independent attestation.
"""
import argparse
import base64
import ctypes
import json
import math
import os
from pathlib import Path
import resource
import selectors
import signal
import subprocess
import sys
import time

import qualify as q

HERE = Path(__file__).resolve().parent
NOTES = Path('/home/palantirkc/work/ws-replan/preparation/slices/S11/qualifier-v3/native')
DEPENDENCIES = Path('/home/palantirkc/projects/education-platform/modules/academy-workbench/tools/academy-mcp')
MCP = 'modules/academy-workbench/tools/academy-mcp'


def stamp():
    m0 = time.monotonic_ns()
    wall = time.time_ns()
    m1 = time.monotonic_ns()
    return {'wallNs': wall, 'monoBeforeNs': m0, 'monoAfterNs': m1,
            'wallResolutionNs': math.ceil(time.get_clock_info('time').resolution * 1e9)}


def recorder_deadlines(start_ns):
    return (start_ns + q.POLICY['limits']['workNs'],
            start_ns + q.POLICY['limits']['recorderHardNs'])


def arm_deadline(deadline_ns):
    remaining = deadline_ns - time.monotonic_ns()
    if remaining <= 0:
        q.uncertain('recorder_internal_deadline', 'record.py')
    signal.setitimer(signal.ITIMER_REAL, remaining / 1e9)


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with path.open('x', encoding='utf8') as f:
        json.dump(value, f, ensure_ascii=False, indent=2, allow_nan=False)
        f.write('\n')
    path.chmod(0o600)


def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if isinstance(data, str):
        data = data.encode()
    if len(data) > q.FILE_LIMIT:
        q.refuse('file_limit', path.name)
    path.write_bytes(data)
    path.chmod(0o600)


def setup_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + '\n').encode()


def identity(path, bun=False):
    """Root's sole >64MiB exception: exact Bun, 1MiB streaming, stable inode."""
    import hashlib
    path = Path(path)
    s = path.stat()
    if bun and str(path) == '/usr/local/bin/bun' and s.st_size != 88979072:
        q.refuse('exact_bun_exception_size_changed', str(path))
    maximum = 96 * 1024**2 if bun and str(path) == '/usr/local/bin/bun' else q.FILE_LIMIT
    if s.st_size > maximum:
        q.refuse('pin_read_limit', str(path))
    digest = hashlib.sha256()
    total = 0
    with path.open('rb') as f:
        while chunk := f.read(1024**2):
            total += len(chunk)
            if total > maximum:
                q.refuse('pin_read_grew', str(path))
            digest.update(chunk)
    t = path.stat()
    if (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns) != (t.st_dev, t.st_ino, t.st_size, t.st_mtime_ns):
        q.refuse('pin_read_changed', str(path))
    return {'sha256': digest.hexdigest(), 'bytes': total}


def preflight(source, dependencies):
    """Only declared public source/dependency/tool files; no version execution."""
    pins = q.parse((HERE / 'ORIGINAL-PINS.json').read_bytes())
    if dependencies != DEPENDENCIES or dependencies.resolve() != dependencies:
        q.refuse('dependency_root_literal', 'preflight')
    found = {'sources': {}, 'packages': {}, 'tools': {}}
    for relative, expected in pins['sources'].items():
        data = q.read(source, relative)
        actual = {'sha256': q.sha(data), 'bytes': len(data)}
        q.require_same(expected, actual, relative)
        found['sources'][relative] = actual
    for package, expected in pins['packages'].items():
        # Paths are immutable original pins, never paths supplied by a capture.
        folder = Path(package)
        if not folder.is_relative_to(DEPENDENCIES.parents[4]) and not folder.is_relative_to(DEPENDENCIES):
            q.refuse('package_pin_outside_existing_repository', 'preflight')
        actual_files = {}
        for p in sorted(folder.rglob('*')):
            if 'node_modules' in p.relative_to(folder).parts:
                continue
            if p.is_symlink():
                q.refuse('package_symlink_unqualified', 'preflight')
            if p.is_file():
                actual_files[p.relative_to(folder).as_posix()] = identity(p)
        q.require_same(expected['files'], actual_files, 'dependency-files')
        manifest = q.parse(q.read(folder, 'package.json'))
        q.require_same(expected['name'], manifest['name'], 'package-name')
        q.require_same(expected['version'], manifest['version'], 'package-version')
        found['packages'][package] = {**expected, 'files': actual_files}
    for name, expected in pins['tools'].items():
        actual = identity(name, bun=True)
        q.require_same({k: expected[k] for k in ['sha256', 'bytes']}, actual, name)
        s = Path(name).stat()
        q.require_same(expected['realpath'], str(Path(name).resolve()), name)
        q.require_same(expected['uid'], s.st_uid, name)
        q.require_same(expected['gid'], s.st_gid, name)
        q.require_same(expected['mode'], oct(s.st_mode & 0o7777), name)
        found['tools'][name] = actual
    return found


def members(group):
    result = []
    for p in Path('/proc').iterdir():
        if not p.name.isdigit():
            continue
        try:
            fields = (p / 'stat').read_text().rsplit(')', 1)[1].split()
            if int(fields[2]) == group:
                result.append({'pid': int(p.name), 'ppid': int(fields[1]), 'pgid': int(fields[2]), 'state': fields[0]})
        except (OSError, ValueError):
            pass
    return result


class Child:
    def __init__(self, argv, root, env, role, mode_deadline, registry=None):
        self.root, self.role = root, role
        self.dir = root / 'commands' / role
        self.dir.mkdir(parents=True, mode=0o700)
        self.deadline = mode_deadline - q.POLICY['limits']['cleanupReserveNs']
        self.hard_deadline = mode_deadline
        self.frames, self.requests, self.actions, self.lifecycle, self.proc = [], [], [], [], {}
        self.buf = b''
        self.total = 0
        self.closed = False
        self.raw = {}
        self.sel = selectors.DefaultSelector()
        self.started = stamp()
        # -v and a 1MiB string limit retain complete selected exec argv; native
        # read/write tracing is not used as an invented independent inner pipe.
        self.argv = ['/usr/bin/strace', '-ff', '-ttt', '-T', '-v', '-s', '1048576',
                     '-e', 'trace=process,file,network,desc', '-o', str(self.dir / 'syscalls'), *argv]
        def limits():
            resource.setrlimit(resource.RLIMIT_CPU, (90, 90))
            resource.setrlimit(resource.RLIMIT_FSIZE, (q.FILE_LIMIT, q.FILE_LIMIT))
            resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        self.p = subprocess.Popen(self.argv, cwd=root, env=env, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  start_new_session=True, preexec_fn=limits)
        self.spawned = stamp()
        self.pgid = os.getpgid(self.p.pid)
        if registry is not None:
            registry.append(self)
        self.start = {'argv': self.argv, 'originalArgv': argv, 'pid': self.p.pid,
                      'pgid': self.pgid, 'cwd': str(root), 'environment': env,
                      'prelaunch': self.started, 'spawnReturned': self.spawned,
                      'limits': q.POLICY['limits']}
        save(self.dir / 'START.json', self.start)
        self.raw = {n: (self.dir / (n + '.bin')).open('xb') for n in ['stdin', 'stdout', 'stderr']}
        for name in ['stdout', 'stderr']:
            self.sel.register(getattr(self.p, name), selectors.EVENT_READ, name)
        os.set_blocking(self.p.stdin.fileno(), False)
        self.observe_processes()

    def observe_processes(self):
        for member in members(self.pgid):
            pid = member['pid']
            try:
                p = Path('/proc') / str(pid)
                data = {**member, 'nativeArgv': (p / 'cmdline').read_bytes().decode().split('\0')[:-1],
                        'cwd': os.readlink(p / 'cwd'), 'uid': p.stat().st_uid,
                        'fds': {str(fd): os.readlink(p / 'fd' / str(fd)) for fd in [0, 1, 2]}, 'sample': stamp()}
                key = str(pid) + ':' + q.sha(q.js_bytes(data['nativeArgv']))
                self.proc.setdefault(key, data)
            except (OSError, UnicodeError):
                continue

    def send(self, value):
        data = value if isinstance(value, bytes) else q.js_bytes(value) + b'\n'
        before = stamp()
        offset = self.raw['stdin'].tell()
        self.actions.append({'op': 'send_attempt', 'offset': offset, 'bytes': len(data), 'sha256': q.sha(data), 'before': before})
        sent = 0
        while sent < len(data):
            if time.monotonic_ns() > self.deadline:
                raise TimeoutError('stdin_write_deadline')
            try:
                count = os.write(self.p.stdin.fileno(), data[sent:])
            except BlockingIOError:
                self.pump(.01)
                continue
            self.raw['stdin'].write(data[sent:sent + count])
            self.raw['stdin'].flush()
            sent += count
        after = stamp()
        self.actions.append({'op': 'send', 'offset': self.raw['stdin'].tell() - len(data),
                             'bytes': len(data), 'sha256': q.sha(data), 'before': before, 'after': after,
                             'jsonRequest': isinstance(value, dict) and 'id' in value})
        if isinstance(value, dict) and 'id' in value:
            self.requests.append({'ordinal': len(self.requests), 'request': value,
                                  'requestSendBracket': {'before': before, 'after': after}})

    def pump(self, wait=.02, cleanup=False):
        now = time.monotonic_ns()
        if now > (self.hard_deadline if cleanup else self.deadline):
            raise TimeoutError('mode_deadline')
        self.observe_processes()
        for key, _ in self.sel.select(wait):
            before = stamp()
            chunk = os.read(key.fileobj.fileno(), 65536)
            after = stamp()
            if not chunk:
                self.sel.unregister(key.fileobj)
                continue
            self.total += len(chunk)
            if self.total > q.RAW_LIMIT:
                raise RuntimeError('combined_output_limit')
            self.raw[key.data].write(chunk)
            self.raw[key.data].flush()
            self.actions.append({'op': 'receive', 'channel': key.data, 'bytes': len(chunk),
                                 'sha256': q.sha(chunk), 'before': before, 'after': after})
            if key.data == 'stdout':
                self.buf += chunk
                while b'\n' in self.buf:
                    line, self.buf = self.buf.split(b'\n', 1)
                    frame = {'index': len(self.frames), 'bytes': (line + b'\n').hex(),
                             'responseReceiveBracket': {'before': before, 'after': after}}
                    try:
                        frame['value'] = q.parse(line)
                    except q.Refusal:
                        frame['nonJSON'] = True
                    self.frames.append(frame)
        if self.p.poll() is not None and not any(x.get('kind') == 'exit_observed' for x in self.lifecycle):
            self.lifecycle.append({'kind': 'exit_observed', 'returncode': self.p.returncode, 'sample': stamp()})

    def response(self, id, timeout=8):
        end = min(time.monotonic_ns() + int(timeout * 1e9), self.deadline)
        while time.monotonic_ns() < end:
            found = [f for f in self.frames if isinstance(f.get('value'), dict) and q.same(f['value'].get('id'), id)]
            if found:
                if len(found) != 1:
                    q.refuse('duplicate_response_id', self.role)
                return found[0]['value']
            self.pump()
            if self.p.poll() is not None and not self.sel.get_map():
                break
        raise TimeoutError('rpc_response_deadline')

    def rpc(self, request):
        self.send(request)
        return self.response(request['id'])

    def init(self, stimulus):
        self.rpc(stimulus['initialize'])
        self.send(stimulus['initialized'])

    def wait_exit(self, timeout=15):
        until = min(time.monotonic_ns() + int(timeout * 1e9), self.deadline)
        while self.p.poll() is None and time.monotonic_ns() < until:
            self.pump()
        if self.p.poll() is None:
            raise TimeoutError('process_exit_deadline')

    def finish(self, terminate=False):
        if self.closed:
            return
        failures = []
        try:
            try:
                self.p.stdin.close()
            except BrokenPipeError:
                pass
            if not terminate:
                end = min(time.monotonic_ns() + 1_000_000_000, self.deadline)
                while self.p.poll() is None and time.monotonic_ns() < end:
                    self.pump(.01, cleanup=True)
            before = members(self.pgid)
            self.lifecycle.append({'kind': 'before_cleanup', 'members': before, 'sample': stamp()})
            if before:
                self.lifecycle.append({'kind': 'signal', 'signal': 'SIGTERM', 'pgid': self.pgid, 'sample': stamp()})
                try:
                    os.killpg(self.pgid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                until = min(time.monotonic_ns() + 500_000_000, self.hard_deadline)
                while self.p.poll() is None and time.monotonic_ns() < until:
                    self.pump(.01, cleanup=True)
                if any(x['state'] != 'Z' for x in members(self.pgid)):
                    self.lifecycle.append({'kind': 'signal', 'signal': 'SIGKILL', 'pgid': self.pgid, 'sample': stamp()})
                    try:
                        os.killpg(self.pgid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            self.p.wait(timeout=2)
            self.lifecycle.append({'kind': 'wait', 'returncode': self.p.returncode, 'sample': stamp()})
            while self.sel.get_map():
                self.pump(.01, cleanup=True)
            while time.monotonic_ns() < self.hard_deadline:
                try:
                    pid, status = os.waitpid(-self.pgid, os.WNOHANG)
                    if pid:
                        self.lifecycle.append({'kind': 'reap', 'pid': pid, 'status': status, 'sample': stamp()})
                        continue
                    if not members(self.pgid):
                        break
                    time.sleep(.005)
                except ChildProcessError:
                    break
        except BaseException as e:
            failures.append(type(e).__name__)
            # This fallback owns only the recorded session group. Retain late cleanup.
            try:
                os.killpg(self.pgid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            self.p.wait(timeout=2)
        finally:
            for f in self.raw.values():
                f.close()
            ended = stamp()
            remaining = members(self.pgid)
            save(self.dir / 'CAPTURE.json', {'requests': self.requests, 'frames': self.frames,
                                          'trailingHex': self.buf.hex(), 'actions': self.actions,
                                          'processSnapshots': list(self.proc.values()), 'lifecycle': self.lifecycle})
            save(self.dir / 'RESULT.json', {'returncode': self.p.returncode, 'ended': ended,
                                         'elapsedNs': ended['monoAfterNs'] - self.started['monoBeforeNs'],
                                         'terminateRequested': terminate, 'membersAfterCleanup': remaining,
                                         'cleanupFailures': failures,
                                         'withinModeDeadline': ended['monoAfterNs'] <= self.hard_deadline,
                                         'channels': {n: {'role': 'aggregate_os_pipe' if n != 'stdin' else 'recorder_input_pipe',
                                                          **identity(self.dir / (n + '.bin'))} for n in self.raw}})
            self.closed = True
        if failures or remaining or ended['monoAfterNs'] > self.hard_deadline:
            raise RuntimeError('cleanup_unqualified')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    parser.add_argument('--dependency-root', type=Path, default=DEPENDENCIES)
    parser.add_argument('--mode', choices=q.POLICY['modes'], required=True)
    parser.add_argument('--run', choices=['A', 'B'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--preflight', action='store_true', help='read/hash only; no subprocess, original import or version probe')
    x = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CPU, (90, 90))
    resource.setrlimit(resource.RLIMIT_FSIZE, (q.FILE_LIMIT, q.FILE_LIMIT))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    started = stamp()
    work_deadline, deadline = recorder_deadlines(started['monoBeforeNs'])
    source = x.source_root
    if source != source.resolve() or source != HERE.parents[1]:
        q.refuse('assigned_source_root', 'record')
    root = x.output
    if not root.is_absolute() or root != root.resolve() or not root.is_relative_to(NOTES) or root == NOTES:
        q.refuse('assigned_output_root', 'record')
    root.mkdir(parents=True, exist_ok=False, mode=0o700)
    children, setup, observations = [], [], []
    def timeout(_signum, _frame):
        raise TimeoutError('mode_cleanup_reserve')
    old_alarm = signal.signal(signal.SIGALRM, timeout)
    arm_deadline(work_deadline)
    try:
        before = preflight(source, x.dependency_root)
        save(root / 'PINS-before.json', before)
        observer = {p.name: identity(p) for p in HERE.iterdir() if p.is_file()}
        save(root / 'OBSERVER-PINS.json', observer)
        stimulus = q.parse((HERE / 'STIMULUS.json').read_bytes())
        save(root / 'RUN.json', {'schema': 's11.original-capture/3', 'mode': x.mode, 'run': x.run,
                                'root': str(root), 'sourceRoot': str(source), 'dependencyRoot': str(x.dependency_root),
                                'uid': os.getuid(), 'started': started, 'clockResolution': {
                                    n: time.get_clock_info(n).resolution for n in ['time', 'monotonic']},
                                'stimulusSha256': observer['STIMULUS.json']['sha256'],
                                'preflightOnly': x.preflight, 'claims': q.POLICY['claims']})
        if x.preflight:
            save(root / 'PINS-after.json', preflight(source, x.dependency_root))
            save(root / 'PREFLIGHT.json', {'pinsMatch': True, 'runtimeExecuted': False, 'ended': stamp()})
            return 0
        # No original is launched above this line. User/STATE release is external.
        if ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) != 0:
            q.refuse('subreaper_unavailable', 'record')
        mirror, observer_root = root / 'source', root / 'observer-source'
        observer_root.mkdir(mode=0o700)
        for n in ['record.py', 'preload.ts']:
            put(observer_root / n, (HERE / n).read_bytes())
        source_map = {}
        for relative, pin in before['sources'].items():
            data = q.read(source, relative)
            q.require_same(pin['sha256'], q.sha(data), relative)
            put(mirror / relative, data)
            source_map[relative] = {**pin, 'executedPath': str(mirror / relative)}
        (mirror / 'node_modules').symlink_to(x.dependency_root / 'node_modules', target_is_directory=True)
        save(root / 'SOURCE-MIRROR.json', source_map)
        fixture = mirror / 'fictional-fault-fixture.ts'
        put(fixture, q.read(source, stimulus['fixture']))
        src, tests = mirror / MCP / 'src', mirror / MCP / 'tests'
        preload = observer_root / 'preload.ts'
        bun = '/usr/local/bin/bun'
        env = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'LANG': 'C.UTF-8', 'TZ': 'UTC', 'TMPDIR': str(root),
               'BUN_RUNTIME_TRANSPILER_CACHE_PATH': '0', 'S11_SOURCE_ROOT': str(mirror), 'S11_MODE_ROOT': str(root),
               'S11_DEPENDENCY_ROOT': str(x.dependency_root), 'NODE_PATH': str(x.dependency_root / 'node_modules'),
               'ACADEMY_MCP_INSTALL_DIR': str(root)}
        def args(entry, *extra):
            return [bun, '--no-install', '--no-env-file', '--preload', str(preload), str(entry), *map(str, extra)]
        def launch(argv, role):
            return Child(argv, root, dict(env), role, deadline, children)
        def configure(path, value):
            relative = q.safe_relative(path)
            data = setup_bytes(value)
            put(root / relative, data)
            setup.append({'path': relative, 'bytesHex': data.hex(), 'sample': stamp()})
        def pin(path):
            return {'path': str(path), 'sha256': identity(path)['sha256']}
        def pointer(action):
            generation, version = action['generation'], action['version']
            extra = {}
            if version == 2:
                extra = {'bridge': pin(src / action['bridge']), 'endpointIdentities':
                         {'dailyrecord': endpoint_identity} if action['endpointIdentities'] == 'identity' else action['endpointIdentities']}
            value = {'schemaVersion': f'academy.mcp-release/{version}', 'generation': generation,
                     **{k: pin(root / (k + '.json')) for k in ['routing', 'endpoints', 'registry']}, **extra}
            configure(generation + '.pointer.json', value)
            os.replace(root / (generation + '.pointer.json'), root / 'release.json')
            setup.append({'op': 'replace', 'from': generation + '.pointer.json', 'path': 'release.json', 'sample': stamp()})
        version = launch([bun, '--version'], 'version-bun')
        version.wait_exit(); version.finish()
        q.require_same(b'1.4.3\n', q.read(version.dir, 'stdout.bin'), 'bun-version')
        q.require_same(0, version.p.returncode, 'bun-version')
        configure('admission.json', stimulus['admission'])
        configure('routing.json', {'admissionPath': str(root / 'admission.json'), 'admissionSha256': pin(root / 'admission.json')['sha256']})
        events = root / 'events.txt'
        endpoints = {'dailyrecord': {'command': bun, 'args': args(fixture, 'dailyrecord')[1:]}
                     if x.mode in ['graph', 'bounds'] else {'command': bun, 'args': args(tests / 'neutral-upstream-fixture.ts', events)[1:]}}
        if x.mode in ['discovery', 'composition']:
            endpoints.update({'saved': {'command': bun, 'args': args(tests / 'neutral-saved-fixture.ts')[1:]},
                              **{n: {'command': bun, 'args': args(tests / 'neutral-paper-header-fixture.ts', events)[1:]} for n in ['paper1', 'paper2']}})
        if x.mode == 'bounds':
            endpoints['dailyrecord'] = {'command': '/usr/bin/flock', 'args': ['--wait', '1', str(root / 'fictional-account.lock'), *args(fixture, 'dailyrecord')]}
        endpoint_identity = None
        if x.mode == 'identity':
            configure('endpoint-identity.json', {'schemaVersion': 'academy.endpoint-identity/1', 'endpoint': 'dailyrecord',
                      'semanticContractVersion': 's11-fictional-artifacts-only', 'launchArgumentsSha256': q.sha(b'[]'),
                      'artifacts': [{'role': 'entry', **pin(src / 'dailyrecord-server.ts')}, {'role': 'source', **pin(src / 'dailyrecord.ts')}]})
            endpoint_identity = pin(root / 'endpoint-identity.json')
            endpoints['dailyrecord'] = {'command': bun, 'args': args(src / 'dailyrecord-server.ts', '--runtime-identity', endpoint_identity['path'], endpoint_identity['sha256'])[1:]}
        for config in endpoints.values():
            config['env'] = dict(env)
        if x.mode == 'inventory':
            # Explicit historical PUBLIC metadata replay. No native pointer/config.
            registry_bytes = q.read(source, stimulus['inventoryReplay'])
            put(root / 'registry.json', registry_bytes)
            setup.append({'path': 'registry.json', 'bytesHex': registry_bytes.hex(), 'sample': stamp(), 'replay': stimulus['inventoryReplay']})
            for entry in q.parse(registry_bytes):
                endpoints.setdefault(entry['endpoint'], endpoints['dailyrecord'])
        configure('endpoints.json', endpoints)
        if x.mode in ['launcher', 'relocation']:
            bridge = root / 'releases/fictional/server.mjs'
            put(bridge, stimulus['launcherAdapter'].replace('{entryJson}', json.dumps(str(src / 'dailyrecord-server.ts'))))
            env['BUN_OPTIONS'] = '--no-install --preload ' + str(preload)
            configure('releases/fictional/bridge-pin.json', pin(bridge))
            configure('release.json', {'schemaVersion': 'academy.mcp-release/1', 'generation': 'fictional'})
            entry = src / 'stable-launcher.ts'
            if x.mode == 'relocation':
                entry = root / 'relocated/launcher.ts'; put(entry, (src / 'stable-launcher.ts').read_bytes())
            c = launch(args(entry), 'valid-launch'); c.init(stimulus); c.rpc(stimulus['toolsList']); c.finish()
            put(bridge, stimulus['launcherTamper'])
            bad = launch(args(entry), 'tampered-bridge'); bad.wait_exit(); bad.finish()
        else:
            if x.mode != 'inventory':
                discovery = launch(args(src / 'neutral-server.ts', '--discover'), 'discover')
                discovery.wait_exit(); discovery.finish()
                q.require_same(0, discovery.p.returncode, 'discover')
                if not (root / 'registry.json').is_file():
                    q.refuse('discovery_registry_missing', 'record')
            c = launch(args(src / 'neutral-server.ts'), 'neutral')
            c.init(stimulus); c.rpc(stimulus['toolsList'])
            if x.mode == 'discovery':
                c.finish(); c = launch(args(src / 'dailyrecord-server.ts'), 'dailyrecord-source'); c.init(stimulus); c.rpc(stimulus['toolsList'])
            for action in stimulus['modes'][x.mode]:
                op = action['op']
                if op == 'rpc':
                    c.rpc({'jsonrpc': '2.0', **{k: v for k, v in action.items() if k != 'op'}})
                elif op == 'write':
                    if 'json' in action:
                        configure(action['path'], action['json'])
                    else:
                        put(root / q.safe_relative(action['path']), action['text'])
                        setup.append({**action, 'sample': stamp()})
                elif op == 'unlink':
                    (root / q.safe_relative(action['path'])).unlink(); setup.append({**action, 'sample': stamp()})
                elif op == 'pointer':
                    pointer(action)
                elif op == 'paper_scopes':
                    routing = q.parse(q.read(root, 'routing.json')); routing['paperScopes'] = stimulus['paperScopes']; configure('routing.json', routing)
                elif op == 'qualified_endpoint':
                    u = launch(args(src / 'dailyrecord-server.ts', '--runtime-identity', endpoint_identity['path'], endpoint_identity['sha256']), 'qualified-endpoint')
                    u.init(stimulus); u.rpc({'jsonrpc': '2.0', **{k: v for k, v in action['request'].items() if k != 'op'}}); u.finish()
                elif op == 'tamper_source':
                    target = src / q.safe_relative(action['path'])
                    old = target.read_bytes(); put(target, old + action['suffix'].encode())
                    setup.append({**action, 'beforeSha256': q.sha(old), 'afterSha256': identity(target)['sha256'], 'sample': stamp()})
                elif op in ['pending', 'hung']:
                    c.send({'jsonrpc': '2.0', 'id': action['id'], 'method': 'tools/call', 'params': action['params']})
                    until = min(time.monotonic_ns() + 5_000_000_000, c.deadline)
                    while time.monotonic_ns() < until:
                        c.pump(.01)
                        p = root / 'fault-events.ndjson'
                        if p.exists() and action['event'] in p.read_text():
                            observations.append({'kind': action['event'], 'id': action['id'], 'sample': stamp()}); break
                    else:
                        q.refuse('handler_event_missing', 'record')
                    if op == 'pending':
                        pointer(action['pointer']); c.response(action['id'])
                    else:
                        try:
                            c.response(action['id'], action['deadlineMs'] / 1000)
                        except TimeoutError:
                            observations.append({'kind': 'hung_deadline', 'id': action['id'], 'outcome': 'unknown', 'automaticRetryAllowed': False, 'sample': stamp()})
                        else:
                            q.refuse('hung_unexpected_response', 'record')
                        c.finish(True)
                        lock = launch(['/usr/bin/flock', '--nonblock', str(root / 'fictional-account.lock'), '/usr/bin/true'], 'lock-release-readback')
                        lock.wait_exit(); lock.finish()
                elif op == 'raw':
                    c.send(bytes.fromhex(action['hex']))
                elif op == 'response':
                    c.response(action['id'])
                elif op == 'oversize':
                    c.send(bytes([action['byte']]) * action['count'] + bytes.fromhex(action['suffixHex']))
                    until = time.monotonic_ns() + 100_000_000
                    while time.monotonic_ns() < until:
                        c.pump(.01)
                    c.finish(True)
                else:
                    q.refuse('unknown_frozen_action', 'record')
            c.finish()
        save(root / 'SETUP.json', setup)
        save(root / 'OBSERVATIONS.json', observations)
        after = preflight(source, x.dependency_root)
        save(root / 'PINS-after.json', after)
        q.require_same(before, after, 'pins-before-after')
        ended = stamp()
        if ended['monoAfterNs'] > deadline:
            q.refuse('mode_budget_overrun', 'record')
        save(root / 'COMPLETE.json', {'mode': x.mode, 'run': x.run, 'ended': ended,
                                    'elapsedNs': ended['monoAfterNs'] - started['monoBeforeNs'],
                                    'commands': [c.role for c in children], 'claims': q.POLICY['claims'],
                                    'qualification': 'instrumented_unchanged_original_fictional_only'})
    except BaseException as e:
        if not (root / 'FAILED.json').exists():
            save(root / 'FAILED.json', {'code': q.safe_error(e, 'record.py'), 'at': stamp()})
        raise
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        def hard_timeout(_signum, _frame):
            q.uncertain('recorder_internal_deadline', 'record.py')
        signal.signal(signal.SIGALRM, hard_timeout)
        try:
            # The 85s deadline remains armed through reap, drain and manifest.
            # The outer 90s supervisor independently reads every child PGID.
            arm_deadline(deadline)
            cleanup_errors = []
            for c in children:
                if not c.closed:
                    try:
                        c.finish(True)
                    except BaseException as e:
                        cleanup_errors.append(q.safe_error(e, 'record.py'))
            if cleanup_errors:
                save(root / 'CLEANUP-FAILED.json', cleanup_errors)
            # Bind retained bytes, not self-attestation. Root custody required.
            manifest = {}
            for p in sorted(root.rglob('*')):
                if time.monotonic_ns() >= deadline:
                    q.uncertain('recorder_internal_deadline', 'record.py')
                if p.is_file() and not p.is_symlink():
                    relative = p.relative_to(root).as_posix()
                    if '/node_modules/' not in relative:
                        manifest[relative] = identity(p)
            save(root / 'MANIFEST.json', manifest)
            if time.monotonic_ns() >= deadline:
                q.uncertain('recorder_internal_deadline', 'record.py')
            if cleanup_errors:
                q.uncertain('recorder_cleanup_failed', 'record.py')
        finally:
            signal.setitimer(signal.ITIMER_REAL, 0)
            signal.signal(signal.SIGALRM, old_alarm)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except BaseException as error:
        if isinstance(error, SystemExit):
            raise
        print(q.safe_error(error, 'record.py'), file=sys.stderr)
        sys.exit(2)
