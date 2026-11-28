import {createHash} from 'node:crypto';
import {selectSourceFunction} from './sourceFunctionText';
export const CISM_PATH='/servlet/controller.cct.common.CourseCommonServlet';
export const CISM_WIRE_SHA256='401beb7c9957d4b688e00655f6f5ece6831e1b857961b131d7cf02269fd6b67b';
export const cismHash=(v:unknown)=>createHash('sha256').update(JSON.stringify(v)).digest('hex');
export const CISM_NARRATIVES=['i_1','i_2','s_1','s_2','s_3','s_4','m_1','m_2'] as const;
export type CismField={fieldRef:string;kind:'narrative'|'class_mode'|'think_note'|'week_test';label:string;value:string;studentName:string|null;groupName:string|null;source:{fieldName:string;studentKey?:string;membershipKey?:string;groupKey?:string}};
/** One row of the block data: column numbers `"1"`… to text, as `makeCISMJSonData` of the CismChartMain page writes them. */
export type CismBlockRow=Record<string,string>;
/** The object `makeCISMJSonData` of the CismChartMain page builds and hands to the StudyQ executor (specs/academy-mcp/pc-executor.md). */
export type CismBlockData={block1:CismBlockRow[];block2:CismBlockRow[];block3:CismBlockRow[];block4:CismBlockRow[];block5:CismBlockRow[];block6:CismBlockRow[]};
export type CismBlockRead={date:string;periodStart:string;periodEnd:string;weekLabel:string;blockData:CismBlockData};
export type CismReport={date:string;periodStart:string;periodEnd:string;weekLabel:string;teacherLabel:string;teacherKey:string;wireSha256:string;reportSha256:string;observedAt:string;fields:CismField[];sourceMode:'live_source';pdfGenerated:false;blockData?:CismBlockData};
/** The block data of a parsed page, read as `makeCISMJSonData` reads it (jQuery `.text()`, `.val()` without `\r`):
 * block1 `"1"` is `"1"` when the numbers of `label[name=label_think_note]` sum to other than 0, else `"0"`; block2 the date,
 * the teacher and the week `<M>월<D>일~<M>월<D>일`; block3 one row per `div[id^=root]` `i`: `grp_name_i`, the level of the
 * `class_mode_tag_i` select (1 하, 2 중, 3 상, else empty), `submit_sum_i`, `homework_rate_i`, the labels of `think_note_i`
 * with `%` and of `week_test_i` with `점`; block4 to block6 the narratives Ｉ-1, Ｉ-2, Ｓ-1 to Ｓ-4, Ｍ-1, Ｍ-2. */
