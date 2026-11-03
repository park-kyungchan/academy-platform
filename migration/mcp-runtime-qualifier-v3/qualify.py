#!/usr/bin/env python3
"""Read-only stdlib comparator for paired, instrumented unchanged originals.

Generated fields are accepted only through producer/receipt/request relations.
Root supplies independent custody; a recorder manifest is not attestation.
"""
import argparse
import base64
import datetime
from dataclasses import dataclass
from functools import cmp_to_key
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys

HERE = Path(__file__).resolve().parent
FILE_LIMIT = 64 * 1024**2
RAW_LIMIT = 12 * 1024**2
UUID = re.compile(r'[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z')


class Refusal(ValueError):
    pass


class Unqualified(Refusal):
    pass


def refuse(code, where):
    raise Refusal(f'{code}@{where}:1')


def uncertain(code, where):
    raise Unqualified(f'{code}@{where}:1')


def safe_error(error, where):
    if isinstance(error, Refusal):
        return str(error)
    # Do not expose exception arguments containing argv, config or payloads.
    return f'{type(error).__name__.lower()}@{where}:1'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def pairs(items):
    result = {}
    for k, v in items:
        if k in result:
            refuse('duplicate_json_key', 'json')
        result[k] = v
    return result


def parse(data):
    def invalid(_):
        refuse('nonfinite_json', 'json')
    def floating(token):
        v = float(token)
        if not math.isfinite(v):
            invalid(token)
        return v
    try:
        return json.loads(data, object_pairs_hook=pairs, parse_constant=invalid, parse_float=floating)
    except Refusal:
        raise
    except (ValueError, UnicodeError, RecursionError):
        refuse('invalid_json', 'json')


POLICY = parse((HERE / 'POLICY.json').read_bytes())
STIMULUS = parse((HERE / 'STIMULUS.json').read_bytes())
PINS = parse((HERE / 'ORIGINAL-PINS.json').read_bytes())


