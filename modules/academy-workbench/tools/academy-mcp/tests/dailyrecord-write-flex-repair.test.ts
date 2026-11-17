import {expect,test} from 'bun:test';
import {readdirSync,readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {registerDailyRecordTools} from '../src/dailyrecord-tools.ts';
import {backend,clock,fact,observation,safeguard} from './dailyrecord-fixture.ts';

// TASK-FLEX-1R. Fictional records only. A supplied fact that contradicts a
// code value holds the row; no fact for a code field is only a warning.

type CodeField='attendance'|'dailyTestAssessment'|'previousHomeworkAssessment';
const CONTRADICTION='explicit_user_source_option_contradicts_value',CONFLICT='conflicting_explicit_user_source_options',UNSUPPORTED='explicit_user_source_option_fact_required';
const codes=(row:any)=>(row.warnings as {code:string;text:string}[]).map(w=>w.code);
const explicitFact=(field:CodeField,explicitValue:string,ref='user-fact:'+field)=>({...fact(field,'선생님 확인 값'),ref,explicitValue});

async function handleWith(field:CodeField,before:string){
  const read=()=>{const o=observation();o.fields[field]={state:'known',value:before,evidenceRefs:['synthetic-source-evidence']};return o;};
  const service=new DailyRecordService(backend(read),safeguard,()=>clock);
  const intake:any=await service.intake({context:{scope:observation().scope}});
  expect(intake.observationHandle).toBeString();
  return {service,handle:intake.observationHandle as string};
}

test('repair check 1: attendance Y against a supplied fact N is held on that row; no fact warns; a fact Y is clean',async()=>{
  const {service,handle}=await handleWith('attendance','');
  const contradicted:any=await service.review({observationHandle:handle,context:{facts:[explicitFact('attendance','N')]},proposals:[{field:'attendance',value:'Y'}]});
  expect(contradicted.status).toBe('held');
  expect(contradicted.rows[0].blockers).toContain(CONTRADICTION);
  expect(contradicted.rows[0].holdActions).toContainEqual(expect.objectContaining({code:CONTRADICTION,action:'supply_or_correct_fact'}));
  expect(contradicted.patch).toEqual([]);

  const none:any=await service.review({observationHandle:handle,proposals:[{field:'attendance',value:'Y'}]});
  expect(none.status).toBe('prepared');expect(none.rows[0].blockers).toEqual([]);
  expect(codes(none.rows[0])).toContain(UNSUPPORTED);
  expect(none.patch).toEqual([expect.objectContaining({field:'attendance',after:'Y'})]);

  const supported:any=await service.review({observationHandle:handle,context:{facts:[explicitFact('attendance','Y')]},proposals:[{field:'attendance',value:'Y'}]});
  expect(supported.status).toBe('prepared');expect(supported.rows[0].blockers).toEqual([]);
  expect(codes(supported.rows[0])).not.toContain(UNSUPPORTED);
  expect(codes(supported.rows[0])).not.toContain(CONTRADICTION);
});

test('repair check 1: the contradiction holds previousHomeworkAssessment too, and blocks the supplied value at intake',async()=>{
  const {service,handle}=await handleWith('previousHomeworkAssessment','-1');
  const review:any=await service.review({observationHandle:handle,context:{facts:[explicitFact('previousHomeworkAssessment','3')]},proposals:[{field:'previousHomeworkAssessment',value:'5'}]});
  expect(review.status).toBe('held');expect(review.rows[0].blockers).toContain(CONTRADICTION);
  const intake:any=await service.intake({context:{scope:observation().scope,facts:[explicitFact('attendance','N')]},suppliedValues:[{field:'attendance',value:'Y'}]});
  expect(intake.blockers).toContain('attendance:'+CONTRADICTION);
});

test('repair check 2: two supplied facts with different explicit dailyTestAssessment values hold the row',async()=>{
  const {service,handle}=await handleWith('dailyTestAssessment','-1');
  const facts=[explicitFact('dailyTestAssessment','8','user-fact:test-a'),explicitFact('dailyTestAssessment','7','user-fact:test-b')];
  const review:any=await service.review({observationHandle:handle,context:{facts},proposals:[{field:'dailyTestAssessment',value:'8'}]});
  expect(review.status).toBe('held');
  expect(review.rows[0].blockers).toContain(CONFLICT);
  expect(review.patch).toEqual([]);
  // One agreeing fact alone is clean.
  const agreed:any=await service.review({observationHandle:handle,context:{facts:[facts[0]!]},proposals:[{field:'dailyTestAssessment',value:'8'}]});
  expect(agreed.status).toBe('prepared');expect(agreed.rows[0].blockers).toEqual([]);
});

test('repair check 3: the Blocking 1 statements are gone from docs and the handle is named authoringHandle.observationHandle',async()=>{
  const root=resolve(import.meta.dir,'..');
  const text=[...readdirSync(resolve(root,'docs')).filter(f=>f.endsWith('.md')).map(f=>resolve(root,'docs',f)),resolve(root,'README.md')]
    .map(path=>readFileSync(path,'utf8')).join('\n').replace(/\s+/g,' ');
  expect([
    'Every tool call requires `academy.dailyrecord-user-context/1`',
    '`not_supplied` yields a current memo interview',
    'Today memo questions apply independently of selected write fields',
    'additionally require a referenced `user_supplied` fact with the exact typed `explicitValue`',
    'Declared incomplete/unconfirmed stage claims cannot justify a new completion phrase',
    'retain their complete current-context requirements and explicit source-option rules',
    'Every call requires current user purpose',
  ].filter(stale=>text.includes(stale))).toEqual([]);

  const descriptions=new Map<string,string>();
  registerDailyRecordTools({registerTool:(name:string,config:{description:string})=>{descriptions.set(name,config.description);}} as any,new DailyRecordService());
  for(const tool of ['academy_dailyrecord_review','academy_dailyrecord_intake'])expect(descriptions.get(tool)).toContain('authoringHandle.observationHandle');
  expect(descriptions.get('academy_dailyrecord_review')).not.toContain('(authoringHandle)');
});
