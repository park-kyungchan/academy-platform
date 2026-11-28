import {describe,expect,spyOn,test} from 'bun:test';
import {mkdtemp,readFile,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {lmsRequestRole,writeForbiddenTransport} from './network-deny.ts';
import {AcademyHttpClient,sourceDomParser} from '../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts';
import {lmsReadServletRoutes} from '../../../../../apps/academy/dc-gang-a/src/lms/lmsRouteRegistry.ts';
import {parsePrestudySearchForm} from '../../../../../apps/academy/dc-gang-a/src/lms/prestudyRead.ts';
import {lmsScheduleRequestPath} from '../../../../../apps/academy/dc-gang-a/src/lms/lmsScheduleRead.ts';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';
import {LmsScheduleService,lmsScheduleGetSchema} from '../src/lms-schedule-tools.ts';
import {ConsolidatedInputInvalid,consolidatedTools,keptTools,planConsolidatedCall,surfaceOrder} from '../src/consolidated-tools.ts';

// specs/academy-mcp/lms-reads.md R1-R9: academy_lms_schedule_get and academy_prestudy_get. Invented keys,
// names and synthetic pages only; 0 live LMS calls. The LMS is a recorded fixture transport that answers
// write_forbidden to every request whose route is classed write or is not in the table "Routes".
const TEACHER='99001',COLLEGE='88001',STUDENT='Invented Student',KEY='77001',WIRE_STUDENT='Invented Wire Student';
const admission={teacherId:TEACHER,teacherLabel:'Invented Teacher',students:[{studentId:KEY,displayName:STUDENT}],sourceBundleSha256:'b'.repeat(64)};
const credential=()=>({username:'fictional',password:'fictional'});
const SESSION='invented-session-token-4471';
const VIDEO_URL='https://cdn.example.invalid/prestudy/1_77001_1_prestudy.mp4';
const ATT='/servlet/controller.cct.tutor.AttendanceServlet',CM='/servlet/controller.coursemanage.CourseManageServlet';
const REPORT_JSP='/app-ui/solution_common/prestudy/prestudy_ai_report.jsp';

const loginPage='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd" type="password"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
const principal=`<a href="/logout">Invented Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="${TEACHER}"></form><form name="form1"><input name="g_pri_no" value="${TEACHER}"><input name="g_clg_no" value="${COLLEGE}"><input name="g_mem_type" value="teacher"></form>`;

// ---- synthetic pages -------------------------------------------------------------------------
const options=(items:[string,string][],selected?:string)=>'<option value="">전체</option>'+items.map(([k,l])=>`<option value="${k}"${k===selected?' selected':''}>${l}</option>`).join('');
const userForm=(teacher=TEACHER)=>`<form name="userForm"><input type="hidden" name="pri_no" value="${teacher}"></form>`;
const ATTENDANCE_HEADERS=['수업일','과정명','수업형태','출결사항','메모'];
const attendanceRows=[
  ['2040-01-14(일)','Invented Course A','정규','출석','Invented memo one'],
  ['2040-01-15(월)','Invented Course A','정규','지각','Invented memo two'],
  ['2040-01-16(화)','Invented Course B','보강','결석','Invented memo three'],
];
function attendancePage(o:{teacher?:string;ym?:string|null;sdate?:string|null;rows?:string[][];headers?:string[];table?:boolean;form?:boolean;tutorField?:boolean;pad?:string}={}){
  const teacher=o.teacher??TEACHER,ym=o.ym===undefined?'204001':o.ym,sdate=o.sdate===undefined?'2040-01':o.sdate;
  const rows=(o.rows??attendanceRows).map(r=>'<tr>'+r.map(c=>`<td>${c}</td>`).join('')+'</tr>').join('');
  return `<html><body>${userForm(teacher)}`+(o.form===false?'':`<form id="form1" name="form1" method="get"><input type="hidden" name="p_process" value="Main">`
    +(o.tutorField===false?'':`<input type="hidden" name="tutor_pri_no" value="${teacher}">`)
    +(ym===null?'':`<input type="hidden" name="ym" value="${ym}">`)+(sdate===null?'':`<input type="hidden" name="sdate" value="${sdate}">`)
    +`<select name="grp_seq">${options([['301','Invented Group A'],['302','Invented Group B']],'301')}</select>`
    +`<select name="stu_obj">${options([[KEY,STUDENT],['77002','Invented Other Student']])}</select></form>`)
    +(o.table===false?'':`<table class="content-table thin"><thead><tr>${(o.headers??ATTENDANCE_HEADERS).map(h=>`<th>${h}</th>`).join('')}</tr></thead><tbody>${rows}</tbody></table>`)
    +(o.pad??'')+'</body></html>';
}
type ListSpec={command:'StudySchedule'|'CourseStudyManager';selects:string[]};
const LISTS:Record<string,ListSpec>={
  study_schedule:{command:'StudySchedule',selects:['grade_no','cls_no','student_no','course']},
  course_study_manager:{command:'CourseStudyManager',selects:['grade_no','semester_no','level_no','status']},
};
function listPage(view:'study_schedule'|'course_study_manager',o:{teacher?:string;dropSelect?:string;list?:boolean;table?:boolean;command?:string;rows?:string[][];userForm?:boolean}={}){
  const spec=LISTS[view]!;
  const rows=(o.rows??[['Invented Course A','월 17:00','Invented Class 1'],['Invented Course B','수 18:00','Invented Class 2']]).map(r=>'<tr>'+r.map(c=>`<td>${c}</td>`).join('')+'</tr>').join('');
  return '<html><body>'+(o.userForm===false?'':userForm(o.teacher))+`<form id="form1" name="form1"><input type="hidden" name="reqCmd" value="${o.command??spec.command}">`
    +spec.selects.filter(s=>s!==o.dropSelect).map(s=>`<select name="${s}">${options([['1',`Invented ${s} one`],['2',`Invented ${s} two`]],'2')}</select>`).join('')+'</form>'
    +(o.list===false?'':`<div class="content-list">`+(o.table===false?'':`<table class="content-table"><thead><tr><th>과정</th><th>시간</th><th>반</th></tr></thead><tbody>${rows}</tbody></table>`)+'</div>')+'</body></html>';
}
const prestudyForm=(cmd:string,teacher=TEACHER)=>`<form name="form1"><input type="hidden" name="reqCmd" value="${cmd}"><input type="hidden" name="g_clg_no" value="${COLLEGE}"><input type="hidden" name="g_mem_type" value="1"><input type="hidden" name="g_pri_no" value="${teacher}"><input type="text" name="student_name" value=""></form>`;
function prestudyList(cmd:'WebUnPreStudy'|'WebCtPreStudy',o:{student?:string;teacher?:string;extraRows?:number;pad?:string}={}){
  const student=o.student??STUDENT;
  const extra=Array.from({length:o.extraRows??0},(_,i)=>`<tr><td>Other Student ${i}</td><td>9-2 Other Unit</td><td>40.01.02 04:05</td><td>OK</td><td><button onclick="prestudy_main(${20000+i});">예습 검사</button></td></tr>`).join('');
  return `<html>${prestudyForm(cmd,o.teacher)}<table><tr><th>학생이름</th><th>단원명</th><th>날짜</th><th>영상 상태</th><th></th></tr>`
    +`<tr><td>${student}</td><td>9-1 Invented Unit (기본)</td><td>40.01.02 03:04</td><td>OK</td><td><button onclick="prestudy_main(${cmd==='WebUnPreStudy'?12345:12346});">예습 검사</button></td></tr>${extra}</table>${o.pad??''}</html>`;
}
function summaryPage(o:{student?:string;extraRows?:number;pad?:string}={}){
  const student=o.student??STUDENT;
  const extra=Array.from({length:o.extraRows??0},(_,i)=>`<tr><td>Other Student ${i}</td><td>9-2 Other Unit</td><td>40.01.02 04:05</td><td><span>70점</span> <button onclick="openReport(${900+i});">보고서</button></td><td>-</td><td><button onclick="playVideo('${VIDEO_URL}')">재생</button></td></tr>`).join('');
  return `<table><tr><th>학생</th><th>단원명</th><th>날짜</th><th>Ai총점</th><th>별점</th><th>action</th></tr>`
    +`<tr><td>${student}(초5)</td><td>9-1 Invented Unit</td><td>40.01.02 03:04</td><td><span>77점</span> <button onclick="openReport(777);">보고서</button></td><td>-</td><td><button onclick="playVideo('${VIDEO_URL}')">재생</button></td></tr>${extra}</table>${o.pad??''}`;
}
const detailPage=`<form name="form1"><input name="student_pri_no" value="${KEY}"><input name="prestudy_key" value="12346"><input name="sunit_name" value="9-1 Invented Unit"><input name="create_datetime" value="2040-01-02 03:04:05"><input name="encoding_result" value="success"></form><button onclick="viewAiReport('55501')">report</button><video src="${VIDEO_URL}"></video>`;
const reportPage='<div class="max-w-5xl rounded-2xl"><h1>개념 이해</h1><h3>reason</h3><div class="text-sm">Invented narrative</div></div>';

type Answer=string|Response|((body:URLSearchParams,u:URL)=>string|Response);
type Pages=Record<string,Answer>;
const opOf=(u:URL,body:URLSearchParams)=>u.pathname===REPORT_JSP?'prestudy_ai_report':u.searchParams.get('p_process')??u.searchParams.get('reqCmd')??body.get('reqCmd')??body.get('p_process')??'';
const defaults=(name=STUDENT):Pages=>({
  Main:attendancePage(),StudySchedule:listPage('study_schedule'),CourseStudyManager:listPage('course_study_manager'),
  WebUnPreStudy:prestudyList('WebUnPreStudy',{student:name}),WebCtPreStudy:prestudyList('WebCtPreStudy',{student:name}),TeacherPrestudySummary:summaryPage({student:name}),
  WebPreStudy:detailPage,prestudy_ai_report:reportPage,DtResultSearch:'<html>invented search result</html>',
});
/** The real LMS client over a recorded synthetic LMS: the login exchange, then the pages given by operation. */
function wire(over:Pages={},name=STUDENT){
  const pages={...defaults(name),...over};
  const seen:{method:string;op:string;url:string}[]=[];
  const guard=writeForbiddenTransport((url,init)=>{
    const u=new URL(url),body=new URLSearchParams(typeof init?.body==='string'?init.body:'');
    if(u.pathname==='/c_login.jsp')return new Response(loginPage,{headers:{'Set-Cookie':`JSESSIONID=${SESSION}; Path=/`}});
    if(u.pathname==='/servlet/controller.login.LoginServlet'||u.searchParams.get('reqCmd')==='StudyCourse')return new Response(principal);
    const op=opOf(u,body);
    seen.push({method:(init?.method??'GET').toUpperCase(),op,url:u.pathname+u.search});
    const answer=pages[op];
    if(answer===undefined)throw Error('unexpected_fixture_request:'+op);
    const out=typeof answer==='function'?answer(body,u):answer;
    return typeof out==='string'?new Response(out):out.clone();
  });
  const client=new AcademyHttpClient({teacherLabel:'Invented Teacher',teacherKey:TEACHER,collegeKey:COLLEGE},guard.transport as never);
  // The student name search of the table "Routes" is a recorded read of the same transport; the pairs are invented.
  (client as any).searchStudentsByName=async(input:{studentName:string})=>{
    await guard.transport('https://dc.gang-a.kr/servlet/controller.dailyzerotest.DailyZeroTestServlet?reqCmd=DtResultSearch',
      {method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},body:new URLSearchParams({sort_date:'1',stu_name:input.studentName}).toString()});
    return [{studentKey:KEY,studentName:input.studentName}];
  };
  const backend=new NativeDailyRecordBackend(client as never,admission,credential);
  const logins=()=>guard.requests.filter(r=>r.role==='login'&&r.route.endsWith('c_login')).length;
  return {guard,backend,client,seen,logins,schedule:new LmsScheduleService(backend),prestudy:new DailyRecordService(backend),
    ops:()=>seen.map(s=>s.method+' '+s.op)};
}
const ask=(displayName=STUDENT,extra:Record<string,unknown>={})=>({displayName,purpose:'Read invented prestudy state',currentRequestRef:'lms-reads-synthetic',...extra});
const json=(value:unknown)=>JSON.stringify(value);

