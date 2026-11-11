"""Termination, pruning and declared-limit regressions, using invented values only."""
import copy
import json
import signal
import time
import unittest
import fidelity


def ambiguous(n, reverse=False):
    prefix='Title\nPart\nName | Action\n'
    source=prefix+'\n'.join(line for i in range(n) for line in (f'| A{i} | R |',f'| A{i} | R / tail{i} |'))+'\n'+'\n'.join(f'tail{i}' for i in range(n))
    short=[f'A{i} | R\ntail{i}' for i in range(n)]
    long=[f'A{i} | R / tail{i}' for i in range(n)]
    return source,prefix+'\n'.join(long+short if reverse else short+long)


class SearchBounds(unittest.TestCase):
    def test_large_identity_iterative(self):
        source='Title\nPart\nName | Count\n'+'\n'.join(f'A{i} | 1' for i in range(2000))
        start=time.perf_counter()
        self.assertEqual(fidelity.evaluate(source,source,fidelity.configured_policy())['facts_kept'],'pass')
        print('large_identity_seconds='+str(round(time.perf_counter()-start,3)))

    def test_suffix_feasibility_both_orders(self):
        times={}
        def timeout(*args):raise TimeoutError('suffix search exceeded one second')
        previous=signal.signal(signal.SIGALRM,timeout)
        try:
            for n in (16,32,64):
                for reverse in (False,True):
                    source,answer=ambiguous(n,reverse);start=time.perf_counter();signal.setitimer(signal.ITIMER_REAL,1)
                    try:self.assertEqual(fidelity.evaluate(source,answer,fidelity.configured_policy())['facts_kept'],'pass')
                    finally:signal.setitimer(signal.ITIMER_REAL,0)
                    times[f'{n}:{int(reverse)}']=round(time.perf_counter()-start,3)
                    wrong=answer.replace('A0 | R / tail0','A0 | WRONG',1)
                    result=fidelity.evaluate(source,wrong,fidelity.configured_policy())
                    self.assertEqual(result['facts_kept'],'FAIL');self.assertIn(1,result['failed_rules']);self.assertIn(4,result['failed_rules'])
        finally:signal.signal(signal.SIGALRM,previous)
        print('suffix_seconds='+json.dumps(times,sort_keys=True))

    def test_each_declared_limit_reason(self):
        source='Title\nPart\nName | Action\nAlice | Review'
        for key,reason in [('reference_units','reference_too_large'),('answer_bytes','answer_too_large'),('search_states','search_budget')]:
            policy=copy.deepcopy(fidelity.configured_policy());policy['limits'][key]=1
            result=fidelity.evaluate(source,source,policy)
            self.assertEqual(result['facts_kept'],'unknown');self.assertEqual(result['reason_code'],reason)

    def test_limits_cover_observed_sizes(self):
        limits=fidelity.configured_policy()['limits']
        self.assertGreaterEqual(limits['reference_units'],370)
        self.assertGreaterEqual(limits['answer_bytes'],51610)

    def test_generated_multiple_populated_students(self):
        import test_fidelity_property as properties
        labels=list(fidelity.vocabulary(fidelity.configured_policy()).values())
        found=False
        for seed in range(properties.CASES):
            m=properties.metadata(properties.generated(seed,labels)); bindings={row[3][:2] for row in m['rows']}
            if any(a[0]==b[0] and a[1]!=b[1] for a in bindings for b in bindings):found=True
        self.assertTrue(found)

    def test_reordering_is_never_identity(self):
        import test_fidelity_property as properties
        identical=[(0,'same',[],()),(1,'same',[],())]
        palindrome=[(0,'a',[],()),(1,'b',[],()),(2,'a',[],())]
        choices=properties.reorderings([identical,palindrome])
        self.assertEqual(len(choices),1)
        for group,changed in choices:self.assertNotEqual([row[1] for row in group],changed)
