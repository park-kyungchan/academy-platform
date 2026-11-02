import {test,expect} from 'bun:test';import {mkdtemp,rm,readFile,readdir,symlink} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';
import {registerCismWeeklyTools,cismReviewInput,cismSaveInput} from '../src/cism-weekly-tools.ts';
import {cismHash} from '../../../../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol';
const field={fieldRef:'1'.repeat(64),kind:'narrative' as const,label:'Fictional section',value:'',studentName:null,groupName:null,source:{fieldName:'s_2'}};
function report(){return {date:'2040-10-15',periodStart:'2040-10-14',periodEnd:'2040-10-20',weekLabel:'Fictional week',teacherLabel:'Fictional Teacher',teacherKey:'90001',wireSha256:'2'.repeat(64),reportSha256:'3'.repeat(64),observedAt:'2040-10-15T00:00:00Z',fields:[{...field}],sourceMode:'live_source' as const,pdfGenerated:false as const};}
const input={date:'2040-10-15',expectedReportSha256:'3'.repeat(64),request:{requestRef:'fictional-request',instruction:'Fill this exact fictional weekly field.'},changes:[{fieldRef:field.fieldRef,before:'',after:'Fictional supported guidance.',evidenceRefs:['dailyrecord:2040-10-15:fictional']}]};
async function harness(fail=false){const root=await mkdtemp(join(tmpdir(),'cism-tools-')),tools=new Map<string,any>();let current=report(),writes=0;const backend={readCismReport:async()=>structuredClone(current),applyCismField:async(action:any,authorize:()=>Promise<boolean>)=>{const prior=(await readdir(root)).filter(n=>n.endsWith('.dispatch.json')).length;if(!await authorize())throw Error('cism_review_required');expect((await readdir(root)).filter(n=>n.endsWith('.dispatch.json')).length).toBe(prior+1);writes++;if(fail)throw Error('private upstream payload');current.fields[0]!.value=action.after;current.reportSha256=cismHash(current.fields);return {status:'persisted' as const,fieldRef:action.fieldRef,report:structuredClone(current)};}};
 registerCismWeeklyTools({registerTool:(n:string,c:any,h:any)=>tools.set(n,{c,h})} as any,backend,root);
 const call=async(n:string,v:any)=>{const t=tools.get(n);return (await t.h(t.c.inputSchema.parse(v))).structuredContent;};return {root,tools,call,set:(x:any)=>current=x,writes:()=>writes};}
test('CISM records review, durably dispatches before POST and verifies exact source readback; duplicate save never writes twice',async()=>{const h=await harness();try{const r=await h.call('academy_cism_weekly_review',input);expect(r.status).toBe('reviewed');expect(h.writes()).toBe(0);const s=await h.call('academy_cism_weekly_save',{...input,reviewDigest:r.reviewDigest});expect(s.status).toBe('persisted');expect(s.pdfGenerated).toBe(false);expect(h.writes()).toBe(1);expect((await readdir(h.root)).some(n=>n.endsWith('.persisted.json'))).toBe(true);expect((await h.call('academy_cism_weekly_save',{...input,reviewDigest:r.reviewDigest})).status).toBe('failed');expect(h.writes()).toBe(1);}finally{await rm(h.root,{recursive:true,force:true});}});
test('CISM preserves concurrent edits and existing nonempty fields, binds exact review input and rejects duplicate fields',async()=>{const h=await harness();try{const r=await h.call('academy_cism_weekly_review',input);expect((await h.call('academy_cism_weekly_save',{...input,reviewDigest:r.reviewDigest,changes:[{...input.changes[0],after:'Different'}]})).code).toBe('cism_prior_review_mismatch');const changed=report();changed.reportSha256='4'.repeat(64);h.set(changed);expect((await h.call('academy_cism_weekly_save',{...input,reviewDigest:r.reviewDigest})).code).toBe('cism_report_drift');h.set({...report(),fields:[{...field,value:'Existing source'}]});expect((await h.call('academy_cism_weekly_review',{...input,changes:[{...input.changes[0],before:'Existing source'}]})).code).toBe('cism_existing_value_preserved');h.set(report());expect((await h.call('academy_cism_weekly_review',{...input,changes:[...input.changes,...input.changes]})).code).toBe('cism_duplicate_field');expect(h.writes()).toBe(0);}finally{await rm(h.root,{recursive:true,force:true});}});
test('uncertain dispatch blocks a new request, failure payload stays private, read-only tools cannot generate PDF',async()=>{const h=await harness(true);try{const r=await h.call('academy_cism_weekly_review',input);const failed=await h.call('academy_cism_weekly_save',{...input,reviewDigest:r.reviewDigest});expect(failed).toMatchObject({status:'failed',automaticRetryAllowed:false});expect(JSON.stringify(failed)).not.toContain('private upstream');const next={...input,request:{...input.request,requestRef:'new-request'}},review=await h.call('academy_cism_weekly_review',next);expect((await h.call('academy_cism_weekly_save',{...next,reviewDigest:review.reviewDigest})).code).toBe('cism_uncertain_dispatch');expect(h.writes()).toBe(1);expect((await h.call('academy_cism_weekly_get',{date:input.date})).authoringPolicy.audience).toBe('deputy_director');}finally{await rm(h.root,{recursive:true,force:true});}});
test('CISM schemas reject arbitrary URLs, credentials, missing evidence and unknown report inputs',()=>{for(const value of [{...input,url:'https://unrelated.invalid'},{...input,changes:[{...input.changes[0],evidenceRefs:[]}]}])expect(()=>cismReviewInput.parse(value)).toThrow();expect(()=>cismSaveInput.parse(input)).toThrow();});
test('explicit narrative completion retains before value while numeric values stay protected',async()=>{const h=await harness();try{const current=report();current.fields[0]!.value='Existing incomplete';h.set(current);const proposed={...input,changes:[{...input.changes[0],before:'Existing incomplete',edit:'complete_existing_narrative'}]};const review=await h.call('academy_cism_weekly_review',proposed);expect(review.status).toBe('reviewed');expect(review.changes[0].before).toBe('Existing incomplete');const metric={...report(),fields:[{...field,kind:'think_note',value:'80',source:{fieldName:'think_note',studentKey:'90002',membershipKey:'90003',groupKey:'1'}}]};h.set(metric);expect((await h.call('academy_cism_weekly_review',{...proposed,changes:[{...proposed.changes[0],before:'80',after:'90'}]})).code).toBe('cism_existing_value_preserved');}finally{await rm(h.root,{recursive:true,force:true});}});
test('an uncertain dispatch is reconciled by source readback of the intended value without another POST',async()=>{const h=await harness(true);try{const r=await h.call('academy_cism_weekly_review',input);await h.call('academy_cism_weekly_save',{...input,reviewDigest:r.reviewDigest});const native=report();native.fields[0]!.value=input.changes[0]!.after;native.reportSha256=cismHash(native.fields);h.set(native);const read=await h.call('academy_cism_weekly_get',{date:input.date});expect(read.executionReadback[0].status).toBe('persisted');expect(h.writes()).toBe(1);expect((await readdir(h.root)).some(n=>n.endsWith('.persisted.json'))).toBe(true);}finally{await rm(h.root,{recursive:true,force:true});}});