function cismBlockData(d:Document,week:string):CismBlockData{
 const text=(id:string)=>d.getElementById(id)?.textContent??'';
 const val=(node:Element|null|undefined)=>{const v=(node as HTMLInputElement|null|undefined)?.value;return typeof v==='string'?v.replace(/\r/g,''):undefined;};
 const labels=(id:string)=>[...d.querySelectorAll('[id="'+id+'"] label')].map(n=>n.textContent??'').join('');
 let sum=0;for(const n of d.querySelectorAll('label[name="label_think_note"]')){const v=Number((n.textContent??'').trim());if(v)sum+=v;}
 const block3=[...Array(d.querySelectorAll('div[id^="root"]').length).keys()].map(i=>{
  const mode=val(d.querySelector('[id="class_mode_tag_'+i+'"] select'));
  return {'1':text('grp_name_'+i),'2':({1:'하',2:'중',3:'상'} as Record<number,string>)[mode===undefined?NaN:Number(mode)]??'','3':text('submit_sum_'+i),
   '4':text('homework_rate_'+i),'5':labels('think_note_'+i)+'%','6':labels('week_test_'+i)+'점'};
 });
 const narrative=(title:string,id:string)=>({'1':title,'2':val(d.getElementById(id))??''});
 return {block1:[{'1':sum===0?'0':'1'}],block2:[{'1':val(d.getElementById('the_date'))??'','2':val(d.getElementById('teacher_name'))??'','3':week}],block3,
  block4:[narrative('Ｉ-1','i_1'),narrative('Ｉ-2','i_2')],block5:[narrative('Ｓ-1','s_1'),narrative('Ｓ-2','s_2'),narrative('Ｓ-3','s_3'),narrative('Ｓ-4','s_4')],
  block6:[narrative('Ｍ-1','m_1'),narrative('Ｍ-2','m_2')]};
}
export function cismDate(date:string){const d=new Date(date+'T00:00:00Z');if(!/^20\d{2}-\d{2}-\d{2}$/.test(date)||!Number.isFinite(+d)||d.toISOString().slice(0,10)!==date)throw Error('cism_date_rejected');return d;}
export function cismWeek(date:string){const d=cismDate(date);d.setUTCDate(d.getUTCDate()-d.getUTCDay());const periodStart=d.toISOString().slice(0,10);d.setUTCDate(d.getUTCDate()+6);return {periodStart,periodEnd:d.toISOString().slice(0,10)};}
export function cismMonthDates(month:string){if(!/^20\d{2}-(0[1-9]|1[0-2])$/.test(month))throw Error('cism_month_rejected');const first=month+'-01',dates=[first],d=cismDate(first);d.setUTCDate(d.getUTCDate()+(7-d.getUTCDay()));while(d.toISOString().slice(0,7)===month){dates.push(d.toISOString().slice(0,10));d.setUTCDate(d.getUTCDate()+7);}return dates;}
export function parseCismReport(html:string,parse:(s:string,m:DOMParserSupportedType)=>Document,date:string,teacher:{teacherKey:string;teacherLabel:string}):CismReport{
 cismDate(date);if(Buffer.byteLength(html)>2000000)throw Error('cism_body_rejected');const d=parse(html,'text/html'),value=(id:string)=>(d.getElementById(id) as HTMLInputElement|null)?.value;
 if(d.querySelector('input[type=password]')||!d.querySelector('h1')?.textContent?.includes('주간 교무회의 CISM 작성'))throw Error('cism_document_rejected');
 if(value('the_date')!==date||value('tutor_pri_no')!==teacher.teacherKey||value('teacher_name')!==teacher.teacherLabel||d.querySelector('#theDateForm [name=reqCmd]')?.getAttribute('value')!=='CismChartMain')throw Error('cism_scope_conflict');
 const wire=['setCismData','onTextareaFocusOut'].map(n=>selectSourceFunction(html.replaceAll('\r\n','\n'),n));
 if(wire.some(x=>!x)||createHash('sha256').update(wire.join('\n')).digest('hex')!==CISM_WIRE_SHA256)throw Error('cism_wire_drift');
 const period=cismWeek(date),start=cismDate(period.periodStart),end=cismDate(period.periodEnd),weekLabel=d.querySelector('#theDateForm label')?.textContent?.trim()??'';
 const label=weekLabel.match(/^(20\d{2})-(\d{1,2}) 주차 \((\d{1,2})월(\d{1,2})일~(\d{1,2})월(\d{1,2})일\)$/);
 if(!label||Number(label[3])!==start.getUTCMonth()+1||Number(label[4])!==start.getUTCDate()||Number(label[5])!==end.getUTCMonth()+1||Number(label[6])!==end.getUTCDate())throw Error('cism_period_conflict');
 const fields:CismField[]=[];
 for(const name of CISM_NARRATIVES){const nodes=d.querySelectorAll('textarea#'+name),node=nodes[0] as HTMLTextAreaElement|undefined;
  if(nodes.length!==1||!node||node.name!==name||node.getAttribute('onfocusout')!=="javascript:onTextareaFocusOut(this,'"+name+"');")throw Error('cism_fields_unqualified');
  const source={fieldName:name};fields.push({fieldRef:cismHash(['narrative',source]),kind:'narrative',label:node.previousElementSibling?.textContent?.trim()??name,value:node.value,studentName:null,groupName:null,source});}
 const controls=[...d.querySelectorAll('select[id^=class_mode_],input[onfocusout]')];
 const groups=new Map<string,string>();for(const mode of controls.filter(n=>n.tagName==='SELECT')){const m=mode.getAttribute('onchange')?.match(/,(\d+)\);$/);const label=mode.closest('[id="root"]')?.querySelector('[id^=grp_name_]')?.textContent?.trim()??d.getElementById('grp_name_'+mode.id.replace('class_mode_',''))?.textContent?.trim();if(m&&label)groups.set(m[1]!,label);}
 for(const node of controls){const handler=node.getAttribute('onchange')??node.getAttribute('onfocusout')??'';if(!handler.includes('setCismData('))continue;
  const m=handler.match(/^setCismData\('(class_mode|think_note|week_test)',(\d+),([1-9]\d{0,15}),([1-9]\d{0,15}),([1-9]\d{0,15})\);$/);
  if(!m||node.id!==m[1]+'_'+m[2]||d.querySelectorAll(node.tagName+'[id="'+node.id+'"]').length!==1)throw Error('cism_fields_unqualified');
  const kind=m[1] as CismField['kind'],source={fieldName:m[1]!,studentKey:m[3]!,membershipKey:m[4]!,groupKey:m[5]!};
  const groupName=groups.get(m[5]!)??null;
  if(kind==='class_mode'&&[...node.querySelectorAll('option')].map(o=>o.getAttribute('value')).join(',')!=='0,3,2,1')throw Error('cism_fields_unqualified');
  fields.push({fieldRef:cismHash([kind,source]),kind,label:kind==='class_mode'?'이번주 반 수업 분위기':kind==='think_note'?'풀이노트작성':'주간테스트',value:(node as HTMLInputElement).value,studentName:kind==='class_mode'?null:node.closest('tr')?.cells[0]?.textContent?.trim()??null,groupName,source});
 }
 if(fields.length<8||fields.length>200||new Set(fields.map(f=>f.fieldRef)).size!==fields.length)throw Error('cism_fields_unqualified');
 return {date,...period,weekLabel,teacherLabel:teacher.teacherLabel,teacherKey:teacher.teacherKey,wireSha256:CISM_WIRE_SHA256,reportSha256:cismHash([period,teacher.teacherKey,CISM_WIRE_SHA256,fields]),observedAt:new Date().toISOString(),fields,sourceMode:'live_source',pdfGenerated:false,
  blockData:cismBlockData(d,label[3]+'월'+label[4]+'일~'+label[5]+'월'+label[6]+'일')};
}
export function cismWriteParameters(report:CismReport,field:CismField,after:string){
 if(!report.fields.some(f=>f.fieldRef===field.fieldRef&&f.kind===field.kind&&JSON.stringify(f.source)===JSON.stringify(field.source)))throw Error('cism_fields_unqualified');
 if(!after.trim()||after.length>6000||/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(after))throw Error('cism_value_rejected');
 if(field.kind==='class_mode'&&!/^[123]$/.test(after)||['think_note','week_test'].includes(field.kind)&&(!/^\d{1,3}(?:\.\d{1,2})?$/.test(after)||Number(after)>100))throw Error('cism_value_rejected');
 const params=new URLSearchParams({reqCmd:field.kind==='narrative'?'SetCismContentData':'SetCismChartData',tutor_pri_no:report.teacherKey,the_date:report.date,field_name:field.source.fieldName,field_value:after});
 if(field.kind!=='narrative')for(const [wire,key] of [['stu_pri_no','studentKey'],['cm_seq','membershipKey'],['grp_seq','groupKey']] as const){const value=field.source[key];if(!value)throw Error('cism_fields_unqualified');params.set(wire,value);}
 return params;
}
