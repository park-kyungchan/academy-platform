import {spawn} from 'node:child_process';
import {createHash,randomBytes} from 'node:crypto';
import {chmod,link,mkdir,mkdtemp,open,readFile,rename,rm,stat,unlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname,isAbsolute,join} from 'node:path';
import {sshOptionArgv} from './ssh-argv.ts';

/** The PC executor (specs/academy-mcp/pc-executor.md): one official StudyQ report PDF made by the original
 * `StudyQReport.exe` on the user's Windows PC, reached over SSH with fixed argv only. The data is the Academy
 * server's answer, passed verbatim as `block_data` of `report.info`; the PDF named by `report_result.info` is
 * copied back beside the output path and checked (`%PDF-`, Producer `Haru Free PDF Library`). Calls are
 * serialized host-wide by an exclusive lock file. Nothing on the PC is created or removed but `report.info`,
 * `report_result.info` and the one PDF of the call, and only the `StudyQReport.exe` process the call started
 * is ever stopped. No error holds a student value. */

/** The `report_type` of each report, as the dispatcher `FUN_0043dba0` of `StudyQReport.exe` reads it. */
export const PC_REPORT_TYPES={course_book:22,cism:200,diag:20,new_diag:19,formative:17,ganga:16,na_score:18,smart_book:21} as const;
export type PcReportType=keyof typeof PC_REPORT_TYPES;
export const PC_ERROR_CODES=['pc_unreachable','pc_host_key_mismatch','pc_executor_failed','pc_executor_timeout','pc_output_invalid','pc_input_unencodable'] as const;
export type PcErrorCode=typeof PC_ERROR_CODES[number];
export type PcCleanup='done'|'incomplete';
/** A failure of one call: `message` is the code; `pcCleanup` is set once `report.info` was sent. */
export class PcExecutorError extends Error {
  constructor(code:string,readonly pcCleanup?:PcCleanup){super(code);this.name='PcExecutorError';}
}
/** The keys of the `ReportViewServlet` request of a report (all absent for `course_book` and `cism`). */
export interface ReportInfoKeys {testingNo?:string;bookCode?:string;paperNo?:string;studentKey?:string;classNo?:string;applyCode?:string}
export interface PcRenderInput {reportType:PcReportType;keys:ReportInfoKeys;blockData:Record<string,unknown>;outputPath:string}
export interface PcRendered {status:'rendered';pages:number;sha256:string;outputPath:string;pcCleanup:PcCleanup}
export interface PcExecutor {render(input:PcRenderInput):Promise<PcRendered>}

/** The directory of the executor's files on the PC, under the user's home (scp form and PowerShell form). */
export const PC_REMOTE_DIR='AppData/Roaming/otwoline/studyq/report';
const PS_REMOTE_DIR='AppData\\Roaming\\otwoline\\studyq\\report';
export const PC_EXE_DIR=/^[A-Za-z]:\\[A-Za-z0-9 ()._\\-]+$/;
export const PC_TARGET=/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}@[A-Za-z0-9][A-Za-z0-9.-]{0,252}$/;
export const PC_PDF_NAME=/^\d{17}\.pdf$/;
const EXE_WAIT_SECONDS=120,EXE_TIMEOUT_EXIT=124;
/** Printed with the exit code 124, so a timeout is known also where the remote shell does not pass the code on. */
const EXE_TIMEOUT_WORD='pc-executor-timeout';

/** A member of `report.info`: the digits of a key (leading zeros dropped), or 0. Every member is a JSON number. */
const numberWord=(value:string|undefined)=>value!==undefined&&/^[0-9]{1,64}$/.test(value)?value.replace(/^0+(?=\d)/,''):'0';
/** The text of `report.info`: the eight members of the dispatcher, `block_data` the server's answer as it is. */
export function reportInfoText(reportType:PcReportType,keys:ReportInfoKeys,blockData:Record<string,unknown>):string{
  if(!Object.hasOwn(PC_REPORT_TYPES,reportType))throw new PcExecutorError('pc_executor_failed');
  if(blockData===null||typeof blockData!=='object'||Array.isArray(blockData))throw new PcExecutorError('pc_executor_failed');
  return '{"testing_no":'+numberWord(keys.testingNo)+',"book_code":'+numberWord(keys.bookCode)+',"p_no":'+numberWord(keys.paperNo)
    +',"pri_no":'+numberWord(keys.studentKey)+',"cls_no":'+numberWord(keys.classNo)+',"apply_code":'+numberWord(keys.applyCode)
    +',"report_type":'+PC_REPORT_TYPES[reportType]+',"block_data":'+JSON.stringify(blockData)+'}';
}

