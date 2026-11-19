import {expect,test} from 'bun:test';
import {resolve} from 'node:path';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {JOURNAL_HTTP_SOURCE_CONTRACT} from '../../../../../apps/academy/dc-gang-a/src/lms/lessonJournalHttp';
import {DailyRecordService,dailyRecordIntakeSchema,dailyRecordReviewSchema,dailyRecordSaveSchema,dailyRecordReconcileSchema,cismSectionSchema} from '../src/dailyrecord.ts';
import {ASSESSMENT_CODE_LABELS,ASSESSMENT_CODE_TABLES,dailyRecordFields} from '../src/dailyrecord-context.ts';
import {readDailyRecordByName} from '../src/dailyrecord-read.ts';
import {backend,clock,observation,safeguard,scope,context as fixtureContext} from './dailyrecord-fixture.ts';

// Fictional fixtures only; no test contacts the Academy server.
const service=(read=observation)=>new DailyRecordService(backend(read),safeguard,()=>clock,null);

/** The 11-name flat form of the recorded intake call (ERR-20261118-002). */
const flatIntake=()=>({schemaVersion:'academy.dailyrecord-user-context/1',purpose:'Prepare one fictional lesson record.',currentRequestRef:'flat-request-1',
  sourceNamespace:scope.sourceNamespace,teacherId:scope.teacherId,studentId:scope.studentId,lessonDate:scope.lessonDate,
  selectedFields:['progress','homework'],memoAnswer:{kind:'no_addition'},facts:[],suppliedValues:[]});

test('intake takes the flat members and the written-out scope, and answers as the nested form does',async()=>{
  const flat=flatIntake();expect(Object.keys(flat)).toHaveLength(11);
  const {sourceNamespace,teacherId,studentId,lessonDate,suppliedValues,...rest}=flat;
  const nested={context:{...rest,scope:{sourceNamespace,teacherId,studentId,lessonDate}},suppliedValues};
  const viaFlat=await service().intake(flat),viaNested=await service().intake(nested);
  expect(viaFlat.userContext).toEqual({...viaNested.userContext});
  expect(viaFlat.userContext).toMatchObject({currentRequestRef:'flat-request-1',scope,selectedFields:['progress','homework'],memoAnswer:{kind:'no_addition',requestRef:'flat-request-1'}});
  expect(viaFlat.sourceAvailability).toBe('observed');
  // The flat `scope` object and a partly flat context both work.
  const mixed=await service().intake({scope:{...scope},context:{purpose:'mixed',selectedFields:['progress']}});
  expect(mixed.userContext).toMatchObject({scope,purpose:'mixed',selectedFields:['progress']});
});

test('intake refuses a member given both ways, a scope given twice or in part, and no scope',()=>{
  const flat=flatIntake();
  const refused=(input:unknown)=>{const result=dailyRecordIntakeSchema.safeParse(input);return result.success?[]:result.error.issues.map(i=>i.message);};
  expect(refused({...flat,context:{purpose:'again'}})).toEqual(['context_member_conflict']);
  expect(refused({...flat,context:{scope:{...scope}}})).toEqual(['context_member_conflict']);
  expect(refused({...flat,scope:{...scope}})).toEqual(['context_member_conflict']);
  const {studentId:_,...partial}=flat;
  expect(refused(partial)).toEqual(['context_scope_members_incomplete']);
  expect(refused({purpose:'no scope'})).toEqual(['context_scope_required']);
  expect(refused({context:{purpose:'no scope'}})).toEqual(['context_scope_required']);
  expect(refused({...flat,approved:true}).length).toBeGreaterThan(0);
  expect(refused({scope:{...scope},approved:true}).length).toBeGreaterThan(0);
  expect(refused({scope:{...scope}})).toEqual([]);
});

