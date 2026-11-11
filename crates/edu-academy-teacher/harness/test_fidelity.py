"""Invented strings only; no model calls or real run evidence."""
import unittest
from unittest.mock import patch
import fidelity


class FidelityTests(unittest.TestCase):
    def score(self, server, answer, catalog=None):
        return fidelity.score(server, answer, catalog={} if catalog is None else catalog)

    def test_exact_and_headers_excluded(self):
        text = 'Example title\n알림\n학생 | 범위 | 할 일 | 상태 | 근거 | 다음 확인\nAlice | Scope | Task | Pending | Source | Check'
        result = self.score(text, text)
        self.assertEqual((result['cells_total'], result['cells_kept']), (6, 6))
        self.assertEqual((result['rows_total'], result['rows_kept']), (1, 1))
        self.assertEqual(result['added_lines'], 1)  # A header is not a data/nonrow anchor.

    def test_partial_field_order_and_whitespace(self):
        text = 'Title\nAlice | Scope | Task | Pending | Source | Check'
        answer = 'Title\nAlice Scope  Task\nSource Pending Check'
        result = self.score(text, answer)
        self.assertEqual(result['cells_kept'], 5)
        self.assertEqual(result['rows_kept'], 0)

    def test_maximum_ordered_match_can_skip_early_field(self):
        self.assertEqual(fidelity.ordered_fields(['late', 'early', 'middle'], 'early middle late')[0], 2)

    def test_reversed_rows(self):
        first, second = 'A | B | C | D | E | F', 'G | H | I | J | K | L'
        result = self.score('Title\n'+first+'\n'+second, 'Title\n'+second+'\n'+first)
        self.assertEqual(result['rows_kept'], 2)
        self.assertEqual(result['rows_order_kept'], 'no')

    def test_empty_kept_rows_order_is_vacuously_true(self):
        result = self.score('Title\nA | B | C | D | E | F', 'No matching content')
        self.assertEqual(result['rows_order_kept'], 'yes')
        self.assertEqual(result['cells_kept'], 0)

    def test_all_nonrow_categories_and_markdown(self):
        text = ('Title\n알림\n학생별 할 일\nAlice 학원 내 처리\n미확인 사항\nMissing item\n'
                '다음 확인\n- Check item\n읽은 원천 Source\nContinuation\n이번 결과는 읽기만 했고 저장한 것은 없습니다.')
        result = self.score(text, text.replace('Missing item\n', '').replace('Title', '# **Title**'))
        self.assertEqual(result['nonrows']['unknown'], {'total': 1, 'kept': 0})
        for kind in fidelity.KINDS:
            if kind != 'unknown':
                self.assertEqual(result['nonrows'][kind]['total'], result['nonrows'][kind]['kept'])
        self.assertEqual(result['nonrows']['sources']['total'], 2)
        self.assertEqual(result['added_lines'], 0)

    def test_added_lines_and_chars_strip_syntax(self):
        result = self.score('Title\nAlpha | Beta | Gamma | Delta | Epsilon | Zeta', '# Title\n**Extra!**\n- More\n```')
        self.assertEqual((result['added_lines'], result['added_chars']), (2, 10))

    def test_state_multiplicity(self):
        result = self.score('Title\nA | B | C | Pending | Pending | F',
                            'Title A B C Pending F Confirmed', catalog={'PENDING': 'Pending', 'CONFIRMED': 'Confirmed'})
        self.assertEqual(result['state_words_changed'], {'PENDING': {'server': 2, 'answer': 1}, 'CONFIRMED': {'server': 0, 'answer': 1}})

    def test_state_table_and_dated_teacher_without_prefix_double_count(self):
        result = fidelity.score('Title\nA | B | C | 교사 확인(11월 9일) | 교사 확인 | F',
                                'Title A B C 교사 확인(11월 9일) F')
        self.assertEqual(result['state_words_changed']['TEACHER_DATED_11_9'], {'server': 1, 'answer': 1})
        self.assertEqual(result['state_words_changed']['TEACHER'], {'server': 1, 'answer': 0})

    def test_missing_evidence_exports_only_status(self):
        with patch.object(fidelity, 'score_saved', side_effect=FileNotFoundError), patch(
                'sys.argv', ['fidelity.py', '/invented/missing']), patch('builtins.print') as output:
            self.assertEqual(fidelity.main(), 2)
        self.assertEqual(output.call_args.args, ('{"status": "evidence_unavailable"}',))


