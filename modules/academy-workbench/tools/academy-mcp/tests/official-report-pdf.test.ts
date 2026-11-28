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
    async assemblePrint(){throw Error('not_called');},async renderCism(){throw Error('not_called_for_course_book');}};
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
    async assemblePrint(){throw Error('not_called');},async renderCism(input,path){calls.runner.push({input,path});return rendered??{status:'rendered',pages:3,sha256:'b'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
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
    const runner:StudyqRunner={async renderCourseBook(){throw Error('x');},async assemblePrint(){throw Error('not_called');},async renderCism(input,path){seen.push(input);return {status:'rendered',pages:2,sha256:'c'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
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
