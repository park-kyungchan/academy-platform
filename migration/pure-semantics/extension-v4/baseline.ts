import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {writeFileSync,unlinkSync} from 'node:fs';
import {effects,uuidEvents} from './isolation.ts';
const args=process.argv.slice(2), root=args[args.indexOf('--source-root')+1];
if(!args.includes('--source-root')||!root?.startsWith('/'))throw Error('explicit_source_root_required');
const input=JSON.parse(await Bun.stdin.text()), events:any[]=[], calls:string[]=[];
const edu='apps/academy/dc-gang-a/src/',mcp='modules/academy-workbench/tools/academy-mcp/src/';
const paths={learning:edu+'learning/model.ts',cism:edu+'lms/cismWeeklyProtocol.ts',templates:edu+'lms/lessonJournalTemplates.ts',admission:mcp+'dailyrecord-save-admission.ts',daily:mcp+'dailyrecord.ts',context:mcp+'dailyrecord-context.ts'};
const load=(family:keyof typeof paths)=>import(pathToFileURL(resolve(root,paths[family])).href);
const errorValue=(e:any)=>({name:e?.name??'NonError',message:e?.message??String(e),...(e?.issues?{issues:e.issues}:{})});
function materialize(v:any):any{
 if(!v||typeof v!=='object')return v;
 if(Array.isArray(v))return v.map(materialize);
 if(Object.keys(v).length===1&&'$js' in v){
  switch(v.$js){
   case 'undefined':return undefined;case '-0':return -0;case 'NaN':return NaN;case 'Infinity':return Infinity;
   case 'bigint':return 1n;case 'symbol':return Symbol('synthetic');case 'function':return ()=>1;
   case 'cycle':{const x:any={};x.self=x;return x;}
   case 'array_cycle':{const x:any[]=[];x.push(x);return x;}
   case 'sparse':return new Array(2);
   case 'decorated':return Object.assign([1],{extra:2});
   case 'symbol_key':return {[Symbol('key')]:1,a:2};
   case 'accessor':return Object.defineProperty({},'a',{enumerable:true,get(){events.push('accessor_read');return 1;}});
   case 'date':return new Date('2040-01-01T00:00:00Z');
   case 'null_prototype':return Object.assign(Object.create(null),{b:1,a:2});
   case 'shared':{const x={z:0};return {a:x,b:x};}
   case 'inherited':return Object.assign(Object.create({inherited:1}),{a:2});
   case 'nonenumerable':return Object.defineProperty({a:1},'hidden',{value:2,enumerable:false});
   default:if(/^depth_\d+$/.test(v.$js)){let x:any=null;for(let n=+v.$js.slice(6);n--; )x=[x];return x;}
  }throw Error('unknown_js_descriptor');
 }
 return Object.fromEntries(Object.entries(v).map(([k,x])=>[k,materialize(x)]));
}
function observed(v:any):any{
 if(v===undefined)return {$observed:'undefined'};
 if(typeof v==='number'&&(!Number.isFinite(v)||Object.is(v,-0)))return {$observed:String(v),negativeZero:Object.is(v,-0)};
 if(v instanceof Date)return {$observed:'Date',iso:v.toISOString()};
 if(v instanceof URLSearchParams)return {entries:[...v.entries()],wire:v.toString()};
 if(Array.isArray(v))return v.map(observed);
 if(v&&typeof v==='object')return Object.fromEntries(Object.entries(v).map(([k,x])=>[k,observed(x)]));
 return v;
}
function deepFrozen(v:any):boolean{return !v||typeof v!=='object'||Object.isFrozen(v)&&Object.values(v).every(deepFrozen);}
async function execute(){
 if(input.op==='exports'){const result:any={};for(const key of Object.keys(paths) as (keyof typeof paths)[])result[key]=Object.keys(await load(key)).sort();return result;}
 if(input.op==='learning'){
  const m=await load('learning');const allowed=['stableId','contextFromLessonKey','validateCatalog','resolveProblemPath','validateScope','validateRecord','studentOf','assertContext','parseCalendarDate','assertDate','assertTime','assertInstant','requireText','assertRefs','assertKnowledge','assertExactKeys','canonicalJson','digest','known','unknown','immutableCopy'];
  if(!allowed.includes(input.fn))throw Error('unknown_learning_export');
  const a=materialize(input.args);calls.push('learning.'+input.fn);const result=m[input.fn](...a);
  if(input.fn==='immutableCopy'||input.fn==='known'||input.fn==='unknown'||input.fn==='contextFromLessonKey'||input.fn==='resolveProblemPath'){
   const mutation:any[]=[];
   for(const [label,target] of [['root',result],['value',result?.value],['refs',result?.evidenceRefs],['a',result?.a]] as any){
    if(target&&typeof target==='object'){try{target.__probe=1;mutation.push({label,status:'mutated'});}catch(e){mutation.push({label,status:'throw',error:errorValue(e)});}}
   }
   return {value:observed(result),rootSame:result===a[0],valueSame:!!result&&typeof result==='object'&&Object.hasOwn(result,'value')?result.value===a[0]:null,
    refsSame:result?.evidenceRefs===a[1],sharedAfter:result?.a&&result?.b?result.a===result.b:null,
    allFrozen:deepFrozen(result),prototype:result&&typeof result==='object'?Object.getPrototypeOf(result)===Object.prototype?'Object':Object.getPrototypeOf(result)===null?'null':'other':null,
    ownKeys:result&&typeof result==='object'?Object.keys(result):[],mutation};
  }return observed(result);
 }
 if(input.op==='json'){
  const v=materialize(input.value),m=await load(input.family);calls.push(input.family+'.canonicalJson/hash');
  if(input.family==='cism'){const bytes=JSON.stringify(v);return observed({bytes,sha256:m.cismHash(v)});}
  const bytes=m.canonicalJson(v);return observed({bytes,sha256:input.family==='learning'?m.digest(v):m.canonicalActionDigest(v)});
 }
 if(input.op==='schema'){
  const m=await load(input.module);calls.push(input.module+'.'+input.schema+'.parse');
  const result=m[input.schema].parse(materialize(input.value));
  return {parsed:result,...(input.instant?{dateParse:observed(Date.parse(input.instant)),dateParseFinite:Number.isFinite(Date.parse(input.instant))}:{})};
 }
 if(input.op==='cism_write'){
  const m=await load('cism');calls.push('cism.cismWriteParameters');return observed(m.cismWriteParameters(materialize(input.report),materialize(input.field),materialize(input.after)));
 }
 if(input.op==='cism_date'){
  const m=await load('cism');calls.push('cism.cismDate');return observed(m.cismDate(materialize(input.value)));
 }
 if(input.op==='admission'){
  const m=await load('admission'),admission=materialize(input.admission),steps=materialize(input.steps);calls.push('admission.createDailyRecordSaveAdmission');
  let current:any=input.current,consumeResult=true,throwRead=false,throwConsume=false;
  const owner={async readCurrentRequest(){events.push({event:'local_current_request'});if(throwRead)throw Error('synthetic_read_failure');return current;},async consumeOnce(key:string,record:any){events.push({event:'local_consume_once',key,record,frozen:Object.isFrozen(record),requestFrozen:Object.isFrozen(record.request),sameRequest:record.request===current});if(throwConsume)throw Error('synthetic_consume_failure');return consumeResult;}};
  const approval=m.createDailyRecordSaveAdmission(admission,owner),results:any[]=[];
  for(const step of steps){
   if(Object.hasOwn(step,'current'))current=step.current;if(Object.hasOwn(step,'consumeResult'))consumeResult=step.consumeResult;
   throwRead=step.throwRead??false;throwConsume=step.throwConsume??false;
   if(step.mutateAdmission)Object.assign(admission,step.mutateAdmission);
   if(step.mutateEntry)Object.assign(admission.admitted[0],step.mutateEntry);
   calls.push('admission.consumeApproval');try{results.push({kind:'return',value:await approval.consumeApproval(step.action)});}catch(e){results.push({kind:'throw',error:errorValue(e)});}
  }return {admissionDigest:approval.admissionDigest,results};
 }
 if(input.op==='live_evidence'){
  const m=await load('admission'),path=resolve('local-current-request.json');calls.push('admission.liveSaveRequestEvidence');
  const read=m.liveSaveRequestEvidence(path),results:any[]=[];
  for(const step of input.steps){if(step.missing){try{unlinkSync(path);}catch{}}else writeFileSync(path,step.bytes);
   calls.push('admission.liveSaveRequestEvidence.read');try{results.push({kind:'return',value:await read()});}catch(e){results.push({kind:'throw',error:errorValue(e)});}}
  return results;
 }
 if(input.op==='review'){
  if(!input.ports||!Number.isFinite(input.ports.nowEpochMs)||input.ports.uuid!=='sequential-v4')throw Error('explicit_test_ports_required');
  const {DailyRecordService}=await load('daily');let readIndex=0;
  const backend={async readDailyRecord(scope:any,requestRef:string){events.push({event:'recorded_read',scope,requestRef});const observation=input.observations?.[Math.min(readIndex++,input.observations.length-1)]??input.observation;return observation===null?{status:'unavailable',reasonCode:'synthetic_source_unavailable'}:{status:'observed',observation:structuredClone(observation)};}};
  const safeguard={policySha256:'3'.repeat(64),safeguardSourceSha256:'4'.repeat(64),sourceRef:'synthetic-parent-policy',checkLines:(s:string)=>/^- [^\r\n]+(?:\n- [^\r\n]+)*$/.test(s),check:(s:string)=>/교사\s*(?:최종\s*)?완료\s*판정/.test(s)};
  const policy={bundleSha256:'5'.repeat(64),configSha256:'6'.repeat(64),catalogSha256:'7'.repeat(64),scopeQualified:()=>true,review:(_s:any,value:string,entries:any)=>{events.push({event:'fake_learning_review',value,entries:entries??null});return [];}};
  const now=()=>input.ports.nowEpochMs;
  const service=new DailyRecordService(backend,safeguard,now,policy);calls.push('daily.DailyRecordService.intake');
  const intake=await service.intake(input.intake??{context:input.context});calls.push('daily.DailyRecordService.review');
  const review=await service.review(input.review??{context:input.context,...(intake.observationHandle?{observationHandle:intake.observationHandle}:{}),proposals:input.proposals});
  return {intake,review};
 }
 throw Error('unknown_extension_operation');
}
let outcome;try{outcome={kind:'return',value:await execute()};}catch(e){outcome={kind:'throw',error:errorValue(e)};}
const frame={outcome,events,effects,metadata:{sourceCalls:calls,uuidValues:uuidEvents}};
if(input.control==='network'){try{await fetch('http://127.0.0.1:1/never');}catch{}}
if(input.control==='dispatch')effects.forbiddenFakeDispatches++;
if(input.control==='stderr')process.stderr.write('junk\n');
if(input.control==='stdout')process.stdout.write('junk\n');
process.stdout.write(JSON.stringify(frame)+'\n');
if(input.control==='nonzero')process.exitCode=7;