def same(a, b):
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(same(v, b[k]) for k, v in a.items())
    if isinstance(a, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    return a == b


def require_same(a, b, where):
    if not same(a, b):
        refuse('literal_mismatch', where)


def safe_relative(relative):
    p = PurePosixPath(relative)
    if not relative or p.is_absolute() or '..' in p.parts or str(p) != relative or '\\' in relative:
        refuse('unsafe_relative_path', 'capture')
    return relative


def read(root, relative):
    current = Path(root)
    if current.is_symlink() or current.resolve() != current:
        refuse('noncanonical_capture_root', 'capture')
    for component in PurePosixPath(safe_relative(relative)).parts:
        current /= component
        if current.is_symlink():
            refuse('symlink_capture', relative)
    try:
        before = current.stat()
        if not stat.S_ISREG(before.st_mode) or before.st_size > FILE_LIMIT:
            refuse('file_type_or_limit', relative)
        data = current.read_bytes()
        after = current.stat()
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
            refuse('read_changed', relative)
        if len(data) > FILE_LIMIT or (relative.endswith('.bin') and len(data) > RAW_LIMIT):
            refuse('channel_or_file_limit', relative)
        return data
    except OSError:
        uncertain('capture_missing', relative)


def js_bytes(value):
    """Finite source JSON.stringify subset, preserving insertion order.

    No float guesses: only safe integers, booleans, null, well-formed Unicode,
    arrays and objects without integer-index keys. Unsupported input withholds
    qualification. This is a source preimage recipe, not wire canonicalization.
    """
    def validate(v):
        if v is None or type(v) is bool:
            return
        if type(v) is int:
            if abs(v) > 2**53 - 1:
                uncertain('unsupported_js_integer', 'preimage')
            return
        if type(v) is str:
            if any(0xD800 <= ord(c) <= 0xDFFF for c in v):
                uncertain('unsupported_js_surrogate', 'preimage')
            return
        if type(v) is list:
            for x in v:
                validate(x)
            return
        if type(v) is dict:
            for k, x in v.items():
                if type(k) is not str or (k.isdecimal() and str(int(k)) == k and int(k) < 2**32 - 1):
                    uncertain('unsupported_js_index_key', 'preimage')
                validate(k); validate(x)
            return
        uncertain('unsupported_js_type', 'preimage')
    validate(value)
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode()


def pointer(path):
    return '/' + '/'.join(str(x).replace('~', '~0').replace('/', '~1') for x in path)


class LexicalJSON:
    """V2 token policy plus explicitly validated, pointer-bound scalar roles."""
    def __init__(self, data, roles=None, text_roles=None):
        parse(data)
        self.s = data.decode() if isinstance(data, bytes) else data
        self.i = 0
        self.decoder = json.JSONDecoder()
        self.roles, self.text_roles = roles or {}, text_roles or {}
        self.spans = {}

    def whitespace(self):
        begin = self.i
        while self.i < len(self.s) and self.s[self.i] in ' \t\r\n':
            self.i += 1
        return self.s[begin:self.i]

    def scalar(self):
        begin = self.i
        value, self.i = self.decoder.raw_decode(self.s, self.i)
        return value, self.s[begin:self.i], begin, self.i

    def value(self, path=()):
        if self.s[self.i] not in '{[':
            value, token, begin, end = self.scalar()
            self.spans[path] = (begin, end)
            if path in self.roles:
                return ('role', self.roles[path])
            if path in self.text_roles:
                token = mask_text_token(token, value, self.text_roles[path])
            return ('scalar', token)
        kind = self.s[self.i]
        self.i += 1
        end = '}' if kind == '{' else ']'
        items, prefix = [], self.whitespace()
        if self.s[self.i] == end:
            self.i += 1
            return (kind, prefix, ())
        while True:
            if kind == '{':
                key, rawkey, _, _ = self.scalar()
                before_colon = self.whitespace(); self.i += 1
                after_colon = self.whitespace()
                value = self.value(path + (key,)); suffix = self.whitespace()
                items.append((key, rawkey, prefix, before_colon, after_colon, value, suffix))
            else:
                value = self.value(path + (len(items),)); suffix = self.whitespace()
                items.append((prefix, value, suffix))
            char = self.s[self.i]; self.i += 1
            if char == end:
                break
            prefix = self.whitespace()
        if kind == '{':
            items.sort(key=lambda x: x[0])
        return (kind, tuple(items))

    def tree(self):
        prefix = self.whitespace()
        value = self.value()
        suffix = self.whitespace()
        if self.i != len(self.s):
            refuse('trailing_json', 'wire')
        return prefix, value, suffix


def decoded_spans(raw):
    """Map decoded string character boundaries back to original escaped token."""
    positions = [1]
    i = 1
    while i < len(raw) - 1:
        if raw[i] == '\\':
            if raw[i + 1] == 'u':
                high = int(raw[i + 2:i + 6], 16)
                width = 12 if 0xD800 <= high <= 0xDBFF and raw[i + 6:i + 8] == '\\u' else 6
                i += width
            else:
                i += 2
        else:
            i += 1
        positions.append(i)
    return positions


def mask_text_token(raw, text, roles):
    inner = LexicalJSON(text)
    inner.tree()
    positions = decoded_spans(raw)
    if len(positions) != len(text) + 1:
        uncertain('unsupported_text_escape_map', 'wire')
    replacements = []
    for path, role in roles.items():
        if path not in inner.spans:
            refuse('text_role_missing', 'wire')
        lo, hi = inner.spans[path]
        replacements.append((positions[lo], positions[hi], json.dumps('"<' + role + '>"')[1:-1]))
    for lo, hi, token in sorted(replacements, reverse=True):
        raw = raw[:lo] + token + raw[hi:]
    return raw


def wire_equal(a, b, roles_a=None, roles_b=None, texts_a=None, texts_b=None):
    if LexicalJSON(a, roles_a, texts_a).tree() != LexicalJSON(b, roles_b, texts_b).tree():
        refuse('wire_token_mismatch', 'wire')


def utc_ms(value):
    if type(value) is not str or not re.fullmatch(r'\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z', value):
        refuse('timestamp_format', 'roles')
    try:
        dt = datetime.datetime.fromisoformat(value.replace('Z', '+00:00'))
        return int(dt.timestamp()) * 1000 + dt.microsecond // 1000
    except ValueError:
        refuse('timestamp_invalid', 'roles')


@dataclass(frozen=True)
class ClockDomain:
    kind: str
    capture: str
    pid: int
    command: int
    exec_sha256: str


class ClockSample(dict):
    # In-memory provenance only. Never add owner fields to captured/wire JSON.
    def __init__(self, raw, owner):
        super().__init__(raw)
        self.owner = owner


def own_sample(raw, owner):
    if not isinstance(owner, ClockDomain) or owner.kind not in ['recorder', 'bun']:
        uncertain('clock_domain_missing', 'clock')
    if isinstance(raw, ClockSample) and raw.owner != owner:
        refuse('clock_owner_mismatch', 'clock')
    sample = ClockSample(raw, owner)
    if ('monoNs' in sample) != (owner.kind == 'bun'):
        refuse('clock_domain_sample_shape', 'clock')
    sample_bounds(sample)
    return sample


def same_clock(before, after):
    sample_bounds(before); sample_bounds(after)
    if before.owner != after.owner:
        uncertain('clock_owner_mismatch', 'clock')


def wall_cell(sample):
    wall, _, _ = sample_bounds(sample)
    resolution = sample.get('wallResolutionNs', 1000000 if 'monoNs' in sample else 1)
    if type(resolution) is not int or resolution < 1:
        uncertain('clock_resolution_unavailable', 'clock')
    # Date.now truncates to milliseconds. Python's precision is recorded.
    return (wall if 'monoNs' in sample else wall - resolution), wall + resolution


def enclose_cell(cell, lower, upper, selection=False):
    lo, hi = cell
    if hi < lower or lo > upper:
        if selection:
            return False
        refuse('clock_sample_outside_window', 'clock')
    if lo < lower or hi > upper:
        uncertain('clock_window_boundary_ambiguous', 'clock')
    return True


class NativeClock:
    """One verified native exec owns one Bun hrtime domain, within one command."""
    def __init__(self, capture, recorder, command, start):
        pid = start['pid']; native = command['native']
        require_same(native['execs'][pid]['argv'], start['nativeArgv'], 'clock-native-exec')
        require_same(command['start']['pgid'], start['pgid'], 'clock-native-pgid')
        require_same(native['parents'].get(pid, command['start']['pid']), start['ppid'], 'clock-native-parent')
        if pid not in native['terminals']:
            uncertain('clock_native_terminal_missing', 'clock')
        self.recorder, self.command = recorder, command
        self.owner = ClockDomain('bun', capture, pid, command['start']['pid'], sha(js_bytes(start['nativeArgv'])))
        self.exec_cell = native['execs'][pid].get('wallCell')
        self.terminal_cell = native.get('terminalClocks', {}).get(pid)
        if self.exec_cell is None or self.terminal_cell is None:
            uncertain('clock_native_domain_missing', 'clock')
        before, after = command['start']['prelaunch'], command['result']['ended']
        same_clock(before, after)
        if before.owner != recorder or recorder.kind != 'recorder' or recorder.capture != capture:
            refuse('clock_owner_mismatch', 'clock')
        clock_interval(before, after)
        low, high = wall_cell(before)[0], wall_cell(after)[1]
        enclose_cell(self.exec_cell, low, high)
        enclose_cell(self.terminal_cell, low, high)
        if self.exec_cell[1] > self.terminal_cell[0]:
            uncertain('clock_native_lifetime_ambiguous', 'clock')

    def recorder_window(self, before, after):
        same_clock(before, after)
        if before.owner != self.recorder:
            refuse('clock_owner_mismatch', 'clock')
        clock_interval(before, after)
        start, end = self.command['start']['prelaunch'], self.command['result']['ended']
        if sample_bounds(before)[1] < sample_bounds(start)[1] or sample_bounds(after)[2] > sample_bounds(end)[2]:
            refuse('clock_request_outside_command', 'clock')
        return wall_cell(before)[0], wall_cell(after)[1]

    def contains(self, sample, before, after, selection=False):
        sample_bounds(sample)
        if sample.owner != self.owner:
            refuse('clock_owner_mismatch', 'clock')
        cell = wall_cell(sample)
        enclose_cell(cell, self.exec_cell[0], self.terminal_cell[1])
        low, high = self.recorder_window(before, after)
        return enclose_cell(cell, low, high, selection)

    def causal_interval(self, before, after):
        # Explicit recorder -> this native producer bound; never subtract hrtime
        # from Python's OS monotonic clock or another producer's local clock.
        if before.owner != self.recorder or after.owner != self.owner:
            refuse('clock_owner_mismatch', 'clock')
        self.contains(after, self.command['start']['prelaunch'], self.command['result']['ended'])
        self.recorder_window(before, self.command['result']['ended'])
        lo, end = wall_cell(before), wall_cell(after)
        if lo[1] > end[0]:
            uncertain('clock_window_boundary_ambiguous', 'clock')
        return lo[0] // 1000000, (end[1] + 999999) // 1000000


def clock_order(left, right):
    sample_bounds(left); sample_bounds(right)
    if left.owner == right.owner:
        if sample_bounds(left) == sample_bounds(right):
            return 0  # Stable source-call ordinal for one sampled input chunk.
        if sample_bounds(left)[1] <= sample_bounds(right)[1]:
            clock_interval(left, right)
            return -1
        clock_interval(right, left)
        return 1
    if left.owner.capture != right.owner.capture or left.owner.command != right.owner.command:
        refuse('clock_owner_mismatch', 'clock')
    a, b = wall_cell(left), wall_cell(right)
    if a[1] < b[0]:
        return -1
    if b[1] < a[0]:
        return 1
    uncertain('clock_cross_process_order_ambiguous', 'clock')


def sample_bounds(sample):
    if not isinstance(sample, ClockSample) or not isinstance(sample.owner, ClockDomain):
        uncertain('clock_domain_missing', 'clock')
    if 'monoNs' in sample:
        if not all(type(sample.get(k)) is str and re.fullmatch(r'[0-9]+', sample[k]) for k in ['wallNs', 'monoNs', 'monoBeforeNs', 'monoAfterNs']):
            refuse('source_clock_sample_type', 'clock')
        lower, upper = int(sample['monoBeforeNs']), int(sample['monoAfterNs'])
        if lower > upper or int(sample['monoNs']) != upper:
            uncertain('clock_observation_unstable', 'clock')
        return int(sample['wallNs']), lower, upper
    if not all(type(sample.get(k)) is int for k in ['wallNs', 'monoBeforeNs', 'monoAfterNs']):
        refuse('recorder_clock_sample_type', 'clock')
    if sample['monoAfterNs'] < sample['monoBeforeNs']:
        uncertain('clock_observation_unstable', 'clock')
    return int(sample['wallNs']), int(sample['monoBeforeNs']), int(sample['monoAfterNs'])


def clock_interval(before, after, native=None):
    sample_bounds(before); sample_bounds(after)
    if before.owner != after.owner:
        if native is None:
            uncertain('clock_owner_mismatch', 'clock')
        return native.causal_interval(before, after)
    w0, m0, m1 = sample_bounds(before)
    w1, n0, n1 = sample_bounds(after)
    # Preload's Date samples have 1ms resolution; Python time resolution is
    # recorded separately. Outward rounding is precision, not a stale tolerance.
    r0 = before.get('wallResolutionNs', 1000000 if 'monoNs' in before else 1)
    r1 = after.get('wallResolutionNs', 1000000 if 'monoNs' in after else 1)
    if type(r0) is not int or type(r1) is not int or min(r0, r1) < 1:
        uncertain('clock_resolution_unavailable', 'clock')
    if w1 + r1 < w0 - r0 or n0 < m0 or n1 < m1:
        uncertain('clock_observation_unstable', 'clock')
    uncertainty = (m1 - m0) + (n1 - n0) + r0 + r1
    if not (n0 - m1 - uncertainty <= w1 - w0 <= n1 - m0 + uncertainty):
        uncertain('clock_observation_unstable', 'clock')
    return w0 // 1_000_000, (w1 + 999999) // 1_000_000


def age_predicate(age):
    if type(age) is not int:
        refuse('age_type', 'clock')
    return 0 <= age <= 300000


def freshness(observed, before, after, fresh_generation=False, native=None):
    source = utc_ms(observed)
    lower, upper = clock_interval(before, after, native)
    if upper < source or lower - source > 300000:
        refuse('source_time_outside_original_window', 'clock')
    if fresh_generation:
        if source < lower or source > upper:
            refuse('producer_generation_outside_window', 'clock')
        if upper - source <= 300000:
            return
    elif source <= lower and upper - source <= 300000:
        return
    uncertain('clock_interval_ambiguous', 'clock')


def elapsed(value, before, after):
    if type(value) is not int or value < 0:
        refuse('elapsed_type_or_negative', 'duration')
    lo, hi = clock_interval(before, after)
    if value > hi - lo:
        refuse('elapsed_outside_enclosing_span', 'duration')


def uuid(value):
    if type(value) is not str or not UUID.fullmatch(value):
        refuse('occurrence_type_or_uuid', 'roles')


def rooted(value, root, relative):
    safe_relative(relative)
    expected = str(root / relative)
    if type(value) is not str or value != expected:
        refuse('root_component_mismatch', relative)
    return '<R>/' + relative


def validate_terminal(role, returncode, native_statuses, group, hung=False):
    if group:
        uncertain('owned_group_survives', role)
    if not native_statuses:
        uncertain('native_terminal_missing', role)
    if hung:
        if returncode != -15 or not any(s == -15 for s in native_statuses) or any(s not in [0, -15] for s in native_statuses):
            refuse('hung_native_signal_mismatch', role)
    elif role == 'tampered-bridge':
        if returncode != 1 or any(s not in [0, 1] for s in native_statuses):
            refuse('launcher_tamper_terminal_mismatch', role)
    elif returncode != 0 or any(s != 0 for s in native_statuses):
        refuse('native_child_terminal_mismatch', role)


class Atoms:
    """Roles are explicit pointers on one request/producer, never value regex masks."""
    def __init__(self, name):
        self.name = name
        self.roles = {}
        self.uuid_values = set()
        self.receipt_semantics = {}
        self.upstream = []

    def add(self, path, value, role):
        if path in self.roles and self.roles[path] != role:
            refuse('role_collision', self.name)
        self.roles[path] = role

    def occurrence(self, path, value, role):
        uuid(value)
        self.uuid_values.add(value)
        self.add(path, value, role)

    def result_wire(self, frame):
        # Only source-declared dual toolResult text has generated token spans.
        roles = {('result',) + p: r for p, r in self.roles.items()}
        texts = {}
        result = frame.get('result', {})
        data = result.get('structuredContent')
        content = result.get('content', [])
        data_roles = {p[1:]: r for p, r in self.roles.items() if p[:1] == ('structuredContent',)}
        if data_roles:
            if len(content) != 1 or content[0].get('type') != 'text' or not same(parse(content[0]['text']), data):
                refuse('dual_representation_mismatch', self.name)
            texts[('result', 'content', 0, 'text')] = data_roles
        return roles, texts


def stripped_result(result, key):
    out = {k: v for k, v in result.items() if k != '_meta'}
    meta = {k: v for k, v in result.get('_meta', {}).items() if k != key}
    if meta:
        out['_meta'] = meta
    return out


def source_result_preimage(result, producer=None, identity=False):
    """Original ordered producer object, not reordered claimant wire."""
    if identity:
        if producer is None:
            uncertain('ordered_original_preimage_missing', 'identity')
        require_same(result['structuredContent'], producer['structuredContent'], 'independent_identity_producer')
        return js_bytes(producer['structuredContent'])
    base = stripped_result(result, 'academy/bridgeAudit')
    if producer is not None:
        require_same(base, producer, 'independent_result_producer')
        return js_bytes(producer)
    uncertain('ordered_original_preimage_missing', 'result')


def audit_roles(result, request, before, after, atoms, label):
    audit = result.get('_meta', {}).get('academy/audit')
    if audit is None:
        return
    p = ('_meta', 'academy/audit')
    atoms.occurrence(p + ('occurrence',), audit.get('occurrence'), label + ':audit')
    require_same(sha(js_bytes(request.get('params', {}).get('arguments', {}))), audit.get('inputSha256'), 'audit-input')
    elapsed(audit.get('elapsedMs'), before, after)
    atoms.add(p + ('elapsedMs',), audit['elapsedMs'], label + ':audit-elapsed')
    original = stripped_result(result, 'academy/audit')
    require_same(sha(js_bytes(original)), audit.get('resultSha256'), 'audit-result')
    # It is a derived digest, not arbitrary volatile material.
    atoms.add(p + ('inputSha256',), audit['inputSha256'], label + ':input-digest')
    atoms.add(p + ('resultSha256',), audit['resultSha256'], label + ':result-digest')


def bridge_roles(request, result, receipts, before, after, atoms, label, producer=None, pre_dispatch=False, hung=False):
    """Check semantic rules against independently retained successful write events."""
    tool = request['params']['name']
    dispatch = receipts.get('dispatch')
    terminal = receipts.get('result') or receipts.get('failure')
    data = result.get('structuredContent', {}) if result else {}
    meta = result.get('_meta', {}).get('academy/bridgeAudit') if result else None
    if pre_dispatch:
        if receipts:
            refuse('receipt_in_pre_dispatch_release_failure', label)
        atoms.occurrence(('structuredContent', 'occurrence'), data.get('occurrence'), label + ':bridge')
        elapsed(data.get('elapsedMs'), before, after)
        atoms.add(('structuredContent', 'elapsedMs'), data['elapsedMs'], label + ':body-elapsed')
        return
    if not dispatch:
        refuse('dispatch_receipt_missing', label)
    if 'occurrence' in dispatch['body']:
        refuse('dispatch_body_invented_occurrence', label)
    occ = dispatch['occurrence']; uuid(occ)
    expected_tool = 'unrecognized_tool' if tool == 'unknown_fictional_tool' else tool
    require_same(expected_tool, dispatch['body'].get('tool'), 'dispatch-tool')
    require_same(sha(js_bytes(request['params'].get('arguments', {}))), dispatch['body'].get('argumentsSha256'), 'dispatch-arguments')
    require_same(False, dispatch['body'].get('automaticRetryAllowed'), 'dispatch-no-retry')
    if hung:
        if terminal or result is not None:
            refuse('hung_invented_completion', label)
        atoms.uuid_values.add(occ)
        return
    if not terminal:
        refuse('terminal_receipt_missing', label)
    require_same(occ, terminal['occurrence'], 'receipt-filename-occurrence')
    body = terminal['body']
    require_same(occ, body.get('occurrence'), 'receipt-body-occurrence')
    elapsed(body.get('elapsedMs'), before, after)
    if 'result' in receipts:
        if tool == 'academy_runtime_identity':
            if 'occurrence' in data or meta is not None:
                refuse('identity_body_invented_occurrence', label)
            preimage = source_result_preimage(result, producer, identity=True)
        else:
            base = stripped_result(result, 'academy/bridgeAudit')
            preimage = source_result_preimage(result, producer)
            candidate_meta = {'occurrence': occ, 'endpoint': body['endpoint'], 'upstreamName': body['upstreamName'],
                              'generation': body['generation'], 'elapsedMs': body['elapsedMs']}
            # A cap decision is measured on the source recipe. A later elapsed
            # can gain digits; actual returned metadata, when present, supplies it.
            audited = dict(base)
            audited['_meta'] = {**base.get('_meta', {}), 'academy/bridgeAudit': meta or candidate_meta}
            fits = len(js_bytes(audited)) <= 512 * 1024
            if (meta is None) == fits:
                refuse('bridge_metadata_cap_role', label)
            if meta:
                require_same(occ, meta.get('occurrence'), 'bridge-meta-occurrence')
                for key in ['endpoint', 'upstreamName', 'generation']:
                    require_same(body[key], meta.get(key), 'bridge-meta-' + key)
                elapsed(meta.get('elapsedMs'), before, after)
                if body['elapsedMs'] > meta['elapsedMs']:
                    refuse('elapsed_source_order', label)
                atoms.occurrence(('_meta', 'academy/bridgeAudit', 'occurrence'), occ, label + ':bridge')
                atoms.add(('_meta', 'academy/bridgeAudit', 'elapsedMs'), meta['elapsedMs'], label + ':meta-elapsed')
        require_same(sha(preimage), body.get('resultSha256'), 'bridge-result-digest')
    else:
        require_same(occ, data.get('occurrence'), 'blocked-body-occurrence')
        for key in ['code', 'phase', 'endpoint', 'upstreamName']:
            require_same(body.get(key), data.get(key), 'blocked-receipt-' + key)
        elapsed(data.get('elapsedMs'), before, after)
        if body['elapsedMs'] > data['elapsedMs']:
            refuse('elapsed_source_order', label)
        atoms.occurrence(('structuredContent', 'occurrence'), occ, label + ':bridge')
        atoms.add(('structuredContent', 'elapsedMs'), data['elapsedMs'], label + ':body-elapsed')
    atoms.uuid_values.add(occ)


def check_propagated_source(original, candidate, before, after, fresh=True):
    """Anchor stays unchanged even when claimant local hashes are recomputed."""
    require_same(original, candidate, 'independent_original_source')
    freshness(original['observedAt'], before, after, fresh_generation=fresh)


def validate_supervisor(record, recorder_start_ns):
    """Outer0 cannot erase late finalization, a failed drain or another session."""
    limits = POLICY['limits']
    for key in ['startMonoNs', 'endMonoNs', 'elapsedNs', 'outerCapNs', 'childExitObservedMonoNs', 'recorderInternalDeadlineNs']:
        if type(record.get(key)) is not int:
            uncertain('supervisor_measurement_missing', 'SUPERVISOR.json')
    duration = record['endMonoNs'] - record['startMonoNs']
    if duration != record['elapsedNs'] or duration < 0 or duration > limits['wallNs']:
        uncertain('outer_actual_overrun_or_duration', 'SUPERVISOR.json')
    if record['outerCapNs'] != limits['wallNs']:
        uncertain('outer_cap_changed', 'SUPERVISOR.json')
    if record['recorderInternalDeadlineNs'] != recorder_start_ns + limits['recorderHardNs']:
        uncertain('recorder_deadline_mismatch', 'SUPERVISOR.json')
    if not (record['startMonoNs'] <= recorder_start_ns <= record['childExitObservedMonoNs'] <= record['recorderInternalDeadlineNs']):
        uncertain('recorder_actual_overrun', 'SUPERVISOR.json')
    if record['exit'] != 0 or record['failure'] is not None:
        uncertain('outer_command_failed', 'SUPERVISOR.json')
    if not all(record.get(key) is True for key in ['qualified', 'pipeDrainComplete', 'sourceSnapshotComplete', 'sourceUnchanged', 'pidGone', 'groupGone', 'recorderWithinInternalDeadline', 'currentRecorderRun']):
        uncertain('outer_finalization_incomplete', 'SUPERVISOR.json')
    if not isinstance(record.get('childPGIDReadbacks'), list) or any(c.get('groupGone') is not True for c in record['childPGIDReadbacks']):
        uncertain('child_pgid_unreconciled', 'SUPERVISOR.json')


class Capture:
    def __init__(self, root, external_manifest=None, external_supervisor=None):
        self.root = Path(root)
        manifest_raw = read(self.root, 'MANIFEST.json')
        if external_manifest is None:
            uncertain('external_raw_custody_missing', 'MANIFEST.json')
        require_same(external_manifest, sha(manifest_raw), 'external-manifest')
        self.manifest = parse(manifest_raw)
        if len(self.manifest) > POLICY['limits']['captureEntries']:
            refuse('capture_entry_limit', 'MANIFEST.json')
        for relative, expected in self.manifest.items():
            data = read(self.root, relative)
            require_same(expected, {'sha256': sha(data), 'bytes': len(data)}, relative)
        self.run = self.json('RUN.json')
        if external_supervisor is None:
            uncertain('external_supervisor_custody_missing', 'SUPERVISOR.json')
        supervisor_raw = read(self.root, 'SUPERVISOR.json')
        require_same(external_supervisor, sha(supervisor_raw), 'external-supervisor')
        validate_supervisor(parse(supervisor_raw), self.run['started']['monoBeforeNs'])
        self.supervisor = parse(supervisor_raw)
        self.check_tree()
        self.complete = self.json('COMPLETE.json')
        self.recorder = ClockDomain('recorder', str(self.root), self.supervisor['pid'], self.supervisor['pid'], '')
        self.run['started'] = own_sample(self.run['started'], self.recorder)
        self.complete['ended'] = own_sample(self.complete['ended'], self.recorder)
        require_same(str(self.root), self.run['root'], 'run-root')
        require_same(str(HERE.parents[1]), self.run['sourceRoot'], 'assigned-source-root')
        require_same(PINS['dependencyRoot'], self.run['dependencyRoot'], 'assigned-dependency-root')
        if self.root.resolve() != self.root or self.root.is_symlink():
            refuse('run_root_noncanonical', 'RUN.json')
        if self.run.get('preflightOnly') or 'FAILED.json' in self.manifest or 'CLEANUP-FAILED.json' in self.manifest:
            uncertain('capture_incomplete', 'COMPLETE.json')
        self.mode = self.run['mode']
        require_same(self.mode, self.complete['mode'], 'complete-mode')
        require_same(POLICY['claims'], self.complete['claims'], 'claims')
        require_same(sha((HERE / 'STIMULUS.json').read_bytes()), self.run['stimulusSha256'], 'frozen-stimulus')
        for phase in ['before', 'after']:
            pins = self.json('PINS-' + phase + '.json')
            require_same(PINS['sources'], pins['sources'], 'original-source-' + phase)
            require_same(PINS['packages'], pins['packages'], 'original-dependencies-' + phase)
            require_same({p: {k: v[k] for k in ['sha256', 'bytes']} for p, v in PINS['tools'].items()}, pins['tools'], 'tool-pins')
        self.setup = self.json('SETUP.json')
        self.check_source_mirror()
        self.check_setup()
        clock_interval(self.run['started'], self.complete['ended'])
        duration = self.complete['ended']['monoAfterNs'] - self.run['started']['monoBeforeNs']
        require_same(duration, self.complete['elapsedNs'], 'mode-duration')
        if duration > POLICY['limits']['recorderHardNs']:
            uncertain('mode_budget_overrun', 'COMPLETE.json')
        self.commands, self.processes, self.uuid_values = {}, {}, set()
        for role in self.complete['commands']:
            self.command(role)
        readbacks = self.supervisor['childPGIDReadbacks']
        if len(readbacks) != len(self.commands) or {c.get('role') for c in readbacks} != set(self.commands):
            uncertain('child_pgid_readback_coverage', 'SUPERVISOR.json')
        for child in readbacks:
            require_same(self.commands[child['role']]['start']['pgid'], child.get('pgid'), 'supervisor-child-pgid')
        self.bind_processes()

    def check_tree(self):
        found = set()
        pending = [self.root]
        entries = 0
        while pending:
            for p in pending.pop().iterdir():
                entries += 1
                if entries > POLICY['limits']['captureEntries']:
                    refuse('capture_entry_limit', 'capture')
                relative = p.relative_to(self.root).as_posix()
                if p.is_symlink():
                    if relative != 'source/node_modules' or str(p.resolve()) != PINS['dependencyRoot'] + '/node_modules':
                        refuse('unexpected_capture_symlink', relative)
                elif p.is_dir():
                    pending.append(p)
                elif p.is_file():
                    found.add(relative)
                else:
                    refuse('capture_special_file', relative)
        require_same(set(self.manifest) | {'MANIFEST.json', 'SUPERVISOR.json'}, found, 'manifest-file-coverage')

    def check_source_mirror(self):
        mirror = self.json('SOURCE-MIRROR.json')
        require_same(set(PINS['sources']), set(mirror), 'source-mirror-set')
        for relative, pin in PINS['sources'].items():
            rooted(mirror[relative]['executedPath'], self.root, 'source/' + relative)
            require_same(pin['sha256'], mirror[relative]['sha256'], 'source-mirror-original')
            data = self.raw('source/' + relative)
            if self.mode == 'identity' and relative.endswith('/src/dailyrecord.ts'):
                suffix = b'\n// fictional disposable tamper\n'
                if not data.endswith(suffix):
                    refuse('identity_tamper_bytes', relative)
                require_same(pin['sha256'], sha(data[:-len(suffix)]), 'identity-pre-tamper')
            else:
                require_same(pin, {'sha256': sha(data), 'bytes': len(data)}, 'executed-source')
        for n in ['record.py', 'preload.ts']:
            require_same((HERE / n).read_bytes(), self.raw('observer-source/' + n), 'observer-executed-bytes')
        require_same(self.raw('source/' + STIMULUS['fixture']), self.raw('source/fictional-fault-fixture.ts'), 'fictional-fixture-copy')

    def check_setup(self):
        root, mode = self.root, self.mode
        env = expected_environment(self, 'setup')
        prefix = ['/usr/local/bin/bun', '--no-install', '--no-env-file', '--preload', str(root / 'observer-source/preload.ts')]
        src = root / 'source/modules/academy-workbench/tools/academy-mcp/src'
        tests = src.parent / 'tests'
        def command(entry, *extra):
            return {'command': '/usr/local/bin/bun', 'args': prefix[1:] + [str(entry), *map(str, extra)], 'env': dict(env)}
        endpoints = {'dailyrecord': command(root / 'source/fictional-fault-fixture.ts', 'dailyrecord') if mode in ['graph', 'bounds']
                     else command(tests / 'neutral-upstream-fixture.ts', root / 'events.txt')}
        if mode in ['discovery', 'composition']:
            endpoints.update({'saved': command(tests / 'neutral-saved-fixture.ts'),
                              **{n: command(tests / 'neutral-paper-header-fixture.ts', root / 'events.txt') for n in ['paper1', 'paper2']}})
        if mode == 'bounds':
            endpoints['dailyrecord']['command'] = '/usr/bin/flock'
            endpoints['dailyrecord']['args'] = ['--wait', '1', str(root / 'fictional-account.lock'), *prefix, str(root / 'source/fictional-fault-fixture.ts'), 'dailyrecord']
        if mode == 'identity':
            expected_manifest = {'schemaVersion': 'academy.endpoint-identity/1', 'endpoint': 'dailyrecord',
                                 'semanticContractVersion': 's11-fictional-artifacts-only', 'launchArgumentsSha256': sha(b'[]'),
                                 'artifacts': [{'role': role, 'path': str(src / n),
                                                'sha256': PINS['sources']['modules/academy-workbench/tools/academy-mcp/src/' + n]['sha256']}
                                               for role, n in [('entry', 'dailyrecord-server.ts'), ('source', 'dailyrecord.ts')]]}
            require_same(expected_manifest, parse(self.raw('endpoint-identity.json')), 'frozen-endpoint-manifest')
            endpoints['dailyrecord'] = command(src / 'dailyrecord-server.ts', '--runtime-identity', root / 'endpoint-identity.json', sha(self.raw('endpoint-identity.json')))
        if mode == 'inventory':
            require_same(PINS['sources'][STIMULUS['inventoryReplay']]['sha256'], sha(self.raw('registry.json')), 'public-registry-replay')
            for entry in parse(self.raw('registry.json')):
                endpoints.setdefault(entry['endpoint'], endpoints['dailyrecord'])
        require_same(STIMULUS['admission'], parse(self.raw('admission.json')), 'frozen-admission')
        require_same(endpoints, parse(self.raw('endpoints.json')), 'frozen-endpoint-configuration')
        routing_base = {'admissionPath': str(root / 'admission.json'), 'admissionSha256': sha(self.raw('admission.json'))}
        routing_expected = {**routing_base, **({'paperScopes': STIMULUS['paperScopes']} if mode == 'composition' else {})}
        require_same(routing_expected, parse(self.raw('routing.json')), 'frozen-routing')
        declared = STIMULUS['modes'][mode]
        expected_sequence = [('json', 'admission.json'), ('json', 'routing.json')]
        if mode == 'identity':
            expected_sequence.append(('json', 'endpoint-identity.json'))
        if mode == 'inventory':
            expected_sequence.append(('json', 'registry.json'))
        expected_sequence.append(('json', 'endpoints.json'))
        if mode in ['launcher', 'relocation']:
            expected_sequence += [('json', 'releases/fictional/bridge-pin.json'), ('json', 'release.json')]
        for a in declared:
            if a['op'] == 'write':
                expected_sequence.append(('json' if 'json' in a else 'write', a['path']))
            elif a['op'] in ['unlink', 'tamper_source']:
                expected_sequence.append((a['op'], a['path']))
            elif a['op'] == 'paper_scopes':
                expected_sequence.append(('json', 'routing.json'))
            if a['op'] in ['pointer', 'pending']:
                action = a if a['op'] == 'pointer' else a['pointer']
                expected_sequence += [('json', action['generation'] + '.pointer.json'), ('replace', 'release.json')]
        require_same(expected_sequence, [('json' if 'bytesHex' in e else e.get('op'), e['path']) for e in self.setup], 'frozen-control-action-sequence')
        pointer_actions = [a for a in declared if a['op'] == 'pointer'] + [a['pointer'] for a in declared if a['op'] == 'pending']
        for e in self.setup:
            if 'bytesHex' not in e:
                if e.get('op') == 'write':
                    if not any(same({k: v for k, v in e.items() if k != 'sample'}, a) for a in declared if a['op'] == 'write'):
                        refuse('unfrozen_control_write', 'SETUP.json')
                continue
            path = e['path']; raw = bytes.fromhex(e['bytesHex']); value = parse(raw)
            if path == 'admission.json':
                require_same(STIMULUS['admission'], value, 'setup-admission')
            elif path == 'endpoints.json':
                require_same(endpoints, value, 'setup-endpoints')
            elif path == 'routing.json':
                if not same(routing_base, value) and not (mode == 'composition' and same(routing_expected, value)):
                    refuse('unfrozen_routing', 'SETUP.json')
            elif path == 'registry.json':
                require_same(PINS['sources'][STIMULUS['inventoryReplay']]['sha256'], sha(raw), 'setup-public-registry')
            elif path.endswith('.pointer.json'):
                actions = [a for a in pointer_actions if a['generation'] + '.pointer.json' == path]
                if len(actions) != 1:
                    refuse('unfrozen_pointer_generation', path)
                a = actions[0]
                require_same('academy.mcp-release/' + str(a['version']), value['schemaVersion'], 'pointer-version')
                require_same(a['generation'], value['generation'], 'pointer-generation')
                keys = {'schemaVersion', 'generation', 'routing', 'endpoints', 'registry'}
                for key in ['routing', 'endpoints', 'registry']:
                    rooted(value[key]['path'], root, key + '.json')
                    versions = setup_versions(self, key + '.json')
                    if key == 'registry' and not versions:
                        versions = [(parse(self.raw('registry.json')), self.raw('registry.json'))]
                    if value[key]['sha256'] not in [sha(b) for _, b in versions]:
                        refuse('pointer_payload_preimage', key)
                if a['version'] == 2:
                    keys |= {'bridge', 'endpointIdentities'}
                    require_same({'path': str(src / a['bridge']), 'sha256': PINS['sources']['modules/academy-workbench/tools/academy-mcp/src/' + a['bridge']]['sha256']}, value['bridge'], 'pointer-bridge')
                    expected_id = {'dailyrecord': {'path': str(root / 'endpoint-identity.json'), 'sha256': sha(self.raw('endpoint-identity.json'))}} if a['endpointIdentities'] == 'identity' else a['endpointIdentities']
                    require_same(expected_id, value['endpointIdentities'], 'pointer-endpoint-identities')
                require_same(keys, set(value), 'pointer-source-field-set')
            elif path == 'fixture-control.json':
                if not any(a.get('path') == path and same(a.get('json'), value) for a in declared):
                    refuse('unfrozen_fault_control', path)
            elif path == 'endpoint-identity.json' and mode == 'identity':
                require_same(expected_manifest, value, 'setup-endpoint-manifest')
            elif path == 'release.json' and mode in ['launcher', 'relocation']:
                require_same({'schemaVersion': 'academy.mcp-release/1', 'generation': 'fictional'}, value, 'launcher-pointer')
            elif path == 'releases/fictional/bridge-pin.json' and mode in ['launcher', 'relocation']:
                require_same({'path': str(root / 'releases/fictional/server.mjs'), 'sha256': self.entry_pin(str(root / 'releases/fictional/server.mjs'))}, value, 'launcher-adapter-pin')
            else:
                refuse('unfrozen_setup_file', path)

    def raw(self, path):
        if path not in self.manifest:
            uncertain('unbound_capture', path)
        return read(self.root, path)

    def json(self, path):
        return parse(self.raw(path))

    def command(self, role):
        safe_relative(role)
        prefix = 'commands/' + role + '/'
        start, result, cap = (self.json(prefix + n) for n in ['START.json', 'RESULT.json', 'CAPTURE.json'])
        for record, keys in [(start, ['prelaunch', 'spawnReturned']), (result, ['ended'])]:
            for key in keys:
                record[key] = own_sample(record[key], self.recorder)
        for group, bracket in [('requests', 'requestSendBracket'), ('frames', 'responseReceiveBracket')]:
            for item in cap[group]:
                for edge in ['before', 'after']:
                    item[bracket][edge] = own_sample(item[bracket][edge], self.recorder)
                    clock_interval(start['prelaunch'], item[bracket][edge])
                    clock_interval(item[bracket][edge], result['ended'])
                clock_interval(item[bracket]['before'], item[bracket]['after'])
        for item in cap['actions'] + cap['processSnapshots'] + cap['lifecycle']:
            for key in ['before', 'after', 'sample']:
                if key in item:
                    item[key] = own_sample(item[key], self.recorder)
                    clock_interval(start['prelaunch'], item[key]); clock_interval(item[key], result['ended'])
        require_same(expected_command_argv(self, role), start['originalArgv'], 'declared-command-argv')
        require_same(expected_environment(self, role), start['environment'], 'minimal-environment')
        expected_trace = ['/usr/bin/strace', '-ff', '-ttt', '-T', '-v', '-s', '1048576',
                          '-e', 'trace=process,file,network,desc', '-o', str(self.root / prefix / 'syscalls'), *start['originalArgv']]
        require_same(expected_trace, start['argv'], 'tracer-complete-argv')
        require_same(str(self.root), start['cwd'], 'command-cwd')
        require_same(start['pid'], start['pgid'], 'command-pgid')
        clock_interval(start['prelaunch'], result['ended'])
        duration = result['ended']['monoAfterNs'] - start['prelaunch']['monoBeforeNs']
        require_same(duration, result['elapsedNs'], 'command-duration')
        if duration < 0 or duration > POLICY['limits']['recorderHardNs'] or not result['withinModeDeadline'] or result['cleanupFailures']:
            uncertain('command_lifetime_unqualified', role)
        require_same(POLICY['limits'], start['limits'], 'command-limits')
        raw = {n: self.raw(prefix + n + '.bin') for n in ['stdin', 'stdout', 'stderr']}
        if len(raw['stdout']) + len(raw['stderr']) > RAW_LIMIT:
            refuse('combined_top_output_limit', role)
        for n, data in raw.items():
            expected_role = 'recorder_input_pipe' if n == 'stdin' else 'aggregate_os_pipe'
            require_same({'role': expected_role, 'sha256': sha(data), 'bytes': len(data)}, result['channels'][n], 'os-channel-binding')
        require_same(b''.join(expected_stdin(self.mode, role)), raw['stdin'], 'frozen-input-byte-sequence')
        trace = {int(p.rsplit('.', 1)[1]): self.raw(p).decode() for p in self.manifest
                 if p.startswith(prefix + 'syscalls.') and p.rsplit('.', 1)[1].isdigit()}
        graph = native_trace(trace)
        statuses = list(graph['terminals'].values())
        hung = self.mode == 'bounds' and role == 'neutral'
        validate_terminal(role, result['returncode'], statuses, result['membersAfterCleanup'], hung)
        if not any(e['argv'] == start['originalArgv'] for e in graph['execHistory']):
            refuse('complete_exec_argv_missing', role)
        if role == 'lock-release-readback' and not any(e['argv'] == ['/usr/bin/true'] for e in graph['execHistory']):
            uncertain('fictional_flock_true_exec_missing', role)
        if hung:
            kinds = self.json('OBSERVATIONS.json')
            if not any(o.get('kind') == 'hung_deadline' and o.get('outcome') == 'unknown' and o.get('automaticRetryAllowed') is False for o in kinds):
                refuse('hung_unknown_effect_missing', role)
            if not any(x.get('kind') == 'signal' and x.get('signal') == 'SIGTERM' and x.get('pgid') == start['pgid'] for x in cap['lifecycle']):
                refuse('hung_own_sigterm_missing', role)
        # Frame bytes reconstruct the exact stdout channel, including LF framing.
        framed = b''.join(bytes.fromhex(f['bytes']) for f in cap['frames']) + bytes.fromhex(cap['trailingHex'])
        require_same(raw['stdout'], framed, 'raw-frame-binding')
        if cap['trailingHex']:
            refuse('truncated_wire_frame', role)
        for f in cap['frames']:
            if 'value' in f:
                require_same(parse(bytes.fromhex(f['bytes'])), f['value'], 'frame-parsed-binding')
        sends = [a for a in cap['actions'] if a['op'] == 'send']
        for a in sends:
            data = raw['stdin'][a['offset']:a['offset'] + a['bytes']]
            require_same(sha(data), a['sha256'], 'stdin-write-binding')
        if sum(a['bytes'] for a in sends) != len(raw['stdin']):
            refuse('stdin_write_coverage', role)
        request_sends = []
        for a in sends:
            data = raw['stdin'][a['offset']:a['offset'] + a['bytes']]
            try:
                value = parse(data)
            except Refusal:
                continue
            if a.get('jsonRequest') and isinstance(value, dict) and 'id' in value:
                request_sends.append((value, a))
        for req, (value, send) in zip(cap['requests'], request_sends):
            require_same(value, req['request'], 'recorded-request-stdin')
            require_same({'before': send['before'], 'after': send['after']}, req['requestSendBracket'], 'send-before-first-byte')
        if len(cap['requests']) != len(request_sends):
            refuse('request_send_count', role)
        self.commands[role] = {'start': start, 'result': result, 'cap': cap, 'raw': raw, 'native': graph, 'traces': trace}

    def bind_processes(self):
        for path in self.manifest:
            if not re.fullmatch(r'processes/[0-9]+/START.json', path):
                continue
            start = self.json(path); pid = int(path.split('/')[1])
            require_same(pid, start['pid'], 'process-directory-identity')
            matches = [(role, c) for role, c in self.commands.items() if pid in c['native']['execs']]
            if len(matches) != 1:
                refuse('process_exec_binding_ambiguous', path)
            role, command = matches[0]
            native = command['native']
            require_same(native['execs'][pid]['argv'], start['nativeArgv'], 'complete-native-argv')
            require_same(command['start']['pgid'], start['pgid'], 'process-pgid')
            require_same(self.run['uid'], start['uid'], 'process-owner')
            require_same(str(self.root), start['cwd'], 'process-cwd')
            parent = native['parents'].get(pid, command['start']['pid'])
            require_same(parent, start['ppid'], 'native-ancestry')
            if set(start['fds']) != {'0', '1', '2'}:
                uncertain('native_stdio_fd_role_unavailable', path)
            snapshots = [s for s in command['cap']['processSnapshots'] if s['pid'] == pid and s.get('nativeArgv') == start['nativeArgv']]
            if not all(re.fullmatch(r'pipe:\[[0-9]+\]', v) for v in start['fds'].values()):
                declared_socket_stdio(self, command, start, snapshots)
            for snapshot in snapshots:
                require_same(start['fds'], snapshot['fds'], 'independent-native-fd-readback')
                require_same(start['pgid'], snapshot['pgid'], 'independent-native-pgid-readback')
            if pid not in native['terminals']:
                uncertain('process_native_terminal_missing', path)
            if start['execPath'] != '/usr/local/bin/bun':
                refuse('process_exec_path', path)
            entry = start['argv'][1]
            allowed_entry = self.entry_pin(entry)
            eventpath = f'processes/{pid}/events.ndjson'
            events = [parse(line) for line in self.raw(eventpath).splitlines()]
            native_clock = NativeClock(str(self.root), self.recorder, command, start)
            start['sample'] = own_sample(start['sample'], native_clock.owner)
            native_clock.contains(start['sample'], command['start']['prelaunch'], command['result']['ended'])
            previous = start['sample']
            for i, e in enumerate(events, 1):
                require_same(pid, e['pid'], 'event-pid'); require_same(i, e['sequence'], 'event-sequence')
                e['sample'] = own_sample(e['sample'], native_clock.owner)
                clock_interval(start['sample'], e['sample'])
                clock_interval(previous, e['sample'])
                previous = e['sample']
                if 'before' in e:
                    e['before'] = own_sample(e['before'], native_clock.owner)
                    clock_interval(start['sample'], e['before']); clock_interval(e['before'], e['sample'])
                native_clock.contains(e['sample'], command['start']['prelaunch'], command['result']['ended'])
            first = [e for e in events if e['kind'] == 'entry']
            if len(first) != 1:
                refuse('process_entry_observation_missing', path)
            require_same(entry, first[0]['path'], 'entry-path')
            require_same(allowed_entry, first[0]['sha256'], 'entry-source-pin')
            lifetime_before, lifetime_after = command['start']['prelaunch'], command['result']['ended']
            exits = f'processes/{pid}/EXIT.json'
            if exits in self.manifest:
                ex = self.json(exits)
                ex['sample'] = own_sample(ex['sample'], native_clock.owner)
                clock_interval(start['sample'], ex['sample'])
                clock_interval(previous, ex['sample'])
                native_clock.contains(ex['sample'], lifetime_before, lifetime_after)
                freshness(ex['at'], command['start']['prelaunch'], command['result']['ended'], True)
                if native['terminals'][pid] == 0:
                    require_same(0, ex['code'], 'js-native-exit')
            # No empty file manufactured for absent taps. Distinguish no-event.
            channels = {}
            for name, kind in [('stdin', 'stdin_data'), ('stdout', 'stream_write'), ('stderr', 'stream_write')]:
                selected = [e for e in events if e['kind'] == kind and (kind == 'stdin_data' or e['channel'] == name)]
                aggregate = b''.join(event_bytes(e) for e in selected)
                p = f'processes/{pid}/{name}.bin'
                if selected:
                    require_same(aggregate, self.raw(p), 'js-tap-binding')
                    channels[name] = {'scope': 'js_emit_or_write_tap', 'raw': aggregate, 'events': selected}
                else:
                    if p in self.manifest:
                        refuse('manufactured_empty_tap', p)
                    channels[name] = {'scope': 'js_no_event', 'raw': None, 'events': []}
            self.processes[pid] = {'start': start, 'role': role, 'events': events, 'channels': channels, 'entry': entry,
                                   'before': lifetime_before, 'after': lifetime_after, 'clock': native_clock}
        for role, command in self.commands.items():
            if role in ['version-bun', 'lock-release-readback']:
                continue
            direct = [p for p in self.processes.values() if p['role'] == role and p['start']['nativeArgv'] == command['start']['originalArgv']]
            if len(direct) != 1:
                uncertain('direct_original_process_missing', role)
            command['direct'] = direct[0]
            # Native inherited FD identity corroborates aggregate top pipes.
            top = direct[0]['start']['fds']
            for p in self.processes.values():
                if p['role'] == role:
                    p['osScope'] = stdio_scope(p['start']['fds'], top)

    def entry_pin(self, path):
        root = str(self.root) + '/'
        if not path.startswith(root):
            refuse('entry_outside_run_root', 'entry')
        relative = path[len(root):]
        if relative == 'source/fictional-fault-fixture.ts':
            return PINS['sources'][STIMULUS['fixture']]['sha256']
        if relative == 'relocated/launcher.ts':
            return PINS['sources']['modules/academy-workbench/tools/academy-mcp/src/stable-launcher.ts']['sha256']
        if relative == 'releases/fictional/server.mjs':
            entry = str(self.root / 'source/modules/academy-workbench/tools/academy-mcp/src/dailyrecord-server.ts')
            return sha(STIMULUS['launcherAdapter'].replace('{entryJson}', json.dumps(entry)).encode())
        if not relative.startswith('source/') or relative[7:] not in PINS['sources']:
            refuse('entry_not_original_pin', 'entry')
        return PINS['sources'][relative[7:]]['sha256']


def stdio_scope(fds, top):
    # Native topology is role evidence, never an independent child OS capture.
    return {name: ('inherited_shared_command_pipe' if fds[str(fd)] == top[str(fd)]
                   else 'js_emit_or_write_tap') for fd, name in [(1, 'stdout'), (2, 'stderr')]}


def declared_socket_stdio(capture, command, start, snapshots):
    parent_path = 'processes/' + str(start['ppid']) + '/START.json'
    if parent_path not in capture.manifest:
        return declared_flock_stdio(capture, command, start, snapshots)
    parent = capture.json(parent_path)
    source = 'modules/academy-workbench/tools/academy-mcp/src/neutral-server.ts'
    # This exact source declares StdioClientTransport(..., stderr:'pipe').
    # Pinned client2.3.1 dist/stdio.mjs declares stdio:[pipe,pipe,stderr].
    if parent['pid'] != start['ppid'] or parent['argv'][1] != str(capture.root / 'source' / source):
        uncertain('native_stdio_source_role_unsupported', 'stdio')
    require_same(command['native']['execs'][parent['pid']]['argv'], parent['nativeArgv'], 'stdio-parent-exec')
    require_same(command['start']['pgid'], parent['pgid'], 'stdio-parent-pgid')
    require_same(PINS['sources'][source]['sha256'], sha(capture.raw('source/' + source)), 'stdio-source-pin')
    package = PINS['dependencyRoot'] + '/node_modules/@modelcontextprotocol/client'
    dependency = Path(package) / 'dist/stdio.mjs'
    data = dependency.read_bytes()
    require_same(PINS['packages'][package]['files']['dist/stdio.mjs'],
                 {'bytes': len(data), 'sha256': sha(data)}, 'stdio-client-pin')
    # The initial spawn argv (flock -> Bun can retain a PID) must be one of the
    # already validated frozen endpoint declarations; no arbitrary child role.
    endpoints = capture.json('endpoints.json')
    declared = [[e['command'], *e.get('args', [])] for e in endpoints.values()]
    if not any(e['pid'] == start['pid'] and e['argv'] in declared for e in command['native']['execHistory']):
        uncertain('native_stdio_endpoint_role_missing', 'stdio')
    traces = command.get('traces', {})
    if start['ppid'] not in traces or start['pid'] not in traces:
        uncertain('native_stdio_trace_binding_missing', 'stdio')
    return socket_stdio_binding(traces[start['ppid']], traces[start['pid']], parent, start, snapshots)


def declared_flock_stdio(capture, command, start, snapshots):
    """Only bounds' declared native flock intermediary, with actual FD readbacks."""
    if capture.mode != 'bounds':
        uncertain('native_flock_mode_unsupported', 'stdio')
    wrapper_pid = start['ppid']; native = command['native']; traces = command['traces']
    entry = str(capture.root / 'source/fictional-fault-fixture.ts')
    bun = ['/usr/local/bin/bun', '--no-install', '--no-env-file', '--preload',
           str(capture.root / 'observer-source/preload.ts'), entry, 'dailyrecord']
    expected = ['/usr/bin/flock', '--wait', '1', str(capture.root / 'fictional-account.lock'), *bun]
    if wrapper_pid not in native['execs'] or start['pid'] not in native['execs']:
        uncertain('native_flock_exec_missing', 'stdio')
    require_same(bun, start['nativeArgv'], 'flock-child-argv')
    require_same(bun, native['execs'][start['pid']]['argv'], 'flock-child-exec')
    require_same(expected, native['execs'][wrapper_pid]['argv'], 'flock-wrapper-exec')
    declared = [[e['command'], *e.get('args', [])] for e in capture.json('endpoints.json').values()]
    if expected not in declared:
        uncertain('native_flock_endpoint_undeclared', 'stdio')
    if native['parents'].get(start['pid']) != wrapper_pid or wrapper_pid not in native['parents']:
        uncertain('native_flock_ancestry_missing', 'stdio')
    origin_pid = native['parents'][wrapper_pid]
    origin = capture.json(f'processes/{origin_pid}/START.json')
    source = 'modules/academy-workbench/tools/academy-mcp/src/neutral-server.ts'
    require_same(str(capture.root / 'source' / source), origin['argv'][1], 'flock-origin-source')
    require_same(native['execs'][origin_pid]['argv'], origin['nativeArgv'], 'flock-origin-exec')
    require_same(command['start']['pgid'], origin['pgid'], 'flock-origin-pgid')
    require_same(origin['pgid'], start['pgid'], 'flock-child-pgid')
    require_same(PINS['sources'][source], {'bytes': len(capture.raw('source/' + source)), 'sha256': sha(capture.raw('source/' + source))}, 'flock-origin-source-pin')
    package = PINS['dependencyRoot'] + '/node_modules/@modelcontextprotocol/client'
    data = (Path(package) / 'dist/stdio.mjs').read_bytes()
    require_same(PINS['packages'][package]['files']['dist/stdio.mjs'], {'bytes': len(data), 'sha256': sha(data)}, 'flock-client-pin')
    require_same(PINS['sources'][STIMULUS['fixture']], {'bytes': len(capture.raw('source/fictional-fault-fixture.ts')), 'sha256': sha(capture.raw('source/fictional-fault-fixture.ts'))}, 'flock-fixture-pin')
    wrappers = [x for x in command['cap']['processSnapshots'] if x['pid'] == wrapper_pid and x.get('nativeArgv') == expected]
    children = [x for x in snapshots if x['pid'] == start['pid'] and x.get('nativeArgv') == bun]
    if not wrappers or not children or any(pid not in traces for pid in [origin_pid, wrapper_pid, start['pid']]):
        uncertain('native_flock_independent_binding_missing', 'stdio')
    wrapper = wrappers[0]
    for snapshot in wrappers:
        require_same(origin_pid, snapshot['ppid'], 'flock-wrapper-parent')
        require_same(start['pgid'], snapshot['pgid'], 'flock-wrapper-pgid')
        require_same(start['fds'], snapshot['fds'], 'flock-wrapper-fd-inheritance')
    for snapshot in children:
        for key in ['ppid', 'pgid', 'fds']:
            require_same(start[key], snapshot[key], 'flock-child-independent-' + key)
    # Reuse only the existing finite Bun three-socketpair/vfork/dup2 prefix.
    # Wrapper is an actual Python /proc snapshot, never a fabricated JS START.
    wrapper_lines, wrapper_execs = socket_spawn_prefix(traces[origin_pid], traces[wrapper_pid], origin, wrapper, wrappers)
    if len(wrapper_execs) != 1:
        uncertain('native_flock_exec_sequence_unsupported', 'stdio')
    clones = [i for i, line in enumerate(wrapper_lines) if re.search(
        r'clone\(child_stack=NULL, flags=CLONE_CHILD_CLEARTID\|CLONE_CHILD_SETTID\|SIGCHLD, child_tidptr=0x[0-9a-f]+\)\s*= ' + str(start['pid']) + r'(?:\s|$)', line)]
    if len(clones) != 1:
        uncertain('native_flock_clone_binding_missing', 'stdio')
    # Native flock is selected explicitly; only its known lock call may precede clone.
    if not any(re.search(r'flock\([0-9]+, LOCK_EX\)\s*= 0(?:\s|$)', line) for line in wrapper_lines[wrapper_execs[0]:clones[0]]):
        uncertain('native_flock_lock_binding_missing', 'stdio')
    rebound = r'(?:dup2|dup3)\([^,]+, [012](?:,|\))|close\([012]\)|close_range\('
    if any(re.search(rebound, line) for line in wrapper_lines[wrapper_execs[0] + 1:]):
        uncertain('native_flock_stdio_rebound', 'stdio')
    child_lines = traces[start['pid']].splitlines()
    child_execs = [i for i, line in enumerate(child_lines) if 'execve(' in line and re.search(r'\)\s*= 0(?:\s|$)', line)]
    if child_execs != [0]:
        uncertain('native_flock_child_exec_sequence_unsupported', 'stdio')
    for fd in range(3):
        links = [(i, re.search(r'readlink\("/proc/self/fd/' + str(fd) + r'", "(socket:\[[0-9]+\])", [0-9]+\)\s*= ([0-9]+)(?:\s|$)', line)) for i, line in enumerate(child_lines)]
        links = [(i, match) for i, match in links if match]
        if not links:
            uncertain('native_flock_child_inode_missing', 'stdio')
        index, link = links[0]
        require_same(start['fds'][str(fd)], link[1], 'flock-child-inode')
        require_same(len(link[1]), int(link[2]), 'flock-child-readlink-length')
        if any(re.search(rebound, line) for line in child_lines[1:index]):
            uncertain('native_flock_child_stdio_rebound', 'stdio')
    for pid in [wrapper_pid, start['pid']]:
        if pid not in native['terminals']:
            uncertain('native_flock_terminal_missing', 'stdio')


def socket_spawn_prefix(parent_trace, child_trace, parent, child, snapshots):
    """Finite Bun1.4.3 three-pipe AF_UNIX/vfork/dup2 pattern, not an FD emulator."""
    if child['ppid'] != parent['pid'] or child['pgid'] != parent['pgid']:
        uncertain('native_stdio_ancestry_missing', 'stdio')
    fds = child['fds']
    if set(fds) != {'0', '1', '2'} or not all(re.fullmatch(r'socket:\[[0-9]+\]', v) for v in fds.values()):
        uncertain('native_stdio_socket_role_unsupported', 'stdio')
    if len(set(fds.values())) != 3 or set(fds.values()) & set(parent['fds'].values()):
        uncertain('native_stdio_fresh_pipe_role_missing', 'stdio')
    matches = [s for s in snapshots if s['pid'] == child['pid'] and s.get('nativeArgv') == child['nativeArgv']]
    if not matches:
        uncertain('native_stdio_independent_readback_missing', 'stdio')
    for snapshot in matches:
        for key in ['ppid', 'pgid', 'fds']:
            require_same(child[key], snapshot[key], 'stdio-independent-' + key)
    parent_lines = parent_trace.splitlines()
    forks = [i for i, line in enumerate(parent_lines)
             if re.search(r'vfork\(\)\s*= ' + str(child['pid']) + r'(?:\s|$)', line)]
    if len(forks) != 1:
        uncertain('native_stdio_vfork_binding_missing', 'stdio')
    fork = forks[0]
    pairs = [(i, re.search(r'socketpair\(([^,]+), ([^,]+), ([^,]+), \[([0-9]+), ([0-9]+)\]\)\s*= 0(?:\s|$)', line))
             for i, line in enumerate(parent_lines[:fork]) if 'socketpair(' in line]
    pairs = [(i, m) for i, m in pairs if m is not None][-3:]
    if len(pairs) != 3:
        uncertain('native_stdio_socketpair_binding_missing', 'stdio')
    selected = []
    for i, match in pairs:
        family, kind, protocol, first, second = match.groups()
        if family != 'AF_UNIX' or set(kind.split('|')) != {'SOCK_STREAM', 'SOCK_CLOEXEC'} or protocol != '0':
            uncertain('native_stdio_socketpair_family_unsupported', 'stdio')
        selected.append((int(first), int(second)))
    if len({fd for pair in selected for fd in pair}) != 6 or min(fd for pair in selected for fd in pair) < 3:
        uncertain('native_stdio_socketpair_alias', 'stdio')
    # From the first selected creation through vfork, only the observed pair
    # construction and non-destructive fcntl operations are supported. Reuse,
    # close/dup/open/unparsed interrupted creation cannot silently keep a role.
    for line in parent_lines[pairs[0][0]:fork]:
        if re.search(r'socketpair\(.*\)\s*= 0(?:\s|$)', line):
            continue
        if re.search(r'fcntl\([0-9]+, F_(?:GETFL|SETFL),?.*\)\s*= (?:0x[0-9a-f]+|[0-9]+)(?:\s|$)', line):
            continue
        uncertain('native_stdio_parent_fd_sequence_unsupported', 'stdio')
    expected = {0: selected[0][0], 1: selected[1][1], 2: selected[2][1]}
    child_lines = child_trace.splitlines()
    execs = [i for i, line in enumerate(child_lines) if 'execve(' in line and re.search(r'\)\s*= 0(?:\s|$)', line)]
    if not execs:
        uncertain('native_stdio_child_exec_missing', 'stdio')
    prefix = child_lines[:execs[0]]
    observed, operations = {}, []
    for line in prefix:
        dup = re.search(r'dup2\(([0-9]+), ([012])\)\s*= ([012])(?:\s|$)', line)
        if dup:
            source, target, returned = map(int, dup.groups())
            if target in observed or target != returned or expected.get(target) != source:
                uncertain('native_stdio_dup_binding_wrong', 'stdio')
            observed[target] = source
            operations.append(('dup2', source, target))
        elif not re.search(r'close\((?:' + '|'.join(map(str, expected.values())) + r')\)\s*= 0(?:\s|$)', line):
            uncertain('native_stdio_child_fd_sequence_unsupported', 'stdio')
        else:
            operations.append(('close', int(re.search(r'close\(([0-9]+)\)', line)[1])))
    if observed != expected:
        uncertain('native_stdio_dup_binding_missing', 'stdio')
    expected_operations = [op for fd, source in expected.items() for op in [('dup2', source, fd), ('close', source)]]
    if operations != expected_operations:
        uncertain('native_stdio_child_fd_sequence_unsupported', 'stdio')
    return child_lines, execs


def socket_stdio_binding(parent_trace, child_trace, parent, child, snapshots):
    child_lines, execs = socket_spawn_prefix(parent_trace, child_trace, parent, child, snapshots)
    fds = child['fds']
    # Native readlink bytes tie each dup destination to the recorded socket
    # inode; Python /proc readback above independently corroborates that inode.
    for fd in range(3):
        links = [(i, re.search(r'readlink\("/proc/self/fd/' + str(fd) + r'", "(socket:\[[0-9]+\])", [0-9]+\)\s*= ([0-9]+)(?:\s|$)', line))
                 for i, line in enumerate(child_lines) if i > execs[-1]]
        links = [(i, m) for i, m in links if m is not None]
        if not links:
            uncertain('native_stdio_inode_binding_missing', 'stdio')
        index, match = links[0]
        if match[1] != fds[str(fd)] or int(match[2]) != len(match[1]):
            uncertain('native_stdio_inode_binding_wrong', 'stdio')
        for line in child_lines[execs[0] + 1:index]:
            if re.search(r'(?:dup2|dup3)\([^,]+, ' + str(fd) + r'(?:,|\))|close\(' + str(fd) + r'\)|close_range\(', line):
                uncertain('native_stdio_child_fd_rebound', 'stdio')
    return {'0': 'declared_local_ipc_input', '1': 'declared_local_ipc_output', '2': 'declared_local_ipc_diagnostic'}


def event_bytes(event):
    try:
        b = base64.b64decode(event['data'], validate=True)
    except (ValueError, TypeError):
        refuse('event_base64', 'events')
    require_same(len(b), event['bytes'], 'event-bytes')
    require_same(sha(b), event['sha256'], 'event-digest')
    return b


def native_wall_cell(line):
    match = re.match(r'([0-9]+)\.([0-9]{1,9})\s', line)
    if match is None:
        return None
    seconds, fraction = match.groups()
    lower = int(seconds) * 1000000000 + int(fraction.ljust(9, '0'))
    return (lower, lower + 10 ** (9 - len(fraction)))


def native_trace(traces):
    execs, parents, terminals, history, threads, terminal_clocks = {}, {}, {}, [], {}, {}
    for pid, trace in traces.items():
        pending_thread = False
        for line in trace.splitlines():
            if 'execve(' in line and re.search(r'\)\s*=\s*0\s*(?:<[^>]+>)?$', line):
                m = re.search(r'execve\(("(?:\\.|[^"\\])*"), (\[.*?\]), ', line)
                if not m or '...' in m.group(2):
                    uncertain('native_exec_argv_truncated', 'trace')
                try:
                    argv = parse(m.group(2)); executable = parse(m.group(1))
                except Refusal:
                    uncertain('unsupported_native_exec_encoding', 'trace')
                execs[pid] = {'argv': argv, 'executable': executable, 'wallCell': native_wall_cell(line)}
                history.append({'pid': pid, **execs[pid]})
            if any(n in line for n in ['clone(', 'clone3(', 'fork(', 'vfork(', '<... clone resumed>', '<... clone3 resumed>', '<... vfork resumed>']):
                if 'CLONE_THREAD' in line:
                    pending_thread = True
                m = re.search(r'= ([0-9]+)(?:\s+<[^>]+>)?$', line)
                if m:
                    child = int(m.group(1))
                    if pending_thread:
                        threads[child] = pid
                    else:
                        parents[child] = pid
                    pending_thread = False
                elif '<unfinished ...>' not in line:
                    pending_thread = False
            if '+++ exited with ' in line:
                terminal_clocks[pid] = native_wall_cell(line)
                terminals[pid] = int(re.search(r'\+\+\+ exited with ([0-9]+)', line).group(1))
            if '+++ killed by ' in line:
                terminal_clocks[pid] = native_wall_cell(line)
                name = re.search(r'\+\+\+ killed by (SIG[A-Z0-9]+)', line).group(1)
                import signal
                terminals[pid] = -int(getattr(signal, name))
    # A process may exec flock -> Bun or flock -> true under the same PID;
    # retain every successful exec separately as well as final process identity.
    for child, parent in list(parents.items()):
        seen = set()
        while parent in threads:
            if parent in seen:
                refuse('native_thread_parent_cycle', 'trace')
            seen.add(parent); parent = threads[parent]
        parents[child] = parent
    return {'execs': execs, 'execHistory': history, 'parents': parents, 'terminals': terminals, 'terminalClocks': terminal_clocks}


def stream_frames(process, channel):
    record = process['channels'][channel]
    if record['raw'] is None:
        return []
    buf, result = b'', []
    for event in record['events']:
        buf += event_bytes(event)
        while b'\n' in buf:
            raw, buf = buf.split(b'\n', 1)
            try:
                value = parse(raw)
            except Refusal:
                continue  # Literal diagnostics are still compared on top OS raw.
            result.append({'value': value, 'raw': raw + b'\n', 'sample': event['sample']})
    if buf:
        uncertain('inner_tap_partial_frame', 'process')
    return result


def source_calls(process):
    inputs, outputs = stream_frames(process, 'stdin'), stream_frames(process, 'stdout')
    calls = []
    for f in inputs:
        req = f['value']
        if not isinstance(req, dict) or req.get('method') != 'tools/call':
            continue
        replies = [r for r in outputs if isinstance(r['value'], dict) and same(r['value'].get('id'), req.get('id'))]
        if len(replies) != 1:
            # A hung possibly-effectful request legitimately has no reply.
            calls.append({'request': req, 'before': f['sample'], 'after': None, 'result': None, 'raw': None})
        else:
            reply = replies[0]
            clock_interval(f['sample'], reply['sample'])
            calls.append({'request': req, 'before': f['sample'], 'after': reply['sample'], 'result': reply['value'].get('result'), 'raw': reply['raw']})
    return calls


NATIVE_STRING = r'"(?:\\.|[^"\\])*"'
NATIVE_OPEN = re.compile(r'^[0-9]+\.[0-9]+\s+openat\(AT_FDCWD, (' + NATIVE_STRING + r')')


def native_bytes(token):
    """Strict C byte string, not JSON/Unicode normalization or an FD interpreter."""
    if not re.fullmatch(NATIVE_STRING, token) or not token.isascii():
        uncertain('native_string_encoding_unsupported', 'native-write')
    output = bytearray(); i = 1
    escapes = {'n': 10, 'r': 13, 't': 9, 'a': 7, 'b': 8, 'f': 12, 'v': 11, '\\': 92, '"': 34}
    while i < len(token) - 1:
        c = token[i]; i += 1
        if c != '\\':
            if ord(c) < 32:
                uncertain('native_string_encoding_unsupported', 'native-write')
            output.append(ord(c)); continue
        c = token[i]; i += 1
        if c in escapes:
            output.append(escapes[c])
        elif c in '01234567':
            digits = c
            while len(digits) < 3 and i < len(token) - 1 and token[i] in '01234567':
                digits += token[i]; i += 1
            value = int(digits, 8)
            if value > 255:
                uncertain('native_string_encoding_unsupported', 'native-write')
            output.append(value)
        else:
            uncertain('native_string_encoding_unsupported', 'native-write')
    return bytes(output)


def native_operation_cell(line):
    start = native_wall_cell(line)
    duration = re.search(r'<([0-9]+\.[0-9]{1,9})>$', line)
    if start is None or duration is None:
        uncertain('native_write_clock_unavailable', 'native-write')
    span = native_wall_cell(duration[1] + ' ')
    if span[1] > POLICY['limits']['recorderHardNs']:
        uncertain('native_write_duration_unqualified', 'native-write')
    # -ttt and -T each have their own recorded decimal precision. No Bun clock
    # sample is invented; completion uses both outward resolution cells.
    return start[0], start[1] + span[1]


def native_open_paths(trace):
    for i, line in enumerate(trace.splitlines()):
        match = NATIVE_OPEN.match(line)
        if match:
            try:
                path = native_bytes(match[1]).decode('utf8')
            except UnicodeError:
                uncertain('native_path_encoding_unsupported', 'native-write')
            yield i, path, line


def native_file_chains(trace, path, kind):
    """Only contiguous exclusive/write/truncate/close or append/write/close."""
    if kind not in ['receipt', 'tap', 'fault']:
        uncertain('native_write_role_unsupported', 'native-write')
    lines = trace.splitlines(); records = []
    exclusive = kind == 'receipt'
    flags = 'O_WRONLY|O_CREAT|' + ('O_EXCL' if exclusive else 'O_APPEND') + '|O_LARGEFILE|O_CLOEXEC'
    mode = '0666' if kind == 'fault' else '0600'
    for i, selected, line in native_open_paths(trace):
        if selected != path:
            continue
        opening = re.fullmatch(r'[0-9]+\.[0-9]+\s+openat\(AT_FDCWD, (' + NATIVE_STRING +
                              r'), ([A-Z0-9_|]+), (0[0-7]+)\)\s*= ([0-9]+)\s+<[0-9.]+>', line)
        if opening is None or opening[2] != flags or opening[3] != mode:
            uncertain('native_write_open_unsupported', 'native-write')
        fd = opening[4]; count = 4 if exclusive else 3
        if i + count > len(lines):
            uncertain('native_write_chain_incomplete', 'native-write')
        write = re.fullmatch(r'[0-9]+\.[0-9]+\s+write\(' + fd + r', (' + NATIVE_STRING +
                             r'), ([0-9]+)\)\s*= ([0-9]+)\s+<[0-9.]+>', lines[i+1])
        if write is None:
            uncertain('native_write_truncated_or_unsupported', 'native-write')
        raw = native_bytes(write[1])
        if len(raw) != int(write[2]) or len(raw) != int(write[3]):
            uncertain('native_write_partial_or_length', 'native-write')
        if exclusive and not re.fullmatch(r'[0-9]+\.[0-9]+\s+ftruncate\(' + fd + ', ' + str(len(raw)) +
                                         r'\)\s*= 0\s+<[0-9.]+>', lines[i+2]):
            uncertain('native_write_truncate_missing', 'native-write')
        if not re.fullmatch(r'[0-9]+\.[0-9]+\s+close\(' + fd + r'\)\s*= 0\s+<[0-9.]+>', lines[i+count-1]):
            uncertain('native_write_close_missing', 'native-write')
        cells = [native_operation_cell(op) for op in lines[i:i+count]]
        if any(b[0] < a[0] for a, b in zip(cells, cells[1:])):
            uncertain('native_write_clock_unstable', 'native-write')
        records.append({'raw': raw, 'path': path, 'openLine': i+1, 'closeLine': i+count,
                        'wallCell': (cells[0][0], cells[-1][1]), 'scope': 'native_successful_' + kind + '_chain'})
    return records


def native_writer_trace(capture, command, process):
    start = process['start']; pid = start['pid']; native = command['native']
    if pid not in native['execs'] or pid not in native['terminals']:
        uncertain('native_write_owner_unavailable', 'native-write')
    require_same(native['execs'][pid]['argv'], start['nativeArgv'], 'native-write-exec-owner')
    require_same(command['start']['pgid'], start['pgid'], 'native-write-group-owner')
    require_same(native['parents'].get(pid, command['start']['pid']), start['ppid'], 'native-write-parent-owner')
    require_same(capture.entry_pin(process['entry']), capture.entry_pin(start['argv'][1]), 'native-write-source-owner')
    if process['clock'].owner.pid != pid or process['clock'].owner.command != command['start']['pid']:
        uncertain('native_write_clock_owner_wrong', 'native-write')
    trace = command.get('traces', {}).get(pid)
    if trace is None:
        uncertain('native_write_trace_missing', 'native-write')
    return trace


def native_tap_bounds(capture, command, process, request, reply=None):
    trace = native_writer_trace(capture, command, process); pid = process['start']['pid']
    logpath = f'processes/{pid}/events.ndjson'
    logs = native_file_chains(trace, str(capture.root / logpath), 'tap')
    require_same(capture.raw(logpath), b''.join(x['raw'] for x in logs), 'native-event-append-bytes')
    by_sequence = {}
    for item in logs:
        if not item['raw'].endswith(b'\n') or len(item['raw'].splitlines()) != 1:
            uncertain('native_event_line_unsupported', 'native-write')
        event = parse(item['raw']); require_same(pid, event['pid'], 'native-event-owner')
        if event['sequence'] in by_sequence:
            uncertain('native_event_sequence_ambiguous', 'native-write')
        by_sequence[event['sequence']] = item
    inputs = [f for f in stream_frames(process, 'stdin') if same(f['value'], request)]
    if len(inputs) != 1:
        uncertain('native_request_tap_ambiguous', 'native-write')
    def event_for(frame, kind):
        selected = [e for e in process['events'] if e['kind'] == kind and e['sample'] is frame['sample']]
        if len(selected) != 1 or selected[0]['sequence'] not in by_sequence:
            uncertain('native_tap_event_binding_missing', 'native-write')
        return selected[0], by_sequence[selected[0]['sequence']]
    _, incoming = event_for(inputs[0], 'stdin_data')
    outputs = [f for f in stream_frames(process, 'stdout') if isinstance(f['value'], dict) and same(f['value'].get('id'), request['id'])]
    if not outputs:
        if reply is not None:
            uncertain('native_response_tap_missing', 'native-write')
        # Only an explicitly hung request may use the observed terminal as upper
        # boundary. Existing native -15/group/unknown/no-retry checks still apply.
        if capture.mode != 'bounds' or request['params']['name'] != 'academy_cism_weekly_save':
            uncertain('native_missing_reply_unsupported', 'native-write')
        terminals = [i+1 for i, line in enumerate(trace.splitlines()) if '+++ killed by SIGTERM +++' in line]
        if len(terminals) != 1:
            uncertain('native_hung_terminal_missing', 'native-write')
        return incoming['closeLine'], terminals[0]
    if len(outputs) != 1:
        uncertain('native_response_tap_ambiguous', 'native-write')
    if reply is not None:
        require_same(outputs[0]['raw'], bytes.fromhex(reply['bytes']), 'native-response-os-frame')
    event, outgoing = event_for(outputs[0], 'stream_write')
    stdout = native_file_chains(trace, str(capture.root / f'processes/{pid}/stdout.bin'), 'tap')
    require_same(capture.raw(f'processes/{pid}/stdout.bin'), b''.join(x['raw'] for x in stdout), 'native-stdout-append-bytes')
    copies = [x for x in stdout if x['raw'] == event_bytes(event)]
    if len(copies) != 1 or copies[0]['closeLine'] >= outgoing['openLine']:
        uncertain('native_response_byte_append_ambiguous', 'native-write')
    return incoming['closeLine'], copies[0]['openLine']


def receipt_path(root, path):
    return re.fullmatch(re.escape(str(root)) + r'/receipts/(' + UUID.pattern[:-2] + r')-(dispatch|result|failure)\.json', path)


def native_receipt_records(capture, command):
    direct = command['direct']; trace = native_writer_trace(capture, command, direct)
    source = 'modules/academy-workbench/tools/academy-mcp/src/neutral-server.ts'
    if direct['entry'] != str(capture.root / 'source' / source):
        uncertain('native_receipt_source_unsupported', 'native-write')
    require_same(PINS['sources'][source]['sha256'], capture.entry_pin(direct['entry']), 'native-receipt-source-pin')
    paths = {path for _, path, _ in native_open_paths(trace) if receipt_path(capture.root, path)}
    records = []
    for path in sorted(paths):
        match = receipt_path(capture.root, path); occurrence, phase = match.groups()
        chains = native_file_chains(trace, path, 'receipt')
        if len(chains) != 1:
            uncertain('native_receipt_repeated_write', 'native-write')
        record = chains[0]; raw = capture.raw('receipts/' + Path(path).name)
        require_same(raw, record['raw'], 'native-receipt-retained-bytes')
        records.append({'occurrence': occurrence, 'phase': phase, 'raw': raw, 'body': parse(raw), 'nativeProof': record})
    occurrences = {r['occurrence'] for r in records}
    for other in capture.commands.values():
        for pid, text in other.get('traces', {}).items():
            for _, path, line in native_open_paths(text):
                match = receipt_path(capture.root, path)
                if match and (other is command or match[1] in occurrences) and (other is not command or pid != direct['start']['pid']):
                    uncertain('native_receipt_second_or_wrong_writer', 'native-write')
    windows = []
    for ordinal, request in enumerate(command['cap']['requests']):
        req = request['request']
        if req['method'] != 'tools/call':
            continue
        replies = [f for f in command['cap']['frames'] if isinstance(f.get('value'), dict) and same(f['value'].get('id'), req['id'])]
        if len(replies) > 1:
            uncertain('native_receipt_reply_ambiguous', 'native-write')
        reply = replies[0] if replies else None
        before = request['requestSendBracket']['before']; after = reply['responseReceiveBracket']['after'] if reply else command['result']['ended']
        wall = direct['clock'].recorder_window(before, after)
        incoming, outgoing = native_tap_bounds(capture, command, direct, req, reply)
        windows.append((ordinal, req, wall, incoming, outgoing))
    for record in records:
        proof = record['nativeProof']; low, high = proof['wallCell']
        selected = []
        for ordinal, req, (before, after), incoming, outgoing in windows:
            if enclose_cell((low, high), before, after, selection=True):
                selected.append((ordinal, req, incoming, outgoing))
        if len(selected) != 1:
            uncertain('native_receipt_request_window_collision', 'native-write')
        ordinal, req, incoming, outgoing = selected[0]
        if not incoming < proof['openLine'] <= proof['closeLine'] < outgoing:
            uncertain('native_receipt_causal_order_missing', 'native-write')
        record['requestOrdinal'] = ordinal
    return records


def native_fault_events(capture, command, process, call, before, after):
    source = 'source/fictional-fault-fixture.ts'
    if capture.mode != 'bounds' or process['entry'] != str(capture.root / source) or call['request']['params']['name'] != 'academy_cism_weekly_save' or call['result'] is not None:
        uncertain('native_fault_role_unsupported', 'native-write')
    require_same(PINS['sources'][STIMULUS['fixture']]['sha256'], sha(capture.raw(source)), 'native-fault-source-pin')
    trace = native_writer_trace(capture, command, process)
    incoming, outgoing = native_tap_bounds(capture, command, process, call['request'])
    path = str(capture.root / 'fault-events.ndjson')
    chains = native_file_chains(trace, path, 'fault'); retained = capture.raw('fault-events.ndjson').splitlines(keepends=True)
    wall = process['clock'].recorder_window(before, after); found = []
    for item in chains:
        raw = item['raw']
        if not raw.endswith(b'\n') or len(raw.splitlines()) != 1 or retained.count(raw) != 1:
            uncertain('native_fault_segment_ambiguous', 'native-write')
        body = parse(raw); require_same(process['start']['pid'], body.get('pid'), 'native-fault-payload-pid')
        if body.get('event') != 'hung':
            continue
        if set(body) != {'pid', 'endpoint', 'event'} or body['endpoint'] != process['start']['argv'][2]:
            uncertain('native_fault_payload_unsupported', 'native-write')
        if not incoming < item['openLine'] <= item['closeLine'] < outgoing:
            uncertain('native_fault_request_order_missing', 'native-write')
        enclose_cell(item['wallCell'], *wall)
        found.append(body)
    for e in process['events']:
        if e['kind'] == 'file_write' and e.get('success') and e['path'] == path:
            if event_bytes(e) not in [x['raw'] for x in chains]:
                uncertain('native_js_fault_disagreement', 'native-write')
            body = parse(event_bytes(e))
            if body.get('event') == 'hung':
                same_clock(e['sample'], call['before'])
                clock_interval(call['before'], e['sample'])
                process['clock'].contains(e['sample'], before, after)
    if not found:
        uncertain('native_fault_anchor_missing', 'native-write')
    return found


def direct_request_sample(capture, command, request, reply, sample, channel):
    """Finite direct tap proof only; generic clock containment stays strict."""
    process = command['direct']; clock = process['clock']
    before = request['requestSendBracket']['before']
    after = reply['responseReceiveBracket']['after'] if reply else command['result']['ended']
    try:
        clock.contains(sample, before, after)
        return wall_cell(sample)
    except Unqualified as error:
        if str(error) != 'clock_window_boundary_ambiguous@clock:1':
            raise
    if channel not in ['stdin', 'stdout']:
        uncertain('direct_clock_channel_unsupported', 'clock')
    if sample.owner != clock.owner:
        refuse('clock_owner_mismatch', 'clock')
    trace = native_writer_trace(capture, command, process); start = process['start']
    expected_owner = ClockDomain('bun', str(capture.root), start['pid'], command['start']['pid'], sha(js_bytes(start['nativeArgv'])))
    if clock.owner != expected_owner or clock.recorder != capture.recorder:
        refuse('clock_owner_mismatch', 'clock')
    source = 'modules/academy-workbench/tools/academy-mcp/src/neutral-server.ts'
    require_same(str(capture.root / 'source' / source), process['entry'], 'direct-clock-source')
    require_same(PINS['sources'][source]['sha256'], capture.entry_pin(process['entry']), 'direct-clock-source-pin')
    require_same((HERE / 'preload.ts').read_bytes(), capture.raw('observer-source/preload.ts'), 'direct-clock-preload-order')
    if sum(r is request for r in command['cap']['requests']) != 1:
        uncertain('direct_clock_request_binding_missing', 'clock')
    if reply is not None and (sum(r is reply for r in command['cap']['frames']) != 1 or
                              not same(reply['value'].get('id'), request['request']['id'])):
        uncertain('direct_clock_reply_binding_missing', 'clock')
    frames = [f for f in stream_frames(process, channel) if f['sample'] is sample and
              (same(f['value'], request['request']) if channel == 'stdin' else
               isinstance(f['value'], dict) and same(f['value'].get('id'), request['request']['id']))]
    if len(frames) != 1:
        uncertain('direct_clock_frame_ambiguous', 'clock')
    frame = frames[0]
    if channel == 'stdout':
        if reply is None:
            uncertain('direct_clock_reply_binding_missing', 'clock')
        require_same(bytes.fromhex(reply['bytes']), frame['raw'], 'direct-clock-response-frame')
    elif command['raw']['stdin'].count(frame['raw']) != 1:
        uncertain('direct_clock_input_frame_binding', 'clock')
    kind = 'stdin_data' if channel == 'stdin' else 'stream_write'
    events = [e for e in process['events'] if e['sample'] is sample and e['kind'] == kind and
              (channel == 'stdin' or e.get('channel') == 'stdout')]
    if len(events) != 1:
        uncertain('direct_clock_event_ambiguous', 'clock')
    event = events[0]; require_same(start['pid'], event['pid'], 'direct-clock-event-owner')
    require_same(frame['raw'], event_bytes(event), 'direct-clock-event-frame')
    logpath = f"processes/{start['pid']}/events.ndjson"
    logs = native_file_chains(trace, str(capture.root / logpath), 'tap')
    require_same(capture.raw(logpath), b''.join(x['raw'] for x in logs), 'direct-clock-event-append-bytes')
    selected = [x for x in logs if parse(x['raw']).get('sequence') == event['sequence']]
    if len(selected) != 1:
        uncertain('direct_clock_sequence_ambiguous', 'clock')
    logged = selected[0]
    # Strip only the in-memory ownership subclass, never captured fields/bytes.
    require_same({**event, 'sample': dict(sample)}, parse(logged['raw']), 'direct-clock-native-event')
    path = f"processes/{start['pid']}/{channel}.bin"
    copies = native_file_chains(trace, str(capture.root / path), 'tap')
    require_same(capture.raw(path), b''.join(x['raw'] for x in copies), 'direct-clock-stream-append-bytes')
    selected = [x for x in copies if x['raw'] == event_bytes(event)]
    if len(selected) != 1:
        uncertain('direct_clock_stream_append_ambiguous', 'clock')
    copied = selected[0]
    if copied['closeLine'] >= logged['openLine'] or any(
            copied['closeLine'] < x['closeLine'] < logged['openLine'] for x in copies):
        uncertain('direct_clock_append_order_ambiguous', 'clock')
    lines = trace.splitlines()
    lower = native_operation_cell(lines[copied['closeLine'] - 1])[0]
    upper = native_operation_cell(lines[logged['openLine'] - 1])[1]
    if lower > upper:
        uncertain('direct_clock_native_envelope_reversed', 'clock')
    cell = wall_cell(sample); narrowed = max(cell[0], lower), min(cell[1], upper)
    if narrowed[0] > narrowed[1]:
        uncertain('direct_clock_native_envelope_disjoint', 'clock')
    enclose_cell(narrowed, clock.exec_cell[0], clock.terminal_cell[1])
    enclose_cell(narrowed, *clock.recorder_window(before, after))
    return narrowed


def receipt_write_sample(capture, command, request, reply, event):
    """Only an actual successful receipt write may narrow its logging sample."""
    process = command['direct']; clock = process['clock']; sample = event['sample']
    before = request['requestSendBracket']['before']
    after = reply['responseReceiveBracket']['after'] if reply else command['result']['ended']
    try:
        clock.contains(sample, before, after)
        return wall_cell(sample)
    except Unqualified as error:
        if str(error) != 'clock_window_boundary_ambiguous@clock:1':
            raise
    trace = native_writer_trace(capture, command, process); start = process['start']
    owner = ClockDomain('bun', str(capture.root), start['pid'], command['start']['pid'], sha(js_bytes(start['nativeArgv'])))
    if sample.owner != owner or clock.owner != owner or clock.recorder != capture.recorder:
        refuse('clock_owner_mismatch', 'receipt-clock')
    source = 'modules/academy-workbench/tools/academy-mcp/src/neutral-server.ts'
    require_same(str(capture.root / 'source' / source), process['entry'], 'receipt-clock-source')
    require_same(PINS['sources'][source]['sha256'], capture.entry_pin(process['entry']), 'receipt-clock-source-pin')
    require_same((HERE / 'preload.ts').read_bytes(), capture.raw('observer-source/preload.ts'), 'receipt-clock-preload-order')
    if event['kind'] != 'file_write' or event.get('success') is not True or event.get('api') != 'writeFileSync':
        uncertain('receipt_clock_write_role_unsupported', 'receipt-clock')
    if sum(e is event for e in process['events']) != 1 or sum(r is request for r in command['cap']['requests']) != 1:
        uncertain('receipt_clock_event_request_ambiguous', 'receipt-clock')
    require_same(start['pid'], event['pid'], 'receipt-clock-event-owner')
    calls = [c for c in source_calls(process) if same(c['request'], request['request'])]
    if len(calls) != 1:
        uncertain('receipt_clock_request_owner_missing', 'receipt-clock')
    call = calls[0]; same_clock(call['before'], sample)
    clock_interval(call['before'], event['before']); clock_interval(event['before'], sample)
    if call['after'] is not None:
        clock_interval(sample, call['after'])
    if reply is not None:
        require_same(call['raw'], bytes.fromhex(reply['bytes']), 'receipt-clock-response-frame')
        if sum(f is reply for f in command['cap']['frames']) != 1:
            uncertain('receipt_clock_reply_ambiguous', 'receipt-clock')
    path = event['path']
    if not receipt_path(capture.root, path):
        uncertain('receipt_clock_path_unsupported', 'receipt-clock')
    chains = native_file_chains(trace, path, 'receipt')
    if len(chains) != 1:
        uncertain('receipt_clock_native_write_ambiguous', 'receipt-clock')
    written = chains[0]
    require_same(event_bytes(event), written['raw'], 'receipt-clock-native-write-bytes')
    require_same(written['raw'], capture.raw('receipts/' + Path(path).name), 'receipt-clock-retained-bytes')
    logpath = f"processes/{start['pid']}/events.ndjson"
    logs = native_file_chains(trace, str(capture.root / logpath), 'tap')
    require_same(capture.raw(logpath), b''.join(x['raw'] for x in logs), 'receipt-clock-event-append-bytes')
    selected = [x for x in logs if parse(x['raw']).get('sequence') == event['sequence']]
    if len(selected) != 1:
        uncertain('receipt_clock_sequence_ambiguous', 'receipt-clock')
    logged = selected[0]
    require_same({**event, 'sample': dict(sample), 'before': dict(event['before'])}, parse(logged['raw']), 'receipt-clock-native-event')
    if written['closeLine'] >= logged['openLine']:
        uncertain('receipt_clock_native_order_ambiguous', 'receipt-clock')
    lines = trace.splitlines()
    lower = native_operation_cell(lines[written['closeLine'] - 1])[0]
    upper = native_operation_cell(lines[logged['openLine'] - 1])[1]
    cell = wall_cell(sample); narrowed = max(cell[0], lower), min(cell[1], upper)
    if lower > upper or narrowed[0] > narrowed[1]:
        uncertain('receipt_clock_native_envelope_disjoint', 'receipt-clock')
    enclose_cell(narrowed, clock.exec_cell[0], clock.terminal_cell[1])
    enclose_cell(narrowed, *clock.recorder_window(before, after))
    return narrowed


def request_receipts(capture, command, request, reply):
    direct = command['direct']
    lo = request['requestSendBracket']['before']
    hi = reply['responseReceiveBracket']['after'] if reply else command['result']['ended']
    calls = [c for c in source_calls(direct) if same(c['request'], request['request'])]
    if len(calls) != 1:
        uncertain('receipt_request_owner_missing', 'clock')
    call = calls[0]
    direct_request_sample(capture, command, request, reply, call['before'], 'stdin')
    if call['after'] is not None:
        direct_request_sample(capture, command, request, reply, call['after'], 'stdout')
    found = {}
    for e in direct['events']:
        if e['kind'] != 'file_write' or not e.get('success'):
            continue
        m = re.fullmatch(re.escape(str(capture.root)) + r'/receipts/(' + UUID.pattern[:-2] + r')-(dispatch|result|failure)\.json', e['path'])
        if not m:
            continue
        same_clock(call['before'], e['sample'])
        if sample_bounds(e['sample'])[1] < sample_bounds(call['before'])[1]:
            continue
        if call['after'] is not None and sample_bounds(e['sample'])[2] > sample_bounds(call['after'])[2]:
            continue
        receipt_write_sample(capture, command, request, reply, e)
        occurrence, phase = m.groups()
        if phase in found:
            refuse('serial_receipt_window_collision', command['direct']['role'])
        data = event_bytes(e)
        require_same(data, capture.raw('receipts/' + occurrence + '-' + phase + '.json'), 'successful-write-raw')
        found[phase] = {'occurrence': occurrence, 'body': parse(data), 'raw': data, 'event': e}
    native = native_receipt_records(capture, command)
    matches = [i for i, r in enumerate(command['cap']['requests']) if r is request]
    if len(matches) != 1:
        uncertain('native_receipt_request_ordinal_missing', 'native-write')
    selected = {}
    for item in native:
        if item['requestOrdinal'] != matches[0]:
            continue
        phase = item['phase']
        if phase in selected:
            refuse('serial_receipt_window_collision', command['direct']['role'])
        selected[phase] = {k: v for k, v in item.items() if k not in ['phase', 'requestOrdinal']}
    for phase, js in found.items():
        if phase not in selected or js['occurrence'] != selected[phase]['occurrence'] or js['raw'] != selected[phase]['raw']:
            uncertain('native_js_receipt_disagreement', 'native-write')
        selected[phase]['event'] = js['event']  # Actual event only, never synthesized.
    return selected


def upstream_in_window(capture, command, before, after):
    result = []
    for pid, process in capture.processes.items():
        if process['role'] != command['direct']['role'] or process is command['direct']:
            continue
        for ordinal, call in enumerate(source_calls(process)):
            if process['clock'].contains(call['before'], before, after, selection=True):
                if call['after'] is not None:
                    process['clock'].contains(call['after'], before, after)
                result.append((pid, ordinal, process, call))
    result.sort(key=cmp_to_key(lambda a, b: clock_order(a[3]['before'], b[3]['before'])))
    return result


def relation_result(capture, command, req, reply, ordinal):
    request = req['request']
    before = req['requestSendBracket']['before']
    after = reply['responseReceiveBracket']['after'] if reply else command['result']['ended']
    label = command['direct']['role'] + ':' + str(ordinal)
    atoms = Atoms(label)
    result = reply['value'].get('result') if reply else None
    if request['method'] != 'tools/call':
        return atoms
    original_calls = [c for c in source_calls(command['direct']) if same(c['request'].get('id'), request['id'])]
    if len(original_calls) != 1:
        uncertain('direct_original_request_binding', label)
    direct_call = original_calls[0]
    require_same(request, direct_call['request'], 'original-request-anchor')
    require_same(direct_call['result'], result, 'original-response-anchor')
    tool = request['params']['name']
    bridge = command['direct']['entry'].endswith('/neutral-server.ts')
    if not bridge:
        if result:
            audit_roles(result, request, before, after, atoms, label)
            if tool == 'academy_endpoint_identity' and result.get('structuredContent', {}).get('status') == 'qualified':
                endpoint_identity_roles(capture, result['structuredContent'], atoms, label)
        return atoms
    receipts = request_receipts(capture, command, req, reply)
    pre_dispatch = (capture.mode == 'graph' and request['id'] in [6, 8]) or (capture.mode == 'identity' and request['id'] == 5)
    upstream = upstream_in_window(capture, command, before, after)
    producer = None
    if tool not in ['academy_runtime_identity', 'academy_dailyrecord_get', 'academy_dailytest_history_with_headers']:
        matching = [c for _, _, _, c in upstream if c['request']['params']['name'] == tool and c['result'] is not None]
        if matching:
            producer = matching[-1]['result']
    if tool == 'academy_runtime_identity':
        producer = direct_call['result']
    if tool in ['academy_dailyrecord_get', 'academy_dailytest_history_with_headers'] and result and 'result' in receipts:
        # Composition returns a new source-owned {content,structuredContent};
        # independent upstream joins below validate its generated fields first.
        producer = {k: direct_call['result'][k] for k in ['content', 'structuredContent']}
    bridge_roles(request, result, receipts, before, after, atoms, label, producer,
                 pre_dispatch=pre_dispatch, hung=capture.mode == 'bounds' and request['id'] == 6)
    bridge_generated = set(atoms.uuid_values)
    for n, (_, inner_ordinal, process, call) in enumerate(upstream):
        atoms.upstream.append(upstream_semantics(capture, command, process, call, label + ':upstream:' + str(n)))
        if call['result']:
            audit = call['result'].get('_meta', {}).get('academy/audit')
            generated = []
            if audit:
                generated.append(audit['occurrence'])
            handle = call['result'].get('structuredContent', {}).get('observationHandle')
            if isinstance(handle, str) and handle.startswith('dailyrecord-observation:'):
                generated.append(handle[len('dailyrecord-observation:'):])
            for value in generated:
                uuid(value)
                if value in atoms.uuid_values:
                    refuse('reused_upstream_generated_occurrence', label)
                atoms.uuid_values.add(value)
    atoms.receipt_semantics = receipt_semantics(capture, receipts, before, after, label, upstream)
    if not result:
        if capture.mode == 'bounds' and request['id'] == 6:
            hung = []
            for _, _, process, call in upstream:
                if call['request']['params']['name'] != 'academy_cism_weekly_save':
                    continue
                hung.extend(native_fault_events(capture, command, process, call, before, after))
            if len(hung) != 1:
                refuse('hung_actual_handler_event_missing', label)
        return atoms
    data = result.get('structuredContent', {})
    if tool == 'academy_runtime_identity' and not result.get('isError'):
        process_time = data.get('processStartedAt')
        freshness(process_time, command['start']['prelaunch'], command['result']['ended'], True)
        atoms.add(('structuredContent', 'processStartedAt'), process_time, label.split(':')[0] + ':process-started')
        identity_roles(capture, data, atoms)
    if tool == 'academy_item_answer_lookup' and 'observedAt' in data:
        candidates = [c for _, _, _, c in upstream if c['request']['params']['name'] == tool and c['result']]
        if len(candidates) != 1:
            uncertain('item_answer_producer_missing', label)
        original = candidates[0]['result']['structuredContent']['observedAt']
        require_same(original, data['observedAt'], 'item-answer-original-timestamp')
        freshness(original, candidates[0]['before'], candidates[0]['after'], True)
        atoms.add(('structuredContent', 'observedAt'), original, label + ':item-answer-time')
    if tool == 'academy_dailyrecord_get' and data.get('status') == 'observed':
        intake = [c for _, _, _, c in upstream if c['request']['params']['name'] == 'academy_dailyrecord_intake' and c['result']]
        resolve = [c for _, _, _, c in upstream if c['request']['params']['name'] == 'academy_student_resolve' and c['result']]
        if len(intake) != 1 or len(resolve) != 1:
            uncertain('intake_resolve_producer_binding', label)
        source = intake[0]['result']['structuredContent']['source']
        check_propagated_source(source, data.get('source'), intake[0]['before'], intake[0]['after'])
        freshness(source['observedAt'], before, after, True)  # bridge's separate check
        atoms.add(('structuredContent', 'source', 'observedAt'), source['observedAt'], label + ':source-time')
        terminal = receipts['result']['body']
        audited = []
        for step, upstream_call in zip(terminal['steps'], [resolve[0], intake[0]]):
            audit = upstream_call['result'].get('_meta', {}).get('academy/audit')
            if audit:
                audit_roles(upstream_call['result'], upstream_call['request'], upstream_call['before'], upstream_call['after'], Atoms('producer'), 'producer')
                require_same(audit['occurrence'], step.get('upstreamOccurrence'), 'step-upstream-occurrence')
                audited.append(audit['occurrence'])
        if request['id'] in [20, 21, 22]:
            if len(audited) != 2 or audited[0] == audited[1] or any(o in bridge_generated for o in audited):
                refuse('distinct_audit_generations', label)
    return atoms


def setup_versions(capture, relative):
    return [(parse(bytes.fromhex(e['bytesHex'])), bytes.fromhex(e['bytesHex'])) for e in capture.setup if e.get('path') == relative and 'bytesHex' in e]


def identity_roles(capture, data, atoms):
    require_same(PINS['sources']['modules/academy-workbench/tools/academy-mcp/src/neutral-server.ts']['sha256'], data.get('loadedBridgeSha256'), 'identity-loaded-bridge')
    if 'pointerSha256' not in data:
        return
    pointers = []
    for e in capture.setup:
        if e.get('path', '').endswith('.pointer.json') and 'bytesHex' in e:
            raw = bytes.fromhex(e['bytesHex']); value = parse(raw)
            if value['generation'] == data.get('generation'):
                pointers.append((value, raw))
    if len(pointers) != 1:
        refuse('identity_pointer_generation', 'identity')
    value, raw = pointers[0]
    require_same(value['schemaVersion'], data['schemaVersion'], 'identity-schema')
    require_same(sha(raw), data['pointerSha256'], 'identity-pointer-preimage')
    atoms.add(('structuredContent', 'pointerSha256'), data['pointerSha256'], 'identity:' + value['generation'] + ':pointer')
    for key in ['routing', 'endpoints', 'registry']:
        ref = value[key]
        rooted(ref['path'], capture.root, key + '.json')
        versions = setup_versions(capture, key + '.json')
        if key == 'registry' and not versions:
            versions = [(parse(capture.raw('registry.json')), capture.raw('registry.json'))]
        matching = [b for _, b in versions if sha(b) == ref['sha256']]
        if len(matching) != 1:
            refuse('identity_payload_preimage', key)
        require_same(ref['sha256'], data[key + 'Sha256'], 'identity-payload-digest')
        atoms.add(('structuredContent', key + 'Sha256'), data[key + 'Sha256'], 'identity:' + value['generation'] + ':' + key)
    admission_raw = capture.raw('admission.json')
    require_same(sha(admission_raw), data['admissionSha256'], 'identity-admission-preimage')
    routing_raw = next(b for _, b in setup_versions(capture, 'routing.json') if sha(b) == value['routing']['sha256'])
    routing = parse(routing_raw)
    rooted(routing['admissionPath'], capture.root, 'admission.json')
    require_same(sha(admission_raw), routing['admissionSha256'], 'routing-admission-preimage')
    if 'endpointIdentities' in data:
        for i, endpoint in enumerate(data['endpointIdentities']):
            expected = setup_versions(capture, 'endpoint-identity.json')
            if len(expected) != 1:
                refuse('endpoint_manifest_preimage_missing', 'identity')
            manifest, b = expected[0]
            require_same(sha(b), endpoint['manifestSha256'], 'manifest-preimage')
            for artifact in manifest['artifacts']:
                relative = artifact['path'].removeprefix(str(capture.root) + '/source/')
                if relative not in PINS['sources']:
                    refuse('endpoint_artifact_pin', 'identity')
                rooted(artifact['path'], capture.root, 'source/' + relative)
                require_same(PINS['sources'][relative]['sha256'], artifact['sha256'], 'endpoint-source-digest')
            atoms.add(('structuredContent', 'endpointIdentities', i, 'manifestSha256'), endpoint['manifestSha256'], 'endpoint-manifest')


def expected_requests(mode, role):
    if role in ['version-bun', 'discover', 'tampered-bridge', 'lock-release-readback']:
        return []
    requests = [STIMULUS['initialize'], STIMULUS['toolsList']]
    if role == 'qualified-endpoint':
        requests = [STIMULUS['initialize']]
        actions = [a['request'] for a in STIMULUS['modes']['identity'] if a['op'] == 'qualified_endpoint']
    elif mode == 'discovery' and role == 'neutral':
        actions = []
    else:
        actions = STIMULUS['modes'][mode]
    for a in actions:
        if a['op'] == 'rpc':
            requests.append({'jsonrpc': '2.0', **{k: v for k, v in a.items() if k != 'op'}})
        if a['op'] in ['pending', 'hung']:
            requests.append({'jsonrpc': '2.0', 'id': a['id'], 'method': 'tools/call', 'params': a['params']})
    return requests


def expected_stdin(mode, role):
    if role in ['version-bun', 'discover', 'tampered-bridge', 'lock-release-readback']:
        return []
    chunks = [js_bytes(STIMULUS['initialize']) + b'\n', js_bytes(STIMULUS['initialized']) + b'\n']
    if role != 'qualified-endpoint':
        chunks.append(js_bytes(STIMULUS['toolsList']) + b'\n')
    if role == 'qualified-endpoint':
        actions = [a['request'] for a in STIMULUS['modes']['identity'] if a['op'] == 'qualified_endpoint']
    elif mode == 'discovery' and role == 'neutral':
        actions = []
    else:
        actions = STIMULUS['modes'][mode]
    for a in actions:
        if a['op'] == 'rpc':
            chunks.append(js_bytes({'jsonrpc': '2.0', **{k: v for k, v in a.items() if k != 'op'}}) + b'\n')
        elif a['op'] in ['pending', 'hung']:
            chunks.append(js_bytes({'jsonrpc': '2.0', 'id': a['id'], 'method': 'tools/call', 'params': a['params']}) + b'\n')
        elif a['op'] == 'raw':
            chunks.append(bytes.fromhex(a['hex']))
        elif a['op'] == 'oversize':
            chunks.append(bytes([a['byte']]) * a['count'] + bytes.fromhex(a['suffixHex']))
    return chunks


def expected_environment(capture, role):
    root = capture.root
    dependencies = PINS['dependencyRoot']
    env = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'LANG': 'C.UTF-8', 'TZ': 'UTC', 'TMPDIR': str(root),
           'BUN_RUNTIME_TRANSPILER_CACHE_PATH': '0', 'S11_SOURCE_ROOT': str(root / 'source'),
           'S11_MODE_ROOT': str(root), 'S11_DEPENDENCY_ROOT': dependencies,
           'NODE_PATH': dependencies + '/node_modules', 'ACADEMY_MCP_INSTALL_DIR': str(root)}
    if capture.mode in ['launcher', 'relocation'] and role in ['valid-launch', 'tampered-bridge']:
        env['BUN_OPTIONS'] = '--no-install --preload ' + str(root / 'observer-source/preload.ts')
    return env


