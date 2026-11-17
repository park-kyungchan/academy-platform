import {expect,test} from 'bun:test';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {dailyRecordContextSchema,ASSESSMENT_CODE_TABLES} from '../src/dailyrecord-context.ts';
import {DailyRecordWorkflowStore} from '../src/dailyrecord-workflow.ts';
import {dailyRecordToolConfiguration} from '../src/dailyrecord-tools.ts';
import {backend,clock,context,fact,observation,proposal,safeguard,learningPolicy} from './dailyrecord-fixture.ts';

async function setup(){const service=new DailyRecordService(backend(),safeguard,()=>clock,learningPolicy);const c=context();const intake=await service.intake({context:c});return {service,c,handle:intake.observationHandle!};}
test('missing context and caller authority/source flags fail before any backend call',async()=>{
  let calls=0;const service=new DailyRecordService(backend(()=>{calls++;return observation();}),safeguard,()=>clock,learningPolicy);
  for(const input of [{},{context:{...context(),approved:true}},{context:{...context(),verified:true}},{context:{...context(),scope:{...context().scope,studentId:''}}},{context:{...context(),selectedFields:['progress','progress']}}])await expect(service.intake(input)).rejects.toThrow();
  expect(calls).toBe(0);
});
test('unconfigured intake performs useful missing-only interview without inventing source success',async()=>{
  const service=new DailyRecordService(null,null,()=>clock,learningPolicy),c=context();c.memoAnswer={kind:'not_supplied'};
  const result=await service.intake({context:c});
  expect(result).toMatchObject({status:'held',sourceAvailability:'unavailable',observationHandle:null,academyWrites:0,parentSends:0});
  expect(result.interviews.map(v=>v.field)).toEqual(['homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment']);
  expect(result.toneSamples).toEqual([]);expect(result.fields.find(f=>f.field==='progress')?.disposition).toBe('user_facts_available');
});
test('qualified existing assessments/attendance stay preserved and previous target stays independent',async()=>{
  const service=new DailyRecordService(backend(),safeguard,()=>clock,learningPolicy),c=context();const result=await service.intake({context:c});
  expect(result.status).toBe('ready_for_review');expect(result.interviews).toEqual([]);
  for(const field of ['attendance','dailyTestAssessment','previousHomeworkAssessment'])expect(result.fields.find(f=>f.field===field)?.disposition).toBe('preserve_existing');
  expect(result.fields.find(f=>f.field==='previousHomeworkAssessment')?.target?.recordId).toBe('previous-record-synthetic');
  expect(result.fields.find(f=>f.field==='dailyTestAssessment')?.target?.recordId).toBe('current-record-synthetic');
});
test('caller mutation of returned context cannot retarget an issued source handle',async()=>{
  const service=new DailyRecordService(backend(),safeguard,()=>clock,learningPolicy),c=context(),intake=await service.intake({context:c});
  intake.userContext.scope.studentId='foreign-mutated';
  await expect(service.review({context:intake.userContext,observationHandle:intake.observationHandle,proposals:[proposal('progress','지정 문제 12문항 풀이 진행.')]})).rejects.toThrow('exact_backend_observation_handle_required');
  const result=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[proposal('progress','지정 문제 12문항 풀이 진행.')]});expect(result.status).toBe('prepared');
});
test('unknown assessments need explicit field values; activity prose does not establish scores or attendance',async()=>{
  const o=observation();o.fields.attendance={state:'unknown',reasonCode:'no-attendance'};o.fields.dailyTestAssessment={state:'known',value:'-1',evidenceRefs:['synthetic']};o.fields.previousHomeworkAssessment={state:'unknown',reasonCode:'no-rate'};
  const service=new DailyRecordService(backend(()=>o),safeguard,()=>clock,learningPolicy),c=context();c.facts.push(fact('dailyTestAssessment','총 12문항 실시. 정답률 80%.'));
  const result=await service.intake({context:c});expect(result.interviews.map(q=>q.field)).toEqual(['attendance','dailyTestAssessment','previousHomeworkAssessment']);
  c.selectedFields.push('dailyTestAssessment');const unsupported=await service.intake({context:c,suppliedValues:[proposal('dailyTestAssessment','8')]});
  expect(unsupported.blockers).toContain('dailyTestAssessment:explicit_user_source_option_fact_required');expect(unsupported.interviews.map(q=>q.field)).toContain('dailyTestAssessment');
  c.facts[c.facts.length-1]={...c.facts[c.facts.length-1],explicitValue:'8'};const supplied=await service.intake({context:c,suppliedValues:[proposal('dailyTestAssessment','8')]});
  expect(supplied.interviews.map(q=>q.field)).not.toContain('dailyTestAssessment');
  const guessed=await service.intake({context:c,suppliedValues:[proposal('dailyTestAssessment','80')]});expect(guessed.blockers).toContain('dailyTestAssessment:invalid_explicit_field_value');
});
test('qualified blank attendance/text remains missing information, not recorded attendance or activity',async()=>{
  const o=observation();o.fields.attendance={state:'known',value:' ',evidenceRefs:['synthetic']};o.fields.homework={state:'known',value:'',evidenceRefs:['synthetic']};
  const result=await new DailyRecordService(backend(()=>o),safeguard,()=>clock,learningPolicy).intake({context:context()});expect(result.interviews.map(q=>q.field)).toEqual(['homework','attendance']);
  expect(result.fields.find(f=>f.field==='attendance')?.before).toMatchObject({state:'known',value:' '});
});
test('contradictory typed deadline rendering is rejected before backend reads',async()=>{
  let calls=0;const service=new DailyRecordService(backend(()=>{calls++;return observation();}),safeguard,()=>clock,learningPolicy),c=context();
  c.facts=[{...fact('homework','1월 19일까지 지정 문제 풀어오기.'),deadlines:[{date:'2040-01-19',rendering:'1월 20일'}]}];
  await expect(service.intake({context:c})).rejects.toThrow('deadline_rendering_must_preserve_date');expect(calls).toBe(0);
});
test('today memo is always interviewed when absent; no-addition never authorizes deletion',async()=>{
  const {service,c,handle}=await setup();c.selectedFields=['lessonMemo'];c.facts=[fact('lessonMemo','추가 메모 없음')];
  const review=await service.review({context:c,observationHandle:handle,proposals:[{...proposal('lessonMemo',''),decision:{kind:'clear',requestRef:c.currentRequestRef}}]});
  expect(review.rows[0].blockers).toContain('no_addition_preserves_existing_memo');expect(review.patch).toEqual([]);
  c.memoAnswer={kind:'not_supplied'};const intake=await service.intake({context:c});expect(intake.interviews).toEqual([{field:'lessonMemo',reasonCode:'today_student_specific_memo_required'}]);
});
test('exact handles reject foreign task/student before source reads; before drift and staleness block patch',async()=>{
  let calls=0;const service=new DailyRecordService(backend(()=>{calls++;const o=observation();if(calls>1)o.fields.homework={state:'known',value:'사용자가 방금 수정한 숙제',evidenceRefs:['synthetic-fresh']};return o;}),safeguard,()=>clock,learningPolicy);
  const c=context(),intake=await service.intake({context:c}),handle=intake.observationHandle!;
  await expect(service.review({context:{...c,scope:{...c.scope,studentId:'foreign'}},observationHandle:handle,proposals:[proposal('progress','지정 문제 12문항 풀이 진행.')]})).rejects.toThrow('exact_backend_observation_handle_required');expect(calls).toBe(1);
  const review=await service.review({context:c,observationHandle:handle,proposals:[proposal('progress','지정 문제 12문항 풀이 진행.')]});expect(review.rows[0].blockers).toContain('source_before_or_contract_drift');expect(review.patch).toEqual([]);
  const stale=new DailyRecordService(backend(),safeguard,()=>clock+300001);const result=await stale.intake({context:c});expect(result.blockers).toContain('stale_source');expect(result.observationHandle).toBeNull();
});
test('foreign current/prior/tone target, invalid date, and unbound previous value are unqualified',async()=>{
  for(const mutate of [(o:ReturnType<typeof observation>)=>{o.target.teacherId='foreign';},(o:ReturnType<typeof observation>)=>{o.previousTarget!.recordId=o.target.recordId;},(o:ReturnType<typeof observation>)=>{o.toneSamples[0].target.studentId='foreign';},(o:ReturnType<typeof observation>)=>{o.previousTarget=null;}]){
    const o=observation();mutate(o);let calls=0;const service=new DailyRecordService(backend(()=>{calls++;return o;}),safeguard,()=>clock,learningPolicy);const result=await service.intake({context:context()});expect(result.sourceAvailability).toBe('unavailable');expect(result.observationHandle).toBeNull();const again=await service.intake({context:context()});expect(again.sourceAvailability).toBe('unavailable');expect(calls).toBe(2);
  }
  await expect(new DailyRecordService().intake({context:{...context(),scope:{...context().scope,lessonDate:'2040-02-30'}}})).rejects.toThrow();
});
test('completed solving/grading cannot justify completed unconfirmed correction',async()=>{
  const {service,c,handle}=await setup();c.facts=[{...fact('progress','풀이·채점 완료. 오답 수정 진행.'),stageClaims:[{activity:'지정 문제',stage:'grading',status:'complete',rendering:'풀이·채점 완료'},{activity:'지정 문제',stage:'correction',status:'unconfirmed',rendering:'오답 수정 진행'}]}];
  const result=await service.review({context:c,observationHandle:handle,proposals:[proposal('progress','풀이·채점 완료. 오답 수정 완료.')]});
  expect(result.rows[0].blockers).toContain('stage_completion_mismatch:correction');expect(result.rows[0].blockers).toContain('unsupported_stage_completion_claim:correction');expect(result.patch).toEqual([]);
});
test('review preserves counts, deadlines, stages and existing edits; canonical blockers are field-local',async()=>{
  const {service,c,handle}=await setup();c.selectedFields=['progress','homework'];
  c.facts=[{...fact('progress','지정 문제 12문항 풀이·채점까지 완료. 오답 수정 진행.'),requiredFragments:['오답 수정 진행'],questionCounts:[{count:12,rendering:'12문항'}],stageClaims:[{activity:'지정 문제',stage:'grading',status:'complete',rendering:'풀이·채점까지 완료'}]},
    {...fact('homework','1월 19일까지 지정 7문항 풀어오기.'),questionCounts:[{count:7,rendering:'7문항'}],deadlines:[{date:'2040-01-19',rendering:'1월 19일'}]}];
  const good=await service.review({context:c,observationHandle:handle,proposals:[proposal('progress','지정 문제 12문항 풀이·채점까지 완료. 오답 수정 진행.'),proposal('homework','1월 19일까지 지정 7문항 풀어오기.')]});expect(good.status).toBe('prepared');expect(good.patch).toHaveLength(2);expect(good.academyWrites).toBe(0);expect(good.parentSends).toBe(0);
  const bad=await service.review({context:c,observationHandle:handle,proposals:[proposal('progress','교사 최종 완료판정. 15문항 오답 수정 완료.'),{...proposal('homework','1월 20일까지 지정 7문항 풀어오기.'),decision:{kind:'set',requestRef:c.currentRequestRef}}]});
  expect(bad.rows[0].blockers).toContain('parent_internal_language_excluded');expect(bad.rows[0].blockers).toContain('unsupported_question_count');expect(bad.rows[0].blockers).toContain('supplied_fact_fragment_missing');
  expect(bad.rows[1].blockers).toContain('unsupported_deadline');expect(bad.rows[1].blockers).toContain('existing_value_requires_explicit_replace');expect(bad.patch).toEqual([]);
});
test('missing canonical safeguard holds text review and no-op remains nonwriting',async()=>{
  const c=context(),service=new DailyRecordService(backend(),null,()=>clock,learningPolicy),intake=await service.intake({context:c});
  const held=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[proposal('progress','지정 문제 12문항 풀이 진행.')]});expect(held.rows[0].blockers).toContain('canonical_parent_safeguard_not_connected');
  const ready=await setup();const noop=await ready.service.review({context:ready.c,observationHandle:ready.handle,proposals:[{...proposal('progress','기존 사용자 진도 문안'),value:'기존 사용자 진도 문안'}]});expect(noop.rows[0].disposition).toBe('noop');expect(noop.patch).toEqual([]);
});
test('pure parent-text review works without source and emits substantive blockers without a patch',async()=>{
  const c=context(),service=new DailyRecordService(null,safeguard,()=>clock,learningPolicy);
  const result=await service.review({context:c,proposals:[proposal('progress','교사 최종 완료판정. 19문항 풀이 완료.')]});
  expect(result.observationHandle).toBeNull();expect(result.sourceRevision).toBeNull();expect(result.status).toBe('held');expect(result.patch).toEqual([]);
  for(const blocker of ['qualified_before_source_unavailable','parent_internal_language_excluded','unsupported_question_count','unsupported_completion_claim'])expect(result.rows[0].blockers).toContain(blocker);
});
test('review digest binds full current context, canonical pins and exact CISM source identity',async()=>{
  const {service,c,handle}=await setup(),args={context:c,observationHandle:handle,proposals:[proposal('progress','지정 문제 12문항 풀이 진행.')]};
  const first=await service.review(args);c.purpose='Additional teacher-provided purpose.';const second=await service.review(args);expect(second.rows).toEqual(first.rows);expect(second.reviewDigest).not.toBe(first.reviewDigest);expect(second.userContextDigest).not.toBe(first.userContextDigest);
  c.facts[0].text+=' 사용자 근거 설명 추가.';const third=await service.review(args);expect(third.rows).toEqual(second.rows);expect(third.reviewDigest).not.toBe(second.reviewDigest);
  const other=new DailyRecordService(null,{...safeguard,policySha256:'9'.repeat(64)},()=>clock,learningPolicy);
  const pure1=await new DailyRecordService(null,safeguard,()=>clock,learningPolicy).review({context:c,proposals:args.proposals}),pure2=await other.review({context:c,proposals:args.proposals});
  expect(pure1.rows).toEqual(pure2.rows);expect(pure1.reviewDigest).not.toBe(pure2.reviewDigest);
});
test('new read evidence refs preserve value comparisons and transmission state remains observational',async()=>{
  let calls=0;const service=new DailyRecordService(backend(()=>{const o=observation();calls++;o.fields.progress={...o.fields.progress,state:'known',value:'기존 사용자 진도 문안',evidenceRefs:['synthetic-query-'+calls]};o.transmissionState='already_sent';return o;}),safeguard,()=>clock,learningPolicy);
  const c=context(),intake=await service.intake({context:c});
  const result=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[proposal('progress','지정 문제 12문항 풀이 진행.')]});expect(result.status).toBe('prepared');expect(result.transmissionState).toBe('already_sent');expect(result.parentSends).toBe(0);
});
test('read failures close per request and serialize; no automatic retry or leaked backend error',async()=>{
  let calls=0,active=0,max=0;const service=new DailyRecordService({async readDailyRecord(){calls++;active++;max=Math.max(max,active);await new Promise(r=>setTimeout(r,5));active--;throw Error('SECRET_MUST_NOT_LEAK');}},safeguard,()=>clock,learningPolicy);
  const results=await Promise.all([service.intake({context:context()}),service.intake({context:context()})]);expect(calls).toBe(2);expect(max).toBe(1);expect(JSON.stringify(results)).not.toContain('SECRET');
});
test('CISM is unavailable by default; exact period/student/section and bound source handles are required',async()=>{
  const c=context(),selection={reportId:'report-synthetic',periodStart:'2040-01-08',periodEnd:'2040-01-14',section:'learning_support' as const};
  expect(await new DailyRecordService().cism({context:c,selection})).toMatchObject({status:'unavailable',observationHandle:null,parentSends:0});
  const cism={scope:c.scope,selection,observedAt:new Date(clock).toISOString(),sourceRevision:'5'.repeat(64),sourceReferenceDate:'2040-01-12',sectionLabel:'학습부진아 지도' as const,text:'매일 짧게 복습하는 습관을 안내합니다.',evidenceRefs:['synthetic-cism-section']};
  const port=backend();port.readCismSection=async()=>({status:'observed',observation:cism});const service=new DailyRecordService(port,safeguard,()=>clock,learningPolicy);
  const result=await service.cism({context:c,selection});expect(result.status).toBe('observed');
  const intake=await service.intake({context:c});c.selectedFields=['lessonMemo'];c.memoAnswer={kind:'content',requestRef:c.currentRequestRef,text:cism.text};c.facts=[{...fact('lessonMemo',cism.text),ref:'synthetic-cism-section',origin:'cism_section'}];
  const p={...proposal('lessonMemo',cism.text),factRefs:['synthetic-cism-section']};
  const unbound=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[p]});expect(unbound.rows[0].blockers).toContain('cism_fact_not_bound_to_exact_observed_section');
  const bound=await service.review({context:c,observationHandle:intake.observationHandle,cismObservationHandle:result.observationHandle,proposals:[p]});expect(bound.status).toBe('prepared');
  cism.sourceRevision='6'.repeat(64);const next=await service.cism({context:c,selection});
  const rebound=await service.review({context:c,observationHandle:intake.observationHandle,cismObservationHandle:next.observationHandle,proposals:[p]});expect(rebound.rows).toEqual(bound.rows);expect(rebound.reviewDigest).not.toBe(bound.reviewDigest);expect(rebound.cismSource?.sourceRevision).toBe('6'.repeat(64));
  port.readCismSection=async()=>({status:'observed',observation:{...cism,selection:{...selection,periodStart:'2040-01-01'}}});expect(await service.cism({context:c,selection})).toMatchObject({status:'unavailable'});
  await expect(service.cism({context:c,selection:{...selection,section:'other'}})).rejects.toThrow();
});
test('CISM rejects an outside-period source reference date per request without latching',async()=>{
  const c=context(),selection={reportId:'report-synthetic',periodStart:'2040-01-08',periodEnd:'2040-01-14',section:'learning_support' as const};let calls=0;
  const service=new DailyRecordService({...backend(),async readCismSection(){calls++;return {status:'observed',observation:{scope:c.scope,selection,observedAt:new Date(clock).toISOString(),sourceRevision:'5'.repeat(64),sourceReferenceDate:'2040-01-16',sectionLabel:'학습부진아 지도',text:'합성 내용',evidenceRefs:['synthetic']}};}},safeguard,()=>clock,learningPolicy);
  expect(await service.cism({context:c,selection})).toMatchObject({status:'unavailable'});expect(await service.cism({context:c,selection})).toMatchObject({status:'unavailable'});expect(calls).toBe(2);
});
test('launch configuration reuses hash-pinned canonical regex bytes without importing source',async()=>{
  const dir=await mkdtemp(resolve(tmpdir(),'synthetic-dailyrecord-policy-'));
  try{const policy=resolve(dir,'policy.md'),source=resolve(dir,'source.ts'),policyBytes='# DailyRecord authoring policy\nSynthetic test policy only.',sourceBytes="export const parentDailyRecordLines=/^- [^\\r\\n]+(?:\\n- [^\\r\\n]+)*$/;\nexport const parentInternalLanguage=/교사\\s*(?:최종\\s*)?완료\\s*판정/i;\nthrow Error('must-never-run');\n";
    await writeFile(policy,policyBytes);await writeFile(source,sourceBytes);const sha=(v:string)=>createHash('sha256').update(v).digest('hex');
    const config=await dailyRecordToolConfiguration(['--dailyrecord-policy',policy,sha(policyBytes),source,sha(sourceBytes),'--label-snapshot','synthetic']);expect(config.remainingArgs).toEqual(['--label-snapshot','synthetic']);expect(config.parentSafeguard?.check('교사 최종 완료판정')).toBe(true);
    await expect(dailyRecordToolConfiguration(['--dailyrecord-policy',policy,'0'.repeat(64),source,sha(sourceBytes)])).rejects.toThrow();
  }finally{await rm(dir,{recursive:true,force:true});}
});

