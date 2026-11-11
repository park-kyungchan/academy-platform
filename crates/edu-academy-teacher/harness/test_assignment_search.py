"""Third-round review counterexamples and declared normalization checks."""
import itertools
import unittest
import fidelity


class AssignmentSearchTests(unittest.TestCase):
    def result(self,source,answer):
        return fidelity.evaluate(source,answer,fidelity.configured_policy())

    def test_k1_prefix_rows_both_orders_encodings_and_duplicates(self):
        short='Alice | Review'; long='Alice | Review / book'
        cases=set(itertools.permutations((short,long))) | set(itertools.permutations((short,long,long))) | set(itertools.permutations((short,short,long)))
        for rows in sorted(cases):
            a='Title\nPart\nName | Action\n'+'\n'.join(rows)
            b=a.replace('Review / book','Review\nbook')
            for source,answer in [(a,a),(b,b),(a,b),(b,a)]:
                with self.subTest(rows=rows,direction=(len(source),len(answer))):
                    self.assertEqual(self.result(source,answer)['facts_kept'],'pass')

    def test_k2_required_additions_and_opposites_fail(self):
        source='Title\n미확인 사항\nNeed book\n다음 확인\nCheck book'
        self.assertEqual(self.result(source,source)['facts_kept'],'pass')
        for answer in [source.replace('Need book','Need book\nDo not bring the book'),source+'\nAdditional next action']:
            with self.subTest(length=len(answer)):
                self.assertEqual(self.result(source,answer)['facts_kept'],'FAIL')
                self.assertIn(4,self.result(source,answer)['failed_rules'])

    def test_separator_policy_is_explicit_and_symmetric(self):
        self.assertEqual(fidelity.configured_policy()['normalization'].get('table_separator'),'ignore_whole_lines')
        a='Title\nPart\nName | Action\nAlice | Review'
        b=a.replace('Name | Action','Name | Action\n| --- | --- |')
        for source,answer in [(a,b),(b,a)]:
            self.assertEqual(self.result(source,answer)['facts_kept'],'pass')
        self.assertEqual(self.result(a,a.replace('Alice | Review','| --- | --- |'))['facts_kept'],'FAIL')
        self.assertEqual(self.result('| --- | --- |','| --- | --- |')['facts_kept'],'unknown')

    def test_fixed_public_required_section_addition_is_not_commentary(self):
        for path in sorted((fidelity.CRATE/'tests/fixtures').glob('display-*.txt')):
            source=path.read_text()
            if '미확인 사항\n' not in source: continue
            self.assertEqual(self.result(source,source)['facts_kept'],'pass')
            self.assertEqual(self.result(source,source.replace('미확인 사항\n','미확인 사항\nInvented statement\n',1))['facts_kept'],'FAIL',path.name)

    def test_k1_longest_group_can_require_backtracking(self):
        source='Title\nPart\nName | Action\nAlice | Review\nAlice | Review / book'
        answer=source.replace('Alice | Review\n','Alice | Review\nbook\n',1)
        self.assertEqual(self.result(source,answer)['facts_kept'],'pass')
        self.assertEqual(self.result(source,answer.replace('\nbook\n','\nbook\nExtra commentary\n'))['facts_kept'],'FAIL')