def expected_command_argv(capture, role):
    root = capture.root
    src = root / 'source/modules/academy-workbench/tools/academy-mcp/src'
    base = ['/usr/local/bin/bun', '--no-install', '--no-env-file', '--preload', str(root / 'observer-source/preload.ts')]
    if role == 'version-bun':
        return ['/usr/local/bin/bun', '--version']
    if role == 'lock-release-readback' and capture.mode == 'bounds':
        return ['/usr/bin/flock', '--nonblock', str(root / 'fictional-account.lock'), '/usr/bin/true']
    if role == 'discover':
        return base + [str(src / 'neutral-server.ts'), '--discover']
    if role == 'neutral':
        return base + [str(src / 'neutral-server.ts')]
    if role == 'dailyrecord-source' and capture.mode == 'discovery':
        return base + [str(src / 'dailyrecord-server.ts')]
    if role == 'qualified-endpoint' and capture.mode == 'identity':
        return base + [str(src / 'dailyrecord-server.ts'), '--runtime-identity', str(root / 'endpoint-identity.json'), sha(capture.raw('endpoint-identity.json'))]
    if role in ['valid-launch', 'tampered-bridge'] and capture.mode in ['launcher', 'relocation']:
        return base + [str(root / 'relocated/launcher.ts') if capture.mode == 'relocation' else str(src / 'stable-launcher.ts')]
    refuse('unexpected_command_role', role)


