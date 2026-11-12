"""Diagnostic accounting from constructed synthetic text; no saved evidence."""
import json
import subprocess
import unittest
from unittest.mock import patch

import diagnose_s2d as ds
import fidelity
import request_checks
from test_request_checks import envelope, specs, call


class DiagnosticTests(unittest.TestCase):
    def test_early_refusal_does_not_invent_downstream_wrong_identity_or_topic(self):
        spec=specs('Q12')[0]
        args={'student':'가상학생B','book':'unrecognized synthetic book','topic':'DailyTest'}
        refused={'status':'stop','stop':{'code':'book_not_found'},'data':{}}
        self.assertEqual(set(ds.wrong_elements(spec,args,refused)),
                         {('book','wrong_value'),('level','absent_everywhere')})
        refused={'status':'ask','ask':{'field':'unit'},'data':{}}
        args['unit']='4-4 기본'
        self.assertIn(('level','present_in_unit_not_resolved'),ds.wrong_elements(spec,args,refused))
        self.assertNotIn('student',[k for k,_ in ds.wrong_elements(spec,args,refused)])

    def test_physical_line_counts_and_added_positions_preserve_multiplicity(self):
        policy=fidelity.configured_policy()
        source='Title\nRepeated\nRepeated\nLast'
        answer='Before\nTitle\nRepeated\nBetween\nLast\nAfter'
        missing,positions=ds.display_diff(source,answer,policy)
        self.assertEqual(sum(missing.values()),1)
        self.assertEqual(dict(positions),{'before':1,'between':1,'after':1})
        _,positions=ds.display_diff(source,'Entirely different',policy)
        self.assertEqual(dict(positions),{'unanchored':1})

    def test_closed_argument_shapes_and_candidate_byte_deltas(self):
        self.assertEqual(ds.locate_word({'private-field':'기본 private payload'},'기본'),('other',))
        candidates=ds.candidates()
        self.assertEqual([c['tools_list_bytes_added'] for c in candidates[2:]],[0,0])
        self.assertTrue(all(c['tools_list_bytes_added']>0 for c in candidates[:2]))
        self.assertEqual(ds.line_kind('- Choice: Meaning',2,'ask'),'choice')
        self.assertEqual(ds.line_kind('Reason. Remedy.',0,'stop'),'stop_reason_and_remedy')
        missing,_=ds.display_diff('Reason. Remedy.','Changed.',fidelity.configured_policy(),'stop')
        self.assertEqual({key[0] for key in missing},{'stop_reason_and_remedy'})

    def test_book_ask_probes_cannot_advance_or_replace_original_display(self):
        spec=specs('Q14')[0]; initial=envelope(spec,'Original ask')
        initial['ask']['options']=[{'value':'가우스 5-1-2'}]
        data=envelope(specs('Q11')[0])['data']; data.update(student='가상학생B',topic='클리닉')
        resolved={'status':'ok','data':data,'display':'Separate probe'}
        def completed(env):
            return subprocess.CompletedProcess([],0,json.dumps({'id':0,'result':{'_meta':{'academy/envelope':env}}})+'\n','')
        value=call(spec)
        with patch.object(request_checks.subprocess,'run',side_effect=[completed(initial),completed(resolved),completed(resolved)]) as invoke:
            result=request_checks.replay('fictional-server','fictional-scenario',[value])
        self.assertEqual(result[0]['display'],'Original ask')
        self.assertEqual(invoke.call_count,3)
        self.assertEqual(json.loads(invoke.call_args_list[0].kwargs['input'])['params']['arguments'],
                         json.loads(value['arguments']))
        self.assertEqual(request_checks.evaluate(spec,[dict(value,received_display='Original ask')],result,None)['required_arguments'],'pass')
