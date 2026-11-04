import {createHash} from 'node:crypto';
import {readFileSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {projectRoot} from '../src/modules';
interface File {module:string;path:string;sha256:string;bytes:number}
export function verifyImports(root=projectRoot){
 const manifest=JSON.parse(readFileSync(resolve(root,'provenance/imported-files.json'),'utf8')) as {files:File[]};
 // An adaptation names the original it diverges from; its current bytes are tracked by Git and PR review.
 const changes=JSON.parse(readFileSync(resolve(root,'provenance/adaptations.json'),'utf8')) as {files:{path:string;sourceSha256:string}[]};
 const adaptations=new Map(changes.files.map(file=>[file.path,file]));
 const hash=(value:Uint8Array)=>createHash('sha256').update(value).digest('hex'),fail=(code:string,path=''):never=>{throw Error(code+'@scripts/verify-imports.ts:1'+(path?' '+path:''));};
 let unchanged=0;
 for(const file of manifest.files){
  const p=file.module+'/'+file.path,adaptation=adaptations.get(p),path=resolve(root,p);
  if(!existsSync(path))fail('missing_import',p);
  if(adaptation){if(adaptation.sourceSha256!==file.sha256)fail('adaptation_source_mismatch',p);continue;}
  const actual=hash(readFileSync(path));
  if(actual!==file.sha256)fail('import_hash_mismatch',p);unchanged++;
 }
 return {scope:'imported-code-spec-bytes',unchanged,explicitAdaptations:[...adaptations.keys()],originalCustody:'external-private-bundles-and-source-tars'};
}
if(import.meta.main)console.log(JSON.stringify(verifyImports()));