def receipt_semantics(capture, receipts, before, after, label, upstream):
    output = {}
    for phase, record in receipts.items():
        body = dict(record['body'])
        common = {'occurrence', 'phase', 'endpoint', 'upstreamName', 'elapsedMs', 'upstreamOccurrence', 'tool', 'automaticRetryAllowed'}
        if phase == 'dispatch':
            expected = {'issuedAt', 'tool', 'argumentsSha256', 'automaticRetryAllowed', 'academyWrites', 'parentSends'}
        elif phase == 'failure':
            expected = common | {'code', 'failure', 'observedAt', 'academyWrites', 'parentSends'}
            if body.get('academyWrites') is None:
                expected.add('effectStatus')
        elif body.get('status') == 'identity_observed':
            expected = common | {'generation', 'pointerSha256', 'completedAt', 'status', 'resultSha256'}
        else:
            expected = common | {'generation', 'pointerSha256', 'completedAt', 'isError', 'status', 'sourceAvailability',
                                 'steps', 'failure', 'resultSha256', 'academyWrites', 'parentSends'}
        require_same(expected, set(body), 'receipt-source-field-set')
        for field in ['issuedAt', 'completedAt', 'observedAt']:
            if field in body:
                freshness(body[field], before, after, True)
                body[field] = '<' + label + ':' + phase + ':' + field + '>'
        if 'occurrence' in body:
            require_same(record['occurrence'], body['occurrence'], 'receipt-role-occurrence')
            body['occurrence'] = '<' + label + ':bridge>'
        if 'elapsedMs' in body:
            elapsed(body['elapsedMs'], before, after)
            body['elapsedMs'] = '<' + label + ':' + phase + ':elapsed>'
        if 'resultSha256' in body:
            # bridge_roles has checked the actual source-specific preimage.
            body['resultSha256'] = '<' + label + ':result-preimage>'
        if body.get('pointerSha256') is not None:
            matches = [bytes.fromhex(e['bytesHex']) for e in capture.setup
                       if e.get('path', '').endswith('.pointer.json') and 'bytesHex' in e
                       and parse(bytes.fromhex(e['bytesHex'])).get('generation') == body['generation']]
            if len(matches) != 1:
                refuse('receipt_release_preimage_missing', label)
            require_same(sha(matches[0]), body['pointerSha256'], 'receipt-pointer-digest')
            body['pointerSha256'] = '<release:' + body['generation'] + '>'
        for step_i, step in enumerate(body.get('steps', [])):
            if 'upstreamOccurrence' in step:
                calls = [c for _, _, _, c in upstream if c['request']['params']['name'] == step['tool'] and c['result']]
                if len(calls) != 1:
                    refuse('step_original_producer_missing', label)
                audit = calls[0]['result'].get('_meta', {}).get('academy/audit', {})
                require_same(audit.get('occurrence'), step['upstreamOccurrence'], 'step-original-audit')
                step = dict(step); step['upstreamOccurrence'] = '<' + label + ':step:' + str(step_i) + '>'
                body['steps'] = [dict(x) for x in body['steps']]
                body['steps'][step_i] = step
        if body.get('upstreamOccurrence') is not None:
            matches = [c['result'].get('_meta', {}).get('academy/audit', {}).get('occurrence') for _, _, _, c in upstream if c['result']]
            if body['upstreamOccurrence'] not in matches:
                refuse('receipt_upstream_producer_missing', label)
            body['upstreamOccurrence'] = '<' + label + ':upstream-terminal>'
        output[phase] = body
    return output


