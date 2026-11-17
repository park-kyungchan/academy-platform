import {expect,test} from 'bun:test';
import {NativeDailyRecordBackend,nativeDailyRecordConfigSchema,nativeDailyRecordConfiguration} from '../src/native-dailyrecord-backend.ts';
import {DailyRecordService} from '../src/dailyrecord.ts';

const scope={sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001',lessonDate:'2040-01-04'};
const admission={teacherId:scope.teacherId,teacherLabel:'Invented Teacher',date:scope.lessonDate,
  students:[{studentId:scope.studentId,displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
const target={studentName:'Invented Student',date:scope.lessonDate,groupId:'3',teacherKey:scope.teacherId,studentKey:scope.studentId,courseKey:'5',recordKey:'13',membershipKey:'8'};
const row={...target,fields:{progress:'Current progress',homework:'Current homework',lessonMemo:'Current memo',attendance:'Y',persistentMemo:'Unselected private note'}};
const prior={...row,date:'2040-01-02',recordKey:'12',fields:{...row.fields,progress:'Previous teacher wording'}};
function fixture(){
 const calls:{auth:number;requests:any[]}={auth:0,requests:[]};
 const port={authenticate:async(_:unknown)=>{calls.auth++;},discoverDayRecordGroups:async()=>({groups:[{groupId:'3'}]}),
  execute:async(input:any)=>{calls.requests.push(input);return {dayRecords:[input.date===target.date?row:prior]};},
  journalState:async()=>({target,fields:{...row.fields,dailyTestAssessment:'0',previousHomeworkAssessment:'3'},previousRecordKey:'12',previousDate:prior.date,wireDigest:'a'.repeat(64)}),close:()=>{}};
 return {calls,port,backend:new NativeDailyRecordBackend(port,admission,()=>({username:'fictional',password:'fictional'}))};
}
test('any valid date reaches the source while namespace, teacher and a name for the key stay required',async()=>{
 const f=fixture();
 expect(f.backend.teacherScope()).toEqual({sourceNamespace:'academy.lms.dayrecord',teacherId:'99001'});
 // A date other than the launch date reaches the source; a foreign returned date still fails identity qualification.
 await expect(f.backend.readDailyRecord({...scope,lessonDate:'2040-01-05'})).rejects.toThrow('native_dailyrecord_identity_conflict');
 expect(f.calls.auth).toBe(1);expect(f.calls.requests[0].date).toBe('2040-01-05');
 const historical=fixture();
 await expect(historical.backend.readDailyRecord({...scope,lessonDate:'2031-01-01'})).rejects.toThrow('native_dailyrecord_identity_conflict');
 expect(historical.calls.requests[0].date).toBe('2031-01-01');
});
test('the native source port rereads current/prior occurrences, exports only six selected fields and keeps source revision independent of read time',async()=>{
 const {backend,calls}=fixture();const a=await backend.readDailyRecord(scope),b=await backend.readDailyRecord(scope);
 expect(calls.auth).toBe(1);expect(calls.requests).toHaveLength(4);
 expect(a.observation.target.recordId).toBe('13');expect(a.observation.previousTarget?.recordId).toBe('12');
 expect(Object.keys(a.observation.fields).sort()).toEqual(['attendance','dailyTestAssessment','homework','lessonMemo','previousHomeworkAssessment','progress']);
 expect(JSON.stringify(a)).not.toContain('Unselected private note');expect(JSON.stringify(a)).not.toContain('password');
 expect(a.observation.toneSamples[0]!.text).toContain('Previous teacher wording');
 expect(a.observation.sourceRevision).toBe(b.observation.sourceRevision);
 expect(a.observation.fields.progress).toMatchObject({state:'known',value:'Current progress'});
});
test('foreign namespace and teacher, an invalid date and a key with no known name are refused before authentication',async()=>{
 for(const changed of [{sourceNamespace:'foreign'},{teacherId:'99002'},{studentId:'77002'},{lessonDate:'2040-02-30'}]){
  const {backend,calls}=fixture();await expect(backend.readDailyRecord({...scope,...changed})).rejects.toThrow('native_dailyrecord_scope_refused');expect(calls.auth).toBe(0);
 }
});
test('a matching name cannot substitute for current native student identity',async()=>{
 const f=fixture();f.port.execute=async()=>({dayRecords:[{...row,studentKey:'77002'}]});
 await expect(f.backend.readDailyRecord(scope)).rejects.toThrow('native_dailyrecord_identity_conflict');
});
test('a source-resolved student outside the launch list is read by the same request, by exact name only',async()=>{
 const f=fixture(),newTarget={...target,studentName:'Second Invented Student',studentKey:'77002'};
 const newRow={...row,...newTarget},newPrior={...newRow,date:'2040-01-02',recordKey:'12'};
 const port={...f.port,searchStudentsByName:async()=>[{studentName:newTarget.studentName,studentKey:newTarget.studentKey}],
  execute:async(input:any)=>({dayRecords:[input.date===newTarget.date?newRow:newPrior]}),
  journalState:async()=>({target:newTarget,fields:{...row.fields,dailyTestAssessment:'0',previousHomeworkAssessment:'3'},previousRecordKey:'12',previousDate:newPrior.date,wireDigest:'a'.repeat(64)})};
 const backend=new NativeDailyRecordBackend(port,admission,()=>({username:'fictional',password:'fictional'}));
 await backend.searchStudentsByName(newTarget.studentName,'same-request');
 const newScope={...scope,studentId:'77002'};
 await expect(backend.readDailyRecord(newScope,'other-request')).rejects.toThrow('native_dailyrecord_scope_refused');
 expect((await backend.readDailyRecord(newScope,'same-request')).status).toBe('observed');
 const service=new DailyRecordService(new NativeDailyRecordBackend(port,admission,()=>({username:'fictional',password:'fictional'})));
 expect(await service.get({displayName:newTarget.studentName,lessonDate:scope.lessonDate,purpose:'Synthetic lesson read',currentRequestRef:'composed-request'}))
  .toMatchObject({status:'observed',sourceAvailability:'observed',academyWrites:0,parentSends:0});
 const ambiguous=new NativeDailyRecordBackend({...port,searchStudentsByName:async()=>[
  {studentName:newTarget.studentName,studentKey:'77002'},{studentName:newTarget.studentName,studentKey:'77003'}]},admission,()=>({username:'fictional',password:'fictional'}));
 await ambiguous.searchStudentsByName(newTarget.studentName,'same-request');
 await expect(ambiguous.readDailyRecord(newScope,'same-request')).rejects.toThrow('native_dailyrecord_scope_refused');
});
test('date-qualified lookup uses the exact dated lesson row instead of a recent-test search',async()=>{
 const f=fixture(),events:string[]=[];
 const port={...f.port,searchStudentsByName:async()=>{events.push('recent-test');return [];},
  discoverDayRecordGroups:async(input:any)=>{events.push('groups:'+input.date);return {groups:[{groupId:'3'}]};},
  execute:async(input:any)=>{events.push('day:'+input.date);return {dayRecords:[input.date===target.date?row:prior]};}};
 const service=new DailyRecordService(new NativeDailyRecordBackend(port,admission,()=>({username:'fictional',password:'fictional'})));
 const result=await service.get({displayName:'Invented Student',lessonDate:scope.lessonDate,purpose:'Synthetic dated read',currentRequestRef:'dated-request'});
 expect(result).toMatchObject({status:'observed',sourceAvailability:'observed'});
 expect(events).toEqual(['groups:2040-01-04','day:2040-01-04','groups:2040-01-04','day:2040-01-04','day:2040-01-02']);
 expect(events).not.toContain('recent-test');
});
test('date-qualified lookup never substitutes a foreign source row or falls back after a miss',async()=>{
 for(const rows of [[],[{...row,teacherKey:'99002'}],[row,{...row,recordKey:'14'}]]){
  let recent=0;const f=fixture(),port={...f.port,searchStudentsByName:async()=>{recent++;return [];},execute:async()=>({dayRecords:rows})};
  const service=new DailyRecordService(new NativeDailyRecordBackend(port,admission,()=>({username:'fictional',password:'fictional'})));
  const result=await service.get({displayName:'Invented Student',lessonDate:scope.lessonDate,purpose:'Synthetic dated read',currentRequestRef:'dated-request'});
  expect(result.status).toBe('unavailable');expect(recent).toBe(0);
 }
});
test('a foreign previous occurrence cannot supply homework assessment or a tone sample',async()=>{
 const f=fixture();f.port.execute=async(input:any)=>({dayRecords:[input.date===target.date?row:{...prior,recordKey:'11'}]});
 await expect(f.backend.readDailyRecord(scope)).rejects.toThrow('native_dailyrecord_previous_conflict');
});
test('authentication failure is retained, without automatically issuing another login',async()=>{
 const f=fixture();f.port.authenticate=async()=>{f.calls.auth++;throw Error('source_login_challenge');};
 await expect(f.backend.readDailyRecord(scope)).rejects.toThrow('source_authentication_failed');
 await expect(f.backend.readDailyRecord(scope)).rejects.toThrow('source_authentication_failed');expect(f.calls.auth).toBe(1);
});
test('native configuration rejects caller authority fields, duplicates and non-native launch',async()=>{
 const v={schemaVersion:'academy.native-dailyrecord-read/1',sourceBundlePath:'/explicit/bundle.mjs',sourceBundleSha256:'b'.repeat(64),date:scope.lessonDate,students:admission.students};
 expect(nativeDailyRecordConfigSchema.parse(v).students).toHaveLength(1);
 expect(()=>nativeDailyRecordConfigSchema.parse({...v,approved:true})).toThrow();
 expect(()=>nativeDailyRecordConfigSchema.parse({...v,students:[...v.students,...v.students]})).toThrow();
 await expect(nativeDailyRecordConfiguration(['--dailyrecord-native','/missing/config.json','b'.repeat(64)])).rejects.toThrow('native_dailyrecord_owner_required');
});

const textbookRequest={displayName:'Invented Student',purpose:'Read fictional textbook sections',currentRequestRef:'synthetic-textbook-request'};
function textbookFixture(bindings:{courseKey:string;bookNo:string;studentKey:string;bookLabel:string}[]){
 const calls={inspect:0,execute:0,inspectInputs:[] as any[],executeInputs:[] as any[]};
 const port={authenticate:async()=>{},discoverDayRecordGroups:async()=>({groups:[]}),
  readTextbookBindings:async()=>bindings,
  inspectTextbookIndex:async(input:any)=>{calls.inspect++;calls.inspectInputs.push(input);
   return {rows:[{sourceTitle:'Unit for book '+input.bookNo,sourceDate:{literal:'10-23',meaning:'source_apply_date_not_exact_grade_time'}}]};},
  execute:async(input:any)=>{calls.execute++;calls.executeInputs.push(input);
   return {family:'textbook',status:'observed',observedAt:'2040-01-16T09:00:00.000Z',coverage:'x',
    records:input.sourceTitles.map((sourceTitle:string)=>({sourceTitle,studentKey:'77001',studentName:'Invented Student',courseKey:input.courseKey,bookNo:input.bookNo,score:80,items:[{originalOrdinal:'1',lectureKey:'8101',outerLectureKey:null,scoring:'O'}],sourceDate:{literal:null,precision:'unknown',meaning:'source_apply_date_not_exact_grade_time'}}))};},
  journalState:async()=>{throw Error('not_called');},close:()=>{}};
 return {calls,backend:new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}))};
}
const multiBindings=[{courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'Book One'},{courseKey:'6',bookNo:'8',studentKey:'77001',bookLabel:'Book Two'}];
test('textbook sections by name: multiple bindings without bookLabel returns interview_required candidates with first-title hints and no grading',async()=>{
 const {calls,backend}=textbookFixture(multiBindings);
 const result=await backend.readTextbookSectionsByName(textbookRequest);
 expect(result).toMatchObject({kind:'interview_required',student:{studentId:'77001',displayName:'Invented Student'},
  bindings:[{bookLabel:'Book One',hint:'Unit for book 7'},{bookLabel:'Book Two',hint:'Unit for book 8'}],reason:'multiple_book_bindings_select_one'});
 expect(typeof (result as any).observedAt).toBe('string');
 expect(calls.inspect).toBe(2);expect(calls.execute).toBe(0);
 // The service surfaces the same candidates as interview_required, not observed/unavailable.
 const serviceResult=await new DailyRecordService(backend).textbookSectionsByName(textbookRequest);
 expect(serviceResult).toMatchObject({status:'interview_required',kind:'interview_required',bindings:[{bookLabel:'Book One',hint:'Unit for book 7'},{bookLabel:'Book Two',hint:'Unit for book 8'}]});
 expect(calls.inspect).toBe(4);expect(calls.execute).toBe(0);
});
test('textbook sections by name: exact bookLabel selects that binding for index and grading',async()=>{
 const {calls,backend}=textbookFixture(multiBindings);
 expect(await backend.readTextbookSectionsByName({...textbookRequest,bookLabel:'Book Two'}))
  .toMatchObject({courseKey:'6',bookNo:'8',bookLabel:'Book Two',index:[{sourceTitle:'Unit for book 8'}]});
 expect(calls.inspectInputs.at(-1)).toMatchObject({courseKey:'6',bookNo:'8'});
 const graded=await backend.readTextbookSectionsByName({...textbookRequest,bookLabel:'Book Two',sourceTitles:['Unit for book 8']});
 expect(graded).toMatchObject({courseKey:'6',bookNo:'8',records:[{sourceTitle:'Unit for book 8',score:80,correctCount:1,itemCount:1}]});
 expect(calls.executeInputs.at(-1)).toMatchObject({courseKey:'6',bookNo:'8'});
});
test('textbook sections by name: bookLabel with no exact match still throws textbook_binding_missing_or_ambiguous',async()=>{
 const {calls,backend}=textbookFixture(multiBindings);
 await expect(backend.readTextbookSectionsByName({...textbookRequest,bookLabel:'No Such Book'})).rejects.toThrow('textbook_binding_missing_or_ambiguous');
 expect(calls.inspect).toBe(0);expect(calls.execute).toBe(0);
});
test('textbook sections by name: unqualified book labels fail closed before candidates',async()=>{
 const percent=textbookFixture([{courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'88.8 %'},{...multiBindings[1]!}]);
 await expect(percent.backend.readTextbookSectionsByName(textbookRequest)).rejects.toThrow('textbook_binding_source_unqualified');
 expect(percent.calls.inspect).toBe(0);expect(percent.calls.execute).toBe(0);
 const dupLabel=textbookFixture([{...multiBindings[0]!,bookLabel:'Same Book'},{...multiBindings[1]!,bookLabel:'Same Book'}]);
 expect(await dupLabel.backend.readTextbookSectionsByName(textbookRequest)).toMatchObject({bindings:[{bookLabel:'Same Book',bookNo:'7'},{bookLabel:'Same Book',bookNo:'8'}]});
 await expect(dupLabel.backend.readTextbookSectionsByName({...textbookRequest,bookLabel:'Same Book'})).rejects.toThrow('textbook_binding_missing_or_ambiguous');
 const {backend}=textbookFixture(multiBindings);
 expect(await backend.readTextbookSectionsByName(textbookRequest)).toMatchObject({bindings:[{bookLabel:'Book One',bookNo:'7'},{bookLabel:'Book Two',bookNo:'8'}]});
});
test('textbook sections by name: single binding without bookLabel is unchanged',async()=>{
 const {backend}=textbookFixture([multiBindings[0]!]);
 expect(await backend.readTextbookSectionsByName(textbookRequest))
  .toMatchObject({courseKey:'5',bookNo:'7',bookLabel:'Book One',index:[{sourceTitle:'Unit for book 7'}]});
});

test('a dated source proof is request/date bound and expires before a dynamic student read',async()=>{
 const f=fixture(),newRow={...row,studentName:'Second Invented Student',studentKey:'77002'};
 const backend=new NativeDailyRecordBackend({...f.port,execute:async()=>({dayRecords:[newRow]})},admission,()=>({username:'fictional',password:'fictional'}));
 await backend.searchStudentsByName(newRow.studentName,'dated-proof',scope.lessonDate);
 await expect(backend.readDailyRecord({...scope,studentId:'77002',lessonDate:'2040-01-05'},'dated-proof')).rejects.toThrow('native_dailyrecord_scope_refused');
 const realNow=Date.now;try{const later=realNow()+300001;Date.now=()=>later;
  await expect(backend.readDailyRecord({...scope,studentId:'77002'},'dated-proof')).rejects.toThrow('native_dailyrecord_scope_refused');
 }finally{Date.now=realNow;}
});

// One tool call sends one batch upstream; a failed or unqualified batch is
// reported with the same code as before and is never followed by another call.
type UpstreamRecord={sourceTitle:string;studentKey:string;score:number};
function countingTextbook(answer:(titles:string[])=>unknown){
 const calls:string[][]=[];
 const record=(r:UpstreamRecord)=>({sourceTitle:r.sourceTitle,studentKey:r.studentKey,studentName:'Invented Student',courseKey:'5',bookNo:'7',score:r.score,
  items:[{originalOrdinal:'1',lectureKey:'8101',outerLectureKey:null,scoring:r.score?'O':'X'}],sourceDate:{literal:null,precision:'unknown',meaning:'source_apply_date_not_exact_grade_time'}});
 const port={authenticate:async()=>{},discoverDayRecordGroups:async()=>({groups:[]}),readTextbookBindings:async()=>[multiBindings[0]!],
  execute:async(input:any)=>{calls.push([...input.sourceTitles]);const out=answer([...input.sourceTitles]) as any;
   return Array.isArray(out)?{family:'textbook',status:'observed',observedAt:'2040-01-16T09:00:00.000Z',coverage:'x',records:out.map(record)}:out;},
  journalState:async()=>{throw Error('not_called');},close:()=>{}};
 const read=(sourceTitles:string[])=>new DailyRecordService(new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'})))
  .textbookSectionsByName({...textbookRequest,sourceTitles});
 return {calls,read};
}
const ok=(sourceTitle:string,score=100):UpstreamRecord=>({sourceTitle,studentKey:'77001',score});
const notFound=(titles:string[])=>{if(titles.some(t=>t.startsWith('Missing')))throw Error('textbook_section_not_in_either_observed_source');return titles.map(t=>ok(t));};
{
 const cases:[string,(t:string[])=>unknown,Record<string,unknown>][]=[
  ['record of another studentKey',()=>[ok('A'),{...ok('B'),studentKey:'77002'}],{status:'unavailable',reasonCode:'textbook_section_result_unqualified'}],
  ['two conflicting records for one title',()=>[ok('A',100),ok('A',0)],{status:'unavailable',reasonCode:'textbook_section_scope_conflict'}],
  ['extra conflicting record',()=>[ok('A'),ok('B'),ok('B',0)],{status:'unavailable',reasonCode:'textbook_section_result_unqualified'}],
 ];
 for(const [name,answer,expected] of cases)test(`multi-title textbook read: ${name} stays a refusal after exactly one upstream call`,async()=>{
  const {calls,read}=countingTextbook(answer);
  expect(await read(['A','B'])).toMatchObject(expected);
  expect(calls).toEqual([['A','B']]);
 });
}
{
 const cases:[string,string[],(t:string[])=>unknown,Record<string,unknown>][]=[
  ['unknown error',['A','B'],()=>{throw Error('fictional_upstream_fault');},{status:'unavailable',reasonCode:'textbook_section_unqualified_no_automatic_retry'}],
  ['blocked batch',['A','B'],()=>({family:'textbook',status:'blocked',records:[],observedAt:'2040-01-16T09:00:00.000Z',coverage:'x'}),{status:'unavailable',reasonCode:'textbook_section_result_unqualified'}],
  ['one title not found',['A','Missing B'],notFound,{status:'not_observed_in_either_source'}],
  ['one title not returned',['A','Missing B'],()=>[ok('A')],{status:'unavailable',reasonCode:'textbook_section_result_unqualified'}],
  ['all titles not found',['Missing A','Missing B'],notFound,{status:'not_observed_in_either_source'}],
 ];
 for(const [name,titles,answer,expected] of cases)test(`multi-title textbook read: ${name} gives the batch's code after exactly one upstream call`,async()=>{
  const {calls,read}=countingTextbook(answer);const result=await read(titles);
  expect(result).toMatchObject({...expected,academyWrites:0,parentSends:0,automaticRetryAllowed:false});
  expect('failedTitles' in result).toBe(false);
  expect(calls).toEqual([titles]);
 });
}
test('multi-title textbook read keeps the source order of records',async()=>{
 const {calls,read}=countingTextbook(()=>[ok('B',0),ok('A',100)]);
 const result=await read(['A','B']) as any;
 expect(result.status).toBe('observed');
 expect(result.records.map((r:any)=>r.sourceTitle)).toEqual(['B','A']);
 expect(calls).toEqual([['A','B']]);
});
// A first lesson: the day row has no previous-lesson homework handler or control,
// so the client state carries no previous record and no previous homework assessment.
function firstLessonFixture(){
 const f=fixture(),days:string[]=[];
 const port={...f.port,execute:async(input:any)=>{days.push(input.date);return {dayRecords:input.date===target.date?[row]:[prior]};},
  journalState:async()=>({target,fields:{...row.fields,dailyTestAssessment:'0'},previousRecordKey:null,previousDate:null,wireDigest:'a'.repeat(64)})};
 return {days,port};
}
test('a first lesson reads as observed without a previous day-record request',async()=>{
 const {days,port}=firstLessonFixture();
 const result=await new NativeDailyRecordBackend(port,admission,()=>({username:'fictional',password:'fictional'})).readDailyRecord(scope);
 expect(result.status).toBe('observed');expect(days).toEqual([scope.lessonDate]);
 expect(result.observation.fields.previousHomeworkAssessment).toEqual({state:'unknown',reasonCode:'first_lesson_no_previous_record'});
 expect(result.observation.previousTarget).toBeNull();expect(result.observation.toneSamples).toEqual([]);
 expect(result.observation.fields.progress).toMatchObject({state:'known',value:'Current progress'});
 expect(result.observation.fields.dailyTestAssessment).toMatchObject({state:'known',value:'0'});
});
test('academy_dailyrecord_get of a first lesson is observed with no blocker and no previous homework interview',async()=>{
 const {port}=firstLessonFixture();
 const open={...range,schemaVersion:'academy.owner-admission/2' as const,allowedFrom:null,allowedThrough:null,studentScope:'source_verified' as const};
 const service=new DailyRecordService(new NativeDailyRecordBackend(port,admission,()=>({username:'fictional',password:'fictional'}),open));
 const result=await service.get({displayName:'Invented Student',lessonDate:scope.lessonDate,purpose:'Synthetic first lesson read',currentRequestRef:'first-lesson-request'}) as any;
 expect(result).toMatchObject({status:'observed',sourceAvailability:'observed'});
 expect(result.reasonCode).toBeUndefined();
 expect(result.preparation.interviews.map((i:any)=>i.field)).not.toContain('previousHomeworkAssessment');
 const previous=result.fields.find((f:any)=>f.field==='previousHomeworkAssessment');
 expect(previous).toMatchObject({before:{state:'unknown',reasonCode:'first_lesson_no_previous_record'},target:null});
 const intake=await service.intake({context:{schemaVersion:'academy.dailyrecord-user-context/1',purpose:'Synthetic first lesson intake',currentRequestRef:'first-lesson-intake',
  scope,selectedFields:['progress','homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment'],memoAnswer:{kind:'not_supplied'},facts:[]}}) as any;
 expect(intake.sourceAvailability).toBe('observed');expect(intake.blockers).toEqual([]);
 expect(intake.interviews.map((i:any)=>i.field)).not.toContain('previousHomeworkAssessment');
 // The other five fields keep the dispositions of a lesson with an earlier lesson.
 const earlierService=new DailyRecordService(new NativeDailyRecordBackend(fixture().port,admission,()=>({username:'fictional',password:'fictional'}),open));
 const earlier=await earlierService.intake({context:{schemaVersion:'academy.dailyrecord-user-context/1',purpose:'Synthetic earlier lesson intake',currentRequestRef:'earlier-lesson-intake',
  scope,selectedFields:['progress','homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment'],memoAnswer:{kind:'not_supplied'},facts:[]}}) as any;
 const others=(r:any)=>r.fields.filter((f:any)=>f.field!=='previousHomeworkAssessment').map((f:any)=>[f.field,f.disposition]);
 expect(others(intake)).toEqual(others(earlier));
});
