#!/usr/bin/env python3
"""Read-only stdlib comparator for paired, instrumented unchanged originals.

Generated fields are accepted only through producer/receipt/request relations.
Root supplies independent custody; a recorder manifest is not attestation.
"""
import argparse
import base64
import datetime
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


def sample_bounds(sample):
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


def clock_interval(before, after):
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


def freshness(observed, before, after, fresh_generation=False):
    source = utc_ms(observed)
    lower, upper = clock_interval(before, after)
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
        self.commands[role] = {'start': start, 'result': result, 'cap': cap, 'raw': raw, 'native': graph}

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
            if set(start['fds']) != {'0', '1', '2'} or not all(re.fullmatch(r'pipe:\[[0-9]+\]', v) for v in start['fds'].values()):
                uncertain('native_stdio_fd_role_unavailable', path)
            snapshots = [s for s in command['cap']['processSnapshots'] if s['pid'] == pid and s.get('nativeArgv') == start['nativeArgv']]
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
            for i, e in enumerate(events, 1):
                require_same(pid, e['pid'], 'event-pid'); require_same(i, e['sequence'], 'event-sequence')
                clock_interval(command['start']['prelaunch'], e['sample'])
                clock_interval(e['sample'], command['result']['ended'])
            first = [e for e in events if e['kind'] == 'entry']
            if len(first) != 1:
                refuse('process_entry_observation_missing', path)
            require_same(entry, first[0]['path'], 'entry-path')
            require_same(allowed_entry, first[0]['sha256'], 'entry-source-pin')
            lifetime_before, lifetime_after = command['start']['prelaunch'], command['result']['ended']
            clock_interval(lifetime_before, start['sample']); clock_interval(start['sample'], lifetime_after)
            exits = f'processes/{pid}/EXIT.json'
            if exits in self.manifest:
                ex = self.json(exits)
                clock_interval(start['sample'], ex['sample'])
                clock_interval(ex['sample'], command['result']['ended'])
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
                                   'before': lifetime_before, 'after': lifetime_after}
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
                    p['osScope'] = {n: ('inherited_shared_command_pipe' if p['start']['fds'][str(i)] == top[str(i)]
                                        else 'js_emit_or_write_tap') for i, n in [(1, 'stdout'), (2, 'stderr')]}

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


def event_bytes(event):
    try:
        b = base64.b64decode(event['data'], validate=True)
    except (ValueError, TypeError):
        refuse('event_base64', 'events')
    require_same(len(b), event['bytes'], 'event-bytes')
    require_same(sha(b), event['sha256'], 'event-digest')
    return b


def native_trace(traces):
    execs, parents, terminals, history, threads = {}, {}, {}, [], {}
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
                execs[pid] = {'argv': argv, 'executable': executable}
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
                terminals[pid] = int(re.search(r'\+\+\+ exited with ([0-9]+)', line).group(1))
            if '+++ killed by ' in line:
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
    return {'execs': execs, 'execHistory': history, 'parents': parents, 'terminals': terminals}


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
            calls.append({'request': req, 'before': f['sample'], 'after': reply['sample'], 'result': reply['value'].get('result'), 'raw': reply['raw']})
    return calls


def request_receipts(capture, command, request, reply):
    direct = command['direct']
    lo = request['requestSendBracket']['before']
    hi = reply['responseReceiveBracket']['after'] if reply else command['result']['ended']
    lo_m, hi_m = sample_bounds(lo)[1], sample_bounds(hi)[2]
    found = {}
    for e in direct['events']:
        if e['kind'] != 'file_write' or not e.get('success'):
            continue
        m = re.fullmatch(re.escape(str(capture.root)) + r'/receipts/(' + UUID.pattern[:-2] + r')-(dispatch|result|failure)\.json', e['path'])
        if not m or not (lo_m <= sample_bounds(e['sample'])[1] <= hi_m):
            continue
        occurrence, phase = m.groups()
        if phase in found:
            refuse('serial_receipt_window_collision', command['direct']['role'])
        data = event_bytes(e)
        require_same(data, capture.raw('receipts/' + occurrence + '-' + phase + '.json'), 'successful-write-raw')
        found[phase] = {'occurrence': occurrence, 'body': parse(data), 'raw': data, 'event': e}
    return found


def upstream_in_window(capture, command, before, after):
    lower, upper = sample_bounds(before)[1], sample_bounds(after)[2]
    result = []
    for pid, process in capture.processes.items():
        if process['role'] != command['direct']['role'] or process is command['direct']:
            continue
        for ordinal, call in enumerate(source_calls(process)):
            lo = sample_bounds(call['before'])[1]
            if lower <= lo <= upper:
                result.append((pid, ordinal, process, call))
    result.sort(key=lambda x: sample_bounds(x[3]['before'])[1])
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
                for e in process['events']:
                    if e['kind'] == 'file_write' and e['path'] == str(capture.root / 'fault-events.ndjson'):
                        event = parse(event_bytes(e))
                        require_same(process['start']['pid'], event['pid'], 'hung-handler-pid')
                        if event.get('event') == 'hung' and sample_bounds(e['sample'])[1] >= sample_bounds(call['before'])[1]:
                            hung.append(event)
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
        freshness(attribution['observedAt'], command['start']['prelaunch'], call['before'], True)
        request_roles[p + ('observedAt',)] = label + ':attribution-time'
    input_tree = LexicalJSON(js_bytes(request), request_roles).tree()
    result = call['result']
    if result is None:
        return {'input': input_tree, 'output': None, 'effect': 'unknown'}
    atoms = Atoms(label)
    tool = request['params']['name']
    audit_roles(result, request, call['before'], call['after'], atoms, label)
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


def compare_process_material(a, b):
    for role in a.commands:
        left = sorted([p for p in a.processes.values() if p['role'] == role], key=lambda p: sample_bounds(p['start']['sample'])[1])
        right = sorted([p for p in b.processes.values() if p['role'] == role], key=lambda p: sample_bounds(p['start']['sample'])[1])
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
