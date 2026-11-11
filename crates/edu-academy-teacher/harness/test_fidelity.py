"""Strict scorer API and offline export checks; invented displays only."""
import json
import unittest
from unittest.mock import patch

import fidelity


class AcceptanceTests(unittest.TestCase):
    FIRST = 'Alice | RangeAlpha | TaskAlpha | 미확인(실물 확인 전) | SourceAlpha | NextAlpha'
    SECOND = 'Bob | RangeBeta | TaskBeta | 미확인(실물 확인 전) | SourceBeta | NextBeta'
    TEXT = ('Example title\n알림\n없음\n학생별 할 일\n학생 | 범위 | 할 일 | 상태 | 근거 | 다음 확인\n'
            + FIRST + '\n' + SECOND + '\n미확인 사항\nUnresolved fact\n다음 확인\n- Review item\n'
            '읽은 원천 Source listing\n이번 결과는 읽기만 했고 저장한 것은 없습니다.')

    def result(self, answer):
        return fidelity.evaluate(self.TEXT, answer, fidelity.configured_policy())

    def test_exact_structure_counts_and_no_additions(self):
        result = self.result(self.TEXT)
        self.assertEqual(result['facts_kept'],'pass')
        counts=result['fidelity']
        self.assertEqual((counts['rows_total'],counts['rows_kept']), (2,2))
        self.assertEqual((counts['cells_total'],counts['cells_kept']), (12,12))
        self.assertEqual(counts['added_lines'],0)
        for kind in fidelity.KINDS:
            self.assertEqual(counts['nonrows'][kind]['kept'],counts['nonrows'][kind]['total'])

    def test_missing_row_is_not_kept(self):
        self.assertEqual(self.result(self.TEXT.replace(self.FIRST+'\n',''))['facts_kept'],'FAIL')

    def test_provenance_loss_counted_separately(self):
        answer=self.TEXT.replace('읽은 원천 Source listing\n','').replace('이번 결과는 읽기만 했고 저장한 것은 없습니다.','')
        result=self.result(answer)
        self.assertEqual(result['facts_kept'],'pass')
        for kind in ('sources','closing'):
            self.assertEqual(result['fidelity']['nonrows'][kind],{'total':1,'kept':0})

    def test_missing_title_is_context_loss(self):
        result=self.result(self.TEXT.replace('Example title\n',''))
        self.assertEqual(result['facts_kept'],'FAIL')
        self.assertIn(2,result['failed_rules'])

    def test_commentary_is_counted_whole_line(self):
        result=self.result('One introductory sentence\n'+self.TEXT)
        self.assertEqual(result['facts_kept'],'pass')
        self.assertEqual(result['fidelity']['added_lines'],1)
        self.assertEqual(result['fidelity']['added_chars'],25)

    def test_missing_empty_or_unconfigured_evidence_unknown(self):
        for server,answer,policy in [(None,self.TEXT,fidelity.configured_policy()),
                                    (self.TEXT,'',fidelity.configured_policy()),(self.TEXT,self.TEXT,None)]:
            self.assertEqual(fidelity.evaluate(server,answer,policy)['facts_kept'],'unknown')

    def test_ask_whole_lines_and_choices_not_vacuous_rows(self):
        text='Choose a class\n- ClassAlpha: explanation\n- ClassBeta: explanation'
        policy=fidelity.configured_policy('Q03')
        self.assertEqual(fidelity.evaluate(text,text,policy)['facts_kept'],'pass')
        self.assertEqual(fidelity.evaluate(text,'Choose a class',policy)['facts_kept'],'FAIL')
        self.assertEqual(fidelity.evaluate(text,text.replace('ClassAlpha','ClassAlphaX'),policy)['facts_kept'],'FAIL')

    def test_missing_evidence_exports_status_only(self):
        with patch.object(fidelity,'score_saved',side_effect=FileNotFoundError),patch(
                'sys.argv',['fidelity.py','/invented/missing']),patch('builtins.print') as output:
            self.assertEqual(fidelity.main(),2)
        self.assertEqual(output.call_args.args,('{"status": "evidence_unavailable"}',))

    def test_export_has_no_reference_or_answer_text(self):
        expected={'shown_unchanged':'FAIL','facts_kept':'FAIL','failed_rules':[1]}
        with patch.object(fidelity,'score_saved',return_value=expected),patch(
                'sys.argv',['fidelity.py','/invented/run']),patch('builtins.print') as output:
            self.assertEqual(fidelity.main(),0)
        self.assertEqual(json.loads(output.call_args.args[0]),expected)
