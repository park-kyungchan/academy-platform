import {createHash} from 'node:crypto';
import {constants} from 'node:fs';
import {open,realpath,stat} from 'node:fs/promises';
import {isAbsolute,join} from 'node:path';

// V1 has no bridge pin in its strict pointer. A release-owned immutable
// bridge-pin.json supplies that pin until the complete V2 graph is available.
const base=process.env.ACADEMY_MCP_INSTALL_DIR ?? '/root/.local/share/academy-mcp-scoped-20261030';
const digest=(bytes:Uint8Array)=>createHash('sha256').update(bytes).digest('hex');
async function qualifiedFile(path:string,max:number){
  if(await realpath(path)!==path)throw Error('launcher_path_unqualified');
  const f=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW|constants.O_NONBLOCK);
  try{
    const before=await f.stat({bigint:true}),uid=process.getuid?.();
    if(!before.isFile()||before.nlink!==1n||before.size>BigInt(max)||(before.mode&0o022n)!==0n||(uid!==undefined&&before.uid!==BigInt(uid)))throw Error('launcher_artifact_unqualified');
    const bytes=await f.readFile(),after=await f.stat({bigint:true});
    if(BigInt(bytes.length)!==before.size||before.ino!==after.ino||before.size!==after.size||before.mtimeNs!==after.mtimeNs||before.ctimeNs!==after.ctimeNs)throw Error('launcher_artifact_changed');
    return bytes;
  }finally{await f.close();}
}
try{
  if(!isAbsolute(base)||await realpath(base)!==base)throw Error('launcher_base_unqualified');
  const dir=await stat(base),uid=process.getuid?.();
  if(!dir.isDirectory()||(dir.mode&0o022)!==0||(uid!==undefined&&dir.uid!==uid))throw Error('launcher_base_unqualified');
  const pointerPath=join(base,'release.json'),pointerBytes=await qualifiedFile(pointerPath,65536),pointer=JSON.parse(pointerBytes.toString());
  if(!['academy.mcp-release/1','academy.mcp-release/2'].includes(pointer.schemaVersion)||typeof pointer.generation!=='string'||!/^[a-zA-Z0-9._-]{1,64}$/.test(pointer.generation)||pointer.generation==='.'||pointer.generation==='..')throw Error('launcher_pointer_unqualified');
  const release=join(base,'releases',pointer.generation),bridge=join(release,'server.mjs');
  if(await realpath(release)!==release)throw Error('launcher_release_unqualified');
  const pin=pointer.schemaVersion==='academy.mcp-release/2'?pointer.bridge:JSON.parse((await qualifiedFile(join(release,'bridge-pin.json'),4096)).toString());
  if(pin?.path!==bridge||typeof pin.sha256!=='string'||!/^[a-f0-9]{64}$/.test(pin.sha256))throw Error('launcher_bridge_pin_unqualified');
  if(digest(await qualifiedFile(bridge,32*1024*1024))!==pin.sha256)throw Error('launcher_bridge_hash_mismatch');
  if(digest(await qualifiedFile(pointerPath,65536))!==digest(pointerBytes))throw Error('launcher_pointer_changed');
  const child=Bun.spawn([process.execPath,'--no-env-file',bridge],{stdin:'inherit',stdout:'inherit',stderr:'inherit',env:{...process.env,ACADEMY_MCP_INSTALL_DIR:base}});
  for(const signal of ['SIGINT','SIGTERM'] as const)process.on(signal,()=>child.kill(signal));
  process.exit(await child.exited);
}catch(error){
  const code=error instanceof Error&&/^launcher_[a-z_]+$/.test(error.message)?error.message:'launcher_failed';
  process.stderr.write(code+'@src/stable-launcher.ts:1\n');process.exit(1);
}
