import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {lstat,mkdir,mkdtemp,realpath,rm,symlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {OfficialReportPdfService,officialReportPdfGetSchema,type CourseBookReportBackend} from '../src/official-report-pdf.ts';
import type {StudyqRunner} from '../src/studyq-runner.ts';
import {projectCourseBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/courseBookReport.ts';
import {CISM_NARRATIVES,cismMonthDates,cismWeek,type CismField,type CismReport} from '../../../../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts';
import {cismMonthToStudyqInput,cismWeekNumber,cismWeekToStudyqInput} from '../../../../../apps/academy/dc-gang-a/src/lms/cismReportAdapter.ts';
import {projectDiagReport,projectFormativeReport,projectGangaReport,projectNaScoreReport,projectNewDiagReport,projectSmartBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/reportViewClient.ts';
import {writeForbiddenTransport} from './network-deny.ts';

// academy_official_report_pdf_get (src/official-report-pdf.ts) with an injected backend and runner:
// the output root and path rules, the answer and the refusals. Invented values only.
const STUDENT='Invented Student',KEY='770011',COURSE='560013',BOOK='430019';
const report=projectCourseBookReport({block1:[{'1':'가우스 초등 1-1 Invented','2':'2040-01-01','3':'2040-03-01','4':STUDENT}],block2:[{'1':5,'3':82.5,'4':75,'5':'c'}],
  block5:Array.from({length:9},(_,i)=>({'1':i+1,'2':'t'+i,'3':10,'4':70,'5':80,'6':'03-01'})),
  bunit_list:[{'1':'Invented chapter','3':80,'4':70,exam_result:[{'1':1,'2':'Invented type','3':2,'4':1,'5':66,'6':'O'}]}],
  block9:[{'2':1,'3':2,'4':3,'5':4}],block10:[{'1':90},{'1':80},{'1':70},{'1':60}],block11:[{'2':71}],block12:[{'2':'u','3':3,'4':4,'5':70}]},
  {studentName:STUDENT,studentKey:KEY,courseKey:COURSE,bookNo:BOOK});
let base:string;
beforeAll(async()=>{base=await realpath(await mkdtemp(resolve(tmpdir(),'official-report-')));});
afterAll(async()=>{await rm(base,{recursive:true,force:true});});
const now=()=>new Date('2040-01-16T09:00:00.123Z');
function fixture(name:string,over:Partial<{answer:Awaited<ReturnType<CourseBookReportBackend['readCourseBookReportByName']>>;rendered:Awaited<ReturnType<StudyqRunner['renderCourseBook']>>}>={}){
  const calls={backend:[] as unknown[],runner:[] as {input:unknown;path:string}[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(input){calls.backend.push(input);
    return over.answer??{kind:'report',student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:BOOK,bindingSource:'historical_key_registry',report};}};
  const runner:StudyqRunner={async renderCourseBook(input,path){calls.runner.push({input,path});
    return over.rendered??{status:'rendered',pages:8,sha256:'a'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};},
    async assemblePrint(){throw Error('not_called');},async renderCism(){throw Error('not_called_for_course_book');},async renderReport(){throw Error('not_called_for_course_book');}};
  const root=resolve(base,name,'official-reports');
  return {calls,root,service:new OfficialReportPdfService(backend,runner,root,now)};
}

describe('academy_official_report_pdf_get',()=>{
  test('official_report_renders_course_book_under_the_owner_root',async()=>{
    const f=fixture('render');
    const answer=await f.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY}) as any;
    const path=resolve(f.root,'course_book-'+BOOK+'-20400116T090000Z.pdf');
    expect(answer).toMatchObject({status:'rendered',reportType:'course_book',outputPath:path,pages:8,parityOk:true,parityMismatches:0,parityRawRn:0,
      courseKey:COURSE,bookNo:BOOK,bindingSource:'historical_key_registry',academyWrites:0,
      reportSummary:{bookTitle:'가우스 초등 1-1 Invented',period:'2040-01-01 ~ 2040-03-01',myAverage:82.5,allAverage:75,chapterCount:1},
      truncation:{testsDropped:1,chaptersDropped:0,questionsDropped:0,munitsDropped:0}});
    expect(f.calls.backend).toEqual([{displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY}]);
    expect(f.calls.runner).toEqual([{input:report.studyqInput,path}]);
    expect(path).not.toContain('Invented');
    const stat=await lstat(f.root);expect(stat.isDirectory()&&(stat.mode&0o777)===0o700).toBe(true);
    // studentName and params.bookLabel name the student and the book.
    await f.service.get({reportType:'course_book',studentName:STUDENT,params:{bookLabel:'가우스 1-1'},outputPath:'mine.pdf'});
    expect(f.calls.backend.at(-1)).toEqual({displayName:STUDENT,bookLabel:'가우스 1-1'});
    expect(f.calls.runner.at(-1)!.path).toBe(resolve(f.root,'mine.pdf'));
  });

  test('official_report_output_path_confined_and_never_overwritten',async()=>{
    const f=fixture('paths');
    await mkdir(f.root,{recursive:true,mode:0o700});
    await writeFile(resolve(f.root,'taken.pdf'),'x');
    await symlink(base,resolve(f.root,'escape'));
    for(const [outputPath,code] of [['report.txt','output_path_invalid'],['../report.pdf','output_path_invalid'],['sub/../report.pdf','output_path_invalid'],
      [resolve(base,'outside.pdf'),'output_path_invalid'],['escape/report.pdf','output_path_invalid'],['missing/report.pdf','output_path_invalid'],['taken.pdf','output_exists']] as const){
      expect({outputPath,answer:await f.service.get({reportType:'course_book',displayName:STUDENT,outputPath})}).toMatchObject({outputPath,answer:{status:'unavailable',reasonCode:code}});
    }
    // Refused before any LMS request.
    expect(f.calls.backend).toEqual([]);expect(f.calls.runner).toEqual([]);
    // An output root that is a symbolic link is refused.
    const target=resolve(base,'paths-target');await mkdir(target);
    const linked=new OfficialReportPdfService(fixture('x').service.backend,fixture('x').service.runner,resolve(base,'paths-link'),now);
    await symlink(target,resolve(base,'paths-link'));
    expect(await linked.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'unavailable',reasonCode:'output_root_unqualified'});
  });

  test('official_report_refusals_and_interview',async()=>{
    const refused=fixture('refused',{rendered:{status:'refused',code:'input_invalid',detail:'range:bunit_list[0].questions[0].type'}});
    expect(await refused.service.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'refused',reasonCode:'studyq_input_invalid',studyqDetail:'range:bunit_list[0].questions[0].type'});
    const interview=fixture('interview',{answer:{kind:'interview_required',student:{studentId:KEY,displayName:STUDENT},bindings:[{bookLabel:'A',bookNo:'1'},{bookLabel:'B',bookNo:'2'}],
      reason:'multiple_book_bindings_select_one',observedAt:'2040-01-16T09:00:00.000Z'}});
    expect(await interview.service.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'interview_required',bindings:[{bookLabel:'A'},{bookLabel:'B'}]});
    expect(interview.calls.runner).toEqual([]);
    const conflict=fixture('conflict',{answer:{kind:'report',student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:'430099',report}});
    expect(await conflict.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(await new OfficialReportPdfService(null,null,resolve(base,'none')).get({reportType:'course_book',displayName:STUDENT}))
      .toMatchObject({status:'unavailable',reasonCode:'official_report_backend_not_configured',academyWrites:0});
    expect(await new OfficialReportPdfService(fixture('y').service.backend,null,resolve(base,'none')).get({reportType:'course_book',displayName:STUDENT}))
      .toMatchObject({status:'unavailable',reasonCode:'studyq_runner_not_configured'});
    // Input refused before anything runs.
    for(const bad of [{reportType:'cism',displayName:STUDENT},{reportType:'course_book'},{reportType:'course_book',displayName:STUDENT,studentName:'Other Invented'},
      {reportType:'course_book',displayName:STUDENT,courseKey:COURSE},{reportType:'course_book',displayName:STUDENT,params:{unknown:true}},
      {reportType:'course_book',displayName:STUDENT,courseKey:'0',bookNo:BOOK,studentKey:KEY}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
  });
});

// reportType 'cism': the owner's weekly or monthly CISM, read-only, through the adapter to the strict `cism` input.
const field=(kind:CismField['kind'],fieldName:string,value:string,extra:Partial<CismField>={}):CismField=>({fieldRef:fieldName+value,kind,label:'Invented '+fieldName,value,studentName:null,groupName:null,source:{fieldName},...extra});
const student=(kind:'think_note'|'week_test',name:string,studentKey:string,groupKey:string,groupName:string,value:string):CismField=>
  field(kind,kind,value,{studentName:name,groupName,source:{fieldName:kind,studentKey,membershipKey:'9'+studentKey,groupKey}});
function cismReport(date:string,weekNo:number,text:(name:string)=>string,over:Partial<CismReport>={}):CismReport{
  const {periodStart,periodEnd}=cismWeek(date);
  const mode=(groupKey:string,groupName:string,value:string)=>field('class_mode','class_mode',value,{groupName,source:{fieldName:'class_mode',studentKey:'9101',membershipKey:'9201',groupKey}});
  const fields:CismField[]=[
    ...['i_1','i_2','s_1','s_2','s_3','s_4','m_1','m_2'].map(n=>field('narrative',n,text(n),{label:'Invented '+n+' title'})),
    mode('1','Invented Group One','3'),student('think_note','Invented Learner A','9101','1','Invented Group One','85'),student('week_test','Invented Learner A','9101','1','Invented Group One','90'),
    student('think_note','Invented Learner B','9102','1','Invented Group One',''),student('week_test','Invented Learner B','9102','1','Invented Group One','70.5'),
    mode('2','Invented Group Two','0'),student('think_note','Invented Learner C','9103','2','Invented Group Two','n/a'),student('week_test','Invented Learner C','9103','2','Invented Group Two',''),
  ];
  return {date,periodStart,periodEnd,weekLabel:`2040-${weekNo} 주차 (1월15일~1월21일)`,teacherLabel:'Invented Teacher',teacherKey:'90001',wireSha256:'2'.repeat(64),reportSha256:'3'.repeat(64),
    observedAt:'2040-01-16T00:00:00Z',fields,sourceMode:'live_source',pdfGenerated:false,...over};
}
function cismFixture(name:string,reports:(date:string)=>CismReport,rendered?:Awaited<ReturnType<StudyqRunner['renderCism']>>){
  const calls={dates:[] as string[],runner:[] as {input:any;path:string}[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(){throw Error('not_called_for_cism');},async readCismReport(date){calls.dates.push(date);return reports(date);}};
  const runner:StudyqRunner={async renderCourseBook(){throw Error('not_called_for_cism');},
    async assemblePrint(){throw Error('not_called');},async renderReport(){throw Error('not_called_for_cism');},async renderCism(input,path){calls.runner.push({input,path});return rendered??{status:'rendered',pages:3,sha256:'b'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
  const root=resolve(base,name,'official-reports');
  return {calls,root,service:new OfficialReportPdfService(backend,runner,root,now)};
}
// The CISM page of the protocol tests' fictional source, with the pinned handler text of the fixtures.
const cismHandlers=readFileSync(resolve(import.meta.dir,'../../../../../apps/academy/dc-gang-a/tests/lms/fixtures/cism-handlers.js.txt'),'utf8');
function fictionalCismHtml(date:string,narrative:string){
  const {periodStart,periodEnd}=cismWeek(date),a=new Date(periodStart),b=new Date(periodEnd);
  return `<h1>주간 교무회의 CISM 작성</h1><form id="theDateForm"><input name="reqCmd" value="CismChartMain"><input id="the_date" value="${date}"><input id="tutor_pri_no" value="90001"><input id="teacher_name" value="Fictional Teacher"><label>2040-42 주차 (${a.getUTCMonth()+1}월${a.getUTCDate()}일~${b.getUTCMonth()+1}월${b.getUTCDate()}일)</label></form><label id="grp_name_0">Fictional Group</label><select id="class_mode_0" onchange="setCismData('class_mode',0,90002,90003,1);"><option value="0">-선택-</option><option value="3">상</option><option value="2">중</option><option value="1">하</option></select><table><tr><td>Fictional Student</td><td><input id="think_note_0" onfocusout="setCismData('think_note',0,90002,90003,1);" value=""></td><td><input id="week_test_0" onfocusout="setCismData('week_test',0,90002,90003,1);" value=""></td></tr><tr><th id="think_note_0">0</th></tr></table>${CISM_NARRATIVES.map(n=>`<label>${n}</label><textarea name="${n}" id="${n}" onfocusout="javascript:onTextareaFocusOut(this,'${n}');">${n==='i_1'?narrative:''}</textarea>`).join('')}<script>${cismHandlers}</script>`;
}
const python=(code:string,input:unknown)=>{
  const src=resolve(import.meta.dir,'../../../../studyq-linux/src');
  const run=spawnSync('/usr/bin/python3',['-I','-B','-c',code,src],{input:JSON.stringify(input),encoding:'utf8'});
  return {status:run.status,stdout:run.stdout.trim(),stderr:run.stderr};
};
// The real validator and the real planner of modules/studyq-linux, over a fake text meter and the real banner sizes.
const VALIDATE='import sys,json; sys.path.insert(0,sys.argv[1]); from studyq_linux.schema import validate_cism; d=validate_cism(json.load(sys.stdin)); print(json.dumps({"rows":len(d["chart_data"]),"sections":len(d["report_list"])}))';
const PLAN=`import sys,json; sys.path.insert(0,sys.argv[1]); from studyq_linux.schema import validate_cism; from studyq_linux.renderers.cism import plan_cism, required_cism_assets; from studyq_linux.course_book import PlanContext
sizes={"image/cism/cism_1.jpg":(2481,3508),"image/cism/cism_2.jpg":(2481,3509),"image/cism/cism_3.jpg":(2481,3508)}
for n,w in {"4_1":330,"4_2":396,"5_1":319,"5_2":352,"5_3":318,"5_4":431,"6_1":405,"6_2":386}.items(): sizes["image/cism/"+n+".jpg"]=(w,47)
d=validate_cism(json.load(sys.stdin)); plan=plan_cism(d,PlanContext(lambda t,s:len(t)*s*0.6,lambda r:sizes[r])); assert all(r in sizes for r in required_cism_assets(d))
print(json.dumps({"pages":len(plan.pages),"fields":sum(len(p.fields) for p in plan.pages)}))`;

describe('academy_official_report_pdf_get reportType cism',()=>{
  const week=(date:string)=>cismReport(date,3,n=>n==='i_1'?'First line.\r\nSecond line.':n==='s_2'?'Section text':'');
  test('official_report_cism_week_renders_the_owner_week_under_the_owner_root',async()=>{
    const f=cismFixture('cism-week',week);
    const answer=await f.service.get({reportType:'cism',params:{scope:'week',date:'2040-01-16'}}) as any;
    const path=resolve(f.root,'cism-week-2040-01-16-20400116T090000Z.pdf');
    expect(answer).toMatchObject({status:'rendered',reportType:'cism',scope:'week',date:'2040-01-16',outputPath:path,pages:3,parityOk:true,parityMismatches:0,parityRawRn:0,academyWrites:0,weeks:1,
      reportSummary:{studentRows:3,filledSections:2}});
    expect(JSON.stringify(answer)).not.toContain('Invented Learner');
    expect(f.calls.dates).toEqual(['2040-01-16']);
    const input=f.calls.runner[0]!.input;
    expect(f.calls.runner[0]!.path).toBe(path);
    expect(input).toMatchObject({week_no:3,week_label:'2040-3 주차 (1월15일~1월21일)',teacher:'Invented Teacher',period:'2040-01-15 ~ 2040-01-21'});
    expect(input.chart_data).toEqual([
      {group:'Invented Group One',student:'Invented Learner A',class_mode:'A+',think_note:'85%',week_test:'90점'},
      {group:'Invented Group One',student:'Invented Learner B',class_mode:'A+',think_note:'',week_test:'70.5점'},
      {group:'Invented Group Two',student:'Invented Learner C',class_mode:'',think_note:'n/a',week_test:''}]);
    expect(input.report_list.map((s:any)=>[s.group,s.slot])).toEqual([[4,1],[4,2],[5,1],[5,2],[5,3],[5,4],[6,1],[6,2]]);
    expect(input.report_list[0]).toEqual({group:4,slot:1,title:'Invented i_1 title',text:'First line.\r\nSecond line.'});
    expect(input.report_list[1].text).toBe('');
    // The default date is today (Asia/Seoul); the default name holds no personal value.
    const today=await f.service.get({reportType:'cism',params:{scope:'week'}}) as any;
    expect(today).toMatchObject({status:'rendered',date:'2040-01-16',outputPath:path});
  });

  test('official_report_cism_month_reads_every_week_and_merges_the_texts',async()=>{
    const dates=cismMonthDates('2040-01'),weekNos=new Map(dates.map((d,i)=>[d,i+1]));
    const f=cismFixture('cism-month',d=>cismReport(d,weekNos.get(d)!,n=>n==='i_1'?'Text of '+weekNos.get(d):n==='m_1'&&weekNos.get(d)===2?'Only week two':''));
    const answer=await f.service.get({reportType:'cism',params:{scope:'month',month:'2040-01'},outputPath:'month.pdf'}) as any;
    expect(answer).toMatchObject({status:'rendered',scope:'month',month:'2040-01',outputPath:resolve(f.root,'month.pdf'),weeks:dates.length,academyWrites:0});
    expect(f.calls.dates).toEqual(dates);
    const input=f.calls.runner[0]!.input;
    expect(input).toMatchObject({week_no:1,week_label:`2040-01 월간 CISM (${dates.length}주)`,teacher:'Invented Teacher',period:'2040-01'});
    expect(input.chart_data).toHaveLength(3*dates.length);
    expect(input.chart_data[0].group).toBe('[1주차] Invented Group One');
    expect(input.chart_data.at(-1).group).toBe(`[${dates.length}주차] Invented Group Two`);
    const first=input.report_list[0].text as string;
    expect(first.split('\n\n')).toHaveLength(dates.length);
    expect(first.startsWith('[2040-1 주차 (1월15일~1월21일)]\nText of 1\n\n[2040-2 주차')).toBe(true);
    expect(input.report_list.find((s:any)=>s.group===6&&s.slot===1).text).toBe('[2040-2 주차 (1월15일~1월21일)]\nOnly week two');
    // The default name of a month holds the month.
    const named=await f.service.get({reportType:'cism',params:{scope:'month',month:'2040-01'}}) as any;
    expect(named.outputPath).toBe(resolve(f.root,'cism-month-2040-01-20400116T090000Z.pdf'));
  });

  test('official_report_cism_input_is_accepted_by_the_real_studyq_validator_and_planner',()=>{
    const dates=cismMonthDates('2040-01');
    const month=cismMonthToStudyqInput('2040-01',dates.map((d,i)=>cismReport(d,i+1,n=>'Invented text of '+n+'\n'.repeat(2)+'line '.repeat(40))));
    const weekInput=cismWeekToStudyqInput(cismReport('2040-01-16',3,n=>'Invented '+n));
    for(const input of [weekInput,month]){
      const checked=python(VALIDATE,input);
      expect({status:checked.status,stderr:checked.stderr}).toEqual({status:0,stderr:''});
      expect(JSON.parse(checked.stdout)).toMatchObject({sections:8});
      const planned=JSON.parse(python(PLAN,input).stdout);
      expect(planned.pages).toBeGreaterThanOrEqual(2);
    }
    expect(JSON.parse(python(VALIDATE,weekInput).stdout).rows).toBe(3);
    // A key the strict schema does not take is refused by the same validator.
    const refused=python(VALIDATE,{...weekInput,surplus:1});
    expect(refused.status).not.toBe(0);
  });

  test('official_report_cism_refuses_bad_params_and_unqualified_reports',async()=>{
    for(const bad of [{reportType:'cism'},{reportType:'cism',params:{}},{reportType:'cism',params:{scope:'year'}},{reportType:'cism',params:{scope:'month'}},
      {reportType:'cism',params:{scope:'month',month:'2040-13'}},{reportType:'cism',params:{scope:'month',month:'2040-01',date:'2040-01-16'}},
      {reportType:'cism',params:{scope:'week',month:'2040-01'}},{reportType:'cism',params:{scope:'week',bookLabel:'x'}},{reportType:'cism',params:{scope:'week',extra:1}},
      {reportType:'cism',params:{scope:'week'},displayName:STUDENT},{reportType:'cism',params:{scope:'week'},studentName:STUDENT},
      {reportType:'cism',params:{scope:'week'},courseKey:COURSE,bookNo:BOOK,studentKey:KEY},{reportType:'cism',params:{scope:'week'},bookNo:BOOK},
      {reportType:'course_book',displayName:STUDENT,params:{scope:'week'}},{reportType:'course_book',displayName:STUDENT,params:{month:'2040-01'}}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
    for(const ok of [{reportType:'cism',params:{scope:'week'}},{reportType:'cism',params:{scope:'week',date:'today'}},{reportType:'cism',params:{scope:'month',month:'2040-01'},outputPath:'a.pdf'}])
      expect({ok,parsed:officialReportPdfGetSchema.safeParse(ok).success}).toEqual({ok,parsed:true});
    const f=cismFixture('cism-bad',week);
    expect(await f.service.get({reportType:'cism',params:{scope:'week',date:'next tuesday'}})).toMatchObject({status:'unavailable',reasonCode:'date_form_unsupported_use_yyyy_mm_dd_today_yesterday_or_month_day',academyWrites:0});
    expect(await f.service.get({reportType:'cism',params:{scope:'week'},outputPath:'../x.pdf'})).toMatchObject({status:'unavailable',reasonCode:'output_path_invalid'});
    expect(f.calls.dates).toEqual([]);
    // A week label without a week number, a student field without a name, a group with no key.
    const noWeek=cismFixture('cism-noweek',d=>cismReport(d,3,()=>'',{weekLabel:'2040-0 주차 (1월15일~1월21일)'}));
    expect(await noWeek.service.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'cism_week_number_unqualified',date:'2040-01-16'});
    expect(noWeek.calls.runner).toEqual([]);
    for(const label of ['Invented 주차','2040-54 주차 x','2040- 주차'])expect(()=>cismWeekNumber(label)).toThrow('cism_week_number_unqualified');
    expect(cismWeekNumber('2040-53 주차 (12월)')).toBe(53);
    const nameless=cismFixture('cism-nameless',d=>{const r=cismReport(d,3,()=>'');r.fields.find(x=>x.kind==='think_note')!.studentName=null;return r;});
    expect(await nameless.service.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'cism_field_identity_missing'});
    const keyless=cismFixture('cism-keyless',d=>{const r=cismReport(d,3,()=>'');const x=r.fields.find(y=>y.kind==='week_test')!;x.groupName='';delete x.source.groupKey;return r;});
    expect(await keyless.service.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'cism_field_identity_missing'});
    // A refusal of the executor is answered as it is; no backend and no runner are answered as unavailable.
    const refused=cismFixture('cism-refused',week,{status:'refused',code:'input_invalid',detail:'range:report_list[0].text'});
    expect(await refused.service.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'refused',reasonCode:'studyq_input_invalid',studyqDetail:'range:report_list[0].text'});
    expect(await new OfficialReportPdfService(null,null,resolve(base,'none')).get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'official_report_backend_not_configured'});
    const noPort=new OfficialReportPdfService({async readCourseBookReportByName(){throw Error('x');}},refused.service.runner,resolve(base,'noport'),now);
    expect(await noPort.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'cism_source_port_unavailable'});
  });

  test('official_report_cism_reads_only_read_class_routes_i1_write_forbidden',async()=>{
    // jsdom and the HTTP client load here, so a host without the module's dependencies fails only this test.
    const {AcademyHttpClient}=await import('../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
    const {NativeDailyRecordBackend}=await import('../src/native-dailyrecord-backend.ts');
    const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
    const principal='<a href="/logout">Fictional Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="90001"></form><form name="form1"><input name="g_pri_no" value="90001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
    let calls=0;
    const guard=writeForbiddenTransport(url=>{calls++;
      if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
      const date=new URL(url,'https://lms.invalid').searchParams.get('the_date')!;
      return new Response(fictionalCismHtml(date,'Invented narrative'),{headers:{'content-type':'text/html; charset=utf-8'}});});
    const admission={teacherId:'90001',teacherLabel:'Fictional Teacher',date:'2040-01-04',students:[{studentId:'9101',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
    const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
    const backend=new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'}));
    const runner:StudyqRunner={async renderCourseBook(){throw Error('x');},async assemblePrint(){throw Error('not_called');},async renderReport(){throw Error('x');},async renderCism(input,path){seen.push(input);return {status:'rendered',pages:2,sha256:'c'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
    const seen:any[]=[];
    const service=new OfficialReportPdfService(backend as never,runner,resolve(base,'cism-i1','official-reports'),now);
    const week=await service.get({reportType:'cism',params:{scope:'week',date:'2040-10-15'}}) as any;
    expect(week).toMatchObject({status:'rendered',academyWrites:0});
    expect(guard.forbidden()).toEqual([]);
    expect(guard.requests.map(r=>r.role)).toEqual(['login','login','read','read']);
    expect(guard.requests.at(-1)).toEqual({route:'/servlet/controller.cct.common.CourseCommonServlet CismChartMain',role:'read'});
    expect(seen[0]).toMatchObject({week_no:42,teacher:'Fictional Teacher',period:'2040-10-14 ~ 2040-10-20'});
    expect(seen[0].chart_data).toEqual([{group:'Fictional Group',student:'Fictional Student',class_mode:'',think_note:'',week_test:''}]);
    expect(seen[0].report_list[0].text).toBe('Invented narrative');
    expect(python(VALIDATE,seen[0]).status).toBe(0);
  });
});

// reportType 'diag', 'new_diag', 'formative': one read-only POST of ReportViewServlet (specs/academy-mcp/direct-key-lookup.md R8),
// projected to the strict input of specs/studyq-linux/reports-batch1.md. Invented names, keys and values only.
const TESTING='901',CLASS='560077',PAPER='4401',APPLICANT='Invented Applicant';
const diagBlocks=(student=STUDENT)=>({block1:[{'1':'Invented diagnostic','2':'2040-01-10','3':student,'4':'4','5':72.5,'6':61,'7':'Line one.\\r\\nLine two.'}],
  block2:[{'1':'Invented area A','2':80,'3':60,'4':'green','5':'nice'},{'1':'Invented area B','2':'55','3':60,'4':'yellow','5':'base'}],
  block3:[{'1':2,'2':40},{'1':3,'2':55.5},{'1':4,'2':70},{'1':1,'2':90}],
  block4:[{'1':1,'2':'Invented area A','3':'상','4':66,'5':'O'},{'1':2,'2':'Invented area B','3':'중','4':'50.5','5':'X'},{'1':3,'2':'Invented area A','3':'하','4':90,'5':'O'}]});
const newDiagBlocks=()=>{const raw=diagBlocks(APPLICANT);raw.block1[0]={...raw.block1[0]!,'8':'APP-7','9':'INTP'} as never;return raw;};
const formativeNamed=(student=STUDENT)=>({title:'Invented formative',student,test_date:'2040-01-12',my_score:80,avg_score:'66.5',rank_signal:3,comment:'',
  units:[{name:'Invented unit 1',my_score:70,average:60},{name:'Invented unit 2',my_score:90,average:65}],
  difficulty:{top:{questions:1,score:50},high:{questions:2,score:60},mid:{questions:3,score:70},low:{questions:4,score:80}},
  questions:[{no:1,unit:'Invented unit 1',difficulty:'상',correct_rate:40,correct:'O'},{no:2,unit:'Invented unit 2',difficulty:'하',correct_rate:77,correct:false}]});
const VALIDATE_REPORT=(type:string)=>`import sys,json; sys.path.insert(0,sys.argv[1]); from studyq_linux.schema import validate_${type} as v; from studyq_linux.render import REPORTS
from studyq_linux.course_book import PlanContext
d=v(json.load(sys.stdin)); plan=REPORTS["${type}"][1](d,PlanContext(lambda t,s:len(t)*s*0.6,lambda r:(2080,415) if "/9p/" in r else (37,37) if r.endswith(".png") else (100,80)))
print(json.dumps({"pages":len(plan.pages),"questions":len(d.get("questions",[]))}))`;
function viewFixture(name:string,raw:()=>Record<string,unknown>,rendered?:Awaited<ReturnType<StudyqRunner['renderReport']>>){
  const calls={student:[] as unknown[],applicant:[] as unknown[],runner:[] as {type:string;input:any;path:string}[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(){throw Error('not_called');},
    async readStudentReportView(input){calls.student.push(input);
      if(input.studentKey!==undefined&&input.studentKey!==KEY)throw Error('official_report_scope_conflict');
      return {student:{studentId:KEY,displayName:STUDENT},raw:raw()};},
    async readNewStudentReportView(input){calls.applicant.push(input);return {raw:raw()};}};
  const runner:StudyqRunner={async renderCourseBook(){throw Error('not_called');},async renderCism(){throw Error('not_called');},async assemblePrint(){throw Error('not_called');},
    async renderReport(type,input,path){calls.runner.push({type,input,path});return rendered??{status:'rendered',pages:9,sha256:'d'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
  const root=resolve(base,name,'official-reports');
  return {calls,root,service:new OfficialReportPdfService(backend,runner,root,now)};
}

describe('academy_official_report_pdf_get reportType diag, new_diag, formative',()=>{
  test('official_report_diag_renders_the_projected_view_under_the_owner_root',async()=>{
    const f=viewFixture('diag',()=>diagBlocks());
    const answer=await f.service.get({reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}}) as any;
    const path=resolve(f.root,'diag-'+TESTING+'-20400116T090000Z.pdf');
    expect(answer).toMatchObject({status:'rendered',reportType:'diag',outputPath:path,pages:9,parityOk:true,parityMismatches:0,parityRawRn:0,academyWrites:0,
      student:{studentId:KEY,displayName:STUDENT},testingNo:TESTING,classNo:CLASS,reportSummary:{title:'Invented diagnostic',testDate:'2040-01-10',questions:3}});
    expect(f.calls.student).toEqual([{view:'DiagReportView',displayName:STUDENT,testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}]);
    const {type,input}=f.calls.runner[0]!;
    expect(type).toBe('diag');
    expect(input).toEqual({title:'Invented diagnostic',student:STUDENT,test_date:'2040-01-10',grade:4,total_score:72.5,avg_score:61,comment:'Line one.\\r\\nLine two.',
      areas:[{name:'Invented area A',score:80,average:60,signal:'green',rating:'nice'},{name:'Invented area B',score:55,average:60,signal:'yellow',rating:'base'}],
      difficulty:{top:{questions:2,score:40},high:{questions:3,score:55.5},mid:{questions:4,score:70},low:{questions:1,score:90}},
      questions:[{no:1,area:'Invented area A',difficulty:'상',correct_rate:66,correct:true},{no:2,area:'Invented area B',difficulty:'중',correct_rate:50.5,correct:false},
        {no:3,area:'Invented area A',difficulty:'하',correct_rate:90,correct:true}]});
    // formative: FaView, studentName, an explicit studentKey that is the student's.
    const g=viewFixture('formative',()=>formativeNamed());
    const fa=await g.service.get({reportType:'formative',studentName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS},outputPath:'fa.pdf'}) as any;
    expect(fa).toMatchObject({status:'rendered',reportType:'formative',outputPath:resolve(g.root,'fa.pdf')});
    expect(g.calls.student).toEqual([{view:'FaView',displayName:STUDENT,studentKey:KEY,testingNo:TESTING,classNo:CLASS}]);
    expect(g.calls.runner[0]!.input).toMatchObject({avg_score:66.5,units:[{name:'Invented unit 1'},{name:'Invented unit 2'}],questions:[{correct:true},{correct:false}]});
    // new_diag: NewStudentReportView by paper and apply code, no student lookup.
    const h=viewFixture('new-diag',newDiagBlocks);
    const nd=await h.service.get({reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7'}}) as any;
    expect(nd).toMatchObject({status:'rendered',reportType:'new_diag',paperNo:PAPER,outputPath:resolve(h.root,'new_diag-'+PAPER+'-20400116T090000Z.pdf')});
    expect(nd.student).toBeUndefined();
    expect(h.calls.applicant).toEqual([{paperNo:PAPER,applyCode:'APP-7'}]);
    expect(h.calls.runner[0]!.input).toMatchObject({student:APPLICANT,apply_code:'APP-7',mbti:'INTP'});
    await h.service.get({reportType:'new_diag',displayName:APPLICANT,params:{paperNo:PAPER,applyCode:'APP-7',collegeKey:'88002'},outputPath:'nd2.pdf'});
    expect(h.calls.applicant.at(-1)).toEqual({paperNo:PAPER,applyCode:'APP-7',collegeKey:'88002'});
  });

  test('official_report_report_view_params_are_validated_before_any_request',async()=>{
    for(const bad of [{reportType:'diag',params:{testingNo:TESTING,classNo:CLASS}},{reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING}},
      {reportType:'diag',displayName:STUDENT,params:{classNo:CLASS}},{reportType:'diag',displayName:STUDENT,courseKey:COURSE,params:{testingNo:TESTING,classNo:CLASS}},
      {reportType:'formative',displayName:STUDENT,bookNo:BOOK,params:{testingNo:TESTING,classNo:CLASS}},{reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,bookLabel:'x'}},
      {reportType:'diag',displayName:STUDENT,params:{testingNo:'0',classNo:CLASS}},{reportType:'formative',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-1-31'}},
      {reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,paperNo:PAPER}},{reportType:'diag',displayName:STUDENT,studentName:'Other Invented',params:{testingNo:TESTING,classNo:CLASS}},
      {reportType:'new_diag',params:{paperNo:PAPER}},{reportType:'new_diag',params:{applyCode:'APP-7'}},{reportType:'new_diag',params:{paperNo:PAPER,applyCode:''}},
      {reportType:'new_diag',params:{paperNo:PAPER,applyCode:'a'.repeat(65)}},{reportType:'new_diag',params:{paperNo:PAPER,applyCode:' APP'}},
      {reportType:'new_diag',studentKey:KEY,params:{paperNo:PAPER,applyCode:'APP-7'}},{reportType:'new_diag',courseKey:COURSE,params:{paperNo:PAPER,applyCode:'APP-7'}},
      {reportType:'new_diag',bookNo:BOOK,params:{paperNo:PAPER,applyCode:'APP-7'}},{reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7',testingNo:TESTING}},
      {reportType:'course_book',displayName:STUDENT,params:{testingNo:TESTING}},{reportType:'cism',params:{scope:'week',paperNo:PAPER}},{reportType:'smart_book',displayName:STUDENT}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
    for(const ok of [{reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},{reportType:'formative',studentName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}},
      {reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7'}},{reportType:'new_diag',displayName:APPLICANT,params:{paperNo:PAPER,applyCode:'APP-7',collegeKey:'88001'}}])
      expect({ok,parsed:officialReportPdfGetSchema.safeParse(ok).success}).toEqual({ok,parsed:true});
    const f=viewFixture('view-bad',()=>diagBlocks());
    expect(await f.service.get({reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS},outputPath:'../x.pdf'})).toMatchObject({status:'unavailable',reasonCode:'output_path_invalid'});
    expect(f.calls.student).toEqual([]);
    // A student key that is not the named student's refuses with no runner call.
    expect(await f.service.get({reportType:'diag',displayName:STUDENT,studentKey:'770099',params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(f.calls.runner).toEqual([]);
  });

  test('official_report_report_view_refuses_another_student_and_unqualified_payloads',async()=>{
    const other=viewFixture('view-other',()=>diagBlocks('Another Invented'));
    expect(await other.service.get({reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    const applicant=viewFixture('view-applicant',newDiagBlocks);
    expect(await applicant.service.get({reportType:'new_diag',displayName:'Another Invented',params:{paperNo:PAPER,applyCode:'APP-7'}})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    for(const raw of [()=>({block1:[]}),()=>({...diagBlocks(),block3:[{'1':1,'2':1}]}),()=>({...diagBlocks(),block2:{}}),()=>({...diagBlocks(),block1:'x'})]){
      const f=viewFixture('view-unq',raw);
      expect(await f.service.get({reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'report_view_unqualified'});
      expect(f.calls.runner).toEqual([]);
      await rm(f.root,{recursive:true,force:true});
    }
    // Zero invented rows: an empty area block stays empty and the strict validator refuses it.
    const empty=viewFixture('view-empty',()=>({...diagBlocks(),block2:[]}));
    await empty.service.get({reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}});
    expect(empty.calls.runner[0]!.input.areas).toEqual([]);
    expect(python(VALIDATE_REPORT('diag'),empty.calls.runner[0]!.input).status).not.toBe(0);
    const refused=viewFixture('view-refused',()=>diagBlocks(),{status:'refused',code:'input_invalid',detail:'range:areas'});
    expect(await refused.service.get({reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'refused',reasonCode:'studyq_input_invalid',studyqDetail:'range:areas'});
    const noPort=new OfficialReportPdfService({async readCourseBookReportByName(){throw Error('x');}},refused.service.runner,resolve(base,'view-noport'),now);
    expect(await noPort.get({reportType:'formative',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'report_view_source_port_unavailable'});
    expect(await new OfficialReportPdfService(null,null,resolve(base,'none')).get({reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7'}})).toMatchObject({status:'unavailable',reasonCode:'official_report_backend_not_configured'});
  });

  test('official_report_report_view_inputs_are_accepted_by_the_real_studyq_validators',()=>{
    for(const [type,input] of [['diag',projectDiagReport(diagBlocks())],['diag',projectDiagReport(projectDiagReport(diagBlocks()) as never)],
      ['new_diag',projectNewDiagReport(newDiagBlocks())],['formative',projectFormativeReport(formativeNamed())],
      ['formative',projectFormativeReport({block1:[{'1':'Invented formative','2':'2040-01-12','3':STUDENT,'4':80,'5':66.5,'6':'3','7':''}],block2:[{'1':'Invented unit 1','2':70,'3':60}],
        block3:[{'1':1,'2':50},{'1':0,'2':0},{'1':3,'2':70},{'1':4,'2':80}],block4:[{'1':1,'2':'Invented unit 1','3':'상','4':40,'5':'X'}]})]] as const){
      const checked=python(VALIDATE_REPORT(type),input);
      expect({type,status:checked.status,stderr:checked.stderr}).toEqual({type,status:0,stderr:''});
      expect(JSON.parse(checked.stdout).questions).toBe((input as any).questions.length);
    }
    // A projection copies values: a signal or a mark the renderer does not take reaches its validator and is refused there.
    const bad=projectDiagReport({...diagBlocks(),block2:[{'1':'Invented area A','2':80,'3':60,'4':'blue','5':'nice'}]});
    expect(bad.areas[0]!.signal).toBe('blue');
    expect(python(VALIDATE_REPORT('diag'),bad).status).not.toBe(0);
    expect(projectDiagReport({...diagBlocks(),block4:[{'1':1,'2':'a','3':'b','4':1,'5':'?'}]}).questions[0]!.correct).toBe('?');
  });

  test('official_report_report_views_read_only_read_class_routes_i1_write_forbidden',async()=>{
    const {AcademyHttpClient}=await import('../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
    const {NativeDailyRecordBackend}=await import('../src/native-dailyrecord-backend.ts');
    const {REPORT_VIEW_PATH}=await import('../../../../../apps/academy/dc-gang-a/src/lms/reportViewClient.ts');
    const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
    const principal='<a href="/logout">Fictional Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="90001"></form><form name="form1"><input name="g_pri_no" value="90001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
    const envelope=(raw:unknown)=>new Response('<body>SERVER_RET = OK<BR>'+JSON.stringify(raw)+'<END_DATA></body>',{headers:{'content-type':'text/html; charset=utf-8'}});
    const run=async(name:string,request:Record<string,unknown>,raw:unknown,page?:string)=>{
      let calls=0;const bodies:string[]=[];
      const guard=writeForbiddenTransport((url,init)=>{calls++;
        if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
        bodies.push(String(init?.body));return page!==undefined?new Response(page):envelope(raw);});
      const admission={teacherId:'90001',teacherLabel:'Fictional Teacher',date:'2040-01-04',students:[{studentId:KEY,displayName:STUDENT}],sourceBundleSha256:'b'.repeat(64)};
      const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
      const backend=new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'}));
      const seen:any[]=[];
      const runner:StudyqRunner={async renderCourseBook(){throw Error('x');},async renderCism(){throw Error('x');},async assemblePrint(){throw Error('x');},
        async renderReport(type,input,path){seen.push({type,input});return {status:'rendered',pages:9,sha256:'e'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
      const service=new OfficialReportPdfService(backend as never,runner,resolve(base,name,'official-reports'),now);
      return {answer:await service.get(request) as any,guard,bodies,seen,calls:()=>calls};
    };
    const diag=await run('i1-diag',{reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},diagBlocks());
    expect(diag.answer).toMatchObject({status:'rendered',academyWrites:0});
    expect(diag.guard.forbidden()).toEqual([]);
    expect(diag.guard.requests.map(r=>r.role)).toEqual(['login','login','read','read']);
    expect(diag.guard.requests.at(-1)).toEqual({route:REPORT_VIEW_PATH+' DiagReportView',role:'read'});
    expect(diag.bodies).toEqual(['p_process=DiagReportView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date=']);
    expect(python(VALIDATE_REPORT('diag'),diag.seen[0].input).status).toBe(0);
    const fa=await run('i1-fa',{reportType:'formative',displayName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}},formativeNamed());
    expect(fa.answer).toMatchObject({status:'rendered'});
    expect(fa.guard.forbidden()).toEqual([]);
    expect(fa.bodies).toEqual(['p_process=FaView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date=2040-01-31']);
    expect(python(VALIDATE_REPORT('formative'),fa.seen[0].input).status).toBe(0);
    const nd=await run('i1-nd',{reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7'}},newDiagBlocks());
    expect(nd.answer).toMatchObject({status:'rendered'});
    expect(nd.guard.forbidden()).toEqual([]);
    expect(nd.bodies).toEqual(['p_process=NewStudentReportView&p_p_no='+PAPER+'&p_apply_code=APP-7&clg_no=88001']);
    expect(python(VALIDATE_REPORT('new_diag'),nd.seen[0].input).status).toBe(0);
    // A student key that is not the named student's: refused before any report-view request.
    const wrong=await run('i1-wrong',{reportType:'diag',displayName:STUDENT,studentKey:'770099',params:{testingNo:TESTING,classNo:CLASS}},diagBlocks());
    expect(wrong.answer).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(wrong.calls()).toBe(0);
    // A login page in place of the view fails closed: no PDF.
    const loggedOut=await run('i1-login',{reportType:'formative',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},null,login.replace('name="userpwd"','name="userpwd" type="password"'));
    expect(loggedOut.answer).toMatchObject({status:'unavailable',reasonCode:'authentication_required'});
    expect(loggedOut.seen).toEqual([]);
    expect(loggedOut.guard.forbidden()).toEqual([]);
  });
});

// reportType 'ganga', 'na_score', 'smart_book': one read-only POST of ReportViewServlet (GaReportView, NaScoreResultView,
// SmartBookReportView; specs/academy-mcp/direct-key-lookup.md R8), projected to the strict input of
// specs/studyq-linux/reports-batch2.md. Invented names, keys and values only.
const BOOK_CODE='SB-2040.inv_01';
const gangaBlocks=(student=STUDENT)=>({block1:[{'1':'Invented Gang-A','2':'2040-01-11','3':student,'4':78,'5':'64.5','6':'4','7':'Line one.\\r\\nLine two.','8':'2'}],
  block2:[{'1':'Invented unit 1','2':70,'3':60},{'1':'Invented unit 2','2':'90','3':65}],
  block3:[{'1':1,'2':50},{'1':2,'2':60},{'1':3,'2':70},{'1':4,'2':80}],
  block4:[{'1':1,'2':'Invented unit 1','3':'상','4':40,'5':'O'},{'1':2,'2':'Invented unit 2','3':'하','4':'77.5','5':'X'}]});
const naScoreNamed=(student=STUDENT)=>({title:'Invented NA score',student,test_date:'2040-01-13',my_score:88,avg_score:70,rank_band:'2',comment:'',
  units:[{name:'Invented unit 1',my_score:80,average:61}],
  questions:[{no:1,unit:'Invented unit 1',difficulty:'중',correct_rate:55,correct:'O'},{no:2,unit:'Invented unit 1',difficulty:'상',correct_rate:35,correct:'x'}]});
const naScoreBlocks=(student=STUDENT)=>({block1:[{'1':'Invented NA score','2':'2040-01-13','3':student,'4':88,'5':70,'6':2,'7':''}],
  block2:[{'1':'Invented unit 1','2':80,'3':61}],block3:[{'1':1,'2':'Invented unit 1','3':'중','4':55,'5':'O'}]});
const smartBookBlocks=(student=STUDENT)=>({block1:[{'1':'Invented SmartBook','2':'2040-01-01','3':'2040-03-01','4':student}],block2:[{'1':5,'3':82.5,'4':75,'5':'c'}],
  block5:Array.from({length:3},(_,i)=>({'1':i+1,'2':'t'+i,'3':10,'4':70,'5':80,'6':'03-01'})),
  bunit_list:[{'1':'Invented chapter','3':80,'4':70,exam_result:[{'1':1,'2':'Invented type','3':2,'4':1,'5':66,'6':'O'},{'1':2,'2':'Invented type 2','3':'3','4':'2','5':40,'6':'X'}]}],
  block9:[{'2':1,'3':2,'4':3,'5':4}],block10:[{'1':90},{'1':80},{'1':70},{'1':60}],block11:[{'2':71}],block12:[{'2':'u','3':3,'4':4,'5':70}]});
function batch2Fixture(name:string,raw:()=>Record<string,unknown>,rendered?:Awaited<ReturnType<StudyqRunner['renderReport']>>){
  const calls={student:[] as unknown[],book:[] as unknown[],runner:[] as {type:string;input:any;path:string}[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(){throw Error('not_called');},
    async readStudentReportView(input){calls.student.push(input);
      if(input.studentKey!==undefined&&input.studentKey!==KEY)throw Error('official_report_scope_conflict');
      return {student:{studentId:KEY,displayName:STUDENT},raw:raw()};},
    async readSmartBookReportView(input){calls.book.push(input);
      if(input.studentKey!==undefined&&input.studentKey!==KEY)throw Error('official_report_scope_conflict');
      return {student:{studentId:KEY,displayName:STUDENT},raw:raw()};}};
  const runner:StudyqRunner={async renderCourseBook(){throw Error('not_called');},async renderCism(){throw Error('not_called');},async assemblePrint(){throw Error('not_called');},
    async renderReport(type,input,path){calls.runner.push({type,input,path});return rendered??{status:'rendered',pages:6,sha256:'f'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
  const root=resolve(base,name,'official-reports');
  return {calls,root,service:new OfficialReportPdfService(backend,runner,root,now)};
}

describe('academy_official_report_pdf_get reportType ganga, na_score, smart_book',()=>{
  test('official_report_ganga_na_score_smart_book_render_the_projected_views_under_the_owner_root',async()=>{
    const f=batch2Fixture('ganga',()=>gangaBlocks());
    const answer=await f.service.get({reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}}) as any;
    const path=resolve(f.root,'ganga-'+TESTING+'-20400116T090000Z.pdf');
    expect(answer).toMatchObject({status:'rendered',reportType:'ganga',outputPath:path,pages:6,parityOk:true,parityMismatches:0,parityRawRn:0,academyWrites:0,
      student:{studentId:KEY,displayName:STUDENT},testingNo:TESTING,classNo:CLASS,reportSummary:{title:'Invented Gang-A',testDate:'2040-01-11',questions:2}});
    expect(f.calls.student).toEqual([{view:'GaReportView',displayName:STUDENT,testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}]);
    expect(f.calls.runner[0]!.type).toBe('ganga');
    expect(f.calls.runner[0]!.input).toEqual({title:'Invented Gang-A',student:STUDENT,test_date:'2040-01-11',my_score:78,avg_score:64.5,rank_signal:4,comment:'Line one.\\r\\nLine two.',
      units:[{name:'Invented unit 1',my_score:70,average:60},{name:'Invented unit 2',my_score:90,average:65}],
      difficulty:{top:{questions:1,score:50},high:{questions:2,score:60},mid:{questions:3,score:70},low:{questions:4,score:80}},
      questions:[{no:1,unit:'Invented unit 1',difficulty:'상',correct_rate:40,correct:true},{no:2,unit:'Invented unit 2',difficulty:'하',correct_rate:77.5,correct:false}],pyramid_tier:2});
    expect(path).not.toContain('Invented');
    // na_score: NaScoreResultView, studentName and an explicit studentKey that is the student's.
    const g=batch2Fixture('na-score',()=>naScoreNamed());
    const na=await g.service.get({reportType:'na_score',studentName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS}}) as any;
    expect(na).toMatchObject({status:'rendered',reportType:'na_score',outputPath:resolve(g.root,'na_score-'+TESTING+'-20400116T090000Z.pdf'),reportSummary:{questions:2}});
    expect(g.calls.student).toEqual([{view:'NaScoreResultView',displayName:STUDENT,studentKey:KEY,testingNo:TESTING,classNo:CLASS}]);
    expect(g.calls.runner[0]!.input).toEqual({title:'Invented NA score',student:STUDENT,test_date:'2040-01-13',my_score:88,avg_score:70,rank_band:2,comment:'',
      units:[{name:'Invented unit 1',my_score:80,average:61}],
      questions:[{no:1,unit:'Invented unit 1',difficulty:'중',correct_rate:55,correct:true},{no:2,unit:'Invented unit 1',difficulty:'상',correct_rate:35,correct:false}]});
    // smart_book: SmartBookReportView by book code, the course-book blocks projected to the eleven members and book_code.
    const h=batch2Fixture('smart-book',()=>smartBookBlocks());
    const sb=await h.service.get({reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}}) as any;
    expect(sb).toMatchObject({status:'rendered',reportType:'smart_book',bookCode:BOOK_CODE,student:{studentId:KEY,displayName:STUDENT},
      outputPath:resolve(h.root,'smart_book-'+BOOK_CODE+'-20400116T090000Z.pdf'),
      reportSummary:{period:'2040-01-01 ~ 2040-03-01',myAverage:82.5,allAverage:75,chapterCount:1,questions:2}});
    expect(h.calls.book).toEqual([{displayName:STUDENT,bookCode:BOOK_CODE}]);
    const input=h.calls.runner[0]!.input;
    expect(h.calls.runner[0]!.type).toBe('smart_book');
    const {book_code,...eleven}=input;
    expect(book_code).toBe(BOOK_CODE);
    // The blocks are those of the course-book report: the same eleven members as its projection.
    expect(eleven).toEqual(projectCourseBookReport(smartBookBlocks() as never,{studentName:STUDENT,studentKey:KEY,courseKey:COURSE,bookNo:BOOK}).studyqInput);
    expect(input.bunit_list[0].questions).toEqual([{no:1,type:'Invented type',question_type:'객관식',difficulty:'중',correct_rate:66,correct:true},
      {no:2,type:'Invented type 2',question_type:'단답형',difficulty:'상',correct_rate:40,correct:false}]);
    await h.service.get({reportType:'smart_book',studentName:STUDENT,studentKey:KEY,params:{bookCode:BOOK_CODE},outputPath:'sb.pdf'});
    expect(h.calls.book.at(-1)).toEqual({displayName:STUDENT,studentKey:KEY,bookCode:BOOK_CODE});
    expect(h.calls.runner.at(-1)!.path).toBe(resolve(h.root,'sb.pdf'));
  });

  test('official_report_ganga_na_score_smart_book_params_are_validated_before_any_request',async()=>{
    for(const bad of [{reportType:'ganga',params:{testingNo:TESTING,classNo:CLASS}},{reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING}},
      {reportType:'na_score',displayName:STUDENT,params:{classNo:CLASS}},{reportType:'ganga',displayName:STUDENT,courseKey:COURSE,params:{testingNo:TESTING,classNo:CLASS}},
      {reportType:'na_score',displayName:STUDENT,bookNo:BOOK,params:{testingNo:TESTING,classNo:CLASS}},{reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,bookCode:BOOK_CODE}},
      {reportType:'na_score',displayName:STUDENT,params:{testingNo:'0',classNo:CLASS}},{reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-1-31'}},
      {reportType:'na_score',displayName:STUDENT,studentName:'Other Invented',params:{testingNo:TESTING,classNo:CLASS}},
      {reportType:'smart_book',params:{bookCode:BOOK_CODE}},{reportType:'smart_book',displayName:STUDENT},{reportType:'smart_book',displayName:STUDENT,params:{}},
      {reportType:'smart_book',displayName:STUDENT,params:{bookCode:''}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:'a'.repeat(65)}},
      {reportType:'smart_book',displayName:STUDENT,params:{bookCode:'SB 1'}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:'../SB'}},
      {reportType:'smart_book',displayName:STUDENT,params:{bookCode:'책'}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:7}},
      {reportType:'smart_book',displayName:STUDENT,courseKey:COURSE,params:{bookCode:BOOK_CODE}},{reportType:'smart_book',displayName:STUDENT,bookNo:BOOK,params:{bookCode:BOOK_CODE}},
      {reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE,testingNo:TESTING}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE,bookLabel:'x'}},
      {reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,bookCode:BOOK_CODE}},{reportType:'course_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}},
      {reportType:'gang_a',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
    for(const ok of [{reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},{reportType:'na_score',studentName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}},
      {reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}},{reportType:'smart_book',studentName:STUDENT,studentKey:KEY,params:{bookCode:'0.9_-x'},outputPath:'a.pdf'}])
      expect({ok,parsed:officialReportPdfGetSchema.safeParse(ok).success}).toEqual({ok,parsed:true});
    const f=batch2Fixture('batch2-bad',()=>gangaBlocks());
    expect(await f.service.get({reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS},outputPath:'../x.pdf'})).toMatchObject({status:'unavailable',reasonCode:'output_path_invalid'});
    expect(await f.service.get({reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE},outputPath:'x.txt'})).toMatchObject({status:'unavailable',reasonCode:'output_path_invalid'});
    expect(f.calls.student).toEqual([]);expect(f.calls.book).toEqual([]);
    // A student key that is not the named student's refuses with no runner call.
    expect(await f.service.get({reportType:'na_score',displayName:STUDENT,studentKey:'770099',params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(await f.service.get({reportType:'smart_book',displayName:STUDENT,studentKey:'770099',params:{bookCode:BOOK_CODE}})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(f.calls.runner).toEqual([]);
  });

  test('official_report_ganga_na_score_smart_book_refuse_another_student_and_unqualified_payloads',async()=>{
    for(const [type,raw,params] of [['ganga',()=>gangaBlocks('Another Invented'),{testingNo:TESTING,classNo:CLASS}],['na_score',()=>naScoreBlocks('Another Invented'),{testingNo:TESTING,classNo:CLASS}],
      ['smart_book',()=>smartBookBlocks('Another Invented'),{bookCode:BOOK_CODE}],['smart_book',()=>({...smartBookBlocks(),book_code:'SB-other'}),{bookCode:BOOK_CODE}]] as const){
      const f=batch2Fixture('batch2-other-'+type,raw);
      expect(await f.service.get({reportType:type,displayName:STUDENT,params})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
      expect(f.calls.runner).toEqual([]);
      await rm(f.root,{recursive:true,force:true});
    }
    for(const [type,raw,params] of [['ganga',()=>({block1:[]}),{testingNo:TESTING,classNo:CLASS}],['ganga',()=>({...gangaBlocks(),block3:[{'1':1,'2':1}]}),{testingNo:TESTING,classNo:CLASS}],
      ['na_score',()=>({...naScoreBlocks(),block2:{}}),{testingNo:TESTING,classNo:CLASS}],['na_score',()=>({...naScoreBlocks(),block1:'x'}),{testingNo:TESTING,classNo:CLASS}],
      ['smart_book',()=>({block1:[]}),{bookCode:BOOK_CODE}],['smart_book',()=>({...smartBookBlocks(),block10:[{'1':1}]}),{bookCode:BOOK_CODE}],
      ['smart_book',()=>({...smartBookBlocks(),bunit_list:[{'1':'c',exam_result:'x'}]}),{bookCode:BOOK_CODE}],['smart_book',()=>({period:'p',student:STUDENT,score_range:'x'}),{bookCode:BOOK_CODE}]] as const){
      const f=batch2Fixture('batch2-unq-'+type,raw);
      expect({type,answer:await f.service.get({reportType:type,displayName:STUDENT,params})}).toMatchObject({type,answer:{status:'unavailable',reasonCode:'report_view_unqualified'}});
      expect(f.calls.runner).toEqual([]);
      await rm(f.root,{recursive:true,force:true});
    }
    // Zero invented rows: an empty unit block stays empty and the strict validator refuses it.
    const empty=batch2Fixture('batch2-empty',()=>({...naScoreBlocks(),block2:[]}));
    await empty.service.get({reportType:'na_score',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}});
    expect(empty.calls.runner[0]!.input.units).toEqual([]);
    expect(python(VALIDATE_REPORT('na_score'),empty.calls.runner[0]!.input).status).not.toBe(0);
    // Nothing is cut: nine tests reach the validator, which refuses them (its limit is 8).
    const nine=batch2Fixture('batch2-nine',()=>({...smartBookBlocks(),block5:Array.from({length:9},(_,i)=>({'1':i+1,'2':'t'+i,'3':10,'4':70,'5':80,'6':'03-01'}))}));
    await nine.service.get({reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}});
    expect(nine.calls.runner[0]!.input.test_list).toHaveLength(9);
    expect(python(VALIDATE_REPORT('smart_book'),nine.calls.runner[0]!.input).status).not.toBe(0);
    const refused=batch2Fixture('batch2-refused',()=>gangaBlocks(),{status:'refused',code:'input_invalid',detail:'range:pyramid_tier'});
    expect(await refused.service.get({reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'refused',reasonCode:'studyq_input_invalid',studyqDetail:'range:pyramid_tier'});
    const noPort=new OfficialReportPdfService({async readCourseBookReportByName(){throw Error('x');}},refused.service.runner,resolve(base,'batch2-noport'),now);
    expect(await noPort.get({reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}})).toMatchObject({status:'unavailable',reasonCode:'report_view_source_port_unavailable'});
    expect(await noPort.get({reportType:'na_score',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'report_view_source_port_unavailable'});
    expect(await new OfficialReportPdfService(null,null,resolve(base,'none')).get({reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}})).toMatchObject({status:'unavailable',reasonCode:'official_report_backend_not_configured'});
    expect(await new OfficialReportPdfService(noPort.backend,null,resolve(base,'none')).get({reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'studyq_runner_not_configured'});
  });

  test('official_report_ganga_na_score_smart_book_inputs_are_accepted_by_the_real_studyq_validators',()=>{
    for(const [type,input] of [['ganga',projectGangaReport(gangaBlocks())],['ganga',projectGangaReport(projectGangaReport(gangaBlocks()) as never)],
      ['na_score',projectNaScoreReport(naScoreBlocks())],['na_score',projectNaScoreReport(naScoreNamed())],
      ['smart_book',projectSmartBookReport(smartBookBlocks(),BOOK_CODE)],['smart_book',projectSmartBookReport(projectSmartBookReport(smartBookBlocks(),BOOK_CODE) as never,BOOK_CODE)]] as const){
      const checked=python(VALIDATE_REPORT(type),input);
      expect({type,status:checked.status,stderr:checked.stderr}).toEqual({type,status:0,stderr:''});
      if(type==='smart_book')expect(JSON.parse(checked.stdout).pages).toBe(8-2);
      else expect(JSON.parse(checked.stdout).questions).toBe((input as any).questions.length);
    }
    // A projection copies values: a tier or a mark the renderer does not take reaches its validator and is refused there.
    const bad=projectGangaReport({...gangaBlocks(),block1:[{...gangaBlocks().block1[0]!,'8':'7'}]});
    expect(bad.pyramid_tier).toBe(7);
    expect(python(VALIDATE_REPORT('ganga'),bad).status).not.toBe(0);
    expect(projectNaScoreReport({...naScoreBlocks(),block3:[{'1':1,'2':'a','3':'b','4':1,'5':'?'}]}).questions[0]!.correct).toBe('?');
    expect(python(VALIDATE_REPORT('smart_book'),{...projectSmartBookReport(smartBookBlocks(),BOOK_CODE),book_code:'bad code'}).status).not.toBe(0);
  });

  test('official_report_ganga_na_score_smart_book_read_only_read_class_routes_i1_write_forbidden',async()=>{
    const {AcademyHttpClient}=await import('../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
    const {NativeDailyRecordBackend}=await import('../src/native-dailyrecord-backend.ts');
    const {REPORT_VIEW_PATH}=await import('../../../../../apps/academy/dc-gang-a/src/lms/reportViewClient.ts');
    const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
    const principal='<a href="/logout">Fictional Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="90001"></form><form name="form1"><input name="g_pri_no" value="90001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
    const envelope=(raw:unknown)=>new Response('<body>SERVER_RET = OK<BR>'+JSON.stringify(raw)+'<END_DATA></body>',{headers:{'content-type':'text/html; charset=utf-8'}});
    const run=async(name:string,request:Record<string,unknown>,raw:unknown,page?:string)=>{
      let calls=0;const bodies:string[]=[];
      const guard=writeForbiddenTransport((url,init)=>{calls++;
        if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
        bodies.push(String(init?.body));return page!==undefined?new Response(page):envelope(raw);});
      const admission={teacherId:'90001',teacherLabel:'Fictional Teacher',date:'2040-01-04',students:[{studentId:KEY,displayName:STUDENT}],sourceBundleSha256:'b'.repeat(64)};
      const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
      const backend=new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'}));
      const seen:any[]=[];
      const runner:StudyqRunner={async renderCourseBook(){throw Error('x');},async renderCism(){throw Error('x');},async assemblePrint(){throw Error('x');},
        async renderReport(type,input,path){seen.push({type,input});return {status:'rendered',pages:6,sha256:'e'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
      const service=new OfficialReportPdfService(backend as never,runner,resolve(base,name,'official-reports'),now);
      return {answer:await service.get(request) as any,guard,bodies,seen,calls:()=>calls};
    };
    const cases=[
      ['ganga',{reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},gangaBlocks(),'GaReportView',
        'p_process=GaReportView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date='],
      ['na_score',{reportType:'na_score',displayName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}},naScoreBlocks(),'NaScoreResultView',
        'p_process=NaScoreResultView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date=2040-01-31'],
      ['smart_book',{reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}},smartBookBlocks(),'SmartBookReportView',
        'p_process=SmartBookReportView&book_code='+BOOK_CODE+'&pri_no='+KEY]] as const;
    for(const [type,request,raw,view,body] of cases){
      const r=await run('i1-'+type,request,raw);
      expect({type,answer:r.answer}).toMatchObject({type,answer:{status:'rendered',reportType:type,academyWrites:0}});
      expect(r.guard.forbidden()).toEqual([]);
      expect(r.guard.requests.map(q=>q.role)).toEqual(['login','login','read','read']);
      expect(r.guard.requests.at(-1)).toEqual({route:REPORT_VIEW_PATH+' '+view,role:'read'});
      expect(r.bodies).toEqual([body]);
      expect(r.seen[0].type).toBe(type);
      expect(python(VALIDATE_REPORT(type),r.seen[0].input).status).toBe(0);
      // A login page in place of the view fails closed: no PDF.
      const loggedOut=await run('i1-login-'+type,request,null,login.replace('name="userpwd"','name="userpwd" type="password"'));
      expect(loggedOut.answer).toMatchObject({status:'unavailable',reasonCode:'authentication_required'});
      expect(loggedOut.seen).toEqual([]);
      expect(loggedOut.guard.forbidden()).toEqual([]);
      // A student key that is not the named student's: refused before any request.
      const wrong=await run('i1-wrong-'+type,{...request,studentKey:'770099'},raw);
      expect(wrong.answer).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
      expect(wrong.calls()).toBe(0);
    }
  });
});
