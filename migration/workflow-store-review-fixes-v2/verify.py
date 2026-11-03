#!/usr/bin/env python3
"""New original-backed regressions; never freeze/change qualified old oracles.

Freeze invokes only original.ts. Compare invokes only actual Rust via typed ports.
Full results/digests are compared verbatim; there is no result normalization.
"""
import sys
sys.dont_write_bytecode = True
import argparse, ctypes, hashlib, json, os, pathlib, subprocess, time
HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'workflow-store-rust-v1'))
from fixture import Peer, PortError, ATTR, TARGET, put, js, files
sys.path.insert(0, str(HERE.parent / 'workflow-store-reference-v1'))
from process_io import capture

class RegressionPeer(Peer):
    def __init__(self, root, case):
        super().__init__(root)
        self.case, self.calls = case, []

    def port(self, name, value, child):
        c = self.case
        if name == 'read_authoritative':
            self.calls.append({'port': 'read', **value})
            if 'throw' in c['read']:
                raise PortError(c['read']['throw'])
            return c['read']
        if name == 'learning_review':
            self.calls.append({'port': name, **value})
            return c['policy']['blockers']
        if name == 'learning_suggest':
            # Absent optional JS method means no owner callback. The migration
            # RPC's default Null represents that absence, never a fake runtime.
            if 'suggestion' in c['policy']:
                self.calls.append({'port': name, **value})
            return c['policy'].get('suggestion')
        return super().port(name, value, child)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def compare(a, root, cases):
    os.environ['S10_RUST_BINARY'] = str(a.binary.resolve())
    actual = []
    for c in cases:
        directory = root / c['id']
        directory.mkdir(mode=0o700)
        # Peer base only needs fictional source for the inert parent checks.
        put(directory / 'source.json', {'mode': 'normal'})
        peer = RegressionPeer(directory, c)
        if c['held']:
            peer.invoke('store_publish', {'kind': 'observation', 'id': 'dailyrecord-observation:s10-regression',
                'record': {'observation': c['held'], 'scope': c.get('storedScope', c['input']['context']['scope']),
                    'requestRef': c.get('storedRequest', c['input']['context']['currentRequestRef']),
                    'subject': {'displayName': TARGET['studentName']}, 'createdBy': ATTR, 'createdAt': c['now']}})
        # Store open creates no held-review record; compare all journal bytes.
        before = files(directory / 'store')
        try:
            output = {'result': peer.invoke('review', {**c['input'], 'attribution': ATTR}, now=c['now'])}
        except PortError as error:
            output = {'error': str(error)}
        after = files(directory / 'store')
        if before != after:
            raise RuntimeError('held_review_persisted:' + c['id'])
        if (directory / 'events.jsonl').exists():
            raise RuntimeError('unexpected_effect:' + c['id'])
        actual.append({'id': c['id'], **output, 'calls': peer.calls, 'storeUnchanged': True})
    put(root / 'ACTUAL.json', actual)
    expected = json.loads(a.expected.read_text())
    # Native dict/list equality preserves all values and list order, including
    # full rows, nulls, suggestions, hold actions, digests and callback inputs.
    if actual != expected:
        failures = [x['id'] for x, y in zip(actual, expected) if x != y]
        put(root / 'MISMATCH.json', {'cases': failures, 'actualCount': len(actual), 'expectedCount': len(expected)})
        raise RuntimeError('original_rust_regression_mismatch:' + ','.join(failures))
    return {'cases': len(actual), 'exactMatch': True, 'expectedSha256': sha(a.expected),
        'actualSha256': sha(root / 'ACTUAL.json'), 'binarySha256': sha(a.binary)}

def freeze(a, root):
    if a.expected.exists():
        raise RuntimeError('independent_expected_already_frozen')
    env = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'TZ': 'UTC', 'LANG': 'C.UTF-8',
        'S10_EVIDENCE_ROOT': str(root), 'S10_DENIED_LOG': str(root / 'denied.log'),
        'NODE_PATH': '/home/palantirkc/projects/education-platform/modules/academy-workbench/tools/academy-mcp/node_modules'}
    command = ['/usr/local/bin/bun', '--no-env-file', '--preload',
        str(HERE.parent / 'workflow-store-reference-v1/deny.ts'), str(HERE / 'original.ts'),
        str(root / 'original'), str(HERE / 'CASES.json')]
    start = time.monotonic()
    child = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    out, err, timed, overflow = capture(child, 30)
    try:
        os.killpg(child.pid, 0)
        gone = False
    except ProcessLookupError:
        gone = True
    put(root / 'original.channel.json', {'command': command, 'pid': child.pid, 'returncode': child.returncode,
        'durationSeconds': time.monotonic() - start, 'timedOut': timed, 'overflow': overflow,
        'groupGone': gone, 'stdout': out.decode(), 'stderr': err.decode()})
    if child.returncode or err or timed or overflow or not gone:
        raise RuntimeError('original_regression_channel_failure')
    if (root / 'denied.log').exists() and (root / 'denied.log').read_bytes():
        raise RuntimeError('original_denied_effect')
    if not out.endswith(b'\n') or len(out.splitlines()) != 1:
        raise RuntimeError('original_regression_framing')
    result = json.loads(out)
    if len(result) != len(json.loads((HERE / 'CASES.json').read_text())):
        raise RuntimeError('original_case_count')
    with a.expected.open('x') as file:
        file.write(js(result) + '\n')
    a.expected.chmod(0o644)
    return {'cases': len(result), 'expectedSha256': sha(a.expected), 'originalOnly': True}

if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('mode', choices=['freeze', 'compare'])
    p.add_argument('--root', type=pathlib.Path, required=True)
    p.add_argument('--binary', type=pathlib.Path)
    p.add_argument('--expected', type=pathlib.Path, default=HERE / 'EXPECTED.json')
    a = p.parse_args()
    a.root.mkdir(parents=True, mode=0o700, exist_ok=False)
    if ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) != 0:
        raise RuntimeError('s10_subreaper_unavailable')
    result = freeze(a, a.root) if a.mode == 'freeze' else compare(a, a.root, json.loads((HERE / 'CASES.json').read_text()))
    put(a.root / 'RESULT.json', result)
    print(js(result))
