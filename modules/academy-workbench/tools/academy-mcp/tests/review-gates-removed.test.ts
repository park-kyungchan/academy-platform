import {expect,test} from 'bun:test';
import {mkdtempSync,readFileSync,readdirSync,rmSync,statSync,existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {createDailyRecordWorkflow} from '../src/dailyrecord-workflow.ts';
import type {DailyRecordContext,DailyRecordField} from '../src/dailyrecord-context.ts';
import {backend,clock,context,fact,observation,safeguard} from './dailyrecord-fixture.ts';
import {nativeWorkflowService,proposals,userRequest,workflowContext} from './dailyrecord-workflow-fixture.ts';

// Fictional book, unit and sections that no learning catalog holds.
const progressLine='- 가상수학 3-1 2단원 2-3~2-5 개념 학습';
const homeworkLine='- 가상수학 3-1 2단원 2-6~2-8 풀이';
// What the retired learning reviewer answered for a student outside its list.
const unlistedReviewer={bundleSha256:'5'.repeat(64),configSha256:'6'.repeat(64),catalogSha256:'7'.repeat(64),
  scopeQualified:()=>false,review:()=>['student_namespace_bridge_unverified_ask_teacher']};
const retiredMembers={
  learningEntries:[{bookRef:'fictional-book',unitRef:'fictional-unit',scope:'sections' as const,sectionRefs:['fictional-section-1'],activity:'풀이',deadline:null}],
  teacherReported:{preserveBefore:false,mappedText:'',lines:[{factRef:'user-fact:progress',text:'- 다른 문안'}]},
};

function nativeService(base:string,policy:unknown){
  const workflow=createDailyRecordWorkflow({storeDir:resolve(base,'owner-store')});
  const native=nativeWorkflowService(base,workflow).service;
  return new DailyRecordService(native.backend,safeguard,Date.now,policy as never,native.writeBackend,workflow);
}
const writes=(base:string)=>existsSync(resolve(base,'events.txt'))?readFileSync(resolve(base,'events.txt'),'utf8').split('\n').filter(l=>l.startsWith('write:')):[];

async function reviewAndSave(policy:unknown,withRetiredMembers:boolean){
  const base=mkdtempSync(resolve(tmpdir(),'gates-2-review-'));
  try{
    const fields={progress:progressLine,homework:homeworkLine},requestRef='synthetic-gates-2';
    const c=workflowContext(fields,requestRef);
    const offered=proposals(fields,requestRef).map(p=>withRetiredMembers?{...p,...retiredMembers}:p);
    const intake:any=await nativeService(base,policy).intake({context:c});
    expect(intake.observationPersistence).toBe('owner_durable');
    const review:any=await nativeService(base,policy).review({context:c,observationHandle:intake.observationHandle,proposals:offered});
    expect(review.rows.flatMap((r:any)=>r.blockers)).toEqual([]);
    expect(review.status).toBe('prepared');
    for(const row of review.rows){
      expect(row).not.toHaveProperty('learningSuggestedRendering');expect(row).not.toHaveProperty('learningSuggestionMeaning');expect(row).not.toHaveProperty('expectedActivityFormat');
    }
    const saved:any=await nativeService(base,policy).save({context:c,observationHandle:intake.observationHandle,reviewDigest:review.reviewDigest,proposals:offered,request:userRequest(requestRef)});
    expect(saved).toMatchObject({status:'saved',academyWrites:2});
    expect(saved.fields.map((f:any)=>[f.field,f.status])).toEqual([['progress','persisted'],['homework','persisted']]);
    expect(writes(base)).toEqual(['write:progress','write:homework']);
    const readback:any=await nativeService(base,policy).intake({context:workflowContext(fields,'synthetic-gates-2-readback')});
    const value=(field:DailyRecordField)=>readback.fields.find((f:any)=>f.field===field).before;
    expect(value('progress')).toMatchObject({state:'known',value:progressLine});
    expect(value('homework')).toMatchObject({state:'known',value:homeworkLine});
  }finally{rmSync(base,{recursive:true,force:true});}
}

test('check 1: progress and homework naming no catalogued book are prepared and saved without a learning reviewer',async()=>{
  await reviewAndSave(null,false);
},30000);

test('check 1: a learning reviewer handed to the service is not consulted for a student outside its list',async()=>{
  await reviewAndSave(unlistedReviewer,false);
},30000);

test('check 2: learningEntries and teacherReported are accepted and ignored',async()=>{
  await reviewAndSave(unlistedReviewer,true);
},30000);

test('check 2: a server launched with --dailyrecord-learning and missing paths starts and lists its tools',async()=>{
  const missing=resolve(tmpdir(),'gates-2-missing-'+process.pid);
  for(const entry of ['dailyrecord-server.ts','label-server.ts']){
    const client=new Client({name:'synthetic-gates-2-verifier',version:'0.1.0'});
    const transport=new StdioClientTransport({command:process.execPath,args:['--preload',resolve(import.meta.dir,'network-deny.ts'),resolve(import.meta.dir,'../src',entry),
      '--dailyrecord-learning',missing+'/bundle.mjs','a'.repeat(64),missing+'/config.json','b'.repeat(64)],env:{PATH:process.env.PATH??'/usr/bin:/bin'},stderr:'pipe'});
    await client.connect(transport);
    try{
      const tools=(await client.listTools()).tools.map(t=>t.name);
      expect(tools).toContain('academy_dailyrecord_review');expect(tools).toContain('academy_dailyrecord_save');
    }finally{await client.close();await transport.close();}
  }
  expect(existsSync(missing)).toBe(false);
},30000);

function attendanceContext():DailyRecordContext{
  const c=context();c.selectedFields=['attendance'];
  c.facts=[{...fact('attendance','출석'),explicitValue:'Y'}];
  return c;
}
async function reviewAttendance(current:string){
  const o=observation();o.fields.attendance={state:'known',value:current,evidenceRefs:['synthetic-source-evidence']};
  const service=new DailyRecordService(backend(()=>o),safeguard,()=>clock),c=attendanceContext();
  const intake:any=await service.intake({context:c});
  const p={field:'attendance' as const,value:'Y',factRefs:['user-fact:attendance'],decision:{kind:'set' as const,requestRef:c.currentRequestRef}};
  return service.review({context:c,observationHandle:intake.observationHandle,proposals:[p]}) as Promise<any>;
}

test('check 3: a whitespace-only current value is empty, so set needs no replace',async()=>{
  const blank=await reviewAttendance(' ');
  expect(blank.rows[0].blockers).not.toContain('existing_value_requires_explicit_replace');
  expect(blank.status).toBe('prepared');expect(blank.patch).toEqual([expect.objectContaining({field:'attendance',before:' ',after:'Y'})]);
});

test('check 3: a set on a current value with any other character is read as replace (TASK-FLEX-1)',async()=>{
  const filled=await reviewAttendance('N');
  expect(filled.rows[0].blockers).toEqual([]);expect(filled.rows[0].decision.kind).toBe('replace');
  expect(filled.patch).toEqual([expect.objectContaining({field:'attendance',before:'N',after:'Y'})]);
});

// The fixture's dates moved to one fictional lesson day; clock is that day's 09:00Z.
function datedFixture(lessonDate:string){
  const now=Date.parse(lessonDate+'T09:00:00Z');
  const move=<T extends {lessonDate:string}>(t:T,date:string)=>({...t,lessonDate:date});
  const read=()=>{
    const o=observation(),previous=new Date(now-2*86400000).toISOString().slice(0,10),tone=new Date(now-4*86400000).toISOString().slice(0,10);
    return {...o,scope:move(o.scope,lessonDate),target:move(o.target,lessonDate),previousTarget:move(o.previousTarget!,previous),observedAt:new Date(now).toISOString(),
      toneSamples:o.toneSamples.map(s=>({...s,target:move(s.target,tone),observedAt:new Date(now).toISOString()}))};
  };
  const c=context();c.scope={...c.scope,lessonDate};c.selectedFields=['homework'];
  return {service:new DailyRecordService(backend(read),safeguard,()=>now),c};
}
async function reviewDeadline(lessonDate:string,factText:string,value:string){
  const {service,c}=datedFixture(lessonDate);c.facts=[fact('homework',factText)];
  const intake:any=await service.intake({context:c});
  const review:any=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[{field:'homework',value,factRefs:['user-fact:homework'],decision:{kind:'replace',requestRef:c.currentRequestRef}}]});
  // The deadline check is a warning since TASK-FLEX-1: report blockers and warning codes together.
  return [...review.rows[0].blockers,...review.rows[0].warnings.map((w:{code:string})=>w.code)] as string[];
}

