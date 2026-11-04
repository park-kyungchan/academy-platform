// Additive original observers: do not alter the frozen primary reference or its expected bytes.
import {effects,configureTrace,trace,resetEffects} from './guard.ts';
import {readFileSync,writeFileSync,mkdirSync,lstatSync,chmodSync} from 'node:fs';
import {resolve,isAbsolute} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const [root,ref,scratch]=process.argv.slice(2),sha=(v:any)=>createHash('sha256').update(v).digest('hex');
if(![root,ref,scratch].every(isAbsolute))throw Error('s09_explicit_roots_required');
if(lstatSync(scratch).uid!==process.getuid!()||(lstatSync(scratch).mode&0o077)!==0)throw Error('s09_fixture_root_ownership_required');
const pins=JSON.parse(readFileSync(resolve(ref,'SOURCE-PINS.json'),'utf8'));for(const [p,d] of Object.entries(pins.sources) as any)if(sha(readFileSync(resolve(root,p)))!==d.sha256)throw Error('s09_original_source_changed');
configureTrace(resolve(scratch,'effects.ndjson'));
const mcp='modules/academy-workbench/tools/academy-mcp/src/';const load=(p:string)=>import(pathToFileURL(resolve(root,mcp+p)).href);
const {NativeDailyRecordBackend}=await load('native-dailyrecord-backend.ts');
const {loadSavedContext}=await load('saved-context.ts');
function error(e:any):any{return {name:e.name,message:e.message,...Object.fromEntries(['code','category','evidenceLocator','httpStatus','sourceOperation'].filter(k=>k in e).map(k=>[k,e[k]])),...('issues'in e?{issues:e.issues}:{}),...(e.cause?{cause:error(e.cause)}:{})};}
async function observe(f:any){try{return {outcome:'returned',value:await f()};}catch(e){return {outcome:'threw',error:error(e)};}}
const target={studentName:'S09 Observer Learner',date:'2040-01-04',groupId:'3',teacherKey:'99001',studentKey:'77001',courseKey:'5',recordKey:'13',membershipKey:'8'};
function fixture(account:string,fail=false){let auth=0;const fields={progress:'',homework:'',lessonMemo:'',attendance:'Y',dailyTestAssessment:'0',previousHomeworkAssessment:'-1'};
 const port={authenticate:async()=>{auth++;if(fail)throw Error('synthetic_auth_failure');},discoverDayRecordGroups:async()=>({groups:[{groupId:'3'}]}),execute:async(input:any)=>({dayRecords:[{...target,...(input.date===target.date?{}:{date:'2040-01-02',recordKey:'12'}),fields}]}),journalState:async()=>({target,fields,previousRecordKey:'12',previousDate:'2040-01-02',wireDigest:'a'.repeat(64)}),close(){}};
 const admission={teacherId:'99001',teacherLabel:'S09 Synthetic Teacher',date:target.date,students:[{studentId:'77001',displayName:target.studentName}],sourceBundleSha256:'b'.repeat(64)};
 const backend=new NativeDailyRecordBackend(port,admission,()=>({username:'s09-fictional-user',password:'s09-fictional-password'}));return {account,backend,authCalls:()=>auth};}
const scope={sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001',lessonDate:target.date},frames:any[]=[];
async function frame(id:string,f:any){resetEffects();trace({event:'case_start',id});frames.push({id,...await observe(f),effects:{...effects}});trace({event:'case_end',id,effects:{...effects}});}
await frame('authentication-error-own-observers',async()=>{const f=fixture('account-A',true);return {first:await observe(()=>f.backend.readDailyRecord(scope)),second:await observe(()=>f.backend.readDailyRecord(scope)),authCalls:f.authCalls()};});
await frame('same-principal-two-accounts-same-native-key',async()=>{const results=[];for(const account of ['account-A','account-B']){const f=fixture(account),v=await f.backend.readDailyRecord(scope);results.push({fixtureAccount:account,sourceTarget:v.observation.target,sourcePreviousTarget:v.observation.previousTarget,sourceRevision:v.observation.sourceRevision,wireDigest:v.observation.wireDigest,authCalls:f.authCalls()});}return {results,sourceDistinguishesAccounts:JSON.stringify(results[0].sourceTarget)!==JSON.stringify(results[1].sourceTarget),accountIsExternalFixtureMetadata:true};});
await frame('saved-loader-32MiB-leaf-refusal',async()=>{const path=resolve(scratch,'oversize');for(const d of ['private/student-audit-20261014','private/lesson-20261014','tools/academy-mcp/architecture'])mkdirSync(resolve(path,d),{recursive:true,mode:0o700});const audit=resolve(path,'private/student-audit-20261014/audit.json'),lesson=resolve(path,'private/lesson-20261014/overlay.json');writeFileSync(audit,'x'.repeat(32*1024*1024+1));writeFileSync(lesson,'{}');writeFileSync(resolve(path,'tools/academy-mcp/architecture/source-pins.json'),JSON.stringify({files:[{path:'private/student-audit-20261014/audit.json',sha256:sha(readFileSync(audit))},{path:'private/lesson-20261014/overlay.json',sha256:sha('{}')}]}));return loadSavedContext(path);});
process.stdout.write(JSON.stringify({schema:'s09.additional-source-observers/1',frames})+'\n');
