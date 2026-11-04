// Additive correction: directly call hash exports even on values for which
// preliminary adapter JSON.stringify would throw before reaching the source.
import {resolve} from 'node:path';import {pathToFileURL} from 'node:url';
import {effects} from './isolation.ts';
const a=process.argv.slice(2),root=a[a.indexOf('--source-root')+1];if(!root?.startsWith('/'))throw Error('explicit_source_root_required');
const input=JSON.parse(await Bun.stdin.text()),events:any[]=[],sourceCalls:string[]=[];
let outcome;try{
 let value:any;if(input.tag==='cycle'){value={};value.self=value;}else if(input.tag==='array_cycle'){value=[];value.push(value);}else if(input.tag==='undefined')value=undefined;else if(input.tag==='depth512'){value=null;for(let i=0;i<512;i++)value=[value];}else throw Error('unknown_fixed_descriptor');
 const file=input.family==='cism'?'apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts':'modules/academy-workbench/tools/academy-mcp/src/dailyrecord-save-admission.ts';
 const m=await import(pathToFileURL(resolve(root,file)).href);
 if(!((input.family==='cism'&&input.fn==='cismHash')||(input.family==='admission'&&['canonicalJson','canonicalActionDigest'].includes(input.fn))))throw Error('direct_export_not_admitted');
 sourceCalls.push(input.family+'.'+input.fn);const result=m[input.fn](value);
 outcome={kind:'return',value:result===undefined?{$observed:'undefined'}:result};
}catch(e:any){outcome={kind:'throw',error:{name:e?.name??'NonError',message:e?.message??String(e)}};}
process.stdout.write(JSON.stringify({outcome,events,effects,metadata:{sourceCalls,uuidValues:[]}})+'\n');
