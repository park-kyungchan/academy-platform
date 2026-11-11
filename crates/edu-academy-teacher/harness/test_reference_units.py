"""R2 review regressions, invented reference/answer pairs only."""
import unittest
import fidelity


def display():
    return 'Title\n학생별 할 일\nName | Action\nAlice | Review book\n미확인 사항\nNeed book\n다음 확인\nCheck book'


class ReferenceUnitRegressions(unittest.TestCase):
    def check(self, reference, answer, expected):
        self.assertEqual(fidelity.evaluate(reference, answer, fidelity.configured_policy())['facts_kept'],expected)

    def test_r1_formatted_extra_lines_cannot_escape(self):
        source=display()
        self.check(source,source,'pass')
        self.check(source,'# 서버 확인\n# Extra commentary\n'+source,'FAIL')

    def test_r1_invented_student_headings_are_added(self):
        source=display()
        self.check(source,source,'pass')
        self.check(source,'Invented 학원 내 처리\nOther 학원 내 처리\n'+source,'FAIL')

    def test_r1_single_comment_before_header_is_allowed(self):
        source=display()
        self.check(source,source,'pass')
        self.check(source,source.replace('Name | Action','One comment\nName | Action'),'pass')

    def test_r2_tableless_statement_cannot_disappear(self):
        source='Title\nRead chapter 2\n미확인 사항\nNeed book\n다음 확인\nCheck book'
        self.check(source,source,'pass')
        self.check(source,source.replace('Read chapter 2\n',''),'FAIL')

    def test_r2_empty_required_parts_are_supported(self):
        source='Title\n미확인 사항\n다음 확인'
        self.check(source,source,'pass')

    def test_r3_last_cell_break_forward_and_reverse(self):
        source='Title\nPart\nName | Action\nAlice | Review / book'
        wrapped=source.replace('Review / book','Review\nbook')
        self.check(source,source,'pass')
        self.check(source,wrapped,'pass')
        self.check(wrapped,source,'pass')

    def test_one_stateless_comment_at_every_position_has_no_context_effect(self):
        source=display();lines=source.splitlines()
        for position in range(len(lines)+1):
            for comment in ['Plain comment','# New heading','Invented 학원 내 처리','**Formatted comment**']:
                self.check(source,'\n'.join(lines[:position]+[comment]+lines[position:]),'pass')

    def test_answer_formats_never_produce_unknown(self):
        source=display()
        for answer in ['', '```', source+'\n| altered | row |', '# 서버 확인\n'+source]:
            self.check(source,answer,'FAIL')

    def test_last_plain_cell_break_before_another_row_both_directions(self):
        source='Title\nPart\nName | Action\nAlice | Review / book\nBob | Check'
        wrapped=source.replace('Review / book','Review\nbook')
        self.check(source,source,'pass')
        self.check(source,wrapped,'pass')
        self.check(wrapped,source,'pass')

    def test_plain_part_between_same_width_headers_uses_reference_header(self):
        source='Title\nPart A\nName | Action\nAlice | Review\nPart B\nName | State\nBob | Pending'
        self.check(source,source,'pass')
        self.check(source,source.replace('Part B\nName | State\nBob | Pending','Bob | Pending\nPart B\nName | State'),'FAIL')