// CP949 (windows-949, the WHATWG `euc-kr`): the encoder is the reverse of the platform's decoder, built once.
let cp949:Map<number,number>|null=null;
function cp949Table(){
  if(cp949)return cp949;
  const decoder=new TextDecoder('euc-kr'),table=new Map<number,number>();
  for(let lead=0x81;lead<=0xfe;lead++)for(let trail=0x41;trail<=0xfe;trail++){
    const text=decoder.decode(new Uint8Array([lead,trail]));
    if(text.length===1&&text!=='\ufffd'&&!table.has(text.codePointAt(0)!))table.set(text.codePointAt(0)!,lead<<8|trail);
  }
  return cp949=table;
}
/** `text` in CP949; a character outside CP949 refuses with `pc_input_unencodable`. */
export function encodeCp949(text:string):Buffer{
  const table=cp949Table(),out:number[]=[];
  for(const char of text){
    const code=char.codePointAt(0)!;
    if(code<0x80){out.push(code);continue;}
    const bytes=table.get(code);
    if(bytes===undefined)throw new PcExecutorError('pc_input_unencodable');
    out.push(bytes>>8,bytes&0xff);
  }
  return Buffer.from(out);
}
export const decodeCp949=(bytes:Uint8Array)=>new TextDecoder('euc-kr',{fatal:true}).decode(bytes);

/** The Producer of a PDF's information dictionary, its string escapes decoded, or null. */
export function pdfProducer(bytes:Buffer):string|null{
  const text=bytes.toString('latin1');
  const literal=/\/Producer\s*\(((?:\\[\s\S]|[^\\)])*)\)/.exec(text);
  if(literal)return literal[1]!.replace(/\\(\r\n|\n|\r|[0-7]{1,3}|[\s\S])/g,(_,e:string)=>{
    if(/^[0-7]+$/.test(e))return String.fromCharCode(parseInt(e,8)&0xff);
    return ({n:'\n',r:'\r',t:'\t',b:'\b',f:'\f'} as Record<string,string>)[e]??(/^[\r\n]+$/.test(e)?'':e);
  });
  const hex=/\/Producer\s*<([0-9A-Fa-f\s]*)>/.exec(text);
  if(!hex)return null;
  const digits=hex[1]!.replace(/\s+/g,'');
  return Buffer.from(digits.length%2?digits+'0':digits,'hex').toString('latin1');
}
export const PC_PDF_PRODUCER='Haru Free PDF Library';
export const MAX_PDF_BYTES=64*1024*1024;

// The fixed PowerShell programs, each sent as one -EncodedCommand. Only exeDir (checked at launch) and a PDF
// name that matched ^\d{17}\.pdf$ and was not listed before the run are ever part of one.
const PS_HEAD="$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';$d=Join-Path $env:USERPROFILE '"+PS_REMOTE_DIR+"';$r=Join-Path $d 'report_result.info';$i=Join-Path $d 'report.info';";
export const pcScripts={
  list:()=>PS_HEAD+"if(Test-Path -LiteralPath $r){Remove-Item -LiteralPath $r -Force};Get-ChildItem -LiteralPath $d -File|Where-Object{$_.Name -like '*.pdf'}|ForEach-Object{$_.Name};exit 0",
  run:(exeDir:string)=>PS_HEAD+"$p=Start-Process -FilePath '"+exeDir+"\\StudyQReport.exe' -WorkingDirectory '"+exeDir+"' -PassThru;"
    +"if(-not $p.WaitForExit("+EXE_WAIT_SECONDS*1000+")){Stop-Process -Id $p.Id -Force;'"+EXE_TIMEOUT_WORD+"';exit "+EXE_TIMEOUT_EXIT+"};exit 0",
  collect:(removeInfo:boolean)=>PS_HEAD+(removeInfo?"if(Test-Path -LiteralPath $i){Remove-Item -LiteralPath $i -Force};":"")
    +"if(Test-Path -LiteralPath $r){[Convert]::ToBase64String([IO.File]::ReadAllBytes($r))}else{'-'};exit 0",
  cleanup:(name:string)=>{
    if(!PC_PDF_NAME.test(name))throw new PcExecutorError('pc_executor_failed');
    return PS_HEAD+"$f=Join-Path $d '"+name+"';if(Test-Path -LiteralPath $f){Remove-Item -LiteralPath $f -Force};"
      +"if(Test-Path -LiteralPath $r){Remove-Item -LiteralPath $r -Force};if(Test-Path -LiteralPath $i){Remove-Item -LiteralPath $i -Force};exit 0";
  },
};
export const powershellWords=(script:string)=>'powershell -NoProfile -NonInteractive -EncodedCommand '+Buffer.from(script,'utf16le').toString('base64');

