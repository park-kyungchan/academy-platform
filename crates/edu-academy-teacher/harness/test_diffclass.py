"""Constructed difference pairs; no model sessions or private captures."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import diffclass as d
import fidelity


class DifferenceClasses(unittest.TestCase):
    def setUp(self):
        self.policy = fidelity.configured_policy()

    def pair(self, reference, answer):
        return d.analyze('Title\n' + reference, 'Title\n' + answer, self.policy)

    def test_all_missing_classes_from_constructed_pairs(self):
        cases = [
            ('equal_after_marker_strip', 'alpha beta gamma', '1. **alpha beta gamma**:'),
            ('equal_after_spacing_punctuation', 'alpha beta gamma', 'alpha,  beta gamma!'),
            ('merged_or_split', 'alpha beta gamma delta', 'alpha beta\ngamma delta'),
            ('reworded_high', 'alpha beta gamma delta', 'alpha beta gamma extra'),
            ('reworded_low', 'alpha beta gamma delta', 'alpha beta other new'),
            ('absent', 'alpha beta gamma delta', 'nothing matches here'),
        ]
        for category, reference, answer in cases:
            with self.subTest(category=category):
                row = self.pair(reference, answer)
                self.assertEqual(row['analysis'], 'complete')
                self.assertEqual(row['facts_kept'], 'FAIL')
                self.assertEqual(row['missing_classes'][category], 1)
                self.assertEqual(sum(row['missing_classes'].values()), 1)

    def test_two_units_merged_in_one_line(self):
        row = self.pair('alpha beta\ngamma delta', 'alpha beta gamma delta')
        self.assertEqual(row['missing_classes']['merged_or_split'], 2)

    def test_all_added_classes_from_constructed_pairs(self):
        label = fidelity.vocabulary(self.policy)['WAITING']
        cases = [
            ('restyled_reference_unit', '`alpha beta gamma`'),
            ('question_to_user', 'Which edition?'),
            ('offer_of_help', 'I can help you'),
            ('commentary_with_state_label', label + ' extra'),
            ('commentary_other', 'Unrelated statement'),
        ]
        for category, answer in cases:
            with self.subTest(category=category):
                row = self.pair('alpha beta gamma', answer)
                self.assertEqual(row['analysis'], 'complete')
                self.assertEqual(row['added_classes'][category], 1)
                self.assertEqual(sum(row['added_classes'].values()), 1)

    def test_kept_pair_has_no_line_classes_even_with_allowed_commentary(self):
        source = 'Title\nalpha beta gamma'
        row = d.analyze(source, 'One neutral comment\n' + source, self.policy)
        self.assertEqual(row['facts_kept'], 'pass')
        self.assertEqual(sum(row['missing_classes'].values()), 0)
        self.assertEqual(sum(row['added_classes'].values()), 0)

    def test_choices_states_numbers_and_fence_are_independent(self):
        source = 'Which edition?\n- Edition A: first meaning\n- Edition B: second meaning'
        row = d.analyze(source, '```text\n' + source + '\n```', self.policy, 'ask')
        self.assertTrue(row['choices_present'])
        self.assertTrue(row['inside_one_code_fence'])
        self.assertEqual(row['answer_lines'], row['reference_lines'] + 2)
        row = d.analyze(source, source.replace('Edition B', 'Other edition'), self.policy, 'ask')
        self.assertFalse(row['choices_present'])
        label = fidelity.vocabulary(self.policy)['WAITING']
        row = self.pair('80 1-2 ' + label + ' ' + label, '81 1-2 ' + label)
        self.assertFalse(row['states_same_count'])
        self.assertFalse(row['numbers_same_count'])
        self.assertTrue(row['choices_present'])  # Non-asks have an empty choice set.

    def test_resource_bounds_are_unknown_with_reasons(self):
        for reference, answer, reason in [
            ('x', 'a' * (d.MAX_TEXT_BYTES + 1), 'text_bytes_limit'),
            ('x', 'a\n' * (d.MAX_LINES + 1), 'line_limit'),
            ('x', 'a ' * (d.MAX_TOKENS + 1), 'token_limit'),
        ]:
            row = d.analyze(reference, answer, self.policy)
            self.assertEqual(row['analysis'], 'unknown')
            self.assertEqual(row['reason_code'], reason)
            self.assertIsNone(row['missing_classes'])
        with patch.object(d, 'MAX_WORK', 1):
            self.assertEqual(self.pair('alpha beta gamma', 'other values')['reason_code'], 'comparison_budget')
        with patch.object(d, 'MAX_UNITS', 1):
            self.assertEqual(self.pair('alpha beta gamma', 'other values')['reason_code'], 'reference_units_limit')

    def test_saved_private_fields_and_paths_never_reach_output(self):
        secret = 'PRIVATE_RAW_SENTINEL'
        with tempfile.TemporaryDirectory(prefix=secret) as directory:
            root = Path(directory)
            (root / 'measurement.json').write_text(json.dumps({
                'request': 'Q11', 'mode': 'isolated', 'model': 'haiku',
                'private_field': secret, 'answer': secret, 'display': secret}))
            events = [
                {'type': 'assistant', 'message': {'content': [
                    {'type': 'tool_use', 'id': 'one', 'name': 'mcp__academy__academy_unit_status', 'input': {'private_field': secret}}]}},
                {'type': 'user', 'message': {'content': [
                    {'type': 'tool_result', 'tool_use_id': 'one', 'content': 'Title\n' + secret}]}},
                {'type': 'result', 'result': 'Title\nchanged wording'}]
            (root / 'claude.jsonl').write_text('\n'.join(map(json.dumps, events)))
            (root / 'run-log.jsonl').write_text(json.dumps({'tool': 'academy_unit_status', 'code': None}) + '\n')
            out = io.StringIO()
            with patch('sys.argv', ['diffclass.py', str(root)]), contextlib.redirect_stdout(out):
                self.assertEqual(d.main(), 0)
            encoded = out.getvalue()
            self.assertNotIn(secret, encoded)
            self.assertNotIn(directory, encoded)
            self.assertNotIn('private_field', encoded)
            self.assertEqual(json.loads(encoded)['display_kind'], 'result')

    def test_invalid_identity_is_not_reflected(self):
        with tempfile.TemporaryDirectory() as directory:
            rows = Path(directory) / 'rows.jsonl'
            rows.write_text(json.dumps({'request': '/private/secret', 'model_tier': 'low', 'mode': 'isolated', 'turn': 1}))
            out = io.StringIO()
            with patch('sys.argv', ['diffclass.py', '--rows', str(rows)]), contextlib.redirect_stdout(out):
                self.assertEqual(d.main(), 2)
            self.assertNotIn('/private/secret', out.getvalue())
            self.assertEqual(json.loads(out.getvalue())['reason_code'], 'unsupported_row_identity')

    def test_saved_file_limit(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'file'
            path.write_text('12345')
            with self.assertRaisesRegex(d.Unknown, 'saved_file_bytes_limit'):
                d.read_bounded(path, 4)


if __name__ == '__main__':
    unittest.main()