def fixture_echo_roles(capture, command, process, call, atoms, label):
    """Only the pinned fictional save fixture echoes the validated attribution."""
    entry = str(capture.root / 'source/fictional-fault-fixture.ts')
    if process['entry'] != entry or call['request']['params']['name'] != 'academy_cism_weekly_save':
        return
    result = call['result']; data = result.get('structuredContent', {})
    if 'receivedArguments' not in data:
        return  # Error/hung fixture results retain their existing literal meaning.
    require_same(PINS['sources'][STIMULUS['fixture']]['sha256'], capture.entry_pin(entry), 'fixture-echo-source-pin')
    require_same(PINS['sources'][STIMULUS['fixture']],
                 {'sha256': sha(capture.raw('source/fictional-fault-fixture.ts')), 'bytes': len(capture.raw('source/fictional-fault-fixture.ts'))}, 'fixture-echo-source-bytes')
    require_same(command['native']['execs'][process['start']['pid']]['argv'], process['start']['nativeArgv'], 'fixture-echo-native-owner')
    require_same(['/usr/local/bin/bun', entry, 'dailyrecord'], process['start']['argv'], 'fixture-echo-source-argv')
    inputs = [f for f in stream_frames(process, 'stdin') if isinstance(f['value'], dict) and same(f['value'].get('id'), call['request']['id'])]
    if len(inputs) != 1:
        uncertain('fixture_echo_input_anchor_ambiguous', label)
    require_same(js_bytes(inputs[0]['value']), js_bytes(call['request']), 'fixture-echo-original-input')
    args = call['request']['params'].get('arguments', {}); attribution = args.get('attribution')
    if attribution is None:
        uncertain('fixture_echo_attribution_missing', label)
    original = command['direct']['start']
    require_same('neutral-bridge', attribution['observedBy'], 'fixture-echo-attribution-owner')
    for key in ['pid', 'ppid', 'uid']:
        require_same(original[key], attribution['process'][key], 'fixture-echo-native-' + key)
    require_same('strace', attribution['process']['parentCommand'], 'fixture-echo-parent-command')
    require_same(capture.entry_pin(command['direct']['entry']), attribution['process']['bridgeSha256'], 'fixture-echo-bridge-source')
    freshness(attribution['observedAt'], command['start']['prelaunch'], call['before'], True, native=process['clock'])
    expected = {'status': 'observed', 'fictional': True, 'endpoint': 'dailyrecord', 'receivedArguments': args}
    require_same(expected, data, 'fixture-echo-input-equality')
    require_same(js_bytes(args), js_bytes(data['receivedArguments']), 'fixture-echo-input-serialization')
    require_same({'content': [{'type': 'text', 'text': js_bytes(data).decode()}], 'structuredContent': data}, result, 'fixture-echo-dual-bytes')
    path = ('structuredContent', 'receivedArguments', 'attribution')
    atoms.add(path + ('observedAt',), attribution['observedAt'], label + ':attribution-time')
    for key in ['pid', 'ppid']:
        atoms.add(path + ('process', key), attribution['process'][key], label + ':bridge-' + key)


