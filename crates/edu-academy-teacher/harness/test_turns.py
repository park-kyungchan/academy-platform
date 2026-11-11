"""Offline two-turn driver tests; the child is a fictional stream client."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import run
import rescore


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
                raw, _, error, _ = run.drive_turns([sys.executable, str(client)], turns, root, root, timeout=2)
                self.assertEqual(error, "earlier_turn_error" if fail else None)
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
            raw, _, _, _ = run.drive_turns([sys.executable, str(client)], self.specs(), root, root, timeout=2)
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
                self.assertTrue(spec['clock_expectation'] or (q == 14 and spec is specs[0]))

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
            self.assertEqual(row['reason_code'], 'isolation_failed:unexpected_tools')

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

    def test_unsent_export_is_unknown_with_each_driver_stop_reason(self):
        cases = {
            'earlier_turn_error': "print(json.dumps({'type':'result','result':'answer','is_error':True}),flush=True)",
            'first_request_tokens_not_under_6000': "print(json.dumps({'type':'assistant','message':{'usage':{'input_tokens':6000,'cache_read_input_tokens':0,'cache_creation_input_tokens':0}}}),flush=True)\nprint(json.dumps({'type':'result','result':'answer'}),flush=True)",
            'client_timeout': 'time.sleep(0.15)',
            'client_ended_before_all_turns': 'pass',
        }
        for reason, body in cases.items():
            with self.subTest(reason=reason), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                client = root / 'client.py'
                client.write_text('import json,sys,time\nsys.stdin.readline()\n' + body + '\n')
                raw, _, error, sent = run.drive_turns([sys.executable,str(client)],self.specs(),root,root,timeout=0.05 if reason=='client_timeout' else 2)
                self.assertEqual(error,reason)
                self.assertEqual(sent,1)
                results=run.score_turns(raw,self.specs(),[], 'isolated',None,error,sent)
                row=run.public_row(results[1],'Q12','haiku','isolated')
                self.assertEqual(row['result'],'unknown')
                self.assertEqual(row['reason_code'],reason)
                self.assertEqual(row['unknown_by_reason'],{reason:1})
                self.assertFalse(row['sent'])

    def test_changed_prior_arguments_fail_q12_and_q14(self):
        for request, topic in [('Q12','DailyTest'),('Q14','클리닉')]:
            specs=self.specs(request);events=self.stream(specs)
            original={'student':'가상학생B','topic':topic}
            if request=='Q12':original['unit']='4-4'
            events[1]['message']['content'][0]['input']=dict(original)
            second=dict(original)
            if request=='Q14':second['book']='가우스 5-1-1'
            events[4]['message']['content'][0]['input']=second
            rows=run.score_turns('\n'.join(map(json.dumps,events)),specs,['mcp__academy__academy_unit_status'],'isolated',None)
            self.assertEqual(rows[1]['checks']['prior_arguments'],'pass')
            second['unit' if request=='Q12' else 'student']='changed'
            rows=run.score_turns('\n'.join(map(json.dumps,events)),specs,['mcp__academy__academy_unit_status'],'isolated',None)
            self.assertEqual(rows[1]['checks']['prior_arguments'],'FAIL')
            self.assertEqual(run.public_row(rows[1],request,'haiku','isolated')['result'],'FAIL')

    def q15_row(self, server_fixture='display-q15.txt', drop=False):
        specs=self.specs('Q15');events=self.stream(specs)
        text=(run.CRATE/'tests/fixtures'/server_fixture).read_text()
        events[2]['message']['content'][0]['content']=text
        events[3]['result']='\n'.join(line for line in text.splitlines() if not (drop and line.endswith('문항 번호 1003')))
        logs=[{'tool':'academy_unit_status','display_bytes':len(text.encode())}]
        results=run.score_turns('\n'.join(map(json.dumps,events)),specs,['mcp__academy__academy_unit_status'],'isolated',logs)
        return run.public_row(results[0],'Q15','haiku','isolated')

    def test_q15_item_lines_pass_and_a_missing_number_fails(self):
        row=self.q15_row()
        self.assertEqual(row['result'],'pass')
        self.assertEqual(row['facts_kept'],'pass')
        self.assertEqual(row['strict_activity_scope'],'pass')
        row=self.q15_row(drop=True)
        self.assertEqual(row['result'],'FAIL')
        self.assertEqual(row['facts_kept'],'FAIL')

    def test_q15_broad_honest_rendering_and_strict_scope_are_separate(self):
        row=self.q15_row('display-q15-bare.txt')
        self.assertEqual(row['honest_rendering'],'pass')
        self.assertEqual(row['strict_activity_scope'],'FAIL')
        self.assertEqual(row['result'],'FAIL')

    def test_explicit_scenario_v2_is_retained(self):
        explicit=run.CRATE/'tests/fixtures/scenario-v2.json'
        self.assertEqual(run.select_scenario(str(explicit),{'turns':[]}),explicit.resolve())
        self.assertEqual(run.select_scenario(None,{'turns':[]}).name,'scenario-v3.json')

    def test_jsonl_only_contains_harness_rows_of_sent_turns(self):
        import contextlib,io,os
        specs=self.specs();events=self.stream(specs[:1])
        results=run.score_turns('\n'.join(map(json.dumps,events)),specs,['mcp__academy__academy_unit_status'],'isolated',None,'earlier_turn_error',1)
        rows=[dict(run.public_row(r,'Q12','haiku','isolated'),trial=True) for r in results]
        with tempfile.TemporaryDirectory() as directory, contextlib.redirect_stdout(io.StringIO()):
            output=Path(directory)/'rows.jsonl'
            run.emit_rows(rows,output)
            saved=[json.loads(line) for line in output.read_text().splitlines()]
            self.assertEqual(saved,[rows[0]])
            self.assertTrue(saved[0]['trial'])
            self.assertEqual(os.stat(output).st_mode&0o777,0o600)

    def test_native_repeated_init_requires_same_session_and_unchanged_environment(self):
        specs=self.specs();events=self.stream(specs)
        events[0]['session_id']='fictional-session'
        events.insert(4,copy.deepcopy(events[0]))
        def score():
            return run.score_turns('\n'.join(map(json.dumps,events)),specs,['mcp__academy__academy_unit_status'],'isolated',None)
        self.assertEqual([r['isolation']['status'] for r in score()],['pass','pass'])
        events[4]['tools'].append('Bash')
        self.assertEqual([r['isolation']['status'] for r in score()],['FAIL','FAIL'])
        events[4]['tools'].pop();events[4]['session_id']='another-session'
        self.assertEqual([r['isolation']['status'] for r in score()],['FAIL','FAIL'])

    def test_token_bound_stops_only_isolated_and_reports_installed_first_request(self):
        for mode in ('isolated','installed'):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                root=Path(directory);client=root/'client.py'
                client.write_text("""import json,sys
