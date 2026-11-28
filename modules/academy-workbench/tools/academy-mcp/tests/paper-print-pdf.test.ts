import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {chmod,lstat,mkdir,mkdtemp,readFile,readdir,realpath,rm,symlink,writeFile} from 'node:fs/promises';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {PaperPrintPdfService,clinicTitle,paperPrintPdfGetSchema,printAssemblyInput,type PaperPrintBackend} from '../src/paper-print-pdf.ts';
import {pinnedStudyqRunner,studyqRunnerConfiguration,studyqSourceTreeSha256,type StudyqRunner} from '../src/studyq-runner.ts';
import {lmsRequestRole,writeForbiddenTransport} from './network-deny.ts';
import {AcademyHttpClient} from '../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts';
import {MAX_PRINT_ARCHIVE_BYTES,paperExamListForm,parsePaperExamList,printArchiveRelativePath,printArchiveUrl,type PaperExamRow} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead.ts';

// academy_paper_print_pdf_get (src/paper-print-pdf.ts) with an injected backend and runner, the LMS read
// client of apps/academy/dc-gang-a/src/lms/paperPrintRead.ts over a fixture transport, the runner's
// `assemble-print` over a synthetic executor, and the surface budget. Invented keys and bytes only;
// no archive key, no real archive, no network.
const sha=(bytes:Buffer|string)=>createHash('sha256').update(bytes).digest('hex');
let base:string;
beforeAll(async()=>{base=await realpath(await mkdtemp(resolve(tmpdir(),'paper-print-')));});
afterAll(async()=>{await rm(base,{recursive:true,force:true});});
const now=()=>new Date('2040-01-16T09:00:00.123Z');
const row=(over:Partial<PaperExamRow>={}):PaperExamRow=>({lectureKey:1234567,outerLectureKey:null,selType:'obj',examtextKey:0,patName:'Invented pattern',difficulty:'mid',unitName:'Invented unit',...over});
const archiveBytes=(key:number)=>Buffer.from('PK\x03\x04 invented archive '+key,'latin1');

interface Seen {input:any;path:string;scratch:{files:Record<string,{sha256:string;mode:number}>;rootMode:number}}
function fixture(name:string,rows:PaperExamRow[]=[row(),row({lectureKey:7,selType:'sbj',patName:'Second | pattern'})],over:{rendered?:Awaited<ReturnType<StudyqRunner['assemblePrint']>>;downloadFails?:boolean;runnerThrows?:boolean}={}){
  const calls={list:[] as string[],downloads:[] as string[],runner:[] as Seen[]};
  const backend:PaperPrintBackend={
    async readPaperExamList(paperNo){calls.list.push(paperNo);return rows;},
    async downloadPrintArchive(kind,key){calls.downloads.push(kind+':'+key);if(over.downloadFails)throw Error('paper_print_archive_rejected');return archiveBytes(key);},
  };
  const runner:StudyqRunner={
    async renderCourseBook(){throw Error('not_called');},async renderCism(){throw Error('not_called');},
    async assemblePrint(input:any,path){
      if(over.runnerThrows)throw Error('studyq_runner_failed');
      // The archives are on disk in the private scratch directory while the executor runs.
      const files:Seen['scratch']['files']={};
      const walk=async(dir:string)=>{for(const name of await readdir(dir)){
        const p=resolve(dir,name),st=await lstat(p);
        if(st.isDirectory()){expect(st.mode&0o777).toBe(0o700);await walk(p);}else files[p.slice(input.archive_root.length+1)]={sha256:sha(await readFile(p)),mode:st.mode&0o777};
      }};
      await walk(input.archive_root);
      calls.runner.push({input,path,scratch:{files,rootMode:(await lstat(input.archive_root)).mode&0o777}});
      return over.rendered??{status:'rendered',reportType:'print_'+input.kind,pages:3,sha256:'a'.repeat(64),outputPath:path};
    }};
  const root=resolve(base,name,'print-pdfs'),temp=resolve(base,name+'-tmp');
  const ready=mkdir(temp,{recursive:true,mode:0o700});
  return {calls,root,temp,ready,service:new PaperPrintPdfService(backend,runner,root,now,temp)};
}