export interface PcExecutorConfig {target:string;identityFile:string;knownHostsFile:string;exeDir:string;lockDir:string}
export interface PcExecutorOptions {
  sshPath?:string;scpPath?:string;
  /** Each ssh or scp step other than the exe run (default 30 s). */
  stepTimeoutMs?:number;
  /** The whole call (default 300 s). */
  deadlineMs?:number;
  /** The wait before the one read of `report_result.info` after a timeout (default 5 s). */
  afterTimeoutDelayMs?:number;
  lockPollMs?:number;
}
export interface Ran {code:number|null;stdout:string;stderr:string;timedOut:boolean;failedToStart:boolean}
export const HOST_KEY=/Host key verification failed|REMOTE HOST IDENTIFICATION HAS CHANGED|No [A-Za-z0-9-]+ host key is known|host key for .* has changed/i;
export const CONNECT=/Connection (timed out|refused|closed|reset)|Could not resolve|No route to host|Network is unreachable|lost connection|connect to host|Permission denied|kex_exchange_identification/i;
export const sleep=(ms:number)=>new Promise(resolve=>setTimeout(resolve,Math.max(0,ms)));

export function runFixed(file:string,args:string[],timeoutMs:number,cwd?:string):Promise<Ran>{
  return new Promise(resolve=>{
    let child;
    // A clean environment: the MCP user's ssh configuration never applies (-F none), and no Academy value is passed.
    try{child=spawn(file,args,{cwd,env:{PATH:'/usr/bin:/bin',LANG:'C.UTF-8',...(process.env.HOME?{HOME:process.env.HOME}:{})},stdio:['ignore','pipe','pipe'],shell:false});}
    catch{resolve({code:null,stdout:'',stderr:'',timedOut:false,failedToStart:true});return;}
    const out:Buffer[]=[],err:Buffer[]=[];let size=0,timedOut=false,failedToStart=false;
    const timer=setTimeout(()=>{timedOut=true;child.kill('SIGKILL');},Math.max(1,timeoutMs));
    child.stdout.on('data',(chunk:Buffer)=>{size+=chunk.length;if(size>4*1024*1024)child.kill('SIGKILL');else out.push(chunk);});
    child.stderr.on('data',(chunk:Buffer)=>{if(Buffer.concat(err).length<64*1024)err.push(chunk);});
    child.once('error',()=>{failedToStart=true;});
    child.once('close',code=>{clearTimeout(timer);resolve({code,stdout:Buffer.concat(out).toString('utf8'),stderr:Buffer.concat(err).toString('utf8'),timedOut,failedToStart});});
  });
}

/** The start time of a process from /proc/<pid>/stat (field 22), or null when it does not run. */
export async function processStartTime(pid:number):Promise<string|null>{
  if(!Number.isSafeInteger(pid)||pid<1)return null;
  try{const text=await readFile(`/proc/${pid}/stat`,'utf8');return text.slice(text.lastIndexOf(')')+2).split(' ')[19]??null;}catch{return null;}
}

/** The host-wide lock `<lockDir>/pc-executor.lock`: made by exclusive create, holding pid, process start time and a
 * nonce; a lock whose process is gone or started at another time is taken over by renaming a new lock file over it. */