test('a settled weekly field accepts a separately reviewed narrative completion and retains both operation outcomes',async()=>{
 const h=await harness();try{
  const first=await h.call('academy_cism_weekly_review',input);
  expect((await h.call('academy_cism_weekly_save',{...input,reviewDigest:first.reviewDigest})).status).toBe('persisted');
  const observed=await h.call('academy_cism_weekly_get',{date:input.date});
  const completion={...input,expectedReportSha256:observed.report.reportSha256,
   request:{requestRef:'fictional-completion',instruction:'Complete the supported fictional narrative with this exact addition.'},
   changes:[{...input.changes[0],before:input.changes[0]!.after,after:input.changes[0]!.after+' Additional supported guidance.',edit:'complete_existing_narrative'}]};
  const reviewed=await h.call('academy_cism_weekly_review',completion);
  expect(reviewed.status).toBe('reviewed');
  expect((await h.call('academy_cism_weekly_save',{...completion,reviewDigest:reviewed.reviewDigest})).status).toBe('persisted');
  expect(h.writes()).toBe(2);
  const after=await h.call('academy_cism_weekly_get',{date:input.date});
  expect(after.executionReadback).toHaveLength(2);
  expect(after.executionReadback.every((x:any)=>x.status==='persisted')).toBe(true);
  await h.call('academy_cism_weekly_save',{...completion,reviewDigest:reviewed.reviewDigest});
  expect(h.writes()).toBe(2);
 }finally{await rm(h.root,{recursive:true,force:true});}
});
test('legacy field-wide dispatch receipts remain immutable and unresolved ones still fence new completion requests',async()=>{
 const h=await harness();try{
  const key=cismHash([report().teacherKey,report().periodStart,field.fieldRef]);
  const record={fieldRef:field.fieldRef,before:'',after:'Legacy unresolved value',requestRef:'legacy-request',reviewDigest:'a'.repeat(64)};
  const path=join(h.root,key+'.dispatch.json');
  const {writeFile}=await import('node:fs/promises');await writeFile(path,JSON.stringify(record),{mode:0o600});
  const r=await h.call('academy_cism_weekly_review',input);
  expect((await h.call('academy_cism_weekly_save',{...input,reviewDigest:r.reviewDigest})).code).toBe('cism_uncertain_dispatch');
  expect(h.writes()).toBe(0);expect(JSON.parse(await readFile(path,'utf8'))).toEqual(record);
  const current=report();current.fields[0]!.value=record.after;current.reportSha256=cismHash(current.fields);h.set(current);
  const completion={...input,expectedReportSha256:current.reportSha256,changes:[{...input.changes[0],before:record.after,after:'Legacy value with an explicitly requested supported completion.',edit:'complete_existing_narrative'}]};
  const review=await h.call('academy_cism_weekly_review',completion);
  expect((await h.call('academy_cism_weekly_save',{...completion,reviewDigest:review.reviewDigest})).status).toBe('persisted');
  expect(h.writes()).toBe(1);expect(JSON.parse(await readFile(path,'utf8'))).toEqual(record);
 }finally{await rm(h.root,{recursive:true,force:true});}
});

test('one persistent native endpoint serializes concurrent reviewed saves before their fresh before-state checks',async()=>{
 const h=await harness();try{
  const other={...input,request:{requestRef:'competing-request',instruction:'Use this other exact supported narrative.'},changes:[{...input.changes[0],after:'A competing supported narrative.'}]};
  const first=await h.call('academy_cism_weekly_review',input),second=await h.call('academy_cism_weekly_review',other);
  const outcomes=await Promise.all([h.call('academy_cism_weekly_save',{...input,reviewDigest:first.reviewDigest}),h.call('academy_cism_weekly_save',{...other,reviewDigest:second.reviewDigest})]);
  expect(outcomes.filter(x=>x.status==='persisted')).toHaveLength(1);
  expect(outcomes.filter(x=>x.code==='cism_report_drift')).toHaveLength(1);expect(h.writes()).toBe(1);
 }finally{await rm(h.root,{recursive:true,force:true});}
});
