import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {chmod,mkdir,mkdtemp,readFile,readdir,realpath,rm,writeFile} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {PC_REPORT_TYPES,PcExecutorError,cp949JsonText,decodeCp949,encodeCp949,pcExecutorConfiguration,pcReportExecutor,pdfProducer,processStartTime,reportInfoText,
  type PcExecutorOptions} from '../src/pc-executor.ts';
import {sshOptionArgv} from '../src/ssh-argv.ts';

// src/pc-executor.ts (specs/academy-mcp/pc-executor.md) against fake ssh and scp programs that keep an invented
// "PC" report directory on this host: no network, no PC, invented values only.
const TARGET='tester@pc.invalid',EXE_DIR='C:\\Program Files (x86)\\otwoline\\studyq\\report';
const HARU='%PDF-1.3\n%invented\n1 0 obj\n<< /Producer (Haru\\040Free\\040PDF\\040Library 2.0.8) /Title (Invented) >>\nendobj\n%%EOF\n';
let base:string,counter=0;
beforeAll(async()=>{base=await realpath(await mkdtemp(resolve(tmpdir(),'pc-executor-')));});
afterAll(async()=>{await rm(base,{recursive:true,force:true});});

/** The fake ssh: decodes the one -EncodedCommand, recognises the step by its fixed program and acts on `<state>/pc`. */
const fakeSsh=(state:string)=>`#!${process.execPath}
const fs=require('fs'),path=require('path');
const state=${JSON.stringify(state)},pc=path.join(state,'pc'),argv=process.argv.slice(2);
const scenario=JSON.parse(fs.readFileSync(path.join(state,'scenario.json'),'utf8'));
const word=argv[argv.length-1]||'',m=/^powershell -NoProfile -NonInteractive -EncodedCommand ([A-Za-z0-9+/=]+)$/.exec(word);
const script=m?Buffer.from(m[1],'base64').toString('utf16le'):'';
const step=script.includes('Start-Process')?'run':script.includes('Get-ChildItem')?'list':script.includes('ToBase64String')?'collect':script.includes('$f=Join-Path')?'cleanup':'unknown';
fs.appendFileSync(path.join(state,'log.jsonl'),JSON.stringify({tool:'ssh',argv,step,script})+'\\n');
const result=path.join(pc,'report_result.info'),info=path.join(pc,'report.info');
const fail=(scenario.sshFail||{})[step];
if(fail){process.stderr.write(fail.stderr||'');process.exit(fail.code);}
else if((scenario.hang||[]).includes(step)){setTimeout(()=>{},1e8);}
else if(step==='list'){
  if(fs.existsSync(result))fs.unlinkSync(result);
  process.stdout.write(fs.readdirSync(pc).filter(n=>/\\.pdf$/i.test(n)).map(n=>n+'\\r\\n').join(''));
}else if(step==='run'){
  if(!script.includes("Start-Process -FilePath '"+${JSON.stringify(EXE_DIR)}+"\\\\StudyQReport.exe' -WorkingDirectory '"+${JSON.stringify(EXE_DIR)}+"' -PassThru"))process.exit(3);
  fs.copyFileSync(info,path.join(state,'report.info.seen.'+fs.readdirSync(state).filter(n=>n.startsWith('report.info.seen.')).length));
  if(!scenario.keepInfo)fs.unlinkSync(info);
  const n=Number(fs.existsSync(path.join(state,'counter'))?fs.readFileSync(path.join(state,'counter'),'utf8'):'0')+1;
  fs.writeFileSync(path.join(state,'counter'),String(n));
  const name=String(20261128000000000+n)+'.pdf';
  for(const file of scenario.made||[{name,content:${JSON.stringify(HARU)}}])fs.writeFileSync(path.join(pc,file.name),file.content);
  const answer=scenario.result!==undefined?scenario.result:{result:'ok',pdf_list:[{pdf_type:'report',pdf_file:name,pri_no:0,page_cnt:scenario.pages||8}]};
  if(answer!==null)fs.writeFileSync(result,typeof answer==='string'?answer:JSON.stringify(answer));
  if(scenario.timeoutWordOnly){process.stdout.write('pc-executor-timeout\\r\\n');process.exit(1);}
  process.exit(scenario.runExit||0);
}else if(step==='collect'){
  if(script.includes("if(Test-Path -LiteralPath $i){Remove-Item -LiteralPath $i -Force};if(Test-Path -LiteralPath $r){[Convert]")&&fs.existsSync(info))fs.unlinkSync(info);
  process.stdout.write(fs.existsSync(result)?fs.readFileSync(result).toString('base64')+'\\r\\n':'-\\r\\n');
}else if(step==='cleanup'){
  const name=/\\$f=Join-Path \\$d '([^']*)'/.exec(script)[1];
  for(const file of [path.join(pc,name),result,info])if(fs.existsSync(file))fs.unlinkSync(file);
}else process.exit(9);
`;
/** The fake scp: copies between the current directory and `<state>/pc` by the base name of the remote path. */
const fakeScp=(state:string)=>`#!${process.execPath}
const fs=require('fs'),path=require('path');
const state=${JSON.stringify(state)},pc=path.join(state,'pc'),argv=process.argv.slice(2);
const scenario=JSON.parse(fs.readFileSync(path.join(state,'scenario.json'),'utf8'));
const [from,to]=argv.slice(-2),remote=/^${TARGET.replace('.','\\.')}:AppData\\/Roaming\\/otwoline\\/studyq\\/report\\/([^/]+)$/;
fs.appendFileSync(path.join(state,'log.jsonl'),JSON.stringify({tool:'scp',argv,cwd:process.cwd()})+'\\n');
const fail=scenario.scpFail;
if(fail&&(fail.on==='both'||(fail.on==='download')===remote.test(from))){process.stderr.write(fail.stderr||'');process.exit(fail.code);}
if(remote.test(to))fs.copyFileSync(path.resolve(from),path.join(pc,remote.exec(to)[1]));
else{
  const source=path.join(pc,remote.exec(from)[1]);
  if(!fs.existsSync(source)){process.stderr.write('scp: no such file\\n');process.exit(1);}
  if(scenario.raceOutput)fs.writeFileSync(scenario.raceOutput,'earlier');
  fs.copyFileSync(source,path.resolve(to));
}
`;
type Scenario={timeoutWordOnly?:boolean;result?:unknown;made?:{name:string;content:string}[];pages?:number;runExit?:number;keepInfo?:boolean;hang?:string[];
  sshFail?:Record<string,{code:number;stderr?:string}>;scpFail?:{on:'upload'|'download'|'both';code:number;stderr?:string};raceOutput?:string};