// ---- R1 ---------------------------------------------------------------------------------------
describe('lms-reads R1: each view sends only the requests of its own rows',()=>{
  test('lms_reads_r1_schedule_views_send_one_get_of_their_own_page',async()=>{
    const cases:[Record<string,unknown>,string,Record<string,unknown>?][]=[
      [{view:'attendance'},'GET Main'],
      [{view:'study_schedule'},'GET StudySchedule'],
      [{view:'course_study_manager'},'GET CourseStudyManager'],
    ];
    for(const [input,op] of cases){
      const w=wire();
      const out:any=await w.schedule.get(input);
      expect({view:input.view,status:out.status,ops:w.ops()}).toEqual({view:input.view,status:'observed',ops:[op]});
      expect(out).toMatchObject({view:input.view,academyWrites:0,parentSends:0,execution:'read_only'});
      expect(w.logins()).toBe(1);
    }
  });
  test('lms_reads_r1_schedule_filters_use_the_pages_own_parameter_names',async()=>{
    const paths=[
      [{view:'attendance'},ATT+'?p_process=Main'],
      [{view:'attendance',date:'2040-01-15',groupKey:'301',studentKey:KEY},`${ATT}?p_process=Main&ym=204001&sdate=2040-01&grp_seq=301&stu_obj=${KEY}`],
      [{view:'attendance',period:{from:'2040-01-10',through:'2040-01-20'}},`${ATT}?p_process=Main&ym=204001&sdate=2040-01`],
      [{view:'study_schedule',gradeNo:'5',groupKey:'301',studentKey:KEY,courseKey:'672'},`${CM}?reqCmd=StudySchedule&grade_no=5&cls_no=301&student_no=${KEY}&course=672`],
      [{view:'course_study_manager'},`${CM}?reqCmd=CourseStudyManager`],
      [{view:'course_study_manager',gradeNo:'5',semesterNo:'1',levelNo:'3',status:'ING'},`${CM}?reqCmd=CourseStudyManager&grade_no=5&semester_no=1&level_no=3&status=ING&search=1`],
    ] as const;
    for(const [input,path] of paths)expect(lmsScheduleRequestPath(input as never)).toBe(path);
    const w=wire();
    await w.schedule.get({view:'attendance',date:'2040-01-15',groupKey:'301',studentKey:KEY});
    expect(w.seen.map(s=>s.url)).toEqual([paths[1]![1]]);
  });
  test('lms_reads_r1_schedule_rows_and_filter_options_are_parsed_from_the_pinned_shapes',async()=>{
    const w=wire();
    const all:any=await w.schedule.get({view:'attendance'});
    expect(all.rows).toEqual([
      {lessonDate:'2040-01-14',courseName:'Invented Course A',lessonType:'정규',attendanceStatus:'출석',memo:'Invented memo one'},
      {lessonDate:'2040-01-15',courseName:'Invented Course A',lessonType:'정규',attendanceStatus:'지각',memo:'Invented memo two'},
      {lessonDate:'2040-01-16',courseName:'Invented Course B',lessonType:'보강',attendanceStatus:'결석',memo:'Invented memo three'}]);
    expect(all.groups).toEqual([{key:'301',label:'Invented Group A',selected:true},{key:'302',label:'Invented Group B',selected:false}]);
    expect(all.students).toEqual([{key:KEY,label:STUDENT,selected:false},{key:'77002',label:'Invented Other Student',selected:false}]);
    // A date or a period filters the rows to that inclusive range.
    expect((await w.schedule.get({view:'attendance',date:'2040-01-15'}) as any).rows.map((r:any)=>r.lessonDate)).toEqual(['2040-01-15']);
    expect((await w.schedule.get({view:'attendance',period:{from:'2040-01-15',through:'2040-01-16'}}) as any).rows.map((r:any)=>r.lessonDate)).toEqual(['2040-01-15','2040-01-16']);
    expect((await w.schedule.get({view:'attendance',period:{from:'2040-01-17',through:'2040-01-20'}}) as any).rows).toEqual([]);
    const schedule:any=await w.schedule.get({view:'study_schedule'});
    expect(schedule).toMatchObject({status:'observed',columns:['과정','시간','반'],rows:[{과정:'Invented Course A',시간:'월 17:00',반:'Invented Class 1'},{과정:'Invented Course B'}]});
    expect(schedule.grades).toEqual([{key:'1',label:'Invented grade_no one',selected:false},{key:'2',label:'Invented grade_no two',selected:true}]);
    expect(Object.keys(schedule).filter(k=>['grades','classes','students','courses'].includes(k))).toEqual(['grades','classes','students','courses']);
    const manager:any=await w.schedule.get({view:'course_study_manager'});
    expect(manager).toMatchObject({status:'observed',rows:[{과정:'Invented Course A'},{과정:'Invented Course B'}]});
    expect(Object.keys(manager).filter(k=>['grades','semesters','levels','statuses'].includes(k))).toEqual(['grades','semesters','levels','statuses']);
  });
  test('lms_reads_r1_prestudy_views_send_only_the_requests_of_their_own_rows',async()=>{
    const cases:[Record<string,unknown>,string[],string[]][]=[
      [{},['GET WebUnPreStudy','POST WebUnPreStudy','GET WebCtPreStudy','POST WebCtPreStudy','GET TeacherPrestudySummary'],['waiting','completed','summary']],
      [{view:'videos'},['GET WebUnPreStudy','POST WebUnPreStudy','GET WebCtPreStudy','POST WebCtPreStudy','GET TeacherPrestudySummary'],['waiting','completed','summary']],
      [{view:'incomplete'},['GET WebUnPreStudy','POST WebUnPreStudy'],['waiting']],
      [{view:'teacher_summary'},['GET TeacherPrestudySummary'],['summary']],
    ];
    for(const [extra,ops,lists] of cases){
      const w=wire();
      const out:any=await w.prestudy.prestudyGet(ask(STUDENT,extra));
      expect({extra,status:out.status,ops:w.ops()}).toEqual({extra,status:'observed',ops});
      expect({extra,lists:['waiting','completed','summary'].filter(k=>out[k]!==undefined)}).toEqual({extra,lists});
      expect(out.waiting===undefined||out.waiting[0]?.prestudyKey==='12345').toBe(true);
    }
  });
  test('lms_reads_r1_report_id_reads_the_report_row_once_for_that_key',async()=>{
    const w=wire();
    const out:any=await w.prestudy.prestudyReportGet({...ask(),prestudyKey:'12346'});
    expect(out).toMatchObject({status:'observed',prestudyKey:'12346',sections:[{title:'개념 이해'}]});
    expect(w.ops()).toEqual(['GET WebPreStudy','GET prestudy_ai_report']);
    expect(w.seen.map(s=>s.url)).toEqual([`${CM}?reqCmd=WebPreStudy&prestudy_key=12346`,`${REPORT_JSP}?analysis_key=55501`]);
  });
  test('lms_reads_r1_the_bridge_forwards_the_view_and_pairs_include_reports_with_report_id',()=>{
    const plan=(args:Record<string,unknown>)=>planConsolidatedCall('academy_prestudy_get',{displayName:STUDENT,...args});
    expect(plan({}).steps.map(s=>[s.route,s.args])).toEqual([['videos',{displayName:STUDENT}]]);
    expect(plan({view:'videos'}).steps.map(s=>[s.route,s.args])).toEqual([['videos',{displayName:STUDENT}]]);
    expect(plan({view:'incomplete'}).steps.map(s=>[s.route,s.args])).toEqual([['videos',{displayName:STUDENT,view:'incomplete'}]]);
    expect(plan({view:'teacher_summary'}).steps.map(s=>[s.route,s.args])).toEqual([['videos',{displayName:STUDENT,view:'teacher_summary'}]]);
    expect(plan({includeReports:true,reportId:'12346'}).steps.map(s=>s.route)).toEqual(['report']);
    for(const args of [{includeReports:true},{reportId:'12346'},{includeReports:false,reportId:'12346'},{view:'reports'}])
      expect(()=>plan(args)).toThrow(ConsolidatedInputInvalid);
  });
});

