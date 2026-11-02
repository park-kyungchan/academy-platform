// Calls immutable original exports. It never reads EXPECTED.json or candidate output.
import {effects,resetEffects,configureTrace,trace} from './guard.ts';
import {readFileSync,writeFileSync,mkdirSync,symlinkSync,chmodSync,lstatSync,realpathSync} from 'node:fs';
import {resolve,isAbsolute,dirname,relative} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const args=process.argv.slice(2),opt=(n:string)=>args[args.indexOf(n)+1];
const root=opt('--source-root'),ref=opt('--reference-root'),scratch=opt('--scratch-root');
const hash=(v:string|Uint8Array)=>createHash('sha256').update(v).digest('hex');
const json=(p:string)=>JSON.parse(readFileSync(p,'utf8'));
function ownedRoot(p:string|undefined){if(!p||!isAbsolute(p)||realpathSync(p)!==p)throw Error('s09_explicit_canonical_root_required');const s=lstatSync(p);if(!s.isDirectory()||s.uid!==process.getuid!()||(s.mode&0o077)!==0)throw Error('s09_fixture_root_ownership_required');}
// Startup admission is reference-harness policy, NOT a new claim about original paths.ts/loadSavedContext.
if(!root||!isAbsolute(root)||!ref||!isAbsolute(ref))throw Error('s09_explicit_source_and_reference_root_required');
ownedRoot(scratch);
configureTrace(resolve(scratch,'effects.ndjson'));
const pins=json(resolve(ref,'SOURCE-PINS.json'));
for(const [p,v] of Object.entries(pins.sources) as any){if(hash(readFileSync(resolve(root,p)))!==v.sha256)throw Error('s09_original_source_changed');}
const load=(p:string)=>import(pathToFileURL(resolve(root,p)).href);
const lms='apps/academy/dc-gang-a/src/lms/',mcp='modules/academy-workbench/tools/academy-mcp/src/';
const {AcademyHttpClient,sourceDomParser}=await load(lms+'academyHttpClient.ts');
const {inspectTestPoolPresentationProtocol}=await load(lms+'testPoolPresentationProtocol.ts');
const {selectSourceFunction}=await load(lms+'sourceFunctionText.ts');
const {createAcademyReadPlan,readLiteralKey}=await load(lms+'academyReadProtocol.ts');
const {JOURNAL_HTTP_SOURCE_CONTRACT}=await load(lms+'lessonJournalHttp.ts');
const {NativeDailyRecordBackend,nativeDailyRecordConfigSchema,nativeDailyRecordConfiguration}=await load(mcp+'native-dailyrecord-backend.ts');
const {loadSavedContext,SavedContext}=await load(mcp+'saved-context.ts');
const {paginate}=await load(mcp+'pagination.ts');
const {resolveStudentByName}=await load(mcp+'student-resolver.ts');
const {readPinnedLocalBytes}=await load(mcp+'item-label-store.ts');
const {toolResultBytes}=await load(mcp+'tool-result.ts');
const paths=await load(mcp+'paths.ts');
const corpus=resolve(ref,'corpus'),text=(n:string)=>readFileSync(resolve(corpus,n),'utf8');
const principal={teacherLabel:'S09 Synthetic Teacher',teacherKey:'99001',collegeKey:'88001'};
const scope={sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001',lessonDate:'2040-01-04'};
const target={studentName:'S09 Learner 가😀',date:'2040-01-04',groupId:'3',teacherKey:'99001',studentKey:'77001',courseKey:'5',recordKey:'13',membershipKey:'8'};
const request={family:'day_record',teacherLabel:principal.teacherLabel,date:target.date,groupIds:['3'],studentNames:[target.studentName]};
const sid='s09://account-A/student/0001';
const auditPath='private/student-audit-20261014/audit.json',lessonPath='private/lesson-20261014/overlay.json';
const error=(e:any)=>({name:e.name,message:e.message,...(e.issues?{issues:e.issues}:{}),...(e.sourceOperation?{sourceOperation:e.sourceOperation,httpStatus:e.httpStatus}:{}),...(e.code?{code:e.code}:{})});
async function observed(fn:()=>any){try{return {outcome:'returned',value:await fn()};}catch(e){return {outcome:'threw',error:error(e)};}}
function evidence(id:string,a:any=json(resolve(corpus,'audit.json')),b:any=json(resolve(corpus,'lesson.json'))){
 const dir=resolve(scratch,id);mkdirSync(dir,{mode:0o700});
 for(const [p,v] of [[auditPath,a],[lessonPath,b]] as any){const path=resolve(dir,p);mkdirSync(dirname(path),{recursive:true});writeFileSync(path,Buffer.isBuffer(v)?v:JSON.stringify(v)+'\n');}
 const files=[auditPath,lessonPath].map(path=>({path,sha256:hash(readFileSync(resolve(dir,path)))}));
 const pin=resolve(dir,'tools/academy-mcp/architecture/source-pins.json');mkdirSync(dirname(pin),{recursive:true});writeFileSync(pin,JSON.stringify({files}));return {dir,files,pin};
}
function rawSpans(html:string,parsed:any){const doc=parsed.document;return [...doc.querySelectorAll('#repair,td,select,option,input,textarea,script#handler')].map((n:any)=>{const range=parsed.sourceRange(n);return {tag:n.tagName,id:n.id,value:['INPUT','SELECT','TEXTAREA','OPTION'].includes(n.tagName)?n.value:null,text:n.textContent,sourceRange:range,raw:range?html.slice(range.start,range.end):null};});}
function presentation(v:string){let html=text('presentation.html'),helper=text('helpers.js');
 if(v==='duplicate-control')html=html.replace('</select>','</select><select id="difficulty"><option>dup</option></select>');
 if(v==='duplicate-label')html='<label for="difficulty">duplicate</label>'+html;
 if(v==='malformed-script')html=html.replace('var g_examtype_filter="all"','var g_examtype_filter=');
 if(v==='duplicate-function')helper+='\nfunction previewDBExam(key){return "another";}';
 if(v==='oversize')html='x'.repeat(2000001);
 if(v==='too-many-options')html='<select id="difficulty">'+Array.from({length:101},()=>'<option>x</option>').join('')+'</select>';
 const parser=v==='invalid-range'?((h:string)=>{const p=sourceDomParser.parseWithSourceLocations(h);return {...p,sourceRange:()=>({start:-1,end:0})};}):sourceDomParser.parseWithSourceLocations;
 const value=inspectTestPoolPresentationProtocol(html,helper,parser);
 const spans=[];
 for(const c of value.controls){spans.push({kind:'control',range:c.sourceRange,raw:html.slice(c.sourceRange.start,c.sourceRange.end)});for(const o of c.options)spans.push({kind:'option',range:o.sourceRange,raw:html.slice(o.sourceRange.start,o.sourceRange.end)});}
 for(const f of value.functionEvidence){const source=f.source==='selector'?html:helper;spans.push({kind:'function',name:f.name,range:f.sourceRange,raw:source.slice(f.sourceRange.start,f.sourceRange.end)});for(const lit of f.literals)spans.push({kind:'literal',range:lit.sourceRange,raw:source.slice(lit.sourceRange.start,lit.sourceRange.end)});}
 for(const f of value.filterDeclarations)spans.push({kind:'filter',range:f.sourceRange,raw:html.slice(f.sourceRange.start,f.sourceRange.end)});
 for(const f of value.gridColumns)spans.push({kind:'column',range:f.sourceRange,raw:html.slice(f.sourceRange.start,f.sourceRange.end)});
 return {value,requiredOriginalSpans:spans};
}
// Exact fake route inventory: URLs are identifiers only; no default fetch or socket exists.
async function httpCase(v:string,authOnly=false){
 const calls:any[]=[],cookieProof:any[]=[];let count=0;let client:any;let summary:any;
 const fake=async(raw:string,init:any)=>{
  const url=new URL(raw),body=new URLSearchParams(typeof init.body==='string'?init.body:''),q=url.searchParams,selector=q.get('reqCmd')??body.get('p_process')??null;
  const method=init.method??'GET',path=url.pathname,headers=new Headers(init.headers);
  const denied=()=>{effects.deniedOperations++;throw Error('s09_recorded_route_denied');};
  if(url.origin!=='https://dc.gang-a.kr')return denied();
  const loginDoc=path==='/c_login.jsp'&&method==='GET';
  const loginSubmit=path==='/servlet/controller.login.LoginServlet'&&method==='POST'&&body.get('p_process')==='c_login';
  const loginNext=path==='/synthetic-login-next'&&method==='GET';
  const course=path==='/servlet/controller.coursemanage.CourseManageServlet'&&method==='GET'&&q.get('reqCmd')==='StudyCourse';
  const main=path==='/servlet/controller.cct.tutor.DayRecordServlet'&&method==='POST'&&body.get('p_process')==='Main';
  const selected=path==='/servlet/controller.cct.tutor.CourseScheduleServlet'&&method==='GET'&&q.get('reqCmd')==='SelSglRecord';
  if(![loginDoc,loginSubmit,loginNext,course,main,selected].some(Boolean))return denied();
  effects.fakeDispatches++;count++;trace({event:'recorded_dispatch',path,method,selector});
  const loginEncoding=loginSubmit?['userid','userpwd'].map(k=>{const b=Buffer.from(body.get(k)??'','base64').toString('utf8');return {field:k,validSuffix:/  [a-z0-9]{30}$/.test(b),prefixMatchesFixture:b.slice(0,-32)===(k==='userid'?'s09-fictional-user':'s09-fictional-password')};}):undefined;
  calls.push({path,method,selector,redirect:init.redirect,cache:init.cache,hasAbortSignal:init.signal instanceof AbortSignal,signalAborted:init.signal?.aborted??null,origin:headers.get('Origin'),referer:headers.get('Referer'),hasCookie:headers.has('Cookie'),hasAuthorization:headers.has('Authorization'),...(loginEncoding?{loginEncoding}:{parameters:[...body],query:[...q]})});
  cookieProof.push({step:count,hasSession:headers.get('Cookie')?.includes('JSESSIONID=')??false,foreignCookieForwarded:headers.get('Cookie')?.includes('OTHER=')??false});
  if(loginDoc){if(v==='login-failure')return new Response('',{status:403});if(v==='foreign-cookie')return new Response(text('login.html'),{headers:{'Set-Cookie':'JSESSIONID=synthetic; Domain=foreign.invalid; Path=/'}});return new Response(v==='login-wire'?'<form></form>':text('login.html'),{headers:{'Set-Cookie':'JSESSIONID=synthetic; Path='+(v==='cookie-path'?'/servlet/':'/')+'; HttpOnly'}});}
  if(loginSubmit){if(['foreign-login-redirect','login-redirect','second-redirect','missing-location'].includes(v))return new Response('',{status:302,headers:v==='missing-location'?{}:{Location:v==='foreign-login-redirect'?'https://foreign.invalid/':'/synthetic-login-next'}});return new Response('',{headers:v==='cookie-delete'?{'Set-Cookie':'JSESSIONID=; Max-Age=0; Path=/'}:{'Set-Cookie':'OTHER=not-forwarded; Path=/'}});}
  if(loginNext)return new Response('',v==='second-redirect'?{status:302,headers:{Location:'/synthetic-again'}}:{});
  if(course){let html=text('principal.html');if(v==='principal-key')html=html.replaceAll('99001','99002');if(v==='principal-college')html=html.replace('88001','88002');return new Response(html);}
  if(v==='transport-throw')throw Error('synthetic_transport_failure');
  if(v==='slow-abort')return await new Promise<Response>((_,reject)=>{init.signal.addEventListener('abort',()=>{trace({event:'source_abort_signal',path,selector});reject(Error('synthetic_abort'));},{once:true});});
  if(v==='stalled-body')return new Response(new ReadableStream({start(c){c.enqueue(new TextEncoder().encode('<table>'));}}));
  if(v==='redirect')return new Response('',{status:302,headers:{Location:'/c_login.jsp'}});
  if(v==='http-error')return new Response('',{status:503});
  if(v==='login-challenge')return new Response('<input type="password">');
  if(v==='oversize')return new Response('x'.repeat(2000001));
  const prior=body.get('std_ymd')==='20400102'||q.get('record_seq')==='12';
  let html=text(prior?'prior.html':'current.html'),xml=text(prior?'prior.xml':'current.xml');
  if(v==='duplicate-rows')html=html.replace('</tbody>',html.match(/<tbody>([\s\S]*?)<\/tbody>/)![1]+'</tbody>');
  if(v==='inconsistent-key')xml=xml.replace('!77001!','!77002!');
  if(v==='malformed-xml')xml='<root><record>';
  if(v==='duplicate-date-id')html='<input id="duplicate" name="std_date" value="2040-01-04">'+html;
  if(v==='broken-table')html=html.replace('</tbody></table>','</table>').replace('<tbody>','');
  if(v==='leading-zero-key')html=html.replaceAll("'77001'","'077001'");
  if(v==='case-name')html=html.replaceAll('S09 Learner','s09 Learner');
  if(v==='space-name')html=html.replaceAll('S09 Learner',' S09 Learner');
  if(v==='wrong-principal')html=html.replaceAll('99001','99002');
  let response=new Response(selected?xml:html,{headers:{'Content-Type':v==='source-encoding'?'text/html; charset=euc-kr':selected?'text/xml':'text/html'}});
  if(v==='utf8-invalid')response=new Response(Buffer.concat([Buffer.from(selected?xml:html),Buffer.from([255])]));
  if(v==='foreign-response-url')Object.defineProperty(response,'url',{value:'https://foreign.invalid/returned'});
  return response;
 };
 let pp={...principal};if(v==='invalid-principal')pp.teacherKey='0';if(v==='constructor-leading-zero')pp.teacherKey='099001';if(v==='constructor-delimiter')pp.teacherKey='99:001';if(v==='constructor-whitespace')pp.teacherKey='99001 ';if(v==='constructor-case-key')pp.teacherKey='ABC';
 const results:any[]=[];
 try{
  client=new AcademyHttpClient(pp,fake);
  results.push(await observed(()=>client.authenticate({username:'s09-fictional-user',password:'s09-fictional-password'})));
  if(v==='retry-login')results.push(await observed(()=>client.authenticate({username:'s09-fictional-user',password:'s09-fictional-password'})));
  if(!authOnly&&client.authenticated){
   if(v==='journal-unqualified')results.push(await observed(()=>client.journalState(target)));
   else if(v==='wire-inspection')results.push(await observed(()=>client.inspectDayRecordWire(request)));
   else {results.push(await observed(()=>client.discoverDayRecordGroups({teacherLabel:principal.teacherLabel,date:v==='prior'?'2040-01-02':target.date})));results.push(await observed(()=>client.execute(v==='prior'?{...request,date:'2040-01-02'}:request)));}
  }
  summary={results,calls,cookieProof,authenticatedBeforeClose:client.authenticated};return summary;
 }finally{client?.close();if(summary)summary.authenticatedAfterClose=client.authenticated;}
}
async function native(v:string){
 const events:any[]=[],admission={teacherId:'99001',teacherLabel:principal.teacherLabel,date:target.date,students:[{studentId:'77001',displayName:target.studentName}],sourceBundleSha256:'b'.repeat(64)};
 const row={...target,fields:{progress:'Current progress',homework:'',lessonMemo:'Memo',attendance:'Y',persistentMemo:'synthetic unexported'}};
 const prior={...row,date:'2040-01-02',recordKey:'12',fields:{...row.fields,progress:'Previous progress',homework:'Previous homework'}};
 const state={target:{...target},fields:{...row.fields,dailyTestAssessment:'0',previousHomeworkAssessment:'3'},previousRecordKey:'12',previousDate:'2040-01-02',wireDigest:'a'.repeat(64)};
 if(v==='other-account'){admission.teacherId='99002';state.target.teacherKey='99002';row.teacherKey='99002';prior.teacherKey='99002';}
 if(v==='state-target')state.target.recordKey='14';
 if(v==='null-field')(state.fields as any).progress=null;
 if(v==='zero-field')(state.fields as any).progress=0;
 if(v==='missing-field')delete (state.fields as any).progress;
 if(v==='blank-field')state.fields.progress='';
 const port={authenticate:async()=>{events.push({operation:'authenticate'});if(v==='auth-failure-twice')throw Error('synthetic_auth_failure');},discoverDayRecordGroups:async(input:any)=>{events.push({operation:'discover',input});return {groups:[{groupId:'3'}]};},execute:async(input:any)=>{events.push({operation:'execute',input});const current=input.date===target.date;let rows:any[]=[structuredClone(current?row:prior)];
 if(current){if(v==='duplicate-current')rows.push(structuredClone(row));if(v==='wrong-student')rows[0].studentKey='77002';if(v==='wrong-teacher')rows[0].teacherKey='99002';if(v==='wrong-date')rows[0].date='2040-01-05';}
 else{if(v==='previous-missing')rows=[];else {for(const [variant,key,value] of [['previous-record','recordKey','11'],['previous-student','studentKey','77002'],['previous-course','courseKey','6'],['previous-member','membershipKey','9'],['previous-date','date','2040-01-01']])if(v===variant)rows[0][key]=value;}}
 return {dayRecords:rows};},journalState:async(t:any)=>{events.push({operation:'journalState',target:t});return structuredClone(state);},close:()=>{events.push({operation:'close'});}};
 const backend=new NativeDailyRecordBackend(port,admission,()=>({username:'s09-fictional-user',password:'s09-fictional-password'}));
 const selected={...scope,teacherId:admission.teacherId};for(const [variant,key,value] of [['scope-namespace','sourceNamespace','foreign'],['scope-teacher','teacherId','99003'],['scope-student','studentId','77003'],['scope-date','lessonDate','2040-01-05']])if(v===variant)(selected as any)[key]=value;
 const results=[await observed(()=>backend.readDailyRecord(selected))];if(v==='normal'||v==='auth-failure-twice')results.push(await observed(()=>backend.readDailyRecord(selected)));
 return {fixtureAccount:v==='other-account'?'account-B':'account-A',results,events,qualifiedProductionWire:false};
}
async function savedSpecial(id:string,v:string){
 const a=json(resolve(corpus,'audit.json')),b=json(resolve(corpus,'lesson.json'));
 if(v==='budget-partial')a.students[0].attempts=Array.from({length:8},(_,i)=>({id:'budget-'+i,payload:'가"\\'.repeat(12000)}));
 if(v==='budget-single')a.students[0].attempts=[{id:'oversized',payload:'가'.repeat(100000)}];
 if(v==='budget-fixed')a.collectedAt='x'.repeat(300000);
 if(v==='loader-malformed-json')return observed(()=>{const f=evidence(id,Buffer.from('{'),b);return loadSavedContext(f.dir);});
 if(v==='loader-invalid-utf8')return observed(()=>{const f=evidence(id,Buffer.from([255]),b);return loadSavedContext(f.dir);});
 if(v==='loader-duplicate-id')a.students[1].studentId=a.students[0].studentId;
 if(v==='loader-duplicate-name')a.students[1].name=a.students[0].name;
 if(v==='loader-lesson-foreign')b.students[0].studentId='outside';
 if(v==='loader-draft-foreign')b.students[0].dailyRecordDraft.student.studentId='outside';
 if(v==='loader-event-duplicate')b.students[0].teacherEvents.push(b.students[0].teacherEvents[0]);
 const f=evidence(id,a,b);
 if(v==='loader-pin-missing')writeFileSync(f.pin,JSON.stringify({files:[]}));
 if(v==='loader-hash-mismatch')writeFileSync(resolve(f.dir,auditPath),'{}');
 if(v==='loader-world-writable')chmodSync(resolve(f.dir,auditPath),0o666);
 if(v==='loader-symlink'){const copy=resolve(f.dir,'audit-copy.json');writeFileSync(copy,readFileSync(resolve(f.dir,auditPath)));const {unlinkSync}=await import('node:fs');unlinkSync(resolve(f.dir,auditPath));symlinkSync(copy,resolve(f.dir,auditPath));}
 let dir=f.dir;if(v==='loader-parent-symlink'){dir=resolve(scratch,id+'-alias');symlinkSync(f.dir,dir);}
 const loaded=await observed(()=>loadSavedContext(dir));if(loaded.outcome==='threw')return loaded;
 const ctx=(loaded as any).value,q={kind:'student_history',studentId:sid},input={query:q,limit:2};
 if(v.startsWith('loader'))return observed(()=>ctx.query({query:{kind:'roster'}}));
 if(v.startsWith('budget'))return observed(()=>ctx.query({query:{kind:'student_records',studentId:sid,facet:'attempts'},limit:250}));
 const results:any[]=[];let page=ctx.query(input);results.push(page);
 if(v==='pagination'){while(page.page.nextCursor){page=ctx.query({...input,cursor:page.page.nextCursor});results.push(page);}return {outcome:'returned',value:results};}
 let cursor=page.page.nextCursor,query:any=q,limit=2;
 if(v==='cursor-wrong-selection')query={kind:'roster'};
 if(v==='cursor-query-key-order')query={studentId:sid,kind:'student_history'}; // schema reconstruction determines hash, not supplied member order.
 if(v==='cursor-limit-change')limit=3;
 const token=JSON.parse(Buffer.from(cursor,'base64url').toString());
 if(v==='cursor-wrong-snapshot')token.snapshot='f'.repeat(64);
 if(v==='cursor-offset-equal')token.offset=page.page.totalInSavedSelection;
 if(v==='cursor-offset-over')token.offset=page.page.totalInSavedSelection+1;
 if(v==='cursor-negative')token.offset=-1;
 if(v==='cursor-extra')token.extra=true;
 cursor=Buffer.from(JSON.stringify(token)).toString('base64url');results.push(ctx.query({query,limit,cursor}));return {outcome:'returned',value:results};
}
async function operation(c:any){
 const plan=createAcademyReadPlan(request),mutations=JOURNAL_HTTP_SOURCE_CONTRACT.fields;
 let op:any=c.field?mutations[c.field]:plan.operations[0];const url=new URL(op.path,'https://dc.gang-a.kr'),method=c.variant==='wrong-method'?'GET':op.method;
 const key=c.field?'reqCmd':op.selectorKey,selector=c.variant==='unknown-selector'?'unreviewed':op.selector,placement=c.field?'query':op.selectorPlacement;
 const body=new URLSearchParams();(placement==='body'?body:url.searchParams).set(key,selector);
 if(c.variant==='foreign-origin')url.host='foreign.invalid';if(c.variant==='duplicate-selector')(placement==='body'?body:url.searchParams).append(key,selector);
 // Migration read-comparator admission selects actual original plan operations. All live transport remains denied independently.
 const matched=url.origin==='https://dc.gang-a.kr'&&plan.operations.find((p:any)=>p.path===url.pathname&&p.method===method&&(p.selectorPlacement==='body'?body:url.searchParams).getAll(p.selectorKey).length===1&&(p.selectorPlacement==='body'?body:url.searchParams).get(p.selectorKey)===p.selector);
 if(!matched){effects.deniedOperations++;trace({event:'operation_denied',method,path:url.pathname,selector});return {outcome:'threw',error:{name:'Error',message:'s09_recorded_operation_denied'},operation:{method,path:url.pathname,selector},dispatches:0,basis:c.field?'original_mutating_field_contract':'original_read_plan'};}
 effects.fakeDispatches++;trace({event:'operation_fake_dispatch',method,path:url.pathname,selector});return {outcome:'returned',value:{operation:matched,dispatches:1}};
}
async function run(c:any){
 switch(c.kind){
 case 'dom':{const html=text('dom.html'),parsed=sourceDomParser.parseWithSourceLocations(html),doc=parsed.document;return {spans:rawSpans(html,parsed),duplicateIdCount:doc.querySelectorAll('#duplicate').length,firstDuplicateTag:doc.getElementById('duplicate')?.tagName,repairTbody:doc.querySelector('#repair tbody')?.tagName,syntheticTbodyRange:parsed.sourceRange(doc.querySelector('#repair tbody')),sourceFunction:selectSourceFunction(doc.querySelector('#handler')!.textContent!,'onFoPrg'),scriptExecuted:(doc.defaultView as any).s09ScriptRan===true,resourceCount:doc.querySelectorAll('[src]').length};}
 case 'presentation':return presentation(c.variant??'normal');
 case 'auth':return httpCase(c.variant,true);
 case 'http':return httpCase(c.variant);
 case 'native':return native(c.variant);
 case 'operation':return operation(c);
 case 'literal-key':return readLiteralKey(c.value);
 case 'resolve':{let calls=0;const value=await resolveStudentByName(c.name,{searchByName:async(name:string)=>{calls++;return c.rows;}});return {value,calls};}
 case 'saved':{const f=evidence(c.id);return (await loadSavedContext(f.dir)).query(c.input);}
 case 'saved-special':return savedSpecial(c.id,c.variant);
 case 'pinned-local':{const dir=resolve(scratch,c.id);mkdirSync(dir,{mode:0o700});let p=resolve(dir,'input.json');writeFileSync(p,'{"synthetic":true}\n');let sha=hash(readFileSync(p));
 if(c.variant==='bad-hash')sha='0'.repeat(64);if(c.variant==='world-writable')chmodSync(p,0o666);if(c.variant==='oversize')writeFileSync(p,'x'.repeat(4194305));if(c.variant==='directory')p=dir;if(c.variant==='relative')p='input.json';
 if(c.variant==='symlink'){const alias=resolve(dir,'alias.json');symlinkSync(p,alias);p=alias;}if(c.variant==='ancestor-symlink'){const alias=resolve(scratch,c.id+'-alias');symlinkSync(dir,alias);p=resolve(alias,'input.json');}
 const value=await readPinnedLocalBytes(p,sha);return {sha256:value.sha256,bytes:value.bytes.toString('base64')};}
 case 'paginate':{const v=c.variant;return paginate(v==='empty'?[]:['a','b','c'],{kind:'synthetic'},'a'.repeat(64),v==='limit-zero'?0:v==='limit-fraction'?1.5:v==='limit-max'?250:3,undefined,v==='bad-budget'?0:v==='byte-limited'?7:v==='single-too-big'?3:Infinity);}
 case 'native-config':{if(process.getuid!()===10000)throw Error('s09_owner_refusal_probe_requires_nonowner');return nativeDailyRecordConfiguration(c.variant==='empty'?[]:['--dailyrecord-native',resolve(scratch,'absent-config.json'),'b'.repeat(64)]);}
 case 'config-schema':return nativeDailyRecordConfigSchema.parse({schemaVersion:'academy.native-dailyrecord-read/1',sourceBundlePath:'/synthetic/bundle',sourceBundleSha256:'b'.repeat(64),date:target.date,students:[{studentId:'77001',displayName:'S09 Learner'},{studentId:'77001',displayName:'S09 Learner'}]});
 case 'paths':return {packageRoot:relative(root,paths.packageRoot),repoRoot:relative(root,paths.repoRoot),resultRoot:relative(root,paths.resultRoot),reaEntry:relative(root,paths.reaEntry),rootBasis:'original_module_location_not_cwd_or_explicit_evidence'};
 default:throw Error('s09_case_kind_unselected');
 }
}
const cases=json(resolve(corpus,'CASES.json'));const frames=[];
for(const c of cases){if(args.includes('--case')?c.id!==opt('--case'):c.variant==='stalled-body')continue;resetEffects();trace({event:'case_start',id:c.id});const result=await observed(()=>run(c));effects.sourceHandlersExecuted=(globalThis as any).s09ScriptRan===true;frames.push({id:c.id,kind:c.kind,...result,effects:{...effects}});trace({event:'case_end',id:c.id,effects:{...effects}});}
const runtimeExports:any={AcademyHttpClient:Object.getOwnPropertyNames(AcademyHttpClient.prototype).filter(k=>k!=='constructor'),NativeDailyRecordBackend:Object.getOwnPropertyNames(NativeDailyRecordBackend.prototype).filter(k=>k!=='constructor'),SavedContext:Object.getOwnPropertyNames(SavedContext.prototype).filter(k=>k!=='constructor'),nativeConfigSchemaRuntimeAvailable:typeof nativeDailyRecordConfigSchema.parse==='function'};
process.stdout.write(JSON.stringify({schema:'s09.original-reference-output/1',frames,runtimeExports})+'\n');