async function fixture(scenario:Scenario={},pcFiles:Record<string,string>={},options:PcExecutorOptions={}){
  const state=resolve(base,'s'+(++counter)),pc=resolve(state,'pc'),out=resolve(state,'out'),lockDir=resolve(state,'official-reports');
  await mkdir(pc,{recursive:true});await mkdir(out,{recursive:true});
  for(const [name,content] of Object.entries(pcFiles))await writeFile(resolve(pc,name),content);
  await writeFile(resolve(state,'scenario.json'),JSON.stringify(scenario));
  const ssh=resolve(state,'ssh'),scp=resolve(state,'scp');
  await writeFile(ssh,fakeSsh(state));await writeFile(scp,fakeScp(state));await chmod(ssh,0o755);await chmod(scp,0o755);
  const identityFile=resolve(state,'id_invented'),knownHostsFile=resolve(state,'known_hosts_invented');
  await writeFile(identityFile,'invented\n');await writeFile(knownHostsFile,'invented\n');
  const config={target:TARGET,identityFile,knownHostsFile,exeDir:EXE_DIR,lockDir};
  const executor=pcReportExecutor(config,{sshPath:ssh,scpPath:scp,afterTimeoutDelayMs:10,lockPollMs:20,...options});
  const log=async()=>{try{return (await readFile(resolve(state,'log.jsonl'),'utf8')).trim().split('\n').filter(Boolean).map(l=>JSON.parse(l));}catch{return [];}};
  const seen=async(n=0)=>readFile(resolve(state,'report.info.seen.'+n));
  const scenarioSet=(next:Scenario)=>writeFile(resolve(state,'scenario.json'),JSON.stringify(next));
  return {state,pc,out,lockDir,config,executor,log,seen,scenarioSet,ssh,scp,identityFile,knownHostsFile};
}
const BLOCK={block1:[{'1':'Invented book','4':'Invented Student'}],note:'가 Ｉ-1 \\r\\n "quoted"'};
const render=(f:Awaited<ReturnType<typeof fixture>>,name='r.pdf',over:Partial<Parameters<typeof f.executor.render>[0]>={})=>
  f.executor.render({reportType:'course_book',keys:{},blockData:BLOCK,outputPath:resolve(f.out,name),...over});
