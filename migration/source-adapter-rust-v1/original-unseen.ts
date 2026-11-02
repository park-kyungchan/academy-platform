// Independent migration-only original observer for NEW fictional stimuli.
// Never consumed by the product Rust implementation.
import {effects,configureTrace} from '../source-adapter-reference-v1/guard.ts';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const [root,reference,inputPath,tracePath]=process.argv.slice(2);configureTrace(tracePath);
const pins=JSON.parse(readFileSync(resolve(reference,'SOURCE-PINS.json'),'utf8'));
for(const[p,v]of Object.entries(pins.sources)as any)if(createHash('sha256').update(readFileSync(resolve(root,p))).digest('hex')!==v.sha256)throw Error('unseen_original_source_changed');
const load=(p:string)=>import(pathToFileURL(resolve(root,p)).href);
const{sourceDomParser}=await load('apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
const{selectSourceFunction}=await load('apps/academy/dc-gang-a/src/lms/sourceFunctionText.ts');
const{loadSavedContext}=await load('modules/academy-workbench/tools/academy-mcp/src/saved-context.ts');
const input=JSON.parse(readFileSync(inputPath,'utf8'));const dom=[];
for(const html of input.dom){const parsed=sourceDomParser.parseWithSourceLocations(html),doc=parsed.document;const spans=[...doc.querySelectorAll('#repair,td,select,option,input,textarea,script#handler')].map((n:any)=>{const r=parsed.sourceRange(n);return{tag:n.tagName,id:n.id,value:['INPUT','SELECT','TEXTAREA','OPTION'].includes(n.tagName)?n.value:null,text:n.textContent,sourceRange:r,raw:r?html.slice(r.start,r.end):null};});dom.push({spans,duplicateIdCount:doc.querySelectorAll('#duplicate').length,firstDuplicateTag:doc.getElementById('duplicate')?.tagName,repairTbody:doc.querySelector('#repair tbody')?.tagName,syntheticTbodyRange:parsed.sourceRange(doc.querySelector('#repair tbody')),sourceFunction:selectSourceFunction(doc.querySelector('#handler')?.textContent??'','onFoPrg'),scriptExecuted:(doc.defaultView as any).s09ScriptRan===true,resourceCount:doc.querySelectorAll('[src]').length});}
const context=await loadSavedContext(input.savedRoot);const queries=[];for(const q of input.queries)try{queries.push({outcome:'returned',value:context.query(q)});}catch(e:any){queries.push({outcome:'threw',error:{name:e.name,message:e.message,...(e.issues?{issues:e.issues}:{}),...(e.code?{code:e.code}:{})}});}
process.stdout.write(JSON.stringify({dom,queries,effects})+'\n');
