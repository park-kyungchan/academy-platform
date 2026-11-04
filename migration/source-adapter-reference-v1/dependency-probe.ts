import './guard.ts';
import {readFileSync,existsSync,realpathSync,writeFileSync} from 'node:fs';
import {dirname,resolve,join,isAbsolute} from 'node:path';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const [root,output]=process.argv.slice(2),sha=(p:string)=>createHash('sha256').update(readFileSync(p)).digest('hex');
await import(pathToFileURL(resolve(root,'apps/academy/dc-gang-a/src/lms/academyHttpClient.ts')).href);
await import(pathToFileURL(resolve(root,'modules/academy-workbench/tools/academy-mcp/src/native-dailyrecord-backend.ts')).href);
const cache=Object.keys(require.cache),packages=new Map<string,any>();
// Inspect only packages actually present in this selected import's module cache.
for(const file of cache){if(!file.includes('/node_modules/'))continue;let dir=dirname(file);
 while(dir.includes('/node_modules/')){const p=join(dir,'package.json');if(existsSync(p)){const meta=JSON.parse(readFileSync(p,'utf8'));if(meta.name&&meta.version){if(!packages.has(realpathSync(p))){let entry=file;try{const candidate=createRequire(p).resolve(meta.name);if(isAbsolute(candidate))entry=candidate;}catch{}
 packages.set(realpathSync(p),{name:meta.name,version:meta.version,packageJson:realpathSync(p),packageSha256:sha(p),resolvedEntry:realpathSync(entry),entrySha256:sha(entry),observedLoadedFile:realpathSync(file),observedLoadedFileSha256:sha(file),observedModuleCount:0});}packages.get(realpathSync(p)).observedModuleCount++;break;}}dir=dirname(dir);}}
const lock=readFileSync(resolve(root,'apps/academy/dc-gang-a/bun.lock'),'utf8')+readFileSync(resolve(root,'modules/academy-workbench/tools/academy-mcp/bun.lock'),'utf8');
const entries=[...packages.values()].sort((a,b)=>a.name.localeCompare(b.name));for(const e of entries)e.lockContainsExactVersion=lock.includes(e.name+'@'+e.version);
const result={runtime:Bun.version,moduleCacheFiles:cache.length,packages:entries};writeFileSync(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({packages:entries.length,lockMismatches:entries.filter(e=>!e.lockContainsExactVersion).map(e=>e.name)}));