const code=async(promise:Promise<unknown>)=>{try{await promise;return 'resolved';}catch(error){return {code:(error as Error).message,pcCleanup:(error as PcExecutorError).pcCleanup};}};
const sha=(bytes:Buffer|string)=>createHash('sha256').update(bytes).digest('hex');

describe('report.info',()=>{
  test('pc_executor_report_info_members_are_numbers_and_block_data_verbatim',()=>{
    expect(PC_REPORT_TYPES).toEqual({course_book:22,cism:200,diag:20,new_diag:19,formative:17,ganga:16,na_score:18,smart_book:21});
    const raw={block1:[{'1':'Invented',x:1.5,y:null,z:[true]}]};
    expect(reportInfoText('course_book',{},raw)).toBe('{"testing_no":0,"book_code":0,"p_no":0,"pri_no":0,"cls_no":0,"apply_code":0,"report_type":22,"block_data":'+JSON.stringify(raw)+'}');
    expect(JSON.parse(reportInfoText('cism',{},raw))).toEqual({testing_no:0,book_code:0,p_no:0,pri_no:0,cls_no:0,apply_code:0,report_type:200,block_data:raw});
    expect(JSON.parse(reportInfoText('diag',{testingNo:'901',studentKey:'770011',classNo:'0'},raw))).toMatchObject({testing_no:901,pri_no:770011,cls_no:0,p_no:0,book_code:0,apply_code:0,report_type:20});
    expect(JSON.parse(reportInfoText('formative',{testingNo:'902',studentKey:'770011',classNo:'560077'},raw))).toMatchObject({testing_no:902,pri_no:770011,cls_no:560077,report_type:17});
    // apply_code and book_code are numbers when the code is all digits, else 0; leading zeros are not JSON.
    expect(JSON.parse(reportInfoText('new_diag',{paperNo:'4401',applyCode:'0077'},raw))).toMatchObject({p_no:4401,apply_code:77,pri_no:0,testing_no:0,report_type:19});
    expect(JSON.parse(reportInfoText('new_diag',{paperNo:'4401',applyCode:'APP-7'},raw))).toMatchObject({apply_code:0});
    expect(JSON.parse(reportInfoText('smart_book',{bookCode:'2040',studentKey:'770011'},raw))).toMatchObject({book_code:2040,pri_no:770011,report_type:21});
    expect(JSON.parse(reportInfoText('smart_book',{bookCode:'SB-2040.inv_01',studentKey:'770011'},raw))).toMatchObject({book_code:0});
    expect(reportInfoText('new_diag',{paperNo:'4401',applyCode:'123456789012345678901'},raw)).toContain('"apply_code":123456789012345678901,');
    // Every member but block_data is a number.
    for(const type of Object.keys(PC_REPORT_TYPES) as (keyof typeof PC_REPORT_TYPES)[]){
      const info=JSON.parse(reportInfoText(type,{testingNo:'1',studentKey:'2',classNo:'3',paperNo:'4',applyCode:'5',bookCode:'6'},raw));
      expect(Object.keys(info)).toEqual(['testing_no','book_code','p_no','pri_no','cls_no','apply_code','report_type','block_data']);
      expect(Object.entries(info).filter(([k])=>k!=='block_data').every(([,v])=>typeof v==='number')).toBe(true);
    }
    expect(()=>reportInfoText('course_book',{},[] as never)).toThrow('pc_executor_failed');
  });

  test('pc_executor_report_info_is_cp949',()=>{
    const text='{"a":"가갂 Ｉ-1 Ｓ-4 Ｍ-2 하중상 점 % ~"}';
    const bytes=encodeCp949(text);
    expect([...bytes.subarray(6,12)]).toEqual([0xb0,0xa1,0x81,0x41,0x20,0xa3]);
    expect(decodeCp949(bytes)).toBe(text);
    for(const outside of ['\u{1F600}','\u00e9\u0301','\ufffd','\u4e02'])expect(()=>encodeCp949('x'+outside)).toThrow('pc_input_unencodable');
  });

  test('pc_executor_character_outside_cp949_is_escaped',async()=>{
    // The helper: each character outside CP949 as its lowercase JSON escape, a code point above U+FFFF as two; the rest unchanged.
    expect(cp949JsonText('"가 Ｉ-1 ~ 75\u207a%"')).toEqual({text:'"가 Ｉ-1 ~ 75\\u207a%"',escaped:1});
    expect(cp949JsonText('"\u{1F600}\u4e02\ufffd\u0080x"')).toEqual({text:'"\\ud83d\\ude00\\u4e02\\ufffd\\u0080x"',escaped:4});
    expect(cp949JsonText(JSON.stringify({a:'가 Ｉ-1'}))).toEqual({text:JSON.stringify({a:'가 Ｉ-1'}),escaped:0});
    // A lone surrogate is escaped by JSON.stringify already and stays as it writes it.
    expect(JSON.stringify('\ud800')).toBe('"\\ud800"');
    expect(cp949JsonText(JSON.stringify({a:'\ud800'}))).toEqual({text:'{"a":"\\ud800"}',escaped:0});
    // encodeCp949 still refuses raw text outside CP949 for any other caller.
    expect(()=>encodeCp949('75\u207a')).toThrow('pc_input_unencodable');
    // A block data wholly inside CP949 gives the report.info it always gave, byte for byte.
    const head='{"testing_no":0,"book_code":0,"p_no":0,"pri_no":0,"cls_no":0,"apply_code":0,"report_type":22,"block_data":';
    expect(encodeCp949(reportInfoText('course_book',{},BLOCK)).equals(encodeCp949(head+JSON.stringify(BLOCK)+'}'))).toBe(true);
    // Every input: JSON.parse of report.info gives the server's answer back.
    const outside={block3:[{'1':'Invented Group','4':'75\u207a%'},{'1':'Invented \u207a\u{1F600}','\u4e02 key':'\ud800 lone','4':'가'}],n:1.5,z:[null,true]};
    for(const raw of [outside,BLOCK,{a:'\u0080\u00e9\u0301\ufffd'}])
      expect(JSON.parse(reportInfoText('cism',{},raw)).block_data).toEqual(raw);
    // One call: the exe reads the escapes in CP949, the answer counts them and holds no character value.
    const f=await fixture({pages:1});
    const answer=await render(f,'escaped.pdf',{reportType:'cism',blockData:outside});
    expect(answer).toMatchObject({status:'rendered',pages:1,pcCleanup:'done',escapedCharacters:4});
    const info=decodeCp949(await f.seen());
    expect(info).toBe('{"testing_no":0,"book_code":0,"p_no":0,"pri_no":0,"cls_no":0,"apply_code":0,"report_type":200,"block_data":'
      +'{"block3":[{"1":"Invented Group","4":"75\\u207a%"},{"1":"Invented \\u207a\\ud83d\\ude00","4":"가","\\u4e02 key":"\\ud800 lone"}],"n":1.5,"z":[null,true]}}');
    expect(JSON.parse(info).block_data).toEqual(outside);
    expect(JSON.stringify(answer)).not.toMatch(/207a|\u207a|Invented/);
    expect(existsSync(resolve(f.lockDir,'pc-executor.lock'))).toBe(false);
  });

  test('pc_executor_pdf_producer_decodes_string_escapes',()=>{
    expect(pdfProducer(Buffer.from(HARU,'latin1'))).toBe('Haru Free PDF Library 2.0.8');
    expect(pdfProducer(Buffer.from('<< /Producer (Haru Free PDF Library \\(x\\)\\\nend) >>','latin1'))).toBe('Haru Free PDF Library (x)end');
    expect(pdfProducer(Buffer.from('<< /Producer <48617275204672656520504446204C696272617279> >>','latin1'))).toBe('Haru Free PDF Library');
    expect(pdfProducer(Buffer.from('<< /Creator (x) >>','latin1'))).toBeNull();
  });
});

