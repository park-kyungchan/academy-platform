import {mkdir,readFile,writeFile,lstat,realpath,open,readdir} from 'node:fs/promises';
import {constants} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {attributionSchema,observeAttribution} from './dailyrecord-workflow.ts';
import {z} from 'zod';
import type {McpServer} from '@modelcontextprotocol/server';
import {cismHash,cismMonthDates,cismWeek,cismWriteParameters,type CismReport} from '../../../../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol';
import {publicReadFailure} from './read-failure.ts';
import {toolResult} from './tool-result.ts';
import {academyRoutingDescription} from './routing-context.ts';
import {academyDateInputSchema,dateRefusalCode,resolveAcademyDate} from './read-input.ts';
const date=z.iso.date(),hash=z.string().regex(/^[a-f0-9]{64}$/);
export const cismGetInput=z.object({date:academyDateInputSchema}).strict();
export const cismMonthInput=z.object({month:z.string().regex(/^20\d{2}-(0[1-9]|1[0-2])$/)}).strict();
export const cismReviewInput=z.object({date,expectedReportSha256:hash,request:z.object({requestRef:z.string().min(1).max(200),instruction:z.string().trim().min(1).max(8000)}).strict(),changes:z.array(z.object({fieldRef:hash,before:z.string().max(6000),after:z.string().trim().min(1).max(6000),edit:z.enum(['fill_empty','complete_existing_narrative']).default('fill_empty'),evidenceRefs:z.array(z.string().min(1).max(400)).min(1).max(30)}).strict()).min(1).max(80),attribution:attributionSchema.optional()}).strict();
export const cismSaveInput=cismReviewInput.extend({reviewDigest:hash}).strict();
export interface CismBackend{readCismReport(date:string):Promise<CismReport>;applyCismField(input:{date:string;fieldRef:string;before:string;after:string;wireSha256:string;completeExistingNarrative?:boolean},authorize:()=>Promise<boolean>):Promise<{status:'persisted';fieldRef:string;report:CismReport}>;}
const policy={audience:'deputy_director',style:'mixed_by_field_concise_factual_bullets_and_polite_explanatory_prose',basis:'same_week_dailyrecord_and_student_records',unknowns:'do_not_infer_numeric_scores_class_atmosphere_absence_or_completion',pdfGeneration:'user_action_only'};
const safeCodes=new Set([dateRefusalCode,'cism_date_rejected','cism_month_rejected','cism_document_rejected','cism_scope_conflict','cism_wire_drift','cism_period_conflict','cism_fields_unqualified','cism_value_rejected','cism_stale_before','cism_existing_value_preserved','cism_review_required','cism_ack_unqualified','cism_ack_rejected','cism_readback_mismatch','cism_source_port_unavailable','cism_prior_review_mismatch','cism_duplicate_field','cism_request_rebound','cism_uncertain_dispatch','cism_store_unqualified','cism_report_drift']);
const fail=(e:unknown,write=false)=>toolResult({failure:publicReadFailure(e,'cism_source_failed'),academyWrites:write?null:0,status:'failed',code:e instanceof Error&&safeCodes.has(e.message)?e.message:'cism_source_failed',automaticRetryAllowed:false,dependentWork:'stop_and_diagnose',pdfGenerated:false,parentSends:0});
async function directory(root:string){await mkdir(root,{recursive:true,mode:0o700});const st=await lstat(root);if(!st.isDirectory()||st.isSymbolicLink()||await realpath(root)!==resolve(root)||(st.mode&0o077)!==0||st.uid!==process.getuid?.())throw Error('cism_store_unqualified');}
async function readSafe(path:string){const f=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW);try{const s=await f.stat();if(!s.isFile()||s.nlink!==1||s.size>1000000||(s.mode&0o077)!==0||s.uid!==process.getuid?.())throw Error('cism_store_unqualified');return JSON.parse(await f.readFile('utf8'));}finally{await f.close();}}
async function exclusive(path:string,value:unknown){try{const f=await open(path,'wx',0o600);try{await f.writeFile(JSON.stringify(value)+'\n');await f.sync();}finally{await f.close();}const dir=await open(dirname(path),constants.O_RDONLY);try{await dir.sync();}finally{await dir.close();}return true;}catch(e){if((e as NodeJS.ErrnoException).code==='EEXIST')return false;throw e;}}
export function registerCismWeeklyTools(server:McpServer,backend:CismBackend|null,root:string){
 const readAnnotations={readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true};
 // The host account lock serializes processes; a persistent native endpoint
 // must also serialize its own write requests before either fresh preflight.
 let writeTail:Promise<unknown>=Promise.resolve();
 const register=(name:string,inputSchema:any,description:string,fn:(input:any)=>Promise<unknown>,write=false)=>server.registerTool(name,{description:academyRoutingDescription(name,description),inputSchema,annotations:write?{readOnlyHint:false,destructiveHint:false,idempotentHint:false,openWorldHint:true}:readAnnotations},async (input:any)=>{
  const invoke=async()=>{if(!backend)return toolResult({status:'unavailable',code:'cism_source_backend_not_configured',academyWrites:0,pdfGenerated:false});try{return toolResult(await fn(input));}catch(e){return fail(e,write);}};
  if(!write)return invoke();const result=writeTail.then(invoke,invoke);writeTail=result.then(()=>undefined,()=>undefined);return result;
 });
 const fieldExecutions=async(report:CismReport,fieldRef:string)=>{
  const key=cismHash([report.teacherKey,report.periodStart,fieldRef]);
  // Retain the original field-wide receipt. Successors are separate immutable
  // operations; changing request IDs never bypasses an unresolved predecessor.
  const names=(await readdir(root)).filter(name=>name===key+'.dispatch.json'||new RegExp('^'+key+'\\.[a-f0-9]{64}\\.dispatch\\.json$').test(name));
  return Promise.all(names.map(async name=>({record:await readSafe(resolve(root,name)),done:resolve(root,name.replace(/\.dispatch\.json$/,'.persisted.json'))})));
 };
 const settle=async(report:CismReport,field:CismReport['fields'][number],execution:{record:any;done:string})=>{
  let receipt:any=null;try{receipt=await readSafe(execution.done);}catch(e){if((e as NodeJS.ErrnoException).code!=='ENOENT')throw e;}
  if(receipt){if(receipt.fieldRef!==field.fieldRef||receipt.after!==execution.record.after)throw Error('cism_store_unqualified');return true;}
  if(field.value!==execution.record.after)return false;
  await exclusive(execution.done,{fieldRef:field.fieldRef,after:execution.record.after,observedAt:report.observedAt});return true;
 };
 const executionReadback=async(report:CismReport)=>{
  const states:unknown[]=[];await directory(root);
  for(const field of report.fields){for(const execution of await fieldExecutions(report,field.fieldRef)){
   const persisted=await settle(report,field,execution),record=execution.record;
   states.push({fieldRef:field.fieldRef,status:persisted?'persisted':'uncertain',requestRef:record.requestRef,reviewDigest:record.reviewDigest});
   }
  }return states;
 };
 const verify=(input:z.infer<typeof cismReviewInput>,report:CismReport)=>{
  if(report.reportSha256!==input.expectedReportSha256)throw Error('cism_report_drift');
  if(new Set(input.changes.map(c=>c.fieldRef)).size!==input.changes.length)throw Error('cism_duplicate_field');
  for(const change of input.changes){const f=report.fields.find(f=>f.fieldRef===change.fieldRef);if(!f||f.value!==change.before)throw Error('cism_stale_before');if(f.value.trim()&&(f.kind!=='class_mode'||f.value!=='0')&&!(f.kind==='narrative'&&change.edit==='complete_existing_narrative'))throw Error('cism_existing_value_preserved');cismWriteParameters(report,f,change.after);}
  return cismHash({input,periodStart:report.periodStart,teacherKey:report.teacherKey,wireSha256:report.wireSha256,policy});
 };
 register('academy_cism_weekly_get',cismGetInput,'Read the current owner CISM week and exact editable fieldRefs/values, selected Sunday–Saturday period and internal deputy-director authoring policy. No PDF generation, print, save or send.',async input=>{const date=resolveAcademyDate(input.date,Date.now()),report=await backend!.readCismReport(date);return {status:'observed',date,report,executionReadback:await executionReadback(report),authoringPolicy:policy};});
 register('academy_cism_month',cismMonthInput,'Read each source CISM week intersecting the requested month, including the cross-month first week. Return saved field values and empty-field counts; an empty field is not proof a PDF was never generated or work was incomplete. No writes/PDF/send.',async input=>{const reports:CismReport[]=[];for(const d of cismMonthDates(input.month))reports.push(await backend!.readCismReport(d));return {status:'observed',month:input.month,reports:reports.map(report=>({report,emptyFields:report.fields.filter(f=>!f.value.trim()||f.kind==='class_mode'&&f.value==='0').length})),authoringPolicy:policy,pdfGenerated:false};});
 register('academy_cism_weekly_review',cismReviewInput,'Review exact supported additions and explicitly requested narrative completions against a fresh CISM before-state and record the review in the private owner store. Include verbatim current user instruction and dated DailyRecord/student/teacher evidenceRefs. Content truth requires the source-grounded author; no weekly conclusions or numbers are invented by this tool. CISM uses deputy-director mixed report/prose style, separate from parent DailyRecord. No Academy write, PDF or send.',async raw=>{const {attribution,...input}=cismReviewInput.parse(raw),report=await backend!.readCismReport(input.date),reviewDigest=verify(input,report);await directory(root);await exclusive(resolve(root,reviewDigest+'.review.json'),{input,reviewDigest,periodStart:report.periodStart,teacherKey:report.teacherKey,wireSha256:report.wireSha256,attribution:attribution??observeAttribution({observedBy:'upstream',client:server.server?.getClientVersion?.()??null,bridgeSha256:null,declared:null})});return {status:'reviewed',reviewDigest,periodStart:report.periodStart,periodEnd:report.periodEnd,changes:input.changes,authoringPolicy:policy,academyWrites:0,pdfGenerated:false};});
 register('academy_cism_weekly_save',cismSaveInput,'Save the exact prior-reviewed additions/narrative completions under the existing shared owner account lock. Requires unchanged reviewDigest/input and fresh before-state; records each dispatch before its single POST, verifies native readback, and stops on failure. An uncertain dispatch blocks all later requests for that exact weekly field; never replay or automatically retry. Already-saved content is preserved. No PDF/report generation, printing or parent transmission.',async raw=>{
  const parsed=cismSaveInput.parse(raw),{reviewDigest,attribution,...input}=parsed;await directory(root);const saved=await readSafe(resolve(root,reviewDigest+'.review.json'));if(saved.reviewDigest!==reviewDigest||JSON.stringify(saved.input)!==JSON.stringify(input))throw Error('cism_prior_review_mismatch');
  let current=await backend!.readCismReport(input.date);if(verify(input,current)!==reviewDigest)throw Error('cism_prior_review_mismatch');
  const requestKey=cismHash([current.teacherKey,input.request.requestRef]),requestPath=resolve(root,requestKey+'.request.json');if(!await exclusive(requestPath,input.request)&&JSON.stringify(await readSafe(requestPath))!==JSON.stringify(input.request))throw Error('cism_request_rebound');
  const results:unknown[]=[];
  for(const change of input.changes){
   const key=cismHash([current.teacherKey,current.periodStart,change.fieldRef]),operation=key+'.'+reviewDigest,dispatch=resolve(root,operation+'.dispatch.json'),done=resolve(root,operation+'.persisted.json');
   const previous=await fieldExecutions(current,change.fieldRef);
   for(const execution of previous){
    const field=current.fields.find(f=>f.fieldRef===change.fieldRef)!;
    if(!await settle(current,field,execution))throw Error('cism_uncertain_dispatch');
    if(execution.record.requestRef===input.request.requestRef)throw Error('cism_request_rebound');
   }
   let issued=false;
   const outcome=await backend!.applyCismField({date:input.date,fieldRef:change.fieldRef,before:change.before,after:change.after,wireSha256:current.wireSha256,completeExistingNarrative:change.edit==='complete_existing_narrative'},async()=>{
    if(issued)return false;issued=true;
    return exclusive(dispatch,{fieldRef:change.fieldRef,after:change.after,before:change.before,requestRef:input.request.requestRef,reviewDigest,attribution:attribution??observeAttribution({observedBy:'upstream',client:server.server?.getClientVersion?.()??null,bridgeSha256:null,declared:null}),recordedAt:new Date().toISOString()});
   });
   if(!issued||outcome.status!=='persisted'||outcome.fieldRef!==change.fieldRef||outcome.report.fields.find(f=>f.fieldRef===change.fieldRef)?.value!==change.after)throw Error('cism_readback_mismatch');
   await exclusive(done,{fieldRef:change.fieldRef,after:change.after,observedAt:outcome.report.observedAt});
   results.push(outcome);current=outcome.report;
  }
  return {status:'persisted',academyWrites:results.length,results,pdfGenerated:false,parentSends:0};
 },true);
}
