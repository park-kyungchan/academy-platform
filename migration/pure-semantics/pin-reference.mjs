// One-time source/dependency/corpus pin authoring. Refuses existing pins.
import {readFileSync, writeFileSync, readdirSync, existsSync, realpathSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {resolve, dirname, relative, join} from 'node:path';
import {fileURLToPath} from 'node:url';
const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, '../..');
const deps = process.argv[2];
if (!deps?.startsWith('/')) throw Error('existing_dependency_root_required');
const sha = v => createHash('sha256').update(v).digest('hex');
const entries = ['apps/academy/dc-gang-a/src/lms/lessonJournalTemplates.ts','apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts','apps/academy/dc-gang-a/src/learning/model.ts','modules/academy-workbench/tools/academy-mcp/src/dailyrecord-save-admission.ts','modules/academy-workbench/tools/academy-mcp/src/dailyrecord.ts','apps/academy/dc-gang-a/src/lms/lessonJournalHttp.ts','apps/academy/dc-gang-a/tests/lms/lessonJournalTemplates.test.ts'];
const closure = new Set();
function visit(file) {
  if (closure.has(file)) return;
  closure.add(file);
  const bytes = readFileSync(join(root, file));
  const original = spawnSync('git', ['show', 'cc6e81f1be2fcb56f011041e3d606e21edf76da3:' + file], {cwd:root,maxBuffer:4*1024*1024});
  if (original.status !== 0 || !bytes.equals(original.stdout)) throw Error('original_source_changed:' + file);
  const source = bytes.toString();
  const matches = [...source.matchAll(/(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s*)['"]([^'"]+)['"]/g)];
  for (const [, specifier] of matches) {
    if (!specifier.startsWith('.')) continue;
    const base = resolve(root, dirname(file), specifier);
    const path = [base, base+'.ts', base+'.tsx', join(base,'index.ts')].find(p => existsSync(p) && /\.[cm]?[jt]sx?$/.test(p));
    if (!path) throw Error('unresolved_source_import:' + specifier + '@' + file);
    visit(relative(root,path));
  }
}
entries.forEach(visit);
const zodRoot = realpathSync(join(deps,'zod'));
if (JSON.parse(readFileSync(join(zodRoot,'package.json'))).version !== '4.4.3') throw Error('zod_4_4_3_required');
function walk(dir, base = dir) {
  return readdirSync(dir,{withFileTypes:true}).flatMap(e => e.isDirectory() ? walk(join(dir,e.name),base) : e.isFile() && (e.name.endsWith('.js') || e.name === 'package.json') ? [relative(base,join(dir,e.name))] : []);
}
const files = [...closure].sort();
const pins = {schema:'s08.reference-pins/1', sourceBase:'cc6e81f1be2fcb56f011041e3d606e21edf76da3', prerequisite:'049f7c3da0af6c320c818d2afdab3da34bccd37a', zipSha256:'964845e260a25350823fb42d38a60e7cba973f8af607e10b505727d3bf78915c', bun:'1.4.3', node:'v24.22.1', zod:'4.4.3', sourceEntries:entries, sourceClosure:Object.fromEntries(files.map(f=>[f,sha(readFileSync(join(root,f)))])), zodFiles:Object.fromEntries(walk(zodRoot).sort().map(f=>[f,sha(readFileSync(join(zodRoot,f)))])), corpusSha256:sha(readFileSync(join(here,'corpus/cases.json')))};
writeFileSync(join(here,'pins.json'),JSON.stringify(pins,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({sourceFiles:files.length,zodFiles:Object.keys(pins.zodFiles).length}));
