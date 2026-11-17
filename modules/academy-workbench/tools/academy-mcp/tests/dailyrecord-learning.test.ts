import {expect,test} from 'bun:test';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {withoutRetiredLearningArguments} from '../src/dailyrecord-learning.ts';
import {dailyRecordToolConfiguration} from '../src/dailyrecord-tools.ts';
import {fileURLToPath} from 'node:url';
import {backend,clock,context,fact,observation,proposal,safeguard,learningPolicy} from './dailyrecord-fixture.ts';
import {parentInternalLanguage} from '../../../../../apps/academy/dc-gang-a/src/contracts/parentInternalLanguage.ts';

test('pinned source loader preserves publisher exclusions and algebra allowance',async()=>{
 const policy=fileURLToPath(new URL('../../../../../apps/academy/dc-gang-a/docs/daily-record-authoring-policy.md',import.meta.url));
 const source=fileURLToPath(new URL('../../../../../apps/academy/dc-gang-a/src/contracts/parentInternalLanguage.ts',import.meta.url));
 const sha=(bytes:Buffer)=>createHash('sha256').update(bytes).digest('hex');
 const config=await dailyRecordToolConfiguration(['--dailyrecord-policy',policy,sha(await readFile(policy)),source,sha(await readFile(source))]);
 for(const text of ['- 가우스 1-1-2 (1P6A) 준비','- 가우스 (1U0T) 준비','- 가우스 2Q4Z 준비','Teacher REVIEW'])expect(config.parentSafeguard?.check(text)).toBe(true);
 expect(config.parentSafeguard?.check('- 2x3y의 계산 연습')).toBe(false);
});

test('internal version facts remain accepted but parent memo metadata is blocked',async()=>{
 for(const label of ['(1P6A)','(1U0T)','(2Q4Z)','v3(1P6A)','v 3','v:3','시리즈: 2','(1SAG)','(1U0U)','(1N87)','교재5-1-1v1']){
  const c=context();c.selectedFields=['lessonMemo'];const value='- 가상 교재 '+label+' 준비';
  c.memoAnswer={kind:'content',requestRef:c.currentRequestRef,text:value};c.facts=[fact('lessonMemo',value)];
  const s=new DailyRecordService(backend(),{...safeguard,check:text=>parentInternalLanguage.test(text)},()=>clock);
  const i=await s.intake({context:c});
  const r=await s.review({context:c,observationHandle:i.observationHandle,proposals:[proposal('lessonMemo',value)]});
  expect(r.rows[0].blockers).toContain('parent_internal_language_excluded');expect(r.patch).toEqual([]);
  expect(c.facts[0]!.text).toContain(label);
 }
});

test('bracketed known publisher metadata cannot enter the lessonMemo patch',async()=>{
 const policy=fileURLToPath(new URL('../../../../../apps/academy/dc-gang-a/docs/daily-record-authoring-policy.md',import.meta.url));
 const source=fileURLToPath(new URL('../../../../../apps/academy/dc-gang-a/src/contracts/parentInternalLanguage.ts',import.meta.url));
 const sha=(bytes:Buffer)=>createHash('sha256').update(bytes).digest('hex');
 const config=await dailyRecordToolConfiguration(['--dailyrecord-policy',policy,sha(await readFile(policy)),source,sha(await readFile(source))]);
 for(const value of ['- 가우스 5-1-2 [1P6A] 준비','- 가우스 5-1-2 [1SAG] 준비','- 가우스 5-1-2 {1U0T} 준비','- [1p6a] 준비']){
  const c=context();c.selectedFields=['lessonMemo'];
  c.memoAnswer={kind:'content',requestRef:c.currentRequestRef,text:value};c.facts=[fact('lessonMemo',value)];
  const s=new DailyRecordService(backend(),config.parentSafeguard!,()=>clock);
  const i=await s.intake({context:c});
  const r=await s.review({context:c,observationHandle:i.observationHandle,proposals:[proposal('lessonMemo',value)]});
  expect(r.status).toBe('held');expect(r.rows[0].blockers).toContain('parent_internal_language_excluded');expect(r.patch).toEqual([]);
  expect(c.memoAnswer.text).toBe(value);expect(c.facts[0]!.text).toBe(value);
 }
});