// ---- R2 ---------------------------------------------------------------------------------------
describe('lms-reads R2: no write request',()=>{
  const SEARCH_POSTS=new Set(['WebUnPreStudy','WebCtPreStudy','DtResultSearch']);
  test('lms_reads_r2_all_views_send_only_gets_and_the_named_search_posts_of_read_routes',async()=>{
    const w=wire({},WIRE_STUDENT);
    for(const input of [{view:'attendance'},{view:'study_schedule'},{view:'course_study_manager'}])expect((await w.schedule.get(input) as any).status).toBe('observed');
    for(const extra of [{},{view:'incomplete'},{view:'teacher_summary'}])expect((await w.prestudy.prestudyGet(ask(WIRE_STUDENT,extra)) as any).status).toBe('observed');
    expect((await w.prestudy.prestudyReportGet({...ask(WIRE_STUDENT),prestudyKey:'12346'}) as any).status).toBe('observed');
    expect(w.guard.requests.every(r=>r.role==='login'||r.role==='read')).toBe(true);
    for(const s of w.seen)if(s.method!=='GET')expect({op:s.op,allowed:SEARCH_POSTS.has(s.op)}).toEqual({op:s.op,allowed:true});
    expect(w.seen.some(s=>s.op==='DtResultSearch')).toBe(true);
    // The only POSTs besides the login exchange are the named search forms; every other request is a GET.
    expect(new Set(w.seen.filter(s=>s.method==='POST').map(s=>s.op))).toEqual(SEARCH_POSTS);
  });
  test('lms_reads_r2_the_eight_read_routes_are_classed_read_and_a_neighbouring_route_is_write',()=>{
    const routes=lmsReadServletRoutes();
    expect(routes.map(r=>r.operation+'|'+r.servletPath)).toEqual([
      `Main|${ATT}`,`StudySchedule|${CM}`,`CourseStudyManager|${CM}`,`WebUnPreStudy|${CM}`,`WebCtPreStudy|${CM}`,`TeacherPrestudySummary|${CM}`,`WebPreStudy|${CM}`,
      `prestudy_ai_report|${REPORT_JSP}`]);
    for(const route of routes){
      expect({id:route.id,effect:route.semanticEffect,safe:route.safeToProbe}).toEqual({id:route.id,effect:'read',safe:true});
      expect({id:route.id,role:lmsRequestRole('https://dc.gang-a.kr'+route.routeTemplate)}).toEqual({id:route.id,role:'read'});
    }
    for(const url of [`${ATT}?p_process=Insert`,`${ATT}?p_process=Update`,`${CM}?reqCmd=StudyScheduleSave`,`${CM}?reqCmd=CourseStudyManagerDelete`,`${CM}?reqCmd=WebPreStudyDelete`,'/app-ui/solution_common/prestudy/prestudy_ai_report_save.jsp'])
      expect({url,role:lmsRequestRole('https://dc.gang-a.kr'+url)}).toEqual({url,role:'write'});
  });
});

