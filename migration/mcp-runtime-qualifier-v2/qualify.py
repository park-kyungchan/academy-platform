#!/usr/bin/env python3
"""Read-only, original-derived S11 captured-boundary comparator (stdlib only)."""
import argparse
import datetime
import hashlib
import json
import math
import pathlib
import stat
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
V1 = REPO / 'migration/mcp-runtime-reference-v1'
BASE = '807db88af175b4dc60b843584e2cc62101e1ac31'
MANIFEST_SHA256 = '4b829f786e07f91c803bc162eeb8a6b05dbe6cfa5ff6e479c1693c8fb2626831'
FILE_LIMIT = 64 * 1024**2
RAW_LIMIT = 12 * 1024**2


class Refusal(ValueError):
    """A bounded comparison refusal, without payload disclosure."""


def refuse(code, where):
    raise Refusal(f'{code}@{where}:1')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(root, relative):
    """Do not follow candidate symlinks, special files, or embedded locators."""
    p = pathlib.PurePosixPath(relative)
    if p.is_absolute() or '..' in p.parts:
        refuse('unsafe_relative_path', relative)
    current = pathlib.Path(root)
    for part in p.parts:
        current = current / part
        if current.is_symlink():
            refuse('symlink_capture', relative)
    try:
        s = current.stat()
        if not stat.S_ISREG(s.st_mode) or s.st_size > FILE_LIMIT:
            refuse('capture_type_or_size', relative)
        with current.open('rb') as f:
            data = f.read(FILE_LIMIT + 1)
        if len(data) > FILE_LIMIT:
            refuse('capture_grew_past_limit', relative)
        if current.name.endswith('.bin') and len(data) > RAW_LIMIT:
            refuse('raw_channel_limit', relative)
        return data
    except OSError:
        refuse('missing_or_unreadable_capture', relative)


def pairs(items):
    result = {}
    for key, value in items:
        if key in result:
            raise Refusal('duplicate_json_key@json:1')
        result[key] = value
    return result


def invalid_constant(_):
    raise Refusal('nonfinite_json@json:1')


def finite_float(token):
    value = float(token)
    if not math.isfinite(value):
        invalid_constant(token)
    return value


def parse(data):
    try:
        return json.loads(data, object_pairs_hook=pairs, parse_constant=invalid_constant,
                          parse_float=finite_float)
    except (ValueError, UnicodeError, RecursionError) as e:
        if isinstance(e, Refusal):
            raise
        raise Refusal('invalid_json@json:1') from None


def same(expected, actual):
    """Exact types (including bool/int/float), omissions and array order."""
    if type(expected) is not type(actual):
        return False
    if isinstance(expected, dict):
        return expected.keys() == actual.keys() and all(
            same(value, actual[key]) for key, value in expected.items())
    if isinstance(expected, list):
        return len(expected) == len(actual) and all(map(
            lambda ab: same(*ab), zip(expected, actual)))
    return expected == actual


def require_same(expected, actual, where):
    if not same(expected, actual):
        refuse('literal_mismatch', where)


class LexicalJSON:
    """Keep scalar tokens and whitespace; only object member order is immaterial.

    In particular a content.text string is ONE literal token, never recursively
    parsed as JSON. Duplicate keys are refused before constructing this tree.
    """

    def __init__(self, data):
        parse(data)
        self.s = data.decode('utf-8')
        self.i = 0
        self.decoder = json.JSONDecoder()

    def whitespace(self):
        begin = self.i
        while self.i < len(self.s) and self.s[self.i] in ' \t\r\n':
            self.i += 1
        return self.s[begin:self.i]

    def scalar(self):
        begin = self.i
        value, self.i = self.decoder.raw_decode(self.s, self.i)
        return value, self.s[begin:self.i]

    def value(self):
        if self.s[self.i] not in '{[':
            return ('scalar', self.scalar()[1])
        kind = self.s[self.i]
        self.i += 1
        end = '}' if kind == '{' else ']'
        items = []
        prefix = self.whitespace()
        if self.s[self.i] == end:
            self.i += 1
            return (kind, prefix, ())
        while True:
            if kind == '{':
                key, rawkey = self.scalar()
                before_colon = self.whitespace()
                self.i += 1  # colon; the strict parser already validated syntax
                after_colon = self.whitespace()
                value = self.value()
                suffix = self.whitespace()
                items.append((key, rawkey, prefix, before_colon, after_colon, value, suffix))
            else:
                value = self.value()
                suffix = self.whitespace()
                items.append((prefix, value, suffix))
            char = self.s[self.i]
            self.i += 1
            if char == end:
                break
            prefix = self.whitespace()
        if kind == '{':
            items.sort(key=lambda item: item[0])
        return (kind, tuple(items))

    def tree(self):
        prefix = self.whitespace()
        result = self.value()
        return (prefix, result, self.whitespace())


