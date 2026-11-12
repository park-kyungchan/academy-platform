"""Constructed call contracts; no model, private capture or build dependency."""
import copy
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import request_checks as c
import run
import acceptance


def specs(request):
    return json.loads((run.HERE/'checks.json').read_text())[request]['turns']


def envelope(spec, display='Title\nStatement'):
    r = spec['required_arguments']
    if r.get('ask'):
        return {'status':'ask', 'ask':{'field':r['ask']}, 'display':display, 'data':{}}
    return {'status':'ok', 'display':display, 'data':dict(r, minors=[{'minor':{'code':u}} for u in r['minors']])}


def call(spec, display='Title\nStatement'):
    r = spec['required_arguments']
    args = {'student':r['student']+' (가상반A)', 'topic':r['topic']}
    if r.get('book_explicit'): args['book']=r['edition']
    if len(r.get('minors',[])) == 1:
        args['unit']=r['minors'][0]+' '+(r.get('activity') or r.get('level') or '')
    return {'name':'mcp__academy__academy_unit_status', 'arguments':json.dumps(args), 'received_display':display}


class RequestChecks(unittest.TestCase):
    def test_integrated_honest_alternative_and_broader_q12_answer(self):
        from test_turns import TurnTests
        spec=specs('Q12')[0]
        for basic in (True,False):
            events=TurnTests().stream([spec]); response=envelope(spec,events[3]['result'])
            args=json.loads(call(spec)['arguments'])
            if not basic:
                args['unit']='4-4'; response['data']['level']=None
            events[1]['message']['content'][0]['input']=args
            log=[{'tool':'academy_unit_status','display_bytes':len(response['display'].encode())}]
            result=run.score_turns('\n'.join(map(json.dumps,events)),[spec],
                ['mcp__academy__academy_unit_status'],'isolated',log,replayed=[response])[0]
            self.assertEqual(result['facts_kept'],'pass')
            self.assertEqual(result['checks']['server_replay'],'pass')
            self.assertEqual(result['checks']['required_arguments'],'pass' if basic else 'FAIL')

    def test_matrix_plan_has_all_samples_and_does_not_stop_on_failed_turns(self):
        import tempfile
        self.assertEqual(len(acceptance.plan()),90)
        self.assertEqual(sum(len(specs(q)) for _,q,_ in acceptance.plan()),120)
        with tempfile.TemporaryDirectory() as directory:
            output=Path(directory)/'runs.jsonl'; output.touch()
            with patch.object(acceptance,'plan',return_value=[('isolated','Q11',1)]*2), \
                 patch.object(acceptance,'conversation',return_value=([], 'earlier_turn_error')) as invoke, \
                 patch('builtins.print'):
                acceptance.matrix(output)
            self.assertEqual(invoke.call_count,2)

    def test_every_request_turn_accepts_an_alternative_and_rejects_wrong_student_topic_and_scope(self):
        for request in ('Q11','Q12','Q13','Q14','Q15','Q16'):
            for i,spec in enumerate(specs(request)):
                with self.subTest(request=request, turn=i+1):
                    value, response = call(spec), envelope(spec)
                    previous = envelope(specs(request)[i-1]) if i else None
                    self.assertTrue(all(v=='pass' for v in c.evaluate(spec,[value],[response],previous).values()))
                    for key in ('student','topic'):
                        wrong=copy.deepcopy(value); args=json.loads(wrong['arguments']); args[key]='wrong'
                        wrong['arguments']=json.dumps(args)
                        self.assertEqual(c.evaluate(spec,[wrong],[response],previous)['required_arguments'],'FAIL')
                    wrong=copy.deepcopy(response)
                    if spec['required_arguments'].get('ask'): wrong['ask']['field']='student'
                    else: wrong['data']['minors']=[]
                    self.assertEqual(c.evaluate(spec,[value],[wrong],previous)['required_arguments'],'FAIL')

    def test_q12_level_omission_fails_even_when_broader_display_is_delivered_honestly(self):
        spec=specs('Q12')[0]; value=call(spec); response=envelope(spec)
        args=json.loads(value['arguments']); args['unit']='4-4'; value['arguments']=json.dumps(args)
        response['data']['level']=None
        scored=c.evaluate(spec,[value],[response],None)
        self.assertEqual(scored['required_arguments'],'FAIL')
        self.assertEqual(scored['server_replay'],'pass')

    def test_explicit_book_presence_and_q14_chosen_book_are_required(self):
        for request,index in (('Q11',0),('Q14',1)):
            spec=specs(request)[index]; value=call(spec); response=envelope(spec)
            previous=envelope(specs('Q14')[0]) if index else None
            args=json.loads(value['arguments']); args.pop('book'); value['arguments']=json.dumps(args)
            self.assertEqual(c.evaluate(spec,[value],[response],previous)['required_arguments'],'FAIL')
            value=call(spec); response['data']['edition']='wrong book'
            self.assertEqual(c.evaluate(spec,[value],[response],previous)['required_arguments'],'FAIL')

    def test_q15_activity_is_in_unit_independently_of_the_display(self):
        spec=specs('Q15')[0]; response=envelope(spec); value=call(spec)
        self.assertEqual(c.evaluate(spec,[value],[response],None)['activity_scope'],'pass')
        args=json.loads(value['arguments']); args['unit']='1-2'; args['book']='필수예제'
        value['arguments']=json.dumps(args)
        result=c.evaluate(spec,[value],[response],None)
        self.assertEqual(result['activity_scope'],'FAIL')
        self.assertEqual(result['server_replay'],'pass')

    def test_wrong_tool_count_order_and_changed_received_text_fail_independently(self):
        spec=specs('Q14')[1]; value=call(spec); response=envelope(spec); previous=envelope(specs('Q14')[0])
        self.assertEqual(c.evaluate(spec,[value],[response],previous)['call_order'],'pass')
        self.assertEqual(c.evaluate(spec,[value],[response],None)['call_order'],'FAIL')
        self.assertEqual(c.evaluate(spec,[value,value],[response,response],previous)['call_order'],'FAIL')
        wrong=dict(value,name='mcp__academy__academy_lesson_prepare')
        self.assertEqual(c.evaluate(spec,[wrong],[response],previous)['expected_tool'],'FAIL')
        self.assertEqual(c.evaluate(spec,[],[],previous)['call_order'],'FAIL')
        self.assertEqual(c.evaluate(spec,[value],None,previous)['server_replay'],'unknown')
        wrong=dict(value,received_display='changed')
        self.assertEqual(c.evaluate(spec,[wrong],[response],previous)['server_replay'],'FAIL')

    def test_replay_keeps_exact_arguments_and_order_and_fails_closed_on_wrong_response_ids(self):
        spec=specs('Q12')[0]; calls=[call(spec),call(spec)]
        responses=[{'id':i,'result':{'_meta':{'academy/envelope':envelope(spec,str(i))}}} for i in range(2)]
        completed=subprocess.CompletedProcess([],0,'\n'.join(map(json.dumps,responses)),'')
        with patch.object(c.subprocess,'run',return_value=completed) as invoke:
            replayed=c.replay(Path('fictional-binary'),Path('fictional-scenario'),calls)
        self.assertEqual([r['display'] for r in replayed],['0','1'])
        sent=[json.loads(v) for v in invoke.call_args.kwargs['input'].splitlines()]
        self.assertEqual([r['id'] for r in sent],[0,1])
        self.assertEqual([r['params']['arguments'] for r in sent],[json.loads(v['arguments']) for v in calls])
        responses[0]['id']=1; completed.stdout='\n'.join(map(json.dumps,responses))
        with patch.object(c.subprocess,'run',return_value=completed), self.assertRaisesRegex(ValueError,'replay_response_order'):
            c.replay(Path('fictional-binary'),Path('fictional-scenario'),calls)
