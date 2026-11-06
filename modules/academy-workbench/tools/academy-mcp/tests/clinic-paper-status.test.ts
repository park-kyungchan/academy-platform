import {expect,test} from 'bun:test';
import {DailyRecordService,type DailyRecordReadBackend,type TextbookSectionObservation,type TextbookSectionIndexObservation} from '../src/dailyrecord.ts';

const request={displayName:'Invented Student',purpose:'Read fictional clinic bundle',currentRequestRef:'synthetic-clinic-request'};
const indexObs:TextbookSectionIndexObservation={student:{studentId:'77001',displayName:'Invented Student'},courseKey:'5',bookNo:'7',bookLabel:'Fictional Book',
  index:[{sourceTitle:'Unit A 필수예제',sourceDate:{literal:'10-23',meaning:'source_apply_date_not_exact_grade_time'}}],observedAt:'2040-01-16T09:00:00.000Z',coverage:'index',indexBasis:'wrong_answer_note_plus_student_paper_search'};
const gradeObs:TextbookSectionObservation={student:{studentId:'77001',displayName:'Invented Student'},courseKey:'5',bookNo:'7',
  records:[{sourceTitle:'Unit A 필수예제',score:80,correctCount:4,itemCount:5,
    items:[{ordinal:'1',lectureKey:'101',outerLectureKey:null,scoring:'O'},{ordinal:'5',lectureKey:'105',outerLectureKey:null,scoring:'X'}],
    sourceDate:{literal:'10-23',precision:'month_day',meaning:'source_apply_date_not_exact_grade_time'}}],
  observedAt:'2040-01-16T09:00:00.000Z',coverage:'exact'};

test('clinic bundle without titles returns index rows with teacher-judgment slots and unqualified generation',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSectionsByName:async()=>indexObs};
  const result=await new DailyRecordService(backend).clinicPaperStatus(request) as Record<string,any>;
  expect(result).toMatchObject({status:'observed',academyWrites:0,generatedMembership:'unqualified',clinicGeneration:'unqualified',
    sections:[{sourceTitle:'Unit A 필수예제',originalAttempt:'paper_row_observed',clinicEligible:null}],
    teacherJudgmentSlots:{solved:'teacher_judged_only_no_server_score_input',scored:'teacher_judged_only_no_server_score_input'}});
});

test('clinic bundle with titles marks eligibility from wrong items only',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSectionsByName:async input=>input.sourceTitles?gradeObs:indexObs};
  const result=await new DailyRecordService(backend).clinicPaperStatus({...request,sourceTitles:['Unit A 필수예제']}) as Record<string,any>;
  expect(result.sections[0]).toMatchObject({originalAttempt:'observed_original_attempt',clinicEligible:true,
    wrongItems:[{ordinal:'5',lectureKey:'105'}]});
});

test('clinic bundle propagates binding interview and fails closed without backend',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSectionsByName:async()=>({kind:'interview_required' as const,student:{studentId:'77001',displayName:'Invented Student'},
      bindings:[{bookLabel:'A',bookNo:'1'},{bookLabel:'B',bookNo:'2'}],reason:'multiple_book_bindings_select_one' as const,observedAt:'2040-01-16T09:00:00.000Z'})};
  expect(await new DailyRecordService(backend).clinicPaperStatus(request)).toMatchObject({status:'interview_required',bindings:[{bookLabel:'A',bookNo:'1'},{bookLabel:'B',bookNo:'2'}]});
  expect(await new DailyRecordService().clinicPaperStatus(request))
    .toMatchObject({status:'unavailable',reasonCode:'clinic_paper_backend_not_configured',academyWrites:0});
});