describe('academy_paper_print_pdf_get input',()=>{
  test('paper_print_schema_validates_keys_before_any_lms_request',()=>{
    const ok=[{mode:'exam',paperNo:'5001'},{mode:'exam',paperNo:'999999999'},{mode:'exam',paperKeys:['1','999999999'],selType:'des'},{mode:'clinic',paperKeys:['7']},
      {mode:'clinic',paperNo:'12',includeAnswers:true,includeExplain:false},{mode:'examtext',paperKeys:['999999'],selType:'sbj',outputPath:'a.pdf'}];
    for(const value of ok)expect(paperPrintPdfGetSchema.safeParse(value).success).toBe(true);
    const bad:unknown[]=[{},{mode:'other',paperNo:'1'},{mode:'exam'},{mode:'exam',paperNo:'1',paperKeys:['1']},{mode:'clinic'},{mode:'clinic',paperNo:'1',paperKeys:['1']},
      {mode:'exam',paperNo:'0'},{mode:'exam',paperNo:'01'},{mode:'exam',paperNo:'-1'},{mode:'exam',paperNo:'1.5'},{mode:'exam',paperNo:1},{mode:'exam',paperNo:'1000000000'},
      {mode:'exam',paperNo:'12345678901234567'},{mode:'exam',paperKeys:[]},{mode:'exam',paperKeys:['0']},{mode:'exam',paperKeys:['1000000000']},{mode:'exam',paperKeys:['x']},
      {mode:'exam',paperKeys:Array.from({length:65},(_,i)=>String(i+1))},{mode:'exam',paperNo:'5',selType:'obj'},{mode:'clinic',paperNo:'5',selType:'obj'},
      {mode:'exam',paperKeys:['1'],selType:'1'},{mode:'exam',paperKeys:['1'],selType:'OBJ'},{mode:'examtext',paperNo:'5'},{mode:'examtext'},{mode:'examtext',paperKeys:['1000000']},
      {mode:'examtext',paperNo:'5',paperKeys:['1']},{mode:'exam',paperNo:'5',includeAnswers:'yes'},{mode:'exam',paperNo:'5',extra:1},{mode:'exam',paperNo:'5',outputPath:'a'},
      {mode:'exam',paperNo:'5',outputPath:'x'.repeat(513)}];
    for(const value of bad)expect({value,ok:paperPrintPdfGetSchema.safeParse(value).success}).toEqual({value,ok:false});
    expect(paperPrintPdfGetSchema.safeParse({mode:'exam',paperKeys:Array.from({length:64},(_,i)=>String(i+1))}).success).toBe(true);
  });

  test('paper_print_the_service_refuses_a_bad_input_before_any_request',async()=>{
    const f=fixture('bad-input');await f.ready;
    for(const value of [{mode:'exam'},{mode:'exam',paperNo:'1000000000'},{mode:'examtext',paperKeys:['1000000']},{mode:'clinic',paperNo:'5',selType:'obj'}])
      await expect(f.service.get(value)).rejects.toThrow();
    expect(f.calls.list).toEqual([]);expect(f.calls.downloads).toEqual([]);expect(f.calls.runner).toEqual([]);
  });
});

