import {readFileSync,writeFileSync,mkdirSync,copyFileSync,symlinkSync,realpathSync,existsSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {judge,judgeSuite,frameValid} from './judge.mjs';
const here=dirname(fileURLToPath(import.meta.url)),root=resolve(here,'../../..'),args=process.argv.slice(2);
const get=k=>{if(!args.includes(k))throw Error('missing_argument:'+k);return args[args.indexOf(k)+1];};
const output=resolve(get('--results')),deps=realpathSync(get('--deps-root')),freeze=args.includes('--freeze');
if(!output.startsWith(here+'/raw/'))throw Error('results_must_be_new_extension_raw_directory');
const read=p=>JSON.parse(readFileSync(p,'utf8')),write=(p,v)=>writeFileSync(p,JSON.stringify(v,null,2)+'\n',{flag:'wx'}),sha=v=>createHash('sha256').update(v).digest('hex');
const pins=read(join(here,'pins.json')),cases=read(join(here,'cases.json'));
assert.equal(sha(readFileSync(join(here,'cases.json'))),pins.casesSha256);
assert.equal(process.version,pins.node);
for(const [p,h]of Object.entries(pins.sourceClosure)){
 const bytes=readFileSync(join(root,p));assert.equal(sha(bytes),h,p);
 const g=spawnSync('git',['show',pins.originalSource+':'+p],{cwd:root,maxBuffer:8*1024*1024});assert.equal(g.status,0);assert.ok(bytes.equals(g.stdout),'original:'+p);
}
const zod=realpathSync(join(deps,'zod'));for(const [p,h]of Object.entries(pins.zodFiles))assert.equal(sha(readFileSync(join(zod,p))),h,'zod:'+p);
for(const [p,h]of Object.entries(pins.retainedPins))assert.equal(sha(readFileSync(join(root,p))),h,p);
if(!freeze){const f=read(join(here,'freeze.json'));for(const [p,h]of Object.entries(f.files))assert.equal(sha(readFileSync(join(here,p))),h,p);}
mkdirSync(output);const workspace=join(output,'workspace'),sourceRoot=join(workspace,'source'),home=join(workspace,'home');mkdirSync(home,{recursive:true});
for(const p of Object.keys(pins.sourceClosure)){mkdirSync(dirname(join(sourceRoot,p)),{recursive:true});copyFileSync(join(root,p),join(sourceRoot,p));}
mkdirSync(join(sourceRoot,'node_modules'));symlinkSync(zod,join(sourceRoot,'node_modules/zod'));
const env={PATH:process.env.PATH,HOME:home,TMPDIR:workspace,TZ:'UTC',LANG:'C.UTF-8',BUN_CONFIG_NO_AUTO_INSTALL:'1'};
function run(command,argv,input=''){
 const r=spawnSync(command,argv,{input,encoding:'utf8',cwd:workspace,env,timeout:5000,killSignal:'SIGKILL',maxBuffer:8*1024*1024});
 return {command:[command,...argv],cwd:workspace,status:r.status,signal:r.signal,spawnError:r.error?{code:r.error.code,message:r.error.message}:null,stdout:r.stdout??'',stderr:r.stderr??''};
}
const version=run('bun',['--version']);assert.equal(version.status,0);assert.equal(version.stdout.trim(),pins.bun);
const invoke=input=>run('bun',['--no-env-file','--no-install',join(here,'baseline.ts'),'--source-root',sourceRoot],JSON.stringify(input));
const raw=[];for(const c of cases){const actual=invoke(c.input);raw.push({id:c.id,actual});}
write(join(output,'reference.json'),raw);
let expected;
if(freeze){
 expected={};const uncharacterized=[];
 for(const r of raw){
  let frame;try{frame=JSON.parse(r.actual.stdout);}catch{}
  if(!frame||!judge(frame,r.actual).accepted){uncharacterized.push({id:r.id,status:'UNCHARACTERIZED',actual:r.actual});continue;}
  expected[r.id]=frame;
 }
 write(join(output,'uncharacterized.json'),uncharacterized);
 if(uncharacterized.length){write(join(output,'report.json'),{passed:false,uncharacterized:uncharacterized.length});throw Error('source_call_uncharacterized_see_raw');}
 write(join(here,'expected.json'),expected);
}else expected=read(join(here,'expected.json'));
const suite=judgeSuite(cases,expected,raw);
write(join(output,'comparison.json'),suite);
const control=[];
const scalar=expected['learning-text-44']??expected[cases.find(c=>c.input.op==='learning'&&c.input.fn==='requireText').id];
const base=cases.find(c=>c.id==='learning-stableId-opaque'),exp=expected[base.id];
for(const c of ['network','dispatch','stderr','stdout','nonzero']){const actual=invoke({...base.input,control:c});control.push({id:c,actual,judgment:judge(exp,actual)});}
function child(id,frame,text=null,extra=''){
 const actual=run('node',['-e','let x=require("node:fs").readFileSync(0,"utf8");process.stdout.write(x);'+extra],text??JSON.stringify(frame)+'\n');
 control.push({id,actual,judgment:judge(exp,actual)});
}
function mutate(id,fn,e=exp){const x=structuredClone(e);fn(x);const actual=run('node',['-e','process.stdout.write(require("node:fs").readFileSync(0,"utf8"))'],JSON.stringify(x)+'\n');control.push({id,actual,judgment:judge(e,actual)});}
mutate('wrong-value',x=>x.outcome.value='plausible-wrong-id');mutate('missing-outcome',x=>delete x.outcome);
mutate('missing-events',x=>delete x.events);mutate('missing-presence',x=>delete x.outcome.value);
mutate('extra-field',x=>x.extra=true);mutate('bool-number-effect',x=>x.effects.deniedTransportAttempts=false);
mutate('bool-number-value',x=>x.outcome.value=true,expected['learning-knowledge-74']??expected[cases.find(c=>c.input.fn==='studentOf'&&c.input.args[0]?.studentId===0).id]);
mutate('null-missing',x=>delete x.outcome.value.review.cismSource,expected['review-instant-0']);
mutate('wrong-error-name',x=>x.outcome.error.name='Error',expected['review-unknown-context-key']);
mutate('wrong-error-issues',x=>x.outcome.error.issues.reverse(),expected['review-multiple-errors']);
mutate('wrong-error-message',x=>x.outcome.error.message='generic',expected['review-unknown-context-key']);
mutate('digest-change',x=>x.outcome.value.sha256='0'.repeat(64),expected['json-admission-depth-65']);
mutate('bytes-change',x=>x.outcome.value.bytes+=' ',expected['json-admission-depth-65']);
mutate('unintended-dispatch-event',x=>x.events.push({event:'http_dispatch'}));
mutate('event-order',x=>x.events.reverse(),expected['admission-once-and-repeat']);
mutate('uuid-metadata-missing',x=>delete x.metadata.uuidValues);
child('duplicate-key',null,JSON.stringify(exp).replace('"events":[]','"events":[1],"events":[]')+'\n');
child('duplicate-nested-key',null,JSON.stringify(exp).replace('"deniedTransportAttempts":0','"deniedTransportAttempts":1,"deniedTransportAttempts":0')+'\n');
child('multi-frame',null,JSON.stringify(exp)+'\n'+JSON.stringify(exp)+'\n');
child('whitespace',null,' '+JSON.stringify(exp)+'\n');
child('nonzero-plausible',exp,null,'process.exitCode=9;');
const suiteControls=[
 {id:'missing-case',value:judgeSuite(cases,expected,raw.slice(1))},
 {id:'duplicate-case',value:judgeSuite(cases,expected,[raw[0],...raw])},
 {id:'reordered-case',value:judgeSuite(cases,expected,[raw[1],raw[0],...raw.slice(2)])},
 {id:'extra-expected',value:judgeSuite(cases,{...expected,extra:exp},raw)},
 {id:'missing-expected',value:judgeSuite(cases,Object.fromEntries(Object.entries(expected).slice(1)),raw)},
 {id:'bool-status',value:judge(exp,{...raw[0].actual,status:false})},
 {id:'string-status',value:judge(exp,{...raw[0].actual,status:'0'})},
 {id:'missing-signal',value:judge(exp,{...raw[0].actual,signal:undefined})},
];
write(join(output,'qualification.json'),{children:control,strictProtocolAndScope:suiteControls});
const report={schema:'s08.extension-source-results/4',candidateExecuted:false,independentReview:'REQUIRED_NOT_PERFORMED',cases:cases.length,matched:suite.comparisons?.filter(x=>x.accepted).length??0,sourceChildStatuses:Object.fromEntries([...new Set(raw.map(x=>String(x.actual.status)))].map(s=>[s,raw.filter(x=>String(x.actual.status)===s).length])),sourceSpawnFailures:raw.filter(x=>x.actual.spawnError).length,childControls:control.length,rejectedChildControls:control.filter(x=>!x.judgment.accepted).length,strictControls:suiteControls.length,rejectedStrictControls:suiteControls.filter(x=>!x.value.accepted).length,expectedSha256:sha(readFileSync(join(here,'expected.json'))),bunVersion:version,sourceRoot,ports:{clock:'explicit per review case before service/intake',uuid:'sequential mock.module before unchanged source import'},passed:suite.accepted&&control.every(x=>!x.judgment.accepted)&&suiteControls.every(x=>!x.value.accepted)};
write(join(output,'report.json'),report);console.log(JSON.stringify({passed:report.passed,cases:report.cases,matched:report.matched,childControls:report.childControls,rejectedChildControls:report.rejectedChildControls,strictControls:report.strictControls,results:output}));
if(!report.passed)process.exitCode=1;
