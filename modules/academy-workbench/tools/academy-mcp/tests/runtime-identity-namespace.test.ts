import {expect,test} from 'bun:test';
import {mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {createHash} from 'node:crypto';
import {verifyEndpointManifest,verifyEndpointLauncher,endpointIdentitySchema} from '../src/runtime-identity.ts';

test('host and container namespaces independently verify identical manifest and artifact bytes',async()=>{
 const base=await mkdtemp(resolve(tmpdir(),'academy-identity-namespace-'));
 try{
  const host=resolve(base,'host'),runtime=resolve(base,'runtime');await mkdir(host);await mkdir(runtime);
  const hash=(b:string)=>createHash('sha256').update(b).digest('hex');
  const hostEntry=resolve(host,'server.mjs'),runtimeEntry=resolve(runtime,'server.mjs');
  const entryCode=`import {runtimeIdentityConfiguration} from ${JSON.stringify(resolve(import.meta.dir,'../src/runtime-identity.ts'))};\nconst value=await runtimeIdentityConfiguration(process.argv.slice(2));\nconsole.log(JSON.stringify(value.identity.summary));\n`;
  await writeFile(hostEntry,entryCode);await writeFile(runtimeEntry,entryCode);
  const hostManifest=resolve(host,'manifest.json'),runtimeManifest=resolve(runtime,'manifest.json');
  const bytes=JSON.stringify({schemaVersion:'academy.endpoint-identity/1',endpoint:'paper1',semanticContractVersion:'synthetic/1',launchArgumentsSha256:hash('[]'),launchPrefixSha256:hash('{}'),accountLockPath:'/fictional/lock',runtimeManifestPath:runtimeManifest,
   artifacts:[{role:'entry',path:hostEntry,runtimePath:runtimeEntry,sha256:hash(entryCode)}]});
  await writeFile(hostManifest,bytes);await writeFile(runtimeManifest,bytes);
  const hostResult=await verifyEndpointManifest({path:hostManifest,sha256:hash(bytes)});
  const runtimeResult=await verifyEndpointManifest({path:runtimeManifest,sha256:hash(bytes)},'runtime');
  expect(runtimeResult.summary).toEqual(hostResult.summary);
  const child=Bun.spawn([process.execPath,runtimeEntry,'--runtime-identity',runtimeManifest,hash(bytes)],{stdout:'pipe',stderr:'pipe'});
  const output=await new Response(child.stdout).text();expect(await child.exited).toBe(0);
  expect(JSON.parse(output)).toEqual(hostResult.summary);
  await writeFile(runtimeEntry,'different bytes');
  await expect(verifyEndpointManifest({path:runtimeManifest,sha256:hash(bytes)},'runtime')).rejects.toThrow('release_artifact_mismatch');
  expect((await verifyEndpointManifest({path:hostManifest,sha256:hash(bytes)})).summary).toEqual(hostResult.summary);
  await expect(verifyEndpointManifest({path:hostManifest,sha256:hash(bytes)},'runtime')).rejects.toThrow('release_endpoint_identity_mismatch');
 }finally{await rm(base,{recursive:true,force:true});}
});

test('runtime launcher binds the exact executable, full prefix and shared account lock',()=>{
 const hash=(s:string)=>createHash('sha256').update(s).digest('hex');
 const prefixArgs=['--nonblock','/fictional/account.lock','docker','exec'];
 const manifest=endpointIdentitySchema.parse({schemaVersion:'academy.endpoint-identity/1',endpoint:'paper1',semanticContractVersion:'paper/1',launchArgumentsSha256:hash('[]'),
  launchPrefixSha256:hash(JSON.stringify({command:'/usr/bin/flock',prefixArgs})),accountLockPath:'/fictional/account.lock',runtimeManifestPath:'/runtime/manifest.json',
  artifacts:[{role:'entry',path:'/host/server.mjs',runtimePath:'/runtime/server.mjs',sha256:hash('code')}]});
 expect(()=>verifyEndpointLauncher(manifest,'/usr/bin/flock',prefixArgs)).not.toThrow();
 expect(()=>verifyEndpointLauncher(manifest,'/usr/bin/flock',['--nonblock','/other/lock','docker','exec'])).toThrow('release_endpoint_identity_mismatch');
 expect(()=>verifyEndpointLauncher(manifest,'/usr/bin/env',prefixArgs)).toThrow('release_endpoint_identity_mismatch');
 const altered=endpointIdentitySchema.parse({...manifest,launchPrefixSha256:hash(JSON.stringify({command:'/usr/bin/env',prefixArgs}))});
 expect(()=>verifyEndpointLauncher(altered,'/usr/bin/env',prefixArgs)).toThrow('release_endpoint_identity_mismatch');
});
