/** Builds release/surface.json (academy.mcp-surface/1) from src/consolidated-tools.ts: the 15 tools of
 * specs/academy-mcp/tool-surface.md. Deterministic; tests/tool-surface.test.ts checks the file equals this output.
 * Usage: bun scripts/build-surface.ts [--check] */
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {surfaceDocument} from '../src/consolidated-tools.ts';

export const surfacePath=resolve(import.meta.dir,'../release/surface.json');
export const surfaceText=()=>JSON.stringify(surfaceDocument(),null,1)+'\n';
if(import.meta.main){
  const args=process.argv.slice(2);
  if(args.length>1||(args.length===1&&args[0]!=='--check')){console.error('usage: bun scripts/build-surface.ts [--check]');process.exit(2);}
  if(args[0]==='--check'){if(readFileSync(surfacePath,'utf8')!==surfaceText()){console.error('surface_json_stale: run bun scripts/build-surface.ts');process.exit(1);}}
  else writeFileSync(surfacePath,surfaceText());
}