class AcceptanceTests(unittest.TestCase):
    FIRST = 'Alice | RangeAlpha | TaskAlpha | 미확인(실물 확인 전) | SourceAlpha | NextAlpha'
    SECOND = 'Bob | RangeBeta | TaskBeta | 미확인(실물 확인 전) | SourceBeta | NextBeta'
    TEXT = ('Example title\n알림\n없음\n학생별 할 일\n학생 | 범위 | 할 일 | 상태 | 근거 | 다음 확인\n'
            + FIRST + '\n' + SECOND + '\n미확인 사항\nUnresolved fact\n다음 확인\n- Review item\n'
            '읽은 원천 Source listing\n이번 결과는 읽기만 했고 저장한 것은 없습니다.')

    def result(self, answer):
        return fidelity.evaluate(self.TEXT, answer, 'checklist')

    def test_complete_plain_and_reformatted_pass(self):
        self.assertEqual(self.result(self.TEXT)['facts_kept'], 'pass')
        answer = self.TEXT.replace('Example title', '# Example title').replace('알림', '**알림**')
        self.assertEqual(self.result(answer)['facts_kept'], 'pass')

    def test_missing_field_and_in_row_order_fail(self):
        for answer in [self.TEXT.replace('RangeAlpha', 'Other range'),
                       self.TEXT.replace('Alice | RangeAlpha', 'RangeAlpha | Alice')]:
            self.assertEqual(self.result(answer)['facts_kept'], 'FAIL')

    def test_missing_unknown_fact_fails(self):
        self.assertEqual(self.result(self.TEXT.replace('Unresolved fact', 'Different fact'))['facts_kept'], 'FAIL')

    def test_state_count_losses_additions_and_new_labels_fail(self):
        for answer in [self.TEXT + '\n미확인(실물 확인 전)', self.TEXT + '\n서버 확인',
                       self.TEXT.replace('미확인(실물 확인 전)', 'Other status', 1),
                       self.TEXT + '\n교사 확인(11월 12일)']:
            self.assertEqual(self.result(answer)['facts_kept'], 'FAIL')

    def test_added_line_boundary(self):
        self.assertEqual(self.result(self.TEXT + '\nExtra advice')['facts_kept'], 'pass')
        self.assertEqual(self.result(self.TEXT + '\nExtra advice\nOther advice')['facts_kept'], 'FAIL')

    def test_footer_closing_title_and_global_order_do_not_gate(self):
        answer = self.TEXT.replace(self.FIRST+'\n'+self.SECOND, self.SECOND+'\n'+self.FIRST)
        answer = answer.replace('Example title\n', '').replace('읽은 원천 Source listing\n', '').replace('이번 결과는 읽기만 했고 저장한 것은 없습니다.', '')
        result = self.result(answer)
        self.assertEqual(result['facts_kept'], 'pass')
        self.assertEqual(result['fidelity']['rows_order_kept'], 'no')

    def test_unknown_evidence_fails_closed(self):
        for server, answer, kind in [(None, self.TEXT, 'checklist'), (self.TEXT, None, 'checklist'),
                                     ('', '', 'checklist'), ('Only a title', 'Only a title', 'checklist'),
                                     (self.TEXT, self.TEXT, None)]:
            self.assertEqual(fidelity.evaluate(server, answer, kind)['facts_kept'], 'unknown')

    def test_ask_requires_complete_text_and_added_line_limit(self):
        ask = 'Choose a class\n- ClassAlpha: explanation'
        self.assertEqual(fidelity.evaluate(ask, ask, 'text')['facts_kept'], 'pass')
        self.assertEqual(fidelity.evaluate(ask, 'Choose a class', 'text')['facts_kept'], 'FAIL')
        self.assertEqual(fidelity.evaluate(ask, ask+'\nExtra line\nOther line', 'text')['facts_kept'], 'FAIL')


if __name__ == '__main__':
    unittest.main()