describe('academy_paper_print_pdf_get answer',()=>{
  test('paper_print_exam_by_paper_no_downloads_each_archive_once_into_a_private_scratch_directory_and_removes_it',async()=>{
    const f=fixture('exam-paper',[row(),row({lectureKey:7,selType:'sbj'}),row({lectureKey:1234567,selType:'des'}),row({lectureKey:9,selType:'obj',examtextKey:55})]);await f.ready;
    const answer=await f.service.get({mode:'exam',paperNo:'5001',includeAnswers:true}) as any;
    const path=resolve(f.root,'print-exam-5001-20400116T090000Z.pdf');
    expect(answer).toEqual({status:'rendered',mode:'exam',reportType:'print_exam',outputPath:path,sha256:'a'.repeat(64),pages:3,
      sources:[{kind:'exam',key:1234567,selType:'obj'},{kind:'exam',key:7,selType:'sbj'},{kind:'exam',key:1234567,selType:'des'},{kind:'exam',key:9,selType:'obj'}],
      academyWrites:0,parentSends:0,execution:'read_only_lms_local_pdf'});
    // The exam list is read once; a passage (examtext_key above 0) is not fetched by mode exam; each archive once.
    expect(f.calls.list).toEqual(['5001']);
    expect(f.calls.downloads).toEqual(['exam:1234567','exam:7','exam:9']);
    const seen=f.calls.runner[0]!;
    expect(seen.path).toBe(path);
    expect(seen.input).toEqual({archive_root:seen.input.archive_root,kind:'exam',items:[
      {lecture_key:1234567,sel_type:'obj',include_answers:true},{lecture_key:7,sel_type:'sbj',include_answers:true},
      {lecture_key:1234567,sel_type:'des',include_answers:true},{lecture_key:9,sel_type:'obj',include_answers:true}]});
    expect(seen.input.archive_root.startsWith(f.temp+'/paper-print-')).toBe(true);
    expect(seen.scratch.rootMode).toBe(0o700);
    expect(seen.scratch.files).toEqual({
      'data/contents/exam/d6-001/d4-23/d2-45/ek-1234567/1234567-print.zip':{sha256:sha(archiveBytes(1234567)),mode:0o600},
      'data/contents/exam/d6-000/d4-00/d2-00/ek-7/7-print.zip':{sha256:sha(archiveBytes(7)),mode:0o600},
      'data/contents/exam/d6-000/d4-00/d2-00/ek-9/9-print.zip':{sha256:sha(archiveBytes(9)),mode:0o600}});
    expect(await readdir(f.temp)).toEqual([]);
    expect((await lstat(f.root)).mode&0o777).toBe(0o700);
    expect(path).not.toContain('Invented');
  });

  test('paper_print_exam_by_paper_keys_defaults_sel_type_obj_and_reads_no_exam_list',async()=>{
    const f=fixture('exam-keys');await f.ready;
    const answer=await f.service.get({mode:'exam',paperKeys:['42','5'],includeExplain:true,outputPath:'mine.pdf'}) as any;
    expect(answer).toMatchObject({status:'rendered',outputPath:resolve(f.root,'mine.pdf'),sources:[{kind:'exam',key:42,selType:'obj'},{kind:'exam',key:5,selType:'obj'}]});
    expect(f.calls.list).toEqual([]);
    expect(f.calls.runner[0]!.input.items).toEqual([{lecture_key:42,sel_type:'obj',include_explain:true},{lecture_key:5,sel_type:'obj',include_explain:true}]);
    await f.service.get({mode:'exam',paperKeys:['42'],selType:'sbj'});
    expect(f.calls.runner[1]!.input.items).toEqual([{lecture_key:42,sel_type:'sbj'}]);
    expect(f.calls.runner[1]!.path).toBe(resolve(f.root,'print-exam-42-20400116T090000Z.pdf'));
  });

  test('paper_print_examtext_fetches_the_passage_archives_of_explicit_keys',async()=>{
    const f=fixture('examtext');await f.ready;
    const answer=await f.service.get({mode:'examtext',paperKeys:['123456','7'],selType:'des'}) as any;
    expect(answer).toMatchObject({status:'rendered',reportType:'print_examtext',sources:[{kind:'examtext',key:123456,selType:'des'},{kind:'examtext',key:7,selType:'des'}]});
    expect(f.calls.list).toEqual([]);expect(f.calls.downloads).toEqual(['examtext:123456','examtext:7']);
    const seen=f.calls.runner[0]!;
    expect(seen.input).toEqual({archive_root:seen.input.archive_root,kind:'examtext',items:[{examtext_key:123456,sel_type:'des'},{examtext_key:7,sel_type:'des'}]});
    expect(Object.keys(seen.scratch.files).sort()).toEqual(['data/contents/examtext/d4-00/d2-00/etk-7/7-print.zip','data/contents/examtext/d4-12/d2-34/etk-123456/123456-print.zip']);
  });

  test('paper_print_clinic_builds_the_clinic_input_with_sanitised_titles_and_per_item_sel_types',async()=>{
    const f=fixture('clinic',[row({lectureKey:10,selType:'obj',patName:'Alpha|,|Beta'}),row({lectureKey:20,selType:'sbj',patName:'  '}),row({lectureKey:30,selType:'des',patName:'Gamma'})]);await f.ready;
    const answer=await f.service.get({mode:'clinic',paperNo:'77',includeAnswers:true,includeExplain:true}) as any;
    expect(answer).toMatchObject({status:'rendered',mode:'clinic',reportType:'print_clinic',outputPath:resolve(f.root,'print-clinic-77-20400116T090000Z.pdf'),
      sources:[{kind:'exam',key:10,selType:'obj'},{kind:'exam',key:20,selType:'sbj'},{kind:'exam',key:30,selType:'des'}]});
    const input=f.calls.runner[0]!.input;
    expect(input).toEqual({archive_root:input.archive_root,kind:'clinic',clinic_list:'1|,|2|,|3',exam_key:'10|,|20|,|30',content_list:'Alpha Beta|,|item|,|Gamma',
      clinic_type:'clinic',lecture_value:'10|,|20|,|30',sel_types:'obj|,|sbj|,|des',include_answers:true,include_explain:true});
    // Paper keys: the titles are `item` and the sel_type is the one given.
    await f.service.get({mode:'clinic',paperKeys:['4','5'],selType:'sbj'});
    expect(f.calls.runner[1]!.input).toMatchObject({kind:'clinic',clinic_list:'1|,|2',exam_key:'4|,|5',content_list:'item|,|item',lecture_value:'4|,|5',sel_types:'sbj|,|sbj'});
    expect(f.calls.runner[1]!.input).not.toHaveProperty('include_answers');
  });

  test('paper_print_clinic_title_and_assembly_input_helpers',()=>{
    expect(clinicTitle('a|,|b|,|c')).toBe('a b c');expect(clinicTitle('|,|')).toBe('item');expect(clinicTitle('')).toBe('item');expect(clinicTitle(' x ')).toBe('x');
    expect(clinicTitle('a|,||,|b')).not.toContain('|,|');
    const items=[{kind:'exam' as const,key:1,selType:'obj' as const,patName:'a'}];
    expect(printAssemblyInput('exam','/s',items,{includeAnswers:false,includeExplain:false})).toEqual({archive_root:'/s',kind:'exam',items:[{lecture_key:1,sel_type:'obj'}]});
  });

  test('paper_print_a_row_that_is_not_obj_sbj_or_des_refuses_before_any_download',async()=>{
    const f=fixture('sel-type');await f.ready;
    const refusing=new PaperPrintPdfService({
      async readPaperExamList(){return parsePaperExamList(JSON.stringify({result:'OK',exam_list:[{lecture_key:1,sel_type:'obj'},{lecture_key:2,sel_type:'1'}]}));},
      async downloadPrintArchive(){throw Error('must_not_download');}},{async assemblePrint(){throw Error('must_not_run');},async renderCourseBook(){throw Error('x');},async renderCism(){throw Error('x');}},f.root,now,f.temp);
    expect(await refusing.get({mode:'exam',paperNo:'5'})).toMatchObject({status:'unavailable',reasonCode:'paper_exam_sel_type_unqualified',academyWrites:0});
    expect(await readdir(f.temp)).toEqual([]);
  });

  test('paper_print_output_path_is_confined_and_checked_before_any_request',async()=>{
    const f=fixture('paths');await f.ready;
    await mkdir(f.root,{recursive:true,mode:0o700});
    await writeFile(resolve(f.root,'taken.pdf'),'x');
    await symlink(base,resolve(f.root,'escape'));
    for(const [outputPath,code] of [['report.txt','output_path_invalid'],['../report.pdf','output_path_invalid'],[resolve(base,'outside.pdf'),'output_path_invalid'],
      ['escape/report.pdf','output_path_invalid'],['missing/report.pdf','output_path_invalid'],['taken.pdf','output_exists']] as const)
      expect({outputPath,answer:await f.service.get({mode:'exam',paperNo:'5',outputPath})}).toMatchObject({outputPath,answer:{status:'unavailable',reasonCode:code,academyWrites:0}});
    expect(f.calls.list).toEqual([]);expect(f.calls.downloads).toEqual([]);expect(f.calls.runner).toEqual([]);
    // A default name that exists already is not overwritten either.
    await writeFile(resolve(f.root,'print-exam-5-20400116T090000Z.pdf'),'x');
    expect(await f.service.get({mode:'exam',paperNo:'5'})).toMatchObject({status:'unavailable',reasonCode:'output_exists'});
    expect(f.calls.list).toEqual([]);
    // An output root that is a symbolic link is refused.
    await symlink(await mkdtemp(resolve(base,'paths-target-')),resolve(base,'paths-link'));
    const linked=new PaperPrintPdfService(f.service.backend,f.service.runner,resolve(base,'paths-link'),now,f.temp);
    expect(await linked.get({mode:'exam',paperNo:'5'})).toMatchObject({status:'unavailable',reasonCode:'output_root_unqualified'});
  });

  test('paper_print_refusal_failure_and_missing_ports_leave_no_scratch_directory',async()=>{
    const refused=fixture('refused',undefined,{rendered:{status:'refused',code:'archive_key_missing',detail:'key file not configured'}});await refused.ready;
    expect(await refused.service.get({mode:'exam',paperNo:'5'})).toMatchObject({status:'refused',reasonCode:'studyq_archive_key_missing',studyqDetail:'key file not configured',academyWrites:0,parentSends:0});
    expect(await readdir(refused.temp)).toEqual([]);
    const runnerFails=fixture('runner-fails',undefined,{runnerThrows:true});await runnerFails.ready;
    expect(await runnerFails.service.get({mode:'exam',paperNo:'5'})).toMatchObject({status:'unavailable',reasonCode:'studyq_runner_failed'});
    expect(await readdir(runnerFails.temp)).toEqual([]);
    const downloadFails=fixture('download-fails',undefined,{downloadFails:true});await downloadFails.ready;
    expect(await downloadFails.service.get({mode:'exam',paperNo:'5'})).toMatchObject({status:'unavailable',reasonCode:'paper_print_archive_rejected'});
    expect(await readdir(downloadFails.temp)).toEqual([]);expect(downloadFails.calls.runner).toEqual([]);
    const none=fixture('no-ports');await none.ready;
    expect(await new PaperPrintPdfService(null,none.service.runner,none.root,now).get({mode:'exam',paperNo:'5'})).toMatchObject({status:'unavailable',reasonCode:'paper_print_backend_not_configured'});
    expect(await new PaperPrintPdfService(none.service.backend,null,none.root,now).get({mode:'exam',paperNo:'5'})).toMatchObject({status:'unavailable',reasonCode:'studyq_runner_not_configured'});
  });
});

