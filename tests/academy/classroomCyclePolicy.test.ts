import {describe,expect,test} from 'bun:test';
import {classroomCyclePolicy,requirementsForSeries} from '../../src/academy/classroomCyclePolicy';

describe('teacher-admitted classroom cycle requirements',()=>{
  test('every Gauss minor unit requires prestudy review and all separately owned cycle checks',()=>{
    const requirements=requirementsForSeries('gauss');
    expect(requirements.map(x=>x.activity)).toEqual(['prestudy_review','concept_note_inspection','solution_note_inspection','app_problem_grading','concept_blank_test','wrong_clinic','similar_clinic','daily_test','havruta']);
    expect(requirements.find(x=>x.activity==='app_problem_grading')?.completionAuthority).toBe('verified_app_grading_for_exact_assigned_range');
    expect(requirements.filter(x=>x.activity!=='app_problem_grading').every(x=>x.completionAuthority==='explicit_teacher_judgment')).toBe(true);
    expect(classroomCyclePolicy.auditRange).toBe('every_progressed_minor_unit_through_latest_assigned_homework');
  });
  test('Davinci requires only wrong and similar clinic inspection, not accidental prestudy uploads',()=>{
    expect(requirementsForSeries('davinci').map(x=>x.activity)).toEqual(['wrong_clinic','similar_clinic']);
    expect(requirementsForSeries('davinci').every(x=>x.completionAuthority==='explicit_teacher_judgment')).toBe(true);
  });
  test('callers cannot change the shared requirements through a returned array',()=>{
    const first=requirementsForSeries('gauss');first[0]!.activity='daily_test';first.pop();
    expect(requirementsForSeries('gauss')[0]!.activity).toBe('prestudy_review');
    expect(requirementsForSeries('gauss')).toHaveLength(9);
    expect(()=>requirementsForSeries('unknown' as never)).toThrow('unsupported_classroom_series');
  });
  test('evidence and refresh guidance never installs a loop or substitutes source grades for teacher review',()=>{
    expect(classroomCyclePolicy.evidenceRules.sourceGradeIsTeacherInspection).toBe(false);
    expect(classroomCyclePolicy.evidenceRules.missingRowIsNonperformance).toBe(false);
    expect(classroomCyclePolicy.refresh.activeTeacherScreen).toBe('automatic_source_change_reflection_plus_manual_refresh');
    expect(classroomCyclePolicy.refresh.runtimeInstalledByPolicy).toBe(false);
    expect(classroomCyclePolicy.refresh.classes.map(x=>x.kind)).toEqual(['shared_content_version','student_book_assignment','lesson_and_homework','submission_and_grading','teacher_judgment_revision']);
    expect(classroomCyclePolicy.havrutaTiming).toEqual({usualMinutesBeforeLessonEnd:30,meaning:'teacher_reported_usual_timing_not_a_hard_deadline_or_scheduler'});
  });
});
