"""Offline two-turn driver tests; the child is a fictional stream client."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest

import run


class TurnTests(unittest.TestCase):
    def specs(self, request='Q12'):
        return json.loads((run.HERE / 'checks.json').read_text())[request]['turns']

    def test_driver_orders_teacher_turns_and_never_retries(self):
        for fail in (False, True):
            with self.subTest(fail=fail), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                client = root / 'client.py'
                client.write_text('''import json,sys
from pathlib import Path
seen=[]
for line in sys.stdin:
    seen.append(json.loads(line)['message']['content'])
    Path('seen.json').write_text(json.dumps(seen))
    print(json.dumps({'type':'assistant','message':{'usage':{'input_tokens':1,'cache_read_input_tokens':0,'cache_creation_input_tokens':0}}}),flush=True)
    print(json.dumps({'type':'result','result':'answer','is_error':''' + str(fail) + '''}),flush=True)
''')
                turns = self.specs()
                raw, _, error = run.drive_turns([sys.executable, str(client)], turns, root, root, timeout=2)
                self.assertIsNone(error)
                seen = json.loads((root / 'seen.json').read_text())
                self.assertEqual(seen, [t['request_text'] for t in turns[:1 if fail else 2]])
                self.assertEqual(sum(json.loads(x).get('type') == 'result' for x in raw.splitlines()), len(seen))

    def test_first_request_token_stop_prevents_second_turn(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            client = root / 'client.py'
            client.write_text('''import json,sys
for line in sys.stdin:
 print(json.dumps({'type':'assistant','message':{'usage':{'input_tokens':6000,'cache_read_input_tokens':0,'cache_creation_input_tokens':0}}}),flush=True)
 print(json.dumps({'type':'result','result':'answer'}),flush=True)
''')
            raw, _, _ = run.drive_turns([sys.executable, str(client)], self.specs(), root, root, timeout=2)
            self.assertEqual(sum(json.loads(x).get('type') == 'result' for x in raw.splitlines()), 1)

    def stream(self, specs):
        events = [{'type':'system','subtype':'init','tools':['mcp__academy__academy_unit_status']}]
        for i, spec in enumerate(specs):
            text = (run.CRATE / 'tests/fixtures' / spec['fixture']).read_text()
            events += [
                {'type':'assistant','message':{'usage':{'input_tokens':1,'cache_read_input_tokens':0,'cache_creation_input_tokens':0},'content':[{'type':'tool_use','id':str(i),'name':'mcp__academy__academy_unit_status','input':{'student':'synthetic'}}]}},
                {'type':'user','message':{'content':[{'type':'tool_result','tool_use_id':str(i),'content':text}]}},
                {'type':'result','result':text,'duration_ms':12,'usage':{'input_tokens':i+1}}]
        return events

    def test_scoring_keeps_turn_order_and_each_display_separate(self):
        specs = self.specs()
        events = self.stream(specs)
        rows = run.score_turns('\n'.join(map(json.dumps, events)), specs, ['mcp__academy__academy_unit_status'], 'isolated', None)
        self.assertEqual([r['turn'] for r in rows], [1,2])
        self.assertEqual([r['call_count'] for r in rows], [1,1])
        self.assertEqual([r['shown_unchanged'] for r in rows], ['pass','pass'])
        self.assertEqual([r['facts_kept'] for r in rows], ['pass','pass'])
        self.assertEqual([r['checks']['raw_fixture'] for r in rows], ['pass','pass'])
        self.assertEqual([r['usage']['input_tokens'] for r in rows], [1,2])
        # A later answer must not conceal a failed first answer.
        events[3]['result'] = 'omitted'
        rows = run.score_turns('\n'.join(map(json.dumps, events)), specs, [], 'isolated', None)
        self.assertNotEqual(rows[0]['facts_kept'], 'pass')
        self.assertEqual(rows[1]['facts_kept'], 'pass')

    def test_every_unit_request_declares_real_teacher_text_and_one_call(self):
        for q in range(11,17):
            specs = self.specs(f'Q{q}')
            self.assertEqual(len(specs), 2 if q in (12,14) else 1)
            for spec in specs:
                self.assertTrue(spec['request_text'])
                self.assertEqual(spec['expected_tool'], 'academy_unit_status')
                self.assertEqual(spec['target_calls'], 1)
                self.assertEqual(spec['max_academy_calls'], 1)
                self.assertTrue(spec['clock_expectation'])

    def test_missing_second_turn_is_unknown_not_first_answer_reused(self):
        specs = self.specs()
        events = self.stream(specs[:1])
        rows = run.score_turns('\n'.join(map(json.dumps, events)), specs, [], 'isolated', None)
        self.assertEqual(rows[1]['facts_kept'], 'unknown')
        self.assertEqual(rows[1]['call_count'], 'unknown')

    def test_q11_order_uses_rows_instead_of_book_name_substrings(self):
        specs = self.specs('Q11')
        events = self.stream(specs)
        rows = run.score_turns('\n'.join(map(json.dumps, events)), specs, ['mcp__academy__academy_unit_status'], 'isolated', None)
        self.assertEqual(rows[0]['checks']['unit_row_order'], 'pass')
        text = events[-1]['result'].splitlines()
        text[3], text[4] = text[4], text[3]
        events[-1]['result'] = '\n'.join(text)
        row = run.score_turns('\n'.join(map(json.dumps, events)), specs, ['mcp__academy__academy_unit_status'], 'isolated', None)[0]
        self.assertEqual(row['checks']['unit_row_order'], 'FAIL')

    def test_public_result_rejects_failed_isolation_on_both_turns(self):
        specs = self.specs()
        events = self.stream(specs)
        events[0]['tools'].append('Bash')
        rows = run.score_turns('\n'.join(map(json.dumps, events)), specs, ['mcp__academy__academy_unit_status'], 'isolated', None)
        for result in rows:
            self.assertEqual(result['facts_kept'], 'pass')
            row = run.public_row(result, 'Q12', 'haiku', 'isolated')
            self.assertEqual(row['result'], 'FAIL')
            self.assertEqual(row['reason_code'], 'isolation_failed')

    def test_first_request_budget_does_not_become_second_turn_budget(self):
        specs = self.specs()
        events = self.stream(specs)
        events[4]['message']['usage']['input_tokens'] = 9000
        rows = run.score_turns('\n'.join(map(json.dumps, events)), specs, ['mcp__academy__academy_unit_status'], 'isolated', None)
        self.assertEqual([r['isolation']['first_request_tokens'] for r in rows], [1,1])
        self.assertEqual([r['isolation']['status'] for r in rows], ['pass','pass'])

    def test_nonobject_stream_is_unknown_instead_of_crashing(self):
        rows = run.score_turns('[]\n', self.specs(), [], 'isolated', None)
        self.assertEqual([r['facts_kept'] for r in rows], ['unknown','unknown'])


if __name__ == '__main__':
    unittest.main()