// ---- R3 ---------------------------------------------------------------------------------------
describe('lms-reads R3: the shared account lock',()=>{
  test('lms_reads_r3_a_held_account_lock_ends_each_call_with_owner_account_busy_and_sends_no_request',async()=>{
    // Through the bridge: an endpoint launched as `flock --wait 0 --conflict-exit-code 75 <lock> …` while the lock is held.
    const base=await mkdtemp(resolve(tmpdir(),'lms-reads-busy-'));
    const deny=resolve(import.meta.dir,'network-deny.ts'),fixture=resolve(import.meta.dir,'tool-surface-fixture.ts'),entry=resolve(import.meta.dir,'../src/neutral-server.ts');
    const lock=resolve(base,'account.lock'),events=resolve(base,'events.txt');
    let holder:ReturnType<typeof Bun.spawn>|undefined,client:Client|undefined,transport:StdioClientTransport|undefined;
    try{
      await writeFile(events,'');await writeFile(lock,'');
      await writeFile(resolve(base,'endpoints.json'),JSON.stringify({dailyrecord:{command:'/usr/bin/flock',
        args:['--wait','0','--conflict-exit-code','75',lock,process.execPath,'--preload',deny,fixture,'dailyrecord',events]}}),{mode:0o600});
      await writeFile(resolve(base,'routing.json'),'{}',{mode:0o600});
      const env={PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base};
      const discovery=Bun.spawn([process.execPath,'--preload',deny,entry,'--discover'],{env,stdout:'pipe',stderr:'pipe'});
      expect(await discovery.exited).toBe(0);
      // The holder keeps the lock until its standard input closes.
      holder=Bun.spawn(['/usr/bin/flock',lock,'/bin/sh','-c','read held'],{stdin:'pipe',stdout:'ignore',stderr:'ignore'});
      for(let i=0;i<200&&Bun.spawnSync(['/usr/bin/flock','--nonblock',lock,'true']).exitCode===0;i++)await Bun.sleep(10);
      expect(Bun.spawnSync(['/usr/bin/flock','--nonblock',lock,'true']).exitCode).toBe(1);
      client=new Client({name:'fictional-busy-client',version:'1.0.0'});
      transport=new StdioClientTransport({command:process.execPath,args:['--preload',deny,entry],env,stderr:'pipe'});
      await client.connect(transport);
      for(const [name,args] of [['academy_lms_schedule_get',{view:'attendance'}],['academy_lms_schedule_get',{view:'study_schedule'}],
        ['academy_lms_schedule_get',{view:'course_study_manager'}],['academy_prestudy_get',{displayName:STUDENT}],['academy_prestudy_get',{displayName:STUDENT,view:'incomplete'}]] as const){
        const result:any=await client.callTool({name,arguments:{...args}});
        expect({name,isError:result.isError}).toEqual({name,isError:true});
        expect(result.structuredContent).toMatchObject({status:'blocked',code:'owner_account_busy',academyWrites:0});
      }
      expect(await readFile(events,'utf8')).toBe('');
    }finally{
      await client?.close();await transport?.close();
      if(holder){(holder.stdin as {end():void}).end();await Promise.race([holder.exited,Bun.sleep(2000)]);holder.kill('SIGKILL');}
      await rm(base,{recursive:true,force:true});
    }
  },30000);
});

