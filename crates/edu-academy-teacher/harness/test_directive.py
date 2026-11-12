"""M24 carrier extraction, delivery lengths, and unchanged M21 echo accounting."""
import json
import unittest

import fidelity
import run


class DirectiveTests(unittest.TestCase):
    def test_only_exact_leading_carrier_is_removed_and_legacy_display_survives(self):
        display = 'Title\nStatement'
        directive = {'type': 'text', 'text': run.DISPLAY_DIRECTIVE}
        block = {'type': 'text', 'text': display}
        for content in ([directive, block], run.DISPLAY_DIRECTIVE+'\n'+display,
                        [{'type': 'text', 'text': run.DISPLAY_DIRECTIVE+'\n\n'+display}]):
            self.assertEqual(run.display_text(content), display)
        for content in (display, [block]):
            self.assertEqual(run.display_text(content), display)
        self.assertEqual(run.display_text([block, directive]), display+'\n'+run.DISPLAY_DIRECTIVE)
        self.assertIsNone(run.display_text([directive, {'type': 'image'}]))
        changed = run.DISPLAY_DIRECTIVE + ' changed\n' + display
        self.assertEqual(run.display_text(changed), changed)

    def test_directive_is_excluded_from_reference_but_echo_is_an_added_answer_line(self):
        source = 'Title\nStatement'
        for count in (0, 1, 2):
            answer = '\n'.join([run.DISPLAY_DIRECTIVE]*count + [source])
            events = [
                {'type': 'system', 'subtype': 'init', 'tools': ['mcp__academy__academy_unit_status']},
                {'type': 'assistant', 'message': {'content': [{'type': 'tool_use', 'id': 'one', 'name': 'mcp__academy__academy_unit_status', 'input': {}}]}},
                {'type': 'user', 'message': {'content': [{'type': 'tool_result', 'tool_use_id': 'one', 'content': [
                    {'type': 'text', 'text': run.DISPLAY_DIRECTIVE}, {'type': 'text', 'text': source}]}]}},
                {'type': 'result', 'result': answer}]
            result = run.score_stream('\n'.join(map(json.dumps, events)), {'fidelity_policy': fidelity.configured_policy()},
                ['mcp__academy__academy_unit_status'], mode='installed',
                call_log=[{'tool': 'academy_unit_status', 'display_bytes': len(source.encode())}])
            self.assertEqual(result['display'], source)
            self.assertEqual(result['answer'], answer)
            self.assertEqual(result['received_text_bytes'], len(source.encode()))
            self.assertEqual(result['delivery'], 'same')
            self.assertEqual(result['facts_kept'], 'pass' if count <= 1 else 'FAIL')
            self.assertEqual(result['unit_counts']['added'], count)


if __name__ == '__main__':
    unittest.main()
