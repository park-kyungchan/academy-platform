import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {lstat,mkdir,mkdtemp,readFile,realpath,rm,symlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import {OfficialReportPdfService,officialReportPdfGetSchema,type CourseBookReportBackend} from '../src/official-report-pdf.ts';
import {PcExecutorError,type PcExecutor,type PcRenderInput} from '../src/pc-executor.ts';
import {verifyCourseBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/courseBookReport.ts';
import {CISM_NARRATIVES,CISM_WIRE_SHA256,cismWeek,type CismBlockRead} from '../../../../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts';
import {reportViewForm} from '../../../../../apps/academy/dc-gang-a/src/lms/reportViewClient.ts';
import {writeForbiddenTransport} from './network-deny.ts';

// academy_official_report_pdf_get (src/official-report-pdf.ts, specs/academy-mcp/pc-executor.md) with an injected backend
// and an injected PC executor: the server's answer reaches the executor verbatim, with the keys of its request. Invented values only.
const STUDENT='Invented Student',KEY='770011',COURSE='560013',BOOK='430019';
const courseBookRaw=()=>({block1:[{'1':'가우스 초등 1-1 Invented','2':'2040-01-01','3':'2040-03-01','4':STUDENT}],block2:[{'1':5,'3':82.5,'4':75,'5':'c'}],
  block5:Array.from({length:9},(_,i)=>({'1':i+1,'2':'t'+i,'3':10,'4':70,'5':80,'6':'03-01'})),
  bunit_list:[{'1':'Invented chapter','3':80,'4':70,exam_result:[{'1':1,'2':'Invented type','3':2,'4':1,'5':66,'6':'O'}]}],
  block9:[{'2':1,'3':2,'4':3,'5':4}],block10:[{'1':90},{'1':80},{'1':70},{'1':60}],block11:[{'2':71}],block12:[{'2':'u','3':3,'4':4,'5':70}],surplus:{kept:['as','is']}});
const report=verifyCourseBookReport(courseBookRaw(),{studentName:STUDENT,studentKey:KEY,courseKey:COURSE,bookNo:BOOK});
let base:string;
beforeAll(async()=>{base=await realpath(await mkdtemp(resolve(tmpdir(),'official-report-')));});
afterAll(async()=>{await rm(base,{recursive:true,force:true});});
const now=()=>new Date('2040-01-16T09:00:00.123Z');
/** A PC executor that records its input and answers as the real one does. */
function fakeExecutor(fails?:PcExecutorError){
  const calls:PcRenderInput[]=[];
  const executor:PcExecutor={async render(input){calls.push(structuredClone(input));
    if(fails)throw fails;
    await writeFile(input.outputPath,'%PDF-1.3 invented');
    return {status:'rendered',pages:8,sha256:'a'.repeat(64),outputPath:input.outputPath,pcCleanup:'done'};}};
  return {calls,executor};
}
function fixture(name:string,over:Partial<{answer:Awaited<ReturnType<CourseBookReportBackend['readCourseBookReportByName']>>;fails:PcExecutorError}>={}){
  const calls={backend:[] as unknown[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(input){calls.backend.push(input);
    return over.answer??{kind:'report',student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:BOOK,bindingSource:'historical_key_registry',report};}};
  const pc=fakeExecutor(over.fails);
  const root=resolve(base,name,'official-reports');
  return {calls,pc,root,service:new OfficialReportPdfService(backend,pc.executor,root,now)};
}

describe('academy_official_report_pdf_get',()=>{
  test('official_report_course_book_passes_the_raw_answer_verbatim',async()=>{
    const f=fixture('render');
    const answer=await f.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY}) as any;
    const path=resolve(f.root,'course_book-'+BOOK+'-20400116T090000Z.pdf');
    expect(answer).toEqual({reportType:'course_book',displayName:STUDENT,academyWrites:0,parentSends:0,automaticRetryAllowed:false,
      execution:'read_only_lms_pc_executor',sourceContentIsUntrustedData:true,student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:BOOK,
      bindingSource:'historical_key_registry',reportSummary:{bookTitle:'가우스 초등 1-1 Invented',period:'2040-01-01 ~ 2040-03-01',myAverage:82.5,allAverage:75,chapterCount:1},
      status:'rendered',outputPath:path,sha256:'a'.repeat(64),pages:8,pcCleanup:'done'});
    // No parity members: the original executor renders what the server returns.
    for(const gone of ['parityOk','parityMismatches','parityRawRn','truncation'])expect(answer[gone]).toBeUndefined();
    expect(f.calls.backend).toEqual([{displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY}]);
    // The answer of getCourseBookReport exactly as the server sent it: nine tests, the unknown member, nothing cut or renamed.
    expect(f.pc.calls).toEqual([{reportType:'course_book',keys:{},blockData:courseBookRaw(),outputPath:path}]);
    expect(path).not.toContain('Invented');
    const stat=await lstat(f.root);expect(stat.isDirectory()&&(stat.mode&0o777)===0o700).toBe(true);
    // studentName and params.bookLabel name the student and the book.
    await f.service.get({reportType:'course_book',studentName:STUDENT,params:{bookLabel:'가우스 1-1'},outputPath:'mine.pdf'});
    expect(f.calls.backend.at(-1)).toEqual({displayName:STUDENT,bookLabel:'가우스 1-1'});
    expect(f.pc.calls.at(-1)!.outputPath).toBe(resolve(f.root,'mine.pdf'));
  });

  test('official_report_course_book_without_tests_renders_what_the_server_returns',async()=>{
    // Superseded course_book_no_tests: an empty test list (block5) is the server's answer, and the executor renders it.
    const raw={...courseBookRaw(),block5:[]};
    const f=fixture('no-tests',{answer:{kind:'report',student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:BOOK,
      report:verifyCourseBookReport(raw,{studentName:STUDENT,studentKey:KEY,courseKey:COURSE,bookNo:BOOK})}});
    expect(await f.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY})).toMatchObject({status:'rendered',pages:8});
    expect(f.pc.calls[0]!.blockData).toEqual(raw);
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
    // Refused before any LMS request and any PC step.
    expect(f.calls.backend).toEqual([]);expect(f.pc.calls).toEqual([]);
    // An output root that is a symbolic link is refused.
    const target=resolve(base,'paths-target');await mkdir(target);
    const linked=new OfficialReportPdfService(fixture('x').service.backend,fakeExecutor().executor,resolve(base,'paths-link'),now);
    await symlink(target,resolve(base,'paths-link'));
    expect(await linked.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'unavailable',reasonCode:'output_root_unqualified'});
  });

  test('official_report_refusals_and_interview',async()=>{
    const interview=fixture('interview',{answer:{kind:'interview_required',student:{studentId:KEY,displayName:STUDENT},bindings:[{bookLabel:'A',bookNo:'1'},{bookLabel:'B',bookNo:'2'}],
      reason:'multiple_book_bindings_select_one',observedAt:'2040-01-16T09:00:00.000Z'}});
    expect(await interview.service.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'interview_required',bindings:[{bookLabel:'A'},{bookLabel:'B'}]});
    expect(interview.pc.calls).toEqual([]);
    const conflict=fixture('conflict',{answer:{kind:'report',student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:'430099',report}});
    expect(await conflict.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(conflict.pc.calls).toEqual([]);
    expect(await new OfficialReportPdfService(null,fakeExecutor().executor,resolve(base,'none')).get({reportType:'course_book',displayName:STUDENT}))
      .toMatchObject({status:'unavailable',reasonCode:'official_report_backend_not_configured',academyWrites:0});
    // Input refused before anything runs.
    for(const bad of [{reportType:'cism',displayName:STUDENT},{reportType:'course_book'},{reportType:'course_book',displayName:STUDENT,studentName:'Other Invented'},
      {reportType:'course_book',displayName:STUDENT,courseKey:COURSE},{reportType:'course_book',displayName:STUDENT,params:{unknown:true}},
      {reportType:'course_book',displayName:STUDENT,courseKey:'0',bookNo:BOOK,studentKey:KEY}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
  });

  test('official_report_without_pc_executor_is_pc_executor_not_configured',async()=>{
    const backend=fixture('no-pc').service.backend;
    const requests=[{reportType:'course_book',displayName:STUDENT},{reportType:'cism',params:{scope:'week'}},{reportType:'cism',params:{scope:'month',month:'2040-01'}},
      {reportType:'diag',displayName:STUDENT,params:{testingNo:'901',classNo:'0'}},{reportType:'formative',displayName:STUDENT,params:{testingNo:'901',classNo:'0'}},
      {reportType:'ganga',displayName:STUDENT,params:{testingNo:'901',classNo:'0'}},{reportType:'na_score',displayName:STUDENT,params:{testingNo:'901',classNo:'0'}},
      {reportType:'new_diag',params:{paperNo:'4401',applyCode:'APP-7'}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:'SB-1'}}];
    for(const service of [new OfficialReportPdfService(backend,null,resolve(base,'no-pc')),new OfficialReportPdfService(null,null,resolve(base,'no-pc'))])
      for(const request of requests)
        expect({request,answer:await service.get(request)}).toMatchObject({request,answer:{status:'unavailable',reasonCode:'pc_executor_not_configured',academyWrites:0,execution:'read_only_lms_pc_executor'}});
    expect((fixture('no-pc-calls').calls.backend)).toEqual([]);
  });

  test('official_report_pc_failures_are_answered_with_their_code',async()=>{
    for(const [code,pcCleanup] of [['pc_unreachable',undefined],['pc_host_key_mismatch',undefined],['pc_executor_failed','done'],['pc_executor_timeout','incomplete'],
      ['pc_output_invalid','done'],['pc_input_unencodable',undefined]] as const){
      const f=fixture('fail-'+code,{fails:new PcExecutorError(code,pcCleanup)});
      const answer=await f.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY}) as any;
      expect({code,answer}).toMatchObject({code,answer:{status:'unavailable',reasonCode:code,academyWrites:0,courseKey:COURSE,bookNo:BOOK,
        reportSummary:{bookTitle:'가우스 초등 1-1 Invented'}}});
      expect({code,pcCleanup:answer.pcCleanup}).toEqual({code,pcCleanup});
      // No detail of a failure holds a student value.
      expect(JSON.stringify(answer.failure)).not.toContain(STUDENT);
    }
  });
});

