import {createHash,randomBytes} from 'node:crypto';
import {chmod,link,mkdtemp,readFile,rm,stat,unlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname,isAbsolute,join} from 'node:path';
import {sshOptionArgv} from './ssh-argv.ts';
import {
  CONNECT,HOST_KEY,MAX_PDF_BYTES,PC_EXE_DIR,PC_PDF_NAME,PC_PDF_PRODUCER,PcExecutorError,acquireLock,decodeCp949,encodeCp949,pdfProducer,
  powershellWords,runFixed,sleep,type PcCleanup,type PcExecutorConfig,type PcExecutorOptions,type Ran,
} from './pc-executor.ts';

/** The PC paper executor (specs/academy-mcp/pc-paper-executor.md): one paper PDF (exam, answer, score or clinic paper) per
 * print item, made by the native programs of the StudyQ install on the user's Windows PC, reached over SSH with fixed argv
 * only and the configuration of `--pc-executor`. `StudyQPdfManager.exe` reads `pdf.info`, prepares the images and QR codes
 * and writes `examset\examset.info`; it is stopped with its own child processes as soon as that file is written, and
 * `ETestMaker.exe` is started alone, so that it writes the PDF file `examset\<17 digits>.pdf` named by
 * `examset\examset_result.info`. Nothing here draws a page. Each item holds the host-wide lock of src/pc-executor.ts.
 * Nothing on the PC is created or removed but `pdf.info`, `examset.info`, `examset_result.info` and the PDF that result
 * names, besides the native cache (`exam\`, `examtext\`, `examset\theme\`); only processes the call started are stopped. */

/** The StudyQ data directory under the user's home (scp form and PowerShell form). */
export const PC_STUDYQ_DIR='AppData/Roaming/otwoline/studyq';
const PS_STUDYQ_DIR='AppData\\Roaming\\otwoline\\studyq';
const EXE_WAIT_SECONDS=120,EXE_TIMEOUT_EXIT=124,MANAGER_FAILED_EXIT=3;
const EXE_TIMEOUT_WORD='pc-executor-timeout',BUSY_WORD='pc-busy';
/** The native programs whose running instance means the teacher is printing. */
export const PC_PRINT_PROGRAMS=['StudyQPdfManager','ETestMaker','PDFMerge'] as const;

/** The StudyQ install root on the PC: the parent folder of `exeDir` (`<root>\report`), or null when it has none. */
export function studyqInstallRoot(exeDir:string):string|null{
  const trimmed=exeDir.replace(/\\+$/,''),at=trimmed.lastIndexOf('\\');
  if(at<0)return null;
  const root=trimmed.slice(0,at);
  return PC_EXE_DIR.test(root)&&/\\./.test(root)?root:null;
}

/** The members a print item must hold before any PC step: an `examset.info` without one of them crashed `ETestMaker.exe`
 * (measured). An item built from `getTestExamList` also needs `add_incorrect_comment` (from its `add_comment`). */
export const PRINT_ITEM_MEMBERS=['add_answer','add_code','add_difficulty','add_explain','add_pat','exam_dispose_type','exam_spacer'] as const;
const isRecord=(value:unknown):value is Record<string,unknown>=>value!==null&&typeof value==='object'&&!Array.isArray(value);
/** One exam row with its `exam_size_list` as a JSON array (a string is parsed); anything else refuses. */
function examForPc(exam:unknown):Record<string,unknown>{
  if(!isRecord(exam))throw Error('paper_print_item_incomplete');
  if(exam.exam_size_list===undefined)return exam;
  let sizes=exam.exam_size_list;
  if(typeof sizes==='string'){try{sizes=JSON.parse(sizes);}catch{throw Error('paper_print_item_incomplete');}}
  if(!Array.isArray(sizes))throw Error('paper_print_item_incomplete');
  return {...exam,exam_size_list:sizes};
}
/** The answer paper of a multi-testing clinic write (measured): `paper_type` `'answer'`, no `exam_list`, and a `user_list` array. */
const isAnswerForm=(raw:Record<string,unknown>)=>raw.paper_type==='answer'&&raw.exam_list===undefined&&Array.isArray(raw.user_list);
/** One print item as the PC takes it: an object holding the members of `PRINT_ITEM_MEMBERS` (and `add_incorrect_comment` when
 * `fromExamList`) and a non-empty `exam_list`, each `exam_size_list` a JSON array (the server sends it as a string, which
 * `StudyQPdfManager.exe` would copy into a broken `examset.info`). In place of `exam_list` the answer paper of a clinic write
 * holds `user_list`: 1 to 200 entries, each with a non-empty `exam_list` (at most 5000 rows, each with an `exam_no`), whose
 * `exam_size_list` strings are parsed the same way. Anything else refuses with `paper_print_item_incomplete`.
 * The input is not changed. */