// ---- R4 ---------------------------------------------------------------------------------------
describe('lms-reads R4: a login or session page refuses and no login is retried',()=>{
  const refusals:[string,()=>string|Response][]=[
    ['a login page',()=>loginPage],
    ['normal_session_error.jsp',()=>'<script>location.href="/common/normal_session_error.jsp"</script>'],
    ['popup_session_error.jsp',()=>'<script>location.href="/common/popup_session_error.jsp"</script>'],
    ['a page with a password input',()=>'<html><form id="form1"><input type="password" name="p"></form></html>'],
    ['a redirect to the login page',()=>new Response('',{status:302,headers:{location:'/c_login.jsp'}})],
    ['another 3xx redirect',()=>new Response('',{status:301,headers:{location:'/servlet/controller.coursemanage.CourseManageServlet?reqCmd=Other'}})],
    ['HTTP 401',()=>new Response('denied',{status:401})],
    ['HTTP 403',()=>new Response('denied',{status:403})],
  ];
  const scheduleOps:[Record<string,unknown>,string][]=[[{view:'attendance'},'Main'],[{view:'study_schedule'},'StudySchedule'],[{view:'course_study_manager'},'CourseStudyManager']];
  test('lms_reads_r4_schedule_views_refuse_with_authentication_required_and_0_login_retries',async()=>{
    for(const [label,page] of refusals)for(const [input,op] of scheduleOps){
      const w=wire({[op]:()=>page()});
      const out:any=await w.schedule.get(input);
      expect({label,view:input.view,status:out.status,reasonCode:out.reasonCode}).toEqual({label,view:input.view,status:'unavailable',reasonCode:'authentication_required'});
      expect(out.rows).toBeUndefined();
      expect(out.failure).toMatchObject({category:'authentication',code:'authentication_required',automaticRetryAllowed:false});
      const requests=w.guard.requests.length;
      expect(w.logins()).toBe(1);
      // The session is closed: the next call is refused before any request, and no login is sent again.
      const again:any=await w.schedule.get(input);
      expect({label,reasonCode:again.reasonCode}).toEqual({label,reasonCode:'authentication_required'});
      expect(w.guard.requests.length).toBe(requests);
      expect(w.logins()).toBe(1);
      expect(w.client.authenticated).toBe(false);
    }
  });
  test('lms_reads_r4_prestudy_views_refuse_with_authentication_required_and_0_login_retries',async()=>{
    for(const [label,page] of refusals)for(const [extra,op] of [[{},'WebUnPreStudy'],[{view:'incomplete'},'WebUnPreStudy'],[{view:'teacher_summary'},'TeacherPrestudySummary']] as const){
      const w=wire({[op]:()=>page()});
      const out:any=await w.prestudy.prestudyGet(ask(STUDENT,extra));
      expect({label,extra,status:out.status,reasonCode:out.reasonCode}).toEqual({label,extra,status:'unavailable',reasonCode:'authentication_required'});
      expect(out.waiting??out.completed??out.summary).toBeUndefined();
      const requests=w.guard.requests.length;
      expect(w.logins()).toBe(1);
      expect((await w.prestudy.prestudyGet(ask(STUDENT,extra)) as any).reasonCode).toBe('authentication_required');
      expect(w.guard.requests.length).toBe(requests);
      expect(w.logins()).toBe(1);
    }
    // The report row refuses the same way, at the detail page and at the report page.
    for(const op of ['WebPreStudy','prestudy_ai_report']){
      const w=wire({[op]:()=>loginPage});
      expect(await w.prestudy.prestudyReportGet({...ask(),prestudyKey:'12346'})).toMatchObject({status:'unavailable',reasonCode:'authentication_required'});
      expect(w.logins()).toBe(1);
    }
  });
  test('lms_reads_r4_a_session_page_after_the_search_form_post_refuses_too',async()=>{
    const w=wire({WebUnPreStudy:(body)=>body.has('student_name')?loginPage:prestudyList('WebUnPreStudy')});
    expect(await w.prestudy.prestudyGet(ask())).toMatchObject({status:'unavailable',reasonCode:'authentication_required'});
    expect(w.ops()).toEqual(['GET WebUnPreStudy','POST WebUnPreStudy']);
  });
  test('lms_reads_r4_the_parser_keeps_its_own_authentication_throw',()=>{
    expect(()=>parsePrestudySearchForm('<input type=password>'+prestudyList('WebUnPreStudy'),sourceDomParser.parseFromString,'WebUnPreStudy',TEACHER)).toThrow('prestudy_authentication_required');
  });
});

// ---- R5 ---------------------------------------------------------------------------------------
describe('lms-reads R5: scope_conflict and source_drift',()=>{
  const code=(out:any)=>({status:out.status,reasonCode:out.reasonCode,rows:out.rows});
  const refused=(reasonCode:string)=>({status:'unavailable',reasonCode,rows:undefined});
  test('lms_reads_r5_attendance_scope_fields_and_shape',async()=>{
    const cases:[string,Pages,Record<string,unknown>,string][]=[
      ['another teacher in tutor_pri_no',{Main:attendancePage({teacher:'99002'})},{view:'attendance'},'scope_conflict'],
      ['another teacher in the page user form',{Main:attendancePage().replace(`name="pri_no" value="${TEACHER}"`,'name="pri_no" value="99002"')},{view:'attendance'},'scope_conflict'],
      ['another month in ym',{Main:attendancePage({ym:'204002'})},{view:'attendance',date:'2040-01-15'},'scope_conflict'],
      ['another month in sdate',{Main:attendancePage({sdate:'2040-02'})},{view:'attendance',period:{from:'2040-01-15',through:'2040-01-16'}},'scope_conflict'],
      ['no month field to compare with the requested month',{Main:attendancePage({ym:null,sdate:null})},{view:'attendance',date:'2040-01-15'},'source_drift'],
      ['no teacher field at all',{Main:attendancePage({tutorField:false}).replace(userForm(),'')},{view:'attendance'},'source_drift'],
      ['no form1',{Main:attendancePage({form:false})},{view:'attendance'},'source_drift'],
      ['no content table',{Main:attendancePage({table:false})},{view:'attendance'},'source_drift'],
      ['another set of headers',{Main:attendancePage({headers:['수업일','과정명','수업형태','출결','메모']})},{view:'attendance'},'source_drift'],
      ['four headers',{Main:attendancePage({headers:ATTENDANCE_HEADERS.slice(0,4)})},{view:'attendance'},'source_drift'],
      ['a row of four cells',{Main:attendancePage({rows:[['2040-01-15','a','b','c']]})},{view:'attendance'},'source_drift'],
      ['a lesson date no filter can read',{Main:attendancePage({rows:[['someday','a','b','c','d']]})},{view:'attendance',date:'2040-01-15'},'source_drift'],
    ];
    for(const [label,pages,input,reasonCode] of cases){
      const out:any=await wire(pages).schedule.get(input);
      expect({label,...code(out)}).toEqual({label,...refused(reasonCode)});
    }
  });
  test('lms_reads_r5_study_schedule_and_course_study_manager_scope_fields_and_shape',async()=>{
    for(const view of ['study_schedule','course_study_manager'] as const){
      const op=view==='study_schedule'?'StudySchedule':'CourseStudyManager';
      const cases:[string,string,string][]=[
        ['another teacher',listPage(view,{teacher:'99002'}),'scope_conflict'],
        ['no user form',listPage(view,{userForm:false}),'source_drift'],
        ['no reqCmd of that command',listPage(view,{command:'Other'}),'source_drift'],
        ['no content-list',listPage(view,{list:false}),'source_drift'],
        ['no content table',listPage(view,{table:false}),'source_drift'],
        ...LISTS[view]!.selects.map(select=>[`no select ${select}`,listPage(view,{dropSelect:select}),'source_drift'] as [string,string,string]),
      ];
      for(const [label,html,reasonCode] of cases){
        const out:any=await wire({[op]:html}).schedule.get({view});
        expect({view,label,...code(out)}).toEqual({view,label,...refused(reasonCode)});
      }
    }
  });
  test('lms_reads_r5_prestudy_search_form_scope_and_shape',async()=>{
    const cases:[string,Pages,Record<string,unknown>,string][]=[
      ['another teacher in the search form',{WebUnPreStudy:prestudyList('WebUnPreStudy',{teacher:'99002'})},{},'scope_conflict'],
      ['no search form',{WebUnPreStudy:'<html><table></table></html>'},{},'source_drift'],
      ['the form of another command',{WebUnPreStudy:prestudyList('WebCtPreStudy')},{view:'incomplete'},'source_drift'],
      ['a completed list of another teacher',{WebCtPreStudy:prestudyList('WebCtPreStudy',{teacher:'99002'})},{},'scope_conflict'],
      ['a row that is not a row',{WebUnPreStudy:prestudyList('WebUnPreStudy').replace('<td>OK</td>','').replace('<td>40.01.02 03:04</td>','')},{view:'incomplete'},'source_drift'],
    ];
    for(const [label,pages,extra,reasonCode] of cases){
      const out:any=await wire(pages).prestudy.prestudyGet(ask(STUDENT,extra));
      expect({label,status:out.status,reasonCode:out.reasonCode}).toEqual({label,status:'unavailable',reasonCode});
      expect(out.waiting).toBeUndefined();
    }
  });
});

