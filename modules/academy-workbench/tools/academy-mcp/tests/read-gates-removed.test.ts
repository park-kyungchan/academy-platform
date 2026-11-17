import {expect,test} from 'bun:test';
import {mkdtempSync,readdirSync,readFileSync,rmSync,statSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {createDailyRecordWorkflow} from '../src/dailyrecord-workflow.ts';
import {learningPolicy,safeguard} from './dailyrecord-fixture.ts';
import {proposals,userRequest,workflowContext} from './dailyrecord-workflow-fixture.ts';

// TASK-GATES-1. Fictional data only; no request leaves the process.
// The launch lists one student on one date. The student read below is in no
// list, and its date lies outside every old window.
const teacherId='99001',listedDate='2040-01-04',outsideDate='2041-06-30';
const launch={teacherId,teacherLabel:'Invented Teacher',date:listedDate,students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
const unlisted={studentId:'77009',displayName:'Unlisted Student'};
const credential=()=>({username:'fictional',password:'fictional'});
// The gates of origin/main, passed where an installed endpoint passes them. They must gate nothing.
const legacyOwner={schemaVersion:'academy.owner-admission/2',decider:'user',decidedAt:listedDate,sourceNamespace:'academy.lms.dayrecord',teacherId,
  allowedFrom:'2040-01-03',allowedThrough:'2040-01-05',failClosedDefault:'2026-10-23',studentScope:'source_verified'};
const legacyMetadata={schemaVersion:'academy.metadata-admission/1',sourceNamespace:'academy.lms.metadata',teacherId,allowedFrom:'2040-01-03',allowedThrough:'2040-01-05',
  studentScope:'source_verified',relativeSelectors:[],operations:['daily_inventory']};
const legacyNames=new Set(['Invented Student']);
const request={purpose:'Synthetic gate removal read',currentRequestRef:'synthetic-gates-request'};

function dayRow(name:string,key:string,date:string,teacherKey=teacherId){
  return {studentName:name,date,groupId:'3',teacherKey,studentKey:key,courseKey:'5',recordKey:date.replaceAll('-',''),membershipKey:'8',
    fields:{progress:'- Fictional progress',homework:'- Fictional homework',lessonMemo:'',attendance:'Y'}};
}
const previousDate=(date:string)=>new Date(Date.parse(date+'T00:00:00Z')-86400000).toISOString().slice(0,10);
const metadataPage=(key:string,name:string,selector:string)=>({rows:[{studentKey:key,studentName:name,paperKey:'81001',attemptKey:'91001',sourceRowId:null,sourceScore:80,titles:{title:'Fictional DailyTest'}}],
  operation:'DtResultSearch',page:1,sourcePeriod:null,observedAt:new Date().toISOString(),coverage:'one_observed_student_search_page_not_complete_history',
  generatedMembership:'unqualified',fixedSourceFilter:{sort_date:selector},expectedPaperDefinitions:'not_observed'});

/** A fake owner client. `rowTeacher` lets a row carry a foreign teacher key. */
function sourcePort(options:{rowTeacher?:string}={}){
  const events:string[]=[];
  const keyOf=(name:string)=>name===unlisted.displayName?unlisted.studentId:name===launch.students[0]!.displayName?launch.students[0]!.studentId:null;
  const state:Record<string,string>={progress:'- Fictional progress',homework:'- Fictional homework',lessonMemo:'',attendance:'Y',dailyTestAssessment:'0',previousHomeworkAssessment:'3'};
  const port={
    authenticate:async()=>{events.push('auth');},
    discoverDayRecordGroups:async(input:any)=>{events.push('groups:'+input.date);return {groups:[{groupId:'3'}]};},
    searchStudentsByName:async(input:any)=>{events.push('search:'+input.studentName);const key=keyOf(input.studentName);return key?[{studentKey:key,studentName:input.studentName}]:[];},
    execute:async(input:any)=>{
      if(input.family==='day_record'){
        events.push('day:'+input.date);const name=input.studentNames[0],key=keyOf(name);
        return {dayRecords:key?[{...dayRow(name,key,input.date,options.rowTeacher),fields:{progress:state.progress,homework:state.homework,lessonMemo:state.lessonMemo,attendance:state.attendance}}]:[]};
      }
      if(input.family==='textbook'){
        events.push('textbook:'+input.studentKey);
        return {family:'textbook',status:'observed',observedAt:new Date().toISOString(),coverage:'fictional',
          records:input.sourceTitles.map((sourceTitle:string)=>({sourceTitle,studentKey:input.studentKey,studentName:input.studentName,courseKey:input.courseKey,bookNo:input.bookNo,score:80,
            items:[{originalOrdinal:'1',lectureKey:'8101',outerLectureKey:null,scoring:'O'}],sourceDate:{literal:null,precision:'unknown',meaning:'source_apply_date_not_exact_grade_time'}}))};
      }
      throw Error('unexpected_family');
    },
    journalState:async(target:any)=>({target,fields:{...state},previousRecordKey:previousDate(target.date).replaceAll('-',''),previousDate:previousDate(target.date),wireDigest:'a'.repeat(64)}),
    applyJournalAction:async(action:any,approval:any)=>{
      events.push('write:'+action.field);
      if((await approval.consumeApproval(action)).status!=='verified')throw Error('journal_http_approval_rejected');
      state[action.field]=action.after;return {status:'persisted' as const,transport:'returned' as const};
    },
    readPrestudy:async(input:any)=>{events.push('prestudy:'+input.studentName);return {waiting:[],completed:[{unitLabel:'Fictional unit',dateLiteral:'06-30',statusLiteral:'done',prestudyKey:'4401'}],summary:[],observedAt:new Date().toISOString(),coverage:'fictional'};},
    readTextbookBindings:async(input:any)=>{events.push('bindings:'+input.studentKey);return [{courseKey:'5',bookNo:'7',studentKey:input.studentKey,bookLabel:'Book One'}];},
    readDailyTestHistory:async(input:any)=>{events.push('history:'+input.studentKey);
      return {status:'observed',studentKey:input.studentKey,studentName:input.studentName,currentFilterQualification:{observedAt:new Date().toISOString(),selector:'1',rows:metadataPage(input.studentKey,input.studentName,'1').rows},
        observations:input.periods.map((selector:string)=>({...metadataPage(input.studentKey,input.studentName,selector),selector})),
        calendarBounds:'source_relative_labels_only_not_exact_calendar_bounds',expectedPaperDefinitions:'not_observed',coverage:'all_parsed_embedded_rows_on_each_observed_response_not_complete_history'};},
    close:()=>{},
  };
  return {port,events,state};
}
function gatedBackend(port:unknown){
  return new NativeDailyRecordBackend(port as never,launch,credential,legacyOwner as never,legacyNames as never,legacyMetadata as never);
}

test('check 1: a student in no list, on a date outside every old window, is resolved and read by every read family',async()=>{
  const read=async(fn:(service:DailyRecordService)=>Promise<unknown>)=>{const f=sourcePort();return fn(new DailyRecordService(gatedBackend(f.port)));};
  expect(await read(s=>s.resolveStudent({...request,displayName:unlisted.displayName})))
    .toMatchObject({status:'resolved',studentKey:unlisted.studentId,displayName:unlisted.displayName});
  expect(await read(s=>s.resolveStudent({...request,displayName:unlisted.displayName,lessonDate:outsideDate})))
    .toMatchObject({status:'resolved',studentKey:unlisted.studentId});
  expect(await read(s=>s.get({...request,displayName:unlisted.displayName,lessonDate:outsideDate})))
    .toMatchObject({status:'observed',sourceAvailability:'observed',scope:{teacherId,studentId:unlisted.studentId,lessonDate:outsideDate}});
  expect(await read(s=>s.prestudyGet({...request,displayName:unlisted.displayName})))
    .toMatchObject({status:'observed',student:{studentId:unlisted.studentId,displayName:unlisted.displayName}});
  expect(await read(s=>s.textbookSectionsByName({...request,displayName:unlisted.displayName,bookLabel:'Book One',sourceTitles:['Fictional section']})))
    .toMatchObject({status:'observed',student:{studentId:unlisted.studentId},records:[{sourceTitle:'Fictional section',score:80}]});
  expect(await read(s=>s.metadataByName({...request,displayName:unlisted.displayName,lessonDate:outsideDate,queries:[{kind:'daily_history',periods:['11']}]})))
    .toMatchObject({status:'observed',scope:{sourceNamespace:'academy.lms.metadata',teacherId,studentId:unlisted.studentId,lessonDate:outsideDate},
      results:[{query:{kind:'daily_history',periods:['11']},observation:{status:'observed',studentKey:unlisted.studentId}}]});
});

test('check 2: a row whose teacher key differs from the configured teacher is still refused',async()=>{
  // The listed student on the listed date, so that origin/main reaches the same teacher check.
  const scope={sourceNamespace:'academy.lms.dayrecord',teacherId,studentId:'77001',lessonDate:listedDate};
  const plain=()=>{const f=sourcePort({rowTeacher:'99002'});return {...f,backend:new NativeDailyRecordBackend(f.port as never,launch,credential)};};
  const direct=plain();
  await expect(direct.backend.readDailyRecord(scope,'foreign-teacher')).rejects.toThrow(/native_dailyrecord_identity_conflict|native_dated_name_scope_conflict/);
  const routed=plain();
  const got=await new DailyRecordService(routed.backend).get({...request,displayName:'Invented Student',lessonDate:listedDate});
  expect(got).toMatchObject({status:'unavailable',sourceAvailability:'unavailable'});
  expect(routed.events.some(e=>e.startsWith('write:'))).toBe(false);
  const saving=plain(),approval={consumeApproval:async()=>({status:'verified' as const,actionDigest:'c'.repeat(64)})};
  await expect(saving.backend.saveDailyRecord(scope,[{field:'progress',value:'- New',before:'- Fictional progress',wireDigest:'a'.repeat(64)}],approval,{displayName:'Invented Student'}))
    .rejects.toThrow('native_dailyrecord_identity_conflict');
  expect(saving.events.some(e=>e.startsWith('write:'))).toBe(false);
});

test('check 4: a save for a student outside the old list succeeds with its approval and its readback',async()=>{
  const base=mkdtempSync(resolve(tmpdir(),'gates-1-save-'));
  try{
    const f=sourcePort(),store=resolve(base,'store');
    const service=()=>{const backend=new NativeDailyRecordBackend(f.port as never,launch,credential);
      return new DailyRecordService(backend,safeguard,Date.now,learningPolicy,backend,createDailyRecordWorkflow({storeDir:store}));};
    const scope={sourceNamespace:'academy.lms.dayrecord',teacherId,studentId:unlisted.studentId,lessonDate:outsideDate};
    const currentRequestRef='synthetic-gates-save';
    const got:any=await service().get({displayName:unlisted.displayName,lessonDate:outsideDate,purpose:'Synthetic save',currentRequestRef});
    expect(got).toMatchObject({status:'observed',authoringHandle:{persistence:'owner_durable',boundTo:{scope,currentRequestRef}}});
    const value='- 지정 문제 풀이 진행.';
    const context={...workflowContext({progress:value},currentRequestRef),scope};
    const review:any=await service().review({context,observationHandle:got.authoringHandle.observationHandle,proposals:proposals({progress:value},currentRequestRef)});
    expect(review.status).toBe('prepared');
    const saved:any=await service().save({context,observationHandle:got.authoringHandle.observationHandle,reviewDigest:review.reviewDigest,
      proposals:proposals({progress:value},currentRequestRef),request:userRequest(currentRequestRef)});
    expect(saved).toMatchObject({status:'saved',academyWrites:1,fields:[{field:'progress',status:'persisted'}]});
    expect(f.events.filter(e=>e.startsWith('write:'))).toEqual(['write:progress']);
    // Readback through the same read route.
    const after:any=await service().get({displayName:unlisted.displayName,lessonDate:outsideDate,purpose:'Synthetic readback',currentRequestRef:'synthetic-gates-readback'});
    expect(after.fields.find((row:any)=>row.field==='progress').before).toMatchObject({state:'known',value});
  }finally{rmSync(base,{recursive:true,force:true});}
},20000);

test('check 5: the module source holds none of the removed gate codes and symbols',()=>{
  const removed=['student_name_not_admitted','textbook_student_not_admitted','prestudy_student_not_admitted','metadata_namespace_not_admitted',
    'native_dailyrecord_managed_student_required','lesson_date_out_of_admitted_scope','dailyrecord_owner_read_admission_not_configured','read_scope_not_admitted',
    'managedNames','expectedStudentNames','isDateAdmitted','FAIL_CLOSED_DEFAULT','metadataDateAdmitted'];
  const root=resolve(import.meta.dir,'../src'),found:string[]=[];
  const walk=(dir:string)=>{for(const name of readdirSync(dir)){const path=join(dir,name);
    if(statSync(path).isDirectory()){walk(path);continue;}
    const text=readFileSync(path,'utf8');for(const symbol of removed)if(text.includes(symbol))found.push(name+':'+symbol);}};
  walk(root);
  expect(found).toEqual([]);
});
