import {readFileSync,writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve,join} from 'node:path';
const read=p=>JSON.parse(readFileSync(p,'utf8')),sha=b=>createHash('sha256').update(b).digest('hex'),root=resolve('.'),ref='migration/pure-semantics/extension-v4';
const results=[];function pin(p,h){const actual=sha(readFileSync(p));results.push({path:p,expected:h,actual,matched:actual===h});}
for(const [p,h]of Object.entries(read('migration/pure-semantics/freeze.json').files))pin(p,h);
for(const [p,h]of Object.entries(read(ref+'/pins.json').sourceClosure))pin(p,h);
for(const [p,h]of Object.entries(read(ref+'/supplement-pins.json').sourceExtra))pin(p,h);
const artifacts=read(ref+'/artifact-pins-v2.json');
for(const [p,h]of Object.entries(artifacts.files??artifacts.artifacts??{}))pin(p.startsWith('migration/')?p:join(ref,p),h);
for(const [p,h]of [['expected-v2.json','9073faa3c80aced0bcc01f77ed99e4d1984b218134bc1d11910e15933d202578'],['extension-v4/expected.json','eaa5af1ca3f6d938cc8d31f0fba1e091605fc7bfda5976562a04318bc9eb454b'],['extension-v4/supplement-expected.json','4bfe0bd9fa9cdb670150a643bd4b5590781cc6f3899613195892b69d79bb806f'],['extension-v4/direct-expected-v2.json','24612df666a4c9e64e7e7f4a35d0a8cccfd2c91a99ffb89454dba8de5cdb9055']])pin('migration/pure-semantics/'+p,h);
const report={cwd:root,pins:results.length,passed:results.every(p=>p.matched),results};if(process.argv[2])writeFileSync(process.argv[2],JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({pins:report.pins,passed:report.passed}));if(!report.passed)process.exitCode=1;
