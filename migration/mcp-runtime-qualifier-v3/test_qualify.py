"""Pure stdlib semantic/structural tests; no original import or subprocess.

Fixtures are fictional and retained in a unique author-notes directory. Original
frozen public bytes are read only as independent anchors, never regenerated.
"""
import copy
import ast
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from contextlib import contextmanager
from unittest.mock import patch

import qualify as q

NOTES = Path('/home/palantirkc/work/ws-replan/preparation/slices/S11/qualifier-v3/native')
V1 = q.HERE.parents[1] / 'migration/mcp-runtime-reference-v1'
ORIGINAL_RAW = Path('/home/palantirkc/work/ws-replan/preparation/slices/S11/reference-v1/native')
U1 = '12345678-1234-4234-8234-123456789abc'
U2 = '23456789-1234-4234-8234-123456789abc'
T = '2026-11-03T00:00:00.000Z'
W = q.utc_ms(T) * 1_000_000


FIXTURE_CLOCK = q.ClockDomain('recorder', 'fictional-only', 99, 99, '')


def stamp(wall, mono):
    return q.own_sample({'wallNs': wall, 'monoBeforeNs': mono, 'monoAfterNs': mono + 100}, FIXTURE_CLOCK)


def bun_raw(wall, mono):
    return {'wallNs': str(wall), 'monoNs': str(mono + 100), 'monoBeforeNs': str(mono),
            'monoAfterNs': str(mono + 100), 'wallResolutionNs': 1000000}



BEFORE = stamp(W - 1_000_000, 10_000_000)
AFTER = stamp(W + 1_000_000_000, 1_011_000_000)


def result(data):
    return {'content': [{'type': 'text', 'text': q.js_bytes(data).decode()}], 'structuredContent': data}


def normal_fixture(large=False, identity=False):
    req = {'jsonrpc': '2.0', 'id': 20, 'method': 'tools/call', 'params': {
        'name': 'academy_runtime_identity' if identity else 'academy_source_status', 'arguments': {}}}
    data = {'status': 'observed', 'text': 'x' * (600000 if large else 4)}
    original = result(data)
    wire = copy.deepcopy(original)
    dispatch = {'occurrence': U1, 'body': {'issuedAt': T, 'tool': req['params']['name'],
        'argumentsSha256': q.sha(b'{}'), 'automaticRetryAllowed': False, 'academyWrites': 0, 'parentSends': 0}}
    body = {'occurrence': U1, 'phase': 'release' if identity else 'release_readback', 'endpoint': None if identity else 'dailyrecord',
            'upstreamName': None if identity else 'academy_source_status', 'generation': None, 'pointerSha256': None,
            'elapsedMs': 1, 'upstreamOccurrence': None, 'completedAt': T, 'tool': req['params']['name'],
            'status': 'identity_observed' if identity else 'observed', 'resultSha256': q.sha(q.js_bytes(data if identity else original)),
            'automaticRetryAllowed': False}
    if not identity:
        body.update(isError=False, sourceAvailability=None, steps=[], failure=None, academyWrites=0, parentSends=0)
    receipts = {'dispatch': dispatch, 'result': {'occurrence': U1, 'body': body}}
    if not large and not identity:
        wire['_meta'] = {'academy/bridgeAudit': {'occurrence': U1, 'endpoint': 'dailyrecord', 'upstreamName': 'academy_source_status',
                                               'generation': None, 'elapsedMs': 2}}
    return req, original, wire, receipts


@contextmanager
def historical_observer_source(root):
    """Test-only source context for immutable old857 pure fixtures, never admission."""
    if root not in [NOTES/'protocol-A-01', NOTES/'protocol-B-01']:
        raise AssertionError('historical_fixture_root_unsupported')
    custody = q.parse((NOTES.parent/'LEAD-PROTOCOL-CUSTODY-03.json').read_bytes())
    historical = root/'observer-source'
    digest = q.sha((historical/'preload.ts').read_bytes())
    if digest != '85799fe454c284d84460d092cc466526fe3620137eb1026d25c0114b16edc3a2' or digest != custody['qualifierFiles']['preload.ts']:
        raise AssertionError('historical_fixture_observer_pin_mismatch')
    with patch.object(q, 'HERE', historical):
        yield


class PureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        configured = Path(os.environ.get('S11_TEST_ROOT', str(NOTES / 'pure-fixtures')))
        if not configured.is_absolute() or not configured.resolve().is_relative_to(NOTES):
            raise RuntimeError('test_root_outside_allocation@test_qualify.py:1')
        configured.mkdir(parents=True, exist_ok=True)
        cls.dir = Path(tempfile.mkdtemp(prefix='stdlib-', dir=configured))
        (cls.dir / 'PURPOSE.json').write_text(json.dumps({'fictional': True, 'runtimeSubprocesses': 0, 'originalExecution': False}) + '\n')

    def check_bridge(self, fixture, **kw):
        req, original, wire, receipts = fixture
        atoms = q.Atoms('neutral:0')
        q.bridge_roles(req, wire, receipts, BEFORE, AFTER, atoms, 'neutral:0', original, **kw)
        return atoms

    def rejected(self, code, fn, *args, **kw):
        with self.assertRaisesRegex(q.Refusal, code):
            fn(*args, **kw)

    def test_01_json_duplicate_nonfinite_and_types(self):
        for data in [b'{"a":1,"a":2}', b'NaN', b'1e999', b'{"x":Infinity}']:
            self.rejected('json', q.parse, data)
        self.assertFalse(q.same(True, 1)); self.assertFalse(q.same(1, 1.0))
        self.assertFalse(q.same([1, 2], [2, 1])); self.assertFalse(q.same({}, {'x': None}))

    def test_02_wire_outer_order_only(self):
        q.wire_equal(b'{"a":1,"b":2}\n', b'{"b":2,"a":1}\n')
        for changed in [b'{"a":1.0,"b":2}\n', b'{"a": 1,"b":2}\n', b'{"a":1,"b":2}\r\n', b'{"a":1,"b":2}']:
            self.rejected('wire_token', q.wire_equal, b'{"a":1,"b":2}\n', changed)

    def test_03_wire_escape_and_literal_inner_json(self):
        self.rejected('wire_token', q.wire_equal, b'{"x":"a"}', b'{"x":"\\u0061"}')
        a = q.js_bytes({'content': [{'type': 'text', 'text': '{"a":1,"b":2}'}]})
        b = q.js_bytes({'content': [{'type': 'text', 'text': '{"b":2,"a":1}'}]})
        self.rejected('wire_token', q.wire_equal, a, b)

    def test_04_generated_text_spans_preserve_other_characters(self):
        def frame(u, text='literal'):
            return {'jsonrpc': '2.0', 'id': 1, 'result': result({'occurrence': u, 'prose': text})}
        aa, bb = q.Atoms('a'), q.Atoms('b')
        for a, u in [(aa, U1), (bb, U2)]:
            a.occurrence(('structuredContent', 'occurrence'), u, 'request:1:bridge')
        a, b = frame(U1), frame(U2)
        ra, ta = aa.result_wire(a); rb, tb = bb.result_wire(b)
        q.wire_equal(q.js_bytes(a), q.js_bytes(b), ra, rb, ta, tb)
        changed = frame(U2, 'literal ' + U2)
        self.rejected('wire_token', q.wire_equal, q.js_bytes(a), q.js_bytes(changed), ra, rb, ta, tb)
        changed = frame(U2)
        changed['result']['content'][0]['text'] = changed['result']['content'][0]['text'].replace(',"prose"', ', "prose"')
        self.rejected('wire_token', q.wire_equal, q.js_bytes(a), q.js_bytes(changed), ra, rb, ta, tb)

    def test_05_dual_copy_mutation_rejected(self):
        a = q.Atoms('bridge'); a.occurrence(('structuredContent', 'occurrence'), U1, 'bridge')
        frame = {'result': result({'occurrence': U1})}; frame['result']['structuredContent']['occurrence'] = U2
        self.rejected('dual_representation', a.result_wire, frame)

    def test_06_source_serializer_order_and_explicit_subset(self):
        self.assertEqual(q.js_bytes({'b': 2, 'a': 1}), b'{"b":2,"a":1}')
        for v in [1.0, 2**53, {'0': 'x'}, '\ud800']:
            self.rejected('unsupported_js', q.js_bytes, v)
        self.assertEqual(q.js_bytes({'x': '\n한글', 'a': [True, None, 1]}), b'{"x":"\\n\xed\x95\x9c\xea\xb8\x80","a":[true,null,1]}')

    def test_07_age_arithmetic_source_edges(self):
        self.assertEqual([q.age_predicate(i) for i in [-1, 0, 300000, 300001]], [False, True, True, False])
        self.rejected('age_type', q.age_predicate, True)

    def test_08_general_clock_interval_ambiguity_is_unqualified(self):
        for lo, hi in [(-1, 1), (300000, 300001)]:
            a, b = stamp(W + lo * 1000000, 10000000), stamp(W + hi * 1000000, 10000000 + (hi-lo)*1000000)
            with self.assertRaisesRegex(q.Unqualified, 'clock_interval_ambiguous'):
                q.freshness(T, a, b)
        # A source may accept at exactly +300000 and the reply arrive later.
        # The external interval may withhold proof; it must not assert stale.

    def test_09_fresh_causal_generation_and_stable_clock(self):
        q.freshness(T, BEFORE, AFTER, True)
        with self.assertRaisesRegex(q.Unqualified, 'clock_observation_unstable'):
            q.freshness(T, AFTER, BEFORE, True)
        stepped = stamp(W + 100_000_000_000, 11_000_000)
        with self.assertRaisesRegex(q.Unqualified, 'clock_observation_unstable'):
            q.freshness(T, BEFORE, stepped, True)

    def test_10_coherent_timestamp_mutants_keep_local_hashes(self):
        anchor = {'observedAt': T, 'sourceRevision': 'a'*64, 'wireDigest': 'b'*64, 'transmissionState': 'not_sent'}
        (self.dir / 'producer.raw').write_bytes(q.js_bytes(anchor))
        for milliseconds in [-1, 300001, -300001]:
            candidate = dict(anchor)
            candidate['observedAt'] = (q.datetime.datetime.fromtimestamp((q.utc_ms(T)+milliseconds)/1000, q.datetime.timezone.utc).isoformat(timespec='milliseconds').replace('+00:00','Z'))
            wire = result({'source': candidate})
            digest = q.sha(q.js_bytes(wire))
            self.assertEqual(digest, q.sha(q.js_bytes(wire)))
            # Both returned copies and the local digest are internally consistent.
            self.assertEqual(q.parse(wire['content'][0]['text']), wire['structuredContent'])
            self.rejected('independent_original_source', q.check_propagated_source, anchor, candidate, BEFORE, AFTER)
        self.assertEqual((self.dir/'producer.raw').read_bytes(), q.js_bytes(anchor))

    def test_11_normal_receipts_and_independent_producer(self):
        fixture = normal_fixture(); atoms = self.check_bridge(fixture)
        self.assertIn(U1, atoms.uuid_values)
        # Local hashes are not sufficient to replace an unchanged producer.
        req, original, wire, receipts = copy.deepcopy(fixture)
        wire['structuredContent']['text'] = 'edited'
        wire['content'][0]['text'] = q.js_bytes(wire['structuredContent']).decode()
        receipts['result']['body']['resultSha256'] = q.sha(q.js_bytes(q.stripped_result(wire, 'academy/bridgeAudit')))
        self.rejected('independent_result_producer', q.bridge_roles, req, wire, receipts, BEFORE, AFTER, q.Atoms('x'), 'x', original)

    def test_12_occurrence_group_swap_with_equal_arguments(self):
        first, second = normal_fixture(), normal_fixture()
        for record in second[3].values():
            record['occurrence'] = U2
            if 'occurrence' in record['body']: record['body']['occurrence'] = U2
        second[2]['_meta']['academy/bridgeAudit']['occurrence'] = U2
        first[2]['_meta']['academy/bridgeAudit']['occurrence'] = U2
        # Same args digest in the two requests cannot override fixed receipt windows.
        self.rejected('bridge-meta-occurrence', self.check_bridge, first)

    def test_13_identity_has_receipts_without_body_occurrence(self):
        fixture = normal_fixture(identity=True)
        self.check_bridge(fixture)
        fixture[2]['structuredContent']['occurrence'] = U1
        self.rejected('identity_body_invented', self.check_bridge, fixture)

    def test_14_pre_dispatch_absence_is_a_specific_source_role(self):
        req, _, _, receipts = normal_fixture()
        wire = result({'status': 'blocked', 'occurrence': U1, 'elapsedMs': 1})
        q.bridge_roles(req, wire, {}, BEFORE, AFTER, q.Atoms('x'), 'x', pre_dispatch=True)
        self.rejected('receipt_in_pre_dispatch', q.bridge_roles, req, wire, receipts, BEFORE, AFTER, q.Atoms('x'), 'x', pre_dispatch=True)

    def test_15_missing_normal_and_invented_dispatch_field(self):
        fixture = normal_fixture(); del fixture[3]['dispatch']
        self.rejected('dispatch_receipt_missing', self.check_bridge, fixture)
        fixture = normal_fixture(); del fixture[3]['result']
        self.rejected('terminal_receipt_missing', self.check_bridge, fixture)
        fixture = normal_fixture(); fixture[3]['dispatch']['body']['occurrence'] = U1
        self.rejected('dispatch_body_invented', self.check_bridge, fixture)

    def test_16_large_cap_absence_requires_real_receipt(self):
        fixture = normal_fixture(large=True); self.check_bridge(fixture)
        del fixture[3]['result']; self.rejected('terminal_receipt_missing', self.check_bridge, fixture)
        fixture = normal_fixture(); fixture[2].pop('_meta')
        self.rejected('bridge_metadata_cap_role', self.check_bridge, fixture)

    def test_17_duration_roles_can_equal_or_differ(self):
        fixture = normal_fixture()
        self.check_bridge(fixture)
        fixture[2]['_meta']['academy/bridgeAudit']['elapsedMs'] = 1
        self.check_bridge(fixture)  # Equality is legitimate source behavior.
        fixture[3]['result']['body']['elapsedMs'] = 3
        self.rejected('elapsed_source_order', self.check_bridge, fixture)
        for value in [True, 1.0, -1, 999999]:
            self.rejected('elapsed_', q.elapsed, value, BEFORE, AFTER)

    def test_18_old_equalization_not_a_universal_rule(self):
        # Independent old raw had receipt790/meta791. Mutating metadata to790
        # violates that producer anchor, not an invented strict inequality.
        anchor = {'receiptElapsed': 790, 'metaElapsed': 791}
        mutant = {'receiptElapsed': 790, 'metaElapsed': 790}
        local = q.sha(q.js_bytes(mutant)); self.assertEqual(local, q.sha(q.js_bytes(mutant)))
        self.rejected('literal_mismatch', q.require_same, anchor, mutant, 'independent-elapsed-anchor')

    def test_19_hung_unknown_and_actual_native_terminal(self):
        req, original, wire, receipts = normal_fixture(); receipts.pop('result')
        q.bridge_roles(req, None, receipts, BEFORE, AFTER, q.Atoms('x'), 'x', hung=True)
        q.validate_terminal('neutral', -15, [-15, 0], [], True)
        for rc in [0, 15, -9]:
            self.rejected('hung_native_signal', q.validate_terminal, 'neutral', rc, [-15], [], True)
        with self.assertRaises(q.Unqualified): q.validate_terminal('neutral', 0, [], [])
        with self.assertRaises(q.Unqualified): q.validate_terminal('neutral', -15, [-15], [{'pid': 1}], True)

    def test_20_outer_zero_never_hides_child23(self):
        self.rejected('native_child_terminal', q.validate_terminal, 'discover', 0, [0, 23], [])
        q.validate_terminal('lock-release-readback', 0, [0, 0], [])
        q.validate_terminal('tampered-bridge', 1, [1, 0], [])

    def test_21_complete_native_exec_history_and_signal(self):
        trace = {10: '1000.000 execve("/usr/bin/flock", ["/usr/bin/flock", "--nonblock", "/fictional/lock", "/usr/bin/true"], []) = 0\n1000.001 execve("/usr/bin/true", ["/usr/bin/true"], []) = 0\n1000.002 +++ exited with 0 +++\n',
                 11: '1000.004 +++ killed by SIGTERM +++\n'}
        graph = q.native_trace(trace)
        self.assertEqual(len(graph['execHistory']), 2)
        self.assertEqual(graph['execs'][10]['argv'], ['/usr/bin/true'])
        self.assertEqual(graph['terminals'], {10: 0, 11: -15})
        with self.assertRaises(q.Unqualified): q.native_trace({12: 'execve("bun", ["bun", ...], []) = 0\n'})

    def test_22_root_component_and_no_arbitrary_locator_reads(self):
        root = self.dir
        q.rooted(str(root/'events.txt'), root, 'events.txt')
        for path in [str(root)+'-other/events.txt', str(root/'child/../events.txt'), '/private/events.txt']:
            self.rejected('root_component', q.rooted, path, root, 'events.txt')
        (root/'link').symlink_to('/etc/passwd')
        self.rejected('symlink_capture', q.read, root, 'link')
        self.rejected('unsafe_relative', q.read, root, '../passwd')

    def test_23_manifest_is_not_external_custody(self):
        (self.dir/'MANIFEST.json').write_text('{}\n')
        with self.assertRaisesRegex(q.Unqualified, 'external_raw_custody_missing'):
            q.Capture(self.dir)
        self.rejected('external-manifest', q.Capture, self.dir, '0'*64)
        with self.assertRaisesRegex(q.Unqualified, 'external_git_raw_custody_missing'):
            q.qualify_pair(self.dir, self.dir, 'protocol', None)

    def test_24_original_source_pins_and_full_mode_requirement(self):
        self.assertEqual(len(q.PINS['packages']), 14)
        self.assertEqual(sum(len(p['files']) for p in q.PINS['packages'].values()), 1070)
        for relative, pin in q.PINS['sources'].items():
            data = q.read(q.HERE.parents[1], relative)
            self.assertEqual(pin, {'sha256': q.sha(data), 'bytes': len(data)}, relative)
        self.assertEqual(len(q.POLICY['modes']), 10)
        self.assertFalse(any(q.POLICY['claims'].values()))
        self.assertNotIn('minimumSeconds', json.dumps(q.POLICY))

    def test_25_frozen_audit_extension_and_omitted_arguments(self):
        calls = q.expected_requests('protocol', 'neutral')
        by_id = {r['id']: r for r in calls}
        self.assertNotIn('arguments', by_id[4]['params'])
        self.assertEqual(by_id[20]['params']['arguments'], by_id[22]['params']['arguments'])
        self.assertNotEqual(by_id[20]['params']['arguments'], by_id[21]['params']['arguments'])
        self.assertEqual([r['id'] for r in calls][-3:], [20, 21, 22])

    def test_26_actual_original_seven_preimages(self):
        count = 0
        for mode, ids in [('protocol', [4, 8, 10]), ('composition', [2]), ('graph', [3, 7]), ('bounds', [2])]:
            observations = q.parse((V1/'frozen'/mode/'observations.json').read_bytes())
            for id in ids:
                response = next(o['response']['result'] for o in observations if o['id'] == id)
                occurrence = response['_meta']['academy/bridgeAudit']['occurrence']
                receipt = q.parse((ORIGINAL_RAW/('frozen-'+mode)/'receipts'/(occurrence+'-result.json')).read_bytes())
                # Independent retained raw preimage, not fresh execution evidence.
                pre = q.stripped_result(response, 'academy/bridgeAudit')
                self.assertEqual(q.sha(q.js_bytes(pre)), receipt['resultSha256'])
                count += 1
        self.assertEqual(count, 7)

    def test_27_argument_serialization_and_wire_order_are_different(self):
        a, b = {'x': 1, 'y': 2}, {'y': 2, 'x': 1}
        self.assertTrue(q.same(a, b)); self.assertNotEqual(q.sha(q.js_bytes(a)), q.sha(q.js_bytes(b)))
        q.wire_equal(q.js_bytes(a), q.js_bytes(b))

    def test_28_no_subprocess_on_import(self):
        with patch('subprocess.Popen', side_effect=AssertionError('forbidden test subprocess')):
            spec = importlib.util.spec_from_file_location('s11_record_pure_test', q.HERE/'record.py')
            module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
            self.assertTrue(callable(module.preflight))

    def test_29_audit_input_and_result_preimage(self):
        req = {'params': {'name': 'fictional', 'arguments': {'x': 1}}}
        original = result({'status': 'observed'})
        wire = copy.deepcopy(original)
        wire['_meta'] = {'academy/audit': {'occurrence': U2, 'inputSha256': q.sha(q.js_bytes({'x': 1})),
                                         'elapsedMs': 1, 'resultSha256': q.sha(q.js_bytes(original))}}
        q.audit_roles(wire, req, BEFORE, AFTER, q.Atoms('x'), 'x')
        wire['_meta']['academy/audit']['inputSha256'] = q.sha(q.js_bytes({'x': 2}))
        self.rejected('audit-input', q.audit_roles, wire, req, BEFORE, AFTER, q.Atoms('x'), 'x')

    def test_30_native_boundary_proofs_do_not_invent_js_exit_pid(self):
        capture = q.Capture.__new__(q.Capture)
        capture.root = self.dir/'binding'; capture.run = {'uid': os.getuid()}
        capture.recorder = q.ClockDomain('recorder', str(capture.root), 99, 99, '')
        entry = str(capture.root/'source/original.ts')
        argv = ['/usr/local/bin/bun', '--no-install', '--no-env-file', '--preload', str(capture.root/'observer-source/preload.ts'), entry]
        start = {'pid': 101, 'ppid': 100, 'pgid': 100, 'uid': os.getuid(), 'cwd': str(capture.root),
                 'nativeArgv': argv, 'argv': ['/usr/local/bin/bun', entry], 'execPath': '/usr/local/bin/bun',
                 'fds': {'0': 'pipe:[1]', '1': 'pipe:[2]', '2': 'pipe:[3]'}, 'sample': bun_raw(W, 11000000)}
        event = {'pid': 101, 'sequence': 1, 'kind': 'entry', 'sample': bun_raw(W,11000000), 'path': entry, 'sha256': 'f'*64}
        raw = {'processes/101/START.json': json.dumps(start).encode(), 'processes/101/events.ndjson': json.dumps(event).encode()+b'\n'}
        capture.manifest = {p: {} for p in raw}
        capture.processes = {}
        capture.raw = lambda p: raw[p]
        capture.json = lambda p: q.parse(raw[p])
        capture.entry_pin = lambda p: 'f'*64
        command = {'start': {'pid': 100, 'pgid': 100, 'originalArgv': argv, 'prelaunch': q.own_sample(dict(BEFORE), capture.recorder)},
                   'result': {'ended': q.own_sample(dict(AFTER), capture.recorder)}, 'cap': {'processSnapshots': []},
                   'native': {'execs': {101: {'argv': argv, 'wallCell': (W-500000, W-499000)}},
                              'parents': {101: 100}, 'terminals': {101: -15},
                              'terminalClocks': {101: (W+900000000, W+900001000)}}}
        capture.commands = {'neutral': command}
        capture.bind_processes()
        self.assertEqual(capture.processes[101]['channels']['stderr']['scope'], 'js_no_event')
        self.assertEqual(capture.processes[101]['osScope']['stdout'], 'inherited_shared_command_pipe')
        # No JS EXIT file exists, while native SIGTERM is independently present.
        for key, changed, code in [('pgid', 999, 'process-pgid'), ('ppid', 999, 'native-ancestry'),
                                    ('nativeArgv', argv[0:1]+argv[2:], 'complete-native-argv')]:
            mutant = copy.deepcopy(start); mutant[key] = changed
            raw['processes/101/START.json'] = json.dumps(mutant).encode()
            self.rejected(code, capture.bind_processes)
        raw['processes/101/START.json'] = json.dumps(start).encode()
        raw['processes/101/stderr.bin'] = b''; capture.manifest['processes/101/stderr.bin'] = {}
        self.rejected('manufactured_empty_tap', capture.bind_processes)

    def actual_clock_fixture(self):
        # Exact first failed observation, read-only from immutable original A.
        # This is a focused clock-unit regression, not Capture/qualify_pair.
        root = NOTES/'protocol-A-01'
        prefix = root/'commands/discover'
        start = q.parse((prefix/'START.json').read_bytes())
        end = q.parse((prefix/'RESULT.json').read_bytes())
        pid = 3437440
        source = q.parse((root/f'processes/{pid}/START.json').read_bytes())
        event = q.parse((root/f'processes/{pid}/events.ndjson').read_bytes().splitlines()[0])
        exit_ = q.parse((root/f'processes/{pid}/EXIT.json').read_bytes())
        graph = q.native_trace({pid: (prefix/f'syscalls.{pid}').read_text()})
        recorder_pid = q.parse((root/'SUPERVISOR.json').read_bytes())['pid']
        recorder = q.ClockDomain('recorder', str(root), recorder_pid, recorder_pid, '')
        start['prelaunch'] = q.own_sample(start['prelaunch'], recorder)
        end['ended'] = q.own_sample(end['ended'], recorder)
        command = {'start': start, 'result': end, 'native': graph}
        native = q.NativeClock(str(root), recorder, command, source)
        return native, source, event, exit_, command

    def test_40_real_python_and_bun_epochs_are_distinct(self):
        native, source, event, exit_, command = self.actual_clock_fixture()
        lo, hi = command['start']['prelaunch'], command['result']['ended']
        source_sample = q.own_sample(source['sample'], native.owner)
        sample = q.own_sample(event['sample'], native.owner)
        terminal = q.own_sample(exit_['sample'], native.owner)
        self.assertEqual(lo['monoBeforeNs'], 837261896152681)
        self.assertEqual(sample['monoBeforeNs'], '97290474')
        self.assertLess(int(sample['monoAfterNs']), lo['monoBeforeNs'])
        self.assertTrue(native.contains(sample, lo, hi))
        q.clock_interval(source_sample, sample); q.clock_interval(sample, terminal)
        q.freshness(exit_['at'], lo, hi, True)
        # Explicit causal bound may cross owners; the generic API may not.
        q.clock_interval(lo, sample, native)
        self.rejected('clock_owner_mismatch', q.clock_interval, lo, sample)
        q.freshness('2026-11-03T05:44:58.970Z', lo, sample, True, native=native)

    def test_41_missing_wrong_owner_and_native_domain_refused(self):
        native, source, event, _, command = self.actual_clock_fixture()
        lo, hi = command['start']['prelaunch'], command['result']['ended']
        self.rejected('clock_domain_missing', q.clock_interval, dict(lo), dict(hi))
        self.rejected('clock_domain_missing', native.contains, event['sample'], lo, hi)
        other = q.ClockDomain('bun', native.owner.capture, native.owner.pid + 1,
                              native.owner.command, native.owner.exec_sha256)
        wrong = q.own_sample(event['sample'], other)
        self.rejected('clock_owner_mismatch', native.contains, wrong, lo, hi)
        self.rejected('clock_owner_mismatch', q.own_sample, wrong, native.owner)
        missing = copy.deepcopy(command); missing['native']['terminalClocks'] = {}
        self.rejected('clock_native_domain_missing', q.NativeClock,
                      native.owner.capture, native.recorder, missing, source)
        wrong = copy.deepcopy(command); wrong['native']['execs'][source['pid']]['argv'] = ['wrong']
        self.rejected('clock-native-exec', q.NativeClock,
                      native.owner.capture, native.recorder, wrong, source)

    def test_42_outside_and_ambiguous_cross_domain_windows_refuse(self):
        native, _, event, _, command = self.actual_clock_fixture()
        lo, hi = command['start']['prelaunch'], command['result']['ended']
        sample = q.own_sample(event['sample'], native.owner)
        # A well-formed recorder window, wholly after the source observation.
        delta = 500000000
        later = q.own_sample({k: (v + delta if k != 'wallResolutionNs' else v)
                              for k, v in dict(lo).items()}, native.recorder)
        self.rejected('clock_sample_outside_window', native.contains, sample, later, hi)
        self.assertFalse(native.contains(sample, later, hi, selection=True))
        # The original Date sample's 1ms cell overlaps a precise outer edge.
        delta = int(sample['wallNs']) - lo['wallNs'] + 500000
        edge = q.own_sample({k: (v + delta if k != 'wallResolutionNs' else v)
                             for k, v in dict(lo).items()}, native.recorder)
        self.rejected('clock_window_boundary_ambiguous', native.contains, sample, edge, hi)
        late = dict(event['sample']); late['wallNs'] = str(hi['wallNs'] + 1000000000)
        self.rejected('clock_sample_outside_window', native.contains,
                      q.own_sample(late, native.owner), lo, hi)

    def test_43_same_owner_reversal_step_and_wrong_shape_still_refuse(self):
        native, source, event, _, _ = self.actual_clock_fixture()
        first = q.own_sample(source['sample'], native.owner)
        last = q.own_sample(event['sample'], native.owner)
        self.rejected('clock_observation_unstable', q.clock_interval, last, first)
        stepped = dict(event['sample']); stepped['wallNs'] = str(int(stepped['wallNs']) + 1000000000)
        self.rejected('clock_observation_unstable', q.clock_interval,
                      first, q.own_sample(stepped, native.owner))
        self.rejected('clock_domain_sample_shape', q.own_sample, dict(BEFORE), native.owner)
        # Native trace timestamp precision is observed, not an invented tolerance.
        self.assertEqual(q.native_wall_cell('1793684698.857369 execve('),
                         (1793684698857369000, 1793684698857370000))

    def test_44_cross_process_order_uses_wall_and_native_exec(self):
        a = q.ClockDomain('bun', 'fictional-only', 101, 100, 'a'*64)
        b = q.ClockDomain('bun', 'fictional-only', 102, 100, 'b'*64)
        first = q.own_sample(bun_raw(W, 900000000), a)
        second = q.own_sample(bun_raw(W+10000000, 1), b)
        self.assertEqual(q.clock_order(first, second), -1)
        self.rejected('clock_cross_process_order_ambiguous', q.clock_order,
                      first, q.own_sample(bun_raw(W, 1), b))
        # Process pairing uses native exec order, not incomparable local epochs.
        class C: pass
        capture = C()
        capture.processes = {101: {'role':'neutral', 'clock': C()}, 102: {'role':'neutral', 'clock': C()}}
        capture.processes[101]['clock'].exec_cell = (10, 11)
        capture.processes[102]['clock'].exec_cell = (20, 21)
        self.assertEqual(q.native_process_order(capture, 'neutral'), list(capture.processes.values()))
        capture.processes[102]['clock'].exec_cell = (11, 12)
        self.rejected('native_exec_order_ambiguous', q.native_process_order, capture, 'neutral')

    def actual_fd_fixture(self):
        root = NOTES/'protocol-A-01'
        parent_pid, child_pid = 3437440, 3437449
        parent = q.parse((root/f'processes/{parent_pid}/START.json').read_bytes())
        child = q.parse((root/f'processes/{child_pid}/START.json').read_bytes())
        prefix = root/'commands/discover'
        parent_trace = (prefix/f'syscalls.{parent_pid}').read_text()
        child_trace = (prefix/f'syscalls.{child_pid}').read_text()
        snapshots = q.parse((prefix/'CAPTURE.json').read_bytes())['processSnapshots']
        graph = q.native_trace({parent_pid:parent_trace, child_pid:child_trace})
        command = {'native':graph, 'start':{'pgid':parent['pgid']},
                   'traces':{parent_pid:parent_trace, child_pid:child_trace}}
        class C: pass
        capture = C(); capture.root = root
        capture.raw = lambda relative: (root/relative).read_bytes()
        capture.json = lambda relative: q.parse(capture.raw(relative))
        return capture, command, parent, child, snapshots, parent_trace, child_trace

    def test_45_actual_declared_bun_socket_stdio_is_local_ipc(self):
        capture, command, parent, child, snapshots, pt, ct = self.actual_fd_fixture()
        roles = q.declared_socket_stdio(capture, command, child, snapshots)
        self.assertEqual(child['fds'], {'0':'socket:[105528427]', '1':'socket:[105528430]', '2':'socket:[105528432]'})
        self.assertEqual(roles, {'0':'declared_local_ipc_input', '1':'declared_local_ipc_output',
                                 '2':'declared_local_ipc_diagnostic'})
        self.assertEqual(q.stdio_scope(child['fds'], parent['fds']),
                         {'stdout':'js_emit_or_write_tap', 'stderr':'js_emit_or_write_tap'})
        self.assertEqual(q.stdio_scope(parent['fds'], parent['fds']),
                         {'stdout':'inherited_shared_command_pipe', 'stderr':'inherited_shared_command_pipe'})

    def test_46_wrong_socket_family_dup_and_missing_native_bindings_refuse(self):
        _, _, parent, child, snapshots, pt, ct = self.actual_fd_fixture()
        self.rejected('native_stdio_socketpair_family_unsupported', q.socket_stdio_binding,
                      pt.replace('socketpair(AF_UNIX,', 'socketpair(AF_INET,'), ct, parent, child, snapshots)
        self.rejected('native_stdio_dup_binding_wrong', q.socket_stdio_binding,
                      pt, ct.replace('dup2(7, 0)', 'dup2(8, 0)'), parent, child, snapshots)
        lines = ct.splitlines()
        lines[0], lines[1] = lines[1], lines[0]
        self.rejected('native_stdio_child_fd_sequence_unsupported', q.socket_stdio_binding,
                      pt, '\n'.join(lines), parent, child, snapshots)
        self.rejected('native_stdio_vfork_binding_missing', q.socket_stdio_binding,
                      pt.replace('vfork()', 'unsupported_fork()'), ct, parent, child, snapshots)
        self.rejected('native_stdio_independent_readback_missing', q.socket_stdio_binding,
                      pt, ct, parent, child, [])
        self.rejected('native_stdio_socketpair_binding_missing', q.socket_stdio_binding,
                      pt.replace('socketpair(', 'unsupported_pair('), ct, parent, child, snapshots)
        self.rejected('native_stdio_inode_binding_missing', q.socket_stdio_binding,
                      pt, ct.replace('/proc/self/fd/1', '/proc/self/fd/9'), parent, child, snapshots)

    def test_47_consistent_inode_mutant_and_shared_role_cannot_bypass_native_anchor(self):
        capture, command, parent, child, snapshots, pt, ct = self.actual_fd_fixture()
        mutant = copy.deepcopy(child); mutant['fds']['1'] = 'socket:[999999999]'
        changed = copy.deepcopy(snapshots)
        for x in changed:
            if x['pid'] == child['pid']: x['fds'] = copy.deepcopy(mutant['fds'])
        # START and independent snapshot agree locally; unchanged native readlink
        # remains the separate anchor and must exercise the FD semantic refusal.
        self.rejected('native_stdio_inode_binding_wrong', q.socket_stdio_binding,
                      pt, ct, parent, mutant, changed)
        mutant = copy.deepcopy(parent); mutant['fds'] = copy.deepcopy(child['fds'])
        self.rejected('native_stdio_fresh_pipe_role_missing', q.socket_stdio_binding,
                      pt, ct, mutant, child, snapshots)
        missing = copy.deepcopy(command); missing['traces'].pop(child['pid'])
        self.rejected('native_stdio_trace_binding_missing', q.declared_socket_stdio,
                      capture, missing, child, snapshots)
        unsupported = copy.deepcopy(command)
        unsupported['native']['execHistory'] = []
        self.rejected('native_stdio_endpoint_role_missing', q.declared_socket_stdio,
                      capture, unsupported, child, snapshots)
        # FD provenance does not manufacture an observed stream channel/file.
        self.assertEqual(q.stdio_scope(child['fds'], parent['fds'])['stderr'], 'js_emit_or_write_tap')

    @classmethod
    def retained_capture(cls, label):
        if not hasattr(cls, 'native_captures'):
            cls.native_captures = {}
        if label not in cls.native_captures:
            root = NOTES/f'protocol-{label}-01'
            custody = q.parse((NOTES.parent/'LEAD-PROTOCOL-CUSTODY-03.json').read_bytes())
            # Old custody supplies raw anchors and the historical observer source pin,
            # never admission of old bytes as current763 runtime/pair evidence.
            class HistoricalFixtureCapture(q.Capture):
                def check_source_mirror(self):
                    # Only this pure helper reads historical observer files. Constructor
                    # sourceRoot/STIMULUS checks retain the real repository HERE.
                    with historical_observer_source(self.root):
                        super().check_source_mirror()
            with patch('subprocess.Popen', side_effect=AssertionError('forbidden subprocess')):
                cls.native_captures[label] = HistoricalFixtureCapture(root, custody['captures'][str(root)], custody['supervisors'][str(root)])
        return cls.native_captures[label]

    def receipt_excerpt(self):
        capture = self.retained_capture('A');command=capture.commands['neutral']
        pid=command['direct']['start']['pid'];trace=command['traces'][pid]
        proof=q.parse((NOTES/'receipt-diagnosis-proof-01/PROOF.json').read_bytes())['captures']['A']['records'][0]
        lines=trace.splitlines()[proof['openLine']-1:proof['closeLine']]
        return capture,command,proof,'\n'.join(lines)+'\n'

    def test_48_actual_all56_native_receipt_positives(self):
        count=0
        for label in ['A','B']:
            capture=self.retained_capture(label);command=capture.commands['neutral']
            records=q.native_receipt_records(capture,command)
            self.assertEqual(len(records),28)
            self.assertEqual({r['requestOrdinal'] for r in records},set(range(2,13))|{14,15,16})
            for record in records:
                with self.subTest(run=label,phase=record['phase'],ordinal=record['requestOrdinal']):
                    proof=record['nativeProof'];self.assertEqual(proof['scope'],'native_successful_receipt_chain')
                    self.assertNotIn('sample',proof);self.assertNotIn('event',record)
                    self.assertEqual(record['raw'],capture.raw('receipts/'+Path(proof['path']).name))
                    self.assertEqual(q.sha(record['raw']),q.sha(proof['raw']))
                    count+=1
            request=command['cap']['requests'][2]
            reply=next(f for f in command['cap']['frames'] if f.get('value',{}).get('id')==2)
            result=q.request_receipts(capture,command,request,reply)
            self.assertEqual(set(result),{'dispatch','result'})
            self.assertFalse(any(e['kind']=='file_write' for e in command['direct']['events']))
        self.assertEqual(count,56)

    def test_49_native_receipt_truncation_partial_flags_fd_and_bytes(self):
        _,_,proof,trace=self.receipt_excerpt();path=str(NOTES/'protocol-A-01'/proof['file'])
        records=q.native_file_chains(trace,path,'receipt');self.assertEqual(len(records),1)
        mutations=[('native_write_truncated_or_unsupported',trace.replace('write(7, ','write(7, ...',1)),
                   ('native_write_partial_or_length',trace.replace(') = 223 ',') = 222 ',1)),
                   ('native_write_open_unsupported',trace.replace('|O_EXCL','',1)),
                   ('native_write_truncate_missing',trace.replace('ftruncate(7,','ftruncate(8,',1)),
                   ('native_write_close_missing',trace.replace('close(7)','close(8)',1)),
                   ('native_write_chain_incomplete','\n'.join(trace.splitlines()[:2]))]
        for code,mutant in mutations:
            with self.subTest(code=code):self.rejected(code,q.native_file_chains,mutant,path,'receipt')
        # A same-length payload change is a valid native chain but must not match
        # the unchanged independent retained bytes. This exercises semantic proof.
        changed=trace.replace('academy_runtime_identity','academy_runtime_identitx')
        raw=q.native_file_chains(changed,path,'receipt')[0]['raw']
        self.assertEqual(len(raw),proof['bytes'])
        capture=self.retained_capture('A');command=capture.commands['neutral']
        mutant,cmd=self.mutant_capture(capture,command);pid=cmd['direct']['start']['pid']
        cmd['traces'][pid]=cmd['traces'][pid].replace(trace,changed,1)
        self.rejected('native-receipt-retained-bytes',q.native_receipt_records,mutant,cmd)

    def mutant_capture(self, capture, command):
        mutant=copy.copy(capture);cmd=copy.copy(command)
        cmd['cap']=copy.deepcopy(command['cap']);cmd['traces']=dict(command['traces'])
        mutant.commands={**capture.commands,'neutral':cmd}
        return mutant,cmd

    def test_50_native_wrong_owner_second_writer_and_window_collision(self):
        capture,command,proof,trace=self.receipt_excerpt()
        wrong=copy.copy(command['direct']);wrong['start']={**wrong['start'],'pid':999999}
        self.rejected('native_write_owner_unavailable',q.native_writer_trace,capture,command,wrong)
        mutant,cmd=self.mutant_capture(capture,command);cmd['traces'][999999]=trace
        self.rejected('native_receipt_second_or_wrong_writer',q.native_receipt_records,mutant,cmd)
        mutant,cmd=self.mutant_capture(capture,command)
        cmd['cap']['requests'].append(copy.deepcopy(cmd['cap']['requests'][2]))
        self.rejected('native_receipt_request_window_collision',q.native_receipt_records,mutant,cmd)

    def test_51_native_js_agreement_and_complete_absence(self):
        capture=self.retained_capture('A');command=capture.commands['neutral']
        request=command['cap']['requests'][2];reply=next(f for f in command['cap']['frames'] if f.get('value',{}).get('id')==2)
        # Keep the actual native chains/bytes as anchors; a fabricated successful
        # JS record cannot replace a different native occurrence/phase.
        direct=command['direct'];before=q.source_calls(direct)[0]['before']
        dispatch=q.native_receipt_records(capture,command)
        dispatch=next(r for r in dispatch if r['requestOrdinal']==2 and r['phase']=='dispatch')
        import base64
        event={'kind':'file_write','success':True,'pid':direct['start']['pid'],
               'sample':before,'path':dispatch['nativeProof']['path'],'data':base64.b64encode(dispatch['raw']).decode(),
               'bytes':len(dispatch['raw']),'sha256':q.sha(dispatch['raw'])}
        direct['events'].append(event)
        try:
            agreed=q.request_receipts(capture,command,request,reply)
            self.assertIs(agreed['dispatch']['event'],event)
            with patch.object(q,'native_receipt_records',return_value=[]):
                self.rejected('native_js_receipt_disagreement',q.request_receipts,capture,command,request,reply)
        finally:direct['events'].pop()
        with patch.object(q,'native_receipt_records',return_value=[]):
            self.assertEqual(q.request_receipts(capture,command,request,reply),{})
        fixture=normal_fixture(identity=True)
        self.rejected('dispatch_receipt_missing',q.bridge_roles,fixture[0],fixture[2],{},BEFORE,AFTER,q.Atoms('missing'),'missing',fixture[1])

    def fictional_fault(self):
        import base64
        root=self.dir/'native-fault';pid=101;parent=100
        entry=str(root/'source/fictional-fault-fixture.ts')
        req={'jsonrpc':'2.0','id':6,'method':'tools/call','params':{'name':'academy_cism_weekly_save','arguments':{}}}
        raw=q.js_bytes(req)+b'\n';owner=q.ClockDomain('bun',str(root),pid,parent,'f'*64)
        event={'pid':pid,'sequence':1,'kind':'stdin_data','sample':q.own_sample(bun_raw(W,10),owner),
               'data':base64.b64encode(raw).decode(),'bytes':len(raw),'sha256':q.sha(raw)}
        eventraw=json.dumps(event,separators=(',',':')).encode()+b'\n'
        tool=q.js_bytes({'pid':pid,'endpoint':'dailyrecord','tool':'academy_cism_weekly_save','input':{}})+b'\n'
        hung=q.js_bytes({'pid':pid,'endpoint':'dailyrecord','event':'hung'})+b'\n'
        def chain(path,data,index,mode):
            sec=W//1000000000
            return [f'{sec}.{index:06d} openat(AT_FDCWD, {json.dumps(path)}, O_WRONLY|O_CREAT|O_APPEND|O_LARGEFILE|O_CLOEXEC, {mode}) = 7 <0.000001>',
                    f'{sec}.{index+2:06d} write(7, {json.dumps(data.decode())}, {len(data)}) = {len(data)} <0.000001>',
                    f'{sec}.{index+4:06d} close(7) = 0 <0.000001>']
        trace='\n'.join([f'{W//1000000000}.000001 execve("bun", ["bun"], []) = 0 <0.000001>',
            *chain(str(root/'processes/101/events.ndjson'),eventraw,100,'0600'),
            *chain(str(root/'fault-events.ndjson'),tool,200,'0666'),
            *chain(str(root/'fault-events.ndjson'),hung,300,'0666'),
            f'{W//1000000000}.001000 +++ killed by SIGTERM +++'])+'\n'
        class C:pass
        capture=C();capture.root=root;capture.mode='bounds'
        originals={'source/fictional-fault-fixture.ts':(q.HERE.parents[1]/q.STIMULUS['fixture']).read_bytes(),
                   'processes/101/events.ndjson':eventraw,'fault-events.ndjson':tool+hung}
        capture.raw=lambda path:originals[path]
        capture.entry_pin=lambda path:q.PINS['sources'][q.STIMULUS['fixture']]['sha256']
        clock=C();clock.owner=owner;clock.recorder_window=lambda a,b:(W-1000000,W+1000000000)
        start={'pid':pid,'ppid':parent,'pgid':parent,'nativeArgv':['bun',entry],
               'argv':['bun',entry,'dailyrecord']}
        process={'start':start,'entry':entry,'role':'neutral','clock':clock,'events':[event],
                 'channels':{'stdin':{'raw':raw,'events':[event]},'stdout':{'raw':None,'events':[]}}}
        command={'start':{'pid':parent,'pgid':parent},'native':{'execs':{pid:{'argv':start['nativeArgv']}},'terminals':{pid:-15},'parents':{pid:parent}},'traces':{pid:trace}}
        call={'request':req,'before':event['sample'],'after':None,'result':None}
        return capture,command,process,call,originals,tool,hung

    def test_52_declared_fault_append_and_pid_segment_order_negatives(self):
        capture,command,process,call,raw,tool,hung=self.fictional_fault()
        self.assertEqual(q.native_fault_events(capture,command,process,call,BEFORE,AFTER),
                         [{'pid':101,'endpoint':'dailyrecord','event':'hung'}])
        trace=command['traces'][101]
        command['traces'][101]=trace.replace('O_APPEND','O_EXCL')
        self.rejected('native_write_open_unsupported',q.native_fault_events,capture,command,process,call,BEFORE,AFTER)
        command['traces'][101]=trace
        raw['fault-events.ndjson']=tool+hung+hung
        self.rejected('native_fault_segment_ambiguous',q.native_fault_events,capture,command,process,call,BEFORE,AFTER)
        raw['fault-events.ndjson']=tool+hung
        # Locally consistent native+retained payload mutation keeps independent
        # process/source/request ownership unchanged and reaches the PID rule.
        new=hung.replace(b'101',b'999');raw['fault-events.ndjson']=tool+new
        command['traces'][101]=trace.replace(json.dumps(hung.decode()),json.dumps(new.decode()))
        self.rejected('native-fault-payload-pid',q.native_fault_events,capture,command,process,call,BEFORE,AFTER)
        raw['fault-events.ndjson']=tool+hung;command['traces'][101]=trace
        capture.mode='protocol'
        self.rejected('native_fault_role_unsupported',q.native_fault_events,capture,command,process,call,BEFORE,AFTER)

    def test_53_native_wall_boundary_and_outside_window_stay_unqualified(self):
        capture,command,proof,_=self.receipt_excerpt()
        for boundary,code in [(proof['nativeBeginNs']+500,'clock_window_boundary_ambiguous'),
                              (proof['nativeCloseCompleteUpperNs']+1000000,'native_receipt_request_window_collision')]:
            mutant,cmd=self.mutant_capture(capture,command)
            bracket=cmd['cap']['requests'][2]['requestSendBracket']
            delta=boundary-bracket['before']['wallNs']
            for edge in ['before','after']:
                sample=bracket[edge]
                bracket[edge]=q.own_sample({k:(v+delta if k in ['wallNs','monoBeforeNs','monoAfterNs'] else v)
                                           for k,v in sample.items()},sample.owner)
            self.rejected(code,q.native_receipt_records,mutant,cmd)

    def test_54_fault_before_request_missing_or_partial_append_refused(self):
        capture,command,process,call,raw,tool,hung=self.fictional_fault()
        trace=command['traces'][101];lines=trace.splitlines()
        command['traces'][101]='\n'.join([lines[0],*lines[7:10],*lines[1:7],lines[10]])+'\n'
        self.rejected('native_fault_request_order_missing',q.native_fault_events,capture,command,process,call,BEFORE,AFTER)
        command['traces'][101]='\n'.join([*lines[:7],lines[10]])+'\n';raw['fault-events.ndjson']=tool
        self.rejected('native_fault_anchor_missing',q.native_fault_events,capture,command,process,call,BEFORE,AFTER)
        raw['fault-events.ndjson']=tool+hung
        command['traces'][101]=trace.replace(') = '+str(len(hung))+' ',') = '+str(len(hung)-1)+' ')
        self.rejected('native_write_partial_or_length',q.native_fault_events,capture,command,process,call,BEFORE,AFTER)

    def direct_boundary_fixture(self):
        capture = self.retained_capture('A'); command = capture.commands['neutral']
        mutant, cmd = self.mutant_capture(capture, command)
        cmd['direct'] = copy.copy(command['direct'])
        # Copy the linked event/channel graph together; mutants never touch cache.
        cmd['direct'].update(copy.deepcopy({k:command['direct'][k] for k in ['events','channels']}))
        request = cmd['cap']['requests'][4]
        reply = next(f for f in cmd['cap']['frames'] if f.get('value', {}).get('id') == 4)
        call = next(c for c in q.source_calls(cmd['direct']) if c['request']['id'] == 4)
        return mutant, cmd, request, reply, call['after']

    def direct_boundary_check(self, fixture):
        with historical_observer_source(fixture[0].root):
            return q.direct_request_sample(*fixture, 'stdout')

    def test_55_actual_direct_response_native_enclosure(self):
        fixture = self.direct_boundary_fixture(); capture, cmd, request, reply, sample = fixture
        self.rejected('clock_window_boundary_ambiguous', cmd['direct']['clock'].contains, sample,
                      request['requestSendBracket']['before'], reply['responseReceiveBracket']['after'])
        self.assertEqual(q.wall_cell(sample), (1793684704952000000,1793684704953000000))
        self.assertEqual(self.direct_boundary_check(fixture), (1793684704952192000,1793684704952410000))
        with historical_observer_source(capture.root):
            self.assertEqual(set(q.request_receipts(capture,cmd,request,reply)), {'dispatch','result'})
            incoming = next(c for c in q.source_calls(cmd['direct']) if c['request']['id']==4)['before']
            self.assertEqual(q.direct_request_sample(capture,cmd,request,reply,incoming,'stdin'), q.wall_cell(incoming))

    def test_56_direct_native_owner_and_exec_controls(self):
        for kind, code in [('pid','native_write_owner_unavailable'),('exec','native-write-exec-owner'),
                           ('pgid','native-write-group-owner'),('parent','native-write-parent-owner'),
                           ('domain','clock_owner_mismatch')]:
            with self.subTest(kind=kind):
                fixture=self.direct_boundary_fixture(); _,cmd,_,_,sample=fixture
                cmd['direct']['start']=copy.deepcopy(cmd['direct']['start'])
                start=cmd['direct']['start']
                if kind=='pid': start['pid']=99999999
                if kind=='exec': start['nativeArgv']=[*start['nativeArgv'],'wrong']
                if kind=='pgid': start['pgid']+=1
                if kind=='parent': start['ppid']+=1
                if kind=='domain':
                    sample.owner=q.ClockDomain('bun',sample.owner.capture,99999999,sample.owner.command,sample.owner.exec_sha256)
                self.rejected(code,self.direct_boundary_check,fixture)

    def test_57_direct_event_sequence_frame_and_native_byte_controls(self):
        for kind,code in [('sequence','direct_clock_sequence_ambiguous'),('pid','direct-clock-event-owner'),
                          ('frame','direct_clock_frame_ambiguous'),('native','direct-clock-event-append-bytes')]:
            with self.subTest(kind=kind):
                fixture=self.direct_boundary_fixture();_,cmd,_,_,sample=fixture
                event=next(e for e in cmd['direct']['events'] if e['sample'] is sample)
                if kind=='sequence': event['sequence']=999999
                if kind=='pid': event['pid']+=1
                if kind=='frame':
                    raw=q.event_bytes(event).replace(b'"id":4',b'"id":9')
                    import base64
                    event.update(data=base64.b64encode(raw).decode(),bytes=len(raw),sha256=q.sha(raw))
                if kind=='native':
                    pid=cmd['direct']['start']['pid'];lines=cmd['traces'][pid].splitlines()
                    lines[1268]=lines[1268].replace('3792357029','3792357028')
                    cmd['traces'][pid]='\n'.join(lines)+'\n'
                self.rejected(code,self.direct_boundary_check,fixture)

    def test_58_direct_missing_partial_and_reordered_append_controls(self):
        for kind,code in [('missing','direct-clock-stream-append-bytes'),('partial','native_write_partial_or_length'),
                          ('order','direct_clock_append_order_ambiguous'),('trace','native_write_trace_missing')]:
            with self.subTest(kind=kind):
                fixture=self.direct_boundary_fixture();_,cmd,_,_,_=fixture
                pid=cmd['direct']['start']['pid'];lines=cmd['traces'][pid].splitlines()
                if kind=='missing': del lines[1264:1267]
                if kind=='partial': lines[1265]=lines[1265].replace('= 428 ', '= 427 ')
                if kind=='order': lines[1264:1270]=lines[1267:1270]+lines[1264:1267]
                cmd['traces'][pid]='\n'.join(lines)+'\n'
                if kind=='trace': del cmd['traces'][pid]
                self.rejected(code,self.direct_boundary_check,fixture)

    def test_59_direct_disjoint_and_outside_recorder_window_controls(self):
        fixture=self.direct_boundary_fixture();_,cmd,_,_,_=fixture
        pid=cmd['direct']['start']['pid'];lines=cmd['traces'][pid].splitlines()
        for index in range(1264,1270):
            stamp,rest=lines[index].split(' ',1);seconds,micros=stamp.split('.')
            shifted=int(seconds)*1000000+int(micros)+10000
            lines[index]=str(shifted//1000000)+'.'+str(shifted%1000000).zfill(6)+' '+rest
        cmd['traces'][pid]='\n'.join(lines)+'\n'
        self.rejected('direct_clock_native_envelope_disjoint',self.direct_boundary_check,fixture)
        fixture=self.direct_boundary_fixture();_,_,_,reply,_=fixture
        bracket=reply['responseReceiveBracket'];delta=1793684704952100000-bracket['after']['wallNs']
        for edge in ['before','after']:
            value=bracket[edge]
            bracket[edge]=q.own_sample({k:(v+delta if k in ['wallNs','monoBeforeNs','monoAfterNs'] else v)
                                       for k,v in value.items()},value.owner)
        self.rejected('clock_sample_outside_window',self.direct_boundary_check,fixture)

    def test_60_direct_boundary_without_pinned_proof_stays_closed(self):
        fixture=self.direct_boundary_fixture();capture,cmd,request,reply,sample=fixture
        self.rejected('clock_window_boundary_ambiguous',q.enclose_cell,q.wall_cell(sample),
                      request['requestSendBracket']['before']['wallNs']-1,reply['responseReceiveBracket']['after']['wallNs']+1)
        original=capture.raw
        capture.raw=lambda path: b'changed preload' if path=='observer-source/preload.ts' else original(path)
        self.rejected('direct-clock-preload-order',self.direct_boundary_check,fixture)
        fixture=self.direct_boundary_fixture();_,_,request,_,_=fixture
        request['request']['id']=99999
        self.rejected('direct_clock_reply_binding_missing',self.direct_boundary_check,fixture)

    def test_61_current763_refuses_old857_observer_evidence(self):
        actual_here = Path(q.__file__).resolve().parent
        self.assertEqual(q.HERE, actual_here)
        self.assertEqual(q.sha((q.HERE/'preload.ts').read_bytes()),
                         '763e3d173c5031224b9b0023b29c4aff5474793418e94c902e487978b8033460')
        fixture=self.direct_boundary_fixture();capture,cmd,request,reply,sample=fixture
        self.assertEqual(q.HERE, actual_here)  # Historical constructor restored it.
        self.rejected('direct-clock-preload-order',q.direct_request_sample,*fixture,'stdout')
        self.rejected('direct-clock-preload-order',q.request_receipts,capture,cmd,request,reply)
        custody=q.parse((NOTES.parent/'LEAD-PROTOCOL-CUSTODY-03.json').read_bytes())
        with patch('subprocess.Popen',side_effect=AssertionError('forbidden subprocess')):
            self.rejected('observer-executed-bytes',q.Capture,capture.root,
                          custody['captures'][str(capture.root)],custody['supervisors'][str(capture.root)])
        try:
            with historical_observer_source(capture.root):
                self.assertEqual(q.HERE,capture.root/'observer-source')
                raise RuntimeError('historical_context_exception')
        except RuntimeError:
            pass
        self.assertEqual(q.HERE,actual_here)  # Also restored on exceptions.

    def test_31_frozen_framing_bytes_and_split_order(self):
        chunks = q.expected_stdin('framing', 'neutral')
        self.assertEqual(chunks[3], b'{not-json}\n')
        self.assertTrue(chunks[4].endswith(b'\r\n'))
        self.assertEqual(chunks[5]+chunks[6], b'{"jsonrpc":"2.0","id":9,"method":"tools/list","params":{}}\n')
        self.assertEqual(chunks[7], b'X'*1048577+b'\n')
        protocol = q.expected_stdin('protocol','neutral')
        self.assertTrue(all(x.startswith(b'{"jsonrpc":"2.0"') for x in protocol))

    def test_32_wrong_external_qualifier_blob_refused(self):
        names = ['CONTRACT.md','POLICY.json','STIMULUS.json','ORIGINAL-PINS.json','record.py','preload.ts','qualify.py','test_qualify.py']
        custody = {'qualifierFiles': {n:q.sha((q.HERE/n).read_bytes()) for n in names}, 'captures': {}}
        custody['qualifierFiles']['record.py'] = '0'*64
        self.rejected('external-qualifier-blob-record.py', q.qualify_pair, self.dir, self.dir, 'protocol', custody)

    def test_33_source_identity_digest_preimage_uses_original_order(self):
        req, original, wire, receipts = normal_fixture(identity=True)
        original['structuredContent'] = {'b':2,'a':1}
        original['content'][0]['text'] = q.js_bytes(original['structuredContent']).decode()
        wire = {'structuredContent': {'a':1,'b':2}, 'content': copy.deepcopy(original['content'])}
        receipts['result']['body']['resultSha256'] = q.sha(q.js_bytes(original['structuredContent']))
        q.bridge_roles(req, wire, receipts, BEFORE, AFTER, q.Atoms('x'), 'x', original)
        receipts['result']['body']['resultSha256'] = q.sha(q.js_bytes(wire['structuredContent']))
        self.rejected('bridge-result-digest', q.bridge_roles, req, wire, receipts, BEFORE, AFTER, q.Atoms('x'), 'x', original)

    def test_34_thread_parent_is_not_a_different_process_owner(self):
        trace = {100: '1.000 clone3({flags=CLONE_VM|CLONE_THREAD}, 88) = 102\n',
                 102: '1.001 clone3({flags=CLONE_VM|CLONE_VFORK}, 88) = 103\n',
                 103: '1.002 execve("bun", ["bun", "original.ts"], []) = 0\n1.003 +++ exited with 0 +++\n'}
        self.assertEqual(q.native_trace(trace)['parents'][103], 100)

    def supervisor_functions(self):
        # Compile only pure function definitions from the notes supervisor.
        # Its command-launching top-level body is never evaluated by tests.
        path = NOTES/'command.py'
        tree = ast.parse(path.read_text())
        chosen = {'recorder_root', 'reconcile_children', 'budget_qualified'}
        body = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in chosen]
        namespace = {'Path': Path, 'BASE': NOTES, 'OUTER_NS': 90000000000, 'json': json}
        exec(compile(ast.Module(body=body, type_ignores=[]), str(path), 'exec'), namespace)
        return namespace

    def supervisor_record(self):
        return {'startMonoNs': 0, 'endMonoNs': 89_000_000_000, 'elapsedNs': 89_000_000_000,
                'outerCapNs': 90_000_000_000, 'childExitObservedMonoNs': 84_000_000_000,
                'recorderInternalDeadlineNs': 86_000_000_000, 'exit': 0, 'failure': None,
                'qualified': True, 'pipeDrainComplete': True, 'sourceSnapshotComplete': True,
                'sourceUnchanged': True, 'pidGone': True, 'groupGone': True,
                'recorderWithinInternalDeadline': True, 'currentRecorderRun': True,
                'childPGIDReadbacks': [{'role': 'neutral', 'pgid': 101, 'groupGone': True}]}

    def test_35_internal_work80_hard85_alarm_is_absolute(self):
        spec = importlib.util.spec_from_file_location('s11_record_deadline_test', q.HERE/'record.py')
        module = importlib.util.module_from_spec(spec)
        with patch('subprocess.Popen', side_effect=AssertionError('forbidden subprocess')):
            spec.loader.exec_module(module)
        self.assertEqual(module.recorder_deadlines(7), (80_000_000_007, 85_000_000_007))
        with patch.object(module.time, 'monotonic_ns', return_value=79_000_000_000), patch.object(module.signal, 'setitimer') as timer:
            module.arm_deadline(80_000_000_000)
            timer.assert_called_once_with(module.signal.ITIMER_REAL, 1.0)
        with patch.object(module.time, 'monotonic_ns', return_value=85_000_000_001), patch.object(module.signal, 'setitimer') as timer:
            with self.assertRaisesRegex(q.Unqualified, 'recorder_internal_deadline'):
                module.arm_deadline(85_000_000_000)
            timer.assert_not_called()

    def test_36_late_snapshot_or_pipe_drain_cannot_accept_outer_zero(self):
        supervisor = self.supervisor_functions()
        valid = self.supervisor_record()
        self.assertTrue(supervisor['budget_qualified'](valid))
        q.validate_supervisor(valid, 1_000_000_000)
        for key in ['pipeDrainComplete', 'sourceSnapshotComplete', 'recorderWithinInternalDeadline']:
            late = copy.deepcopy(valid); late[key] = False
            self.assertFalse(supervisor['budget_qualified'](late))
            with self.assertRaises(q.Unqualified): q.validate_supervisor(late, 1_000_000_000)
        late = copy.deepcopy(valid)
        late['endMonoNs'] = late['elapsedNs'] = 90_000_000_001
        self.assertEqual(late['exit'], 0)
        self.assertFalse(supervisor['budget_qualified'](late))
        with self.assertRaisesRegex(q.Unqualified, 'outer_actual_overrun'):
            q.validate_supervisor(late, 1_000_000_000)

    def test_37_failure_reconciles_every_separate_recorded_session(self):
        supervisor = self.supervisor_functions()
        root = self.dir/'separate-sessions'
        for role, pid in [('first', 101), ('second', 202)]:
            folder = root/'commands'/role; folder.mkdir(parents=True)
            (folder/'START.json').write_text(json.dumps({'pid': pid, 'pgid': pid}))
        visited = []
        def inspect(pgid):
            visited.append(pgid)
            return {'pgid': pgid, 'groupGone': pgid != 101, 'members': [{'pid': 101}] if pgid == 101 else []}
        readings = supervisor['reconcile_children'](root, inspect)
        self.assertEqual(visited, [101, 202])
        record = self.supervisor_record(); record['childPGIDReadbacks'] = readings
        self.assertFalse(supervisor['budget_qualified'](record))
        with self.assertRaisesRegex(q.Unqualified, 'child_pgid_unreconciled'):
            q.validate_supervisor(record, 1_000_000_000)
        (root/'commands'/'missing-start').mkdir()
        readings = supervisor['reconcile_children'](root, inspect)
        self.assertTrue(any(r.get('error') == 'child_group_readback_unavailable' for r in readings))

    def test_38_old85_outer_or90_internal_proof_is_unqualified(self):
        proof = self.supervisor_record(); proof['outerCapNs'] = 85_000_000_000
        with self.assertRaisesRegex(q.Unqualified, 'outer_cap_changed'):
            q.validate_supervisor(proof, 1_000_000_000)
        proof = self.supervisor_record(); proof['recorderInternalDeadlineNs'] = 91_000_000_000
        with self.assertRaisesRegex(q.Unqualified, 'recorder_deadline_mismatch'):
            q.validate_supervisor(proof, 1_000_000_000)
        self.assertEqual(q.POLICY['limits']['workNs']+q.POLICY['limits']['cleanupReserveNs'], q.POLICY['limits']['recorderHardNs'])
        self.assertEqual(q.POLICY['limits']['wallNs'], 90_000_000_000)

    def test_39_manifest_overrun_cannot_return_preflight_zero(self):
        spec = importlib.util.spec_from_file_location('s11_record_finalization_test', q.HERE/'record.py')
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        now = [0]
        target = self.dir/'late-manifest'
        argv = ['record.py', '--source-root', str(q.HERE.parents[1]), '--mode', 'protocol',
                '--run', 'A', '--output', str(target), '--preflight']
        save = module.save
        def delayed_manifest(path, value):
            save(path, value)
            if path.name == 'MANIFEST.json':
                now[0] = 85_000_000_001
        with patch.object(module.sys, 'argv', argv), patch.object(module.resource, 'setrlimit'), \
             patch.object(module.signal, 'signal'), patch.object(module.signal, 'setitimer'), \
             patch.object(module.time, 'monotonic_ns', side_effect=lambda: now[0]), \
             patch.object(module, 'stamp', side_effect=lambda: stamp(W+now[0], now[0])), \
             patch.object(module, 'preflight', return_value={'sources': {}, 'packages': {}, 'tools': {}}), \
             patch.object(module, 'save', side_effect=delayed_manifest), \
             patch('subprocess.Popen', side_effect=AssertionError('forbidden subprocess')):
            with self.assertRaisesRegex(q.Unqualified, 'recorder_internal_deadline'):
                module.main()
        self.assertTrue((target/'PREFLIGHT.json').exists())
        self.assertTrue((target/'MANIFEST.json').exists())
        # Earlier success-shaped files do not turn late finalization into exit0.


if __name__ == '__main__':
    unittest.main()
