// Guard installed before dynamic imports. Actual unchanged original exports only.
import {effects,configureTrace,trace,resetEffects} from '../source-adapter-reference-v1/guard.ts';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const [root,inputPath,tracePath]=process.argv.slice(2);configureTrace(tracePath);
const ref=resolve(import.meta.dir,'../source-adapter-reference-v1');
const pins=JSON.parse(readFileSync(resolve(ref,'SOURCE-PINS.json'),'utf8'));
for(const [p,v] of Object.entries(pins.sources) as any)if(createHash('sha256').update(readFileSync(resolve(root,p))).digest('hex')!==v.sha256)throw Error('original_source_changed');
const load=(p:string)=>import(pathToFileURL(resolve(root,'apps/academy/dc-gang-a/src/lms',p)).href);
const {AcademyHttpClient,sourceDomParser}=await load('academyHttpClient.ts');
const {inspectTestPoolPresentationProtocol}=await load('testPoolPresentationProtocol.ts');
const {assertJournalAssessmentOptions,JOURNAL_HTTP_SOURCE_CONTRACT}=await load('lessonJournalHttp.ts');
const stimuli=JSON.parse(readFileSync(inputPath,'utf8'));
async function observed(fn:()=>any){try{return {outcome:'returned',value:await fn()};}catch(e:any){return {outcome:'threw',error:{name:e.name,message:e.message}};}}
const presentation=[];
for(const c of stimuli.presentation){resetEffects();const result=await observed(()=>inspectTestPoolPresentationProtocol(c.html,c.helpers,sourceDomParser.parseWithSourceLocations));presentation.push({id:c.id,result,effects:{...effects}});}
const assessment=Object.entries(JOURNAL_HTTP_SOURCE_CONTRACT.assessmentLabels).flatMap(([field,vs])=>Object.entries(vs as any).map(([value,label])=>({field,value,label})));
const assessmentResults=[];
for(const [id,options] of [['exact',assessment],['reverse',[...assessment].reverse()],['duplicate',[...assessment,assessment[0]]],['missing',assessment.slice(1)],['label',assessment.map((r,i)=>i? r:{...r,label:r.label+' '})],['extra',assessment.map((r,i)=>i?r:{...r,extra:true})]] as any){assessmentResults.push({id,result:await observed(()=>{assertJournalAssessmentOptions(options);return null;})});}
const redirects=[];
const principal={teacherLabel:'S09 Synthetic Teacher',teacherKey:'99001',collegeKey:'88001'};
const NativeURL=globalThis.URL;
for(const [i,location] of stimuli.locations.entries()){
 resetEffects();const calls:any[]=[];let n=0;
 const client=new AcademyHttpClient(principal,async(raw:string,init:any)=>{
  const u=new NativeURL(raw),headers=new Headers(init.headers);const selector=u.searchParams.get('reqCmd')??new URLSearchParams(init.body??'').get('p_process');
  effects.fakeDispatches++;const call={path:u.pathname,query:[...u.searchParams],method:init.method??'GET',selector,cookie:headers.get('cookie')};calls.push(call);trace({event:'redirect_fake_dispatch',id:i,...call});
  n++;
  if(n===1)return new Response(readFileSync(resolve(ref,'corpus/login.html'),'utf8'),{headers:{'Set-Cookie':'JSESSIONID=redirect; Path=/'}});
  if(n===2)return new Response('',{status:302,headers:{Location:location,'Set-Cookie':'JSESSIONID=follow; Path=/'}});
  if(n===3 && u.pathname!=='/servlet/controller.coursemanage.CourseManageServlet')return new Response('',i===stimuli.locations.length-1?{status:302,headers:{Location:'/again'}}:{});
  return new Response(readFileSync(resolve(ref,'corpus/principal.html'),'utf8'));
 });
 const result=await observed(()=>client.authenticate({username:'s09-fictional-user',password:'s09-fictional-password'}));client.close();redirects.push({id:i,location,result,calls,effects:{...effects}});
}
// Inject a static request into the actual full client at its native URL boundary.
// No private method extraction, transformed source or qualification bypass.
const selector=[];
for(const extras of [1,2]){
 resetEffects();let selectedDispatches=0,mainDispatches=0;
 const client=new AcademyHttpClient(principal,async(raw:string,init:any)=>{
  const u=new NativeURL(raw);effects.fakeDispatches++;
  if(u.pathname==='/c_login.jsp')return new Response(readFileSync(resolve(ref,'corpus/login.html'),'utf8'));
  if(u.pathname.includes('LoginServlet'))return new Response('');
  if(u.pathname.includes('CourseManageServlet'))return new Response(readFileSync(resolve(ref,'corpus/principal.html'),'utf8'));
  if(u.pathname.includes('DayRecordServlet')){mainDispatches++;return new Response(readFileSync(resolve(ref,'corpus/current.html'),'utf8'));}
  if(u.pathname.includes('CourseScheduleServlet')){selectedDispatches++;return new Response(readFileSync(resolve(ref,'corpus/current.xml'),'utf8'));}
  throw Error('unlisted_fake_route');
 });
 await client.authenticate({username:'s09-fictional-user',password:'s09-fictional-password'});
 // Command-local injected fake request: populate the opposite placement only.
 const NativeParams=globalThis.URLSearchParams;let selected=false;
 globalThis.URL=class extends NativeURL{constructor(input:any,base?:any){super(input,base);if(this.pathname.includes('CourseScheduleServlet'))selected=true;}} as any;
 globalThis.URLSearchParams=class extends NativeParams{constructor(input:any){super(input);if(selected&&input===''){for(let i=0;i<extras;i++)this.append('reqCmd','SelSglRecord');selected=false;}}} as any;
 const result=await observed(()=>client.execute({family:'day_record',teacherLabel:principal.teacherLabel,date:'2040-01-04',groupIds:['3'],studentNames:['S09 Learner 가😀']}));
 globalThis.URL=NativeURL;globalThis.URLSearchParams=NativeParams;client.close();selector.push({extras,result,selectedDispatches,mainDispatches,effects:{...effects}});
}
process.stdout.write(JSON.stringify({presentation,assessment,assessmentResults,redirects,selector})+'\n');
