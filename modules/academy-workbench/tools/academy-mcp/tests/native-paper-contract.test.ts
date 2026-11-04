import {expect,test} from 'bun:test';
import {nativePaperConfigSchema,paper1HeadersSchema,paper2HeadersSchema,validateNativePaperScope,assertNativePaperArtifacts} from '../src/native-paper-contract.ts';
import {endpointIdentitySchema} from '../src/runtime-identity.ts';

const sha='1'.repeat(64);
const config=nativePaperConfigSchema.parse({schemaVersion:'academy.native-printed-paper-lookup/1',sourceBundlePath:'/runtime/source.mjs',sourceBundleSha256:sha,
 allowedPeriod:{from:'2040-01-01',through:'2040-02-01'},students:[{studentId:'1001',displayName:'Fictional Student'}]});
const input={variant:'paper1',configPath:'/runtime/config.json',configSha:sha,sourcePath:config.sourceBundlePath,sourceSha:sha,ownerPath:'/runtime/owner.json',dependencyPath:'/runtime/jsdom.json',dependencyEntryPath:'/runtime/jsdom.js'};
const artifact=(role:string,name:string)=>({role,path:'/host/'+name,runtimePath:'/runtime/'+name,sha256:sha});
const manifest=endpointIdentitySchema.parse({schemaVersion:'academy.endpoint-identity/1',endpoint:'paper1',semanticContractVersion:'paper/1',launchArgumentsSha256:sha,launchPrefixSha256:sha,accountLockPath:'/fictional/lock',runtimeManifestPath:'/runtime/manifest.json',
 artifacts:[artifact('entry','server.mjs'),artifact('config','config.json'),artifact('source','source.mjs'),artifact('policy','owner.json'),artifact('dependency','jsdom.json'),artifact('dependency','jsdom.js')]});

test('paper native requires the exact endpoint/config/source and owner/dependency pins',()=>{
 expect(()=>assertNativePaperArtifacts(manifest,input)).not.toThrow();
 for(const bad of [{...input,variant:'paper2'},{...input,configSha:'2'.repeat(64)},{...input,sourcePath:'/runtime/other.mjs'},{...input,ownerPath:'/runtime/other-owner.json'},{...input,dependencyPath:'/runtime/other-dependency.json'},{...input,dependencyEntryPath:'/runtime/other-entry.js'}])
  expect(()=>assertNativePaperArtifacts(manifest,bad)).toThrow('release_endpoint_identity_mismatch');
});
test('native retains configured student, exact owner names, date bounds and distinct header variants',()=>{
 expect(()=>validateNativePaperScope(config,['Fictional Student'])).not.toThrow();
 expect(()=>validateNativePaperScope(config,['Another Student'])).toThrow('managed_student_scope_required');
 expect(()=>validateNativePaperScope({...config,allowedPeriod:{from:'2040-02-01',through:'2040-01-01'}},['Fictional Student'])).toThrow('invalid_source_scope');
 expect(()=>validateNativePaperScope({...config,students:[...config.students,...config.students]},['Fictional Student'])).toThrow('invalid_source_scope');
 const args={studentId:'1001',paperCodes:['#234']};
 expect(paper1HeadersSchema.safeParse({...args,view:'definition'}).success).toBe(false);
 expect(paper2HeadersSchema.parse(args).view).toBe('header');
 expect(paper2HeadersSchema.parse({...args,view:'definition'}).view).toBe('definition');
});