def wire_equal(expected, actual, where):
    """NDJSON wire frames, not stream.read chunk boundaries."""
    left, right = expected.split(b'\n'), actual.split(b'\n')
    if len(left) != len(right):
        refuse('frame_count_or_truncation', where)
    for i, (a, b) in enumerate(zip(left, right)):
        location = f'{where}/frame-{i}'
        try:
            x = LexicalJSON(a).tree()
        except Refusal:
            # Original diagnostic/non-JSON line: literal bytes, no suppression.
            if a != b:
                refuse('non_json_wire_mismatch', location)
        else:
            try:
                y = LexicalJSON(b).tree()
            except Refusal:
                refuse('invalid_candidate_frame', location)
            if x != y:
                refuse('wire_token_mismatch', location)


def files(root):
    """Bound traversal and prohibit unbound files/directories and symlink trees."""
    root = pathlib.Path(root)
    if root.is_symlink() or not root.is_dir():
        refuse('capture_root_type', 'candidate')
    found = set()
    pending = [root]
    visits = 0
    while pending:
        directory = pending.pop()
        for p in directory.iterdir():
            visits += 1
            if visits > 2048:
                refuse('capture_entry_limit', 'candidate')
            relative = p.relative_to(root).as_posix()
            if p.is_symlink():
                refuse('symlink_capture', relative)
            if p.is_dir():
                pending.append(p)
            elif p.is_file():
                found.add(relative)
            else:
                refuse('special_capture', relative)
    return found


def utc(value):
    if type(value) is not str:
        refuse('timestamp_type', 'START.json')
    try:
        t = datetime.datetime.fromisoformat(value.replace('Z', '+00:00'))
        if t.tzinfo is None or t.utcoffset() != datetime.timedelta(0):
            raise ValueError()
        return t
    except ValueError:
        refuse('timestamp_format', 'START.json')