test('check 4: a month-day fact deadline supports the full date of the lesson year',async()=>{
  expect(await reviewDeadline('2026-11-17','기한은 11월19일(목)까지','- 2026-11-19까지')).toEqual([]);
});

test('check 4: a month-day fact deadline does not support another month',async()=>{
  expect(await reviewDeadline('2026-11-17','기한은 11월19일(목)까지','- 2026-12-19까지')).toContain('unsupported_deadline');
});

test('check 4: the next year is supported only for a month earlier than the lesson month',async()=>{
  expect(await reviewDeadline('2026-12-28','기한은 1월 4일까지','- 2027-01-04까지')).toEqual([]);
  expect(await reviewDeadline('2026-11-17','기한은 11월 19일까지','- 2027-11-19까지')).toContain('unsupported_deadline');
  expect(await reviewDeadline('2026-11-17','기한은 11월 19일까지','- 2025-11-19까지')).toContain('unsupported_deadline');
});

test('check 4: a full fact date supports the full date and the month-day form, nothing else',async()=>{
  expect(await reviewDeadline('2026-11-17','기한은 2026-11-19까지','- 2026-11-19까지')).toEqual([]);
  expect(await reviewDeadline('2026-11-17','기한은 2026-11-19까지','- 11월 19일까지')).toEqual([]);
  expect(await reviewDeadline('2026-11-17','기한은 2027-11-19까지','- 2026-11-19까지')).toContain('unsupported_deadline');
});