// Ordinal scope is not a cardinal question count; invalid semantics must not become an upstream failure.
test('non-question rendering is a field-local hold and cannot support an invented question count',async()=>{
  const service=new DailyRecordService(backend(),safeguard,()=>clock,learningPolicy),c=context();
  c.facts=[{...fact('progress','유형 10까지 풀이 진행.'),questionCounts:[{count:10,rendering:'유형 10'}]}];
  const intake=await service.intake({context:c});
  const result=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[proposal('progress','유형 10까지 10문항 풀이 진행.')]});
  expect(result.status).toBe('held');expect(result.patch).toEqual([]);
  expect(result.rows[0].blockers).toContain('count_rendering_must_preserve_count');
  expect(result.rows[0].blockers).toContain('unsupported_question_count');
  expect(result.rows[0].holdActions.some(a=>a.action==='correct_fact')).toBe(true);
  c.facts=[{...fact('progress','유형 10까지 풀이 진행.'),requiredFragments:['유형 10']}];
  const corrected=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[proposal('progress','유형 10까지 풀이 진행.')]});
  expect(corrected.rows[0].blockers).not.toContain('count_rendering_must_preserve_count');
});

test('dailyRecordFactSchema defaults omitted array fields to empty arrays',()=>{
  const base=context();
  const parsed=dailyRecordContextSchema.parse({
    ...base,
    facts:[{ref:'f-min',field:'dailyTestAssessment',text:'테스트 점수 8',explicitValue:'8'}],
  });
  expect(parsed.facts[0]).toMatchObject({requiredFragments:[],questionCounts:[],deadlines:[],stageClaims:[]});
});