def upstream_semantics(capture, command, process, call, label):
    request = call['request']
    request_roles = {}
    args = request['params'].get('arguments', {})
    attribution = args.get('attribution')
    if attribution is not None:
        original = command['direct']['start']
        p = ('params', 'arguments', 'attribution')
        require_same('neutral-bridge', attribution['observedBy'], 'attribution-owner')
        for key in ['pid', 'ppid', 'uid']:
            require_same(original[key], attribution['process'][key], 'attribution-native-' + key)
            if key != 'uid':
                request_roles[p + ('process', key)] = label + ':bridge-' + key
        require_same('strace', attribution['process']['parentCommand'], 'attribution-parent-command')
        require_same(capture.entry_pin(command['direct']['entry']), attribution['process']['bridgeSha256'], 'attribution-source-sha')
        freshness(attribution['observedAt'], command['start']['prelaunch'], call['before'], True, native=process['clock'])
        request_roles[p + ('observedAt',)] = label + ':attribution-time'
    input_tree = LexicalJSON(js_bytes(request), request_roles).tree()
    result = call['result']
    if result is None:
        return {'input': input_tree, 'output': None, 'effect': 'unknown'}
    atoms = Atoms(label)
    tool = request['params']['name']
    audit_roles(result, request, call['before'], call['after'], atoms, label)
    fixture_echo_roles(capture, command, process, call, atoms, label)
    data = result.get('structuredContent', {})
    if tool == 'academy_dailyrecord_intake' and data.get('source') is not None:
        value = data['source']['observedAt']
        freshness(value, call['before'], call['after'], True)
        atoms.add(('structuredContent', 'source', 'observedAt'), value, label + ':source-time')
        handle = data.get('observationHandle')
        if handle is not None:
            prefix = 'dailyrecord-observation:'
            if not isinstance(handle, str) or not handle.startswith(prefix):
                refuse('observation_handle_role', label)
            uuid(handle[len(prefix):])
            atoms.add(('structuredContent', 'observationHandle'), handle, label + ':observation-handle')
    if tool == 'academy_item_answer_lookup' and 'observedAt' in data:
        value = data['observedAt']; freshness(value, call['before'], call['after'], True)
        atoms.add(('structuredContent', 'observedAt'), value, label + ':item-answer-time')
    if tool == 'academy_endpoint_identity' and data.get('status') == 'qualified':
        endpoint_identity_roles(capture, data, atoms, label)
    frame = parse(call['raw'])
    roles, texts = atoms.result_wire(frame)
    return {'input': input_tree, 'output': LexicalJSON(call['raw'], roles, texts).tree()}