export async function acquireLock(lockDir:string,deadline:number,pollMs:number):Promise<()=>Promise<void>>{
  await mkdir(lockDir,{recursive:true,mode:0o700});
  const path=join(lockDir,'pc-executor.lock'),nonce=randomBytes(16).toString('hex');
  const body=JSON.stringify({pid:process.pid,start:await processStartTime(process.pid),nonce})+'\n';
  const ours=async()=>{try{return JSON.parse(await readFile(path,'utf8')).nonce===nonce;}catch{return false;}};
  const release=async()=>{if(await ours())await unlink(path).catch(()=>{});};
  for(;;){
    if(Date.now()>=deadline)throw new PcExecutorError('pc_executor_timeout');
    try{
      const handle=await open(path,'wx',0o600);
      try{await handle.writeFile(body);await handle.sync();}finally{await handle.close();}
      return release;
    }catch(error){if((error as NodeJS.ErrnoException).code!=='EEXIST')throw new PcExecutorError('pc_executor_failed');}
    let seen:string;
    try{seen=await readFile(path,'utf8');}catch{continue;}
    let held:{pid?:unknown;start?:unknown}|null=null;
    try{held=JSON.parse(seen);}catch{held=null;}
    let stale:boolean;
    if(held&&typeof held.pid==='number'){const start=await processStartTime(held.pid);stale=start===null||start!==held.start;}
    // A lock that is not (yet) readable is a creator between its create and its write, unless it is old.
    else{try{stale=Date.now()-(await stat(path)).mtimeMs>10000;}catch{continue;}}
    if(!stale){await sleep(Math.min(pollMs,deadline-Date.now()));continue;}
    const temp=path+'.'+process.pid+'.'+nonce+'.tmp';
    try{
      await writeFile(temp,body,{mode:0o600,flag:'wx'});
      if(await readFile(path,'utf8').catch(()=>null)===seen)await rename(temp,path);
    }finally{await unlink(temp).catch(()=>{});}
    if(await ours())return release;
  }
}