test('intake and get expose assessmentCodeTables and compact view omits authoringHandle/answerLiteral/items',async()=>{
  const obs=observation();
  const numScope={...obs.scope,sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001'};
  const numObs={...obs,scope:numScope,target:{...obs.target,sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001'},previousTarget:obs.previousTarget?{...obs.previousTarget,sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001'}:null,toneSamples:obs.toneSamples.map(s=>({...s,target:{...s.target,sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001'}}))};
  const b={
    ...backend(()=>numObs),
    teacherScope:()=>({sourceNamespace:'academy.lms.dayrecord' as const,teacherId:'99001'}),
    searchStudentsByName:async(name:string)=>[{studentKey:'77001',studentName:name}],
    readItemAnswers:async(labels:readonly string[])=>({
      family:'item_answer',status:'observed',requestedAnswerLabels:labels,pendingAnswerLabels:[],answerBlockedReason:null,
      itemAnswers:labels.map(l=>({identity:{sourceNamespace:'academy_item',lectureKey:l,outerLectureKey:l,sourceId:l},sourceUnitName:'Unit 1',sourcePatternName:'Pattern A',answerLiteral:'1|_|42',answerAvailability:'literal_payload_not_rendered',answerMeaning:'source_official_item_answer_not_student_response',rendering:'not_qualified',paperMembership:'unqualified'})),
      coverage:'exact_requested_shared_answers_not_printed_versions',observedAt:new Date(clock).toISOString(),nativePrintInvoked:false,automaticRetryAllowed:false,
    }),
    readTextbookSectionsByName:async()=>({
      student:{studentId:'student-synthetic',displayName:'Synthetic Student'},courseKey:'5',bookNo:'7',
      records:[{sourceTitle:'Sec 1',score:100,correctCount:1,itemCount:1,items:[{ordinal:'1',lectureKey:'101',outerLectureKey:'101',scoring:'O' as const}],sourceDate:{literal:'2040-01-12',precision:'day',meaning:'source_apply_date_not_exact_grade_time'}}],
      observedAt:new Date(clock).toISOString(),coverage:'exact_selected_existing_records_not_whole_book',
    }),
  };
  const service=new DailyRecordService(b,safeguard,()=>clock,learningPolicy);
  const intake=await service.intake({context:context()});
  expect(intake.assessmentCodeTables).toEqual(ASSESSMENT_CODE_TABLES);
  const fullGet=await service.get({displayName:'Synthetic Student',lessonDate:'2040-01-16',purpose:'check',currentRequestRef:'req-1'});
  expect(fullGet).toMatchObject({status:'observed',assessmentCodeTables:ASSESSMENT_CODE_TABLES});
  expect('authoringHandle' in fullGet).toBe(true);
  const compactGet=await service.get({displayName:'Synthetic Student',lessonDate:'2040-01-16',purpose:'check',currentRequestRef:'req-1',view:'compact'});
  expect(compactGet).toMatchObject({status:'observed',assessmentCodeTables:ASSESSMENT_CODE_TABLES});
  expect('authoringHandle' in compactGet).toBe(false);
  const compactAns=await service.itemAnswers({labels:['101'],purpose:'check',currentRequestRef:'req-1',view:'compact'});
  expect(compactAns.status).toBe('observed');
  expect('answerLiteral' in (compactAns as any).answers[0]).toBe(false);
  expect((compactAns as any).answers[0].answerPayloadCandidate).toBe('42');
  const compactSec=await service.textbookSectionsByName({displayName:'Synthetic Student',purpose:'check',currentRequestRef:'req-1',sourceTitles:['Sec 1'],view:'compact'});
  expect(compactSec.status).toBe('observed');
  expect('items' in (compactSec as any).records[0]).toBe(false);
  expect((compactSec as any).records[0].score).toBe(100);
});

test('learning_render_mismatch row includes expectedActivityFormat hint',async()=>{
  const strictPolicy={...learningPolicy,review:()=>['learning_render_mismatch']};
  const service=new DailyRecordService(backend(),safeguard,()=>clock,strictPolicy),c=context();
  const intake=await service.intake({context:c});
  const rev=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[proposal('progress','- 지정 문제 12문항 풀이 진행.')]});
  expect(rev.status).toBe('held');
  expect(rev.rows[0].blockers).toContain('learning_render_mismatch');
  expect((rev.rows[0] as any).expectedActivityFormat).toBe('suffix_only_without_unit_title');
});

test('a save whose text differs from the reviewed text only in line endings is held with sanitized mismatchPaths',async()=>{
  const dir=await mkdtemp(resolve(tmpdir(),'synthetic-workflow-norm-'));
  try{
    const store=new DailyRecordWorkflowStore(dir);
    let saved=0;
    const writeBackend={
      saveConfigured:()=>false,
      requestBoundSaveConfigured:()=>true,
      saveDailyRecord:async(_scope:any,patches:any[])=>{saved+=patches.length;return patches.map(p=>({field:p.field,status:'persisted' as const,dispatched:true}));},
    };
    const service=new DailyRecordService(backend(),safeguard,()=>clock,learningPolicy,writeBackend,{store});
    const c=context();
    c.facts=[fact('progress','첫째 줄\n둘째 줄\n지정 문제 12문항 풀이 진행.')];
    const intake=await service.intake({context:c});
    const p={...proposal('progress','- 지정 문제 12문항 풀이 진행 (보충).'),decision:{kind:'replace' as const,requestRef:c.currentRequestRef}};
    const rev=await service.review({context:c,observationHandle:intake.observationHandle!,proposals:[p]});
    expect(rev.status).toBe('prepared');
    // The digest covers the exact submitted input: other line endings are a different text and are held.
    for(const text of ['첫째 줄\\n둘째 줄\\r\\n지정 문제 12문항 풀이 진행.','첫째 줄\r\n둘째 줄\r\n지정 문제 12문항 풀이 진행.']){
      const held=await service.save({
        context:{...c,facts:[fact('progress',text)]},
        observationHandle:intake.observationHandle!,
        reviewDigest:rev.reviewDigest,
        proposals:[p],
        request:{requestRef:c.currentRequestRef,instruction:{text:'save progress'}},
      });
      expect(held).toMatchObject({status:'held',reasonCode:'prior_review_record_mismatch',mismatchPaths:['inputDigest'],academyWrites:0});
    }
    expect(saved).toBe(0);

    // Real mismatch returns sanitized mismatchPaths and lengths without raw text.
    const cMismatch={...c,facts:[fact('progress','완전히 다른 내용 12문항')]};
    const bad=await service.save({
      context:cMismatch,
      observationHandle:intake.observationHandle!,
      reviewDigest:rev.reviewDigest,
      proposals:[p],
      request:{requestRef:c.currentRequestRef,instruction:{text:'save progress'}},
    });
    expect(bad).toMatchObject({status:'held',reasonCode:'prior_review_record_mismatch',mismatchPaths:['inputDigest']});
    expect(typeof (bad as any).expectedLen).toBe('number');
    expect(typeof (bad as any).actualLen).toBe('number');
    expect(JSON.stringify({reasonCode:(bad as any).reasonCode,mismatchPaths:(bad as any).mismatchPaths,expectedLen:(bad as any).expectedLen,actualLen:(bad as any).actualLen})).not.toContain('완전히 다른 내용');
  }finally{await rm(dir,{recursive:true,force:true});}
});

// Digests the base server (b7462e6) computed for these exact inputs, with this
// fixture. A review recorded by that server must still be saved after an upgrade.
const baseReviewVectors=[
  {name:'fact text with CRLF',fact:'첫째 줄\r\n지정 문제 12문항 풀이 진행.',value:'- 지정 문제 12문항 풀이 진행 (보충).',
    inputDigest:'84f3ff10c0711adb934b410e4587b8dded485d829b3268db164a81e247e14186',userContextDigest:'95ddaebd39f58221cf0e5bfeccdd9db8571edfa901b995562310ea422308faff',bodyDigest:'fc9fd68dd04025fbdf056317247bbde591eb72a4f4d1dace15391dfda69b8e0f'},
  {name:'fact text with a literal backslash-n',fact:'첫째 줄\\n지정 문제 12문항 풀이 진행.',value:'- 지정 문제 12문항 풀이 진행 (보충).',
    inputDigest:'d3c2a37dc923be9d84704619debb1ce78490170d046c71ba6441e798df279f0c',userContextDigest:'1daa4e2b61c56e2a9460a4be5a5c68b6d692b0f51b4727f1e6fa4bfe2e0bba9a',bodyDigest:'dbcb2ea0771d2bfd82af3bab0d0c00eb5e94759e01f366f35ea3bd8dea723c6c'},
  {name:'proposal value with a literal backslash-n',fact:'지정 문제 12문항 풀이 진행.',value:'- 지정 문제 12문항 풀이 진행\\n(보충).',
    inputDigest:'833b834f2ed08c7152e308eeb9990871259b9174ee4460f738155c68a94e9738',userContextDigest:'cd8677dcb0b9e4331f2064a7610be190b8eb91efa355c55461a71c139735346b',bodyDigest:'d5cbf767ff8b41b56f9dc064fdc71b49aa1699ac3a2cb34db05ef9610c37bbe9'},
];
const reviewBodyKeys=['observationHandle','inputDigest','scope','requestRef','userContextDigest','parentSafeguard','cismSource','learningPolicy','sourceRevision','wireDigest','transmissionState','rows','patch'];
for(const vector of baseReviewVectors)test(`a review recorded by the base server is saved after the upgrade: ${vector.name}`,async()=>{
  const dir=await mkdtemp(resolve(tmpdir(),'synthetic-workflow-upgrade-'));
  try{
    // The store keeps the record exactly as the base server wrote it: its inputDigest, no inputLength.
    class BaseRecordStore extends DailyRecordWorkflowStore{
      override publishOnce(kind:Parameters<DailyRecordWorkflowStore['publishOnce']>[0],id:string,record:unknown){
        if(kind!=='review')return super.publishOnce(kind,id,record);
        const {inputLength:_omitted,...rest}=record as Record<string,unknown>;
        return super.publishOnce(kind,id,{...rest,inputDigest:vector.inputDigest});
      }
    }
    let writes=0;
    const writer={saveConfigured:()=>false,requestBoundSaveConfigured:()=>true,
      saveDailyRecord:async(_scope:any,patches:any[])=>{writes+=patches.length;return patches.map(p=>({field:p.field,status:'persisted' as const,dispatched:true}));}};
    const service=new DailyRecordService(backend(),safeguard,()=>clock,learningPolicy,writer,{store:new BaseRecordStore(dir)});
    const c=context();c.facts=[fact('progress',vector.fact)];
    const p=proposal('progress',vector.value);
    const intake=await service.intake({context:c});
    const rev=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[p]}) as Record<string,any>;
    expect(rev.status).toBe('prepared');
    const result=await service.save({context:c,observationHandle:intake.observationHandle!,reviewDigest:rev.reviewDigest,proposals:[p],
      request:{requestRef:c.currentRequestRef,instruction:{text:'save progress'}}});
    expect(result).toMatchObject({status:'saved',academyWrites:1});
    expect(writes).toBe(1);
    expect({inputDigest:rev.inputDigest,userContextDigest:rev.userContextDigest}).toEqual({inputDigest:vector.inputDigest,userContextDigest:vector.userContextDigest});
    // Every reviewed fact except the random observation handle digests as at the base.
    expect(createHash('sha256').update(JSON.stringify(Object.fromEntries(reviewBodyKeys.map(k=>[k,k==='observationHandle'?null:rev[k]])))).digest('hex')).toBe(vector.bodyDigest);
  }finally{await rm(dir,{recursive:true,force:true});}
});