export function printItemForPc(raw:unknown,options:{fromExamList:boolean}):Record<string,unknown>{
  if(!isRecord(raw))throw Error('paper_print_item_incomplete');
  const required=options.fromExamList?[...PRINT_ITEM_MEMBERS,'add_incorrect_comment']:PRINT_ITEM_MEMBERS;
  if(required.some(member=>raw[member]===undefined||raw[member]===null))throw Error('paper_print_item_incomplete');
  if(isAnswerForm(raw)){
    const users=raw.user_list as unknown[];
    if(users.length===0||users.length>200)throw Error('paper_print_item_incomplete');
    const userList=users.map(user=>{
      if(!isRecord(user)||!Array.isArray(user.exam_list)||user.exam_list.length===0||user.exam_list.length>5000)throw Error('paper_print_item_incomplete');
      return {...user,exam_list:user.exam_list.map(exam=>{
        const row=examForPc(exam);
        if(row.exam_no===undefined||row.exam_no===null)throw Error('paper_print_item_incomplete');
        return row;
      })};
    });
    return {...raw,user_list:userList};
  }
  if(!Array.isArray(raw.exam_list)||raw.exam_list.length===0)throw Error('paper_print_item_incomplete');
  return {...raw,exam_list:raw.exam_list.map(examForPc)};
}

const KEY=/^[1-9][0-9]{0,15}$/;
export interface PrintPrincipal {schoolKey:string;teacherKey:string}
/** The text of `pdf.info`: the print command of the page with one item, `sk` and `uk` the principal's keys as numbers (without
 * `sk` the native program writes `clg_no` 0), `pid` the time in milliseconds. */
export function pdfInfoText(item:Record<string,unknown>,principal:PrintPrincipal,pid:string):string{
  if(!KEY.test(principal.schoolKey)||!KEY.test(principal.teacherKey)||!/^[0-9]{1,16}$/.test(pid))throw new PcExecutorError('pc_executor_failed');
  return '{"app_ver":"8.6","cmd":"Print","pid":"'+pid+'","print_list":['+JSON.stringify(item)+'],"sk":'+principal.schoolKey+',"uk":'+principal.teacherKey+'}';
}

export interface PcPrintInput {item:Record<string,unknown>;fromExamList:boolean;principal:PrintPrincipal;outputPath:string}
export interface PcPrinted {status:'rendered';paperType:string;pageCount:number;sha256:string;outputPath:string;pcCleanup:PcCleanup}
export interface PcPaperExecutor {print(input:PcPrintInput):Promise<PcPrinted>}

// The fixed PowerShell programs, each sent as one -EncodedCommand. Only the two program folders (from exeDir, checked at launch)
// and a PDF name that matched ^\d{17}\.pdf$ and was not listed before ETestMaker ran are ever part of one.
const PS_HEAD="$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';$d=Join-Path $env:USERPROFILE '"+PS_STUDYQ_DIR+"';"
  +"$x=Join-Path $d 'examset';$i=Join-Path $d 'pdf.info';$e=Join-Path $x 'examset.info';$r=Join-Path $x 'examset_result.info';";