/** The executor of one configuration; `options` replace the ssh and scp programs and the time limits in tests. */
export function pcReportExecutor(config:PcExecutorConfig,options:PcExecutorOptions={}):PcExecutor{
  if(!PC_TARGET.test(config.target)||!isAbsolute(config.identityFile)||!isAbsolute(config.knownHostsFile)||!PC_EXE_DIR.test(config.exeDir)||!isAbsolute(config.lockDir))
    throw Error('pc_executor_arguments_invalid');
  const ssh=options.sshPath??'/usr/bin/ssh',scp=options.scpPath??'/usr/bin/scp';
  const stepMs=options.stepTimeoutMs??30000,deadlineMs=options.deadlineMs??300000,afterTimeoutMs=options.afterTimeoutDelayMs??5000;
  const words=sshOptionArgv({connectTimeoutSeconds:10,identityFile:config.identityFile,knownHostsFile:config.knownHostsFile,noConfig:true});
  const remote=(name:string)=>config.target+':'+PC_REMOTE_DIR+'/'+name;
  let tail:Promise<unknown>=Promise.resolve();

  async function call(input:PcRenderInput,deadline:number):Promise<PcRendered>{
    let sent=false,name:string|null=null;
    const fail=(code:string):never=>{throw new PcExecutorError(code,sent?'incomplete':undefined);};
    /** One step: the time left of the call bounds the step's own limit; a step stopped by the call's deadline is a timeout. */
    const limit=(ms:number)=>{const left=deadline-Date.now();if(left<=0)fail('pc_executor_timeout');return {ms:Math.min(ms,left),byDeadline:left<ms};};
    const connectFailure=(ran:Ran,byDeadline:boolean,isScp:boolean)=>{
      if(ran.timedOut)return byDeadline?'pc_executor_timeout':'pc_unreachable';
      if(ran.failedToStart)return 'pc_executor_failed';
      if(HOST_KEY.test(ran.stderr))return 'pc_host_key_mismatch';
      if(!isScp&&ran.code===255)return 'pc_unreachable';
      if(isScp&&CONNECT.test(ran.stderr))return 'pc_unreachable';
      return null;
    };
    const sshStep=async(script:string,ms=stepMs)=>{
      const {ms:timeout,byDeadline}=limit(ms);
      const ran=await runFixed(ssh,[...words,config.target,powershellWords(script)],timeout);
      return {ran,failure:connectFailure(ran,byDeadline,false)};
    };
    const scpStep=async(from:string,to:string,cwd:string)=>{
      const {ms:timeout,byDeadline}=limit(stepMs);
      const ran=await runFixed(scp,[...words,from,to],timeout,cwd);
      const failure=connectFailure(ran,byDeadline,true);
      if(failure)fail(failure);
      if(ran.code!==0)fail('pc_executor_failed');
    };
    /** The answer of `report_result.info`, or null when the PC holds none. */
    const collect=async(removeInfo:boolean)=>{
      const {ran,failure}=await sshStep(pcScripts.collect(removeInfo));
      if(failure)fail(failure);
      if(ran.code!==0)fail('pc_executor_failed');
      const word=ran.stdout.trim();
      if(word==='-')return null;
      if(!/^[A-Za-z0-9+/]+={0,2}$/.test(word))fail('pc_executor_failed');
      try{const value=JSON.parse(decodeCp949(Buffer.from(word,'base64')));return value&&typeof value==='object'&&!Array.isArray(value)?value as Record<string,unknown>:{};}
      catch{return {};}
    };
    /** The one PDF a result names: exactly one entry whose name matches and was not listed before the run. */
    const namedPdf=(result:Record<string,unknown>|null,listed:Set<string>)=>{
      const list=result?.pdf_list;
      if(!Array.isArray(list)||list.length!==1||!list[0]||typeof list[0]!=='object')return null;
      const file=(list[0] as Record<string,unknown>).pdf_file;
      return typeof file==='string'&&PC_PDF_NAME.test(file)&&!listed.has(file)?file:null;
    };
    const cleanup=async():Promise<PcCleanup>=>{
      if(name===null||deadline-Date.now()<=0)return 'incomplete';
      try{const {ran,failure}=await sshStep(pcScripts.cleanup(name));return failure||ran.code!==0?'incomplete':'done';}
      catch{return 'incomplete';}
    };

    // 1. The PDFs already on the PC, and no stale result.
    const listing=await sshStep(pcScripts.list());
    if(listing.failure)fail(listing.failure);
    if(listing.ran.code!==0)fail('pc_executor_failed');
    const listed=new Set(listing.ran.stdout.split(/\r?\n/).map(line=>line.trim()).filter(Boolean));
    // 2. report.info, written in CP949 under a private temporary directory.
    const bytes=encodeCp949(reportInfoText(input.reportType,input.keys,input.blockData));
    const local=await mkdtemp(join(tmpdir(),'pc-executor-'));
    try{
      await writeFile(join(local,'report.info'),bytes,{mode:0o600,flag:'wx'});
      sent=true;
      await scpStep('./report.info',remote('report.info'),local);
    }finally{await rm(local,{recursive:true,force:true});}
    // 3. The exe, waited for up to 120 s on the PC; past that only the process it started is stopped.
    const run=await sshStep(pcScripts.run(config.exeDir),EXE_WAIT_SECONDS*1000+stepMs);
    if(run.failure&&run.failure!=='pc_executor_timeout')fail(run.failure);
    if(run.failure==='pc_executor_timeout'||run.ran.code===EXE_TIMEOUT_EXIT||run.ran.stdout.split(/\r?\n/).some(line=>line.trim()===EXE_TIMEOUT_WORD)){
      // One more read after 5 s: only a PDF that the result names and that passes both checks is deleted.
      let pcCleanup:PcCleanup='incomplete';
      try{
        await sleep(Math.min(afterTimeoutMs,deadline-Date.now()));
        name=namedPdf(await collect(false),listed);
        pcCleanup=await cleanup();
      }catch{pcCleanup='incomplete';}
      throw new PcExecutorError('pc_executor_timeout',pcCleanup);
    }
    let result:Record<string,unknown>|null=null,failure:string|null=run.ran.code===0?null:'pc_executor_failed';
    try{result=await collect(true);}catch(error){if(!failure)throw error;}
    name=namedPdf(result,listed);
    let rendered:PcRendered|null=null,error:unknown=null;
    try{
      if(failure)fail(failure);
      if(!result||result.result!=='ok')fail('pc_executor_failed');
      const list=result!.pdf_list;
      if(!Array.isArray(list)||list.length!==1||!list[0]||typeof list[0]!=='object'||(list[0] as Record<string,unknown>).pdf_type!=='report')fail('pc_output_invalid');
      const entry=(list as Record<string,unknown>[])[0]!,pages=entry.page_cnt;
      if(typeof pages!=='number'||!Number.isInteger(pages)||pages<1)fail('pc_executor_failed');
      if(name===null)fail('pc_executor_failed');
      // 4. The PDF, copied beside the output path under a fixed temporary name, checked, then linked into place.
      const directory=dirname(input.outputPath),temp='.pc-executor-'+randomBytes(8).toString('hex')+'.tmp';
      try{
        await scpStep(remote(name!),'./'+temp,directory);
        const file=join(directory,temp);
        if((await stat(file)).size>MAX_PDF_BYTES)fail('pc_output_invalid');
        const pdf=await readFile(file);
        if(pdf.subarray(0,5).toString('latin1')!=='%PDF-'||!pdfProducer(pdf)?.startsWith(PC_PDF_PRODUCER))fail('pc_output_invalid');
        await chmod(file,0o600);
        try{await link(file,input.outputPath);}
        catch(e){fail((e as NodeJS.ErrnoException).code==='EEXIST'?'output_exists':'output_path_invalid');}
        rendered={status:'rendered',pages:pages as number,sha256:createHash('sha256').update(pdf).digest('hex'),outputPath:input.outputPath,pcCleanup:'incomplete'};
      }finally{await unlink(join(directory,temp)).catch(()=>{});}
    }catch(e){error=e;}
    // 5. The PDF and report_result.info are deleted on the PC, also after a failure, once a qualified name is known.
    const pcCleanup=await cleanup();
    if(rendered)return {...rendered,pcCleanup};
    throw new PcExecutorError(error instanceof Error?error.message:'pc_executor_failed',pcCleanup);
  }

  return {async render(input){
    if(!Object.hasOwn(PC_REPORT_TYPES,input.reportType)||!isAbsolute(input.outputPath))throw new PcExecutorError('output_path_invalid');
    // A character outside CP949 refuses before any remote step and before the lock.
    encodeCp949(reportInfoText(input.reportType,input.keys,input.blockData));
    const deadline=Date.now()+deadlineMs;
    const result=tail.then(async()=>{
      const release=await acquireLock(config.lockDir,deadline,options.lockPollMs??250);
      try{return await call(input,deadline);}finally{await release();}
    });
    tail=result.catch(()=>{});
    return result;
  }};
}

