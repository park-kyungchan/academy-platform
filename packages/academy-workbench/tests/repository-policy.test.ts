import { test, expect } from 'bun:test';
import { classroomCyclePolicy, requirementsForSeries } from '../../../src/academy/classroomCyclePolicy.ts';

test('repository classroom authority remains separate from synthetic path and grade observations',()=>{
  expect(classroomCyclePolicy.evidenceRules.sourceGradeIsTeacherInspection).toBe(false);
  expect(classroomCyclePolicy.evidenceRules.missingRowIsNonperformance).toBe(false);
  expect(classroomCyclePolicy.seriesRules.gauss).toBe('prestudy_video_is_always_required');
  expect(requirementsForSeries('gauss').find(r=>r.activity==='concept_blank_test')?.completionAuthority).toBe('explicit_teacher_judgment');
  expect(requirementsForSeries('gauss').find(r=>r.activity==='prestudy_review')?.completionAuthority).toBe('explicit_teacher_judgment');
  expect(requirementsForSeries('davinci').some(r=>r.activity==='prestudy_review')).toBe(false);
});
