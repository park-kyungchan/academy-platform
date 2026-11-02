import {readFileSync,writeFileSync,mkdirSync,mkdtempSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {resolve,join,isAbsolute} from 'node:path';
import {tmpdir} from 'node:os';
const args=process.argv.slice(2),arg=n=>args[args.indexOf(n)+1];
const binary=arg('--candidate'),casesPath=arg('--cases'),out=resolve(arg('--results-root'));
if(!args.includes('--candidate')||!args.includes('--cases')||!args.includes('--results-root')||!isAbsolute(binary))throw Error('absolute_candidate_cases_and_new_results_root_required');
const cases=JSON.parse(readFileSync(casesPath,'utf8')).cases;mkdirSync(out);
const temp=mkdtempSync(join(tmpdir(),'s08-admission-'));mkdirSync(join(temp,'home'));
const results=cases.map(c=>{
  const input=c.stdin_hex?Buffer.from(c.stdin_hex,'hex'):c.stdin_repeat?c.stdin_repeat.text.repeat(c.stdin_repeat.count):c.stdin;
  const r=spawnSync(binary,[],{input,encoding:'utf8',cwd:temp,env:{PATH:process.env.PATH,HOME:join(temp,'home'),TMPDIR:temp,TZ:'UTC'},timeout:10000,maxBuffer:4*1024*1024,killSignal:'SIGKILL'});
  const actual={command:[binary],status:r.status,signal:r.signal,spawnError:r.error?{code:r.error.code,message:r.error.message}:null,stdout:r.stdout??'',stderr:r.stderr??''};
  return {id:c.id,actual,passed:actual.status===c.expected.exit&&actual.signal===null&&actual.spawnError===null&&actual.stdout===c.expected.stdout&&actual.stderr===c.expected.stderr};
});
writeFileSync(join(out,'admission.json'),JSON.stringify(results,null,2)+'\n',{flag:'wx'});
const report={cases:results.length,passed:results.filter(r=>r.passed).length};writeFileSync(join(out,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(report));if(report.passed!==report.cases)process.exitCode=1;
