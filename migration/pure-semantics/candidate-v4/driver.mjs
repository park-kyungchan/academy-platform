// Test constructors and lossless lowering only. No expected-file access here.
import {readFileSync} from 'node:fs';
import {lower} from './wire.mjs';
const narratives=['i_1','i_2','s_1','s_2','s_3','s_4','m_1','m_2'];
export function requestFor(input,script,sourceRoot){
 if(script==='direct-baseline-v2.ts')return lower({...input,op:'direct',value:{$js:input.tag==='depth512'?'depth_512':input.tag}});
 if(script==='parser-baseline.ts'){
  const handlers=readFileSync(sourceRoot+'/apps/academy/dc-gang-a/tests/lms/fixtures/cism-handlers.js.txt','utf8');
  // The original-source observer's fixed synthetic HTML constructor, no business result.
  let html=`<h1>주간 교무회의 CISM 작성</h1><form id="theDateForm"><input name="reqCmd" value="CismChartMain"><input id="the_date" value="2040-10-15"><input id="tutor_pri_no" value="90001"><input id="teacher_name" value="Fictional Teacher"><label>2040-42 주차 (10월14일~10월20일)</label></form><label id="grp_name_0">Fictional Group</label><select id="class_mode_0" onchange="setCismData('class_mode',0,90002,90003,1);"><option value="0">-선택-</option><option value="3">상</option><option value="2">중</option><option value="1">하</option></select><table><tr><td>Fictional Student</td><td><input id="think_note_0" onfocusout="setCismData('think_note',0,90002,90003,1);" value=""></td><td><input id="week_test_0" onfocusout="setCismData('week_test',0,90002,90003,1);" value=""></td></tr><tr><th id="think_note_0">0</th></tr></table>${narratives.map(n=>`<label>${n}</label><textarea name="${n}" id="${n}" onfocusout="javascript:onTextareaFocusOut(this,'${n}');">${n==='i_1'?'Synthetic &amp; note':''}</textarea>`).join('')}<script>${handlers}</script>`;
  for(const [from,to]of input.replacements??[])html=html.replace(from,to);if(input.html!==undefined)html=input.html;
  return lower({op:'parser',html,date:input.date??'2040-10-15',teacher:Object.hasOwn(input,'teacher')?input.teacher:{teacherKey:'90001',teacherLabel:'Fictional Teacher'},now:input.ports.now});
 }
 return lower(input);
}
