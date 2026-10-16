import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,rmSync,symlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {validatePrivateStudentPackage} from './validate.ts';

function fixture(run:(root:string,manifest:any)=>void){
 const root=mkdtempSync(join(tmpdir(),'student-context-synthetic-'));
 mkdirSync(join(root,'today'));const body='{"student":"Invented test student","status":"unknown"}\n';
 writeFileSync(join(root,'today','fixture.json'),body);
 const manifest={schemaVersion:'academy.private-student-context/1.0',dataClass:'real_student_evidence',asOf:'2026-10-16T19:00:00+09:00',timezone:'Asia/Seoul',publication:{repository:'park-kyungchan/dc-gang-a',visibility:'private',mergeAllowed:false,productionMutation:false,syntheticReplacement:false,authorizationMessageIds:['Sentinel_synthetic_fixture']},artifacts:[{path:'today/fixture.json',sha256:createHash('sha256').update(body).digest('hex'),bytes:Buffer.byteLength(body),provenanceKind:'user_report',sourceReferences:['synthetic-test-source'],coverage:'partial'}],limitations:['Synthetic test only, not actual student evidence.']};
 try{writeFileSync(join(root,'manifest.json'),JSON.stringify(manifest));run(root,manifest);}finally{rmSync(root,{recursive:true,force:true});}
}
test('valid explicit package verifies bytes without claiming teacher acceptance',()=>fixture(root=>{
 const result=validatePrivateStudentPackage(root);assert.equal(result.artifactCount,1);assert.match(result.scope,/not_semantic_acceptance/);
}));
test('public destination and mutation flags fail closed',()=>fixture((root,m)=>{
 for(const change of [{visibility:'public'},{mergeAllowed:true},{productionMutation:true},{syntheticReplacement:true}]){
  writeFileSync(join(root,'manifest.json'),JSON.stringify({...m,publication:{...m.publication,...change}}));assert.throws(()=>validatePrivateStudentPackage(root));
 }
}));
test('hash mismatch and unmanifested material fail',()=>fixture((root,m)=>{
 writeFileSync(join(root,'today','fixture.json'),'changed');assert.throws(()=>validatePrivateStudentPackage(root),/byte_count_mismatch|hash_mismatch/);
}));
test('unlisted payload is not silently published',()=>fixture(root=>{
 writeFileSync(join(root,'unreviewed.json'),'{}');assert.throws(()=>validatePrivateStudentPackage(root),/unmanifested_file/);
}));
test('relative path escape rejected',()=>fixture((root,m)=>{
 m.artifacts[0].path='today/../../foreign.json';writeFileSync(join(root,'manifest.json'),JSON.stringify(m));assert.throws(()=>validatePrivateStudentPackage(root),/artifact_path_rejected/);
}));
test('symlinks cannot bypass explicit package boundary',()=>fixture(root=>{
 symlinkSync('/tmp',join(root,'escape'));assert.throws(()=>validatePrivateStudentPackage(root),/symlink_rejected/);
}));
test('credential fields rejected even with matching hash',()=>fixture((root,m)=>{
 const body=JSON.stringify({password:'synthetic-only'});writeFileSync(join(root,'today','fixture.json'),body);m.artifacts[0].bytes=Buffer.byteLength(body);m.artifacts[0].sha256=createHash('sha256').update(body).digest('hex');writeFileSync(join(root,'manifest.json'),JSON.stringify(m));assert.throws(()=>validatePrivateStudentPackage(root),/credential_field_rejected/);
}));
test('missing source refs rejected',()=>fixture((root,m)=>{
 m.artifacts[0].sourceReferences=[];writeFileSync(join(root,'manifest.json'),JSON.stringify(m));assert.throws(()=>validatePrivateStudentPackage(root),/source_references_required/);
}));
test('unknown provenance rejected',()=>fixture((root,m)=>{
 m.artifacts[0].provenanceKind='invented_fresh_server_observation';writeFileSync(join(root,'manifest.json'),JSON.stringify(m));assert.throws(()=>validatePrivateStudentPackage(root),/artifact_type_required/);
}));
test('known governance authorization is not an auth header; arbitrary auth material fails',()=>fixture((root,m)=>{
 function replace(body:unknown){const text=JSON.stringify(body);writeFileSync(join(root,'today','fixture.json'),text);m.artifacts[0].bytes=Buffer.byteLength(text);m.artifacts[0].sha256=createHash('sha256').update(text).digest('hex');writeFileSync(join(root,'manifest.json'),JSON.stringify(m));}
 replace({authorization:'runtime managed student read scope'});assert.equal(validatePrivateStudentPackage(root).artifactCount,1);
 replace({authorization:{evaluatedAtRuntime:true,allowedScopeRequired:true,explicitAuthorityForEffects:true,documentGrantsPermission:false}});assert.equal(validatePrivateStudentPackage(root).artifactCount,1);
 replace({authorization:'synthetic-unknown-credential'});assert.throws(()=>validatePrivateStudentPackage(root),/credential_authorization_field_rejected/);
}));