const programFolder=(folder:string)=>{if(!PC_EXE_DIR.test(folder))throw new PcExecutorError('pc_executor_failed');return folder;};
export const pcPaperScripts={
  busy:()=>PS_HEAD+"if(Get-Process -Name "+PC_PRINT_PROGRAMS.map(n=>"'"+n+"'").join(',')+" -ErrorAction SilentlyContinue){'"+BUSY_WORD+"'};exit 0",
  /** StudyQPdfManager, until it has written examset.info after its start; then it and every process whose parent it is are stopped by id. */
  manager:(studylinkDir:string)=>{
    const dir=programFolder(studylinkDir);
    return PS_HEAD+"$s=(Get-Date).ToUniversalTime();$p=Start-Process -FilePath '"+dir+"\\StudyQPdfManager.exe' -WorkingDirectory '"+dir+"' -PassThru;"
      +"$w=[Diagnostics.Stopwatch]::StartNew();$ok=$false;"
      +"while($w.ElapsedMilliseconds -lt "+EXE_WAIT_SECONDS*1000+"){if((Test-Path -LiteralPath $e) -and (Get-Item -LiteralPath $e).LastWriteTimeUtc -gt $s){"
      +"try{$h=[IO.File]::Open($e,'Open','Read','None');$h.Close();$ok=$true;break}catch{}};if($p.HasExited){break};Start-Sleep -Milliseconds 200};"
      +"Get-CimInstance Win32_Process -Filter ('ParentProcessId='+$p.Id)|ForEach-Object{Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue;Wait-Process -Id $_.ProcessId -Timeout 10 -ErrorAction SilentlyContinue};"
      +"if(-not $p.HasExited){Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue;Wait-Process -Id $p.Id -Timeout 10 -ErrorAction SilentlyContinue};"
      +"if(-not $ok){if($w.ElapsedMilliseconds -ge "+EXE_WAIT_SECONDS*1000+"){'"+EXE_TIMEOUT_WORD+"';exit "+EXE_TIMEOUT_EXIT+"};exit "+MANAGER_FAILED_EXIT+"};exit 0";
  },
  /** A stale examset_result.info is deleted and the names of the PDFs already in examset\ are listed. */
  list:()=>PS_HEAD+"if(Test-Path -LiteralPath $r){Remove-Item -LiteralPath $r -Force};if(Test-Path -LiteralPath $x){Get-ChildItem -LiteralPath $x -File|Where-Object{$_.Name -like '*.pdf'}|ForEach-Object{$_.Name}};exit 0",
  maker:(ipmDir:string)=>{
    const dir=programFolder(ipmDir);
    return PS_HEAD+"$p=Start-Process -FilePath '"+dir+"\\ETestMaker.exe' -WorkingDirectory '"+dir+"' -PassThru;"
      +"if(-not $p.WaitForExit("+EXE_WAIT_SECONDS*1000+")){Stop-Process -Id $p.Id -Force;'"+EXE_TIMEOUT_WORD+"';exit "+EXE_TIMEOUT_EXIT+"};exit 0";
  },
  collect:()=>PS_HEAD+"if(Test-Path -LiteralPath $r){[Convert]::ToBase64String([IO.File]::ReadAllBytes($r))}else{'-'};exit 0",
  /** pdf.info, examset.info and examset_result.info, and the PDF the result named when one is known. */
  cleanup:(name:string|null)=>{
    if(name!==null&&!PC_PDF_NAME.test(name))throw new PcExecutorError('pc_executor_failed');
    return PS_HEAD+(name===null?'':"$f=Join-Path $x '"+name+"';if(Test-Path -LiteralPath $f){Remove-Item -LiteralPath $f -Force};")
      +"foreach($f in @($r,$e,$i)){if(Test-Path -LiteralPath $f){Remove-Item -LiteralPath $f -Force}};exit 0";
  },
};

/** The paper executor of one `--pc-executor` configuration, or null when exeDir has no parent folder. `options` replace the ssh
 * and scp programs and the time limits in tests; `deadlineMs` (default 300 s) bounds each item, waiting for the lock included. */
