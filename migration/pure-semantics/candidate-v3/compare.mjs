import {readFileSync,writeFileSync,mkdirSync,mkdtempSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {resolve,dirname,join,isAbsolute} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {compare} from '../comparator.mjs';
import {requestFor} from './wire.mjs';
const here=dirname(fileURLToPath(import.meta.url)),frozenRoot=resolve(here,'..'),root=resolve(here,'../../..');
const args=process.argv.slice(2),arg=n=>args.includes(n)?args[args.indexOf(n)+1]:undefined;
const prepare=args.includes('--prepare-only'),binary=arg('--candidate'),output=arg('--results-root');
if(!output||(!prepare&&!isAbsolute(binary??'')))throw Error('absolute_candidate_and_new_results_root_required');
const json=p=>JSON.parse(readFileSync(p,'utf8')),hash=b=>createHash('sha256').update(b).digest('hex');
for(const [p,digest] of Object.entries(json(join(frozenRoot,'freeze.json')).files))if(hash(readFileSync(join(root,p)))!==digest)throw Error('s08_frozen_input_changed:'+p);
for(const [p,digest] of Object.entries(json(join(frozenRoot,'pins.json')).sourceClosure))if(hash(readFileSync(join(root,p)))!==digest)throw Error('s08_original_source_changed:'+p);
const cases=json(join(frozenRoot,'corpus/cases.json')),expected=json(join(frozenRoot,'expected-v2.json'));
if(cases.length!==202||hash(readFileSync(join(frozenRoot,'expected-v2.json')))!=='9073faa3c80aced0bcc01f77ed99e4d1984b218134bc1d11910e15933d202578')throw Error('s08_reference_identity_rejected');
const out=resolve(output);mkdirSync(out);mkdirSync(join(out,'cases'));
const temp=mkdtempSync(join(tmpdir(),'s08-v3-'));mkdirSync(join(temp,'home'));
const report={schema:'s08.candidate-v3/1',scope:'202 frozen selected source cases; no S09/S10/P8/live claim',mode:prepare?'lowering_only_no_candidate_execution':'actual_candidate',candidate:binary??null,candidateSha256:prepare?null:hash(readFileSync(binary)),expectedSha256:hash(readFileSync(join(frozenRoot,'expected-v2.json'))),cases:cases.length,matched:0,prepared:0,comparisons:[]};
for(const c of cases){
  const input=JSON.stringify(requestFor(c.input));if(Buffer.byteLength(input)>1048576)throw Error('s08_wire_exceeds_cli_bound:'+c.id);
  if(prepare){report.prepared++;report.comparisons.push({id:c.id,inputBytes:Buffer.byteLength(input)});continue;}
  const r=spawnSync(binary,[],{input,encoding:'utf8',cwd:temp,env:{PATH:process.env.PATH,HOME:join(temp,'home'),TMPDIR:temp,TZ:'UTC'},timeout:10000,maxBuffer:4*1024*1024,killSignal:'SIGKILL'});
  const actual={command:[binary],cwd:temp,status:r.status,signal:r.signal,spawnError:r.error?{code:r.error.code,message:r.error.message}:null,stdout:r.stdout??'',stderr:r.stderr??''};
  const comparison=compare(expected[c.id],actual);if(comparison.accepted)report.matched++;
  const artifact='case-'+report.comparisons.length+'-'+encodeURIComponent(c.id)+'.json';
  writeFileSync(join(out,'cases',artifact),JSON.stringify({id:c.id,input,actual,comparison},null,2)+'\n',{flag:'wx'});
  report.comparisons.push({id:c.id,artifact:'cases/'+artifact,...comparison});
}
writeFileSync(join(out,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({mode:report.mode,cases:report.cases,prepared:report.prepared,matched:report.matched,results:out}));
if(!prepare&&report.matched!==report.cases)process.exitCode=1;