test('the registered intake schema lets the flat call through over stdio and refuses a conflict by its code',async()=>{
  const client=new Client({name:'synthetic-dailyrecord-items',version:'0.1.0'});
  const transport=new StdioClientTransport({command:process.execPath,args:['--preload',resolve(import.meta.dir,'network-deny.ts'),resolve(import.meta.dir,'../src/dailyrecord-server.ts')],env:{PATH:process.env.PATH??'/usr/bin:/bin'},stderr:'pipe'});
  await client.connect(transport);
  try{
    const flat=await client.callTool({name:'academy_dailyrecord_intake',arguments:flatIntake()});
    expect(flat.isError).not.toBe(true);
    expect(flat.structuredContent).toMatchObject({sourceAvailability:'unavailable',userContext:{scope,currentRequestRef:'flat-request-1'}});
    const conflict=await client.callTool({name:'academy_dailyrecord_intake',arguments:{...flatIntake(),context:{scope:{...scope}}}});
    expect(conflict.structuredContent).toMatchObject({status:'blocked',code:'tool_input_invalid',issues:[{kind:'custom',code:'context_member_conflict'}]});
  }finally{await client.close();await transport.close();}
},20000);

async function review(proposals:{field:string;value:string}[],memoAnswer:unknown){
  const s=service(),intake=await s.intake({context:fixtureContext()});
  return s.review({observationHandle:intake.observationHandle,context:{memoAnswer},proposals});
}
const fiveFields=['progress','homework','attendance','dailyTestAssessment','previousHomeworkAssessment'] as const;
const plain=(field:string,value:string)=>({field,value:['progress','homework','lessonMemo'].includes(field)?'- '+value:value});
const values={progress:'지정 문제 12문항 풀이 진행.',homework:'오답 정리.',attendance:'Y',dailyTestAssessment:'9',previousHomeworkAssessment:'5',lessonMemo:'준비물 안내.'} as const;

test('review takes no_addition without requestRef: five proposals prepare, six hold on the kept memo blocker',async()=>{
  const memoAnswer={kind:'no_addition'};
  const five=await review(fiveFields.map(f=>plain(f,values[f])),memoAnswer);
  expect(five).toMatchObject({status:'prepared'});
  const six=await review([...fiveFields,'lessonMemo' as const].map(f=>plain(f,values[f])),memoAnswer);
  expect(six.status).toBe('held');
  expect(six.rows.flatMap(r=>r.blockers.map(b=>r.field+':'+b))).toContain('lessonMemo:no_addition_preserves_existing_memo');
  expect(six.rows.filter(r=>r.field!=='lessonMemo'&&r.blockers.length)).toEqual([]);
});

test('save and intake take the same memoAnswer; reconcile and the CISM section keep their schemas',async()=>{
  const {context}=dailyRecordIntakeSchema.parse({context:{scope:{...scope},memoAnswer:{kind:'no_addition'}}});
  expect(context?.memoAnswer).toEqual({kind:'no_addition'});
  const memo={context:{memoAnswer:{kind:'no_addition'}}};
  expect(dailyRecordReviewSchema.safeParse({...memo,proposals:[{field:'progress',value:'- x'}]}).success).toBe(true);
  expect(dailyRecordSaveSchema.safeParse({...memo,reviewDigest:'a'.repeat(64)}).success).toBe(true);
  const strict={...fixtureContext(),memoAnswer:{kind:'no_addition'}};
  expect(dailyRecordReconcileSchema.safeParse({context:strict}).success).toBe(false);
  expect(cismSectionSchema.safeParse({context:strict,selection:{reportId:'r',periodStart:'2040-01-08',periodEnd:'2040-01-14',section:'learning_support'}}).success).toBe(false);
  expect(dailyRecordReconcileSchema.safeParse({context:{...strict,memoAnswer:{kind:'no_addition',requestRef:'x'}}}).success).toBe(true);
});

const known=(value:string)=>({state:'known' as const,value,evidenceRefs:['synthetic-source-evidence']});

test('intake labels each known assessment code with the source page label; attendance has none',async()=>{
  const intake=await service().intake({context:fixtureContext()});
  const before=Object.fromEntries(intake.fields.map(f=>[f.field,f.before])) as Record<string,Record<string,unknown>>;
  expect(before.dailyTestAssessment).toMatchObject({state:'known',value:'8',label:'일일 테스트 80점'});
  expect(before.previousHomeworkAssessment).toMatchObject({state:'known',value:'4',label:'숙제 수행도 A'});
  for(const field of ['progress','homework','lessonMemo','attendance'])expect('label' in before[field]!).toBe(false);
  const unconfirmed=observation();unconfirmed.fields.dailyTestAssessment=known('-1');unconfirmed.fields.previousHomeworkAssessment={state:'unknown',reasonCode:'no-rate'};
  const second=await service(()=>unconfirmed).intake({context:fixtureContext()});
  expect(second.fields.find(f=>f.field==='dailyTestAssessment')?.before).toMatchObject({value:'-1',label:'미확인'});
  expect('label' in second.fields.find(f=>f.field==='previousHomeworkAssessment')!.before).toBe(false);
  const odd=observation();odd.fields.dailyTestAssessment=known('constructor');
  expect('label' in (await service(()=>odd).intake({context:fixtureContext()})).fields.find(f=>f.field==='dailyTestAssessment')!.before).toBe(false);
});