// reportType 'cism': the owner's week, its block data as the CismChartMain page function makeCISMJSonData builds it.
const BLOCK_ROWS=(text:(n:string)=>string)=>({block4:[{'1':'Ｉ-1','2':text('i_1')},{'1':'Ｉ-2','2':text('i_2')}],
  block5:[{'1':'Ｓ-1','2':text('s_1')},{'1':'Ｓ-2','2':text('s_2')},{'1':'Ｓ-3','2':text('s_3')},{'1':'Ｓ-4','2':text('s_4')}],
  block6:[{'1':'Ｍ-1','2':text('m_1')},{'1':'Ｍ-2','2':text('m_2')}]});
const weekRead=(date:string):CismBlockRead=>{const {periodStart,periodEnd}=cismWeek(date);return {date,periodStart,periodEnd,weekLabel:'2040-3 주차 (1월15일~1월21일)',
  blockData:{block1:[{'1':'1'}],block2:[{'1':date,'2':'Invented Teacher','3':'1월15일~1월21일'}],
    block3:[{'1':'Invented Group','2':'상','3':'3','4':'75%','5':'80%','6':'90점'}],...BLOCK_ROWS(n=>n==='i_1'?'Invented narrative':'')}};};
function cismFixture(name:string,read?:(date:string)=>CismBlockRead,fails?:PcExecutorError){
  const calls={dates:[] as string[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(){throw Error('not_called_for_cism');},
    ...(read?{async readCismBlockData(date:string){calls.dates.push(date);return read(date);}}:{})};
  const pc=fakeExecutor(fails);
  const root=resolve(base,name,'official-reports');
  return {calls,pc,root,service:new OfficialReportPdfService(backend,pc.executor,root,now)};
}
// The CISM page of the protocol tests' fictional source, with the pinned handler text of the fixtures, and the
// group containers, summary cells and labels that makeCISMJSonData reads.
const cismHandlers=readFileSync(resolve(import.meta.dir,'../../../../../apps/academy/dc-gang-a/tests/lms/fixtures/cism-handlers.js.txt'),'utf8');
function fictionalCismHtml(date:string,narrative:string){
  const {periodStart,periodEnd}=cismWeek(date),a=new Date(periodStart),b=new Date(periodEnd);
  const group=(i:number,name:string,mode:string,student:string,keys:string,think:string,week:string,submit:string,rate:string)=>
    `<div id="root"><label id="grp_name_${i}">${name}</label><span id="class_mode_tag_${i}"><select id="class_mode_${i}" onchange="setCismData('class_mode',${i},${keys});">`
    +['0:-선택-','3:상','2:중','1:하'].map(o=>{const [v,t]=o.split(':');return `<option value="${v}"${v===mode?' selected':''}>${t}</option>`;}).join('')
    +`</select></span><span id="submit_sum_${i}">${submit}</span><span id="homework_rate_${i}">${rate}</span><table><tr><td>${student}</td>`
    +`<td id="think_note_${i}"><input id="think_note_${i}" onfocusout="setCismData('think_note',${i},${keys});" value="${think}"><label name="label_think_note">${think}</label></td>`
    +`<td id="week_test_${i}"><input id="week_test_${i}" onfocusout="setCismData('week_test',${i},${keys});" value="${week}"><label>${week}</label></td></tr></table></div>`;
  return `<h1>주간 교무회의 CISM 작성</h1><form id="theDateForm"><input name="reqCmd" value="CismChartMain"><input id="the_date" value="${date}"><input id="tutor_pri_no" value="90001"><input id="teacher_name" value="Fictional Teacher"><label>2040-42 주차 (${a.getUTCMonth()+1}월${a.getUTCDate()}일~${b.getUTCMonth()+1}월${b.getUTCDate()}일)</label></form>`
    +group(0,'Fictional Group','3','Fictional Student','90002,90003,1','80','90','3/4','75%')+group(1,'Fictional Group Two','0','Fictional Student Two','90004,90005,2','','','','')
    +CISM_NARRATIVES.map(n=>`<label>${n}</label><textarea name="${n}" id="${n}" onfocusout="javascript:onTextareaFocusOut(this,'${n}');">${n==='i_1'?narrative:''}</textarea>`).join('')+`<script>${cismHandlers}</script>`;
}
const fictionalBlockData=(date:string,narrative:string)=>{
  const {periodStart,periodEnd}=cismWeek(date),a=new Date(periodStart),b=new Date(periodEnd);
  return {block1:[{'1':'1'}],block2:[{'1':date,'2':'Fictional Teacher','3':`${a.getUTCMonth()+1}월${a.getUTCDate()}일~${b.getUTCMonth()+1}월${b.getUTCDate()}일`}],
    block3:[{'1':'Fictional Group','2':'상','3':'3/4','4':'75%','5':'80%','6':'90점'},{'1':'Fictional Group Two','2':'','3':'','4':'','5':'%','6':'점'}],
    ...BLOCK_ROWS(n=>n==='i_1'?narrative:'')};
};
const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
const principal='<a href="/logout">Fictional Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="90001"></form><form name="form1"><input name="g_pri_no" value="90001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';

describe('academy_official_report_pdf_get reportType cism',()=>{
  test('official_report_cism_week_passes_the_page_block_data',async()=>{
    const f=cismFixture('cism-week',weekRead);
    const answer=await f.service.get({reportType:'cism',params:{scope:'week',date:'2040-01-16'}}) as any;
    const path=resolve(f.root,'cism-week-2040-01-16-20400116T090000Z.pdf');
    expect(answer).toMatchObject({status:'rendered',reportType:'cism',scope:'week',date:'2040-01-16',outputPath:path,pages:8,pcCleanup:'done',academyWrites:0,
      execution:'read_only_lms_pc_executor',reportSummary:{weekLabel:'2040-3 주차 (1월15일~1월21일)',period:'2040-01-15 ~ 2040-01-21',groups:1,filledSections:1}});
    expect(JSON.stringify(answer)).not.toContain('Invented Group');
    expect(f.calls.dates).toEqual(['2040-01-16']);
    expect(f.pc.calls).toEqual([{reportType:'cism',keys:{},blockData:weekRead('2040-01-16').blockData,outputPath:path}]);
    // The default date is today (Asia/Seoul); the default name holds no personal value.
    expect(await f.service.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'output_exists',date:'2040-01-16'});
  });

  test('official_report_cism_month_is_cism_month_unavailable',async()=>{
    const f=cismFixture('cism-month',weekRead);
    expect(await f.service.get({reportType:'cism',params:{scope:'month',month:'2040-01'}})).toEqual({reportType:'cism',scope:'month',month:'2040-01',academyWrites:0,parentSends:0,
      automaticRetryAllowed:false,execution:'read_only_lms_pc_executor',sourceContentIsUntrustedData:true,status:'unavailable',reasonCode:'cism_month_unavailable'});
    expect(f.calls.dates).toEqual([]);expect(f.pc.calls).toEqual([]);
  });

  test('official_report_cism_refuses_bad_params',async()=>{
    for(const bad of [{reportType:'cism'},{reportType:'cism',params:{}},{reportType:'cism',params:{scope:'year'}},{reportType:'cism',params:{scope:'month'}},
      {reportType:'cism',params:{scope:'month',month:'2040-13'}},{reportType:'cism',params:{scope:'month',month:'2040-01',date:'2040-01-16'}},
      {reportType:'cism',params:{scope:'week',month:'2040-01'}},{reportType:'cism',params:{scope:'week',bookLabel:'x'}},{reportType:'cism',params:{scope:'week',extra:1}},
      {reportType:'cism',params:{scope:'week'},displayName:STUDENT},{reportType:'cism',params:{scope:'week'},studentName:STUDENT},
      {reportType:'cism',params:{scope:'week'},courseKey:COURSE,bookNo:BOOK,studentKey:KEY},{reportType:'cism',params:{scope:'week'},bookNo:BOOK},
      {reportType:'course_book',displayName:STUDENT,params:{scope:'week'}},{reportType:'course_book',displayName:STUDENT,params:{month:'2040-01'}}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
    const f=cismFixture('cism-bad',weekRead);
    expect(await f.service.get({reportType:'cism',params:{scope:'week',date:'next tuesday'}})).toMatchObject({status:'unavailable',reasonCode:'date_form_unsupported_use_yyyy_mm_dd_today_yesterday_or_month_day',academyWrites:0});
    expect(await f.service.get({reportType:'cism',params:{scope:'week'},outputPath:'../x.pdf'})).toMatchObject({status:'unavailable',reasonCode:'output_path_invalid'});
    expect(f.calls.dates).toEqual([]);
    const noPort=cismFixture('cism-noport');
    expect(await noPort.service.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'cism_source_port_unavailable',date:'2040-01-16'});
    const failing=cismFixture('cism-fail',weekRead,new PcExecutorError('pc_unreachable'));
    expect(await failing.service.get({reportType:'cism',params:{scope:'week'}})).toMatchObject({status:'unavailable',reasonCode:'pc_unreachable',date:'2040-01-16',reportSummary:{groups:1}});
  });

  test('official_report_cism_block_data_is_read_as_the_page_function_builds_it',async()=>{
    // jsdom resolves from the dc-gang-a module through its own DOM parser, as the HTTP client uses it.
    const {sourceDomParser}=await import('../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
    const {parseCismReport}=await import('../../../../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts');
    const parse=(html:string)=>parseCismReport(html,sourceDomParser.parseFromString as any,'2040-10-15',{teacherKey:'90001',teacherLabel:'Fictional Teacher'});
    const report=parse(fictionalCismHtml('2040-10-15','Line one.\r\nLine two.'));
    expect(report.blockData).toEqual(fictionalBlockData('2040-10-15','Line one.\nLine two.'));
    expect(Object.keys(report.blockData!)).toEqual(['block1','block2','block3','block4','block5','block6']);
    // The fields, the wire hash and the report hash are those of before: the block data is beside them.
    expect(report.wireSha256).toBe(CISM_WIRE_SHA256);
    expect(report.fields.filter(x=>x.kind==='class_mode').map(x=>x.groupName)).toEqual(['Fictional Group','Fictional Group Two']);
    // No think_note label above 0: block1 is "0"; a class mode of 1 or 2 is 하 or 중.
    const quiet=parse(fictionalCismHtml('2040-10-15','').replace('<label name="label_think_note">80</label>','<label name="label_think_note">0</label>').replace('<option value="3" selected>','<option value="3">').replace('<option value="1">','<option value="1" selected>'));
    expect(quiet.blockData!.block1).toEqual([{'1':'0'}]);
    expect(quiet.blockData!.block3.map(r=>r['2'])).toEqual(['하','']);
  });

  test('official_report_cism_reads_only_read_class_routes_i1_write_forbidden',async()=>{
    // jsdom and the HTTP client load here, so a host without the module's dependencies fails only this test.
    const {AcademyHttpClient}=await import('../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
    const {NativeDailyRecordBackend}=await import('../src/native-dailyrecord-backend.ts');
    let calls=0;
    const guard=writeForbiddenTransport(url=>{calls++;
      if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
      const date=new URL(url,'https://lms.invalid').searchParams.get('the_date')!;
      return new Response(fictionalCismHtml(date,'Invented narrative'),{headers:{'content-type':'text/html; charset=utf-8'}});});
    const admission={teacherId:'90001',teacherLabel:'Fictional Teacher',date:'2040-01-04',students:[{studentId:'9101',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
    const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
    const backend=new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'}));
    const pc=fakeExecutor();
    const service=new OfficialReportPdfService(backend as never,pc.executor,resolve(base,'cism-i1','official-reports'),now);
    const week=await service.get({reportType:'cism',params:{scope:'week',date:'2040-10-15'}}) as any;
    expect(week).toMatchObject({status:'rendered',academyWrites:0,reportSummary:{period:'2040-10-14 ~ 2040-10-20',groups:2,filledSections:1}});
    expect(guard.forbidden()).toEqual([]);
    expect(guard.requests.map(r=>r.role)).toEqual(['login','login','read','read']);
    expect(guard.requests.at(-1)).toEqual({route:'/servlet/controller.cct.common.CourseCommonServlet CismChartMain',role:'read'});
    expect(pc.calls[0]).toMatchObject({reportType:'cism',keys:{},blockData:fictionalBlockData('2040-10-15','Invented narrative')});
    // The CISM tools answer the week as before: without the block data.
    const toolWeek=await backend.readCismReport('2040-10-15');
    expect('blockData' in toolWeek).toBe(false);
    expect(guard.forbidden()).toEqual([]);
  });
});

// reportType 'diag', 'new_diag', 'formative', 'ganga', 'na_score', 'smart_book': one read-only POST of ReportViewServlet
// (specs/academy-mcp/direct-key-lookup.md R8), its JSON passed verbatim with the keys of the request. Invented values only.
const TESTING='901',CLASS='560077',PAPER='4401',APPLICANT='Invented Applicant',BOOK_CODE='SB-2040.inv_01';
const viewRaw=(student=STUDENT)=>({block1:[{'1':'Invented diagnostic','2':'2040-01-10','3':student,'4':'4','5':72.5,'6':61,'7':'Line one.\\r\\nLine two.'}],
  block2:[{'1':'Invented area A','2':80,'3':60,'4':'green','5':'nice'}],block3:[{'1':2,'2':40}],block4:[{'1':1,'2':'Invented area A','3':'상','4':66,'5':'O'}],
  surplus:'kept as it is'});
function viewFixture(name:string,raw:()=>Record<string,unknown>,fails?:PcExecutorError){
  const calls={student:[] as unknown[],applicant:[] as unknown[],book:[] as unknown[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(){throw Error('not_called');},
    async readStudentReportView(input){calls.student.push(input);
      if(input.studentKey!==undefined&&input.studentKey!==KEY)throw Error('official_report_scope_conflict');
      return {student:{studentId:KEY,displayName:STUDENT},raw:raw()};},
    async readSmartBookReportView(input){calls.book.push(input);
      if(input.studentKey!==undefined&&input.studentKey!==KEY)throw Error('official_report_scope_conflict');
      return {student:{studentId:KEY,displayName:STUDENT},raw:raw()};},
    async readNewStudentReportView(input){calls.applicant.push(input);return {raw:raw()};}};
  const pc=fakeExecutor(fails);
  const root=resolve(base,name,'official-reports');
  return {calls,pc,root,service:new OfficialReportPdfService(backend,pc.executor,root,now)};
}

describe('academy_official_report_pdf_get report views',()=>{
  test('official_report_report_views_pass_the_raw_answer_with_their_keys',async()=>{
    for(const [type,view] of [['diag','DiagReportView'],['formative','FaView'],['ganga','GaReportView'],['na_score','NaScoreResultView']] as const){
      const f=viewFixture('view-'+type,()=>viewRaw());
      const answer=await f.service.get({reportType:type,displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}}) as any;
      const path=resolve(f.root,type+'-'+TESTING+'-20400116T090000Z.pdf');
      expect(answer).toMatchObject({status:'rendered',reportType:type,outputPath:path,pages:8,pcCleanup:'done',academyWrites:0,execution:'read_only_lms_pc_executor',
        student:{studentId:KEY,displayName:STUDENT},testingNo:TESTING,classNo:CLASS});
      expect(f.calls.student).toEqual([{view,displayName:STUDENT,testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}]);
      // The view's JSON as the server answered it, and the keys of the request: testing_no, pri_no, cls_no.
      expect(f.pc.calls).toEqual([{reportType:type,keys:{testingNo:TESTING,studentKey:KEY,classNo:CLASS},blockData:viewRaw(),outputPath:path}]);
      expect(path).not.toContain('Invented');
    }
    // new_diag: NewStudentReportView by paper and apply code, no student lookup; p_no and apply_code.
    const h=viewFixture('new-diag',()=>viewRaw(APPLICANT));
    const nd=await h.service.get({reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7'}}) as any;
    expect(nd).toMatchObject({status:'rendered',reportType:'new_diag',paperNo:PAPER,outputPath:resolve(h.root,'new_diag-'+PAPER+'-20400116T090000Z.pdf')});
    expect(nd.student).toBeUndefined();
    expect(h.calls.applicant).toEqual([{paperNo:PAPER,applyCode:'APP-7'}]);
    expect(h.pc.calls[0]).toMatchObject({reportType:'new_diag',keys:{paperNo:PAPER,applyCode:'APP-7'},blockData:viewRaw(APPLICANT)});
    await h.service.get({reportType:'new_diag',displayName:APPLICANT,params:{paperNo:PAPER,applyCode:'0077',collegeKey:'88002'},outputPath:'nd2.pdf'});
    expect(h.calls.applicant.at(-1)).toEqual({paperNo:PAPER,applyCode:'0077',collegeKey:'88002'});
    expect(h.pc.calls.at(-1)!.keys).toEqual({paperNo:PAPER,applyCode:'0077'});
    // smart_book: SmartBookReportView by book code; book_code and pri_no.
    const s=viewFixture('smart-book',()=>viewRaw());
    const sb=await s.service.get({reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}}) as any;
    expect(sb).toMatchObject({status:'rendered',reportType:'smart_book',bookCode:BOOK_CODE,student:{studentId:KEY,displayName:STUDENT},
      outputPath:resolve(s.root,'smart_book-'+BOOK_CODE+'-20400116T090000Z.pdf')});
    expect(s.calls.book).toEqual([{displayName:STUDENT,bookCode:BOOK_CODE}]);
    expect(s.pc.calls[0]).toMatchObject({reportType:'smart_book',keys:{bookCode:BOOK_CODE,studentKey:KEY},blockData:viewRaw()});
    // formative with studentName and an explicit studentKey that is the student's.
    const g=viewFixture('formative-key',()=>viewRaw());
    expect(await g.service.get({reportType:'formative',studentName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS},outputPath:'fa.pdf'})).toMatchObject({status:'rendered',outputPath:resolve(g.root,'fa.pdf')});
    expect(g.calls.student).toEqual([{view:'FaView',displayName:STUDENT,studentKey:KEY,testingNo:TESTING,classNo:CLASS}]);
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
      {reportType:'course_book',displayName:STUDENT,params:{testingNo:TESTING}},{reportType:'cism',params:{scope:'week',paperNo:PAPER}},{reportType:'smart_book',displayName:STUDENT},
      {reportType:'smart_book',params:{bookCode:BOOK_CODE}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:'SB 1'}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:'../SB'}},
      {reportType:'smart_book',displayName:STUDENT,courseKey:COURSE,params:{bookCode:BOOK_CODE}},{reportType:'gang_a',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
    const f=viewFixture('view-bad',()=>viewRaw());
    expect(await f.service.get({reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS},outputPath:'../x.pdf'})).toMatchObject({status:'unavailable',reasonCode:'output_path_invalid'});
    expect(f.calls.student).toEqual([]);
    // A student key that is not the named student's refuses with no PC step.
    expect(await f.service.get({reportType:'diag',displayName:STUDENT,studentKey:'770099',params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(await f.service.get({reportType:'smart_book',displayName:STUDENT,studentKey:'770099',params:{bookCode:BOOK_CODE}})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(f.pc.calls).toEqual([]);
    const noPort=new OfficialReportPdfService({async readCourseBookReportByName(){throw Error('x');}},fakeExecutor().executor,resolve(base,'view-noport'),now);
    for(const request of [{reportType:'formative',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},{reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}},
      {reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7'}}])
      expect(await noPort.get(request)).toMatchObject({status:'unavailable',reasonCode:'report_view_source_port_unavailable'});
    const failing=viewFixture('view-fail',()=>viewRaw(),new PcExecutorError('pc_output_invalid','done'));
    expect(await failing.service.get({reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}})).toMatchObject({status:'unavailable',reasonCode:'pc_output_invalid',pcCleanup:'done',testingNo:TESTING});
  });

  const envelope=(raw:unknown)=>new Response('<body>SERVER_RET = OK<BR>'+JSON.stringify(raw)+'<END_DATA></body>',{headers:{'content-type':'text/html; charset=utf-8'}});
  const run=async(name:string,request:Record<string,unknown>,raw:unknown,page?:string)=>{
    const {AcademyHttpClient}=await import('../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
    const {NativeDailyRecordBackend}=await import('../src/native-dailyrecord-backend.ts');
    let calls=0;const bodies:string[]=[];
    const guard=writeForbiddenTransport((url,init)=>{calls++;
      if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
      bodies.push(String(init?.body));return page!==undefined?new Response(page):envelope(raw);});
    const admission={teacherId:'90001',teacherLabel:'Fictional Teacher',date:'2040-01-04',students:[{studentId:KEY,displayName:STUDENT}],sourceBundleSha256:'b'.repeat(64)};
    const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
    const backend=new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'}));
    const pc=fakeExecutor();
    const service=new OfficialReportPdfService(backend as never,pc.executor,resolve(base,name,'official-reports'),now);
    return {answer:await service.get(request) as any,guard,bodies,seen:pc.calls,calls:()=>calls};
  };
  const REPORT_VIEW_PATH='/servlet/controller.common.ReportViewServlet';

  test('official_report_report_views_read_only_read_class_routes_i1_write_forbidden',async()=>{
    const diag=await run('i1-diag',{reportType:'diag',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},viewRaw());
    expect(diag.answer).toMatchObject({status:'rendered',academyWrites:0});
    expect(diag.guard.forbidden()).toEqual([]);
    expect(diag.guard.requests.map(r=>r.role)).toEqual(['login','login','read','read']);
    expect(diag.guard.requests.at(-1)).toEqual({route:REPORT_VIEW_PATH+' DiagReportView',role:'read'});
    expect(diag.bodies).toEqual(['p_process=DiagReportView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date=']);
    expect(diag.seen[0]).toMatchObject({reportType:'diag',keys:{testingNo:TESTING,studentKey:KEY,classNo:CLASS},blockData:viewRaw()});
    const fa=await run('i1-fa',{reportType:'formative',displayName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}},viewRaw());
    expect(fa.answer).toMatchObject({status:'rendered'});
    expect(fa.guard.forbidden()).toEqual([]);
    expect(fa.bodies).toEqual(['p_process=FaView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date=2040-01-31']);
    const nd=await run('i1-nd',{reportType:'new_diag',params:{paperNo:PAPER,applyCode:'APP-7'}},viewRaw(APPLICANT));
    expect(nd.answer).toMatchObject({status:'rendered'});
    expect(nd.guard.forbidden()).toEqual([]);
    expect(nd.bodies).toEqual(['p_process=NewStudentReportView&p_p_no='+PAPER+'&p_apply_code=APP-7&clg_no=88001']);
    expect(nd.seen[0]).toMatchObject({reportType:'new_diag',keys:{paperNo:PAPER,applyCode:'APP-7'},blockData:viewRaw(APPLICANT)});
    // A student key that is not the named student's: refused before any report-view request.
    const wrong=await run('i1-wrong',{reportType:'diag',displayName:STUDENT,studentKey:'770099',params:{testingNo:TESTING,classNo:CLASS}},viewRaw());
    expect(wrong.answer).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(wrong.calls()).toBe(0);
    // A login page in place of the view fails closed: no PDF.
    const loggedOut=await run('i1-login',{reportType:'formative',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},null,login.replace('name="userpwd"','name="userpwd" type="password"'));
    expect(loggedOut.answer).toMatchObject({status:'unavailable',reasonCode:'authentication_required'});
    expect(loggedOut.seen).toEqual([]);
    expect(loggedOut.guard.forbidden()).toEqual([]);
  });

  test('official_report_ganga_na_score_smart_book_read_only_read_class_routes_i1_write_forbidden',async()=>{
    const cases=[
      ['ganga',{reportType:'ganga',displayName:STUDENT,params:{testingNo:TESTING,classNo:CLASS}},'GaReportView',
        'p_process=GaReportView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date=',{testingNo:TESTING,studentKey:KEY,classNo:CLASS}],
      ['na_score',{reportType:'na_score',displayName:STUDENT,studentKey:KEY,params:{testingNo:TESTING,classNo:CLASS,limitDate:'2040-01-31'}},'NaScoreResultView',
        'p_process=NaScoreResultView&p_testing_no='+TESTING+'&p_pri_no='+KEY+'&p_cls_no='+CLASS+'&p_limit_date=2040-01-31',{testingNo:TESTING,studentKey:KEY,classNo:CLASS}],
      ['smart_book',{reportType:'smart_book',displayName:STUDENT,params:{bookCode:BOOK_CODE}},'SmartBookReportView',
        'p_process=SmartBookReportView&book_code='+BOOK_CODE+'&pri_no='+KEY,{bookCode:BOOK_CODE,studentKey:KEY}]] as const;
    for(const [type,request,view,body,keys] of cases){
      const r=await run('i1-'+type,request,viewRaw());
      expect({type,answer:r.answer}).toMatchObject({type,answer:{status:'rendered',reportType:type,academyWrites:0}});
      expect(r.guard.forbidden()).toEqual([]);
      expect(r.guard.requests.map(q=>q.role)).toEqual(['login','login','read','read']);
      expect(r.guard.requests.at(-1)).toEqual({route:REPORT_VIEW_PATH+' '+view,role:'read'});
      expect(r.bodies).toEqual([body]);
      expect(r.seen[0]).toMatchObject({reportType:type,keys,blockData:viewRaw()});
      const loggedOut=await run('i1-login-'+type,request,null,login.replace('name="userpwd"','name="userpwd" type="password"'));
      expect(loggedOut.answer).toMatchObject({status:'unavailable',reasonCode:'authentication_required'});
      expect(loggedOut.seen).toEqual([]);
      expect(loggedOut.guard.forbidden()).toEqual([]);
      const wrong=await run('i1-wrong-'+type,{...request,studentKey:'770099'},viewRaw());
      expect(wrong.answer).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
      expect(wrong.calls()).toBe(0);
    }
  });
});

// Unit ama-fix-report-keys: the keys of the batch reports as the LMS lists them (specs/academy-mcp/direct-key-lookup.md R6, R8).
describe('academy_official_report_pdf_get: the live keys of the batch reports',()=>{
  test('official_report_class_no_zero_is_accepted',async()=>{
    for(const reportType of ['diag','formative','ganga','na_score'] as const){
      for(const classNo of ['0',CLASS,'1'])expect({reportType,classNo,ok:officialReportPdfGetSchema.safeParse({reportType,displayName:STUDENT,params:{testingNo:TESTING,classNo}}).success}).toEqual({reportType,classNo,ok:true});
      for(const classNo of ['00','01','-1','0.5',' 0','','a','1'.repeat(17),'9007199254740992'])
        expect({reportType,classNo,ok:officialReportPdfGetSchema.safeParse({reportType,displayName:STUDENT,params:{testingNo:TESTING,classNo}}).success}).toEqual({reportType,classNo,ok:false});
      expect(officialReportPdfGetSchema.safeParse({reportType,displayName:STUDENT,params:{testingNo:'0',classNo:'0'}}).success).toBe(false);
      expect(officialReportPdfGetSchema.safeParse({reportType,displayName:STUDENT,params:{testingNo:TESTING}}).success).toBe(false);
    }
    expect(reportViewForm('FaView',{testingNo:'902',studentKey:KEY,classNo:'0'}).toString()).toBe('p_process=FaView&p_testing_no=902&p_pri_no='+KEY+'&p_cls_no=0&p_limit_date=');
    for(const view of ['DiagReportView','GaReportView','NaScoreResultView'] as const)expect(reportViewForm(view,{testingNo:'903',studentKey:KEY,classNo:'0'}).get('p_cls_no')).toBe('0');
    for(const bad of [{testingNo:'0',studentKey:KEY,classNo:'0'},{testingNo:'903',studentKey:'0',classNo:'0'},{testingNo:'903',studentKey:KEY,classNo:'00'},{testingNo:'903',studentKey:KEY,classNo:0}])
      expect(()=>reportViewForm('FaView',bad)).toThrow('report_view_params_invalid');
    // A call with classNo 0 renders: the backend is asked with the class 0, report.info carries cls_no 0, and the answer states it.
    const f=viewFixture('class-zero',()=>viewRaw());
    const answer=await f.service.get({reportType:'formative',displayName:STUDENT,params:{testingNo:TESTING,classNo:'0'}}) as any;
    expect(answer).toMatchObject({status:'rendered',reportType:'formative',testingNo:TESTING,classNo:'0',academyWrites:0});
    expect(f.calls.student).toEqual([{view:'FaView',displayName:STUDENT,testingNo:TESTING,classNo:'0'}]);
    expect(f.pc.calls.map(c=>c.keys)).toEqual([{testingNo:TESTING,studentKey:KEY,classNo:'0'}]);
    expect(await readFile(answer.outputPath,'utf8')).toBe('%PDF-1.3 invented');
  });
});
