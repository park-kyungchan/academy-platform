"""M21 regression counterexamples; invented displays only."""
import json
import unittest
from pathlib import Path

import fidelity


def policy():
    spec = json.loads((Path(__file__).parent / 'checks.json').read_text())['Q01']
    return spec.get('fidelity_policy', spec.get('fidelity_kind'))


def wrapper(rows, unknown='Unknown', next_line=''):
    return ('Title\n알림\n학생별 할 일\n' + rows + '\n미확인 사항\n' + unknown
            + '\n다음 확인' + ('\n'+next_line if next_line else ''))


class ReviewCounterexamples(unittest.TestCase):
    def reference(self, rows, unknown="Unknown", next_line=""):
        return wrapper("H1 | H2 | H3 | H4 | H5 | H6\n"+rows,unknown,next_line)

    def rejected(self, source, answer):
        self.assertEqual(fidelity.evaluate(source, source, policy())['facts_kept'], 'pass')
        self.assertEqual(fidelity.evaluate(source, answer, policy())['facts_kept'], 'FAIL')

    def test_b1_cross_rows_duplicate(self):
        first, second = 'A | B | C | D | E | F', 'A | C | B | D | E | F'
        self.rejected(self.reference(first+'\n'+second), self.reference(first+'\n'+first))

    def test_b2_number_and_date_prefix(self):
        for original, changed in [('Unit 1', 'Unit 10'), ('2026-11-1', '2026-11-10')]:
            source = self.reference('A | '+original+' | Task | D | E | F')
            self.rejected(source, source.replace(original, changed))

    def test_b3_student_heading_reassignment(self):
        row = 'A | B | C | D | E | F'
        source = self.reference('Alice 학원 내 처리\n'+row+'\nBob 학원 내 처리')
        answer = self.reference('Alice 학원 내 처리\nBob 학원 내 처리\n'+row)
        self.rejected(source, answer)

    def test_b4_opposite_statement_equal_count_state(self):
        source = self.reference('A | B | C | 교사 확인 | E | F', '제출 필요')
        self.rejected(source, source.replace('제출 필요', '제출 필요 없음'))
        self.rejected(source, source.replace('교사 확인', '교사 확인 아님'))

    def test_b5_invented_rows_share_anchor_or_new(self):
        source = self.reference('A | B | C | D | E | F')
        for row in ['A | invented | invented | invented | invented | invented',
                    'New | invented | invented | invented | invented | invented']:
            self.rejected(source, source+'\n'+row)

    def test_b6_next_action_truncation(self):
        source = self.reference('A | B | C | D | E | F', next_line='- Review the book')
        self.rejected(source, source.split('\n다음 확인')[0])


