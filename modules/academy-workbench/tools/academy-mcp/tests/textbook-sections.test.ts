import {expect,test} from 'bun:test';
import {DailyRecordService,type DailyRecordReadBackend,type TextbookSectionObservation} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';

const request={displayName:'Invented Student',purpose:'Read fictional section grading',currentRequestRef:'synthetic-textbook-request',
  courseKey:'5',bookNo:'7',sourceTitles:['Unit A 필수예제','Unit A 유형다지기']};
const observation:TextbookSectionObservation={student:{studentId:'77001',displayName:'Invented Student'},courseKey:'5',bookNo:'7',
  records:[
    {sourceTitle:'Unit A 필수예제',score:88,correctCount:7,itemCount:8,items:[{ordinal:'1',lectureKey:'8101',outerLectureKey:'9101',scoring:'O'},{ordinal:'2',lectureKey:null,outerLectureKey:'9102',scoring:'X'}],sourceDate:{literal:'10-23',precision:'month_day',meaning:'source_apply_date_not_exact_grade_time'}},
    {sourceTitle:'Unit A 유형다지기',score:89,correctCount:8,itemCount:9,items:[{ordinal:'1',lectureKey:'8201',outerLectureKey:null,scoring:'O'}],sourceDate:{literal:null,precision:'unknown',meaning:'source_apply_date_not_exact_grade_time'}},
  ],observedAt:'2040-01-16T09:00:00.000Z',coverage:'exact_selected_existing_records_not_whole_book'};

test('textbook section results project exact titles, scores and item outcomes',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSections:async input=>{expect(input.sourceTitles).toEqual(request.sourceTitles);return observation;}};
  const result=await new DailyRecordService(backend).textbookSections(request);
  expect(result).toMatchObject({status:'observed',academyWrites:0,parentSends:0,
    records:[{sourceTitle:'Unit A 필수예제',correctCount:7,itemCount:8},{sourceTitle:'Unit A 유형다지기',correctCount:8,itemCount:9}]});
});

test('textbook section results fail closed without a backend or on scope drift',async()=>{
  expect(await new DailyRecordService().textbookSections(request))
    .toMatchObject({status:'unavailable',reasonCode:'textbook_section_backend_not_configured',academyWrites:0});
  const drifting:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSections:async()=>({...observation,courseKey:'9'})};
  expect(await new DailyRecordService(drifting).textbookSections(request)).toMatchObject({status:'unavailable',academyWrites:0});
});

test('a title absent from both observed sources is coverage-qualified, never a no-record claim',async()=>{
  const missing:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSections:async()=>{throw Error('textbook_section_not_in_either_observed_source');},
    readTextbookSectionsByName:async()=>{throw Error('textbook_section_not_in_either_observed_source');}};
  const service=new DailyRecordService(missing);
  const byKey=await service.textbookSections(request);
  expect(byKey).toMatchObject({status:'not_observed_in_either_source',reasonCode:'textbook_section_not_in_either_observed_source',
    coverage:'wrong_answer_note_index_plus_one_observed_student_paper_search_page_not_complete_history',
    meaning:'requested_title_absent_from_both_observed_sources_only_not_proof_of_no_record',academyWrites:0});
  const byName=await service.textbookSectionsByName({displayName:'Invented Student',purpose:'x',currentRequestRef:'y',sourceTitles:['Missing title']});
  expect(byName).toMatchObject({status:'not_observed_in_either_source',reasonCode:'textbook_section_not_in_either_observed_source'});
  // Other failures keep the generic unavailable contract.
  const broken:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSections:async()=>{throw Error('textbook_section_result_unqualified');}};
  expect(await new DailyRecordService(broken).textbookSections(request)).toMatchObject({status:'unavailable'});
});

test('native index passes through the merged paper-search basis and all-correct titles',async()=>{
  const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',
    students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
  const perfect='[필수예제] [가우스] 5-4 대분수의 뺄셈 - Invented Student';
  const port={authenticate:async()=>{},close:()=>{},
    readTextbookBindings:async()=>[{courseKey:'693',bookNo:'2517',studentKey:'77001',bookLabel:'Gauss fraction book'}],
    inspectTextbookIndex:async()=>({indexBasis:'wrong_answer_note_plus_student_paper_search',rows:[
      {sourceTitle:'[필수예제] [가우스] 5-3 진분수의 뺄셈 - Invented Student',sourceDate:{literal:'11-03',meaning:'source_apply_date_not_exact_grade_time'},sources:['wrong_answer_note','student_paper_search']},
      {sourceTitle:perfect,sourceDate:{literal:'2026-11-03 10:05:00',meaning:'source_apply_date_not_exact_grade_time'},sources:['student_paper_search']},
    ]})};
  const backend=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
  const result=await backend.readTextbookSectionsByName({displayName:'Invented Student',purpose:'x',currentRequestRef:'y',bookLabel:'Gauss fraction book'});
  if(!('index' in result))throw Error('index_expected');
  expect(result.indexBasis).toBe('wrong_answer_note_plus_student_paper_search');
  expect(result.coverage).toContain('not_complete_history');
  expect(result.index.map(r=>r.sourceTitle)).toContain(perfect);
  expect(result.index[1]).toMatchObject({sources:['student_paper_search']});
});

test('native backend binds textbook execute output to the admitted student and request',async()=>{
  const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',
    students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
  const target={studentName:'Invented Student',date:'2040-01-04',groupId:'3',teacherKey:'99001',studentKey:'77001',courseKey:'5',recordKey:'13',membershipKey:'8'};
  const port={authenticate:async()=>{},discoverDayRecordGroups:async()=>({groups:[{groupId:'3'}]}),
    execute:async(input:any)=>{expect(input).toMatchObject({family:'textbook',studentKey:'77001',courseKey:'5',bookNo:'7',gradeViewAdmission:true});
      return {family:'textbook',status:'observed',observedAt:'2040-01-16T09:00:00.000Z',coverage:'x',records:request.sourceTitles.map((t,i)=>({
        sourceTitle:t,studentKey:'77001',studentName:'Invented Student',courseKey:'5',bookNo:'7',score:80+i,
        items:[{originalOrdinal:'1',lectureKey:'8101',outerLectureKey:'9101',scoring:'O'},{originalOrdinal:'2',lectureKey:null,outerLectureKey:'9102',scoring:'X'}],sourceDate:{literal:null,precision:'unknown',meaning:'source_apply_date_not_exact_grade_time'}}))};},
    journalState:async()=>({target,fields:{},previousRecordKey:'12',previousDate:'2040-01-02',wireDigest:'a'.repeat(64)}),close:()=>{}};
  const backend=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
  const result=await backend.readTextbookSections(request);
  expect(result.records).toHaveLength(2);expect(result.records[0]).toMatchObject({correctCount:1,itemCount:2,items:[{ordinal:'1',lectureKey:'8101'},{ordinal:'2',lectureKey:null,outerLectureKey:'9102'}]});
  await expect(backend.readTextbookSections({...request,displayName:'Foreign Student'})).rejects.toThrow('textbook_student_not_admitted');
});