describe('the fixed argv',()=>{
  test('pc_executor_ssh_and_scp_take_fixed_words_only',async()=>{
    const f=await fixture();
    const answer=await render(f,'argv.pdf');
    expect(answer.status).toBe('rendered');
    const words=['-i',f.identityFile,'-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','UserKnownHostsFile='+f.knownHostsFile,'-o','IdentitiesOnly=yes','-o','ConnectTimeout=10','-F','none'];
    const log=await f.log();
    expect(log.map(e=>e.tool+':'+(e.step??''))).toEqual(['ssh:list','scp:','ssh:run','ssh:collect','scp:','ssh:cleanup']);
    for(const entry of log){
      expect(entry.argv.slice(0,words.length)).toEqual(words);
      expect(entry.argv).toHaveLength(words.length+2);
    }
    for(const entry of log.filter(e=>e.tool==='ssh')){
      expect(entry.argv[words.length]).toBe(TARGET);
      expect(entry.argv[words.length+1]).toMatch(/^powershell -NoProfile -NonInteractive -EncodedCommand [A-Za-z0-9+/]+=*$/);
    }
    const scps=log.filter(e=>e.tool==='scp');
    expect(scps[0].argv.slice(-2)).toEqual(['./report.info',TARGET+':AppData/Roaming/otwoline/studyq/report/report.info']);
    expect(scps[1].argv.at(-2)).toMatch(new RegExp('^'+TARGET.replace('.','\\.')+':AppData/Roaming/otwoline/studyq/report/\\d{17}\\.pdf$'));
    expect(scps[1].argv.at(-1)).toMatch(/^\.\/\.pc-executor-[a-f0-9]{16}\.tmp$/);
    // The PDF is copied beside the output path: scp runs in its directory, so no path of the caller is a word.
    expect(scps[1].cwd).toBe(f.out);
    expect(JSON.stringify(log.map(e=>e.argv))).not.toContain('argv.pdf');
    // The exe is started and waited for by its process id; only that process is stopped, never an image name.
    const run=log.find(e=>e.step==='run').script as string;
    expect(run).toContain("$p=Start-Process -FilePath '"+EXE_DIR+"\\StudyQReport.exe' -WorkingDirectory '"+EXE_DIR+"' -PassThru;");
    expect(run).toContain("if(-not $p.WaitForExit(120000)){Stop-Process -Id $p.Id -Force;'pc-executor-timeout';exit 124};exit 0");
    expect(run).not.toMatch(/taskkill|-Name|Get-Process/i);
    expect(log.map(e=>e.script??'').join('\n')).toContain("Join-Path $env:USERPROFILE 'AppData\\Roaming\\otwoline\\studyq\\report'");
    expect(sshOptionArgv({connectTimeoutSeconds:10,identityFile:'/i',knownHostsFile:'/k',noConfig:true})).toEqual(['-i','/i','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','UserKnownHostsFile=/k','-o','IdentitiesOnly=yes','-o','ConnectTimeout=10','-F','none']);
  });

  test('native_pool_ssh_argv_is_byte_identical',async()=>{
    // The argv the native pool bridge has always sent, now built by the shared builder (zod loads here, so a host
    // without the module's dependencies fails only this test).
    const {nativePoolSshArgv}=await import('../src/native-pool-backend.ts');
    expect(nativePoolSshArgv('invented-host','remote words')).toEqual(['-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','ConnectTimeout=8','invented-host','remote words']);
    expect(sshOptionArgv({connectTimeoutSeconds:8})).toEqual(['-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','ConnectTimeout=8']);
    expect(()=>sshOptionArgv({connectTimeoutSeconds:0})).toThrow('ssh_arguments_invalid');
  });

  test('pc_executor_launch_option',async()=>{
    const f=await fixture();
    const lock={lockDir:f.lockDir};
    expect((await pcExecutorConfiguration(['--other'],lock)).executor).toBeNull();
    const ok=await pcExecutorConfiguration(['--a','--pc-executor',TARGET,f.identityFile,f.knownHostsFile,EXE_DIR,'--b'],lock);
    expect(ok.remainingArgs).toEqual(['--a','--b']);expect(ok.executor).not.toBeNull();
    for(const bad of [
      ['--pc-executor',TARGET,f.identityFile,f.knownHostsFile],
      ['--pc-executor',TARGET,'relative/id',f.knownHostsFile,EXE_DIR],
      ['--pc-executor',TARGET,f.identityFile,resolve(f.state,'missing'),EXE_DIR],
      ['--pc-executor',TARGET,f.identityFile,f.state,EXE_DIR],
      ['--pc-executor','-oProxyCommand=x',f.identityFile,f.knownHostsFile,EXE_DIR],
      ['--pc-executor','nohost',f.identityFile,f.knownHostsFile,EXE_DIR],
      ['--pc-executor',TARGET,f.identityFile,f.knownHostsFile,"C:\\x';Remove-Item C:\\"],
      ['--pc-executor',TARGET,f.identityFile,f.knownHostsFile,'C:/Program Files'],
      ['--pc-executor',TARGET,f.identityFile,f.knownHostsFile,'\\\\server\\share'],
      ['--pc-executor',TARGET,f.identityFile,f.knownHostsFile,EXE_DIR,'--pc-executor',TARGET,f.identityFile,f.knownHostsFile,EXE_DIR],
    ])await expect(pcExecutorConfiguration(bad,lock)).rejects.toThrow('pc_executor_arguments_invalid');
  });
});