def endpoint_identity_roles(capture, data, atoms, label):
    originals = setup_versions(capture, 'endpoint-identity.json')
    if len(originals) != 1:
        refuse('endpoint_identity_manifest_missing', label)
    manifest, raw = originals[0]
    require_same(sha(raw), data.get('manifestSha256'), 'endpoint-manifest-digest')
    require_same({'status': 'qualified', 'endpoint': manifest['endpoint'], 'semanticContractVersion': manifest['semanticContractVersion'],
                  'manifestSha256': sha(raw), 'launchArgumentsSha256': manifest['launchArgumentsSha256'],
                  'artifacts': [{k: a[k] for k in ['role', 'sha256']} for a in manifest['artifacts']]}, data, 'endpoint-identity-source-model')
    atoms.add(('structuredContent', 'manifestSha256'), data['manifestSha256'], label + ':manifest')


def compare_commands(a, b):
    require_same(a.complete['commands'], b.complete['commands'], 'command-role-order')
    for role, left in a.commands.items():
        right = b.commands[role]
        # Diagnostics are literal, never UUID/clock regex normalized.
        require_same(left['raw']['stderr'], right['raw']['stderr'], 'stderr-diagnostic')
        expected = expected_requests(a.mode, role)
        requests_a, requests_b = left['cap']['requests'], right['cap']['requests']
        require_same(expected, [r['request'] for r in requests_a], 'frozen-request-sequence-A')
        require_same(expected, [r['request'] for r in requests_b], 'frozen-request-sequence-B')
        require_same(left['raw']['stdin'], right['raw']['stdin'], 'same-stdin')
        frames_a, frames_b = left['cap']['frames'], right['cap']['frames']
        if len(frames_a) != len(frames_b):
            refuse('wire_frame_count', role)
        mapped_a, mapped_b = {}, {}
        for capture, command, requests, target in [(a, left, requests_a, mapped_a), (b, right, requests_b, mapped_b)]:
            if 'direct' not in command:
                continue
            for ordinal, req in enumerate(requests):
                require_same(ordinal, req['ordinal'], 'request-ordinal')
                matches = [f for f in command['cap']['frames'] if isinstance(f.get('value'), dict) and same(f['value'].get('id'), req['request']['id'])]
                hung = capture.mode == 'bounds' and role == 'neutral' and req['request']['id'] == 6
                if len(matches) != (0 if hung else 1):
                    refuse('request_reply_binding', role)
                reply = matches[0] if matches else None
                atoms = relation_result(capture, command, req, reply, ordinal)
                if capture.uuid_values.intersection(atoms.uuid_values):
                    refuse('reused_occurrence_between_requests', role)
                capture.uuid_values.update(atoms.uuid_values)
                if reply:
                    target[reply['index']] = atoms
        for i, (fa, fb) in enumerate(zip(frames_a, frames_b)):
            if 'value' not in fa or 'value' not in fb:
                require_same(bytes.fromhex(fa['bytes']), bytes.fromhex(fb['bytes']), 'non-json-wire')
                continue
            aa = mapped_a.get(i, Atoms('literal')); bb = mapped_b.get(i, Atoms('literal'))
            require_same(aa.receipt_semantics, bb.receipt_semantics, 'receipt-semantic-comparison')
            require_same(aa.upstream, bb.upstream, 'upstream-semantic-comparison')
            ra, ta = aa.result_wire(fa['value']); rb, tb = bb.result_wire(fb['value'])
            require_same(ra, rb, 'generated-role-pointer-map')
            wire_equal(bytes.fromhex(fa['bytes']), bytes.fromhex(fb['bytes']), ra, rb, ta, tb)
    if a.uuid_values.intersection(b.uuid_values):
        refuse('constant_occurrence_replay', 'paired-original')
    compare_process_material(a, b)


