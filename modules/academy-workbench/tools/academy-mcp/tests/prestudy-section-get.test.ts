import {expect,test} from 'bun:test';
import {DailyRecordService,type DailyRecordReadBackend,type PrestudyObservation,type PrestudyReportObservation,type TextbookSectionObservation,type TextbookSectionIndexObservation} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';

const prestudyRequest={displayName:'Invented Student',purpose:'Read fictional prestudy state',currentRequestRef:'synthetic-prestudy-request'};
const prestudy:PrestudyObservation={student:{studentId:'77001',displayName:'Invented Student'},
  waiting:[{unitLabel:'9-1 Fictional Unit',dateLiteral:'40.01.02 03:04',statusLiteral:'OK',prestudyKey:'12345'}],
  completed:[],summary:[{unitLabel:'9-1 Fictional Unit',dateLiteral:'40.01.02 03:04',aiScoreLiteral:'77점 보고서',ratingLiteral:'-',reportAvailable:true,videoAvailable:true}],
  teacherJudgment:null,observedAt:'2040-01-16T09:00:00.000Z',coverage:'fictional_coverage'};

test('prestudy get projects waiting/completed/summary with teacher judgment separate',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readPrestudyByName:async input=>{expect(input.displayName).toBe('Invented Student');return prestudy;}};
  const result=await new DailyRecordService(backend).prestudyGet(prestudyRequest);
  expect(result).toMatchObject({status:'observed',academyWrites:0,parentSends:0,teacherJudgment:null,waiting:[{prestudyKey:'12345'}]});
  expect(await new DailyRecordService().prestudyGet(prestudyRequest))
    .toMatchObject({status:'unavailable',reasonCode:'prestudy_backend_not_configured',academyWrites:0});
});

const reportObservation:PrestudyReportObservation={student:{studentId:'77001',displayName:'Invented Student'},prestudyKey:'12345',
  sections:[{title:'개념 이해',subtitle:'개념을 정확히 이해하고 설명하는가?',blocks:[{heading:'포함된 핵심 개념',text:'Fictional narrative'}]}],
  scores:[77,80],encodingResult:'success',observedAt:'2040-01-16T09:00:00.000Z',coverage:'fictional_coverage'};

test('prestudy report get returns teacher-only sections for the exact record and nothing without a backend',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readPrestudyReportByName:async input=>{expect(input).toMatchObject({displayName:'Invented Student',prestudyKey:'12345'});return reportObservation;}};
  const result=await new DailyRecordService(backend).prestudyReportGet({...prestudyRequest,prestudyKey:'12345'});
  expect(result).toMatchObject({status:'observed',academyWrites:0,parentSends:0,studentContent:'teacher_only_do_not_store_or_relay',
    sections:[{title:'개념 이해'}],scores:[77,80]});
  expect(await new DailyRecordService().prestudyReportGet({...prestudyRequest,prestudyKey:'12345'}))
    .toMatchObject({status:'unavailable',reasonCode:'prestudy_report_backend_not_configured',academyWrites:0});
});

test('section get by name without titles returns the owner-discovered index; with titles returns grading',async()=>{
  const indexObs:TextbookSectionIndexObservation={student:{studentId:'77001',displayName:'Invented Student'},courseKey:'5',bookNo:'7',bookLabel:'Fictional Book',
    index:[{sourceTitle:'Unit A 필수예제',sourceDate:{literal:'10-23',meaning:'source_apply_date_not_exact_grade_time'}}],observedAt:'2040-01-16T09:00:00.000Z',coverage:'index'};
  const gradeObs:TextbookSectionObservation={student:{studentId:'77001',displayName:'Invented Student'},courseKey:'5',bookNo:'7',
    records:[{sourceTitle:'Unit A 필수예제',score:80,correctCount:4,itemCount:5,items:[{ordinal:'5',lectureKey:null,outerLectureKey:null,scoring:'X'}],sourceDate:{literal:'10-23',precision:'month_day',meaning:'source_apply_date_not_exact_grade_time'}}],
    observedAt:'2040-01-16T09:00:00.000Z',coverage:'exact'};
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readTextbookSectionsByName:async input=>input.sourceTitles?gradeObs:indexObs};
  const service=new DailyRecordService(backend);
  expect(await service.textbookSectionsByName(prestudyRequest)).toMatchObject({status:'observed',index:[{sourceTitle:'Unit A 필수예제'}]});
  expect(await service.textbookSectionsByName({...prestudyRequest,sourceTitles:['Unit A 필수예제']})).toMatchObject({status:'observed',records:[{score:80}]});
  expect(await new DailyRecordService().textbookSectionsByName(prestudyRequest))
    .toMatchObject({status:'unavailable',reasonCode:'textbook_section_backend_not_configured'});
});

test('native backend resolves exactly one binding and never accepts caller keys',async()=>{
  const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',
    students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
  const target={studentName:'Invented Student',date:'2040-01-04',groupId:'3',teacherKey:'99001',studentKey:'77001',courseKey:'5',recordKey:'13',membershipKey:'8'};
  const port={authenticate:async()=>{},discoverDayRecordGroups:async()=>({groups:[{groupId:'3'}]}),
    readTextbookBindings:async()=>[{courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'Fictional Book'}],
    inspectTextbookIndex:async()=>({rows:[{sourceTitle:'Unit A 필수예제',sourceDate:{literal:'10-23',meaning:'source_apply_date_not_exact_grade_time'}}]}),
    readPrestudy:async()=>({waiting:[{studentLabel:'Invented Student',unitLabel:'9-1 Fictional Unit',dateLiteral:'40.01.02 03:04',statusLiteral:'OK',prestudyKey:'12345'}],
      completed:[],summary:[],observedAt:'2040-01-16T09:00:00.000Z',coverage:'fictional_coverage'}),
    execute:async()=>({family:'textbook',status:'observed',observedAt:'2040-01-16T09:00:00.000Z',coverage:'x',records:[]}),
    journalState:async()=>({target,fields:{},previousRecordKey:'12',previousDate:'2040-01-02',wireDigest:'a'.repeat(64)}),close:()=>{}};
  const backend=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
  expect(await backend.readTextbookSectionsByName(prestudyRequest)).toMatchObject({courseKey:'5',bookNo:'7',index:[{sourceTitle:'Unit A 필수예제'}]});
  expect(await backend.readPrestudyByName(prestudyRequest)).toMatchObject({teacherJudgment:null,waiting:[{prestudyKey:'12345'}]});
});
