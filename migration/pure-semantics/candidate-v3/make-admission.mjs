// Candidate-owned proposed adapter refusals; never an original-source oracle.
import {readFileSync,writeFileSync} from 'node:fs';import {dirname,join} from 'node:path';import {fileURLToPath} from 'node:url';import {lower} from './wire.mjs';
const here=dirname(fileURLToPath(import.meta.url)),cases=[];
function add(id,input,message='s08_protocol_input_rejected'){cases.push({id,stdin:typeof input==='string'?input:JSON.stringify(input),expected:{exit:64,stdout:'',stderr:message+'\n'}});}
add('unknown-envelope-field',{...lower({op:'constants'}),extra:true});
add('duplicate-envelope-op','{"op":"semantic_v3","op":"semantic_v3","wire":{"root":0,"nodes":[{"kind":"null"}]}}');
add('invalid-root-index',{op:'semantic_v3',wire:{root:5,nodes:[]}});
add('invalid-code-unit',{op:'semantic_v3',wire:{root:0,nodes:[{kind:'string',units:[65536]}]}});
add('invalid-binary64',{op:'semantic_v3',wire:{root:0,nodes:[{kind:'number',bits:'NaN'}]}});
add('cyclic-wire-graph',{op:'semantic_v3',wire:{root:0,nodes:[{kind:'array',items:[0]}]}});
for(const family of ['admission','cism'])for(const tag of ['cycle','depth_65'])add(family+'-'+tag,lower({op:'json',family,value:{$js:tag}}),'s08_semantic_admission_required');
const review=JSON.parse(readFileSync(join(here,'../corpus/cases.json'),'utf8')).find(c=>c.input.op==='review').input;
const noPorts=lower(review);delete noPorts.ports;add('review-missing-provider',noPorts,'s08_semantic_admission_required');
const badPorts=lower(review);badPorts.ports.uuid='production-default';add('review-invalid-provider',badPorts,'s08_semantic_admission_required');
const offset=structuredClone(review);offset.observation.observedAt='2040-01-16T18:00:00+09:00';add('review-offset-instant-needs-source-extension',lower(offset),'s08_semantic_admission_required');
writeFileSync(join(here,'admission-v3.json'),JSON.stringify({scope:'Proposed new V3 adapter admission. Independent review/Lead acceptance pending; no source refusals claimed.',cases},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({cases:cases.length,mode:'literal_inputs_only_no_candidate_execution'}));