for line in sys.stdin:
 print(json.dumps({'type':'assistant','message':{'usage':{'input_tokens':1,'cache_read_input_tokens':3000,'cache_creation_input_tokens':3000}}}),flush=True)
 print(json.dumps({'type':'result','result':'answer'}),flush=True)
""")
                raw,_,reason,sent=run.drive_turns([sys.executable,str(client)],self.specs(),root,root,timeout=2,mode=mode)
                self.assertEqual(sent,1 if mode=='isolated' else 2)
                self.assertEqual(reason,'first_request_tokens_not_under_6000' if mode=='isolated' else None)
                rows=run.score_turns(raw,self.specs(),[],mode,None,reason,sent)
                public=run.public_row(rows[0],'Q12','haiku',mode,conversation=2)
                self.assertEqual(public['first_request_tokens'],6001)
                self.assertEqual(public['conversation'],2)

    def test_conversation_annotation_retains_previous_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'runs.jsonl'
            original=b'{"request":"Q12","turn":1}\n'
            path.write_bytes(original)
            run.annotate_conversations(path)
            self.assertEqual(json.loads(path.read_text())['conversation'],1)
            self.assertEqual(path.with_name('runs.before-conversation.jsonl').read_bytes(),original)
            run.annotate_conversations(path)
            self.assertEqual(len(path.read_text().splitlines()),1)

    def test_export_names_all_failed_checks_and_none_for_a_passing_turn(self):
        specs = self.specs('Q15')
        text = (run.CRATE / 'tests/fixtures' / specs[0]['fixture']).read_text()
        log = [{'tool': 'academy_unit_status', 'display_bytes': len(text.encode())}]
        result = run.score_turns('\n'.join(map(json.dumps, self.stream(specs))), specs,
                                ['mcp__academy__academy_unit_status'], 'isolated', log)[0]
        self.assertEqual(run.public_row(result, 'Q15', 'haiku', 'isolated')['failed_request_checks'], [])
        result['checks'] = {'zeta': 'FAIL', 'alpha': 'FAIL', 'good': 'pass', 'clock': 'not_applicable'}
        row = run.public_row(result, 'Q15', 'haiku', 'isolated')
        self.assertEqual(row['failed_request_checks'], ['alpha', 'zeta'])
        result['checks']['alpha'] = 'unknown'
        self.assertEqual(run.failed_request_checks(result), ['alpha', 'zeta'])

    def test_rebuild_failed_checks_replays_evidence_and_preserves_other_bytes(self):
        import contextlib, io
        specs = self.specs('Q15')
        events = self.stream(specs)
        # A broader server result can keep facts but fail the narrow fixture.
        broad = (run.CRATE / 'tests/fixtures/display-q15-bare.txt').read_text()
        events[2]['message']['content'][0]['content'] = broad
        events[3]['result'] = broad
        log = [{'tool': 'academy_unit_status', 'display_bytes': len(broad.encode())}]
        raw = '\n'.join(map(json.dumps, events))
        result = run.score_turns(raw, specs, ['mcp__academy__academy_unit_status'], 'isolated', log)[0]
        row = dict(run.public_row(result, 'Q15', 'haiku', 'isolated'), trial=False)
        expected = row.pop('failed_request_checks')
        self.assertIn('raw_fixture', expected)
        with tempfile.TemporaryDirectory() as directory, contextlib.redirect_stdout(io.StringIO()):
            root = Path(directory)
            (root / 'measurement.json').write_text(json.dumps({'sent_turns': 1, 'error': None}))
            (root / 'claude.jsonl').write_text(raw)
            (root / 'run-log.jsonl').write_text(json.dumps(log[0]) + '\n')
            path = root / 'rows.jsonl'
            original = (json.dumps(row, sort_keys=True, separators=(',', ': ')) + '\r\n').encode()
            path.write_bytes(original)
            with patch('diffclass.resolve', return_value=root):
                rescore.rebuild_rows(path)
            changed = path.read_bytes()
            addition = b', "failed_request_checks": ' + json.dumps(expected, ensure_ascii=False).encode()
            self.assertEqual(changed, original[:-3] + addition + original[-3:])
            self.assertEqual(path.with_name('rows.before-failed-checks.jsonl').read_bytes(), original)
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            rebuilt = json.loads(changed)
            self.assertEqual(rebuilt.pop('failed_request_checks'), expected)
            self.assertEqual(rebuilt, row)


if __name__ == '__main__':
    unittest.main()
