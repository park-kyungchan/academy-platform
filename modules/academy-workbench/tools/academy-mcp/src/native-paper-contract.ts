import {z} from 'zod';
import type {endpointIdentitySchema} from './runtime-identity.ts';

const period=z.object({from:z.iso.date(),through:z.iso.date()}).strict();
export const nativePaperConfigSchema=z.object({schemaVersion:z.literal('academy.native-printed-paper-lookup/1'),sourceBundlePath:z.string().startsWith('/'),sourceBundleSha256:z.string().regex(/^[a-f0-9]{64}$/),allowedPeriod:period,students:z.array(z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/),displayName:z.string().min(1).max(200)}).strict()).min(1).max(8)}).strict();
export const paper1HeadersSchema=z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/),paperCodes:z.array(z.string().regex(/^#[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{1,11}$/)).min(1).max(32)}).strict();
export const paper2HeadersSchema=paper1HeadersSchema.extend({view:z.enum(['header','definition']).default('header')}).strict();

export function validateNativePaperScope(config:z.infer<typeof nativePaperConfigSchema>,managedNames:readonly string[]){
 if(config.allowedPeriod.from>config.allowedPeriod.through||new Set(config.students.map(s=>s.studentId)).size!==config.students.length)throw Error('invalid_source_scope');
 if(config.students.some(s=>!managedNames.includes(s.displayName)))throw Error('managed_student_scope_required');
}
export function assertNativePaperArtifacts(manifest:z.infer<typeof endpointIdentitySchema>,input:{variant:string;configPath:string;configSha:string;sourcePath:string;sourceSha:string;ownerPath:string;dependencyPath:string;dependencyEntryPath:string}){
 const has=(path:string,role:string,sha?:string)=>manifest.artifacts.some(a=>(a.runtimePath??a.path)===path&&a.role===role&&(!sha||a.sha256===sha));
 if(manifest.endpoint!==input.variant||!has(input.configPath,'config',input.configSha)||!has(input.sourcePath,'source',input.sourceSha)
   ||!has(input.ownerPath,'policy')||!has(input.dependencyPath,'dependency')||!has(input.dependencyEntryPath,'dependency'))throw Error('release_endpoint_identity_mismatch');
}
