import {readFileSync,writeFileSync,existsSync,appendFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {DailyRecordService} from '../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord.ts';
import {createDailyRecordWorkflow,workflowRequestKey,textDigest} from '../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord-workflow.ts';
import {NativeDailyRecordBackend} from '../../modules/academy-workbench/tools/academy-mcp/src/native-dailyrecord-backend.ts';
import {ownerAdmissionSchema} from '../../modules/academy-workbench/tools/academy-mcp/src/owner-admission.ts';
import {applyJournalHttpAction,journalHttpActionDigest} from '../../apps/academy/dc-gang-a/src/lms/lessonJournalHttp.ts';
import {canonicalActionDigest} from '../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord-save-admission.ts';
export const time='2040-01-04T12:00:00.000Z';
export const scope={sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001',lessonDate:'2040-01-04'};
export const target={studentName:'Invented S10 Student',date:scope.lessonDate,groupId:'3',teacherKey:scope.teacherId,studentKey:scope.studentId,courseKey:'5',recordKey:'13',membershipKey:'8'};
export const fields={progress:'- 기존 진도',homework:'- 기존 숙제',lessonMemo:'',attendance:'Y',dailyTestAssessment:'0',previousHomeworkAssessment:'3'};
export const attribution={schemaVersion:'academy.dailyrecord-attribution/1',observedBy:'upstream',observedAt:time,client:{name:'s10-synthetic-owner',version:'1'},process:{pid:4242,ppid:4241,uid:0,parentCommand:'synthetic-runner',bridgeSha256:null},herdrPaneId:null,declared:{runtime:'synthetic-reference',sessionRef:'s10-fixture'}};
// Explicit fake policy ports. Their identities are inputs, not canonical policy admission.
export const safeguard={policySha256:'3'.repeat(64),safeguardSourceSha256:'4'.repeat(64),sourceRef:'s10:fake-owner-parent-policy',checkLines:(s:string)=>/^- [^\r\n]+(?:\n- [^\r\n]+)*$/.test(s),check:()=>false};
export const learning={bundleSha256:'5'.repeat(64),configSha256:'6'.repeat(64),catalogSha256:'7'.repeat(64),scopeQualified:()=>true,review:()=>[]};
export const json=(p:string)=>JSON.parse(readFileSync(p,'utf8'));
export const put=(p:string,x:unknown)=>writeFileSync(p,JSON.stringify(x)+'\n',{mode:0o600});
export function event(root:string,x:unknown){appendFileSync(resolve(root,'events.jsonl'),JSON.stringify(x)+'\n',{mode:0o600});}
export function initialize(root:string){mkdirSync(root,{recursive:true,mode:0o700});if(!existsSync(resolve(root,'source.json')))put(resolve(root,'source.json'),{fields,previousRecordKey:'12',previousDate:'2040-01-02',mode:'normal'});}
export function observation(state:any,s=scope){const t={...s,courseId:'5',membershipId:'8',recordId:'13'};return {scope:s,target:t,previousTarget:{...t,lessonDate:state.previousDate,recordId:state.previousRecordKey},observedAt:time,sourceRevision:'1'.repeat(64),wireDigest:'a'.repeat(64),fields:Object.fromEntries(Object.keys(fields).map(f=>[f,typeof state.fields[f]==='string'?{state:'known',value:state.fields[f],evidenceRefs:['s10:synthetic-source:'+f]}:{state:'unknown',reasonCode:'s10_fixture_field_not_observed'}])),toneSamples:[],transmissionState:'unknown'};}
export function context(requestRef='s10-request',field='progress',value='- 지정 문제 풀이 진행.',date=scope.lessonDate){return {schemaVersion:'academy.dailyrecord-user-context/1',purpose:'Characterize original TS against an explicitly fictional owner.',currentRequestRef:requestRef,scope:{...scope,lessonDate:date},selectedFields:[field],memoAnswer:{kind:'no_addition',requestRef:'s10-memo-answer'},facts:[{ref:'s10:fact:'+field,field,text:value.replace(/^- /,''),origin:'user_supplied',...(['attendance','dailyTestAssessment','previousHomeworkAssessment'].includes(field)?{explicitValue:value}:{}),requiredFragments:[],questionCounts:[],deadlines:[],stageClaims:[]}]};}
export function proposal(c:any,value:string){return {field:c.selectedFields[0],value,factRefs:[c.facts[0].ref],decision:{kind:'replace',requestRef:c.currentRequestRef}};}
export function service(root:string,clock=Date.parse(time)){
 initialize(root);const file=resolve(root,'source.json'),state=()=>json(file),workflow=createDailyRecordWorkflow({storeDir:resolve(root,'store'),requiredRoot:root+'/'});
 const read={async readDailyRecord(s:any){event(root,{operation:'read_authoritative',scope:s});return {status:'observed' as const,observation:observation(state(),s),subject:{displayName:target.studentName}};}};
 const port={authenticate:async()=>{event(root,{operation:'fake_authenticate',teacherId:scope.teacherId});},discoverDayRecordGroups:async()=>({groups:[{groupId:'3'}]}),
  execute:async(input:any)=>({dayRecords:[{...target,date:input.date,...(input.date===target.date?{}:{recordKey:state().previousRecordKey}),fields:state().fields}]}),
  journalState:async()=>({target,fields:state().fields,previousRecordKey:state().previousRecordKey,previousDate:state().previousDate,wireDigest:'a'.repeat(64)}),
  applyJournalAction:async(action:any,approval:any)=>{
   event(root,{operation:'action',action,canonicalDigest:canonicalActionDigest(action),ownerDigest:journalHttpActionDigest(action)});let reads=0;
   return applyJournalHttpAction(action,{consumeApproval:async a=>{const decision=await approval.consumeApproval(a);event(root,{operation:'consumed_approval',decision});return decision;},
    read:async()=>{reads++;const s=state();if(reads===2&&s.mode==='before-readback')process.exit(74);if(reads===2&&s.mode==='readback-unavailable')throw Error('s10_fake_readback_unavailable');return {target,fields:s.fields,previousRecordKey:s.previousRecordKey,previousDate:s.previousDate,wireDigest:'a'.repeat(64)};},
    write:async a=>{const s=state();if(s.mode==='before-dispatch')process.exit(72);event(root,{operation:'fake_dispatch',field:a.field,effectRecordKey:a.effectRecordKey});if(s.mode==='timeout-old')throw Error('s10_fake_timeout');if(s.mode!=='returned-old'){s.fields[a.field]=a.after;put(file,s);event(root,{operation:'fake_acceptance',field:a.field,effectRecordKey:a.effectRecordKey});}if(s.mode==='after-acceptance')process.exit(73);if(s.mode==='timeout-new')throw Error('s10_fake_timeout');}});
  },generatePdf:()=>event(root,{operation:'pdf'}),print:()=>event(root,{operation:'print'}),sendParent:()=>event(root,{operation:'parent_send'}),close:()=>{}};
 const admission={teacherId:scope.teacherId,teacherLabel:'Invented S10 Teacher',date:scope.lessonDate,students:[{studentId:scope.studentId,displayName:target.studentName}],sourceBundleSha256:'b'.repeat(64)};
 const owner=ownerAdmissionSchema.parse({schemaVersion:'academy.owner-admission/2',decider:'user',decidedAt:'2040-01-04',sourceNamespace:scope.sourceNamespace,teacherId:scope.teacherId,allowedFrom:null,allowedThrough:null,failClosedDefault:'2026-10-23',studentScope:'source_verified'});
 // No configuration loader or credential getter from the native runtime is called.
 const backend=new NativeDailyRecordBackend(port as never,admission,()=>({username:'s10-fixture',password:'s10-fixture'}),owner);
 const svc=new DailyRecordService(read as never,safeguard,()=>clock,learning,backend,workflow);
 return {workflow,backend,svc,state};
}
export async function prepare(root:string,requestRef='s10-request',field='progress',value='- 지정 문제 풀이 진행.',date=scope.lessonDate){
 const {svc,workflow,state}=service(root),c=context(requestRef,field,value,date),handle='dailyrecord-observation:s10-'+requestRef;
 // Literal old-source-compatible observation published by the actual store owner function.
 workflow.store.publishOnce('observation',handle,{observation:observation(state(),c.scope),scope:c.scope,requestRef,subject:{displayName:target.studentName},createdBy:attribution,createdAt:time});
 const proposals=[proposal(c,value)],review=await svc.review({context:c,observationHandle:handle,proposals,attribution});
 if(review.status!=='prepared')throw Error('s10_fixture_review_not_prepared:'+JSON.stringify(review.rows.map((r:any)=>({field:r.field,blockers:r.blockers}))));
 const input={context:c,observationHandle:handle,proposals,reviewDigest:review.reviewDigest,request:{requestRef,instruction:{text:'Save exactly this reviewed fictional S10 field.',sessionRef:'s10-fixture',receivedAt:time}},attribution};
 put(resolve(root,requestRef+'.input.json'),input);return {input,review};
}
