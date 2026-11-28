import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {lstat,mkdtemp,readdir,readFile,realpath,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {isAbsolute,join,relative} from 'node:path';
import {verifyInterpreter} from './textbook-pdf-reader.ts';

/** The Linux-native StudyQ executor (modules/studyq-linux, specs/studyq-linux/core.md) run from a
 * launch-pinned interpreter and source tree. Before every spawn both are verified again: the
 * interpreter's bytes, and the tree `<studyqSrcDir>/studyq_linux/`, which holds no symbolic link,
 * no `__pycache__` directory or `.pyc` file and nothing group or world writable, and whose
 * sorted `<relpath>\0<sha256>\n` lines over its `.py` files hash to the pinned digest. Tool
 * callers supply no executable or path of the runner. */
export interface StudyqRenderOk {status:'rendered';pages:number;sha256:string;outputPath:string;parityOk:boolean;parityMismatches:number;parityRawRn:number}
export interface StudyqRefusal {status:'refused';code:string;detail:string}
export type StudyqRenderResult=StudyqRenderOk|StudyqRefusal;
export interface StudyqPrintOk {status:'rendered';reportType:string;pages:number;sha256:string;outputPath:string}
export type StudyqPrintResult=StudyqPrintOk|StudyqRefusal;
export interface StudyqRunner {
  /** Assemble the print archives under the `archive_root` of `input` (`assemble-print`, specs/studyq-linux/core.md R20-R25)
   * into `outputPath`. The archive key is read by the executor from the file named by the launch's `STUDYQ_PRINT_KEY_FILE`;
   * the runner takes no key from a caller. The executor opens no network connection. */
  assemblePrint(input:unknown,outputPath:string):Promise<StudyqPrintResult>;
  /** Render `input` (the eleven `course_book` members) to `outputPath`, then check its parity. */
  renderCourseBook(input:unknown,outputPath:string):Promise<StudyqRenderResult>;
  /** Render `input` (the six `cism` members) to `outputPath`, then check its parity (`parity --type cism`). */
  renderCism(input:unknown,outputPath:string):Promise<StudyqRenderResult>;
  /** Render `input` of `reportType` (`render --type <reportType>`, specs/studyq-linux/core.md R5) to `outputPath`,
   * then check its parity (`parity --type <reportType>`). */
  renderReport(reportType:StudyqReportType,input:unknown,outputPath:string):Promise<StudyqRenderResult>;
}
export const studyqReportTypes=['course_book','cism','diag','new_diag','formative'] as const;
export type StudyqReportType=typeof studyqReportTypes[number];

const PACKAGE='studyq_linux';
/** The digest of the source tree `<srcDir>/studyq_linux/`, or a refusal (`studyq_source_rejected`). */
export async function studyqSourceTreeSha256(srcDir:string):Promise<string>{
  const root=join(srcDir,PACKAGE),lines:string[]=[];
  const visit=async(dir:string)=>{
    const stat=await lstat(dir);
    if(stat.isSymbolicLink()||!stat.isDirectory()||(stat.mode&0o022)!==0)throw Error('studyq_source_rejected');
    for(const name of (await readdir(dir)).sort()){
      const path=join(dir,name),entry=await lstat(path);
      if(entry.isSymbolicLink()||name==='__pycache__'||name.endsWith('.pyc')||(entry.mode&0o022)!==0)throw Error('studyq_source_rejected');
      if(entry.isDirectory()){await visit(path);continue;}
      if(!entry.isFile())throw Error('studyq_source_rejected');
      if(name.endsWith('.py'))lines.push(relative(root,path)+'\0'+createHash('sha256').update(await readFile(path)).digest('hex')+'\n');
    }
  };
  await visit(root);
  if(!lines.length)throw Error('studyq_source_rejected');
  return createHash('sha256').update(lines.sort().join('')).digest('hex');
}
export async function verifyStudyqSourceTree(srcDir:string,expected:string){
  if(!/^[a-f0-9]{64}$/.test(expected)||!isAbsolute(srcDir)||await realpath(srcDir)!==srcDir)throw Error('studyq_source_rejected');
  if(await studyqSourceTreeSha256(srcDir)!==expected)throw Error('studyq_source_rejected');
}

/** The one program the interpreter runs: the pinned tree first on sys.path, then the module's main. */
const program=(module:'cli'|'parity')=>`import sys; sys.path.insert(0, sys.argv.pop(1)); from ${PACKAGE}.${module} import main; sys.exit(main(sys.argv[1:]))`;
interface Pinned {interpreter:string;interpreterSha256:string;srcDir:string;sourceSha256:string;assetsDir:string;printKeyFile?:string|undefined;timeoutMs?:number}

async function runPinned(pin:Pinned,module:'cli'|'parity',args:string[],stdin:Buffer):Promise<{exitCode:number|null;stdout:string}>{
  await verifyInterpreter(pin.interpreter,pin.interpreterSha256,'studyq_interpreter_rejected');
  await verifyStudyqSourceTree(pin.srcDir,pin.sourceSha256);
  const cache=await mkdtemp(join(tmpdir(),'studyq-pycache-'));
  try{
    return await new Promise((resolve,reject)=>{
      // A clean environment: no Academy credential, only the archive key file when configured.
      const child=spawn(pin.interpreter,['-I','-B','-X','pycache_prefix='+cache,'-c',program(module),pin.srcDir,...args],
        {env:{PATH:'/usr/bin:/bin',LANG:'C.UTF-8',...(pin.printKeyFile?{STUDYQ_PRINT_KEY_FILE:pin.printKeyFile}:{})},stdio:['pipe','pipe','pipe'],shell:false});
      const out:Buffer[]=[];let size=0;
      const timer=setTimeout(()=>child.kill('SIGKILL'),pin.timeoutMs??90000);
      child.stdout.on('data',(chunk:Buffer)=>{size+=chunk.length;if(size>256*1024)child.kill('SIGKILL');else out.push(chunk);});
      child.stderr.resume();
      child.once('error',()=>{clearTimeout(timer);reject(Error('studyq_runner_failed'));});
      child.once('close',code=>{clearTimeout(timer);resolve({exitCode:code,stdout:Buffer.concat(out).toString('utf8')});});
      child.stdin.on('error',()=>{});
      child.stdin.end(stdin);
    });
  }finally{await rm(cache,{recursive:true,force:true});}
}
const lastJsonLine=(stdout:string):Record<string,unknown>|null=>{
  const line=stdout.trim().split('\n').at(-1);
  if(!line)return null;
  try{const value=JSON.parse(line);return value&&typeof value==='object'&&!Array.isArray(value)?value:null;}catch{return null;}
};
const safeDetail=(value:unknown)=>typeof value==='string'?value.slice(0,256):'';

export function pinnedStudyqRunner(pin:Pinned):StudyqRunner{
  async function render(type:StudyqReportType,input:unknown,outputPath:string):Promise<StudyqRenderResult>{
    if(!(studyqReportTypes as readonly string[]).includes(type))throw Error('studyq_report_type_invalid');
    if(!isAbsolute(outputPath))throw Error('output_path_invalid');
    const json=Buffer.from(JSON.stringify(input),'utf8');
    const run=await runPinned(pin,'cli',['render','--type',type,'--assets',pin.assetsDir,'--out',outputPath],json);
    const line=lastJsonLine(run.stdout);
    if(!line)throw Error('studyq_runner_failed');
    // A refusal of the executor (input_invalid, output_exists …) is its own answer; its detail holds no input value (R7 of core.md).
    if(line.status==='error'&&typeof line.code==='string'&&/^[a-z_]{1,64}$/.test(line.code))return {status:'refused',code:line.code,detail:safeDetail(line.detail)};
    if(run.exitCode!==0||line.status!=='ok'||line.outputPath!==outputPath||typeof line.sha256!=='string'||!/^[a-f0-9]{64}$/.test(line.sha256)
      ||typeof line.pages!=='number'||!Number.isInteger(line.pages))throw Error('studyq_runner_failed');
    if(createHash('sha256').update(await readFile(outputPath)).digest('hex')!==line.sha256)throw Error('studyq_runner_failed');
    // parity.main exits 1 on a mismatch while printing its JSON: read stdout whatever the exit status.
    const parity=lastJsonLine((await runPinned(pin,'parity',['--pdf',outputPath,'--assets',pin.assetsDir,...(type==='course_book'?[]:['--type',type])],json)).stdout);
    if(!parity||(parity.status!=='ok'&&parity.status!=='mismatch')||typeof parity.mismatches!=='number'||typeof parity.rawRn!=='number')throw Error('studyq_runner_failed');
    return {status:'rendered',pages:line.pages,sha256:line.sha256,outputPath,
      parityOk:parity.status==='ok'&&parity.mismatches===0&&parity.rawRn===0,parityMismatches:parity.mismatches,parityRawRn:parity.rawRn};
  }
  async function assemblePrint(input:unknown,outputPath:string):Promise<StudyqPrintResult>{
    if(!isAbsolute(outputPath))throw Error('output_path_invalid');
    const run=await runPinned(pin,'cli',['assemble-print','--out',outputPath],Buffer.from(JSON.stringify(input),'utf8'));
    const line=lastJsonLine(run.stdout);
    if(!line)throw Error('studyq_runner_failed');
    if(line.status==='error'&&typeof line.code==='string'&&/^[a-z_]{1,64}$/.test(line.code))return {status:'refused',code:line.code,detail:safeDetail(line.detail)};
    if(run.exitCode!==0||line.status!=='ok'||line.outputPath!==outputPath||typeof line.sha256!=='string'||!/^[a-f0-9]{64}$/.test(line.sha256)
      ||typeof line.pages!=='number'||!Number.isInteger(line.pages)||line.pages<1||typeof line.reportType!=='string'||!/^print_(exam|examtext|clinic)$/.test(line.reportType))throw Error('studyq_runner_failed');
    if(createHash('sha256').update(await readFile(outputPath)).digest('hex')!==line.sha256)throw Error('studyq_runner_failed');
    return {status:'rendered',reportType:line.reportType,pages:line.pages,sha256:line.sha256,outputPath};
  }
  return {renderCourseBook:(input,outputPath)=>render('course_book',input,outputPath),renderCism:(input,outputPath)=>render('cism',input,outputPath),
    renderReport:(reportType,input,outputPath)=>render(reportType,input,outputPath),assemblePrint};
}

/** `--studyq-runner <interpreter> <interpreterSha256> <studyqSrcDir> <studyqSourceSha256> <assetsDir>`:
 * every path absolute, both pins verified at launch and again before every spawn. */
export async function studyqRunnerConfiguration(args:readonly string[],env:Record<string,string|undefined>=process.env){
  const remainingArgs:string[]=[];let runner:StudyqRunner|null=null;
  for(let i=0;i<args.length;i++){
    if(args[i]!=='--studyq-runner'){remainingArgs.push(args[i]!);continue;}
    if(runner||i+5>=args.length)throw Error('studyq_runner_arguments_invalid');
    const [interpreter,interpreterSha256,srcDir,sourceSha256,assetsDir]=args.slice(i+1,i+6) as [string,string,string,string,string];
    if(![interpreter,srcDir,assetsDir].every(isAbsolute))throw Error('studyq_runner_arguments_invalid');
    await verifyInterpreter(interpreter,interpreterSha256,'studyq_interpreter_rejected');
    await verifyStudyqSourceTree(srcDir,sourceSha256);
    runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir,sourceSha256,assetsDir,printKeyFile:env.STUDYQ_PRINT_KEY_FILE||undefined});
    i+=5;
  }
  return {remainingArgs,runner};
}
