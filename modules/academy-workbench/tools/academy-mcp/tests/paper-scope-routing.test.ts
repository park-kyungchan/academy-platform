import {expect,test} from 'bun:test';
import {mkdtemp,readdir,rm,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {checkPaperScopesCoverRoster,inspectPaperScope,PaperScopeUnresolvedError,paperScopesSchema,selectPaperRoute} from '../src/paper-scope-routing.ts';

const primary={endpoint:'paper1',upstreamName:'academy_paper_headers',tool:{name:'academy_paper_headers'}};
const secondary={endpoint:'paper2',upstreamName:'academy_paper_headers',tool:{name:'class2_academy_paper_headers'}};
const registry=[primary,secondary];
const scopes=paperScopesSchema.parse([{endpoint:'paper1',studentIds:['1001']},{endpoint:'paper2',studentIds:['1002']}]);

test('selects one verified configured scope without probing or widening upstream authority',()=>{
  expect(selectPaperRoute(scopes,registry,primary,{studentId:'1001'})).toBe(primary);
  expect(selectPaperRoute(scopes,registry,primary,{studentId:'1002'})).toBe(secondary);
  const extraAlias={...secondary,tool:{name:'paper2_headers_alias'}};
  expect(selectPaperRoute(scopes,[primary,secondary,extraAlias],primary,{studentId:'1002'})).toBe(secondary);
  expect(selectPaperRoute(undefined,registry,primary,{studentId:'1002'})).toBe(primary);
  expect(selectPaperRoute(scopes,registry,secondary,{studentId:'1002'})).toBe(secondary);
  expect(()=>selectPaperRoute(scopes,registry,secondary,{studentId:'1001'})).toThrow('paper_scope_unresolved');
  for(const name of ['academy_source_read_contracts','academy_paper_read_contracts']){
    const a={...primary,upstreamName:name},b={...secondary,upstreamName:name};
    expect(selectPaperRoute(scopes,[a,b],a,{studentId:'1002'})).toBe(b);
  }
});
test('missing, ambiguous or unsupported mappings refuse before any endpoint dispatch',()=>{
  expect(()=>selectPaperRoute(scopes,registry,primary,{studentId:'1003'})).toThrow('paper_scope_unresolved');
  const ambiguous=paperScopesSchema.parse([{endpoint:'paper1',studentIds:['1001']},{endpoint:'paper2',studentIds:['1001']}]);
  expect(()=>selectPaperRoute(ambiguous,registry,primary,{studentId:'1001'})).toThrow('paper_scope_unresolved');
  expect(()=>selectPaperRoute(ambiguous,registry,secondary,{studentId:'1001'})).toThrow('paper_scope_unresolved');
  expect(()=>selectPaperRoute(scopes,[primary],primary,{studentId:'1002'})).toThrow('paper_scope_unresolved');
  expect(()=>selectPaperRoute(scopes,registry,primary,{studentId:'01'})).toThrow();
  expect(paperScopesSchema.safeParse([{endpoint:'paper1',studentIds:['1001','1001']}]).success).toBe(false);
  expect(paperScopesSchema.safeParse([{endpoint:'paper3',studentIds:['1001']}]).success).toBe(false);
  expect(paperScopesSchema.safeParse([{endpoint:'paper1',studentIds:['1001']},{endpoint:'paper1',studentIds:['1002']}]).success).toBe(false);
});

test('unresolved scope carries a structured admission diagnostic, never caller-widening',()=>{
  let caught:unknown;
  try{selectPaperRoute(scopes,registry,primary,{studentId:'1003'});}catch(e){caught=e;}
  expect(caught).toBeInstanceOf(PaperScopeUnresolvedError);
  expect((caught as PaperScopeUnresolvedError).diagnostic).toEqual({studentId:'1003',upstreamName:'academy_paper_headers',configuredEndpoints:['paper1','paper2'],matchCount:0});
  expect(inspectPaperScope(scopes,'1001')).toEqual({status:'resolved',endpoint:'paper1'});
  expect(inspectPaperScope(scopes,'1003')).toEqual({status:'unresolved',configuredEndpoints:['paper1','paper2'],matchCount:0});
  expect(inspectPaperScope(undefined,'1003')).toEqual({status:'unconfigured'});
});

// specs/academy-mcp/paper-admit.md R22-R25: every active roster student is routed by exactly one entry.
test('paper_admit_r22_roster_covered',()=>{
  expect(checkPaperScopesCoverRoster(scopes,['1001','1002'])).toEqual({covered:true,missingStudentIds:[],duplicateStudentIds:[]});
  expect(checkPaperScopesCoverRoster(scopes,['1001','1003','1003'])).toEqual({covered:false,missingStudentIds:['1003'],duplicateStudentIds:[]});
  const twice=paperScopesSchema.parse([{endpoint:'paper1',studentIds:['1001']},{endpoint:'paper2',studentIds:['1001','1002']}]);
  expect(checkPaperScopesCoverRoster(twice,['1002','1001'])).toEqual({covered:false,missingStudentIds:[],duplicateStudentIds:['1001']});
  // A covered roster is exactly the one that selectPaperRoute resolves for every student.
  for(const id of ['1001','1002'])expect(inspectPaperScope(scopes,id).status).toBe('resolved');
});
test('paper_admit_r23_r25_check_roster_students_preflight',async()=>{
  const base=await mkdtemp(resolve(tmpdir(),'paper-roster-'));
  try{
    const routing=JSON.stringify({paperScopes:[{endpoint:'paper1',studentIds:['90001']},{endpoint:'paper2',studentIds:['90003']}]});
    await writeFile(resolve(base,'routing.json'),routing);
    await writeFile(resolve(base,'release.json'),JSON.stringify({schemaVersion:'academy.mcp-release/1',generation:'g-roster',
      routing:{path:resolve(base,'routing.json'),sha256:createHash('sha256').update(routing).digest('hex')}}));
    const files=JSON.stringify(await readdir(base));
    const run=(...args:string[])=>{
      const p=Bun.spawnSync(['python3','-I',resolve(import.meta.dir,'../install/loop_install.py'),'--paper-admit-preview','--base',base,...args],{stdout:'pipe',stderr:'pipe'});
      return {code:p.exitCode,out:p.stdout.toString(),err:p.stderr.toString().trim().split('\n').at(-1)};
    };
    expect(run('--check-roster-students','90001,90003')).toMatchObject({code:0,out:expect.stringMatching(/^routing sha256 [a-f0-9]{64} admitted 0\n$/)});
    expect(run('--check-roster-students','90001,90004')).toEqual({code:1,out:'',err:'roster_student_unrouted@2'});
    // An admission in the same call counts: the check reads the routing the install would write.
    expect(run('--paper-admit','paper1=90004','--check-roster-students','90001,90004')).toMatchObject({code:0});
    expect(run('--check-roster-students','90001,0900')).toEqual({code:1,out:'',err:'roster_student_id_invalid@2'});
    expect(run('--check-roster-students','90001,,90003')).toEqual({code:1,out:'',err:'roster_students_form'});
    expect(run('--check-roster-students','90001,90001')).toEqual({code:1,out:'',err:'roster_students_form'});
    const twice=JSON.stringify({paperScopes:[{endpoint:'paper1',studentIds:['90001']},{endpoint:'paper2',studentIds:['90001']}]});
    await writeFile(resolve(base,'routing.json'),twice);
    await writeFile(resolve(base,'release.json'),JSON.stringify({schemaVersion:'academy.mcp-release/1',generation:'g-roster',
      routing:{path:resolve(base,'routing.json'),sha256:createHash('sha256').update(twice).digest('hex')}}));
    expect(run('--check-roster-students','90001')).toEqual({code:1,out:'',err:'roster_student_duplicate@1'});
    // No id is printed and the base gains no file.
    for(const r of [run('--check-roster-students','90001,90004'),run('--check-roster-students','90004')])expect(r.out+r.err).not.toMatch(/9000[0-9]/);
    expect(JSON.stringify(await readdir(base))).toBe(files);
  }finally{await rm(base,{recursive:true,force:true});}
});