// ---- R6 ---------------------------------------------------------------------------------------
describe('lms-reads R6: I1 write_forbidden',()=>{
  test('lms_reads_r6_every_view_of_both_tools_completes_with_0_forbidden_requests',async()=>{
    const w=wire({},WIRE_STUDENT);
    const outs:any[]=[];
    for(const input of [{view:'attendance'},{view:'attendance',date:'2040-01-15',groupKey:'301',studentKey:KEY},{view:'study_schedule',gradeNo:'2'},{view:'course_study_manager',status:'ING'}])
      outs.push(await w.schedule.get(input));
    for(const extra of [{},{view:'videos'},{view:'incomplete'},{view:'teacher_summary'}])outs.push(await w.prestudy.prestudyGet(ask(WIRE_STUDENT,extra)));
    outs.push(await w.prestudy.prestudyReportGet({...ask(WIRE_STUDENT),prestudyKey:'12346'}));
    expect(outs.map(o=>o.status)).toEqual(Array(outs.length).fill('observed'));
    expect(w.guard.forbidden()).toEqual([]);
    expect(w.guard.requests.filter(r=>r.role==='write')).toHaveLength(0);
    expect(w.guard.requests.length).toBeGreaterThan(10);
  });
  test('lms_reads_r6_the_transport_answers_write_forbidden_outside_the_table',async()=>{
    const guard=writeForbiddenTransport(()=>new Response('page'));
    for(const url of [`https://dc.gang-a.kr${ATT}?p_process=Delete`,`https://dc.gang-a.kr${CM}?reqCmd=StudyScheduleSave`,'https://dc.gang-a.kr/servlet/controller.unknown.Servlet?p_process=Main']){
      const response=await guard.transport(url,{method:'GET'});
      expect({url,status:response.status,text:await response.text()}).toEqual({url,status:403,text:'write_forbidden'});
    }
    expect(guard.forbidden()).toHaveLength(3);
    expect((await guard.transport(`https://dc.gang-a.kr${ATT}?p_process=Main`,{method:'GET'})).status).toBe(200);
    expect(guard.forbidden()).toHaveLength(3);
  });
});

// ---- R7 ---------------------------------------------------------------------------------------
describe('lms-reads R7: invalid dates and periods refuse before any request',()=>{
  const invalid:[string,Record<string,unknown>][]=[
    ['a day that does not exist',{view:'attendance',date:'2040-02-30'}],
    ['29 February of a common year',{view:'attendance',date:'2039-02-29'}],
    ['month 13',{view:'attendance',date:'2040-13-01'}],
    ['a date without zero padding',{view:'attendance',date:'2040-1-5'}],
    ['a date without separators',{view:'attendance',date:'20400115'}],
    ['a word',{view:'attendance',date:'tomorrow'}],
    ['a date with a time',{view:'attendance',date:'2040-01-15T00:00:00'}],
    ['an invalid period start',{view:'attendance',period:{from:'2040-02-30',through:'2040-03-01'}}],
    ['an invalid period end',{view:'attendance',period:{from:'2040-01-01',through:'2040-01-32'}}],
    ['a period of 32 days',{view:'attendance',period:{from:'2040-01-01',through:'2040-02-01'}}],
    ['a period of 90 days',{view:'attendance',period:{from:'2040-01-01',through:'2040-03-30'}}],
    ['a period that ends before it starts',{view:'attendance',period:{from:'2040-01-16',through:'2040-01-15'}}],
    ['a period of at most 31 days across two months',{view:'attendance',period:{from:'2040-01-20',through:'2040-02-10'}}],
    ['a period across two years',{view:'attendance',period:{from:'2040-12-25',through:'2041-01-05'}}],
    ['a date and a period together',{view:'attendance',date:'2040-01-15',period:{from:'2040-01-15',through:'2040-01-16'}}],
    ['a period with another member',{view:'attendance',period:{from:'2040-01-15',through:'2040-01-16',extra:'x'}}],
    ['a period without its end',{view:'attendance',period:{from:'2040-01-15'}}],
    ['a date for study_schedule',{view:'study_schedule',date:'2040-01-15'}],
    ['a period for course_study_manager',{view:'course_study_manager',period:{from:'2040-01-15',through:'2040-01-16'}}],
    ['an unknown view',{view:'attendance_edit'}],
    ['no view',{date:'2040-01-15'}],
    ['an unknown member',{view:'attendance',date:'2040-01-15',write:true}],
    ['a group key that is not a key',{view:'attendance',groupKey:'abc'}],
    ['a student key with a path',{view:'attendance',studentKey:'1/../2'}],
    ['a grade with a query',{view:'study_schedule',gradeNo:'5&p_process=Save'}],
    ['a status with a query',{view:'course_study_manager',status:'ING&search=0'}],
    ['an empty status',{view:'course_study_manager',status:''}],
  ];
  test('lms_reads_r7_invalid_dates_and_periods_refuse_before_any_request',async()=>{
    const w=wire();
    for(const [label,input] of invalid){
      expect({label,parsed:lmsScheduleGetSchema.safeParse(input).success}).toEqual({label,parsed:false});
      let refusal:unknown;
      try{await w.schedule.get(input);}catch(error){refusal=error;}
      expect({label,refused:refusal!==undefined}).toEqual({label,refused:true});
    }
    // Not even the login exchange was sent.
    expect(w.guard.requests).toEqual([]);
    expect(w.seen).toEqual([]);
  });
  test('lms_reads_r7_a_real_calendar_date_and_a_period_of_31_days_in_one_month_are_accepted',async()=>{
    for(const input of [{view:'attendance',date:'2040-02-29'},{view:'attendance',period:{from:'2040-01-01',through:'2040-01-31'}},
      {view:'attendance',period:{from:'2040-01-15',through:'2040-01-15'}},{view:'attendance',period:{from:'2040-12-01',through:'2040-12-31'}}])
      expect({input,parsed:lmsScheduleGetSchema.safeParse(input).success}).toEqual({input,parsed:true});
  });
  test('lms_reads_r7_each_view_refuses_the_members_of_the_other_views',()=>{
    const members:Record<string,unknown>={date:'2040-01-15',period:{from:'2040-01-15',through:'2040-01-16'},groupKey:'301',studentKey:KEY,gradeNo:'5',semesterNo:'1',levelNo:'3',courseKey:'672',status:'ING'};
    const allowed:Record<string,string[]>={attendance:['date','period','groupKey','studentKey'],study_schedule:['gradeNo','groupKey','studentKey','courseKey'],course_study_manager:['gradeNo','semesterNo','levelNo','status']};
    for(const [view,list] of Object.entries(allowed))for(const [member,value] of Object.entries(members)){
      const parsed=lmsScheduleGetSchema.safeParse({view,[member]:value}).success;
      expect({view,member,parsed}).toEqual({view,member,parsed:list.includes(member)});
    }
    // The members of one view together.
    expect(lmsScheduleGetSchema.safeParse({view:'attendance',date:'2040-01-15',groupKey:'301',studentKey:KEY}).success).toBe(true);
    expect(lmsScheduleGetSchema.safeParse({view:'study_schedule',gradeNo:'5',groupKey:'301',studentKey:KEY,courseKey:'672'}).success).toBe(true);
    expect(lmsScheduleGetSchema.safeParse({view:'course_study_manager',gradeNo:'5',semesterNo:'1',levelNo:'3',status:'ING'}).success).toBe(true);
  });
  test('lms_reads_r7_the_client_refuses_the_same_input_before_a_request',async()=>{
    const w=wire();
    await w.backend.client.authenticate?.(credential());
    const before=w.guard.requests.length;
    for(const [,input] of invalid.slice(0,6))await expect((w.backend.client as any).readLmsSchedule({...input,teacherLabel:'Invented Teacher'})).rejects.toThrow('tool_input_invalid');
    expect(w.guard.requests.length).toBe(before);
  });
});

