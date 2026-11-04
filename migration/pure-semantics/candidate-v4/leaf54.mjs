// Future leaf-only command. Does not execute or regenerate the source oracle.
import {readFileSync, writeFileSync, mkdirSync, mkdtempSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {resolve, dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {compare} from '../comparator.mjs';
const here=resolve(dirname(fileURLToPath(import.meta.url)),'..'),root=resolve(here,'../..'),args=process.argv.slice(2),arg=n=>args[args.indexOf(n)+1];
if(!args.includes('--candidate')||!args.includes('--results-root')||!arg('--candidate')?.startsWith('/'))throw Error('absolute_candidate_and_new_results_root_required');
const json=p=>JSON.parse(readFileSync(p,'utf8')),hash=b=>createHash('sha256').update(b).digest('hex');
const frozen=json(join(here,'freeze.json'));
for(const [path,digest] of Object.entries(frozen.files))if(hash(readFileSync(join(root,path)))!==digest)throw Error('s08_frozen_input_changed:'+path);
const cases=json(join(here,'corpus/cases.json')).filter(c=>c.leaf),expected=json(join(here,'expected-v2.json'));
const candidate=arg('--candidate'),out=resolve(arg('--results-root'));mkdirSync(out);
const temp=mkdtempSync(join(out,'workspace-'));mkdirSync(join(temp,'home'));
const results=cases.map(c=>{
 const r=spawnSync(candidate,[],{input:JSON.stringify(c.input),encoding:'utf8',cwd:temp,env:{PATH:process.env.PATH,HOME:join(temp,'home'),TMPDIR:temp,TZ:'UTC'},timeout:10000,maxBuffer:4*1024*1024,killSignal:'SIGKILL'});
 const actual={command:[candidate],cwd:temp,status:r.status,signal:r.signal,spawnError:r.error?{code:r.error.code,message:r.error.message}:null,stdout:r.stdout??'',stderr:r.stderr??''};
 return {id:c.id,actual,comparison:compare(expected[c.id],actual)};
});
writeFileSync(join(out,'candidate.json'),JSON.stringify(results,null,2)+'\n',{flag:'wx'});
const report={candidate,candidateSha256:hash(readFileSync(candidate)),expectedSha256:hash(readFileSync(join(here,'expected-v2.json'))),scope:'54 finite width/calendar cases only; not full S08',cases:results.length,matched:results.filter(r=>r.comparison.accepted).length};
writeFileSync(join(out,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(report));
if(report.matched!==report.cases)process.exitCode=1;
