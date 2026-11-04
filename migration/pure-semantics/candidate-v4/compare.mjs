import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {judgeSuite} from '../extension-v4/judge.mjs';
import {requestFor} from './driver.mjs';
const here=dirname(fileURLToPath(import.meta.url)),root=resolve(here,'../../..'),reference=resolve(here,'../extension-v4');
const args=process.argv.slice(2),get=k=>args[args.indexOf(k)+1];
const binary=resolve(get('--candidate')),output=resolve(get('--results'));mkdirSync(output);mkdirSync(join(output,'cases'));
const sha=b=>createHash('sha256').update(b).digest('hex'),read=p=>JSON.parse(readFileSync(p,'utf8')),write=(p,v)=>writeFileSync(p,JSON.stringify(v,null,2)+'\n',{flag:'wx'});
const suites=[['main','cases.json','expected.json','eaa5af1ca3f6d938cc8d31f0fba1e091605fc7bfda5976562a04318bc9eb454b'],['supplement','supplement-cases.json','supplement-expected.json','4bfe0bd9fa9cdb670150a643bd4b5590781cc6f3899613195892b69d79bb806f'],['direct','direct-cases-v2.json','direct-expected-v2.json','24612df666a4c9e64e7e7f4a35d0a8cccfd2c91a99ffb89454dba8de5cdb9055']];
const report={candidate:binary,binarySha256:sha(readFileSync(binary)),suites:[],cases:0,matched:0};
for(const [name,c,e,expectedSha]of suites){const cases=read(join(reference,c)),bytes=readFileSync(join(reference,e));if(sha(bytes)!==expectedSha)throw Error('reference_pin_changed');const expected=JSON.parse(bytes),actual=[];
 for(const test of cases){const request=requestFor(test.input,test.script??(name==='direct'?'direct-baseline-v2.ts':'baseline.ts'),root),input=JSON.stringify(request);const r=spawnSync(binary,[],{input,encoding:'utf8',cwd:output,env:{PATH:process.env.PATH,TZ:'UTC'},timeout:10000,maxBuffer:8388608,killSignal:'SIGKILL'});const a={status:r.status,signal:r.signal,spawnError:r.error?{code:r.error.code,message:r.error.message}:null,stdout:r.stdout??'',stderr:r.stderr??''};actual.push({id:test.id,actual:a});write(join(output,'cases',name+'-'+encodeURIComponent(test.id)+'.json'),{id:test.id,cwd:output,argv:[binary],input,actual:a});}
 const comparison=judgeSuite(cases,expected,actual);write(join(output,name+'-comparison.json'),comparison);const matched=comparison.comparisons?.filter(c=>c.accepted).length??0;report.cases+=cases.length;report.matched+=matched;report.suites.push({name,cases:cases.length,matched,expectedSha256:expectedSha,accepted:comparison.accepted});
}
write(join(output,'report.json'),report);console.log(JSON.stringify(report));if(report.matched!==603)process.exitCode=1;