/** `--pc-executor <user@host> <identityFile> <knownHostsFile> <exeDir>`: absolute regular files, exeDir of the
 * form `C:\...`. Without it the executor is null and every report answers `pc_executor_not_configured`. `config` is the
 * checked configuration, which the paper executor (src/pc-paper-executor.ts) shares: no further launch argument. */
export async function pcExecutorConfiguration(args:readonly string[],base:{lockDir:string},options:PcExecutorOptions={}){
  const remainingArgs:string[]=[];let executor:PcExecutor|null=null,config:PcExecutorConfig|null=null;
  for(let i=0;i<args.length;i++){
    if(args[i]!=='--pc-executor'){remainingArgs.push(args[i]!);continue;}
    if(executor||i+4>=args.length)throw Error('pc_executor_arguments_invalid');
    const [target,identityFile,knownHostsFile,exeDir]=args.slice(i+1,i+5) as [string,string,string,string];
    for(const file of [identityFile,knownHostsFile]){
      if(!isAbsolute(file))throw Error('pc_executor_arguments_invalid');
      try{if(!(await stat(file)).isFile())throw Error('pc_executor_arguments_invalid');}catch{throw Error('pc_executor_arguments_invalid');}
    }
    config={target,identityFile,knownHostsFile,exeDir,lockDir:base.lockDir};
    executor=pcReportExecutor(config,options);
    i+=4;
  }
  return {remainingArgs,executor,config};
}
