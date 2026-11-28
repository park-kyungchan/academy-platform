import {CISM_NARRATIVES,cismMonthDates,type CismField,type CismReport} from './cismWeeklyProtocol';

/** The strict `cism` input of modules/studyq-linux (specs/studyq-linux/core.md R19), made from live
 * read-only CISM reports (`CismReport`, parsed from `CismChartMain`). No Academy write, no request:
 * the caller reads the reports. */
export type CismChartRow={group:string;student:string;class_mode:string;think_note:string;week_test:string};
export type CismSection={group:4|5|6;slot:number;title:string;text:string};
export interface CismStudyqInput{week_no:number;week_label:string;teacher:string;period:string;chart_data:CismChartRow[];report_list:CismSection[]}

/** `i_1,i_2,s_1..s_4,m_1,m_2` to the `(group, slot)` of the banner strips `4_1 … 6_2`. */
const NARRATIVE_SLOTS:Record<(typeof CISM_NARRATIVES)[number],[4|5|6,number]>={i_1:[4,1],i_2:[4,2],s_1:[5,1],s_2:[5,2],s_3:[5,3],s_4:[5,4],m_1:[6,1],m_2:[6,2]};
const CLASS_MODE:Record<string,string>={'0':'','3':'A+','2':'A-','1':'B+'};
const WEEK_LABEL=/^(20\d{2})-(\d{1,2}) 주차/;
const NUMERIC=/^\d+(?:\.\d+)?$/;

/** The week number of `weekLabel` (`2040-42 주차 …`), or the refusal `cism_week_number_unqualified`. */
export function cismWeekNumber(weekLabel:string):number{
  const match=WEEK_LABEL.exec(weekLabel);
  const week=match?Number(match[2]):NaN;
  if(!Number.isInteger(week)||week<1||week>53)throw Error('cism_week_number_unqualified');
  return week;
}

/** The rows of one report: think_note and week_test grouped by (groupKey, studentKey) in source order,
 * the class mode of the group joined by groupKey. A student field without a name, or a group without a
 * name and a key, refuses with `cism_field_identity_missing`. */
function chartRows(report:CismReport,groupPrefix:string):CismChartRow[]{
  const modes=new Map<string,string>();
  for(const field of report.fields){
    if(field.kind!=='class_mode')continue;
    const groupKey=field.source.groupKey;
    if(!groupKey)throw Error('cism_field_identity_missing');
    modes.set(groupKey,CLASS_MODE[field.value]??field.value);
  }
  const rows=new Map<string,CismChartRow>();
  for(const field of report.fields){
    if(field.kind!=='think_note'&&field.kind!=='week_test')continue;
    const {groupKey,studentKey}=field.source;
    const student=field.studentName?.trim();
    const groupName=field.groupName?.trim();
    if(!student||!studentKey||(!groupName&&!groupKey))throw Error('cism_field_identity_missing');
    const id=`${groupKey??''}\0${studentKey}`;
    let row=rows.get(id);
    if(!row){
      row={group:groupPrefix+(groupName||`그룹 ${groupKey}`),student,class_mode:modes.get(groupKey??'')??'',think_note:'',week_test:''};
      rows.set(id,row);
    }
    const value=field.value.trim();
    if(field.kind==='think_note')row.think_note=NUMERIC.test(value)?value+'%':value;
    else row.week_test=NUMERIC.test(value)?value+'점':value;
  }
  return [...rows.values()];
}

const narrativeOf=(report:CismReport,name:string):CismField|undefined=>report.fields.find(f=>f.kind==='narrative'&&f.source.fieldName===name);

function sections(reports:readonly CismReport[],monthly:boolean):CismSection[]{
  return CISM_NARRATIVES.map(name=>{
    const [group,slot]=NARRATIVE_SLOTS[name];
    const found=reports.map(r=>({report:r,field:narrativeOf(r,name)}));
    const title=(found.find(f=>f.field)?.field?.label.trim()||name).slice(0,120);
    // A week: the text itself. A month: each non-empty weekly text under its week label, texts separated by a blank line.
    const text=monthly
      ?found.filter(f=>f.field?.value.trim()).map(f=>`[${f.report.weekLabel}]\n${f.field!.value.trim()}`).join('\n\n')
      :found[0]?.field?.value??'';
    return {group,slot,title,text};
  });
}

/** One week: `period` is `periodStart ~ periodEnd`, `week_label` the report's week label. */
export function cismWeekToStudyqInput(report:CismReport):CismStudyqInput{
  return {week_no:cismWeekNumber(report.weekLabel),week_label:report.weekLabel,teacher:report.teacherLabel,period:`${report.periodStart} ~ ${report.periodEnd}`,
    chart_data:chartRows(report,''),report_list:sections([report],false)};
}

/** One month: every week of `cismMonthDates(month)` (the caller read `reports` in that order), the groups
 * prefixed with their week number, the narratives of the weeks joined per slot. */
export function cismMonthToStudyqInput(month:string,reports:readonly CismReport[]):CismStudyqInput{
  if(!reports.length||reports.length!==cismMonthDates(month).length)throw Error('cism_scope_conflict');
  if(reports.some(r=>r.teacherKey!==reports[0]!.teacherKey))throw Error('cism_scope_conflict');
  const weeks=reports.map(r=>cismWeekNumber(r.weekLabel));
  return {week_no:weeks[0]!,week_label:`${month} 월간 CISM (${reports.length}주)`,teacher:reports[0]!.teacherLabel,period:month,
    chart_data:reports.flatMap((r,i)=>chartRows(r,`[${weeks[i]}주차] `)),report_list:sections(reports,true)};
}
