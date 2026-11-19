import {expect,test} from 'bun:test';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {academyBusinessDate,dailyRecordGetSchema,readDailyRecordByName,type DailyRecordReadRoutePort} from '../src/dailyrecord-read.ts';
import {clock,observation} from './dailyrecord-fixture.ts';

const teacher={sourceNamespace:'academy.lms.dayrecord' as const,teacherId:'99001'};
const request={displayName:'Invented Student',purpose:'Read today’s fictional DailyRecord',currentRequestRef:'synthetic-read-request'};
function fixture(){
  const calls:{names:string[];intakes:any[]}={names:[],intakes:[]};
  const service=new DailyRecordService({
    teacherScope:()=>teacher,
    searchStudentsByName:async name=>{calls.names.push(name);return [{studentKey:'77001',studentName:name}];},
    readDailyRecord:async scope=>{
      const value=observation();value.scope={...scope};value.target={...value.target,...scope};
      value.previousTarget={...value.previousTarget!,sourceNamespace:scope.sourceNamespace,teacherId:scope.teacherId,studentId:scope.studentId};
      value.previousTarget.lessonDate=new Date(Date.parse(scope.lessonDate+'T00:00:00Z')-86400000).toISOString().slice(0,10);
      value.toneSamples=[];return {status:'observed',observation:value};
    },
  },null,()=>clock);
  const port:DailyRecordReadRoutePort={resolveStudent:value=>service.resolveStudent(value),intake:async value=>{calls.intakes.push(value);return service.intake(value);}};
  return {service,port,calls};
}
test('name-only get resolves then calls qualified intake and returns fields despite authoring interview',async()=>{
  const f=fixture(),result=await readDailyRecordByName(request,f.port,()=>clock);
  expect(result).toMatchObject({status:'observed',sourceAvailability:'observed',lessonDate:'2040-01-16',academyWrites:0,parentSends:0,
    route:['academy_student_resolve','academy_dailyrecord_intake'],preparation:{status:'interview_required',requiredOnlyForAuthoring:true}});
  expect(f.calls.names).toEqual([request.displayName]);expect(f.calls.intakes).toHaveLength(1);
  expect(f.calls.intakes[0].context.scope).toEqual({sourceNamespace:teacher.sourceNamespace,teacherId:'99001',studentId:'77001',lessonDate:'2040-01-16'});
});
test('read defaults preserve absence of facts/memo and confer no proposal or review handle',async()=>{
  const f=fixture(),result=await f.service.get(request);
  expect(result.status).toBe('observed');expect(result).not.toHaveProperty('observationHandle');
  const routed=await readDailyRecordByName(request,f.port,()=>clock);
  expect(f.calls.intakes[0]).toMatchObject({context:{scope:{sourceNamespace:teacher.sourceNamespace,teacherId:'99001',studentId:'77001',lessonDate:'2040-01-16'},
    memoAnswer:{kind:'not_supplied'},facts:[],selectedFields:['progress','homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment']}});
  expect(f.calls.intakes[0]).not.toHaveProperty('suppliedValues');
  if(routed.status==='observed')expect(routed.fields.find(f=>f.field==='lessonMemo')?.before).toMatchObject({state:'known',value:'기존 준비물 안내'});
});
test('today is the academy date across UTC rollover, with exact real-date validation',async()=>{
  expect(academyBusinessDate(Date.parse('2040-01-16T14:59:59Z'))).toBe('2040-01-16');
  expect(academyBusinessDate(Date.parse('2040-01-16T15:00:00Z'))).toBe('2040-01-17');
  expect(dailyRecordGetSchema.parse(request).lessonDate).toBe('today');
  // TASK-FLEX-2: the schema takes the date words; resolution refuses a date that does not exist, before any read.
  const f=fixture();
  await expect(readDailyRecordByName({...request,lessonDate:'2040-02-30'},f.port,()=>clock)).rejects.toThrow('date_form_unsupported_use_yyyy_mm_dd_today_yesterday_or_month_day');
  expect(f.calls.names).toEqual([]);
});
test('caller cannot inject keys, teacher, namespace, admission, facts, memo answers or approvals',async()=>{
  for(const extra of [{studentId:'77002'},{teacherId:'99002'},{sourceNamespace:'foreign'},{admission:teacher},{facts:[]},{memoAnswer:{kind:'no_addition'}},{approved:true},{suppliedValues:[]}]){
    const f=fixture();await expect(readDailyRecordByName({...request,...extra},f.port,()=>clock)).rejects.toThrow();
    expect(f.calls.names).toEqual([]);expect(f.calls.intakes).toEqual([]);
  }
});
test('a resolver without the configured teacher stops before intake',async()=>{
  const f=fixture(),original=f.port.resolveStudent;
  f.port.resolveStudent=async input=>{const {teacherId:_teacher,...rest}=await original(input) as Record<string,unknown>;return rest;};
  expect(await readDailyRecordByName(request,f.port,()=>clock)).toMatchObject({status:'unavailable',reasonCode:'dailyrecord_teacher_scope_unavailable'});
  expect(f.calls.intakes).toEqual([]);
});
test('any historical or later date reaches intake without caller-supplied scope',async()=>{
  const f=fixture();
  for(const lessonDate of ['2040-01-01','2040-01-28']){
    const result=await readDailyRecordByName({...request,lessonDate},f.port,()=>clock);
    expect(result).toMatchObject({status:'observed',lessonDate});
  }
  expect(f.calls.intakes.map(x=>x.context.scope.lessonDate)).toEqual(['2040-01-01','2040-01-28']);
});
test('unresolved, ambiguous and foreign names stop before intake without retry',async()=>{
  for(const resolution of [{status:'unavailable',reasonCode:'student_name_ambiguous'},{status:'unavailable',reasonCode:'student_name_not_found'},
    {status:'resolved',studentKey:'77001',displayName:'Other Invented Student'}]){
    let calls=0;const f=fixture();f.port.resolveStudent=async()=>{calls++;return resolution;};
    expect((await readDailyRecordByName(request,f.port,()=>clock)).status).toBe('unavailable');
    expect(calls).toBe(1);expect(f.calls.intakes).toEqual([]);
  }
});
test('unavailable name resolution retains safe upstream failure and occurrence without intake',async()=>{
  const f=fixture(),occurrence='6219a405-b016-4585-a929-2845d063f090';
  f.port.resolveStudent=async()=>({status:'unavailable',reasonCode:'student_resolve_search_unqualified_no_automatic_retry',upstreamOccurrence:occurrence,
    failure:{category:'upstream_unavailable',code:'student_resolve_search_unqualified_no_automatic_retry',evidenceLocator:'untrusted',secret:'do not copy'}});
  const result=await readDailyRecordByName(request,f.port,()=>clock);
  expect(result).toMatchObject({status:'unavailable',upstreamOccurrence:occurrence,failure:{code:'student_resolve_search_unqualified_no_automatic_retry',evidenceLocator:'src/student-resolver.ts:resolveStudentByName'}});
  expect(JSON.stringify(result)).not.toContain('do not copy');expect(f.calls.intakes).toEqual([]);
});
test('failed or malformed upstream intake cannot claim observed source or fall back to a snapshot',async()=>{
  for(const malformed of [null,{sourceAvailability:'observed',source:{},blockers:[],fields:[]}]){
    const f=fixture();let calls=0;f.port.intake=async()=>{calls++;return malformed;};
    expect(await readDailyRecordByName(request,f.port,()=>clock)).toMatchObject({status:'unavailable',reasonCode:'dailyrecord_source_unqualified_no_automatic_retry'});
    expect(calls).toBe(1);
  }
  const f=fixture();f.port.resolveStudent=async()=>{throw Error('private backend detail');};
  expect(await readDailyRecordByName(request,f.port,()=>clock)).toMatchObject({reasonCode:'student_resolve_unqualified_no_automatic_retry'});
  expect(f.calls.intakes).toEqual([]);
});
test('stale and foreign-scope intake cannot be reported as a current observed lookup',async()=>{
  for(const mutation of ['stale','scope'] as const){
    const f=fixture(),original=f.port.intake;
    f.port.intake=async input=>{const result:any=await original(input);if(mutation==='stale')result.source.observedAt='2040-01-15T00:00:00Z';else result.userContext.scope.teacherId='99002';return result;};
    expect((await readDailyRecordByName(request,f.port,()=>clock)).status).toBe('unavailable');
  }
});
