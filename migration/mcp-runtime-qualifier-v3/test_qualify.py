"""Pure stdlib semantic/structural tests; no original import or subprocess.

Fixtures are fictional and retained in a unique author-notes directory. Original
frozen public bytes are read only as independent anchors, never regenerated.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import qualify as q

NOTES = Path('/home/palantirkc/work/ws-replan/preparation/slices/S11/qualifier-v3/native')
V1 = q.HERE.parents[1] / 'migration/mcp-runtime-reference-v1'
ORIGINAL_RAW = Path('/home/palantirkc/work/ws-replan/preparation/slices/S11/reference-v1/native')
U1 = '12345678-1234-4234-8234-123456789abc'
U2 = '23456789-1234-4234-8234-123456789abc'
T = '2026-11-03T00:00:00.000Z'
W = q.utc_ms(T) * 1_000_000


def stamp(wall, mono):
    return {'wallNs': wall, 'monoBeforeNs': mono, 'monoAfterNs': mono + 100}


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
        entry = str(capture.root/'source/original.ts')
        argv = ['/usr/local/bin/bun', '--no-install', '--no-env-file', '--preload', str(capture.root/'observer-source/preload.ts'), entry]
        start = {'pid': 101, 'ppid': 100, 'pgid': 100, 'uid': os.getuid(), 'cwd': str(capture.root),
                 'nativeArgv': argv, 'argv': ['/usr/local/bin/bun', entry], 'execPath': '/usr/local/bin/bun',
                 'fds': {'0': 'pipe:[1]', '1': 'pipe:[2]', '2': 'pipe:[3]'}, 'sample': stamp(W, 11000000)}
        event = {'pid': 101, 'sequence': 1, 'kind': 'entry', 'sample': stamp(W,11000000), 'path': entry, 'sha256': 'f'*64}
        raw = {'processes/101/START.json': json.dumps(start).encode(), 'processes/101/events.ndjson': json.dumps(event).encode()+b'\n'}
        capture.manifest = {p: {} for p in raw}
        capture.processes = {}
        capture.raw = lambda p: raw[p]
        capture.json = lambda p: q.parse(raw[p])
        capture.entry_pin = lambda p: 'f'*64
        command = {'start': {'pid': 100, 'pgid': 100, 'originalArgv': argv, 'prelaunch': BEFORE},
                   'result': {'ended': AFTER}, 'cap': {'processSnapshots': []},
                   'native': {'execs': {101: {'argv': argv}}, 'parents': {101: 100}, 'terminals': {101: -15}}}
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


if __name__ == '__main__':
    unittest.main()