test('ordinary algebra, textbook coordinates and full deadlines remain valid parent text',async()=>{
 for(const value of ['- 단항식 2x3y를 간단히 하기','- x2y3를 간단히 하기','- 다항식 3abc의 계산 연습','- 2x3y의 계산 연습','- 가우스 5-1-2 4-1~4-4 학습','- 2026-11-02까지 12문항 풀기']){
  const c=context();c.selectedFields=['lessonMemo'];c.memoAnswer={kind:'content',requestRef:c.currentRequestRef,text:value};c.facts=[fact('lessonMemo',value)];
  const s=new DailyRecordService(backend(),{...safeguard,check:text=>parentInternalLanguage.test(text)},()=>clock),i=await s.intake({context:c});
  const r=await s.review({context:c,observationHandle:i.observationHandle,proposals:[proposal('lessonMemo',value)]});
  expect(r.rows[0].blockers).not.toContain('parent_internal_language_excluded');expect(r.status).toBe('prepared');
 }
});

test('changed text requires hyphen lines and no catalog mapping; untouched legacy text stays unchanged',async()=>{
 const c=context(),service=new DailyRecordService(backend(),safeguard,()=>clock),intake=await service.intake({context:c});
 const changed=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[{...proposal('progress','x'),value:'1. 지정 문제 12문항 풀이 진행.'}]});
 expect(changed.rows[0].blockers).toEqual(['parent_hyphen_lines_required']);expect(changed.patch).toEqual([]);
 const noop=await service.review({context:c,observationHandle:intake.observationHandle,proposals:[{...proposal('progress','x'),value:observation().fields.progress.state==='known'?(observation().fields.progress as any).value:''}]});
 expect(noop.rows[0].disposition).toBe('noop');expect(noop.patch).toEqual([]);
});

test('non-hyphen historical samples are ignored as tone and ignored learningEntries still bind the review digest',async()=>{
 const o=observation();o.toneSamples.push({...o.toneSamples[0],text:'1. 다른 형식'});
 const s=new DailyRecordService(backend(()=>o),safeguard,()=>clock,learningPolicy),c=context(),i=await s.intake({context:c});
 expect(i.toneSamples).toHaveLength(1);expect(i.toneSamples[0].text).toStartWith('- ');
 const p=proposal('progress','지정 문제 12문항 풀이 진행.'),a=await s.review({context:c,observationHandle:i.observationHandle,proposals:[p]});
 const b=await s.review({context:c,observationHandle:i.observationHandle,proposals:[{...p,learningEntries:[{bookRef:'fictional-book',unitRef:'fictional-unit',scope:'unit',sectionRefs:[],activity:'풀이',deadline:null}]}]});
 expect(a.reviewDigest).not.toBe(b.reviewDigest);
});
test('retired --dailyrecord-learning arguments are removed without reading a file; tools cannot pass a catalog',async()=>{
 const args=['--a','--dailyrecord-learning','/nonexistent/owner.mjs','a'.repeat(64),'/nonexistent/config.json','b'.repeat(64),'--other'];
 expect(withoutRetiredLearningArguments(args).remainingArgs).toEqual(['--a','--other']);
 expect(withoutRetiredLearningArguments(['--other']).remainingArgs).toEqual(['--other']);
 expect(()=>withoutRetiredLearningArguments(['--dailyrecord-learning','/nonexistent/owner.mjs','a'.repeat(64)])).toThrow('dailyrecord_learning_arguments_invalid');
 const service=new DailyRecordService();await expect(service.review({context:context(),catalog:{verified:true},proposals:[proposal('progress','x')]})).rejects.toThrow();
});
