// Inspection only: transpile selected imports without evaluating original modules.
import {readFileSync,writeFileSync,existsSync,mkdirSync,symlinkSync,realpathSync} from 'node:fs';
import {resolve,dirname,relative} from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
const root=process.cwd(),out=resolve(root,'migration/source-adapter-reference-v1');
if(existsSync(resolve(out,'SOURCE-PINS.json')))throw Error('s09_source_pins_already_frozen');
const sha=(p:string)=>createHash('sha256').update(readFileSync(p)).digest('hex');
const entries=['apps/academy/dc-gang-a/src/lms/academyHttpClient.ts','apps/academy/dc-gang-a/src/lms/testPoolPresentationProtocol.ts','apps/academy/dc-gang-a/src/lms/sourceFunctionText.ts','modules/academy-workbench/tools/academy-mcp/src/native-dailyrecord-backend.ts','modules/academy-workbench/tools/academy-mcp/src/saved-context.ts','modules/academy-workbench/tools/academy-mcp/src/pagination.ts','modules/academy-workbench/tools/academy-mcp/src/paths.ts','modules/academy-workbench/tools/academy-mcp/src/student-resolver.ts'];
const transpiler=new Bun.Transpiler({loader:'ts',target:'bun'}),sources:Record<string,any>={},externals=new Set<string>();
function walk(path:string){if(sources[path])return;const text=readFileSync(resolve(root,path),'utf8');const js=transpiler.transformSync(text);const imports=transpiler.scanImports(js).map(x=>x.path);sources[path]={sha256:sha(resolve(root,path)),bytes:Buffer.byteLength(text),runtimeImports:imports};for(const spec of imports){if(spec.startsWith('.')){let p=resolve(root,dirname(path),spec);if(!existsSync(p))p+='.ts';if(!p.startsWith(root+'/'))throw Error('source_outside_root');walk(relative(root,p));}else externals.add(spec);}}
entries.forEach(walk);
const dependencies=[];
for(const [dir,names] of [['apps/academy/dc-gang-a',['jsdom','acorn']],['modules/academy-workbench/tools/academy-mcp',['zod']]] as const){
 const manifest=JSON.parse(readFileSync(resolve(root,dir,'package.json'),'utf8'));const lock=readFileSync(resolve(root,dir,'bun.lock'),'utf8');
 const req=createRequire('/home/palantirkc/projects/education-platform/'+dir+'/package.json');
 for(const name of names){const packagePath=req.resolve(name+'/package.json'),p=JSON.parse(readFileSync(packagePath,'utf8'));
 if(p.version!==manifest.dependencies[name]||!lock.includes(name+'@'+p.version))throw Error('installed_dependency_mismatch');
 const target=realpathSync(dirname(packagePath)),entry=req.resolve(name),esm=Bun.resolveSync(name,dirname(packagePath));
 const link=resolve(root,dir,'node_modules',name);mkdirSync(dirname(link),{recursive:true});if(existsSync(link)){if(realpathSync(link)!==target)throw Error('foreign_dependency_link');}else symlinkSync(target,link,'dir');
 dependencies.push({name,version:p.version,link:relative(root,link),target,packagePath,packageSha256:sha(packagePath),requireEntry:entry,requireEntrySha256:sha(entry),bunResolvedEntry:esm,bunResolvedEntrySha256:sha(esm)});
 }
 for(const f of ['package.json','bun.lock'])sources[dir+'/'+f]={sha256:sha(resolve(root,dir,f)),bytes:readFileSync(resolve(root,dir,f)).length,role:'dependency_contract'};
}
const leads=['modules/academy-workbench/tools/academy-mcp/src/service.ts','modules/academy-workbench/tools/academy-mcp/src/query-tool.ts','modules/academy-workbench/tools/academy-mcp/src/dailyrecord-read.ts','modules/academy-workbench/tools/academy-mcp/tests/native-dailyrecord-backend.test.ts','modules/academy-workbench/tools/academy-mcp/tests/fixtures.ts','apps/academy/dc-gang-a/tests/lms/testPoolPresentationProtocol.test.ts','apps/academy/dc-gang-a/tests/lms/dayRecordGroupDiscovery.test.ts','migration/recorded-lookup/pins.json','migration/recorded-lookup/expected.json','migration/recorded-lookup/README.md'];
const old=JSON.parse(readFileSync(resolve(root,'migration/recorded-lookup/pins.json'),'utf8'));const drift=Object.entries(sources).filter(([p])=>old.files[p]).map(([p,v])=>({path:p,oldSha256:old.files[p],currentSha256:v.sha256,changed:old.files[p]!==v.sha256}));
writeFileSync(resolve(out,'SOURCE-PINS.json'),JSON.stringify({schema:'s09.original-source-pins/1',base:'39441f5d205b8a67e5c911da747d213a322d20da',entries,sources,externals:[...externals].sort(),dependencies,readOnlyLeads:Object.fromEntries(leads.map(p=>[p,{sha256:sha(resolve(root,p)),bytes:readFileSync(resolve(root,p)).length}])),s05Drift:drift},null,2)+'\n');
console.log(JSON.stringify({selectedFiles:Object.keys(sources).length,dependencies:dependencies.map(d=>({name:d.name,version:d.version,link:d.link})),externals:[...externals].sort(),s05Drift:drift.filter(d=>d.changed)}));
