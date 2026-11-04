// Migration observer only. Resolution uses existing installed dependencies;
// source modules and protocol handlers are loaded unchanged.
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import {resolve, isAbsolute} from 'node:path';
import {syncBuiltinESMExports} from 'node:module';
import {createHash} from 'node:crypto';
const root=process.env.S11_SOURCE_ROOT!, out=process.env.S11_MODE_ROOT!;
const deps=process.env.S11_DEPENDENCY_ROOT!;
if(![root,out,deps].every(x=>x&&isAbsolute(x)))throw Error('s11_explicit_roots_required');
const append=fs.appendFileSync.bind(fs), mkdir=fs.mkdirSync.bind(fs), write=fs.writeFileSync.bind(fs);
const dir=resolve(out,'processes',String(process.pid));mkdir(dir,{recursive:true,mode:0o700});
const selfStat=fs.readFileSync('/proc/self/stat','utf8').split(')').at(-1)!.trim().split(/\s+/);
write(resolve(dir,'START.json'),JSON.stringify({pid:process.pid,ppid:process.ppid,pgid:Number(selfStat[2]),uid:process.getuid?.(),argv:process.argv,cwd:process.cwd(),startedAt:new Date().toISOString(),execPath:process.execPath})+'\n',{mode:0o600});
const hash=(b:Uint8Array)=>createHash('sha256').update(b).digest('hex');
const pin=(path:string,kind:string)=>{const b=fs.readFileSync(path);append(resolve(dir,'loads.ndjson'),JSON.stringify({path,kind,sha256:hash(b),bytes:b.length})+'\n');};
for(const name of ['stdout','stderr'] as const){const stream=process[name],original=stream.write.bind(stream);stream.write=((chunk:any,...args:any[])=>{append(resolve(dir,name+'.bin'),typeof chunk==='string'?Buffer.from(chunk,typeof args[0]==='string'?args[0]:undefined):chunk);return original(chunk,...args as []);}) as any;}
// Observe data at the point Node emits it; do not add a data listener or start flow.
const emit=process.stdin.emit.bind(process.stdin);
process.stdin.emit=((event:any,...args:any[])=>{if(event==='data')append(resolve(dir,'stdin.bin'),args[0]);return emit(event,...args);}) as any;
process.on('exit',code=>write(resolve(dir,'EXIT.json'),JSON.stringify({code,at:new Date().toISOString()})+'\n'));
const allowed=(p:any)=>{if(typeof p==='number')return;const s=resolve(String(p));if(s!==out&&!s.startsWith(out+'/'))throw Error('s11_write_outside_disposable_root');};
for(const key of ['writeFileSync','appendFileSync','mkdirSync','rmSync','unlinkSync','chmodSync','rmdirSync','truncateSync'] as const){const fn=(fs as any)[key].bind(fs);(fs as any)[key]=(p:any,...args:any[])=>{allowed(p);return fn(p,...args);};}
for(const key of ['writeFile','appendFile','mkdir','rm','unlink','chmod','rmdir','truncate'] as const){const fn=(fsp as any)[key].bind(fsp);(fsp as any)[key]=(p:any,...args:any[])=>{allowed(p);return fn(p,...args);};}
for(const api of [fs,fsp] as any[]){const key=api===fs?'renameSync':'rename',fn=api[key].bind(api);api[key]=(a:any,b:any,...args:any[])=>{allowed(a);allowed(b);return fn(a,b,...args);};}
syncBuiltinESMExports();
await import(root+'/modules/academy-workbench/tools/academy-mcp/tests/network-deny.ts');
const deny=()=>{throw Error('s11_network_forbidden');};
Bun.connect=deny as any;Bun.listen=deny as any;Bun.serve=deny as any;
// Dependency resolution is native: an unchanged-byte disposable source mirror
// has one read-only locator symlink to the exact pre-pinned installed modules.
// No runtime import/plugin rewriting is used.
pin(process.argv[1],'entry');