class StrictCoverage(unittest.TestCase):
    HEADER = 'Owner | Scope | Action | State'
    A = 'Alice | Unit 1 | Review book | 교사 확인'
    B = 'Alice | Unit 2 | Review book | 교사 확인'

    def source(self, rows=None):
        return wrapper(self.HEADER+'\n'+(self.A+'\n'+self.B if rows is None else rows), '제출 필요', '- Check book')

    def result(self, source, answer):
        return fidelity.evaluate(source, answer, policy())

    def pass_pair(self, a, b):
        for source, answer in [(a,b),(b,a)]:
            with self.subTest(direction=(len(source),len(answer))):
                self.assertEqual(self.result(source,answer)['facts_kept'], 'pass')

    def fails(self, source, answer, rule=None):
        self.assertEqual(self.result(source,source)['facts_kept'],'pass')
        result = self.result(source, answer)
        self.assertEqual(result['facts_kept'], 'FAIL')
        if rule is not None:
            self.assertIn(rule,result['failed_rules'])

    def test_dropped_and_duplicated_shared_fields(self):
        source = self.source()
        for answer in [source.replace(self.B,'').replace('\n\n','\n'),source.replace(self.B,self.A)]:
            self.fails(source,answer,1)

    def test_cross_row_matches_and_field_swap(self):
        a='A | B | C | D'; b='A | C | B | D'
        self.fails(self.source(a+'\n'+b),self.source(a+'\n'+a),1)

    def test_heading_reassignment_and_equal_values_wrong_part(self):
        source=wrapper(self.HEADER+'\nAlice 학원 내 처리\n'+self.A+'\nBob 학원 내 처리\n'+self.B,next_line='- Check')
        self.fails(source,source.replace('Alice 학원 내 처리\n'+self.A+'\nBob 학원 내 처리','Alice 학원 내 처리\nBob 학원 내 처리\n'+self.A),2)
        source=self.source(self.A)
        self.fails(source,source.replace(self.A+'\n미확인 사항','미확인 사항\n'+self.A),2)

    def test_equal_count_opposite_state_and_unknown(self):
        source=self.source()
        for old,new in [('교사 확인','교사 확인 아님'),('제출 필요','제출 필요 없음')]:
            self.fails(source,source.replace(old,new),1 if old=='교사 확인' else 3)

    def test_number_and_date_prefixes_whole_values(self):
        for original,changed in [('Unit 1','Unit 10'),('2026-11-1','2026-11-10')]:
            source=self.source(self.A.replace('Unit 1',original))
            self.fails(source,source.replace(original,changed),1)

    def test_invented_rows_and_field_extensions(self):
        source=self.source()
        for answer in [source+'\nAlice | New | New | New',source+'\nZed | New | New | New',
                       source.replace('Unit 1','Unit 1 extra'),source.replace(self.A,self.A+' | Extra')]:
            self.fails(source,answer)

    def test_invented_row_after_optional_footer(self):
        source=self.source()+'\n읽은 원천 Source\n이번 결과는 읽기만 했고 저장한 것은 없습니다.'
        answer=source.replace('\n이번 결과는 읽기만 했고 저장한 것은 없습니다.','\nAlice | New | New | New')
        self.fails(source,answer,4)

    def test_next_action_cut_and_opposite_added_under_required_part(self):
        source=self.source()
        self.fails(source,source.split('\n다음 확인')[0],3)
        self.fails(source,source.replace('제출 필요','제출 필요\n제출 필요 없음'),4)
        self.fails(source,source+'\nAdditional next step',4)

    def test_prose_omission_cannot_borrow_other_rows_field(self):
        source=self.source()
        answer=source.replace(self.A,'Alice Unit 1 교사 확인')
        self.fails(source,answer,1)

    def test_normalize_surrounding_whitespace_both_directions(self):
        source=self.source()
        changed='\n'.join('  '+line.replace(' | ','  |  ')+'  ' for line in source.splitlines())
        self.pass_pair(source,changed)
        self.fails(source,changed.replace('Unit 1','Unit 10'),1)
        self.fails(changed.replace('Unit 1','Unit 10'),source,1)

    def test_normalize_ascii_full_width_bars_both_directions(self):
        source=self.source()
        changed=source.replace('|','｜')
        self.pass_pair(source,changed)
        self.fails(source,changed.replace('Unit 1','Unit 10'),1)
        self.fails(changed.replace('Unit 1','Unit 10'),source,1)

    def test_normalize_line_break_encoding_both_directions(self):
        source=self.source(self.A.replace('Review book','Review / book'))
        changed=source.replace('Review / book','Review\nbook')
        self.pass_pair(source,changed)
        self.fails(source,changed.replace('book','other book'),1)
        self.fails(changed.replace('book','other book'),source,1)

    def test_normalize_outer_fence_both_directions(self):
        source=self.source()
        changed='```text\n'+source+'\n```'
        self.pass_pair(source,changed)
        self.fails(source,changed.replace('Unit 1','Unit 10'),1)
        self.fails(changed.replace('Unit 1','Unit 10'),source,1)

    def test_four_columns_and_two_tables_derived_from_design(self):
        first='Title\nPart One\nThing | Count | Day | Judgment\nBook | 12 | 2026-11-11 | Observed'
        self.pass_pair(first,first)
        second=first+'\nPart Two\nTopic | Grade\nAlgebra | A+'
        self.pass_pair(second,second)
        self.fails(second,second.replace('2026-11-11','2026-11-12'),1)

    def test_tableless_design_compares_complete_lines(self):
        text='Title\n미확인 사항\nNeed book\n다음 확인\n- Check book'
        self.pass_pair(text,text)
        self.fails(text,text.replace('Need book','Need book no longer'),3)
        plain='Title\nOne whole fact\nAnother whole fact'
        self.pass_pair(plain,plain)
        self.assertEqual(self.result(plain,'Title')['facts_kept'],'FAIL')

    def test_unsupported_structure_and_policy_unknown(self):
        source='Title\nPart\nName | Count\nBook | 1 | Extra'
        self.assertEqual(self.result(source,source)['facts_kept'],'unknown')
        bad=dict(policy(),rows='substring')
        self.assertEqual(fidelity.evaluate(self.source(),self.source(),bad)['facts_kept'],'unknown')

    def test_commentary_limit_and_state_words(self):
        source=self.source()
        self.pass_pair(source,source)  # Reference facts cannot be vacuously retained.
        self.assertEqual(self.result(source,'Introductory comment\n'+source)['facts_kept'],'pass')
        self.fails(source,'First comment\nSecond comment\n'+source,5)
        self.fails(source,'서버 확인\n'+source,5)

    def test_footer_closing_optional_and_global_order_with_same_binding(self):
        source=self.source()+'\n읽은 원천 Source\n이번 결과는 읽기만 했고 저장한 것은 없습니다.'
        answer=self.source().replace(self.A+'\n'+self.B,self.B+'\n'+self.A)
        self.assertEqual(self.result(source,answer)['facts_kept'],'pass')

    def test_fence_with_one_outside_comment_and_optional_footer_reflow(self):
        source=self.source()+'\n읽은 원천 Alpha,\nBeta,\nGamma.\n이번 결과는 읽기만 했고 저장한 것은 없습니다.'
        folded=source.replace('Alpha,\nBeta,\nGamma.','Alpha, Beta, Gamma.')
        self.pass_pair(source,folded)
        answer='One comment\n```text\n'+folded+'\n```'
        self.assertEqual(self.result(source,answer)['facts_kept'],'pass')
        self.fails(source,answer.replace('Unit 1','Unit 10'),1)

    def test_required_statement_starting_with_student_is_not_a_heading(self):
        source=self.source().replace('제출 필요','Alice 제출 필요')
        self.assertEqual(self.result(source,source)['facts_kept'],'pass')
        self.fails(source,source.replace('Alice 제출 필요','Alice 제출 필요 없음'),3)
        self.fails(source,source.replace('Alice 제출 필요',''),3)

    def test_identical_duplicate_rows_are_consumed_once(self):
        source=self.source(self.A+'\n'+self.A)
        self.fails(source,self.source(self.A),1)
        self.fails(source,self.source(self.A+'\n'+self.A+'\n'+self.A),4)

    def test_inline_field_markdown_is_not_value_normalization(self):
        source=self.source()
        self.fails(source,source.replace('Unit 1','**Unit 1**'),1)
