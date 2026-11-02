import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {JSDOM} from 'jsdom';
import {parseCismReport,cismWeek,cismMonthDates,cismWriteParameters,CISM_NARRATIVES,CISM_WIRE_SHA256} from '../../src/lms/cismWeeklyProtocol';
const handlers=readFileSync(new URL('./fixtures/cism-handlers.js.txt',import.meta.url),'utf8');
export function fictionalCismHtml(date='2040-10-15',narrative=''){
 const {periodStart,periodEnd}=cismWeek(date),a=new Date(periodStart),b=new Date(periodEnd);
 return `<h1>주간 교무회의 CISM 작성</h1><form id="theDateForm"><input name="reqCmd" value="CismChartMain"><input id="the_date" value="${date}"><input id="tutor_pri_no" value="90001"><input id="teacher_name" value="Fictional Teacher"><label>2040-42 주차 (${a.getUTCMonth()+1}월${a.getUTCDate()}일~${b.getUTCMonth()+1}월${b.getUTCDate()}일)</label></form><label id="grp_name_0">Fictional Group</label><select id="class_mode_0" onchange="setCismData('class_mode',0,90002,90003,1);"><option value="0">-선택-</option><option value="3">상</option><option value="2">중</option><option value="1">하</option></select><table><tr><td>Fictional Student</td><td><input id="think_note_0" onfocusout="setCismData('think_note',0,90002,90003,1);" value=""></td><td><input id="week_test_0" onfocusout="setCismData('week_test',0,90002,90003,1);" value=""></td></tr><tr><th id="think_note_0">0</th></tr></table>${CISM_NARRATIVES.map(n=>`<label>${n}</label><textarea name="${n}" id="${n}" onfocusout="javascript:onTextareaFocusOut(this,'${n}');">${n==='i_1'?narrative:''}</textarea>`).join('')}<script>${handlers}</script>`;
}
const parse=(h:string,date='2040-10-15')=>parseCismReport(h,(s,m)=>new JSDOM(s,{contentType:m}).window.document as any,date,{teacherKey:'90001',teacherLabel:'Fictional Teacher'});
test('CISM validates exact principal, actual week and pinned source handlers without executing scripts',()=>{
 const r=parse(fictionalCismHtml());expect(r.fields).toHaveLength(11);expect(r.wireSha256).toBe(CISM_WIRE_SHA256);expect(r.fields.find(f=>f.kind==='class_mode')?.groupName).toBe('Fictional Group');
 for(const [from,to] of [['value="90001"','value="90009"'],['CismChartMain','Other'],['2040-42 주차 (','2040-42 주차 (1'],['reqCmd=SetCismChartData','reqCmd=WrongOperation']])expect(()=>parse(fictionalCismHtml().replace(from,to))).toThrow();
 expect(()=>parse(fictionalCismHtml().replace('<script>', '<script>throw new Error("do not execute");'))).not.toThrow();
});
test('CISM identities carry native row tuples, duplicates are rejected while source summary-cell ID duplicates are allowed',()=>{
 const r=parse(fictionalCismHtml());expect(r.fields.find(f=>f.kind==='think_note')?.source).toEqual({fieldName:'think_note',studentKey:'90002',membershipKey:'90003',groupKey:'1'});
 expect(()=>parse(fictionalCismHtml().replace('</table>','<input id="think_note_0" onfocusout="setCismData(\'think_note\',0,90002,90003,1);"></table>'))).toThrow('cism_fields_unqualified');
});
test('CISM exact POST parameters encode narrative ampersands/newlines and disallow arbitrary fields and invalid metrics',()=>{
 const r=parse(fictionalCismHtml()),n=r.fields[0]!,metric=r.fields.find(f=>f.kind==='week_test')!,mode=r.fields.find(f=>f.kind==='class_mode')!;
 expect(()=>cismWriteParameters(r,{...n,source:{fieldName:'foreign_field'}},'text')).toThrow('cism_fields_unqualified');
 const p=cismWriteParameters(r,n,'확인 & 조치\n추후 점검');expect(new URLSearchParams(p.toString()).get('field_value')).toBe('확인 & 조치\n추후 점검');expect(p.get('reqCmd')).toBe('SetCismContentData');
 expect(cismWriteParameters(r,metric,'80').get('cm_seq')).toBe('90003');for(const v of ['101','NaN','-1','1&reqCmd=Other',''])expect(()=>cismWriteParameters(r,metric,v)).toThrow();for(const v of ['0','4'])expect(()=>cismWriteParameters(r,mode,v)).toThrow();
});
test('month scope includes every intersecting Sunday–Saturday week and rejects invalid dates',()=>{
 expect(cismMonthDates('2026-10')).toEqual(['2026-10-01','2026-10-04','2026-10-11','2026-10-18','2026-10-25']);expect(cismWeek('2026-10-19')).toEqual({periodStart:'2026-10-18',periodEnd:'2026-10-24'});expect(()=>cismWeek('2026-02-30')).toThrow();expect(()=>cismMonthDates('2026-13')).toThrow();
});