class Qualifier:
    def __init__(self):
        data = read(HERE, 'BASELINE-MANIFEST.json')
        if sha(data) != MANIFEST_SHA256:
            refuse('baseline_manifest_changed', 'BASELINE-MANIFEST.json')
        self.manifest = parse(data)
        self.policy = parse(read(HERE, 'POLICY.json'))
        if sha(read(HERE, 'POLICY.json')) != self.manifest['policySha256']:
            refuse('policy_changed', 'POLICY.json')
        self.verify_originals()

    def verify_originals(self):
        for path, identity in self.manifest['originalArtifacts'].items():
            data = read(REPO, path)
            if len(data) != identity['bytes'] or sha(data) != identity['sha256']:
                refuse('original_artifact_changed', path)
        for path, identity in self.manifest['sourcePins'].items():
            data = read(REPO, path)
            if sha(data) != identity['sha256']:
                refuse('original_source_changed', path)
        # Only these original before/after snapshots are read outside the repo.
        # Never traverse package-root locators or read installed configuration.
        for mode, entry in self.manifest['modes'].items():
            root = pathlib.Path(entry['nativeRoot'])
            for path, identity in entry['nativeSnapshots'].items():
                if sha(read(root, path)) != identity['sha256']:
                    refuse('original_provenance_changed', mode + '/' + path)

    def expected(self, mode, relative):
        entry = self.manifest['modes'][mode]
        if relative in entry['nativeSnapshots']:
            return read(pathlib.Path(entry['nativeRoot']), relative)
        return read(V1 / 'frozen' / mode, relative)

    def result(self, mode, label, root, expected, actual):
        location = f'commands/{label}/RESULT.json'
        if type(actual) is not dict:
            refuse('result_object_required', location)
        a = dict(actual)
        b = dict(expected)
        duration = a.pop('elapsedSeconds', None)
        b.pop('elapsedSeconds')
        # The ONLY admitted variable atom: observer command duration, never
        # payload elapsedMs, source freshness, UUIDs, process identity or clocks.
        rule = self.policy['captureDurationRoles'][mode][label]
        if (type(duration) is not float or not math.isfinite(duration)
                or not rule['minimumSeconds'] <= duration <= rule['maximumSeconds']):
            refuse('capture_duration_type_or_bound', location)
        channels = a.pop('channels', None)
        expected_channels = b.pop('channels')
        if type(channels) is not dict or channels.keys() != expected_channels.keys():
            refuse('channel_bindings_missing', location)
        for name, binding in channels.items():
            if type(binding) is not dict or binding.keys() != {'path', 'sha256'}:
                refuse('channel_binding_schema', location)
            raw = read(root, f'commands/{label}/{name}.bin')
            if binding['path'] != expected_channels[name]['path'] or binding['sha256'] != sha(raw):
                refuse('channel_binding_mismatch', location)
        require_same(b, a, location)

    def compare(self, mode, root):
        """Candidate contributes no expectations, policy, oracle or source pins."""
        entry = self.manifest['modes'][mode]
        root = pathlib.Path(root)
        wanted = set(entry['candidateFiles'])
        actual_files = files(root)
        if actual_files != wanted:
            refuse('missing_or_unbound_capture_files', mode)
        observations = parse(read(root, 'observations.json'))
        expected_observations = parse(self.expected(mode, 'observations.json'))
        require_same(expected_observations, observations, 'observations.json')
        for relative in sorted(wanted - {'observations.json'}):
            original = self.expected(mode, relative)
            candidate = read(root, relative)
            if relative.startswith('commands/') and relative.endswith('/RESULT.json'):
                self.result(mode, relative.split('/')[1], root, parse(original), parse(candidate))
            elif relative.endswith('/stdout.bin'):
                wire_equal(original, candidate, relative)
            elif relative.endswith('.json'):
                require_same(parse(original), parse(candidate), relative)
            elif original != candidate:
                refuse('literal_bytes_mismatch', relative)
        before = parse(read(root, 'PINS-before.json'))
        after = parse(read(root, 'PINS-after.json'))
        require_same(before, after, 'PINS-after.json')
        # Observation binding is original-derived, not inferred by first-seen
        # IDs or by searching the candidate for some convenient response.
        for binding in entry['responseBindings']:
            lines = read(root, f"commands/{binding['command']}/stdout.bin").split(b'\n')
            frame = parse(lines[binding['frame']])
            observation = observations[binding['observation']]
            require_same(observation['response'], frame, 'raw_observation_binding')
            request_lines = read(root, f"commands/{binding['command']}/stdin.bin").split(b'\n')
            request = parse(request_lines[binding['requestFrame']])
            require_same(request['id'], frame['id'], 'request_response_id')
            if 'name' in observation:
                require_same(request['params']['name'], observation['name'], 'request_routing_name')
            if 'receipt' in binding:
                receipt = parse(read(root, binding['receipt']))
                result = frame['result']
                occurrence = result.get('structuredContent', {}).get('occurrence') or result.get(
                    '_meta', {}).get('academy/bridgeAudit', {}).get('occurrence')
                require_same(binding['occurrence'], occurrence, 'response_occurrence_role')
                require_same(occurrence, receipt['occurrence'], 'receipt_occurrence_role')
                if not pathlib.PurePosixPath(binding['receipt']).name.startswith(occurrence + '-'):
                    refuse('receipt_filename_role', binding['receipt'])
        # Missing inner EXIT, JS taps and v1 freeze misbindings stay visible.
        # A semantic match NEVER upgrades a limited recorded boundary to parity.
        return {'mode': mode, 'comparison': 'MATCH', 'qualification': 'LIMITED',
                'fullParity': False, 'graphAdmission': False,
                'responseBindingsVerified': len(entry['responseBindings']),
                'limits': entry['limits'],
                'policyRefusals': self.policy['unsupportedFreshReplay']}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--mode')
    p.add_argument('--candidate', type=pathlib.Path)
    p.add_argument('--verify-baseline', action='store_true')
    a = p.parse_args()
    try:
        q = Qualifier()
        if a.verify_baseline:
            print(json.dumps({'baselineIntegrity': True, 'base': BASE,
                              'modes': list(q.manifest['modes']),
                              'qualification': 'NOT_EVALUATED'}, indent=2))
            return 0
        if a.mode not in q.manifest['modes'] or a.candidate is None:
            p.error('--mode <pinned mode> and --candidate <complete capture directory> required')
        report = q.compare(a.mode, a.candidate)
        print(json.dumps(report, indent=2))
        return 2  # Explicitly LIMITED, never acceptance or success-by-label.
    except (Refusal, OSError, KeyError, IndexError, TypeError, RecursionError):
        e = sys.exc_info()[1]
        code = str(e) if isinstance(e, Refusal) else 'invalid_capture_shape@candidate:1'
        print(json.dumps({'comparison': 'REFUSED', 'qualification': 'UNQUALIFIED',
                          'fullParity': False, 'graphAdmission': False, 'code': code}))
        return 1


if __name__ == '__main__':
    sys.exit(main())
