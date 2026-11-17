import {constants} from 'node:fs';
import {open,realpath,stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {isAbsolute,join} from 'node:path';
import {z} from 'zod';
import {readPinnedLocalBytes} from './item-label-store.ts';
import {artifactPinSchema,verifyEndpointManifest,verifyEndpointLauncher} from './runtime-identity.ts';
import {ReleaseBaseUnqualified} from './read-failure.ts';
import {paperScopesSchema} from './paper-scope-routing.ts';

const pin=z.object({path:z.string().refine(isAbsolute),sha256:z.string().regex(/^[a-f0-9]{64}$/)}).strict();
const commonRelease={generation:z.string().regex(/^[a-zA-Z0-9._-]{1,64}$/),routing:pin,endpoints:pin,registry:pin,surface:pin.optional()};
const releaseSchema=z.discriminatedUnion('schemaVersion',[
  z.object({schemaVersion:z.literal('academy.mcp-release/1'),...commonRelease}).strict(),
  z.object({schemaVersion:z.literal('academy.mcp-release/2'),...commonRelease,bridge:artifactPinSchema,
    endpointIdentities:z.record(z.string().regex(/^[a-z][a-z0-9_]{0,63}$/),artifactPinSchema)}).strict(),
]);
// The admission pin of an earlier release is accepted when present and gates nothing.
const routingSchema=z.object({admissionPath:z.string().refine(isAbsolute).optional(),admissionSha256:z.string().regex(/^[a-f0-9]{64}$/).optional(),paperScopes:paperScopesSchema.optional()}).strict();
const registrySchema=z.array(z.object({endpoint:z.string().regex(/^[a-z][a-z0-9_]{0,63}$/),
  upstreamName:z.string().regex(/^[a-z][a-z0-9_]{0,127}$/),
  tool:z.object({name:z.string().regex(/^[a-z][a-z0-9_]{0,127}$/)}).passthrough()}).strict()).max(256);
/** academy.mcp-surface/1: what tools/list shows and the server's instructions. Validation of calls stays with the registry. */
const surfaceSchema=z.object({schemaVersion:z.literal('academy.mcp-surface/1'),tools:z.array(z.object({
  name:z.string(),description:z.string(),title:z.string().optional(),
  inputSchema:z.record(z.string(),z.unknown()).refine(schema=>schema.type==='object').optional()}).strict()).max(257),
  instructions:z.string().refine(text=>Buffer.byteLength(text,'utf8')<=16384).optional()}).strict();
export type ToolSurface=z.infer<typeof surfaceSchema>['tools'];
/** Tools that no endpoint serves any more. An earlier registry or surface may still
 * name them; they are dropped so that nothing lists, calls or expects them. */
const retiredTools=new Set(['academy_runtime_read_admission']);
export function withoutRetiredTools<T extends {upstreamName?:string;tool?:{name:string};name?:string}>(entries:readonly T[]):T[]{
  return entries.filter(entry=>!retiredTools.has(entry.upstreamName??'')&&!retiredTools.has(entry.tool?.name??entry.name??''));
}

/** Read the owner-controlled pointer as one bounded regular file. A release is
 * activated by atomically replacing this pointer after all pinned files exist. */
async function readPointer(base:string):Promise<Buffer|null>{
  if(!isAbsolute(base))throw new ReleaseBaseUnqualified('relative_path');
  if(await realpath(base)!==base)throw new ReleaseBaseUnqualified('noncanonical_path');
  const directory=await stat(base),uid=process.getuid?.();
  if(!directory.isDirectory())throw new ReleaseBaseUnqualified('not_directory');
  if((directory.mode&0o022)!==0)throw new ReleaseBaseUnqualified('writable_by_others');
  if(uid!==undefined&&directory.uid!==uid)throw new ReleaseBaseUnqualified('owner_mismatch');
  let file;
  try{file=await open(join(base,'release.json'),constants.O_RDONLY|constants.O_NOFOLLOW|constants.O_NONBLOCK);}
  catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return null;throw error;}
  try{
    const before=await file.stat({bigint:true});
    if(!before.isFile()||before.nlink!==1n||before.size>65536n||(before.mode&0o022n)!==0n||(uid!==undefined&&before.uid!==BigInt(uid)))throw Error('release_pointer_unqualified');
    const data=Buffer.alloc(Number(before.size)+1),{bytesRead}=await file.read(data,0,data.length,0),after=await file.stat({bigint:true});
    if(BigInt(bytesRead)!==before.size||before.ino!==after.ino||before.size!==after.size||before.mtimeNs!==after.mtimeNs||before.ctimeNs!==after.ctimeNs)
      throw Error('release_pointer_changed');
    return data.subarray(0,bytesRead);
  }finally{await file.close();}
}