test('assessmentCodeLabels maps every code of assessmentCodeTables to the source label, beside the unchanged tables',async()=>{
  const intake=await service().intake({context:fixtureContext()});
  expect(intake.assessmentCodeTables).toEqual(ASSESSMENT_CODE_TABLES);
  expect(Object.keys(intake.assessmentCodeLabels)).toEqual(['dailyTestAssessment','previousHomeworkAssessment']);
  expect(intake.assessmentCodeLabels).toEqual({dailyTestAssessment:JOURNAL_HTTP_SOURCE_CONTRACT.assessmentLabels.daily_test_radio,
    previousHomeworkAssessment:JOURNAL_HTTP_SOURCE_CONTRACT.assessmentLabels.homework_radio});
  for(const field of ['dailyTestAssessment','previousHomeworkAssessment'] as const){
    expect(Object.keys(ASSESSMENT_CODE_LABELS[field]).sort()).toEqual([...ASSESSMENT_CODE_TABLES[field]].sort());
  }
});

/** A get over a fixture backend: the port fakes resolution and delegates intake to a real service. */
function getPort(inputs:unknown[]){
  const s=service();
  return {resolveStudent:async()=>({status:'resolved',studentKey:'77001',displayName:'Synthetic Student',teacherId:'99001'}),
    intake:async(input:unknown)=>{inputs.push(input);
      const asked=input as {context:{scope:Record<string,string>}};
      return s.intake({context:{...asked.context,scope:{...scope}}}).then(r=>({...r,userContext:{...r.userContext,scope:asked.context.scope}}));}};
}
const getInput={displayName:'Synthetic Student',lessonDate:'2040-01-16',purpose:'check',currentRequestRef:'req-1'};

test('get returns the labels and the label table, and projects fields[] after intake without passing fields on',async()=>{
  const inputs:unknown[]=[];
  const full=await readDailyRecordByName(getInput,getPort(inputs),()=>clock) as any;
  expect(full.status).toBe('observed');
  expect(full.fields.map((f:{field:string})=>f.field)).toEqual([...dailyRecordFields]);
  expect(full.fields.find((f:{field:string})=>f.field==='dailyTestAssessment').before.label).toBe('일일 테스트 80점');
  expect(full.assessmentCodeLabels.previousHomeworkAssessment['5']).toBe('숙제 수행도 A⁺');
  expect(full.assessmentCodeTables).toEqual(ASSESSMENT_CODE_TABLES);
  const part=await readDailyRecordByName({...getInput,fields:['dailyTestAssessment','progress'],view:'compact'},getPort(inputs),()=>clock) as any;
  expect(part.fields.map((f:{field:string})=>f.field)).toEqual(['progress','dailyTestAssessment']);
  expect(part.fields[1].before.label).toBe('일일 테스트 80점');
  expect(inputs.every(input=>Object.keys(input as object).join()==='context')).toBe(true);
});

test('intake projects fields[] by the input fields, in source order, and keeps the rest of its answer',async()=>{
  const all=await service().intake({context:fixtureContext()});
  const part=await service().intake({context:fixtureContext(),fields:['previousHomeworkAssessment','attendance']});
  expect(part.fields.map(f=>f.field)).toEqual(['attendance','previousHomeworkAssessment']);
  expect(part.fields).toEqual(all.fields.filter(f=>['attendance','previousHomeworkAssessment'].includes(f.field)));
  expect({...part,fields:[],observationHandle:null}).toEqual({...all,fields:[],observationHandle:null});
  expect(dailyRecordIntakeSchema.safeParse({context:{scope},fields:[]}).success).toBe(false);
  expect(dailyRecordIntakeSchema.safeParse({context:{scope},fields:['nope']}).success).toBe(false);
});