function agedService(changed:boolean){
  let now=clock,reads=0;
  const read=()=>{reads++;const o=observation();o.observedAt=new Date(now).toISOString();
    if(changed&&reads>1)o.fields.progress={state:'known',value:'- 교사가 방금 바꾼 진도',evidenceRefs:['synthetic-fresh']};return o;};
  const service=new DailyRecordService(backend(read),safeguard,()=>now);
  return {service,advance:(ms:number)=>{now+=ms;}};
}
async function reviewAfter(minutes:number,changed:boolean){
  const {service,advance}=agedService(changed),c=context();
  const intake:any=await service.intake({context:c});expect(intake.observationHandle).toBeString();
  advance(minutes*60000);
  return service.review({context:c,observationHandle:intake.observationHandle,proposals:[{field:'progress',value:'- 지정 문제 12문항 풀이 진행.',factRefs:['user-fact:progress'],decision:{kind:'replace',requestRef:c.currentRequestRef}}]}) as Promise<any>;
}

test('check 5: a held observation 30 minutes old with an unchanged record is prepared',async()=>{
  const review=await reviewAfter(30,false);
  expect(review.rows[0].blockers).toEqual([]);expect(review.status).toBe('prepared');
});

test('check 5: a changed record is refused by the drift check, within 5 minutes and at 30 minutes',async()=>{
  for(const minutes of [1,30]){
    const review=await reviewAfter(minutes,true);
    expect(review.status).toBe('held');expect(review.rows[0].blockers).toContain('source_before_or_contract_drift');expect(review.patch).toEqual([]);
  }
});

test('check 5: a held observation 61 minutes old is stale_source',async()=>{
  const review=await reviewAfter(61,false);
  expect(review.status).toBe('held');expect(review.rows[0].blockers).toContain('stale_source');
});

test('check 5: a fresh read older than 5 minutes is still stale_source',async()=>{
  const service=new DailyRecordService(backend(),safeguard,()=>clock+300001);
  const intake:any=await service.intake({context:context()});
  expect(intake.blockers).toContain('stale_source');expect(intake.observationHandle).toBeNull();
});

test('check 6: the module source holds none of the retired learning review codes',()=>{
  const removed=['student_namespace_bridge_unverified_ask_teacher','canonical_learning_policy_not_connected','canonical_learning_review_unavailable',
    'canonical_learning_result_unqualified','learning_render_mismatch','learning_entries_required','learning_mapping_unqualified',
    'learningSuggestedRendering','learningSuggestionMeaning','expectedActivityFormat','createDailyRecordLearningReviewer','DailyRecordLearningPolicy'];
  // teacher_reported_complete (item-label learningState) and teacher_reported_note (teacher report evidence kind) are not review codes.
  const unrelated=new Set(['teacher_reported_complete','teacher_reported_note']);
  const root=resolve(import.meta.dir,'../src'),found:string[]=[];
  const walk=(dir:string)=>{for(const name of readdirSync(dir)){const path=join(dir,name);
    if(statSync(path).isDirectory()){walk(path);continue;}
    const text=readFileSync(path,'utf8');
    for(const code of removed)if(text.includes(code))found.push(name+':'+code);
    for(const match of text.matchAll(/teacher_reported_[a-z_]*/g))if(!unrelated.has(match[0]))found.push(name+':'+match[0]);
  }};
  walk(root);
  expect(found).toEqual([]);
});