export function pcPaperExecutor(config:PcExecutorConfig,options:PcExecutorOptions={}):PcPaperExecutor|null{
  const root=studyqInstallRoot(config.exeDir);
  if(root===null)return null;
  const studylinkDir=root+'\\studylink',ipmDir=root+'\\ipm';
  const ssh=options.sshPath??'/usr/bin/ssh',scp=options.scpPath??'/usr/bin/scp';
  const stepMs=options.stepTimeoutMs??30000,deadlineMs=options.deadlineMs??300000,afterTimeoutMs=options.afterTimeoutDelayMs??5000;
  const words=sshOptionArgv({connectTimeoutSeconds:10,identityFile:config.identityFile,knownHostsFile:config.knownHostsFile,noConfig:true});
  const remote=(path:string)=>config.target+':'+PC_STUDYQ_DIR+'/'+path;
  let tail:Promise<unknown>=Promise.resolve();

  async function call(input:PcPrintInput,bytes:Buffer,deadline:number):Promise<PcPrinted>{
    let sent=false,listed:Set<string>|null=null,makerStarted=false,makerTimedOut=false,collected=false;
    let result:Record<string,unknown>|null=null,name:string|null=null;
    const fail=(code:string):never=>{throw new PcExecutorError(code);};
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
      const failure=connectFailure(ran,byDeadline,false);
      if(failure)fail(failure);
      return ran;
    };
    const scpStep=async(from:string,to:string,cwd:string)=>{
      const {ms:timeout,byDeadline}=limit(stepMs);
      const ran=await runFixed(scp,[...words,from,to],timeout,cwd);
      const failure=connectFailure(ran,byDeadline,true);
      if(failure)fail(failure);
      if(ran.code!==0)fail('pc_executor_failed');
    };
    const timedOut=(ran:Ran)=>ran.code===EXE_TIMEOUT_EXIT||ran.stdout.split(/\r?\n/).some(line=>line.trim()===EXE_TIMEOUT_WORD);
    /** The answer of examset_result.info, null when the PC holds none, {} when it is not a JSON object. */
    const collect=async()=>{
      const ran=await sshStep(pcPaperScripts.collect());
      if(ran.code!==0)fail('pc_executor_failed');
      collected=true;
      const word=ran.stdout.trim();
      if(word==='-')return null;
      if(!/^[A-Za-z0-9+/]+={0,2}$/.test(word))fail('pc_executor_failed');
      try{const value=JSON.parse(decodeCp949(Buffer.from(word,'base64')));return isRecord(value)?value:{};}catch{return {};}
    };
    /** The one PDF a result names: exactly one entry whose name matches and was not in examset\ before ETestMaker ran. */
    const namedPdf=()=>{
      const list=result?.pdf_list;
      if(listed===null||!Array.isArray(list)||list.length!==1||!isRecord(list[0]))return null;
      const file=list[0].pdf_file;
      return typeof file==='string'&&PC_PDF_NAME.test(file)&&!listed.has(file)?file:null;
    };

    // 1. Refuse while the teacher is printing: nothing has been sent yet.
    const busy=await sshStep(pcPaperScripts.busy());
    if(busy.code!==0)fail('pc_executor_failed');
    if(busy.stdout.split(/\r?\n/).some(line=>line.trim()===BUSY_WORD))fail('pc_busy');
    let printed:PcPrinted|null=null,error:unknown=null;
    try{
      // 2. pdf.info, written in CP949 under a private temporary directory, then StudyQPdfManager until examset.info is written.
      const local=await mkdtemp(join(tmpdir(),'pc-paper-'));
      try{
        await writeFile(join(local,'pdf.info'),bytes,{mode:0o600,flag:'wx'});
        sent=true;
        await scpStep('./pdf.info',remote('pdf.info'),local);
      }finally{await rm(local,{recursive:true,force:true});}
      const manager=await sshStep(pcPaperScripts.manager(studylinkDir),EXE_WAIT_SECONDS*1000+stepMs);
      if(timedOut(manager))fail('pc_executor_timeout');
      if(manager.code!==0)fail('pc_executor_failed');
      // 3. ETestMaker alone, after a stale result is deleted and the PDFs already there are listed.
      const listing=await sshStep(pcPaperScripts.list());
      if(listing.code!==0)fail('pc_executor_failed');
      listed=new Set(listing.stdout.split(/\r?\n/).map(line=>line.trim()).filter(Boolean));
      makerStarted=true;
      let maker:Ran;
      try{maker=await sshStep(pcPaperScripts.maker(ipmDir),EXE_WAIT_SECONDS*1000+stepMs);}
      catch(e){if(e instanceof PcExecutorError&&e.message==='pc_executor_timeout')makerTimedOut=true;throw e;}
      if(timedOut(maker)){makerTimedOut=true;fail('pc_executor_timeout');}
      if(maker.code!==0)fail('pc_executor_failed');
      result=await collect();
      name=namedPdf();
      if(!result||result.result!=='ok'||name===null)fail('pc_output_invalid');
      const entry=(result!.pdf_list as Record<string,unknown>[])[0]!,pages=entry.page_cnt;
      if(typeof pages!=='number'||!Number.isInteger(pages)||pages<1)fail('pc_output_invalid');
      // 4. The PDF, copied beside the output path under a fixed temporary name, checked, then linked into place.
      const directory=dirname(input.outputPath),temp='.pc-executor-'+randomBytes(8).toString('hex')+'.tmp';
      try{
        await scpStep(remote('examset/'+name),'./'+temp,directory);
        const file=join(directory,temp);
        if((await stat(file)).size>MAX_PDF_BYTES)fail('pc_output_invalid');
        const pdf=await readFile(file);
        if(pdf.subarray(0,5).toString('latin1')!=='%PDF-'||!pdfProducer(pdf)?.startsWith(PC_PDF_PRODUCER))fail('pc_output_invalid');
        await chmod(file,0o600);
        try{await link(file,input.outputPath);}
        catch(e){fail((e as NodeJS.ErrnoException).code==='EEXIST'?'output_exists':'output_path_invalid');}
        const paperType=typeof input.item.paper_type==='string'?input.item.paper_type:typeof entry.pdf_type==='string'?entry.pdf_type:'';
        printed={status:'rendered',paperType:paperType.slice(0,32),pageCount:pages as number,sha256:createHash('sha256').update(pdf).digest('hex'),outputPath:input.outputPath,pcCleanup:'incomplete'};
      }finally{await unlink(join(directory,temp)).catch(()=>{});}
    }catch(e){error=e;}
    // A failure after ETestMaker started reads its result once more (after a timeout, 5 s later) to learn the PDF it named.
    if(!printed&&makerStarted&&!collected){
      try{if(makerTimedOut)await sleep(Math.min(afterTimeoutMs,deadline-Date.now()));result=await collect();name=namedPdf();}catch{name=null;}
    }
    // 5. pdf.info, examset.info, examset_result.info and the named PDF are deleted on the PC, after a failure too.
    let pcCleanup:PcCleanup='incomplete';
    if(sent&&deadline-Date.now()>0){
      try{const ran=await sshStep(pcPaperScripts.cleanup(name));pcCleanup=ran.code===0?'done':'incomplete';}catch{pcCleanup='incomplete';}
    }
    if(printed)return {...printed,pcCleanup};
    throw new PcExecutorError(error instanceof Error?error.message:'pc_executor_failed',sent?pcCleanup:undefined);
  }

  return {async print(input){
    if(!isAbsolute(input.outputPath))throw new PcExecutorError('output_path_invalid');
    // The item and a character outside CP949 are refused before the lock and any remote step.
    const item=printItemForPc(input.item,{fromExamList:input.fromExamList});
    const bytes=encodeCp949(pdfInfoText(item,input.principal,String(Date.now())));
    const deadline=Date.now()+deadlineMs;
    const result=tail.then(async()=>{
      const release=await acquireLock(config.lockDir,deadline,options.lockPollMs??250);
      try{return await call({...input,item},bytes,deadline);}finally{await release();}
    });
    tail=result.catch(()=>{});
    return result;
  }};
}
