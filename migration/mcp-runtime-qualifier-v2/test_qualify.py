#!/usr/bin/env python3
"""Original raw-data and review-counterexample tests. Never executes an observer."""
import copy
import json
import pathlib
import tempfile
import unittest

import qualify as q

NOTES = pathlib.Path('/home/palantirkc/work/ws-replan/preparation/slices/S11/qualifier-v2/native')
UUID_A = '11111111-1111-4111-8111-111111111111'
UUID_B = '22222222-2222-4222-8222-222222222222'


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\n').encode()


class QualifierTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.qualifier = q.Qualifier()
        # Retain all fictional mutation bundles and failed attempts for review.
        cls.root = pathlib.Path(tempfile.mkdtemp(prefix='test-captures-', dir=NOTES))
        cls.sequence = 0

    def bundle(self, mode='protocol'):
        type(self).sequence += 1
        root = self.root / f'{type(self).sequence:03}-{mode}'
        root.mkdir()
        for relative in self.qualifier.manifest['modes'][mode]['candidateFiles']:
            p = root / relative
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(self.qualifier.expected(mode, relative))
        return root

    def json_change(self, root, relative, mutator):
        p = root / relative
        v = q.parse(p.read_bytes())
        mutator(v)
        p.write_bytes(encoded(v))

    def rebind(self, root, label, channel='stdout'):
        self.json_change(root, f'commands/{label}/RESULT.json',
                         lambda v: v['channels'][channel].update(
                             sha256=q.sha((root / f'commands/{label}/{channel}.bin').read_bytes())))

    def refused(self, root, mode='protocol', code=None):
        with self.assertRaises(q.Refusal) as caught:
            self.qualifier.compare(mode, root)
        if code:
            self.assertIn(code, str(caught.exception))

    def test_all_ten_unchanged_originals_match_but_remain_limited(self):
        # Assert actual raw correspondence and request/response binding, not
        # merely counts or observer COMPLETE booleans.
        for mode in ['inventory', 'discovery', 'protocol', 'composition', 'graph',
                     'identity', 'framing', 'bounds', 'launcher', 'relocation']:
            with self.subTest(mode=mode):
                root = self.bundle(mode)
                report = self.qualifier.compare(mode, root)
                self.assertEqual(report['comparison'], 'MATCH')
                self.assertEqual(report['qualification'], 'LIMITED')
                self.assertFalse(report['fullParity'])
                self.assertFalse(report['graphAdmission'])
                for b in self.qualifier.manifest['modes'][mode]['responseBindings']:
                    obs = q.parse((root / 'observations.json').read_bytes())[b['observation']]
                    raw = (root / f"commands/{b['command']}/stdout.bin").read_bytes()
                    self.assertTrue(q.same(obs['response'], q.parse(raw.split(b'\n')[b['frame']])))

    def test_review_counterexamples_are_literal(self):
        cases = [
            ([{'o': UUID_A}, {'o': UUID_B}], [{'o': UUID_B}, {'o': UUID_A}]),
            ({'o': UUID_A}, {'o': UUID_B}),
            ({'description': 'x ' + UUID_A}, {'description': 'x ' + UUID_B}),
            ({'observedAt': '2026-11-03T01:00:00Z'}, {'observedAt': '2026-11-03T02:00:00Z'}),
            ({'at': '2026-11-03T01:00:00Z'}, {'at': '2026-11-03T02:00:00Z'}),
            ({'elapsedMs': 640}, {'elapsedMs': 999999}),
            ({'elapsedMs': 640}, {'elapsedMs': -1}),
            ({'elapsedMs': 640}, {'elapsedMs': 640.0}),
            ({'v': 1}, {'v': True}), ({'v': 1}, {'v': 1.0}),
            ({'result': {}}, {'result': {'isError': False}}),
            ({'description': '/tmp/modeX/value'}, {'description': '/tmp/other/value'}),
            ([1, 2], [2, 1]),
        ]
        for a, b in cases:
            with self.subTest(expected=a, actual=b):
                with self.assertRaises(q.Refusal):
                    q.require_same(a, b, 'review-counterexample')

    def test_text_serialization_duplicate_whitespace_and_order_are_literal(self):
        baseline = {'content': [{'type': 'text', 'text': '{"a":1,"b":2}'}]}
        for text in ['{ "a" : 1,"b":2}', '{"a":999,"a":1,"b":2}',
                     '{"b":2,"a":1}', '{"a":1.0,"b":2}']:
            with self.subTest(text=text):
                changed = copy.deepcopy(baseline)
                changed['content'][0]['text'] = text
                with self.assertRaises(q.Refusal):
                    q.require_same(baseline, changed, 'literal-content-text')
                with self.assertRaises(q.Refusal):
                    q.wire_equal(encoded(baseline), encoded(changed), 'literal-content-text-wire')

    def test_duplicate_keys_are_not_last_value_wins(self):
        for raw in [b'{"id":999,"id":1}\n', b'{"v":{"a":999,"a":1}}',
                    b'[{"a":1,"a":1}]']:
            with self.subTest(raw=raw):
                with self.assertRaises(q.Refusal):
                    q.parse(raw)
        with self.assertRaises(q.Refusal):
            q.wire_equal(b'{"id":1}\n', b'{"id":999,"id":1}\n', 'duplicate-wire')

    def test_nonfinite_json_and_wrong_result_shape_fail_closed(self):
        for raw in [b'NaN', b'Infinity', b'1e999']:
            with self.subTest(raw=raw), self.assertRaises(q.Refusal):
                q.parse(raw)
        root = self.bundle()
        (root / 'commands/neutral/RESULT.json').write_bytes(b'[[1]]')
        self.refused(root, code='result_object_required')

    def test_outer_member_order_is_immaterial_without_erasing_tokens(self):
        q.wire_equal(b'{"id":1,"result":{"a":1,"b":2}}\n',
                     b'{"result":{"b":2,"a":1},"id":1}\n', 'outer-reordering')
        for changed in [b'{ "id":1}\n', b'{"id":1.0}\n', b'{"id":true}\n',
                        b'{"id":1}\r\n', b'{"id":1}', b'{"id":1}\n{}\n']:
            with self.subTest(raw=changed):
                with self.assertRaises(q.Refusal):
                    q.wire_equal(b'{"id":1}\n', changed, 'strict-wire')

    def test_actual_outer_order_with_fresh_hash_still_matches(self):
        root = self.bundle('inventory')
        p = root / 'commands/neutral/stdout.bin'
        lines = p.read_bytes().split(b'\n')
        v = q.parse(lines[1])
        v = dict(reversed(list(v.items())))
        lines[1] = encoded(v).rstrip(b'\n')
        p.write_bytes(b'\n'.join(lines))
        self.rebind(root, 'neutral')
        # This is a wire-equivalent transformed original, not fresh execution.
        for process in (root / 'processes').iterdir():
            (process / 'stdout.bin').write_bytes(p.read_bytes())
        self.assertEqual(self.qualifier.compare('inventory', root)['comparison'], 'MATCH')

    def test_exact_source_payload_freshness_is_not_a_clock_wildcard(self):
        root = self.bundle()
        obs = q.parse((root / 'observations.json').read_bytes())
        row = next(o for o in obs if o.get('id') == 8)
        original = row['response']['result']['structuredContent']['source']['observedAt']
        changed = '2026-11-03T00:00:00.000Z'
        self.assertNotEqual(original, changed)
        # Coherently change both text/structured/raw, yet still reject stale
        # source data rather than simply testing text-vs-structured inconsistency.
        for p in [root / 'observations.json', *root.glob('commands/*/stdout.bin'),
                  *root.glob('processes/*/stdout.bin')]:
            p.write_bytes(p.read_bytes().replace(original.encode(), changed.encode()))
        self.refused(root, code='literal_mismatch')

    def test_occurrence_role_swap_and_correlation_are_not_first_seen_mapping(self):
        for kind in ['swap-all', 'text-only', 'structured-only', 'receipt-only']:
            with self.subTest(kind=kind):
                root = self.bundle()
                obs = q.parse((root / 'observations.json').read_bytes())
                rows = [o for o in obs if o.get('id') in [3, 6]]
                a, b = [o['response']['result']['structuredContent']['occurrence'] for o in rows]
                if kind == 'swap-all':
                    for p in root.rglob('*'):
                        if p.is_file():
                            data = p.read_bytes().replace(a.encode(), b'@swap@').replace(b.encode(), a.encode()).replace(b'@swap@', b.encode())
                            p.write_bytes(data)
                elif kind in ['text-only', 'structured-only']:
                    result = rows[0]['response']['result']
                    if kind == 'text-only':
                        result['content'][0]['text'] = result['content'][0]['text'].replace(a, b)
                    else:
                        result['structuredContent']['occurrence'] = b
                    (root / 'observations.json').write_bytes(encoded(obs))
                else:
                    receipt = next((root / 'receipts').glob(a + '-*.json'))
                    self.json_change(root, str(receipt.relative_to(root)), lambda v: v.update(occurrence=b))
                self.refused(root)

    def test_name_schema_omission_types_array_bounds_default_routing_error_digest(self):
        mutations = [
            lambda v: v[0]['response']['result']['tools'][0].update(name='different-tool'),
            lambda v: v[0]['response']['result']['tools'][0]['inputSchema'].update(type='array'),
            lambda v: v[0]['response']['result']['tools'][0].pop('description'),
            lambda v: v[0]['response']['result']['tools'].reverse(),
            lambda v: v[2]['response']['result']['structuredContent'].update(elapsedMs=999999),
            lambda v: v[2]['response']['result']['structuredContent'].update(code='different_error'),
            lambda v: v[2]['response']['result']['structuredContent']['failure'].update(errorSha256='0' * 64),
            lambda v: v[2]['response']['result'].pop('isError'),
            lambda v: v[3].update(name='different_route'),
            lambda v: v[3]['response'].update(id=999),
        ]
        for i, mutation in enumerate(mutations):
            with self.subTest(mutation=i):
                root = self.bundle()
                self.json_change(root, 'observations.json', mutation)
                self.refused(root, code='literal_mismatch')
        for field, value in [('maximum', 4), ('minimum', 0), ('default', 3)]:
            with self.subTest(bound=field):
                root = self.bundle('bounds')
                def change(v):
                    tool = next(t for t in v[0]['response']['result']['tools'] if t['name'] == 'academy_bounded_probe')
                    tool['inputSchema']['properties']['limit'][field] = value
                self.json_change(root, 'observations.json', change)
                self.refused(root, 'bounds')

    def test_same_observations_with_changed_stderr_and_rebound_hash_fail(self):
        root = self.bundle()
        (root / 'commands/neutral/stderr.bin').write_bytes(b'forged diagnostic\n')
        self.rebind(root, 'neutral', 'stderr')
        self.refused(root, code='literal_bytes_mismatch')

    def test_stale_channel_digest_and_replaced_origin_fail(self):
        root = self.bundle()
        (root / 'commands/neutral/stdout.bin').write_bytes(b'{}\n')
        self.refused(root, code='channel_binding_mismatch')
        root = self.bundle()
        relative = next(r for r in self.qualifier.manifest['modes']['protocol']['candidateFiles']
                        if r.endswith('CHANNEL-ORIGINS.json') and q.parse((root / r).read_bytes())[
                            'stderr.bin'] == 'no_stream_data_or_write_observed')
        self.json_change(root, relative, lambda v: v.update({'stderr.bin': 'direct_original_child_pipe:commands/neutral'}))
        self.refused(root)

    def test_raw_response_mismatch_extra_truncation_and_stdin_order_fail(self):
        for kind in ['response', 'extra', 'truncated', 'stdin-order', 'framing-output']:
            with self.subTest(kind=kind):
                root = self.bundle()
                channel = 'stdin' if kind == 'stdin-order' else 'stdout'
                p = root / f'commands/neutral/{channel}.bin'
                b = p.read_bytes()
                if kind == 'response':
                    b = b.replace(b'"id":3', b'"id":999', 1)
                elif kind == 'extra':
                    b += b'{}\n'
                elif kind == 'truncated':
                    b = b[:-1]
                elif kind == 'stdin-order':
                    lines = b.split(b'\n')
                    lines[2], lines[3] = lines[3], lines[2]
                    b = b'\n'.join(lines)
                else:
                    b = b.replace(b'\n', b'\r\n')
                p.write_bytes(b)
                self.rebind(root, 'neutral', channel)
                self.refused(root)

    def test_exit_signal_start_and_inner_failure_after_outer_zero_fail(self):
        cases = [
            ('commands/neutral/RESULT.json', lambda v: v.update(returncode=23)),
            ('commands/neutral/RESULT.json', lambda v: v.update(returncode=-15)),
            ('commands/neutral/RESULT.json', lambda v: v.update(signal=15)),
            ('commands/neutral/START.json', lambda v: v.update(cwd='/different/cwd')),
            ('commands/neutral/START.json', lambda v: v['argv'].append('--different-route')),
            ('commands/neutral/START.json', lambda v: v.update(pid=1, pgid=1)),
            ('commands/neutral/START.json', lambda v: v.update(startedAt='2026-11-03T03:00:00Z')),
        ]
        for relative, change in cases:
            with self.subTest(relative=relative, change=repr(change)):
                root = self.bundle()
                self.json_change(root, relative, change)
                self.refused(root)
        root = self.bundle()
        inner = next(r for r in self.qualifier.manifest['modes']['protocol']['candidateFiles']
                     if r.endswith('/EXIT.json'))
        self.json_change(root, inner, lambda v: v.update(code=23))
        self.assertEqual(q.parse((root / 'commands/neutral/RESULT.json').read_bytes())['returncode'], 0)
        self.refused(root)

    def test_source_after_green_and_source_before_both_changed_fail(self):
        for both in [False, True]:
            root = self.bundle()
            for name in ['PINS-after.json', *(['PINS-before.json'] if both else [])]:
                def change(v):
                    first = next(iter(v['sources']))
                    v['sources'][first]['sha256'] = '0' * 64
                self.json_change(root, name, change)
            self.refused(root)

    def test_json_only_missing_inner_channel_and_extra_capture_refused(self):
        # Do not silently accept a candidate observations report or a forged
        # COMPLETE flag in place of its raw captures.
        root = self.bundle()
        only = self.root / 'json-only'
        only.mkdir()
        (only / 'observations.json').write_bytes((root / 'observations.json').read_bytes())
        self.refused(only, code='missing_or_unbound_capture_files')
        root = self.bundle()
        inner = next(r for r in self.qualifier.manifest['modes']['protocol']['candidateFiles']
                     if r.startswith('processes/') and r.endswith('/stderr.bin'))
        # Rename fictional test data; retain rather than discard it.
        (root / inner).rename(root / 'retained-missing-inner-stderr.bin')
        self.refused(root, code='missing_or_unbound_capture_files')
        root = self.bundle()
        (root / 'extra-channel.bin').write_bytes(b'extra')
        self.refused(root)

    def test_fixture_expected_or_policy_cannot_turn_failure_green(self):
        root = self.bundle()
        (root / 'expected.json').write_bytes((root / 'observations.json').read_bytes())
        (root / 'POLICY.json').write_bytes(b'{"accept":true}')
        self.refused(root, code='missing_or_unbound_capture_files')

    def test_intentional_bounds_sigterm_is_pinned_not_generic_success(self):
        root = self.bundle('bounds')
        result = q.parse((root / 'commands/neutral/RESULT.json').read_bytes())
        self.assertEqual(result['returncode'], -15)
        self.assertTrue(result['cleanupRequested'])
        report = self.qualifier.compare('bounds', root)
        self.assertEqual(report['comparison'], 'MATCH')
        self.assertTrue(any('missing_inner_exit' in x for x in report['limits']))
        for code in [0, 1, -9, 15]:
            with self.subTest(returncode=code):
                root = self.bundle('bounds')
                self.json_change(root, 'commands/neutral/RESULT.json', lambda v: v.update(returncode=code))
                self.refused(root, 'bounds')

    def test_input_crlf_and_split_are_aggregate_bytes_not_chunk_parity(self):
        root = self.bundle('framing')
        raw = (root / 'commands/neutral/stdin.bin').read_bytes()
        self.assertTrue(b'{"jsonrpc":"2.0","id":8,"method":"tools/list","params":{}}\r\n' in raw)
        self.assertIn(b'"id":9,"method":"tools/list","params":{}}\n', raw)
        self.assertEqual(self.qualifier.compare('framing', root)['comparison'], 'MATCH')
        self.assertEqual((root / 'commands/neutral/stderr.bin').read_bytes().count(b'\n'), 3)

    def test_finite_positive_capture_duration_atoms_and_bounds(self):
        # Capture instrumentation elapsedSeconds is justified by observe.py;
        # this does not allow payload elapsedMs or any semantic clock changes.
        label = 'neutral'
        rule = self.qualifier.policy['captureDurationRoles']['protocol'][label]
        for duration in [float(rule['minimumSeconds']) + .05, 90.0]:
            root = self.bundle()
            self.json_change(root, 'commands/neutral/RESULT.json', lambda v: v.update(elapsedSeconds=duration))
            self.assertEqual(self.qualifier.compare('protocol', root)['comparison'], 'MATCH')
        for duration in [-1., 90.01, float('inf'), '1', True, 1, 0.]:
            root = self.bundle()
            self.json_change(root, 'commands/neutral/RESULT.json', lambda v: v.update(elapsedSeconds=duration))
            self.refused(root)

    def test_graph_booleans_cannot_grant_admission(self):
        root = self.bundle('graph')
        self.json_change(root, 'COMPLETE.json', lambda v: v.update(
            graphQualification='artifact_pinned', completeGraphVerified=True,
            mandatoryMismatch=False, nativeOwnersRetained=True))
        self.refused(root, 'graph')
        self.assertFalse(hasattr(q, 'candidate_graph_qualified'))

    def test_missing_channels_are_distinct_from_no_event_taps(self):
        report = self.qualifier.compare('protocol', self.bundle())
        self.assertTrue(any('js_tap_no_event_not_syscall_silence' in x for x in report['limits']))
        self.assertTrue(any('v1_freeze_unqualified_direct_binding' in x for x in report['limits']))
        self.assertIn('early unavailable stderr, native graph/owner admission, dynamic bundles/jsdom', report['policyRefusals'])

    def test_capture_symlink_substitution_refused(self):
        root = self.bundle()
        (root / 'substituted').symlink_to(root / 'commands', target_is_directory=True)
        self.refused(root, code='symlink_capture')


if __name__ == '__main__':
    unittest.main(verbosity=2)