export async function loadRuntimeRelease(base:string){
  const pointer=await readPointer(base);if(pointer===null)return null;
  const release=releaseSchema.parse(JSON.parse(pointer.toString('utf8')));
  const read=async(ref:z.infer<typeof pin>)=>JSON.parse((await readPinnedLocalBytes(ref.path,ref.sha256)).bytes.toString('utf8'));
  const [rawRouting,endpoints,rawRegistry,surfaceBytes]=await Promise.all([read(release.routing),read(release.endpoints),read(release.registry),
    release.surface?readPinnedLocalBytes(release.surface.path,release.surface.sha256).then(r=>r.bytes):undefined]);
  const routing=routingSchema.parse(rawRouting);
  const parsedRegistry=registrySchema.parse(rawRegistry);
  if(!endpoints||typeof endpoints!=='object'||Array.isArray(endpoints)||new Set(parsedRegistry.map(r=>r.tool.name)).size!==parsedRegistry.length
    ||parsedRegistry.some(r=>!Object.hasOwn(endpoints,r.endpoint)))throw Error('release_payload_unqualified');
  const parsedSurface=surfaceBytes===undefined?undefined:parseSurface(surfaceBytes,parsedRegistry.map(r=>r.tool.name));
  const registry=withoutRetiredTools(parsedRegistry);
  const surface=parsedSurface&&withoutRetiredTools(parsedSurface.tools),instructions=parsedSurface?.instructions;
  const endpointIdentities:Record<string,Awaited<ReturnType<typeof verifyEndpointManifest>>['summary']>={};
  if(release.schemaVersion==='academy.mcp-release/2'){
    const accountLocks=new Set<string>();
    try{
      if(await realpath(process.argv[1]!)!==release.bridge.path)throw Error('release_bridge_identity_mismatch');
      await readPinnedLocalBytes(release.bridge.path,release.bridge.sha256);
    }catch{throw Error('release_bridge_identity_mismatch');}
    if(JSON.stringify(Object.keys(endpoints).sort())!==JSON.stringify(Object.keys(release.endpointIdentities).sort()))throw Error('release_payload_unqualified');
    for(const [endpoint,ref] of Object.entries(release.endpointIdentities)){
      const verified=await verifyEndpointManifest(ref);
      if(verified.manifest.endpoint!==endpoint)throw Error('release_endpoint_identity_mismatch');
      const args=endpoints[endpoint]?.args,entry=verified.manifest.artifacts.find(a=>a.role==='entry')!;
      const entryIndex=Array.isArray(args)?args.indexOf(entry.runtimePath??entry.path):-1;
      if(entryIndex<0||args[entryIndex+1]!=='--runtime-identity'||args[entryIndex+2]!==(verified.manifest.runtimeManifestPath??ref.path)||args[entryIndex+3]!==ref.sha256
        ||createHash('sha256').update(JSON.stringify(args.slice(entryIndex+4))).digest('hex')!==verified.manifest.launchArgumentsSha256)throw Error('release_endpoint_identity_mismatch');
      verifyEndpointLauncher(verified.manifest,endpoints[endpoint].command,args.slice(0,entryIndex));
      if(verified.manifest.accountLockPath&&['dailyrecord','paper1','paper2'].includes(endpoint))accountLocks.add(verified.manifest.accountLockPath);
      endpointIdentities[endpoint]=verified.summary;
    }
    if(accountLocks.size>1)throw Error('release_endpoint_identity_mismatch');
  }
  const after=await readPointer(base);
  if(after===null||createHash('sha256').update(pointer).digest('hex')!==createHash('sha256').update(after).digest('hex'))throw Error('release_pointer_changed');
  return {generation:release.generation,endpoints,registry,endpointIdentities,paperScopes:routing.paperScopes,surface,instructions,
    identity:{schemaVersion:release.schemaVersion,generation:release.generation,
      pointerSha256:createHash('sha256').update(pointer).digest('hex'),routingSha256:release.routing.sha256,
      endpointsSha256:release.endpoints.sha256,registrySha256:release.registry.sha256,surfaceSha256:release.surface?.sha256??null,admissionSha256:routing.admissionSha256??null,
      bridgeSha256:release.schemaVersion==='academy.mcp-release/2'?release.bridge.sha256:null,
      graphQualification:release.schemaVersion==='academy.mcp-release/2'?'artifact_pinned':'legacy_graph_incomplete',
      endpointIdentities:Object.values(endpointIdentities)}};
}

/** Names are unique and each is a registry tool or the bridge's own identity tool. */
function parseSurface(bytes:Buffer,registryNames:readonly string[]):{tools:ToolSurface;instructions:string|undefined}{
  const known=new Set([...registryNames,'academy_runtime_identity']);
  let surface:z.infer<typeof surfaceSchema>;
  try{surface=surfaceSchema.parse(JSON.parse(bytes.toString('utf8')));}catch{throw Error('release_surface_unqualified');}
  const {tools}=surface;
  if(new Set(tools.map(t=>t.name)).size!==tools.length||tools.some(t=>!known.has(t.name)))throw Error('release_surface_unqualified');
  return {tools,instructions:surface.instructions};
}
