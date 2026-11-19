/** Bundles the saved endpoint (src/server.ts) and the dailyrecord endpoint (src/dailyrecord-server.ts)
 * for Bun into one directory: `<out>/server.js` and `<out>/dailyrecord-server.js`.
 * Usage: bun run build:endpoints <out dir>  (relative to this package's directory, or absolute)
 * A bundle resolves its data from `<out>/../../..` (src/paths.ts), so the only accepted out directory is
 * `modules/academy-workbench/.local/<name>/src/`. No network, no install, nothing written elsewhere. */
import {basename,dirname,resolve} from 'node:path';

const packageRoot=resolve(import.meta.dir,'..');
const workbenchRoot=resolve(packageRoot,'../..');
const args=process.argv.slice(2);
function refuse(code:string):never{process.stderr.write(code+'\n');process.exit(2);}
if(args.length!==1||!args[0])refuse('build_endpoints_usage: bun run build:endpoints <modules/academy-workbench/.local/<name>/src>');
const out=resolve(process.cwd(),args[0]);
const name=basename(dirname(out));
if(basename(out)!=='src'||dirname(dirname(out))!==resolve(workbenchRoot,'.local')||!/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(name))
 refuse('build_endpoints_out_dir_refused: the out directory must be modules/academy-workbench/.local/<name>/src');

const result=await Bun.build({
 entrypoints:[resolve(packageRoot,'src/server.ts'),resolve(packageRoot,'src/dailyrecord-server.ts')],
 root:resolve(packageRoot,'src'),outdir:out,target:'bun',
});
if(!result.success){for(const log of result.logs)console.error(String(log));refuse('build_endpoints_failed');}
console.log(JSON.stringify({outDir:`.local/${name}/src`,outputs:result.outputs.map(o=>basename(o.path)).sort()}));