def argv_roles(capture, values):
    # Exact whole-field templates, not substring/root regex substitution.
    declared = {str(capture.root / 'source' / p): '<R>/source/' + p for p in PINS['sources']}
    for relative in ['source/fictional-fault-fixture.ts', 'observer-source/preload.ts', 'events.txt',
                     'endpoint-identity.json', 'releases/fictional/server.mjs', 'relocated/launcher.ts']:
        declared[str(capture.root / relative)] = '<R>/' + relative
    if 'endpoint-identity.json' in capture.manifest:
        declared[sha(capture.raw('endpoint-identity.json'))] = '<endpoint-manifest-digest>'
    return [declared.get(v, v) for v in values]


def native_process_order(capture, role):
    processes = [p for p in capture.processes.values() if p['role'] == role]
    processes.sort(key=lambda p: p['clock'].exec_cell[0])
    for left, right in zip(processes, processes[1:]):
        if left['clock'].exec_cell[1] >= right['clock'].exec_cell[0]:
            uncertain('native_exec_order_ambiguous', 'clock')
    return processes


def compare_process_material(a, b):
    for role in a.commands:
        left = native_process_order(a, role)
        right = native_process_order(b, role)
        if len(left) != len(right):
            refuse('process_generation_count', role)
        for n, (pa, pb) in enumerate(zip(left, right)):
            require_same(argv_roles(a, pa['start']['nativeArgv']), argv_roles(b, pb['start']['nativeArgv']), 'native-process-role-argv')
            require_same(pa['osScope'], pb['osScope'], 'native-fd-role')
            for channel in ['stdin', 'stdout', 'stderr']:
                require_same(pa['channels'][channel]['scope'], pb['channels'][channel]['scope'], 'tap-evidence-scope')
            # Inner stderr remains a JS-tap claim. No event is not OS silence.
            require_same(pa['channels']['stderr']['raw'], pb['channels']['stderr']['raw'], 'inner-js-stderr')
            if pa is a.commands[role].get('direct'):
                continue  # Bound to top OS raw and checked per-request above.
            ca, cb = source_calls(pa), source_calls(pb)
            if len(ca) != len(cb):
                refuse('upstream_request_count', role)
            for k, (x, y) in enumerate(zip(ca, cb)):
                label = role + ':process:' + str(n) + ':call:' + str(k)
                require_same(upstream_semantics(a, a.commands[role], pa, x, label),
                             upstream_semantics(b, b.commands[role], pb, y, label), 'inner-original-wire')
            # Initialization and inventory have no tools/call-generated fields.
            called_ids_a = [c['request']['id'] for c in ca]
            called_ids_b = [c['request']['id'] for c in cb]
            for channel in ['stdin', 'stdout']:
                fa = [f for f in stream_frames(pa, channel) if f['value'].get('id') not in called_ids_a]
                fb = [f for f in stream_frames(pb, channel) if f['value'].get('id') not in called_ids_b]
                if len(fa) != len(fb):
                    refuse('inner_protocol_frame_count', role)
                for x, y in zip(fa, fb):
                    wire_equal(x['raw'], y['raw'])


def qualify_pair(root_a, root_b, mode, custody):
    if custody is None:
        uncertain('external_git_raw_custody_missing', 'qualifier')
    names = ['CONTRACT.md', 'POLICY.json', 'STIMULUS.json', 'ORIGINAL-PINS.json', 'record.py', 'preload.ts', 'qualify.py', 'test_qualify.py']
    require_same(set(names), set(custody['qualifierFiles']), 'external-qualifier-file-list')
    for n in names:
        require_same(custody['qualifierFiles'][n], sha((HERE / n).read_bytes()), 'external-qualifier-blob-' + n)
    a = Capture(root_a, custody['captures'].get(str(root_a)), custody.get('supervisors', {}).get(str(root_a)))
    b = Capture(root_b, custody['captures'].get(str(root_b)), custody.get('supervisors', {}).get(str(root_b)))
    require_same('A', a.run['run'], 'run-A'); require_same('B', b.run['run'], 'run-B')
    require_same(mode, a.mode, 'mode-A'); require_same(mode, b.mode, 'mode-B')
    if a.root == b.root:
        refuse('same_capture_root', 'paired-original')
    compare_commands(a, b)
    return {'status': 'PAIRED_ORIGINAL_MATCH', 'qualification': 'BOUNDED_FRESH_FICTIONAL',
            'mode': mode, 'allTenModes': False, **POLICY['claims'],
            'limitations': ['instrumented fictional boundary', 'stable local clock assumption',
                            'inner JS taps are not independent OS channels', 'Root custody is external']}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--original-a', type=Path)
    p.add_argument('--original-b', type=Path)
    p.add_argument('--mode', choices=POLICY['modes'])
    p.add_argument('--custody', type=Path, required=True, help='Root-created independent raw-manifest and qualifier Git-blob SHA256 readback')
    p.add_argument('--suite', type=Path, help='Root-frozen mode -> A/B root mapping, all ten required')
    x = p.parse_args()
    try:
        custody = parse(x.custody.read_bytes())
        if x.suite:
            pairs_by_mode = parse(x.suite.read_bytes())
            require_same(set(POLICY['modes']), set(pairs_by_mode), 'all-ten-mode-set')
            results = [qualify_pair(Path(pairs_by_mode[m]['A']), Path(pairs_by_mode[m]['B']), m, custody) for m in POLICY['modes']]
            result = {'status': 'PAIRED_ORIGINAL_MATCH', 'qualification': 'BOUNDED_FRESH_FICTIONAL',
                      'allTenModes': True, 'modes': results, **POLICY['claims']}
        else:
            if not x.original_a or not x.original_b or not x.mode:
                uncertain('pair_arguments_missing', 'qualifier')
            result = qualify_pair(x.original_a, x.original_b, x.mode, custody)
        print(json.dumps(result, indent=2)); return 0
    except BaseException as error:
        if isinstance(error, SystemExit):
            raise
        unqualified = isinstance(error, (Unqualified, OSError))
        print(json.dumps({'status': 'UNQUALIFIED' if unqualified else 'MISMATCH',
                          'code': safe_error(error, 'qualify.py'), **POLICY['claims']}))
        return 2 if unqualified else 1


if __name__ == '__main__':
    sys.exit(main())