describe('one call',()=>{
  test('pc_executor_renders_one_pdf_and_leaves_the_pc_as_it_was',async()=>{
    const old='20261127000000001.pdf';
    const f=await fixture({pages:8},{[old]:'%PDF-old','report_result.info':'{"result":"stale"}'});
    const out=resolve(f.out,'book.pdf');
    const answer=await render(f,'book.pdf',{blockData:BLOCK});
    const bytes=await readFile(out);
    expect(answer).toEqual({status:'rendered',pages:8,sha256:sha(bytes),outputPath:out,pcCleanup:'done',escapedCharacters:0});
    expect(bytes.toString('latin1')).toBe(HARU);
    // report.info as the executor read it: CP949, the members, the block data verbatim.
    const info=await f.seen();
    expect(JSON.parse(decodeCp949(info))).toEqual({testing_no:0,book_code:0,p_no:0,pri_no:0,cls_no:0,apply_code:0,report_type:22,block_data:BLOCK});
    expect(info.includes(Buffer.from([0xb0,0xa1]))).toBe(true);
    // The PC holds what it held before the call, the stale result removed; the local temporary file is gone.
    expect((await readdir(f.pc)).sort()).toEqual([old]);
    expect(await readdir(f.out)).toEqual(['book.pdf']);
    expect(existsSync(resolve(f.lockDir,'pc-executor.lock'))).toBe(false);
    // A report.info the exe left behind is deleted after the run.
    const g=await fixture({keepInfo:true});
    expect((await render(g)).status).toBe('rendered');
    expect(await readdir(g.pc)).toEqual([]);
  });

  test('pc_executor_refuses_bad_results_and_deletes_only_a_qualified_new_pdf',async()=>{
    const pdf=(name:string,extra:Record<string,unknown>={})=>({result:'ok',pdf_list:[{pdf_type:'report',pdf_file:name,pri_no:0,page_cnt:3,...extra}]});
    const N='20261128123456789.pdf',OLD='20261127000000001.pdf';
    const cases:[string,Scenario,Record<string,string>,string,string,string[]][]=[
      ['result not ok',{result:{...pdf(N),result:'fail'},made:[{name:N,content:HARU}]},{},'pc_executor_failed','done',[]],
      ['page count 0',{result:pdf(N,{page_cnt:0}),made:[{name:N,content:HARU}]},{},'pc_executor_failed','done',[]],
      ['page count text',{result:pdf(N,{page_cnt:'8'}),made:[{name:N,content:HARU}]},{},'pc_executor_failed','done',[]],
      ['two entries',{result:{result:'ok',pdf_list:[pdf(N).pdf_list[0],pdf('20261128123456780.pdf').pdf_list[0]]},made:[{name:N,content:HARU}]},{},'pc_output_invalid','incomplete',[N]],
      ['no entry',{result:{result:'ok',pdf_list:[]},made:[]},{},'pc_output_invalid','incomplete',[]],
      ['another pdf type',{result:pdf(N,{pdf_type:'answer'}),made:[{name:N,content:HARU}]},{},'pc_output_invalid','done',[]],
      ['not a pdf',{result:pdf(N),made:[{name:N,content:'<html>'}]},{},'pc_output_invalid','done',[]],
      ['another producer',{result:pdf(N),made:[{name:N,content:'%PDF-1.4\n<< /Producer (Invented Writer) >>'}]},{},'pc_output_invalid','done',[]],
      ['no result',{result:null,made:[]},{},'pc_executor_failed','incomplete',[]],
      ['result not json',{result:'not json',made:[]},{},'pc_executor_failed','incomplete',[]],
      // A name that fails either check is never deleted and never a word of any argv.
      ['a name of another form',{result:pdf('..\\Invented.pdf'),made:[]},{},'pc_executor_failed','incomplete',[]],
      ['a short name',{result:pdf('2026112812345678.pdf'),made:[{name:'2026112812345678.pdf',content:HARU}]},{},'pc_executor_failed','incomplete',['2026112812345678.pdf']],
      ['a name listed before the run',{result:pdf(OLD),made:[]},{[OLD]:HARU},'pc_executor_failed','incomplete',[OLD]],
    ];
    for(const [label,scenario,files,expected,cleanup,left] of cases){
      const f=await fixture(scenario,files);
      expect({label,answer:await code(render(f,'bad.pdf'))}).toEqual({label,answer:{code:expected,pcCleanup:cleanup}});
      expect({label,pc:(await readdir(f.pc)).filter(n=>n!=='report_result.info').sort()}).toEqual({label,pc:left.sort()});
      const log=await f.log();
      // Cleanup is sent only for a qualified new name, and no refused name is in any argv.
      expect({label,cleanup:log.filter(e=>e.step==='cleanup').length}).toEqual({label,cleanup:cleanup==='done'?1:0});
      for(const refused of ['..\\Invented.pdf','2026112812345678.pdf',OLD])
        expect({label,refused,inArgv:JSON.stringify(log.map(e=>[e.argv,e.script]).filter(([,s])=>!String(s).includes('Get-ChildItem'))).includes(refused.replace('\\','\\\\'))}).toEqual({label,refused,inArgv:false});
      expect(await readdir(f.out)).toEqual([]);
    }
  });

  test('pc_executor_timeout_deletes_only_the_pdf_the_result_names',async()=>{
    const N='20261128123456789.pdf',SECOND='20261128123456790.pdf';
    // The exe ran past its 120 s: only that process was stopped (exit 124). A second new PDF appeared meanwhile.
    const f=await fixture({runExit:124,keepInfo:true,result:{result:'ok',pdf_list:[{pdf_type:'report',pdf_file:N,pri_no:0,page_cnt:2}]},
      made:[{name:N,content:HARU},{name:SECOND,content:HARU}]});
    expect(await code(render(f,'late.pdf'))).toEqual({code:'pc_executor_timeout',pcCleanup:'done'});
    expect(await readdir(f.pc)).toEqual([SECOND]);
    const log=await f.log();
    expect(log.map(e=>e.step??'scp')).toEqual(['list','scp','run','collect','cleanup']);
    // The read after the timeout does not delete report.info by itself.
    expect(log.find(e=>e.step==='collect').script).not.toContain('Remove-Item -LiteralPath $i -Force};if(Test-Path -LiteralPath $r){[Convert]');
    expect(log.filter(e=>e.step==='cleanup').map(e=>/\$f=Join-Path \$d '([^']*)'/.exec(e.script)![1])).toEqual([N]);
    expect(await readdir(f.out)).toEqual([]);
    // Without a result that names a qualified PDF the PC is left as it is.
    const g=await fixture({runExit:124,keepInfo:true,result:null,made:[{name:N,content:HARU}]});
    expect(await code(render(g,'late.pdf'))).toEqual({code:'pc_executor_timeout',pcCleanup:'incomplete'});
    expect((await readdir(g.pc)).sort()).toEqual([N,'report.info']);
    expect((await g.log()).filter(e=>e.step==='cleanup')).toEqual([]);
    // A remote shell that does not pass the exit code on: the printed word is the timeout.
    const w=await fixture({timeoutWordOnly:true,keepInfo:true,result:null,made:[]});
    expect(await code(render(w,'word.pdf'))).toEqual({code:'pc_executor_timeout',pcCleanup:'incomplete'});
    // The local ssh of the run stopped by the call's deadline is a timeout too.
    const h=await fixture({hang:['run']},{},{deadlineMs:1500});
    expect((await code(render(h,'hang.pdf'))as any).code).toBe('pc_executor_timeout');
  });

  test('pc_executor_unreachable_and_host_key_mismatch',async()=>{
    const f=await fixture({sshFail:{list:{code:255,stderr:'ssh: connect to host pc.invalid port 22: Connection timed out\r\n'}}});
    expect(await code(render(f))).toEqual({code:'pc_unreachable',pcCleanup:undefined});
    expect((await f.log()).map(e=>e.step)).toEqual(['list']);
    const g=await fixture({sshFail:{list:{code:255,stderr:'@@@@ WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED! @@@@\nHost key verification failed.\r\n'}}});
    expect(await code(render(g))).toEqual({code:'pc_host_key_mismatch',pcCleanup:undefined});
    const h=await fixture({scpFail:{on:'upload',code:1,stderr:'Host key verification failed.\r\nscp: Connection closed\r\n'}});
    expect(await code(render(h))).toEqual({code:'pc_host_key_mismatch',pcCleanup:'incomplete'});
    const i=await fixture({scpFail:{on:'upload',code:1,stderr:'ssh: connect to host pc.invalid port 22: No route to host\r\n'}});
    expect(await code(render(i))).toEqual({code:'pc_unreachable',pcCleanup:'incomplete'});
    // A step that does not answer within its own limit: unreachable; no further step is sent.
    const j=await fixture({hang:['list']},{},{stepTimeoutMs:300});
    expect(await code(render(j))).toEqual({code:'pc_unreachable',pcCleanup:undefined});
    expect((await j.log()).map(e=>e.step)).toEqual(['list']);
    // A remote program that fails is the executor's failure; a failed copy of the PDF still cleans the PC.
    const k=await fixture({sshFail:{list:{code:1,stderr:'Get-ChildItem: invented'}}});
    expect(await code(render(k))).toEqual({code:'pc_executor_failed',pcCleanup:undefined});
    const l=await fixture({scpFail:{on:'download',code:1,stderr:'scp: invented failure'}});
    expect(await code(render(l))).toEqual({code:'pc_executor_failed',pcCleanup:'done'});
    expect(await readdir(l.pc)).toEqual([]);
    expect(await readdir(l.out)).toEqual([]);
  });

  test('pc_executor_output_that_appeared_meanwhile_is_output_exists',async()=>{
    const f=await fixture();
    const out=resolve(f.out,'raced.pdf');
    await f.scenarioSet({raceOutput:out});
    expect(await code(render(f,'raced.pdf'))).toEqual({code:'output_exists',pcCleanup:'done'});
    expect(await readFile(out,'utf8')).toBe('earlier');
    expect(await readdir(f.out)).toEqual(['raced.pdf']);
    expect(await readdir(f.pc)).toEqual([]);
  });
});