describe('the LMS read client',()=>{
  const principal={teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001'};
  const list=(rows:unknown[],result:unknown='OK')=>JSON.stringify({result,exam_list:rows});

  test('paper_print_exam_list_rows_are_parsed_and_every_sel_type_is_qualified',()=>{
    expect(paperExamListForm('5001')).toEqual({p_process:'getTestExamList',p_no:'5001',p_option_type:'2'});
    for(const bad of ['0','','x','01','1000000000',5 as never])expect(()=>paperExamListForm(bad)).toThrow('paper_no_invalid');
    // `outerLectureKey` is the row's `outer_lecture_key` when the member is present, else null (the exam tools of specs/academy-mcp/gated-writes.md read it; the print path ignores it).
    expect(parsePaperExamList(list([{lecture_key:1234567,outer_lecture_key:100000001,sel_type:'obj',examtext_key:0,pat_name:'Pattern',difficulty:'3',unit_name:'Unit'},
      {lecture_key:'8',outer_lecture_key:'100000002',sel_type:'sbj',examtext_key:'12',pat_name:null},{lecture_key:9,sel_type:'des'}]))).toEqual([
      {lectureKey:1234567,outerLectureKey:100000001,selType:'obj',examtextKey:0,patName:'Pattern',difficulty:'3',unitName:'Unit'},
      {lectureKey:8,outerLectureKey:100000002,selType:'sbj',examtextKey:12,patName:'',difficulty:'',unitName:''},
      {lectureKey:9,outerLectureKey:null,selType:'des',examtextKey:0,patName:'',difficulty:'',unitName:''}]);
    for(const outer of [1000000000,-1,1.5,'x','1e3'])expect(()=>parsePaperExamList(list([{lecture_key:1,outer_lecture_key:outer,sel_type:'obj'}]))).toThrow('paper_exam_list_unverified');
    for(const sel of ['1','OBJ','',' obj',null,1,undefined])
      expect(()=>parsePaperExamList(list([{lecture_key:1,sel_type:'obj'},{lecture_key:2,sel_type:sel}]))).toThrow('paper_exam_sel_type_unqualified');
    for(const [text,code] of [['not json','paper_exam_list_unverified'],[list([{lecture_key:1,sel_type:'obj'}],'FAIL'),'paper_exam_list_unverified'],
      [list([{lecture_key:1,sel_type:'obj'}],'ok'),'paper_exam_list_unverified'],['{"result":"OK"}','paper_exam_list_unverified'],[list([]),'paper_exam_list_empty'],
      [list(Array.from({length:201},()=>({lecture_key:1,sel_type:'obj'}))),'paper_exam_list_too_large'],[list(['x']),'paper_exam_list_unverified'],
      [list([{lecture_key:-1,sel_type:'obj'}]),'paper_exam_list_unverified'],[list([{lecture_key:1.5,sel_type:'obj'}]),'paper_exam_list_unverified'],
      [list([{lecture_key:1000000000,sel_type:'obj'}]),'paper_exam_list_unverified'],[list([{lecture_key:'1e3',sel_type:'obj'}]),'paper_exam_list_unverified'],
      [list([{lecture_key:1,sel_type:'obj',examtext_key:1000000}]),'paper_exam_list_unverified']] as const)
      expect(()=>parsePaperExamList(text)).toThrow(code);
  });

  test('paper_print_exam_list_without_a_session_fails_closed_before_any_request',async()=>{
    const guard=writeForbiddenTransport(()=>new Response('unexpected'));
    const client=new AcademyHttpClient(principal,guard.transport as never);
    await expect(client.readPaperExamList('5001')).rejects.toThrow('authentication_required');
    await expect(client.readPaperExamList('0')).rejects.toThrow('paper_no_invalid');
    expect(guard.requests).toEqual([]);
  });

  test('paper_print_exam_list_wire_posts_the_form_reads_the_rows_and_fails_closed_on_a_login_page',async()=>{
    const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
    const principal='<a href="/logout">Fictional Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="90001"></form><form name="form1"><input name="g_pri_no" value="90001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
    const exam=list([{lecture_key:1234567,sel_type:'obj',examtext_key:0,pat_name:'Invented pattern',difficulty:'3',unit_name:'Invented unit'}]);
    for(const [answer,expected] of [[exam,'rows'],[login,'authentication_required'],['<html>세션이 만료</html>','authentication_required'],[list([{lecture_key:1,sel_type:'1'}]),'paper_exam_sel_type_unqualified']] as const){
      let calls=0;const sent:{url:string;method?:string;body:string;headers:Headers}[]=[];
      const guard=writeForbiddenTransport((url,init)=>{calls++;
        if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
        sent.push({url,method:init?.method,body:String(init?.body),headers:new Headers(init?.headers)});return new Response(answer);});
      const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
      await client.authenticate({username:'fictional',password:'fictional'});
      const result=await client.readPaperExamList('5001').then(rows=>({rows}),error=>({error:(error as Error).message}));
      if(expected==='rows')expect(result).toEqual({rows:[{lectureKey:1234567,outerLectureKey:null,selType:'obj',examtextKey:0,patName:'Invented pattern',difficulty:'3',unitName:'Invented unit'}]});
      else expect(result).toEqual({error:expected});
      expect(sent).toHaveLength(1);
      expect(sent[0]!.url).toBe('https://dc.gang-a.kr/servlet/controller.common.TestpageSelectExServlet');
      expect(sent[0]!.method).toBe('POST');
      expect(Object.fromEntries(new URLSearchParams(sent[0]!.body))).toEqual({p_process:'getTestExamList',p_no:'5001',p_option_type:'2'});
      // One read of the exam list, no write.
      expect(guard.requests.filter(r=>r.route.endsWith(' getTestExamList'))).toEqual([{route:'/servlet/controller.common.TestpageSelectExServlet getTestExamList',role:'read'}]);
      expect(guard.forbidden()).toEqual([]);
    }
  });

  test('paper_print_archive_urls_are_made_from_an_integer_key_only',()=>{
    expect(printArchiveRelativePath('exam',0)).toBe('data/contents/exam/d6-000/d4-00/d2-00/ek-0/0-print.zip');
    expect(printArchiveRelativePath('exam',123456789)).toBe('data/contents/exam/d6-123/d4-45/d2-67/ek-123456789/123456789-print.zip');
    expect(printArchiveRelativePath('exam',999999999)).toBe('data/contents/exam/d6-999/d4-99/d2-99/ek-999999999/999999999-print.zip');
    expect(printArchiveRelativePath('examtext',123456)).toBe('data/contents/examtext/d4-12/d2-34/etk-123456/123456-print.zip');
    expect(printArchiveRelativePath('examtext',999999)).toBe('data/contents/examtext/d4-99/d2-99/etk-999999/999999-print.zip');
    expect(printArchiveUrl('exam',1234567).href).toBe('https://storage.studyq.net/data/contents/exam/d6-001/d4-23/d2-45/ek-1234567/1234567-print.zip');
    for(const [kind,key] of [['exam',-1],['exam',1000000000],['examtext',1000000],['exam',1.5],['exam',Number.NaN],['examtext',-1],['exam','7' as never]] as const)
      expect(()=>printArchiveUrl(kind,key)).toThrow('print_archive_key_invalid');
  });

  test('paper_print_storage_fetch_is_one_credential_free_read_with_no_redirect',async()=>{
    const seen:{url:string;init?:RequestInit}[]=[];
    const guard=writeForbiddenTransport((url,init)=>{seen.push({url,init});return new Response(archiveBytes(1234567),{status:200,headers:{'Content-Type':'application/zip'}});});
    const client=new AcademyHttpClient(principal,guard.transport as never);
    const bytes=await client.downloadPrintArchive('exam',1234567);
    expect(Buffer.from(bytes)).toEqual(archiveBytes(1234567));
    expect(seen).toHaveLength(1);
    expect(seen[0]!.url).toBe('https://storage.studyq.net/data/contents/exam/d6-001/d4-23/d2-45/ek-1234567/1234567-print.zip');
    const init=seen[0]!.init!;
    expect(init).toMatchObject({method:'GET',credentials:'omit',redirect:'manual'});
    expect(init.body).toBeUndefined();
    // No Academy session cookie, credential or origin header is sent to the storage.
    const headers=new Headers(init.headers);
    for(const name of ['cookie','authorization','origin','referer'])expect(headers.has(name)).toBe(false);
    // I1: the request is classed read and nothing was refused as a write.
    expect(guard.requests).toEqual([{route:'/data/contents/exam/d6-001/d4-23/d2-45/ek-1234567/1234567-print.zip',role:'read'}]);
    expect(guard.forbidden()).toEqual([]);
    await client.downloadPrintArchive('examtext',99);
    expect(seen[1]!.url).toBe('https://storage.studyq.net/data/contents/examtext/d4-00/d2-00/etk-99/99-print.zip');
    expect(guard.forbidden()).toEqual([]);
  });

  test('paper_print_download_is_bounded_and_refuses_anything_but_a_plain_200_archive',async()=>{
    const answer=async(make:()=>Response|Promise<Response>)=>{
      const client=new AcademyHttpClient(principal,(async()=>make()) as never);
      return client.downloadPrintArchive('exam',5).then(b=>({ok:true as const,length:b.byteLength}),e=>({ok:false as const,message:(e as Error).message}));
    };
    const zip=archiveBytes(5);
    expect(await answer(()=>new Response(zip,{status:200}))).toEqual({ok:true,length:zip.length});
    const rejected={ok:false,message:'paper_print_archive_rejected'};
    for(const status of [301,302,307,403,404,500,204,206])expect({status,r:await answer(()=>new Response(status===204?null:zip,{status}))}).toEqual({status,r:rejected});
    expect(await answer(()=>new Response('<html>error</html>',{status:200,headers:{'Content-Type':'text/html; charset=utf-8'}}))).toEqual(rejected);
    expect(await answer(()=>new Response('{"a":1}',{status:200,headers:{'Content-Type':'application/json'}}))).toEqual(rejected);
    expect(await answer(()=>new Response(Buffer.from('NOTZIP bytes'),{status:200}))).toEqual(rejected);
    expect(await answer(()=>new Response(new Uint8Array(0),{status:200}))).toEqual(rejected);
    expect(await answer(()=>new Response(zip,{status:200,headers:{'Content-Length':String(MAX_PRINT_ARCHIVE_BYTES+1)}}))).toEqual(rejected);
    expect(await answer(()=>new Response(zip,{status:200,headers:{'Content-Length':'-5'}}))).toEqual(rejected);
    const redirected=new Response(zip,{status:200});Object.defineProperty(redirected,'redirected',{value:true});
    expect(await answer(()=>redirected)).toEqual(rejected);
    const elsewhere=new Response(zip,{status:200});Object.defineProperty(elsewhere,'url',{value:'https://storage.studyq.net/other.zip'});
    expect(await answer(()=>elsewhere)).toEqual(rejected);
    // Without a declared length the bytes actually read count: 16 MiB pass, one byte more refuses.
    const stream=(total:number)=>()=>{
      let sent=0;const chunk=new Uint8Array(1024*1024);chunk.set([0x50,0x4b,0x03,0x04]);
      return new Response(new ReadableStream<Uint8Array>({pull(controller){
        if(sent>=total){controller.close();return;}
        const size=Math.min(chunk.length,total-sent);controller.enqueue(sent===0?chunk.slice(0,size):new Uint8Array(size));sent+=size;}}),{status:200});
    };
    expect(await answer(stream(MAX_PRINT_ARCHIVE_BYTES))).toEqual({ok:true,length:MAX_PRINT_ARCHIVE_BYTES});
    expect(await answer(stream(MAX_PRINT_ARCHIVE_BYTES+1))).toEqual(rejected);
    // A transport failure is its own code and holds no URL.
    const failing=new AcademyHttpClient(principal,(async()=>{throw Error('connect https://storage.studyq.net refused');}) as never);
    await expect(failing.downloadPrintArchive('exam',5)).rejects.toThrow(/^paper_print_transport_failed$/);
  });
});

describe('request classes (I1)',()=>{
  const storage='https://storage.studyq.net/data/contents/';
  const examPath='exam/d6-001/d4-23/d2-45/ek-1234567/1234567-print.zip',textPath='examtext/d4-12/d2-34/etk-123456/123456-print.zip';
  test('paper_print_storage_archives_are_read_only_by_get_on_the_one_host',()=>{
    for(const path of [examPath,textPath]){
      expect(lmsRequestRole(storage+path)).toBe('read');
      expect(lmsRequestRole(storage+path,undefined,'GET')).toBe('read');
      expect(lmsRequestRole(storage+path,undefined,'get')).toBe('read');
      for(const method of ['POST','PUT','DELETE','PATCH','HEAD'])expect({method,role:lmsRequestRole(storage+path,undefined,method)}).toEqual({method,role:'write'});
      // Not on another host, scheme or port, and not with a query, credentials or a fragment.
      for(const host of ['https://dc.gang-a.kr','https://storage.studyq.net.invalid','https://evil.studyq.net','http://storage.studyq.net','https://storage.studyq.net:8443','https://user@storage.studyq.net'])
        expect({host,role:lmsRequestRole(host+'/data/contents/'+path)}).toEqual({host,role:'write'});
      expect(lmsRequestRole(storage+path+'?x=1')).toBe('write');
      expect(lmsRequestRole(storage+path+'#x')).toBe('write');
    }
    // Only the exact path forms.
    for(const path of ['exam/d6-1/d4-23/d2-45/ek-1/1-print.zip','exam/d6-001/d4-23/d2-45/ek-1/1-print.zip.bak','exam/d6-001/d4-23/d2-45/ek-x/1-print.zip','exam/d6-001/d4-23/d2-45/ek-1/1-print.pdf',
      'exam/d6-001/d4-23/d2-45/ek-1/x/1-print.zip','examtext/d4-12/d2-34/ek-1/1-print.zip','exam/d4-12/d2-34/etk-1/1-print.zip','other/d4-12/d2-34/etk-1/1-print.zip','../exam/d6-001/d4-23/d2-45/ek-1/1-print.zip',
      'exam/d6-001/d4-23/d2-45/ek-1/%2e%2e/1-print.zip'])
      expect({path,role:lmsRequestRole(storage+path)}).toEqual({path,role:'write'});
    expect(lmsRequestRole('https://storage.studyq.net/data/contents/')).toBe('write');
    expect(lmsRequestRole('https://storage.studyq.net/')).toBe('write');
  });
  test('paper_print_get_test_exam_list_is_a_read_and_the_neighbours_are_not',()=>{
    const path='/servlet/controller.common.TestpageSelectExServlet';
    expect(lmsRequestRole(path,'p_process=getTestExamList&p_no=5001&p_option_type=2')).toBe('read');
    expect(lmsRequestRole(path,'p_process=getTestExamList&p_no=5001&p_option_type=2','POST')).toBe('read');
    expect(lmsRequestRole(path,'p_process=SaveTestExam&p_no=5001')).toBe('write');
    expect(lmsRequestRole(path+'?p_process=getTestExamList')).toBe('read');
    expect(lmsRequestRole('/servlet/controller.common.OtherServlet','p_process=getTestExamList')).toBe('write');
    const guard=writeForbiddenTransport(()=>new Response('{}'));
    return guard.transport('https://dc.gang-a.kr'+path,{method:'POST',body:'p_process=getTestExamList&p_no=5001&p_option_type=2'}).then(r=>{
      expect(r.status).toBe(200);expect(guard.requests).toEqual([{route:path+' getTestExamList',role:'read'}]);
    });
  });
  test('paper_print_the_fixture_transport_forwards_the_method_and_refuses_a_storage_write',async()=>{
    const guard=writeForbiddenTransport(()=>new Response('ok'));
    expect((await guard.transport(storage+examPath,{method:'GET'})).status).toBe(200);
    expect((await guard.transport(storage+examPath)).status).toBe(200);
    expect((await guard.transport(storage+examPath,{method:'PUT',body:'x'})).status).toBe(403);
    expect((await guard.transport(storage+examPath,{method:'DELETE'})).status).toBe(403);
    expect(guard.forbidden()).toHaveLength(2);
  });
});

describe('the runner of assemble-print',()=>{
  const cli=`import hashlib, json, os, sys
def main(argv):
    data = json.loads(sys.stdin.buffer.read())
    out = argv[argv.index('--out') + 1]
    with open(out + '.env.json', 'w') as handle:
        json.dump({'env': sorted(os.environ), 'isolated': sys.flags.isolated, 'argv': argv, 'input': data}, handle)
    if data.get('kind') == 'refuse':
        print(json.dumps({'status': 'error', 'code': 'archive_refused', 'detail': 'png and pdf members'}))
        return 1
    if data.get('kind') == 'crash':
        print('not json')
        return 1
    if data.get('kind') == 'lie':
        with open(out, 'wb') as handle:
            handle.write(b'%PDF-1.4 other bytes')
        print(json.dumps({'status': 'ok', 'reportType': 'print_exam', 'pages': 1, 'sha256': '0' * 64, 'outputPath': out}))
        return 0
    pdf = b'%PDF-1.4 synthetic ' + hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest().encode()
    with open(out, 'wb') as handle:
        handle.write(pdf)
    print(json.dumps({'status': 'ok', 'reportType': 'print_' + data['kind'], 'pages': 2, 'sha256': hashlib.sha256(pdf).hexdigest(), 'outputPath': out}))
    return 0
`;
  let src:string,sourceSha256:string,interpreter:string,interpreterSha256:string;
  beforeAll(async()=>{
    src=resolve(base,'runner-src');const pkg=resolve(src,'studyq_linux');
    await mkdir(pkg,{recursive:true});await chmod(src,0o755);await chmod(pkg,0o755);
    for(const [file,text] of Object.entries({'__init__.py':'','cli.py':cli,'parity.py':'def main(argv):\n    return 0\n'})){await writeFile(resolve(pkg,file),text);await chmod(resolve(pkg,file),0o644);}
    sourceSha256=await studyqSourceTreeSha256(src);
    interpreter=await realpath('/usr/bin/python3');interpreterSha256=sha(await readFile(interpreter));
  });
  const pin=(extra:Record<string,unknown>={})=>pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:resolve(base,'assets'),...extra});

  test('paper_print_runner_runs_assemble_print_with_only_the_key_file_variable_and_checks_the_answer',async()=>{
    const runner=pin({printKeyFile:'/invented/print.key'});
    const out=resolve(base,'runner-ok.pdf');
    const answer=await runner.assemblePrint({kind:'exam',items:[]},out);
    expect(answer).toEqual({status:'rendered',reportType:'print_exam',pages:2,sha256:sha(await readFile(out)),outputPath:out});
    const seen=JSON.parse(await readFile(out+'.env.json','utf8'));
    expect(seen.argv).toEqual(['assemble-print','--out',out]);
    expect(seen.isolated).toBe(1);
    // The key file name is the launch's own: the environment holds it, no argument and no input does.
    expect(seen.env).toContain('STUDYQ_PRINT_KEY_FILE');
    expect(JSON.stringify(seen.argv)+JSON.stringify(seen.input)).not.toContain('/invented/print.key');
    const without=resolve(base,'runner-nokey.pdf');
    await pin().assemblePrint({kind:'exam',items:[]},without);
    expect(JSON.parse(await readFile(without+'.env.json','utf8')).env).not.toContain('STUDYQ_PRINT_KEY_FILE');
    expect(await runner.assemblePrint({kind:'refuse'},resolve(base,'runner-refused.pdf'))).toEqual({status:'refused',code:'archive_refused',detail:'png and pdf members'});
    await expect(runner.assemblePrint({kind:'crash'},resolve(base,'runner-crash.pdf'))).rejects.toThrow('studyq_runner_failed');
    await expect(runner.assemblePrint({kind:'lie'},resolve(base,'runner-lie.pdf'))).rejects.toThrow('studyq_runner_failed');
    await expect(runner.assemblePrint({kind:'exam'},'relative.pdf')).rejects.toThrow('output_path_invalid');
  });

  test('paper_print_runner_configuration_takes_the_key_file_from_the_environment_only',async()=>{
    const configured=await studyqRunnerConfiguration(['--studyq-runner',interpreter,interpreterSha256,src,sourceSha256,resolve(base,'assets')],{STUDYQ_PRINT_KEY_FILE:'/invented/print.key'});
    const out=resolve(base,'configured.pdf');
    await configured.runner!.assemblePrint({kind:'exam'},out);
    expect(JSON.parse(await readFile(out+'.env.json','utf8')).env).toContain('STUDYQ_PRINT_KEY_FILE');
    // A second flag for the key file does not exist.
    await expect(studyqRunnerConfiguration(['--studyq-runner',interpreter,interpreterSha256,src,sourceSha256,resolve(base,'assets'),'--studyq-print-key-file','/x'],{})).resolves.toMatchObject({remainingArgs:['--studyq-print-key-file','/x']});
  });
});