// ---- R8 ---------------------------------------------------------------------------------------
describe('lms-reads R8: a page over 2 MiB or over 1000 rows is source_rejected',()=>{
  const MiB2=2*1024*1024;
  /** A page whose UTF-8 size is over 2 MiB while its character count is under 2 MiB: bytes are measured, not characters. */
  const hangulPad='<!--'+'가'.repeat(750_000)+'-->';
  test('lms_reads_r8_schedule_pages_over_2_MiB_are_source_rejected',async()=>{
    expect(hangulPad.length).toBeLessThan(MiB2);expect(Buffer.byteLength(hangulPad,'utf8')).toBeGreaterThan(MiB2);
    const asciiPad='<!--'+'x'.repeat(MiB2)+'-->';
    const cases:[string,string,Record<string,unknown>][]=[
      ['attendance',attendancePage({pad:hangulPad}),{view:'attendance'}],['attendance ascii',attendancePage({pad:asciiPad}),{view:'attendance'}],
      ['study_schedule',listPage('study_schedule')+hangulPad,{view:'study_schedule'}],['course_study_manager',listPage('course_study_manager')+asciiPad,{view:'course_study_manager'}],
    ];
    for(const [label,html,input] of cases){
      const op=input.view==='attendance'?'Main':input.view==='study_schedule'?'StudySchedule':'CourseStudyManager';
      const out:any=await wire({[op]:html}).schedule.get(input);
      expect({label,status:out.status,reasonCode:out.reasonCode,rows:out.rows}).toEqual({label,status:'unavailable',reasonCode:'source_rejected',rows:undefined});
    }
    // A declared length over the limit is refused without reading the body.
    const declared:any=await wire({Main:()=>new Response(attendancePage(),{headers:{'Content-Length':String(MiB2+1)}})}).schedule.get({view:'attendance'});
    expect(declared).toMatchObject({status:'unavailable',reasonCode:'source_rejected'});
    // A page just under the limit is read.
    const underPad='<!--'+'x'.repeat(MiB2-Buffer.byteLength(attendancePage(),'utf8')-7)+'-->';
    const underPage=attendancePage({pad:underPad});expect(Buffer.byteLength(underPage,'utf8')).toBe(MiB2);
    const under:any=await wire({Main:underPage}).schedule.get({view:'attendance'});
    expect(under.status).toBe('observed');
  });
  test('lms_reads_r8_schedule_pages_over_1000_rows_are_source_rejected',async()=>{
    const row=(i:number)=>[`2040-01-${String(1+i%28).padStart(2,'0')}`,`Invented Course ${i}`,'정규','출석','m'];
    const many=(n:number)=>Array.from({length:n},(_,i)=>row(i));
    const at1000:any=await wire({Main:attendancePage({rows:many(1000)})}).schedule.get({view:'attendance'});
    expect({status:at1000.status,rows:at1000.rows?.length}).toEqual({status:'observed',rows:1000});
    const at1001:any=await wire({Main:attendancePage({rows:many(1001)})}).schedule.get({view:'attendance'});
    expect(at1001).toMatchObject({status:'unavailable',reasonCode:'source_rejected'});expect(at1001.rows).toBeUndefined();
    // The row limit is on the parsed rows, not on the rows of the requested date.
    expect(await wire({Main:attendancePage({rows:many(1001)})}).schedule.get({view:'attendance',date:'2040-01-02'})).toMatchObject({reasonCode:'source_rejected'});
    for(const view of ['study_schedule','course_study_manager'] as const){
      const op=view==='study_schedule'?'StudySchedule':'CourseStudyManager';
      const rows=(n:number)=>Array.from({length:n},(_,i)=>[`Invented Course ${i}`,'월','c']);
      expect((await wire({[op]:listPage(view,{rows:rows(1000)})}).schedule.get({view}) as any).rows).toHaveLength(1000);
      expect(await wire({[op]:listPage(view,{rows:rows(1001)})}).schedule.get({view})).toMatchObject({status:'unavailable',reasonCode:'source_rejected'});
    }
  });
  test('lms_reads_r8_prestudy_pages_over_2_MiB_or_1000_rows_are_source_rejected',async()=>{
    const asciiPad='<!--'+'x'.repeat(MiB2)+'-->';
    const cases:[string,Pages,Record<string,unknown>][]=[
      ['the waiting form page over 2 MiB',{WebUnPreStudy:prestudyList('WebUnPreStudy',{pad:hangulPad})},{view:'incomplete'}],
      ['the waiting result over 2 MiB',{WebUnPreStudy:(body)=>prestudyList('WebUnPreStudy',{pad:body.has('student_name')?asciiPad:''})},{view:'incomplete'}],
      ['the summary page over 2 MiB',{TeacherPrestudySummary:summaryPage({pad:hangulPad})},{view:'teacher_summary'}],
      ['the waiting list over 1000 rows',{WebUnPreStudy:prestudyList('WebUnPreStudy',{extraRows:1000})},{view:'incomplete'}],
      ['the completed list over 1000 rows',{WebCtPreStudy:prestudyList('WebCtPreStudy',{extraRows:1000})},{}],
      ['the summary over 1000 rows',{TeacherPrestudySummary:summaryPage({extraRows:1000})},{view:'teacher_summary'}],
    ];
    for(const [label,pages,extra] of cases){
      const out:any=await wire(pages).prestudy.prestudyGet(ask(STUDENT,extra));
      expect({label,status:out.status,reasonCode:out.reasonCode}).toEqual({label,status:'unavailable',reasonCode:'source_rejected'});
      expect(out.waiting??out.completed??out.summary).toBeUndefined();
    }
    // 1000 rows are still read.
    const ok:any=await wire({WebUnPreStudy:prestudyList('WebUnPreStudy',{extraRows:999})}).prestudy.prestudyGet(ask(STUDENT,{view:'incomplete'}));
    expect({status:ok.status,waiting:ok.waiting?.length}).toEqual({status:'observed',waiting:1});
    // The report row's pages are bounded the same way.
    for(const op of ['WebPreStudy','prestudy_ai_report']){
      const html=op==='WebPreStudy'?detailPage+asciiPad:reportPage+hangulPad;
      expect(await wire({[op]:html}).prestudy.prestudyReportGet({...ask(),prestudyKey:'12346'})).toMatchObject({status:'unavailable',reasonCode:'source_rejected'});
    }
  });
});

