// Permitted author checks only. There is intentionally no Rust compiler command.
import {spawnSync} from 'node:child_process';import {writeFileSync} from 'node:fs';import {resolve,dirname,join} from 'node:path';import {fileURLToPath} from 'node:url';
const here=dirname(fileURLToPath(import.meta.url)),root=resolve(here,'../../..');
const commands=[
 ['cargo','fmt','--all','--check'],
 ['cargo','metadata','--locked','--offline','--no-deps','--format-version','1'],
 ['node','--test','migration/pure-semantics/candidate-v3/wire.test.mjs'],
 ['node','migration/pure-semantics/candidate-v3/source-data.mjs'],
 ...['wire.mjs','compare.mjs','admission.mjs','make-admission.mjs','source-data.mjs','static.mjs'].map(f=>['node','--check','migration/pure-semantics/candidate-v3/'+f]),
 ['bash','-n','migration/pure-semantics/candidate-v3/LEAD-COMMANDS.sh'],
 ['git','diff','--cached','--check'],
 ['git','diff','--check'],
];
const results=commands.map(command=>{const r=spawnSync(command[0],command.slice(1),{cwd:root,env:{...process.env,RUSTUP_TOOLCHAIN:'1.95.0'},encoding:'utf8',timeout:30000,maxBuffer:4*1024*1024});return {command,cwd:root,status:r.status,signal:r.signal,spawnError:r.error?{code:r.error.code,message:r.error.message}:null,stdout:r.stdout??'',stderr:r.stderr??''};});
const passed=results.every(r=>r.status===0&&r.signal===null&&r.spawnError===null);
writeFileSync(join(here,'checks.json'),JSON.stringify({schema:'s08.candidate-static/3',rustCompilation:'not_run_no_compiler_slot',nativeCandidateComparison:'not_run',rustTests:'seven_tests_written_not_run',initialFmtFailure:{status:1,code:'mismatched_closing_delimiter',location:'crates/edu-academy-core/src/semantic.rs:39',resolution:'Corrected the missing closing parenthesis before the final checks; tool transcript retains the original stderr.'},passed,results},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({passed,commands:results.length,compilation:'not_run',failed:results.filter(r=>r.status!==0).map(r=>r.command)}));if(!passed)process.exitCode=1;