describe('the surface of academy_paper_print_pdf_get',()=>{
  const root=resolve(import.meta.dir,'..');
  const surface=JSON.parse(readFileSync(resolve(root,'release/surface.json'),'utf8')) as {tools:{name:string;description:string;inputSchema:any}[]};
  const bytes=(value:unknown)=>Buffer.byteLength(JSON.stringify(value),'utf8');
  test('paper_print_surface_entry_is_within_795_bytes_and_the_running_cap_holds',()=>{
    const entry=surface.tools.find(t=>t.name==='academy_paper_print_pdf_get')!;
    expect(entry).toBeDefined();
    expect(bytes(entry)).toBeLessThanOrEqual(795);
    expect(surface.tools).toHaveLength(22);
    // The 15 baseline tools are untouched at 18,317 bytes; the 22 tools stay within the cap of 24 tools and 24,000 bytes (no planned tool remains).
    expect(bytes(surface.tools.slice(0,15))).toBe(18317);
    // The diag, new_diag and formative report types of academy_official_report_pdf_get (+47 bytes), then ganga,
    // na_score and smart_book (+56 bytes, with a shorter description), then academy_lms_schedule_get (<= 795 bytes),
    // stay within 20,820 bytes (specs/academy-mcp/lms-reads.md); the two gated clinic tools (<= 793 bytes each) bring the 20 tools to at most 22,406 bytes,
    // and the two gated exam tools (<= 1,997 bytes together) bring the 22 tools to at most 24,000 bytes.
    expect(bytes(surface.tools)).toBeLessThanOrEqual(24000);
    expect(surface.tools.length).toBeLessThanOrEqual(24);
    expect(Object.keys(entry.inputSchema.properties)).toEqual(['mode','paperNo','paperKeys','selType','includeAnswers','includeExplain','outputPath']);
    expect(entry.inputSchema.required).toEqual(['mode']);
    expect(entry.inputSchema.properties.mode.enum).toEqual(['exam','examtext','clinic']);
    expect(entry.inputSchema.properties.selType.enum).toEqual(['obj','sbj','des']);
    // The displayed members are exactly the members of the schema the tool validates with.
    expect(Object.keys(paperPrintPdfGetSchema.shape)).toEqual(Object.keys(entry.inputSchema.properties));
  });
  test('paper_print_skill_names_the_tool_and_every_tool_name_of_the_skills_is_on_the_surface',()=>{
    const repo=resolve(root,'../../../..');
    const skill=readFileSync(resolve(repo,'content/skills/academy-lesson-day/SKILL.md'),'utf8');
    expect(skill).toContain('academy_paper_print_pdf_get');
    const names=new Set(surface.tools.map(t=>t.name));
    const unknown=[...skill.matchAll(/\b(?:class2_)?academy_[a-z0-9_]+\b/g)].map(m=>m[0]).filter(n=>n!=='academy_operations'&&!names.has(n));
    expect(unknown).toEqual([]);
  });
});