describe('the host-wide lock',()=>{
  test('pc_executor_calls_are_serialized_host_wide',async()=>{
    const f=await fixture();
    // A second executor of the same lock directory, as another academy_operations process would hold.
    const other=pcReportExecutor(f.config,{sshPath:f.ssh,scpPath:f.scp,lockPollMs:20});
    const answers=await Promise.all([render(f,'a.pdf'),other.render({reportType:'cism',keys:{},blockData:BLOCK,outputPath:resolve(f.out,'b.pdf')}),render(f,'c.pdf')]);
    expect(answers.map(a=>a.status)).toEqual(['rendered','rendered','rendered']);
    const steps=(await f.log()).map(e=>e.step??'scp');
    expect(steps).toEqual(Array.from({length:3},()=>['list','scp','run','collect','scp','cleanup']).flat());
    expect(new Set(answers.map(a=>a.sha256)).size).toBe(1);
    expect(existsSync(resolve(f.lockDir,'pc-executor.lock'))).toBe(false);
  });

  test('pc_executor_lock_of_a_live_process_waits_and_a_stale_lock_is_taken_over',async()=>{
    const f=await fixture({},{},{deadlineMs:400});
    await mkdir(f.lockDir,{recursive:true});
    const lock=resolve(f.lockDir,'pc-executor.lock'),start=await processStartTime(process.pid);
    expect(start).toMatch(/^\d+$/);
    // A live holder (this process, its own start time, another nonce): the call waits and ends at its deadline.
    await writeFile(lock,JSON.stringify({pid:process.pid,start,nonce:'held'}));
    expect(await code(render(f,'held.pdf'))).toEqual({code:'pc_executor_timeout',pcCleanup:undefined});
    expect(await f.log()).toEqual([]);
    expect(JSON.parse(await readFile(lock,'utf8')).nonce).toBe('held');
    // A holder whose process is gone, or whose pid now names a process started at another time, is stale.
    for(const stale of [{pid:2**22+12345,start:'1',nonce:'gone'},{pid:process.pid,start:'1',nonce:'reused'}]){
      await writeFile(lock,JSON.stringify(stale));
      const g=await fixture({},{},{});
      const executor=pcReportExecutor({...g.config,lockDir:f.lockDir},{sshPath:g.ssh,scpPath:g.scp,lockPollMs:20});
      expect((await executor.render({reportType:'diag',keys:{testingNo:'901',studentKey:'770011',classNo:'0'},blockData:BLOCK,outputPath:resolve(g.out,'x.pdf')})).status).toBe('rendered');
      expect(existsSync(lock)).toBe(false);
      expect((await readdir(f.lockDir)).filter(n=>n.endsWith('.tmp'))).toEqual([]);
    }
  });
});