// ---- R9 ---------------------------------------------------------------------------------------
describe('lms-reads R9: no video URL and no cookie is answered or logged',()=>{
  test('lms_reads_r9_answers_and_failures_hold_no_video_url_and_no_cookie',async()=>{
    const logs:string[]=[];
    const spies=[spyOn(console,'log'),spyOn(console,'error'),spyOn(console,'warn'),spyOn(console,'info'),spyOn(console,'debug')];
    for(const spy of spies)spy.mockImplementation((...args:unknown[])=>{logs.push(args.map(String).join(' '));});
    const stderr=spyOn(process.stderr,'write').mockImplementation(((chunk:unknown)=>{logs.push(String(chunk));return true;}) as never);
    const stdout=spyOn(process.stdout,'write').mockImplementation(((chunk:unknown)=>{logs.push(String(chunk));return true;}) as never);
    const answers:unknown[]=[];
    try{
      const withUrls=(html:string)=>html.replace('Invented memo two',`Invented memo two ${VIDEO_URL}`).replace('Invented Course B',`<a href="${VIDEO_URL}" onclick="playVideo('${VIDEO_URL}')">Invented Course B</a>`);
      const w=wire({Main:withUrls(attendancePage()),StudySchedule:listPage('study_schedule',{rows:[[`Invented Course ${VIDEO_URL}`,'월','//cdn.example.invalid/a.mp4']]}),
        CourseStudyManager:listPage('course_study_manager',{rows:[[`<a href="${VIDEO_URL}">Invented Course</a>`,'월','x']]})});
      for(const input of [{view:'attendance'},{view:'study_schedule'},{view:'course_study_manager'}])answers.push(await w.schedule.get(input));
      for(const extra of [{},{view:'incomplete'},{view:'teacher_summary'}])answers.push(await w.prestudy.prestudyGet(ask(STUDENT,extra)));
      answers.push(await w.prestudy.prestudyReportGet({...ask(),prestudyKey:'12346'}));
      // The cookie of the login is sent to the LMS, and only there.
      expect(w.guard.requests.length).toBeGreaterThan(5);
      // Refusals and failures carry codes only.
      for(const page of [()=>loginPage,()=>new Response('JSESSIONID='+SESSION,{status:500}),()=>new Response('',{status:302,headers:{location:VIDEO_URL}}),()=>`${VIDEO_URL} ${SESSION}`]){
        const bad=wire({Main:page,TeacherPrestudySummary:page,StudySchedule:page});
        answers.push(await bad.schedule.get({view:'attendance'}),await bad.schedule.get({view:'study_schedule'}),await bad.prestudy.prestudyGet(ask(STUDENT,{view:'teacher_summary'})));
      }
    }finally{
      for(const spy of [...spies,stderr,stdout])spy.mockRestore();
    }
    const all=json(answers)+logs.join('\n');
    for(const forbidden of ['cdn.example','.mp4','playVideo','JSESSIONID',SESSION,'Set-Cookie','prestudy/1_77001'])expect({forbidden,found:all.includes(forbidden)}).toEqual({forbidden,found:false});
    expect((answers[0] as any).rows[1].memo).toBe('Invented memo two');
    expect((answers[3] as any).summary).toEqual([{unitLabel:'9-1 Invented Unit',dateLiteral:'40.01.02 03:04',aiScoreLiteral:'77점 보고서',ratingLiteral:'-',reportAvailable:true,videoAvailable:true}]);
  });
});

// ---- the surface and the skill ----------------------------------------------------------------
describe('lms-reads: the tool surface and the lesson-day skill',()=>{
  const root=resolve(import.meta.dir,'..'),repo=resolve(root,'../../../..');
  const surface=JSON.parse(readFileSync(resolve(root,'release/surface.json'),'utf8')) as {tools:{name:string;title:string;description:string;inputSchema:any}[]};
  const bytes=(value:unknown)=>Buffer.byteLength(JSON.stringify(value),'utf8');
  test('lms_reads_surface_entry_is_within_795_bytes_and_names_the_views_and_bounds',()=>{
    const entry=surface.tools.find(t=>t.name==='academy_lms_schedule_get')!;
    expect(entry).toBeDefined();
    expect(bytes(entry)).toBeLessThanOrEqual(795);
    expect(bytes(entry.title)+bytes(entry.description)).toBeLessThanOrEqual(254);
    // The 15 baseline tools stay untouched and the 18 tools stay within 20,820 bytes (specs/academy-mcp/tool-surface.md R2).
    expect(surface.tools).toHaveLength(18);
    expect(bytes(surface.tools.slice(0,15))).toBe(18317);
    expect(bytes(surface.tools)).toBeLessThanOrEqual(20820);
    for(const text of ['attendance','study_schedule','course_study_manager','YYYY-MM-DD','31','same month','Read-only'])expect({text,found:entry.description.includes(text)}).toEqual({text,found:true});
    expect(entry.inputSchema.properties.view.enum).toEqual(['attendance','study_schedule','course_study_manager']);
    expect(entry.inputSchema.required).toEqual(['view']);
    // The displayed members are exactly the members of the schema the tool validates with.
    expect(Object.keys(lmsScheduleGetSchema.shape)).toEqual(Object.keys(entry.inputSchema.properties));
    expect(keptTools.some(t=>t.name==='academy_lms_schedule_get')).toBe(true);
    expect(surfaceOrder).toContain('academy_lms_schedule_get');
    expect(consolidatedTools.some(t=>t.name==='academy_lms_schedule_get')).toBe(false);
  });
  test('lms_reads_skill_names_the_tool_with_all_three_views',()=>{
    const skill=readFileSync(resolve(repo,'content/skills/academy-lesson-day/SKILL.md'),'utf8');
    const lines=skill.split('\n').filter(l=>l.includes('academy_lms_schedule_get'));
    expect(lines.length).toBeGreaterThan(0);
    for(const line of lines)for(const view of ['attendance','study_schedule','course_study_manager'])expect({line,view,named:line.includes(`'${view}'`)}).toEqual({line,view,named:true});
    const names=new Set(surface.tools.map(t=>t.name));
    const unknown=[...skill.matchAll(/\b(?:class2_)?academy_[a-z0-9_]+\b/g)].map(m=>m[0]).filter(n=>n!=='academy_operations'&&!names.has(n));
    expect(unknown).toEqual([]);
  });
});
