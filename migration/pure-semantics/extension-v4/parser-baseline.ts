import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {effects} from './isolation.ts';
const args=process.argv.slice(2),root=args[args.indexOf('--source-root')+1],dom=args[args.indexOf('--dom-root')+1];
if(!root?.startsWith('/')||!dom?.startsWith('/'))throw Error('explicit_local_roots_required');
const input=JSON.parse(await Bun.stdin.text()),events:any[]=[],sourceCalls:string[]=[];
if(input.ports?.now!=='2040-10-15T09:00:00.000Z')throw Error('explicit_parser_clock_required');
const NativeDate=Date,clock=NativeDate.parse(input.ports.now);
class TestDate extends NativeDate{constructor(...a:any[]){if(a.length)super(...a as [any]);else super(clock);}static now(){return clock;}}
globalThis.Date=TestDate as DateConstructor;
let outcome;
try{
 const {JSDOM}=await import(pathToFileURL(resolve(dom,'lib/api.js')).href);
 const m=await import(pathToFileURL(resolve(root,'apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts')).href);
 const handlers=readFileSync(resolve(root,'apps/academy/dc-gang-a/tests/lms/fixtures/cism-handlers.js.txt'),'utf8');
 const date='2040-10-15';
 // Synthetic HTML, faithful to the retained parser's selected caller. No scripts
 // or resources run: JSDOM contentType only, matching the unchanged native port.
 const period=m.cismWeek(date),a=new NativeDate(period.periodStart),b=new NativeDate(period.periodEnd);
 let html=`<h1>주간 교무회의 CISM 작성</h1><form id="theDateForm"><input name="reqCmd" value="CismChartMain"><input id="the_date" value="${date}"><input id="tutor_pri_no" value="90001"><input id="teacher_name" value="Fictional Teacher"><label>2040-42 주차 (${a.getUTCMonth()+1}월${a.getUTCDate()}일~${b.getUTCMonth()+1}월${b.getUTCDate()}일)</label></form><label id="grp_name_0">Fictional Group</label><select id="class_mode_0" onchange="setCismData('class_mode',0,90002,90003,1);"><option value="0">-선택-</option><option value="3">상</option><option value="2">중</option><option value="1">하</option></select><table><tr><td>Fictional Student</td><td><input id="think_note_0" onfocusout="setCismData('think_note',0,90002,90003,1);" value=""></td><td><input id="week_test_0" onfocusout="setCismData('week_test',0,90002,90003,1);" value=""></td></tr><tr><th id="think_note_0">0</th></tr></table>${m.CISM_NARRATIVES.map((n:string)=>`<label>${n}</label><textarea name="${n}" id="${n}" onfocusout="javascript:onTextareaFocusOut(this,'${n}');">${n==='i_1'?'Synthetic &amp; note':''}</textarea>`).join('')}<script>${handlers}</script>`;
 for(const [from,to]of input.replacements??[])html=html.replace(from,to);
 if(input.html!==undefined)html=input.html;
 const teacher=Object.hasOwn(input,'teacher')?input.teacher:{teacherKey:'90001',teacherLabel:'Fictional Teacher'};
 const parser=(s:string,mime:string)=>{events.push({event:'inert_dom_parse',mime,bytes:Buffer.byteLength(s)});return new JSDOM(s,{contentType:mime}).window.document;};
 sourceCalls.push('cism.parseCismReport');
 const value=m.parseCismReport(html,parser,input.date??date,teacher);
 outcome={kind:'return',value};
}catch(e:any){outcome={kind:'throw',error:{name:e?.name??'NonError',message:e?.message??String(e),...(e?.issues?{issues:e.issues}:{})}};}
process.stdout.write(JSON.stringify({outcome,events,effects,metadata:{sourceCalls,uuidValues:[]}})+'\n');
