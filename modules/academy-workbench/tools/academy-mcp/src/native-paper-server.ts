import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {z} from 'zod';
import {readPinnedLocalBytes} from './item-label-store.ts';
import {runtimeIdentityConfiguration,registerRuntimeIdentity} from './runtime-identity.ts';
import {nativePaperConfigSchema,paper1HeadersSchema,paper2HeadersSchema,validateNativePaperScope,assertNativePaperArtifacts} from './native-paper-contract.ts';
import {lstatSync,realpathSync} from 'node:fs';
import {dirname} from 'node:path';
import {pathToFileURL} from 'node:url';

const home='/opt/data/profiles/dc-gang-a-dev';
if(process.getuid?.()!==10000||process.env.HERMES_HOME!==home)throw Error('native_owner_required');
const runtimeIdentity=await runtimeIdentityConfiguration(process.argv.slice(2));
const args=runtimeIdentity.remainingArgs;
if(!runtimeIdentity.identity||args.length!==3||!['paper1','paper2'].includes(args[0]!)||runtimeIdentity.identity.manifest.endpoint!==args[0])throw Error('pinned_native_config_required');
const [variant,configPath,configSha]=args;
const stat=lstatSync(configPath!);
if(!configPath!.startsWith(home+'/artifacts/')||realpathSync(dirname(configPath!))!==dirname(configPath!)||!stat.isFile()||stat.isSymbolicLink()||stat.uid!==10000||(stat.mode&0o077)!==0)throw Error('private_owner_config_required');
const period=z.object({from:z.iso.date(),through:z.iso.date()}).strict();
const configBytes=await readPinnedLocalBytes(configPath!,configSha!);
const config=nativePaperConfigSchema.parse(JSON.parse(configBytes.bytes.toString('utf8')));
const ownerPath=home+'/academy/owner.json',jsdomPackage='/opt/data/workspace/dc-gang-a/node_modules/jsdom/package.json',jsdom='/opt/data/workspace/dc-gang-a/node_modules/jsdom/lib/api.js';
assertNativePaperArtifacts(runtimeIdentity.identity.manifest,{variant:variant!,configPath:configPath!,configSha:configSha!,sourcePath:config.sourceBundlePath,sourceSha:config.sourceBundleSha256,ownerPath,dependencyPath:jsdomPackage,dependencyEntryPath:jsdom});
const verifiedBytes=async(path:string,role:'policy'|'dependency')=>{const ref=runtimeIdentity.identity!.manifest.artifacts.find(a=>(a.runtimePath??a.path)===path&&a.role===role);if(!ref)throw Error('release_endpoint_identity_mismatch');return (await readPinnedLocalBytes(path,ref.sha256)).bytes;};
const owner=JSON.parse((await verifiedBytes(ownerPath,'policy')).toString('utf8'));
validateNativePaperScope(config,owner.expectedStudentNames);
const source=await readPinnedLocalBytes(config.sourceBundlePath,config.sourceBundleSha256);
if(JSON.parse((await verifiedBytes(jsdomPackage,'dependency')).toString('utf8')).version!=='30.1.2')throw Error('source_dependency_drift');
await verifiedBytes(jsdom,'dependency');
const code=source.bytes.toString('utf8');if(code.split('from "jsdom"').length!==2)throw Error('source_import_contract_drift');
const module=await import('data:text/javascript;base64,'+Buffer.from(code.replace('from "jsdom"','from '+JSON.stringify(pathToFileURL(jsdom).href))).toString('base64'));
let sourceRequests=0,authenticationAttempts=0;
const client=new module.AcademyHttpClient({teacherLabel:owner.teacherLabel,teacherKey:owner.teacherSourceKey,collegeKey:owner.collegeSourceKey},(url:string,init:RequestInit)=>{sourceRequests++;return fetch(url,init);});
if(typeof client.lookupPrintedPaperMetadata!=='function')throw Error('canonical_printed_paper_lookup_required');
const secret=process.env.DC_ACADEMY_HTTP_PASSWORD;delete process.env.DC_ACADEMY_HTTP_PASSWORD;
if(!secret)throw Error('native_credential_required');
let ready:Promise<unknown>|undefined,failed=false,tail:Promise<unknown>=Promise.resolve();
const server=new McpServer({name:'academy-printed-paper-lookup',version:'0.1.0'});
registerRuntimeIdentity(server,runtimeIdentity.identity);
const schema=z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/),paperCode:z.string().regex(/^#[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{1,11}$/),period}).strict();
const result=(value:unknown)=>{const structuredContent=JSON.parse(JSON.stringify(value));return {content:[{type:'text' as const,text:JSON.stringify(structuredContent)}],structuredContent};};
server.registerTool('academy_printed_paper_lookup',{
  title:'Exact printed-paper metadata lookup',
  description:'Read the exact admitted student and printed code through the source-owner client for an explicit allowed period. A not-found result covers the observed page only. No grading, generation, printing, save or parent send.',
  inputSchema:schema,
  annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true},
},input=>{
 const run=async()=>{
  const started=performance.now(),before=sourceRequests;
  const student=config.students.find(s=>s.studentId===input.studentId);
  if(!student||input.period.from<config.allowedPeriod.from||input.period.through>config.allowedPeriod.through||input.period.from>input.period.through)return result({status:'blocked',code:'read_scope_not_admitted',sourceRequests:0,totalSourceRequests:sourceRequests,authenticationAttempts,academyWrites:0,parentSends:0});
  if(failed)return result({status:'blocked',code:'source_failure_latched_no_retry',sourceRequests:0,automaticRetryAllowed:false,academyWrites:0,parentSends:0});
  try{
   ready??=(async()=>{authenticationAttempts++;return client.authenticate({username:owner.sourceUsername,password:secret});})();
   await ready;
   const observation=await client.lookupPrintedPaperMetadata({teacherLabel:owner.teacherLabel,studentName:student.displayName,studentKey:student.studentId,paperCode:input.paperCode,period:input.period});
   return result({status:observation.status,scope:{studentId:student.studentId,paperCode:input.paperCode},observation,sourceBundleSha256:config.sourceBundleSha256,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},academyWrites:0,parentSends:0,automaticRetryAllowed:false});
  }catch(error){
   failed=true;client.close();
   const code=error instanceof Error&&/^[a-z][a-z0-9_]{0,100}$/.test(error.message)?error.message:'source_lookup_failed';
   return result({status:'blocked',code,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},automaticRetryAllowed:false,academyWrites:0,parentSends:0});
  }
 };
 const next=tail.then(run);tail=next.then(()=>undefined,()=>undefined);return next;
});
process.once('exit',()=>client.close());
server.registerTool('academy_source_read_contracts',{
 title:'Static source read contracts',description:'Inspect the exact observed DT catalog shell or static printed-code converter program, without invoking source handlers.',
 inputSchema:z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/),kind:z.enum(['printed_code','daily_test_catalog','middle1_definitions','elementary5_fraction_definitions']),unitCodes:z.array(z.string().regex(/^(5-[1-7]|6-[1-5]|7-[12])$/)).min(1).max(14).optional(),unitNames:z.record(z.string(),z.string().min(1).max(100)).optional()}).strict(),annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true},
},input=>{
 const run=async()=>{
  const started=performance.now(),before=sourceRequests,student=config.students.find(s=>s.studentId===input.studentId);
  if(!student)return result({status:'blocked',code:'read_scope_not_admitted',sourceRequests:0,authenticationAttempts,academyWrites:0,parentSends:0});
  if(failed)return result({status:'blocked',code:'source_failure_latched_no_retry',automaticRetryAllowed:false});
  try{
   ready??=(async()=>{authenticationAttempts++;return client.authenticate({username:owner.sourceUsername,password:secret});})();await ready;
   const observation=input.kind==='printed_code'?await client.inspectPrintedCodeProtocol():input.kind==='middle1_definitions'?await client.readMiddle1DailyTestDefinitions({unitCodes:input.unitCodes??[],...(input.unitNames?{unitNames:input.unitNames}:{})}):input.kind==='elementary5_fraction_definitions'?await client.readElementary5FractionAssessmentDefinitions():await client.inspectDailyTestCatalog();
   return result({status:'observed',observation,sourceBundleSha256:config.sourceBundleSha256,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},academyWrites:0,parentSends:0,automaticRetryAllowed:false});
  }catch(error){failed=true;client.close();const code=error instanceof Error&&/^[a-z][a-z0-9_]{0,100}$/.test(error.message)?error.message:'source_contract_read_failed';return result({status:'blocked',code,automaticRetryAllowed:false,academyWrites:0,parentSends:0});}
 };
 const next=tail.then(run);tail=next.then(()=>undefined,()=>undefined);return next;
});
server.registerTool('academy_paper_headers',{
 title:'Exact printed-paper metadata batch',
 description:'Read at most 32 explicit paper headers or definitions, four independent readers at a time with one authentication and one qualified reader contract. Does not generate, print, grade, save or send.',
 inputSchema:variant==='paper1'?paper1HeadersSchema:paper2HeadersSchema,
 annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true},
},input=>{
 const run=async()=>{
  const started=performance.now(),before=sourceRequests,student=config.students.find(s=>s.studentId===input.studentId);
  if(!student||new Set(input.paperCodes).size!==input.paperCodes.length)return result({status:'blocked',code:'read_scope_not_admitted',sourceRequests:0,authenticationAttempts,academyWrites:0,parentSends:0});
  if(failed)return result({status:'blocked',code:'source_failure_latched_no_retry',automaticRetryAllowed:false});
  try{
   ready??=(async()=>{authenticationAttempts++;return client.authenticate({username:owner.sourceUsername,password:secret});})();await ready;
   const target={teacherLabel:owner.teacherLabel,studentName:student.displayName,studentKey:student.studentId,paperCodes:input.paperCodes};
   const observation='view' in input&&input.view==='definition'?await client.readPrintedPaperDefinitions(target):await client.readPrintedPaperHeaders(target);
   return result({status:'observed',scope:{studentId:student.studentId},observation,sourceBundleSha256:config.sourceBundleSha256,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},academyWrites:0,parentSends:0,automaticRetryAllowed:false});
  }catch(error){failed=true;client.close();const code=error instanceof Error&&/^[a-z][a-z0-9_]{0,100}$/.test(error.message)?error.message:'source_headers_read_failed';return result({status:'blocked',code,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},automaticRetryAllowed:false,academyWrites:0,parentSends:0});}
 };
 const next=tail.then(run);tail=next.then(()=>undefined,()=>undefined);return next;
});
server.registerTool('academy_daily_test_inventory',{
 title:'Exact student DailyTest inventory and read contracts',
 description:'Discover the admitted student rows on the currently qualified DT result page, with explicit filter/page coverage. Does not infer unobserved expected papers or grading completion from a score placeholder.',
 inputSchema:z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/),periods:z.array(z.enum(['11','12','13','14','15','16'])).min(1).max(6).optional()}).strict(),
 annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true},
},input=>{
 const run=async()=>{
  const started=performance.now(),before=sourceRequests,student=config.students.find(s=>s.studentId===input.studentId);
  if(!student)return result({status:'blocked',code:'read_scope_not_admitted',sourceRequests:0,authenticationAttempts,academyWrites:0,parentSends:0});
  if(failed)return result({status:'blocked',code:'source_failure_latched_no_retry',automaticRetryAllowed:false});
  try{
   ready??=(async()=>{authenticationAttempts++;return client.authenticate({username:owner.sourceUsername,password:secret});})();await ready;
   const target={teacherLabel:owner.teacherLabel,studentName:student.displayName,studentKey:student.studentId};
   const observation=input.periods?await client.readDailyTestHistory({...target,periods:input.periods}):await client.inspectDailyTestInventory(target);
   return result({status:'observed',scope:{studentId:student.studentId},observation,sourceBundleSha256:config.sourceBundleSha256,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},academyWrites:0,parentSends:0,automaticRetryAllowed:false});
  }catch(error){failed=true;client.close();const code=error instanceof Error&&/^[a-z][a-z0-9_]{0,100}$/.test(error.message)?error.message:'source_inventory_read_failed';return result({status:'blocked',code,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},automaticRetryAllowed:false,academyWrites:0,parentSends:0});}
 };
 const next=tail.then(run);tail=next.then(()=>undefined,()=>undefined);return next;
});
server.registerTool('academy_paper_definition_lookup',{
 title:'Printed paper definition information',
 description:'Read the shared paper title, explicit source category/level and question count by the owner-decoded printed code. Does not establish a student attempt, grade or textbook completion. No generation, printing, save or parent send.',
 inputSchema:z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/),paperCode:z.string().regex(/^#[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{1,11}$/),view:z.enum(['definition','header']).default('definition')}).strict(),
 annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true},
},input=>{
 const run=async()=>{
  const started=performance.now(),before=sourceRequests,student=config.students.find(s=>s.studentId===input.studentId);
  if(!student)return result({status:'blocked',code:'read_scope_not_admitted',sourceRequests:0,authenticationAttempts,academyWrites:0,parentSends:0});
  if(failed)return result({status:'blocked',code:'source_failure_latched_no_retry',automaticRetryAllowed:false});
  try{
   ready??=(async()=>{authenticationAttempts++;return client.authenticate({username:owner.sourceUsername,password:secret});})();await ready;
   const target={teacherLabel:owner.teacherLabel,studentName:student.displayName,studentKey:student.studentId,paperCode:input.paperCode};
   const observation=input.view==='header'?await client.readPrintedPaperHeader(target):await client.readPrintedPaperDefinition(target);
   return result({status:observation.status,scope:{studentId:student.studentId,paperCode:input.paperCode},observation,sourceBundleSha256:config.sourceBundleSha256,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},academyWrites:0,parentSends:0,automaticRetryAllowed:false});
  }catch(error){failed=true;client.close();const code=error instanceof Error&&/^[a-z][a-z0-9_]{0,100}$/.test(error.message)?error.message:'source_definition_read_failed';return result({status:'blocked',code,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},automaticRetryAllowed:false,academyWrites:0,parentSends:0});}
 };
 const next=tail.then(run);tail=next.then(()=>undefined,()=>undefined);return next;
});
server.registerTool('academy_paper_read_contracts',{
 title:'Read-only paper lookup contract inspection',
 description:'Inspect only inert form/function metadata from the existing paper search and TestPool page. It retains no source student rows and executes no page handlers.',
 inputSchema:z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/)}).strict(),
 annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true},
},input=>{
 const run=async()=>{
  const started=performance.now(),before=sourceRequests,student=config.students.find(s=>s.studentId===input.studentId);
  if(!student)return result({status:'blocked',code:'read_scope_not_admitted',sourceRequests:0,authenticationAttempts});
  if(failed)return result({status:'blocked',code:'source_failure_latched_no_retry',automaticRetryAllowed:false});
  try{
   ready??=(async()=>{authenticationAttempts++;return client.authenticate({username:owner.sourceUsername,password:secret});})();await ready;
   const studentPaperForm=await client.inspectStudentPaperForm({teacherLabel:owner.teacherLabel,studentName:student.displayName,studentKey:student.studentId});
   const testPool=await client.inspectTestPoolIndex();
   return result({status:'inert_contracts_observed',studentPaperForm,testPool,metrics:{durationMs:performance.now()-started,sourceRequests:sourceRequests-before,totalSourceRequests:sourceRequests,authenticationAttempts},sourceStudentRowsRetained:0,sourceHandlersExecuted:false,academyWrites:0,parentSends:0});
  }catch(error){failed=true;client.close();const code=error instanceof Error&&/^[a-z][a-z0-9_]{0,100}$/.test(error.message)?error.message:'source_contract_inspection_failed';return result({status:'blocked',code,automaticRetryAllowed:false});}
 };
 const next=tail.then(run);tail=next.then(()=>undefined,()=>undefined);return next;
});
await server.connect(new StdioServerTransport());
