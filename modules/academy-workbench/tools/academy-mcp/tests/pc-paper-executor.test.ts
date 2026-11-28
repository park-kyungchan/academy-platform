import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {chmod,mkdir,mkdtemp,readFile,readdir,realpath,rm,writeFile} from 'node:fs/promises';
import {existsSync,readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {PcExecutorError,decodeCp949,processStartTime,type PcExecutorOptions} from '../src/pc-executor.ts';
import {PRINT_ITEM_MEMBERS,pcPaperExecutor,pcPaperScripts,pdfInfoText,printItemForPc,studyqInstallRoot,type PcPaperExecutor,type PcPrintInput} from '../src/pc-paper-executor.ts';
import {PaperPrintPdfService,paperPrintPdfGetSchema,withoutRetiredStudyqRunnerArguments,type PaperPrintBackend} from '../src/paper-print-pdf.ts';
import {ClinicPaperService} from '../src/clinic-write-tools.ts';
import {keptTools} from '../src/consolidated-tools.ts';
import {writeForbiddenTransport} from './network-deny.ts';
import {STUDENT,TESTING,PAPER,fixtureLms,fixturePort,pageHashes} from './gated-write-fixture-lms.ts';
import {paperPrintItem,paperPrintSourceForm,resultPrintListForm} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead.ts';

// specs/academy-mcp/pc-paper-executor.md: src/pc-paper-executor.ts against fake ssh and scp programs that keep an invented "PC"
// StudyQ folder on this host, the service of academy_paper_print_pdf_get over fake backends, the printPdf of the clinic save over
// the recorded synthetic LMS, and the privacy check I6 of ./check. No network, no PC, invented values only.
const TARGET='tester@pc.invalid',ROOT='C:\\Program Files (x86)\\otwoline\\studyq',EXE_DIR=ROOT+'\\report';
const HARU='%PDF-1.3\n%invented\n1 0 obj\n<< /Producer (Haru\\040Free\\040PDF\\040Library 2.0.8) /Title (Invented) >>\nendobj\n%%EOF\n';
const PRINCIPAL={schoolKey:'88001',teacherKey:'90001'};
const now=()=>new Date('2040-01-16T09:00:00.123Z');
let base:string,counter=0;
beforeAll(async()=>{base=await realpath(await mkdtemp(resolve(tmpdir(),'pc-paper-executor-')));});
afterAll(async()=>{await rm(base,{recursive:true,force:true});});
const sha=(bytes:Buffer|string)=>createHash('sha256').update(bytes).digest('hex');

/** The fake ssh: decodes the one -EncodedCommand, recognises the step by its fixed program and acts on `<state>/pc`, the invented
 * `%APPDATA%\otwoline\studyq`. Its StudyQPdfManager writes the image cache and examset.info from pdf.info as the native program does;
 * its ETestMaker writes no result when examset.info misses a required member (the native program crashes). */
const fakeSsh=(state:string)=>String.raw`#!${process.execPath}
const fs=require('fs'),path=require('path');
const state=${JSON.stringify(state)},pc=path.join(state,'pc'),ex=path.join(pc,'examset'),argv=process.argv.slice(2);
const config=JSON.parse(fs.readFileSync(path.join(state,'config.json'),'utf8'));
const scenario=JSON.parse(fs.readFileSync(path.join(state,'scenario.json'),'utf8'));
const word=argv[argv.length-1]||'',m=/^powershell -NoProfile -NonInteractive -EncodedCommand ([A-Za-z0-9+/=]+)$/.exec(word);
const script=m?Buffer.from(m[1],'base64').toString('utf16le'):'';
const step=script.includes('Get-Process -Name')?'busy':script.includes('StudyQPdfManager.exe')?'manager':script.includes('ETestMaker.exe')?'maker'
  :script.includes('Get-ChildItem')?'list':script.includes('ToBase64String')?'collect':script.includes('foreach($f in @($r,$e,$i))')?'cleanup':'unknown';
fs.appendFileSync(path.join(state,'log.jsonl'),JSON.stringify({tool:'ssh',argv,step,script})+'\n');
const result=path.join(ex,'examset_result.info'),setInfo=path.join(ex,'examset.info'),pdfInfo=path.join(pc,'pdf.info');
const count=name=>{const f=path.join(state,name);const n=Number(fs.existsSync(f)?fs.readFileSync(f,'utf8'):'0')+1;fs.writeFileSync(f,String(n));return n;};
const fail=(scenario.sshFail||{})[step];
if(fail){process.stderr.write(fail.stderr||'');process.exit(fail.code);}
else if((scenario.hang||[]).includes(step)){setTimeout(()=>{},1e8);}
else if(step==='busy'){if(scenario.busy)process.stdout.write('pc-busy\r\n');}
else if(step==='manager'){
  if(!script.includes(config.manager))process.exit(7);
  const n=count('manager-count');
  fs.copyFileSync(pdfInfo,path.join(state,'pdf.info.seen.'+(n-1)));
  if(scenario.managerTimeout){process.stdout.write('pc-executor-timeout\r\n');process.exit(124);}
  const info=JSON.parse(new TextDecoder('euc-kr').decode(fs.readFileSync(pdfInfo)));
  const item=info.print_list[0];
  for(const exam of item.exam_list||[]){const dir=path.join(pc,'exam',String(exam.lecture_key));fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(path.join(dir,exam.lecture_key+'.png'),'qr');}
  fs.mkdirSync(ex,{recursive:true});
  fs.writeFileSync(setInfo,JSON.stringify({...item,solution_type:'',web_server:'api.studyq.net',clg_no:info.sk}));
}else if(step==='list'){
  if(fs.existsSync(result))fs.unlinkSync(result);
  fs.mkdirSync(ex,{recursive:true});
  process.stdout.write(fs.readdirSync(ex).filter(n=>/\.pdf$/i.test(n)).map(n=>n+'\r\n').join(''));
}else if(step==='maker'){
  if(!script.includes(config.maker))process.exit(7);
  const set=JSON.parse(fs.readFileSync(setInfo,'utf8'));
  if(config.required.some(k=>set[k]===undefined))process.exit(0);
  const n=count('maker-count'),name='20261128'+String(n).padStart(9,'0')+'.pdf';
  for(const file of scenario.made||[{name,content:config.haru}])fs.writeFileSync(path.join(ex,file.name),file.content);
  const answer=scenario.result!==undefined?scenario.result:{pdf_list:[{pdf_type:set.paper_type,pdf_file:name,pri_no:0,page_cnt:scenario.pages||4}],result:'ok'};
  if(answer!==null)fs.writeFileSync(result,typeof answer==='string'?answer:JSON.stringify(answer));
  if(scenario.makerTimeout){process.stdout.write('pc-executor-timeout\r\n');process.exit(124);}
}else if(step==='collect'){
  process.stdout.write(fs.existsSync(result)?fs.readFileSync(result).toString('base64')+'\r\n':'-\r\n');
}else if(step==='cleanup'){
  const named=/\$f=Join-Path \$x '([^']*)'/.exec(script);
  for(const file of [named?path.join(ex,named[1]):null,result,setInfo,pdfInfo])if(file&&fs.existsSync(file))fs.unlinkSync(file);
}else process.exit(9);
`;
/** The fake scp: copies between the current directory and `<state>/pc` by the path under the StudyQ folder. */
const fakeScp=(state:string)=>String.raw`#!${process.execPath}
const fs=require('fs'),path=require('path');
const state=${JSON.stringify(state)},pc=path.join(state,'pc'),argv=process.argv.slice(2);
const config=JSON.parse(fs.readFileSync(path.join(state,'config.json'),'utf8'));
const scenario=JSON.parse(fs.readFileSync(path.join(state,'scenario.json'),'utf8'));
const [from,to]=argv.slice(-2),remote=new RegExp('^'+config.target.replace('.','\\.')+':AppData/Roaming/otwoline/studyq/(pdf\\.info|examset/\\d{17}\\.pdf)$');
fs.appendFileSync(path.join(state,'log.jsonl'),JSON.stringify({tool:'scp',argv,cwd:process.cwd()})+'\n');
const fail=scenario.scpFail;
if(fail&&(fail.on==='both'||(fail.on==='download')===remote.test(from))){process.stderr.write(fail.stderr||'');process.exit(fail.code);}
if(remote.test(to))fs.copyFileSync(path.resolve(from),path.join(pc,remote.exec(to)[1]));
else{
  if(!remote.test(from))process.exit(8);
  const source=path.join(pc,remote.exec(from)[1]);
  if(!fs.existsSync(source)){process.stderr.write('scp: no such file\n');process.exit(1);}
  if(scenario.raceOutput)fs.writeFileSync(scenario.raceOutput,'earlier');
  fs.copyFileSync(source,path.resolve(to));
}
`;
type Scenario={busy?:boolean;managerTimeout?:boolean;makerTimeout?:boolean;result?:unknown;made?:{name:string;content:string}[];pages?:number;hang?:string[];
  sshFail?:Record<string,{code:number;stderr?:string}>;scpFail?:{on:'upload'|'download'|'both';code:number;stderr?:string};raceOutput?:string};
async function fixture(scenario:Scenario={},pcFiles:Record<string,string>={},options:PcExecutorOptions={},exeDir=EXE_DIR){
  const state=resolve(base,'s'+(++counter)),pc=resolve(state,'pc'),out=resolve(state,'out'),lockDir=resolve(state,'official-reports');
  await mkdir(resolve(pc,'examset','theme'),{recursive:true});await mkdir(out,{recursive:true});
  for(const [name,content] of Object.entries(pcFiles))await writeFile(resolve(pc,name),content);
  await writeFile(resolve(state,'scenario.json'),JSON.stringify(scenario));
  await writeFile(resolve(state,'config.json'),JSON.stringify({target:TARGET,haru:HARU,required:[...PRINT_ITEM_MEMBERS],
    manager:"Start-Process -FilePath '"+ROOT+"\\studylink\\StudyQPdfManager.exe' -WorkingDirectory '"+ROOT+"\\studylink' -PassThru",
    maker:"Start-Process -FilePath '"+ROOT+"\\ipm\\ETestMaker.exe' -WorkingDirectory '"+ROOT+"\\ipm' -PassThru"}));
  const ssh=resolve(state,'ssh'),scp=resolve(state,'scp');
  await writeFile(ssh,fakeSsh(state));await writeFile(scp,fakeScp(state));await chmod(ssh,0o755);await chmod(scp,0o755);
  const identityFile=resolve(state,'id_invented'),knownHostsFile=resolve(state,'known_hosts_invented');
  await writeFile(identityFile,'invented\n');await writeFile(knownHostsFile,'invented\n');
  const config={target:TARGET,identityFile,knownHostsFile,exeDir,lockDir};
  const executor=pcPaperExecutor(config,{sshPath:ssh,scpPath:scp,afterTimeoutDelayMs:10,lockPollMs:20,...options})!;
  const log=async()=>{try{return (await readFile(resolve(state,'log.jsonl'),'utf8')).trim().split('\n').filter(Boolean).map(l=>JSON.parse(l));}catch{return [];}};
  const seen=async(n=0)=>readFile(resolve(state,'pdf.info.seen.'+n));
  /** Every file under the PC folder, relative, sorted. */
  const files=async()=>(await readdir(pc,{recursive:true,withFileTypes:true})).filter(e=>e.isFile()).map(e=>join(e.parentPath,e.name).slice(pc.length+1)).sort();
  return {state,pc,out,lockDir,config,executor,log,seen,files,ssh,scp};
}
/** An invented score-paper item, as the score view answers it: `exam_size_list` a string in one row and an array in the other. */
const scoreItem=(over:Record<string,unknown>={})=>({print_type:'each',paper_type:'score',pri_no:770011,student_name:'Invented Student',examset_title:'Invented 가 title',
  add_answer:1,add_code:0,add_difficulty:1,add_explain:0,add_pat:1,exam_dispose_type:'d2',exam_spacer:5,
  exam_list:[{lecture_key:5001,sel_type:'obj',exam_size_list:'[{"answer_height":40,"exam_height":100,"exam_type":"obj","explain_height":50,"obj_height":30}]'},
    {lecture_key:5002,sel_type:'sbj',exam_size_list:[{answer_height:41,exam_height:101,exam_type:'sbj',explain_height:51,obj_height:0}]}],...over});
const print=(f:Awaited<ReturnType<typeof fixture>>,name='p.pdf',over:Partial<PcPrintInput>={})=>
  f.executor.print({item:scoreItem(),fromExamList:false,principal:PRINCIPAL,outputPath:resolve(f.out,name),...over});
const code=async(promise:Promise<unknown>)=>{try{await promise;return 'resolved';}catch(error){return {code:(error as Error).message,pcCleanup:(error as PcExecutorError).pcCleanup};}};
const NATIVE_CACHE=['exam/5001/5001.png','exam/5002/5002.png'];
const sorted=(...names:string[])=>[...names].sort();

describe('the paper executor on the PC',()=>{
  test('pc_paper_executor_install_root_is_the_parent_of_exe_dir',()=>{
    expect(studyqInstallRoot(EXE_DIR)).toBe(ROOT);
    expect(studyqInstallRoot(EXE_DIR+'\\')).toBe(ROOT);
    expect(studyqInstallRoot('C:\\report')).toBeNull();
    expect(pcPaperExecutor({target:TARGET,identityFile:'/i',knownHostsFile:'/k',exeDir:'C:\\report',lockDir:'/l'})).toBeNull();
    expect(()=>pcPaperScripts.manager("C:\\x';Remove-Item C:\\")).toThrow('pc_executor_failed');
  });

  test('pc_paper_executor_prints_one_item_and_leaves_only_the_native_cache',async()=>{
    const f=await fixture({},{'examset/20200101000000000.pdf':'teacher kept'});
    const out=await print(f);
    const pdf=await readFile(resolve(f.out,'p.pdf'));
    expect(out).toEqual({status:'rendered',paperType:'score',pageCount:4,sha256:sha(pdf),outputPath:resolve(f.out,'p.pdf'),pcCleanup:'done'});
    expect(pdf.toString('latin1')).toBe(HARU);
    // pdf.info: CP949, the six members in order, sk and uk numbers, every exam_size_list an array.
    const bytes=await f.seen();
    expect([...bytes].some(b=>b>=0x80)).toBe(true);
    const text=decodeCp949(bytes),info=JSON.parse(text);
    expect(Object.keys(info)).toEqual(['app_ver','cmd','pid','print_list','sk','uk']);
    expect(info).toMatchObject({app_ver:'8.6',cmd:'Print',sk:88001,uk:90001});
    expect(info.pid).toMatch(/^\d{13}$/);
    expect(info.print_list).toHaveLength(1);
    expect(info.print_list[0].exam_list.map((e:any)=>Array.isArray(e.exam_size_list))).toEqual([true,true]);
    expect(info.print_list[0].exam_list[0].exam_size_list).toEqual([{answer_height:40,exam_height:100,exam_type:'obj',explain_height:50,obj_height:30}]);
    expect({...info.print_list[0],exam_list:null}).toEqual({...scoreItem(),exam_list:null});
    // The PC holds the teacher's PDF, the theme folder and the native cache only.
    expect(await f.files()).toEqual(sorted('examset/20200101000000000.pdf',...NATIVE_CACHE));
    expect(existsSync(resolve(f.pc,'examset','theme'))).toBe(true);
    // Fixed words only: the steps in order, the two program folders under the install root, scp of pdf.info and the named PDF.
    const log=await f.log();
    expect(log.map(e=>e.tool+':'+(e.step??''))).toEqual(['ssh:busy','scp:','ssh:manager','ssh:list','ssh:maker','ssh:collect','scp:','ssh:cleanup']);
    const scps=log.filter(e=>e.tool==='scp');
    expect(scps[0].argv.slice(-2)).toEqual(['./pdf.info',TARGET+':AppData/Roaming/otwoline/studyq/pdf.info']);
    expect(scps[1].argv.at(-2)).toBe(TARGET+':AppData/Roaming/otwoline/studyq/examset/20261128000000001.pdf');
    expect(scps[1].argv.at(-1)).toMatch(/^\.\/\.pc-executor-[0-9a-f]{16}\.tmp$/);
    expect(scps[1].cwd).toBe(f.out);
    const words=['-i',f.config.identityFile,'-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','UserKnownHostsFile='+f.config.knownHostsFile,'-o','IdentitiesOnly=yes','-o','ConnectTimeout=10','-F','none'];
    for(const e of log){expect(e.argv.slice(0,14)).toEqual(words);expect(e.argv).toHaveLength(16);if(e.tool==='ssh')expect(e.argv[14]).toBe(TARGET);}
    const scripts=log.filter(e=>e.tool==='ssh').map(e=>e.script as string).join('\n');
    expect(scripts).toContain("Join-Path $env:USERPROFILE 'AppData\\Roaming\\otwoline\\studyq'");
    // Only processes the call started are stopped: by id, never by name.
    expect(scripts.match(/Stop-Process[^;]*/g)!.every(s=>/Stop-Process -Id \$(p\.Id|_\.ProcessId) -Force/.test(s))).toBe(true);
    expect(scripts).toContain("Get-CimInstance Win32_Process -Filter ('ParentProcessId='+$p.Id)");
    expect(scripts).not.toMatch(/Stop-Process -Name|taskkill/i);
    // The host-wide lock is gone after the call.
    expect(existsSync(resolve(f.lockDir,'pc-executor.lock'))).toBe(false);
  });

  test('pc_paper_executor_refuses_pc_busy_before_sending_anything',async()=>{
    const f=await fixture({busy:true});
    expect(await code(print(f))).toEqual({code:'pc_busy',pcCleanup:undefined});
    expect((await f.log()).map(e=>e.step)).toEqual(['busy']);
    expect(await f.files()).toEqual([]);
    expect(pcPaperScripts.busy()).toContain("Get-Process -Name 'StudyQPdfManager','ETestMaker','PDFMerge'");
  });

  test('pc_paper_executor_item_incomplete_and_unencodable_refuse_before_any_remote_step',async()=>{
    const f=await fixture();
    for(const member of PRINT_ITEM_MEMBERS){
      const item=scoreItem();delete (item as Record<string,unknown>)[member];
      expect(await code(print(f,'x.pdf',{item}))).toEqual({code:'paper_print_item_incomplete',pcCleanup:undefined});
    }
    expect(await code(print(f,'x.pdf',{item:scoreItem(),fromExamList:true}))).toEqual({code:'paper_print_item_incomplete',pcCleanup:undefined});
    expect(await code(print(f,'x.pdf',{item:scoreItem({exam_list:[{lecture_key:1,exam_size_list:'[{'}]})}))).toEqual({code:'paper_print_item_incomplete',pcCleanup:undefined});
    expect(await code(print(f,'x.pdf',{item:scoreItem({examset_title:'Invented \u{1F600}'})}))).toEqual({code:'pc_input_unencodable',pcCleanup:undefined});
    expect(await f.log()).toEqual([]);
    expect(existsSync(resolve(f.lockDir,'pc-executor.lock'))).toBe(false);
    // An item that the native program would crash on never reaches it: the fake ETestMaker writes no result without the members.
    expect(()=>pdfInfoText(scoreItem(),{schoolKey:'0',teacherKey:'90001'},'1')).toThrow('pc_executor_failed');
  });

  test('pc_paper_executor_manager_timeout_stops_only_its_own_processes_and_cleans_up',async()=>{
    const f=await fixture({managerTimeout:true});
    expect(await code(print(f))).toEqual({code:'pc_executor_timeout',pcCleanup:'done'});
    expect((await f.log()).map(e=>e.step??'scp')).toEqual(['busy','scp','manager','cleanup']);
    expect(await f.files()).toEqual([]);
    const manager=pcPaperScripts.manager(ROOT+'\\studylink');
    expect(manager).toContain('LastWriteTimeUtc -gt $s');
    expect(manager).toContain('Stop-Process -Id $p.Id -Force');
    // A teacher's pdf.info and examset.info of an earlier print are overwritten and deleted the same way.
    const g=await fixture({result:{result:'fail'}},{'pdf.info':'earlier','examset/examset.info':'earlier'});
    expect(await code(print(g))).toEqual({code:'pc_output_invalid',pcCleanup:'done'});
    expect(await g.files()).toEqual(sorted('examset/20261128000000001.pdf',...NATIVE_CACHE));
  });

  test('pc_paper_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf',async()=>{
    const named=(over:Record<string,unknown>)=>({result:'ok',pdf_list:[{pdf_type:'score',pdf_file:'20261128000000001.pdf',pri_no:0,page_cnt:4,...over}]});
    const cases:[Scenario,Record<string,string>,string,string[]][]=[
      [{result:{...named({}),result:'fail'}},{},'pc_output_invalid',[]],
      [{result:null},{},'pc_output_invalid',['examset/20261128000000001.pdf']],
      [{result:'not json'},{},'pc_output_invalid',['examset/20261128000000001.pdf']],
      [{result:named({page_cnt:0})},{},'pc_output_invalid',[]],
      [{result:{result:'ok',pdf_list:[named({}).pdf_list[0],named({pdf_file:'20261128000000002.pdf'}).pdf_list[0]]},
        made:[{name:'20261128000000001.pdf',content:HARU},{name:'20261128000000002.pdf',content:HARU}]},{},'pc_output_invalid',['examset/20261128000000001.pdf','examset/20261128000000002.pdf']],
      [{result:named({pdf_file:'..\\x.pdf'})},{},'pc_output_invalid',['examset/20261128000000001.pdf']],
      [{result:named({pdf_file:'20200101000000000.pdf'}),made:[]},{'examset/20200101000000000.pdf':HARU},'pc_output_invalid',['examset/20200101000000000.pdf']],
      [{made:[{name:'20261128000000001.pdf',content:'%PDF-1.3\n<< /Producer (Other) >>'}]},{},'pc_output_invalid',[]],
      [{made:[{name:'20261128000000001.pdf',content:'not a pdf'}]},{},'pc_output_invalid',[]],
    ];
    for(const [scenario,pcFiles,want,left] of cases){
      const f=await fixture(scenario,pcFiles);
      expect({scenario,out:await code(print(f))}).toEqual({scenario,out:{code:want,pcCleanup:'done'}});
      expect({scenario,files:await f.files()}).toEqual({scenario,files:[...left,...NATIVE_CACHE].sort()});
      expect(existsSync(resolve(f.out,'p.pdf'))).toBe(false);
    }
  });

  test('pc_paper_executor_maker_timeout_deletes_only_the_pdf_the_result_names',async()=>{
    const f=await fixture({makerTimeout:true});
    expect(await code(print(f))).toEqual({code:'pc_executor_timeout',pcCleanup:'done'});
    expect(await f.files()).toEqual(NATIVE_CACHE);
    expect((await f.log()).map(e=>e.step??'scp')).toEqual(['busy','scp','manager','list','maker','collect','cleanup']);
    // A result naming nothing qualified: the infos go, the unnamed PDF stays.
    const g=await fixture({makerTimeout:true,result:{result:'ok',pdf_list:[]}});
    expect(await code(print(g))).toEqual({code:'pc_executor_timeout',pcCleanup:'done'});
    expect(await g.files()).toEqual(sorted('examset/20261128000000001.pdf',...NATIVE_CACHE));
  });

  test('pc_paper_executor_unreachable_and_host_key_mismatch',async()=>{
    const down=await fixture({sshFail:{busy:{code:255,stderr:'ssh: connect to host pc.invalid port 22: Connection timed out\n'}}});
    expect(await code(print(down))).toEqual({code:'pc_unreachable',pcCleanup:undefined});
    const key=await fixture({sshFail:{busy:{code:255,stderr:'Host key verification failed.\n'}}});
    expect(await code(print(key))).toEqual({code:'pc_host_key_mismatch',pcCleanup:undefined});
    const upload=await fixture({scpFail:{on:'upload',code:1,stderr:'ssh: connect to host pc.invalid port 22: Connection refused\n'}});
    expect(await code(print(upload))).toEqual({code:'pc_unreachable',pcCleanup:'done'});
    const hung=await fixture({hang:['list']},{},{stepTimeoutMs:300});
    expect(await code(print(hung))).toMatchObject({code:'pc_unreachable'});
    expect(existsSync(resolve(hung.out,'p.pdf'))).toBe(false);
  });

  test('pc_paper_executor_output_that_appeared_meanwhile_is_output_exists',async()=>{
    const f=await fixture();
    await writeFile(resolve(f.state,'scenario.json'),JSON.stringify({raceOutput:resolve(f.out,'p.pdf')}));
    expect(await code(print(f))).toEqual({code:'output_exists',pcCleanup:'done'});
    expect(await readFile(resolve(f.out,'p.pdf'),'utf8')).toBe('earlier');
    expect((await readdir(f.out)).filter(n=>n.endsWith('.tmp'))).toEqual([]);
  });

  test('pc_paper_executor_shares_the_host_wide_lock_of_the_report_executor',async()=>{
    const f=await fixture({},{},{deadlineMs:300});
    await mkdir(f.lockDir,{recursive:true});
    // The lock of a live process (this one) with its start time: the call waits and ends at its deadline without any remote step.
    await writeFile(resolve(f.lockDir,'pc-executor.lock'),JSON.stringify({pid:process.pid,start:await processStartTime(process.pid),nonce:'a'.repeat(32)})+'\n');
    expect(await code(print(f))).toEqual({code:'pc_executor_timeout',pcCleanup:undefined});
    expect(await f.log()).toEqual([]);
    await rm(resolve(f.lockDir,'pc-executor.lock'));
  });
});

// ---- the print items and the service of academy_paper_print_pdf_get -------------------------------------
/** An invented `getTestExamList` answer of a print case, shaped as the server answers it. */
const examListAnswer=(over:Record<string,unknown>={})=>({result:'OK',examset_title:'[Invented] 유형다지기',sub_title:'Invented sub title',examset_type:'유형다지기',
  add_comment:0,add_answer:0,add_explain:0,add_code:0,add_difficulty:1,add_pat:1,theme_type:'ganga/theme/7-test',theme_color:'1',exam_dispose_type:'d4',exam_spacer:7,
  test_code:'INV01',create_answer:1,unused_member:'x',
  exam_list:[{lecture_key:5001,stage_id:'elem',sel_type:'obj',exam_size_list:'[{"answer_height":49,"exam_height":107,"exam_type":"obj","explain_height":236,"obj_height":253}]'},
    {lecture_key:5002,stage_id:'mid',sel_type:'sbj',exam_size_list:'[]'}],...over});
function fakePaperExecutor(fails:Record<number,PcExecutorError>={}){
  const calls:PcPrintInput[]=[];
  const executor:PcPaperExecutor={async print(input){
    calls.push(input);const failure=fails[calls.length];if(failure)throw failure;
    printItemForPc(input.item,{fromExamList:input.fromExamList});
    await writeFile(input.outputPath,HARU,{flag:'wx'});
    return {status:'rendered',paperType:String(input.item.paper_type),pageCount:4,sha256:sha(HARU),outputPath:input.outputPath,pcCleanup:'done'};
  }};
  return {executor,calls};
}
function fakeBackend(over:Partial<PaperPrintBackend>={}){
  const calls:string[]=[];
  const backend:PaperPrintBackend={
    async paperPrintPrincipal(){calls.push('principal');return PRINCIPAL;},
    async readPaperPrintSource(paperNo,printCase){calls.push('paper '+paperNo+' '+printCase);return examListAnswer();},
    async readResultPrintList(testingNo,studentKey){calls.push('result '+testingNo+' '+studentKey);return [scoreItem()];},
    ...over};
  return {backend,calls};
}
async function serviceFixture(name:string,over:Partial<PaperPrintBackend>={},fails:Record<number,PcExecutorError>={}){
  const root=resolve(base,name,'print-pdfs'),b=fakeBackend(over),pc=fakePaperExecutor(fails);
  return {root,...b,pc,service:new PaperPrintPdfService(b.backend,pc.executor,root,now)};
}

describe('the print items and academy_paper_print_pdf_get',()=>{
  test('pc_paper_item_is_built_as_the_page_builds_it',()=>{
    const item=paperPrintItem(examListAnswer(),'teacher_paper','88001');
    expect(Object.keys(item)).toEqual(['print_type','paper_type','logo','examset_title','sub_title','examset_type','stage_id','add_incorrect_comment','add_answer',
      'add_explain','add_code','add_difficulty','add_pat','theme_type','theme_color','exam_dispose_type','exam_spacer','student_name','test_code','exam_list']);
    expect(item).toEqual({print_type:'each',paper_type:'test',logo:'https://storage.studyq.net/school_data/88001/88001-logo.png',examset_title:'[Invented] 유형다지기',
      sub_title:'Invented sub title',examset_type:'유형다지기',stage_id:'elem',add_incorrect_comment:0,add_answer:0,add_explain:0,add_code:0,add_difficulty:1,add_pat:1,
      theme_type:'ganga/theme/7-test',theme_color:'1',exam_dispose_type:'d4',exam_spacer:7,student_name:'',test_code:'INV01',exam_list:examListAnswer().exam_list});
    expect(paperPrintItem(examListAnswer(),'student_paper','88001').paper_type).toBe('test');
    expect(paperPrintItem(examListAnswer(),'answer_paper','88001').paper_type).toBe('answer');
    // A member the server does not send is absent, as JSON.stringify of the page drops it.
    const without=examListAnswer();delete (without as Record<string,unknown>).test_code;
    expect('test_code' in paperPrintItem(without,'teacher_paper','88001')).toBe(false);
    expect(paperPrintSourceForm('6724110','answer_paper')).toEqual({p_process:'getTestExamList',p_no:'6724110',p_option_type:'answer_paper'});
    expect(resultPrintListForm(TESTING,STUDENT)).toEqual({p_process:'getStudyResultSingleTestingSingleUser',condition:JSON.stringify({testing_no:Number(TESTING),pri_no:Number(STUDENT),student_name:'',
      score:{create:1},incorrect:{create:0},similar:{create:0},advance:{create:0},report:{create:0}})});
  });

  test('pc_paper_item_completeness_is_checked_for_each_source',async()=>{
    // paper: add_incorrect_comment comes from add_comment and is required.
    const noComment=examListAnswer();delete (noComment as Record<string,unknown>).add_comment;
    const paper=await serviceFixture('incomplete-paper',{async readPaperPrintSource(){return noComment;}});
    expect(await paper.service.get({source:'paper',paperNo:'6724110'})).toMatchObject({status:'unavailable',reasonCode:'paper_print_item_incomplete',pdfs:[]});
    expect(paper.pc.calls).toEqual([]);
    // result: the score items need no add_incorrect_comment, but every other member.
    const ok=await serviceFixture('complete-result');
    expect(await ok.service.get({source:'result',testingNo:TESTING,studentKey:STUDENT})).toMatchObject({status:'rendered'});
    const noPat=await serviceFixture('incomplete-result',{async readResultPrintList(){return [scoreItem(),scoreItem({add_pat:undefined})];}});
    expect(await noPat.service.get({source:'result',testingNo:TESTING,studentKey:STUDENT})).toMatchObject({status:'unavailable',reasonCode:'paper_print_item_incomplete',pdfs:[]});
    expect(noPat.pc.calls).toEqual([]);
    // clinic: the items of a write answer, checked before the first PC step.
    const clinic=await serviceFixture('incomplete-clinic');
    expect(await clinic.service.printClinicList([scoreItem(),scoreItem({exam_spacer:null})],PAPER)).toMatchObject({status:'unavailable',reasonCode:'paper_print_item_incomplete',pdfs:[]});
    expect(await clinic.service.printClinicList([],PAPER)).toMatchObject({status:'unavailable',reasonCode:'paper_print_list_empty',pdfs:[]});
    expect(clinic.pc.calls).toEqual([]);
    // exam_size_list strings become arrays; an item without exam_list is incomplete.
    expect(printItemForPc(paperPrintItem(examListAnswer(),'teacher_paper','88001'),{fromExamList:true}).exam_list).toEqual([
      {lecture_key:5001,stage_id:'elem',sel_type:'obj',exam_size_list:[{answer_height:49,exam_height:107,exam_type:'obj',explain_height:236,obj_height:253}]},
      {lecture_key:5002,stage_id:'mid',sel_type:'sbj',exam_size_list:[]}]);
    expect(()=>printItemForPc(scoreItem({exam_list:[]}),{fromExamList:false})).toThrow('paper_print_item_incomplete');
    expect(()=>printItemForPc(scoreItem({exam_list:[{exam_size_list:'{}'}]}),{fromExamList:false})).toThrow('paper_print_item_incomplete');
  });

  test('pc_paper_answers_one_entry_per_item_with_numbered_paths',async()=>{
    const f=await serviceFixture('numbered',{async readResultPrintList(){return [scoreItem(),scoreItem({paper_type:'incorrect'})];}});
    const out=await f.service.get({source:'result',testingNo:TESTING,studentKey:STUDENT,outputPath:'score.pdf'}) as any;
    expect(out).toMatchObject({status:'rendered',source:'result',testingNo:TESTING,studentKey:STUDENT,academyWrites:0,execution:'read_only_lms_pc_executor',pcCleanup:'done'});
    expect(out.pdfs).toEqual([{paperType:'score',pageCount:4,outputPath:resolve(f.root,'score-1.pdf'),sha256:sha(HARU)},{paperType:'incorrect',pageCount:4,outputPath:resolve(f.root,'score-2.pdf'),sha256:sha(HARU)}]);
    expect(f.calls).toEqual(['principal']);
    expect(f.pc.calls.map(c=>[c.fromExamList,c.principal])).toEqual([[false,PRINCIPAL],[false,PRINCIPAL]]);
    // The paper source: one item, built from the answer of the print case, under a default name with no personal value.
    const r=await serviceFixture('result');
    await r.service.get({source:'result',testingNo:TESTING,studentKey:STUDENT});
    expect(r.calls).toEqual(['principal','result '+TESTING+' '+STUDENT]);
    const p=await serviceFixture('paper');
    const paper=await p.service.get({source:'paper',paperNo:'6724110',printCase:'answer_paper'}) as any;
    expect(paper).toMatchObject({status:'rendered',source:'paper',paperNo:'6724110',printCase:'answer_paper'});
    expect(paper.pdfs).toEqual([{paperType:'answer',pageCount:4,outputPath:resolve(p.root,'print-answer-paper-6724110-20400116T090000Z.pdf'),sha256:sha(HARU)}]);
    expect(p.calls).toEqual(['principal','paper 6724110 answer_paper']);
    // The executor takes the item with every exam_size_list parsed into an array.
    expect(p.pc.calls[0]!.item).toEqual(printItemForPc(paperPrintItem(examListAnswer(),'answer_paper','88001'),{fromExamList:true}));
    expect(p.pc.calls[0]!.fromExamList).toBe(true);
    expect((await p.service.get({source:'paper',paperNo:'6724111'}) as any).printCase).toBe('teacher_paper');
    // An existing n-th path refuses before any PC step.
    const exists=await serviceFixture('exists',{async readResultPrintList(){return [scoreItem(),scoreItem()];}});
    await mkdir(exists.root,{recursive:true});await writeFile(resolve(exists.root,'s-2.pdf'),'kept');
    expect(await exists.service.get({source:'result',testingNo:TESTING,studentKey:STUDENT,outputPath:'s.pdf'})).toMatchObject({status:'unavailable',reasonCode:'output_exists'});
    expect(exists.pc.calls).toEqual([]);
  });

  test('pc_paper_failed_item_keeps_the_items_before_it',async()=>{
    const f=await serviceFixture('partial',{async readResultPrintList(){return [scoreItem(),scoreItem(),scoreItem()];}},{2:new PcExecutorError('pc_busy')});
    const out=await f.service.get({source:'result',testingNo:TESTING,studentKey:STUDENT,outputPath:'s.pdf'}) as any;
    expect(out).toMatchObject({status:'unavailable',reasonCode:'pc_busy',failure:{code:'pc_busy'}});
    expect(out.pdfs).toEqual([{paperType:'score',pageCount:4,outputPath:resolve(f.root,'s-1.pdf'),sha256:sha(HARU)}]);
    expect(f.pc.calls).toHaveLength(2);
    expect(existsSync(resolve(f.root,'s-1.pdf'))).toBe(true);
    const timeout=await serviceFixture('timeout',{},{1:new PcExecutorError('pc_executor_timeout','incomplete')});
    expect(await timeout.service.get({source:'result',testingNo:TESTING,studentKey:STUDENT})).toMatchObject({status:'unavailable',reasonCode:'pc_executor_timeout',pcCleanup:'incomplete',pdfs:[]});
  });

  test('pc_paper_without_pc_executor_is_pc_executor_not_configured',async()=>{
    const f=await serviceFixture('none');
    const service=new PaperPrintPdfService(f.backend,null,f.root,now);
    expect(await service.get({source:'paper',paperNo:'6724110'})).toMatchObject({status:'unavailable',reasonCode:'pc_executor_not_configured'});
    expect(await service.printClinicList([scoreItem()],PAPER)).toMatchObject({status:'unavailable',reasonCode:'pc_executor_not_configured'});
    expect(f.calls).toEqual([]);
  });

  test('pc_paper_input_takes_source_and_its_keys_only',()=>{
    const bad=(value:unknown)=>paperPrintPdfGetSchema.safeParse(value).success;
    expect(bad({source:'paper',paperNo:'6724110'})).toBe(true);
    expect(bad({source:'paper',paperNo:'6724110',printCase:'student_paper',outputPath:'a.pdf'})).toBe(true);
    expect(bad({source:'result',testingNo:TESTING,studentKey:STUDENT})).toBe(true);
    for(const value of [{source:'paper'},{source:'paper',paperNo:'6724110',testingNo:TESTING},{source:'result',testingNo:TESTING},
      {source:'result',testingNo:TESTING,studentKey:STUDENT,paperNo:'1'},{source:'paper',paperNo:'1000000000'},{source:'paper',paperNo:'1',printCase:'custom_paper'},
      {paperNo:'1'},{mode:'exam',paperNo:'1'},{source:'paper',paperNo:'1',paperKeys:['1']},{source:'paper',paperNo:'1',selType:'obj'},
      {source:'paper',paperNo:'1',includeAnswers:true},{source:'paper',paperNo:'1',includeExplain:true}])expect({value,ok:bad(value)}).toEqual({value,ok:false});
  });

  test('pc_paper_retired_studyq_runner_option_is_dropped_unread',()=>{
    expect(withoutRetiredStudyqRunnerArguments(['--a','--studyq-runner','/no/python','x','/no/src','y','/no/assets','--b']).remainingArgs).toEqual(['--a','--b']);
    expect(()=>withoutRetiredStudyqRunnerArguments(['--studyq-runner','/no/python'])).toThrow('studyq_runner_arguments_invalid');
  });

  test('pc_paper_tool_surface_is_not_larger_than_before',()=>{
    const tool=keptTools.find(t=>t.name==='academy_paper_print_pdf_get')!;
    expect(Buffer.byteLength(JSON.stringify(tool),'utf8')).toBeLessThanOrEqual(795);
    expect(Object.keys((tool.inputSchema as any).properties)).toEqual(['source','paperNo','printCase','testingNo','studentKey','outputPath']);
    const save=keptTools.find(t=>t.name==='academy_clinic_paper_save')!;
    expect((save.inputSchema as any).properties.printPdf).toEqual({type:'boolean'});
  });

  test('pc_paper_reads_only_read_class_routes_i1_write_forbidden',async()=>{
    // jsdom and the HTTP client load here, so a host without the module's dependencies fails only this test.
    const {AcademyHttpClient}=await import('../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
    const {NativeDailyRecordBackend}=await import('../src/native-dailyrecord-backend.ts');
    const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
    const principal='<a href="/logout">Fictional Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="90001"></form><form name="form1"><input name="g_pri_no" value="90001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
    let calls=0;const bodies:string[]=[];
    const guard=writeForbiddenTransport((_url,init)=>{calls++;
      if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
      const body=String(init?.body??'');bodies.push(body);
      const form=new URLSearchParams(body);
      if(form.get('p_process')==='getTestExamList')return new Response(JSON.stringify(examListAnswer()));
      return new Response(JSON.stringify({result:'OK',print_list:[scoreItem({pri_no:Number(STUDENT)})]}));});
    const admission={teacherId:'90001',teacherLabel:'Fictional Teacher',date:'2040-01-04',students:[],sourceBundleSha256:'b'.repeat(64)};
    const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
    const backend=new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'}));
    const pc=fakePaperExecutor();
    const service=new PaperPrintPdfService(backend as never,pc.executor,resolve(base,'i1','print-pdfs'),now);
    expect(await service.get({source:'paper',paperNo:'6724110',printCase:'student_paper'})).toMatchObject({status:'rendered'});
    expect(await service.get({source:'result',testingNo:TESTING,studentKey:STUDENT})).toMatchObject({status:'rendered'});
    expect(guard.forbidden()).toEqual([]);
    expect(guard.requests.filter(r=>r.role!=='login').map(r=>r.role)).toEqual(['read','read','read']);
    expect(bodies).toEqual([new URLSearchParams(paperPrintSourceForm('6724110','student_paper')).toString(),new URLSearchParams(resultPrintListForm(TESTING,STUDENT)).toString()]);
    expect(pc.calls.map(c=>c.principal)).toEqual([PRINCIPAL,PRINCIPAL]);
    expect(pc.calls[0]!.item.logo).toBe('https://storage.studyq.net/school_data/88001/88001-logo.png');
    // A score item that names another student is refused before any PC step.
    const other=new PaperPrintPdfService({...fakeBackend().backend,async readResultPrintList(){
      const {parseResultPrintList}=await import('../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead.ts');
      return parseResultPrintList(JSON.stringify({result:'OK',print_list:[scoreItem({pri_no:1})]}),STUDENT);}},pc.executor,resolve(base,'i1-other','print-pdfs'),now);
    expect(await other.get({source:'result',testingNo:TESTING,studentKey:STUDENT})).toMatchObject({status:'unavailable',reasonCode:'paper_print_source_unverified'});
    expect(pc.calls).toHaveLength(2);
  });
});

// ---- printPdf of academy_clinic_paper_save -----------------------------------------------------------
const clinicPrintList=()=>[scoreItem({p_no:Number(PAPER),pri_no:Number(STUDENT),paper_type:'similar',exam_list:[{exam_no:1,lecture_key:5001,exam_size_list:'[]'}]}),
  scoreItem({p_no:Number(PAPER)+1,pri_no:Number(STUDENT),paper_type:'advance',exam_list:[{exam_no:2,lecture_key:5002,exam_size_list:'[]'}]})];
async function clinicHarness(printer:(list:readonly unknown[],keyNo:string,ledger:string)=>Promise<any>){
  const dir=await mkdtemp(join(tmpdir(),'pc-paper-clinic-'));await chmod(dir,0o700);
  const ledger=join(dir,'ledger.jsonl'),lms=fixtureLms(),seen:{list:readonly unknown[];keyNo:string}[]=[];
  lms.state.writeAnswer=()=>JSON.stringify({result:'OK',print_list:clinicPrintList()});
  let n=0;
  const service=new ClinicPaperService(fixturePort(lms),ledger,{now:()=>Date.parse('2040-01-16T09:00:00.000Z'),newId:()=>'id-'+(++n),expectedWireHashes:pageHashes(),
    printer:{async printClinicList(list,keyNo){seen.push({list,keyNo});return printer(list,keyNo,ledger);}}});
  const review:any=await service.review({kind:'clinic',studentKey:STUDENT,testingNo:TESTING,clinicMode:'multiply',createSimilar:true});
  expect(review.status).toBe('reviewed');
  const approve=(extra:Record<string,unknown>={})=>({reviewToken:review.reviewToken,userApproved:true,change:review.change,...extra});
  const records=()=>readFileSync(ledger,'utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l));
  return {dir,ledger,lms,service,seen,approve,records,cleanup:()=>rm(dir,{recursive:true,force:true})};
}

describe('printPdf of academy_clinic_paper_save',()=>{
  test('clinic_save_print_pdf_prints_the_write_answer_after_the_persisted_record',async()=>{
    let atPrint:{lock:boolean;last:string}|null=null;
    const h=await clinicHarness(async(_list,_key,ledger)=>{
      const records=readFileSync(ledger,'utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l)).filter((r:any)=>r.kind==='attempt');
      atPrint={lock:existsSync(ledger+'.lock'),last:records.at(-1).step+':'+records.at(-1).state};
      return {status:'rendered',pdfs:[{paperType:'similar',pageCount:2,outputPath:'/invented/a.pdf',sha256:'c'.repeat(64)}],pcCleanup:'done'};
    });
    try{
      const out:any=await h.service.save(h.approve({printPdf:true}));
      expect(out).toMatchObject({status:'saved',academyWrites:1,paperKeys:[PAPER,String(Number(PAPER)+1)],pdf:{status:'rendered',pdfs:[{paperType:'similar'}]}});
      // After the reconciled-persisted record, outside the ledger lock; the list is the write answer's, in the same call.
      expect(atPrint).toEqual({lock:false,last:'reconciled:persisted'});
      expect(h.seen).toEqual([{list:clinicPrintList(),keyNo:PAPER}]);
      expect(h.lms.writes()).toHaveLength(1);
      // The list is not stored: the ledger holds hashes and keys only.
      expect(readFileSync(h.ledger,'utf8')).not.toContain('Invented Student');
      // Without printPdf nothing is printed and the answer has no pdf member.
      const again=await clinicHarness(async()=>{throw Error('must_not_print');});
      try{
        const plain:any=await again.service.save(again.approve());
        expect(plain.status).toBe('saved');expect('pdf' in plain).toBe(false);expect(again.seen).toEqual([]);
      }finally{await again.cleanup();}
    }finally{await h.cleanup();}
  });

  test('clinic_save_print_pdf_error_only_adds_a_pdf_member',async()=>{
    const h=await clinicHarness(async()=>{throw new PcExecutorError('pc_unreachable');});
    try{
      const out:any=await h.service.save(h.approve({printPdf:true}));
      expect(out).toMatchObject({status:'saved',academyWrites:1,pdf:{status:'unavailable',reasonCode:'pc_unreachable',pdfs:[]}});
      expect(out.pdf.nextStep).toContain('academy_paper_print_pdf_get, source paper and each paperKey');
      expect(h.records().filter((r:any)=>r.kind==='attempt').at(-1)).toMatchObject({step:'reconciled',state:'persisted'});
    }finally{await h.cleanup();}
    const busy=await clinicHarness(async()=>({status:'unavailable',reasonCode:'pc_busy',pdfs:[]}));
    try{
      expect(await busy.service.save(busy.approve({printPdf:true}))).toMatchObject({status:'saved',pdf:{status:'unavailable',reasonCode:'pc_busy'}});
    }finally{await busy.cleanup();}
    // A save without a printer: the save stands and the PDF part says why.
    const dir=await mkdtemp(join(tmpdir(),'pc-paper-clinic-'));
    try{
      const lms=fixtureLms();
      const service=new ClinicPaperService(fixturePort(lms),join(dir,'ledger.jsonl'),{expectedWireHashes:pageHashes()});
      const review:any=await service.review({kind:'clinic',studentKey:STUDENT,testingNo:TESTING,clinicMode:'multiply',createSimilar:true});
      expect(await service.save({reviewToken:review.reviewToken,userApproved:true,change:review.change,printPdf:true})).toMatchObject({status:'saved',pdf:{status:'unavailable',reasonCode:'pc_executor_not_configured'}});
    }finally{await rm(dir,{recursive:true,force:true});}
  });

  test('clinic_save_unverified_or_refused_does_not_print',async()=>{
    const h=await clinicHarness(async()=>{throw Error('must_not_print');});
    try{
      h.lms.state.scoreAnswer=()=>JSON.stringify({result:'FAIL'});
      const out:any=await h.service.save(h.approve({printPdf:true}));
      expect(out).toMatchObject({status:'unverified',code:'clinic_readback_unverified'});
      expect('pdf' in out).toBe(false);
      expect(h.seen).toEqual([]);
      expect(await h.service.save({...h.approve({printPdf:true}),userApproved:false})).toMatchObject({status:'refused',code:'user_approval_required'});
      expect(await h.service.save(h.approve({printPdf:'yes'}))).toMatchObject({status:'refused'});
      expect(h.seen).toEqual([]);
    }finally{await h.cleanup();}
  });
});

// ---- the privacy check I6 of ./check (specs/academy-mcp/pc-paper-executor.md R18) ------------------------
const repo=resolve(import.meta.dir,'../../../../..');
const scan=(roots:string[],roster?:string)=>spawnSync('bash',[resolve(repo,'check')],{encoding:'utf8',
  env:{PATH:process.env.PATH??'/usr/bin:/bin',CHECK_I6_ROOTS:roots.join(':'),...(roster?{CHECK_STUDYQ_ROSTER:roster}:{})}});
describe('the privacy check I6',()=>{
  test('i6_check_passes_on_specs_and_the_server_sources',()=>{
    const out=scan(['specs','modules/academy-workbench/tools/academy-mcp/src','apps/academy/dc-gang-a/src']);
    expect({status:out.status,stdout:out.stdout.trim(),stderr:out.stderr}).toEqual({status:0,stdout:'i6: ok',stderr:''});
  });
  test('i6_check_fails_on_secret_values_without_printing_them',async()=>{
    const dir=await mkdtemp(join(tmpdir(),'i6-'));
    try{
      const fixtures:Record<string,string>={'key.txt':'-----BEGIN '+'RSA PRIVATE '+'KEY-----\nabc\n','aws.txt':'id = '+'AKIA'+'ABCDEFGHIJKLMNOP\n','gh.txt':'t '+'ghp_'+'a'.repeat(36)+'\n',
        'sk.txt':'k '+'sk-'+'b'.repeat(24)+'\n','assign.json':'{"pass'+'word": "hunter2'+'hunter2"}\n','bearer.txt':'Authorization: '+'Bearer '+'c'.repeat(30)+'\n'};
      for(const [name,text] of Object.entries(fixtures)){
        const sub=join(dir,name.replace('.','_'));await mkdir(sub);await writeFile(join(sub,name),text);
        const out=scan([sub]);
        expect({name,status:out.status}).toEqual({name,status:5});
        expect(out.stderr).toContain(name);
        expect(out.stderr+out.stdout).not.toContain('hunter2');expect(out.stderr+out.stdout).not.toContain('ABCDEFGHIJKLMNOP');
      }
    }finally{await rm(dir,{recursive:true,force:true});}
  });
  test('i6_check_skips_private_trees_and_roster_names_fail_without_being_printed',async()=>{
    const dir=await mkdtemp(join(tmpdir(),'i6-'));
    try{
      await mkdir(join(dir,'clean','private'),{recursive:true});
      await writeFile(join(dir,'clean','a.md'),'a hash '+'d'.repeat(64)+'\n');
      await writeFile(join(dir,'clean','private','b.txt'),'password'+' = "'+'x'.repeat(12)+'"\n');
      expect(scan([join(dir,'clean')]).status).toBe(0);
      const name='Invented'+'Pupil'+'Name';
      await mkdir(join(dir,'fixture'));await writeFile(join(dir,'fixture','f.json'),'{"student": "'+name+'"}\n');
      await writeFile(join(dir,'roster.txt'),'\n'+name+'\n\n');
      const out=scan([join(dir,'fixture')],join(dir,'roster.txt'));
      expect(out.status).toBe(5);expect(out.stderr).toContain('f.json');expect(out.stderr+out.stdout).not.toContain(name);
      expect(scan([join(dir,'fixture')]).status).toBe(0);
    }finally{await rm(dir,{recursive:true,force:true});}
  });
  test('i6_check_wiring',()=>{
    const text=readFileSync(resolve(repo,'check'),'utf8');
    expect(text).toContain('i6_scan specs modules/academy-workbench/tools/academy-mcp/src apps/academy/dc-gang-a/src');
    expect(text).not.toMatch(/studyq-linux|PYMUPDF/);
  });
});
