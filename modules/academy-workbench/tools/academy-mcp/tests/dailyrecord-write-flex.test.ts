import {expect,test} from 'bun:test';
import {existsSync,mkdtempSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {parentDailyRecordLines,parentInternalLanguage} from '../../../../../apps/academy/dc-gang-a/src/contracts/parentInternalLanguage.ts';
import {DailyRecordService,type DailyRecordParentSafeguard} from '../src/dailyrecord.ts';
import {createDailyRecordWorkflow} from '../src/dailyrecord-workflow.ts';
import {backend,clock,fact,observation,safeguard} from './dailyrecord-fixture.ts';
import {FileSource,nativeScope,nativeWorkflowService} from './dailyrecord-workflow-fixture.ts';

// Purpose-built fictional records only. Each test builds the smallest call a
// low-tier model would make: the handle, the new text, the teacher's words.

/** The parent safeguard with the canonical word list and line grammar of the Education contract. */
const canonicalSafeguard:DailyRecordParentSafeguard={policySha256:'3'.repeat(64),safeguardSourceSha256:'4'.repeat(64),sourceRef:'synthetic-canonical-safeguard',
  check:text=>parentInternalLanguage.test(text),checkLines:text=>parentDailyRecordLines.test(text)};

async function handleFor(read:()=>ReturnType<typeof observation>=observation,parent:DailyRecordParentSafeguard=safeguard){
  const service=new DailyRecordService(backend(read),parent,()=>clock);
  const intake:any=await service.intake({context:{scope:observation().scope}});
  expect(intake.observationHandle).toBeString();
  return {service,handle:intake.observationHandle as string,intake};
}
const codes=(row:any)=>(row.warnings as {code:string;text:string}[]).map(w=>w.code);

test('check 1: a review with only the handle and two proposals on a filled record is prepared with replace decisions',async()=>{
  const {service,handle}=await handleFor();
  const review:any=await service.review({observationHandle:handle,proposals:[
    {field:'progress',value:'- 가상수학 2단원 개념 학습'},{field:'homework',value:'- 가상수학 2단원 연습 문제 풀이'}]});
  expect(review.status).toBe('prepared');
  expect(review.rows.flatMap((r:any)=>r.blockers)).toEqual([]);
  expect(review.rows.map((r:any)=>r.decision.kind)).toEqual(['replace','replace']);
  expect(review.interviews).toEqual([]);
  expect(review.patch.map((p:any)=>p.field)).toEqual(['progress','homework']);
  // The defaults the review filled in, so a model and the teacher can see them.
  expect(review.userContext).toMatchObject({schemaVersion:'academy.dailyrecord-user-context/1',purpose:'DailyRecord write',scope:observation().scope,
    selectedFields:['progress','homework'],facts:[],memoAnswer:{kind:'no_addition'}});
  expect(review.requestRef).toBe(review.userContext.currentRequestRef);
  expect(review.rows.every((r:any)=>r.decision.requestRef===review.requestRef)).toBe(true);
});

test('check 2: an empty attendance takes set with no decision; an explicit set on N is read as replace',async()=>{
  const blank=()=>{const o=observation();o.fields.attendance={state:'known',value:'',evidenceRefs:['synthetic-source-evidence']};return o;};
  const empty=await handleFor(blank);
  const set:any=await empty.service.review({observationHandle:empty.handle,proposals:[{field:'attendance',value:'Y'}]});
  expect(set.status).toBe('prepared');expect(set.rows[0].blockers).toEqual([]);expect(set.rows[0].decision.kind).toBe('set');
  expect(set.patch).toEqual([expect.objectContaining({field:'attendance',before:'',after:'Y'})]);

  const absent=()=>{const o=observation();o.fields.attendance={state:'known',value:'N',evidenceRefs:['synthetic-source-evidence']};return o;};
  const filled=await handleFor(absent);
  const replaced:any=await filled.service.review({observationHandle:filled.handle,proposals:[{field:'attendance',value:'Y',decision:{kind:'set'}}]});
  expect(replaced.status).toBe('prepared');expect(replaced.rows[0].blockers).toEqual([]);expect(replaced.rows[0].decision.kind).toBe('replace');
  expect(replaced.patch).toEqual([expect.objectContaining({field:'attendance',before:'N',after:'Y'})]);
});

test('check 3: no memo interview when lessonMemo is not proposed; a proposed memo is its own content; a blank memo is asked',async()=>{
  const {service,handle}=await handleFor();
  const withoutMemo:any=await service.review({observationHandle:handle,proposals:[{field:'progress',value:'- 가상수학 2단원 개념 학습'}]});
  expect(withoutMemo.interviews).toEqual([]);expect(withoutMemo.status).toBe('prepared');
  const memo='- 다음 시간 준비물은 컴퍼스입니다';
  const withMemo:any=await service.review({observationHandle:handle,proposals:[{field:'lessonMemo',value:memo}]});
  expect(withMemo.status).toBe('prepared');expect(withMemo.interviews).toEqual([]);
  expect(withMemo.userContext.memoAnswer).toMatchObject({kind:'content',text:memo});
  expect(withMemo.patch).toEqual([expect.objectContaining({field:'lessonMemo',after:memo})]);
  const blankMemo:any=await service.review({observationHandle:handle,proposals:[{field:'lessonMemo',value:''}]});
  expect(blankMemo.status).toBe('held');expect(blankMemo.interviews).toEqual([{field:'lessonMemo',reasonCode:'today_student_specific_memo_required'}]);
});

test('check 4: a deadline or 완료 claim that the facts do not hold is a warning, not a blocker',async()=>{
  const {service,handle}=await handleFor();
  const facts=[fact('homework','1월 19일까지 지정 문제 풀어오기.'),fact('progress','지정 문제 풀이 진행.')];
  const review:any=await service.review({observationHandle:handle,context:{facts},proposals:[
    {field:'homework',value:'- 1월 20일까지 지정 문제 풀어오기'},{field:'progress',value:'- 지정 문제 풀이 완료'}]});
  expect(review.status).toBe('prepared');
  expect(review.rows.flatMap((r:any)=>r.blockers)).toEqual([]);
  expect(codes(review.rows[0])).toContain('unsupported_deadline');
  expect(codes(review.rows[1])).toContain('unsupported_completion_claim');
  for(const row of review.rows)for(const warning of row.warnings)expect(warning.text).toBeString();
  expect(review.rows.map((r:any)=>r.factRefs)).toEqual([['user-fact:homework'],['user-fact:progress']]);
  expect(review.patch).toHaveLength(2);

  const none:any=await service.review({observationHandle:handle,proposals:[
    {field:'homework',value:'- 1월 20일까지 지정 문제 풀어오기'},{field:'progress',value:'- 지정 문제 12문항 풀이 완료'}]});
  expect(none.status).toBe('prepared');expect(none.rows.flatMap((r:any)=>r.blockers)).toEqual([]);
});

test('check 4: the stage, count and fragment checks are warnings too',async()=>{
  const {service,handle}=await handleFor();
  const facts=[{...fact('progress','풀이 완료. 오답 수정 진행.'),requiredFragments:['오답 수정 진행'],questionCounts:[{count:10,rendering:'유형 10'}],
    stageClaims:[{activity:'지정 문제',stage:'correction' as const,status:'in_progress' as const,rendering:'오답 수정 진행'}]}];
  const review:any=await service.review({observationHandle:handle,context:{facts},proposals:[{field:'progress',value:'- 지정 문제 12문항 풀이 완료\n- 오답 수정 완료'}]});
  expect(review.status).toBe('prepared');expect(review.rows[0].blockers).toEqual([]);
  for(const code of ['supplied_fact_fragment_missing','unsupported_question_count','count_rendering_must_preserve_count','stage_completion_mismatch:correction'])
    expect(codes(review.rows[0])).toContain(code);
});

test('check 5: an internal word or a line without the hyphen form is still blocked',async()=>{
  const {service,handle}=await handleFor(observation,canonicalSafeguard);
  const internal:any=await service.review({observationHandle:handle,proposals:[{field:'progress',value:'- 지정 문제 채점 진행'}]});
  expect(internal.status).toBe('held');expect(internal.rows[0].blockers).toContain('parent_internal_language_excluded');expect(internal.patch).toEqual([]);
  const plain:any=await service.review({observationHandle:handle,proposals:[{field:'homework',value:'지정 문제 풀어오기'}]});
  expect(plain.status).toBe('held');expect(plain.rows[0].blockers).toContain('parent_hyphen_lines_required');expect(plain.patch).toEqual([]);
});

test('check 3 and 5: the other kept blockers still hold a minimal review',async()=>{
  const {service,handle}=await handleFor();
  const code:any=await service.review({observationHandle:handle,proposals:[{field:'dailyTestAssessment',value:'80'}]});
  expect(code.rows[0].blockers).toContain('invalid_explicit_field_value');
  const long:any=await service.review({observationHandle:handle,proposals:[{field:'progress',value:'- '+'가'.repeat(400)}]});
  expect(long.rows[0].blockers).toContain('invalid_explicit_field_value');
  const first=await handleFor(()=>{const o=observation();o.previousTarget=null;o.fields.previousHomeworkAssessment={state:'unknown',reasonCode:'first_lesson_no_previous_record'};return o;});
  const previous:any=await first.service.review({observationHandle:first.handle,proposals:[{field:'previousHomeworkAssessment',value:'4'}]});
  expect(previous.status).toBe('held');expect(previous.rows[0].blockers).toContain('independent_previous_record_required');
  await expect(service.review({observationHandle:handle,proposals:[{field:'notAField',value:'- x'}]})).rejects.toThrow();
  await expect(service.review({proposals:[{field:'progress',value:'- x'}]})).rejects.toThrow('observation_handle_or_context_scope_required');
});

const writes=(base:string)=>existsSync(resolve(base,'events.txt'))?readFileSync(resolve(base,'events.txt'),'utf8').split('\n').filter(l=>l.startsWith('write:')):[];
async function flexWorkflow(run:(input:{base:string;service:()=>DailyRecordService;source:FileSource})=>Promise<void>){
  const base=mkdtempSync(resolve(tmpdir(),'flex-1-save-'));
  try{
    const workflow=createDailyRecordWorkflow({storeDir:resolve(base,'owner-store')});
    // Every call runs in a new service, as every tool call runs in a new endpoint process.
    const service=()=>nativeWorkflowService(base,workflow).service;
    await run({base,service,source:new FileSource(base)});
  }finally{rmSync(base,{recursive:true,force:true});}
}
const values={progress:'- 가상수학 2단원 개념 학습',homework:'- 가상수학 2단원 연습 문제 풀이',lessonMemo:'- 다음 시간 준비물은 컴퍼스입니다'};
async function minimalReview(service:DailyRecordService){
  const intake:any=await service.intake({context:{scope:{...nativeScope}}});
  expect(intake.observationPersistence).toBe('owner_durable');
  const review:any=await service.review({observationHandle:intake.observationHandle,proposals:Object.entries(values).map(([field,value])=>({field,value}))});
  expect(review.status).toBe('prepared');
  return review;
}

test('check 6: a save with only the review digest and the instruction writes the reviewed fields and reads them back',async()=>{
  await flexWorkflow(async({base,service,source})=>{
    const review=await minimalReview(service());
    const saved:any=await service().save({reviewDigest:review.reviewDigest,instruction:'진도, 숙제, 메모 이대로 저장해줘.'});
    expect(saved).toMatchObject({status:'saved',academyWrites:3});
    expect(saved.fields.map((f:any)=>[f.field,f.status])).toEqual([['progress','persisted'],['homework','persisted'],['lessonMemo','persisted']]);
    expect(writes(base)).toEqual(['write:progress','write:homework','write:lessonMemo']);
    expect(source.read()).toMatchObject(values);
    const readback:any=await service().intake({context:{scope:{...nativeScope}}});
    for(const [field,value] of Object.entries(values))expect(readback.fields.find((f:any)=>f.field===field).before).toMatchObject({state:'known',value});
    // The object form of the instruction is the same instruction.
    const again:any=await service().save({reviewDigest:review.reviewDigest,instruction:{text:'진도, 숙제, 메모 이대로 저장해줘.'}});
    expect(again).toMatchObject({status:'saved',academyWrites:0,reasonCode:'duplicate_execution_already_settled'});
    expect(writes(base)).toHaveLength(3);
  });
},30000);

test('check 6: a save whose record drifted after the review writes nothing',async()=>{
  await flexWorkflow(async({base,service,source})=>{
    const review=await minimalReview(service());
    source.write({...source.read(),homework:'- 교사가 방금 바꾼 숙제'});
    const saved:any=await service().save({reviewDigest:review.reviewDigest,instruction:'저장해줘.'});
    expect(saved).toMatchObject({status:'held',reasonCode:'fresh_review_drift_new_review_required',academyWrites:0});
    expect(writes(base)).toEqual([]);
  });
},30000);

test('check 6: a save with an empty instruction writes nothing',async()=>{
  await flexWorkflow(async({base,service})=>{
    const review=await minimalReview(service());
    for(const instruction of ['','   ',{text:''}]){
      const saved:any=await service().save({reviewDigest:review.reviewDigest,instruction});
      expect(saved).toMatchObject({status:'held',reasonCode:'user_instruction_provenance_required',academyWrites:0});
    }
    const unknown:any=await service().save({reviewDigest:'9'.repeat(64),instruction:'저장해줘.'});
    expect(unknown).toMatchObject({status:'held',reasonCode:'prior_review_record_required',academyWrites:0});
    expect(writes(base)).toEqual([]);
  });
},30000);
