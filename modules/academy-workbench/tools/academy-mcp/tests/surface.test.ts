import {expect,test} from 'bun:test';
import {mkdtemp,readFile,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {loadRuntimeRelease} from '../src/runtime-release.ts';
import {publicReadFailure} from '../src/read-failure.ts';

// Fictional release bases for the optional `surface` pin (academy.mcp-surface/1).
// Nothing here reads an installed path, an LMS or a network.
const entry=resolve(import.meta.dir,'../src/neutral-server.ts'),deny=resolve(import.meta.dir,'network-deny.ts');
const golden=resolve(import.meta.dir,'golden/surface-absent-tools-list.json');
const hash=(bytes:string|Buffer)=>createHash('sha256').update(bytes).digest('hex');
type Pin={path:string;sha256:string};

/** A hand-written registry: stable descriptions, every list-time rewrite of the bridge
 * (owner-admitted dates hint, routing context, saved and class2 scopes, an unrouted tool). */
const fictionalRegistry=[
 {endpoint:'dailyrecord',upstreamName:'academy_student_resolve',tool:{name:'academy_student_resolve',title:'Registry resolve title',
  description:'Fictional resolver. One serialized native read batch. DailyRecord owner-admitted dates: 2000-01-01 through 2000-01-02.',
  inputSchema:{type:'object',properties:{displayName:{type:'string'}},required:['displayName'],additionalProperties:false},
  outputSchema:{type:'object',properties:{status:{type:'string'}}},annotations:{readOnlyHint:true,openWorldHint:true}}},
 {endpoint:'dailyrecord',upstreamName:'academy_source_status',tool:{name:'academy_source_status',description:'Fictional status.',
  inputSchema:{type:'object',properties:{},additionalProperties:false}}},
 {endpoint:'saved',upstreamName:'academy_query',tool:{name:'academy_query',description:'Fictional saved query. Preserved evidence only.',
  inputSchema:{type:'object',properties:{q:{type:'string'}}}}},
 {endpoint:'paper2',upstreamName:'academy_paper_headers',tool:{name:'class2_academy_paper_headers',description:'Fictional paper headers.',
  inputSchema:{type:'object',properties:{keys:{type:'array',items:{type:'string'}}}}}},
 {endpoint:'dailyrecord',upstreamName:'fictional_unrouted',tool:{name:'fictional_unrouted_tool',description:'Fictional tool without routing context.',
  inputSchema:{type:'object'},annotations:{destructiveHint:false}}},
];

async function fixtureBase(options:{allowedThrough?:string|null}={}){
 const base=await mkdtemp(resolve(tmpdir(),'academy-surface-'));
 const events=resolve(base,'events.txt');
 let n=0;
 const pin=async(name:string,value:unknown):Promise<Pin>=>{
  const bytes=typeof value==='string'?value:JSON.stringify(value),path=resolve(base,`${++n}-${name}`);
  await writeFile(path,bytes,{mode:0o600,flag:'wx'});return {path,sha256:hash(bytes)};
 };
 const admission=await pin('admission.json',options.allowedThrough===null
  ?{schemaVersion:'academy.owner-admission/2',decider:'user',decidedAt:'2040-01-16',sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',allowedFrom:'2040-01-15',allowedThrough:null,failClosedDefault:'2026-10-23'}
  :{schemaVersion:'academy.owner-admission/1',decider:'user',decidedAt:'2040-01-16',sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',allowedFrom:'2040-01-15',allowedThrough:options.allowedThrough??'2040-01-17',failClosedDefault:'2026-10-23'});
 const routing=await pin('routing.json',{admissionPath:admission.path,admissionSha256:admission.sha256});
 const upstream={command:process.execPath,args:['--preload',deny,resolve(import.meta.dir,'neutral-upstream-fixture.ts'),events]};
 const endpoints=await pin('endpoints.json',{dailyrecord:upstream,saved:upstream,paper2:upstream});
 const registry=await pin('registry.json',fictionalRegistry);
 const env={PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base};
 let generation=0;
 const publish=async(members:Record<string,unknown>={})=>{
  const next=resolve(base,`release-${++generation}.tmp`);
  await writeFile(next,JSON.stringify({schemaVersion:'academy.mcp-release/1',generation:`surface-${generation}`,routing,endpoints,registry,...members}),{mode:0o600});
  await rename(next,resolve(base,'release.json'));
 };
 return {base,events,pin,publish,env,routing,endpoints,registry};
}

/** A base whose registry comes from the bridge's own discovery of the fictional upstream. */
async function discoveredBase(){
 const f=await fixtureBase();
 const legacyRouting=JSON.parse(await readFile(f.routing.path,'utf8'));
 await writeFile(resolve(f.base,'routing.json'),JSON.stringify(legacyRouting));
 await writeFile(resolve(f.base,'endpoints.json'),JSON.stringify({dailyrecord:JSON.parse(await readFile(f.endpoints.path,'utf8')).dailyrecord}));
 const discovery=Bun.spawn([process.execPath,'--preload',deny,entry,'--discover'],{env:f.env,stdout:'pipe',stderr:'pipe'});
 expect(await discovery.exited).toBe(0);
 const pinExisting=async(name:string)=>{const path=resolve(f.base,name);return {path,sha256:hash(await readFile(path))};};
 const endpoints=await pinExisting('endpoints.json'),registry=await pinExisting('registry.json');
 const publish=(members:Record<string,unknown>={})=>f.publish({endpoints,registry,...members});
 return {...f,endpoints,registry,publish};
}

async function connect(env:Record<string,string>){
 const client=new Client({name:'fictional-surface-client',version:'1.0.0'});
 const transport=new StdioClientTransport({command:process.execPath,args:['--preload',deny,entry],env,stderr:'pipe'});
 await client.connect(transport);
 return {client,close:async()=>{await client.close();await transport.close();}};
}

async function loadFailure(base:string){
 try{await loadRuntimeRelease(base);}catch(error){return {message:(error as Error).message,code:publicReadFailure(error).code};}
 throw Error('expected_release_load_failure');
}

const surfaceDoc=(tools:unknown[],extra:Record<string,unknown>={})=>({schemaVersion:'academy.mcp-surface/1',tools,...extra});

test('surface tools/list: surface names in order, verbatim descriptions, placeholder replaced, surface schema where given, registry members kept',async()=>{
 const f=await fixtureBase();
 const narrowed={type:'object',properties:{displayName:{type:'string',minLength:2}},required:['displayName'],additionalProperties:false};
 const surface=await f.pin('surface.json',surfaceDoc([
  {name:'fictional_unrouted_tool',description:'Unrouted first. Dates {{admittedDates}}; again {{admittedDates}}.'},
  {name:'academy_student_resolve',title:'Surface resolve title',description:'Resolve one exact student name.',inputSchema:narrowed},
  {name:'academy_runtime_identity',description:'Release identity only.'},
  {name:'class2_academy_paper_headers',description:'Paper2 headers, verbatim. DailyRecord owner-admitted dates: 1999-01-01 through 1999-01-02.'},
 ]));
 await f.publish();
 const {client,close}=await connect(f.env);
 try{
  const absent=(await client.callTool({name:'academy_runtime_identity',arguments:{}})).structuredContent;
  expect(absent).toHaveProperty('surfaceSha256',null);
  await f.publish({surface});
  const tools=(await client.listTools()).tools;
  expect(tools.map(t=>t.name)).toEqual(['fictional_unrouted_tool','academy_student_resolve','academy_runtime_identity','class2_academy_paper_headers']);
  expect(tools[0]).toEqual({name:'fictional_unrouted_tool',description:'Unrouted first. Dates 2040-01-15 through 2040-01-17; again 2040-01-15 through 2040-01-17.',
   inputSchema:{type:'object'},annotations:{destructiveHint:false}});
  expect(tools[1]).toEqual({name:'academy_student_resolve',title:'Surface resolve title',description:'Resolve one exact student name.',inputSchema:narrowed,
   outputSchema:{type:'object',properties:{status:{type:'string'}}},annotations:{readOnlyHint:true,openWorldHint:true}});
  expect(tools[2]).toMatchObject({name:'academy_runtime_identity',description:'Release identity only.',inputSchema:{type:'object',properties:{},additionalProperties:false}});
  expect(tools[3]).toEqual({name:'class2_academy_paper_headers',description:'Paper2 headers, verbatim. DailyRecord owner-admitted dates: 1999-01-01 through 1999-01-02.',
   inputSchema:{type:'object',properties:{keys:{type:'array',items:{type:'string'}}}}});
  const identity=await client.callTool({name:'academy_runtime_identity',arguments:{}});
  expect(identity.structuredContent).toMatchObject({surfaceSha256:surface.sha256});
 }finally{await close();await rm(f.base,{recursive:true,force:true});}
 // A missing admission end reads `unbounded`, as the current hint does.
 const open=await fixtureBase({allowedThrough:null});
 await open.publish({surface:await open.pin('surface.json',surfaceDoc([{name:'fictional_unrouted_tool',description:'Dates {{admittedDates}}.'}]))});
 const second=await connect(open.env);
 try{expect((await second.client.listTools()).tools.map(t=>t.description)).toEqual(['Dates 2040-01-15 through unbounded.']);}
 finally{await second.close();await rm(open.base,{recursive:true,force:true});}
},30000);

test('surface tools/call: a registry tool the surface omits fails as unknown_tool before any endpoint starts',async()=>{
 const f=await discoveredBase();
 await f.publish({surface:await f.pin('surface.json',surfaceDoc([{name:'academy_source_status',description:'Saved coverage.'}]))});
 const {client,close}=await connect(f.env);
 try{
  const before=await readFile(f.events,'utf8');
  for(const name of ['academy_cism_weekly_save','academy_runtime_identity','academy_dailyrecord_get']){
   const result=await client.callTool({name,arguments:{}});
   expect(result.isError).toBe(true);
   expect(result.structuredContent).toMatchObject({status:'blocked',code:'unknown_tool',failure:{category:'endpoint_absence'},endpoint:null,academyWrites:0});
  }
  expect(await readFile(f.events,'utf8')).toBe(before);
  // Control: the listed tool still reaches its endpoint.
  expect((await client.callTool({name:'academy_source_status',arguments:{}})).structuredContent).toMatchObject({sourceMode:'saved_context'});
  expect((await readFile(f.events,'utf8')).slice(before.length)).toBe('start\nsource_status:{}\n');
 }finally{await close();await rm(f.base,{recursive:true,force:true});}
},30000);

test('an unqualified surface makes the release fail to load with release_surface_unqualified',async()=>{
 const f=await fixtureBase();
 try{
  const ok={name:'academy_source_status',description:'Status.'};
  const cases:[string,unknown][]=[
   ['absent registry tool',surfaceDoc([ok,{name:'academy_not_in_registry',description:'x'}])],
   ['duplicate name',surfaceDoc([ok,{...ok,description:'again'}])],
   ['missing description',surfaceDoc([{name:'academy_source_status'}])],
   ['unknown tool member',surfaceDoc([{...ok,annotations:{readOnlyHint:true}}])],
   ['unknown top-level member',surfaceDoc([ok],{note:'x'})],
   ['inputSchema type array',surfaceDoc([{...ok,inputSchema:{type:'array'}}])],
   ['inputSchema without type',surfaceDoc([{...ok,inputSchema:{properties:{}}}])],
   ['inputSchema not an object',surfaceDoc([{...ok,inputSchema:['object']}])],
   ['wrong schemaVersion',{schemaVersion:'academy.mcp-surface/2',tools:[ok]}],
   ['not JSON','{"schemaVersion":'],
  ];
  for(const [label,value] of cases){
   await f.publish({surface:await f.pin('surface.json',value)});
   expect({label,...await loadFailure(f.base)}).toEqual({label,message:'release_surface_unqualified',code:'release_surface_unqualified'});
  }
  // The same through the bridge: listing and calling fail closed with that code.
  await f.publish({surface:await f.pin('surface.json',cases[0]![1])});
  const {client,close}=await connect(f.env);
  try{
   const before=await readFile(f.events,'utf8').catch(()=>'');
   const called=await client.callTool({name:'academy_source_status',arguments:{}});
   expect(called.structuredContent).toMatchObject({status:'blocked',code:'release_surface_unqualified',failure:{category:'contract_drift'}});
   let listError:unknown;try{await client.listTools();}catch(error){listError=error;}
   expect((listError as Error).message).toContain('release_surface_unqualified');
   expect(await readFile(f.events,'utf8').catch(()=>'')).toBe(before);
  }finally{await close();}
  // A valid surface on the same base loads.
  const valid=await f.pin('surface.json',surfaceDoc([ok,{name:'academy_runtime_identity',description:'Identity.'}]));
  await f.publish({surface:valid});
  expect((await loadRuntimeRelease(f.base))?.identity).toMatchObject({surfaceSha256:valid.sha256});
 }finally{await rm(f.base,{recursive:true,force:true});}
},30000);

test('a surface whose bytes do not match its pin fails as a registry with a wrong hash does',async()=>{
 const f=await fixtureBase();
 try{
  await f.publish({registry:{...f.registry,sha256:'0'.repeat(64)}});
  const registryFailure=await loadFailure(f.base);
  expect(registryFailure.message).toBe('label_snapshot_hash_mismatch');
  const surface=await f.pin('surface.json',surfaceDoc([{name:'academy_source_status',description:'Status.'}]));
  await f.publish({surface:{...surface,sha256:'0'.repeat(64)}});
  expect(await loadFailure(f.base)).toEqual(registryFailure);
  await f.publish({surface:{...surface,sha256:hash('other bytes')}});
  expect(await loadFailure(f.base)).toEqual(registryFailure);
 }finally{await rm(f.base,{recursive:true,force:true});}
},30000);

test('tool_input_invalid carries at most 5 issues without argument values, with and without a surface',async()=>{
 const f=await discoveredBase();
 const {client,close}=await connect(f.env);
 try{
  const args={lessonDate:12345,purpose:67890,currentRequestRef:24680,CANARY_KEY_A:'CANARY_VALUE_A',CANARY_KEY_B:'CANARY_VALUE_B',
   CANARY_KEY_C:'CANARY_VALUE_C',CANARY_KEY_D:'CANARY_VALUE_D',CANARY_KEY_E:'CANARY_VALUE_E',CANARY_KEY_F:'CANARY_VALUE_F'};
  const before=await readFile(f.events,'utf8');
  for(const surface of [undefined,await f.pin('surface.json',surfaceDoc([{name:'academy_dailyrecord_get',description:'Read one DailyRecord.'}]))]){
   await f.publish(surface?{surface}:{});
   const result=await client.callTool({name:'academy_dailyrecord_get',arguments:args});
   const data=result.structuredContent as any;
   expect(data).toMatchObject({status:'blocked',code:'tool_input_invalid',phase:'input_validation',failure:{category:'input_validation'},academyWrites:0});
   expect(data.issues.length).toBeGreaterThan(0);
   expect(data.issues.length).toBeLessThanOrEqual(5);
   for(const issue of data.issues){
    expect(typeof issue.kind).toBe('string');expect(Array.isArray(issue.path)).toBe(true);
    for(const key of Object.keys(issue))expect(['kind','path','missingKey','expectedType']).toContain(key);
   }
   const text=JSON.stringify(result);
   for(const value of ['12345','67890','24680','CANARY'])expect(text).not.toContain(value);
  }
  expect(await readFile(f.events,'utf8')).toBe(before);
 }finally{await close();await rm(f.base,{recursive:true,force:true});}
},30000);

test('a listed tool is validated against the registry schema, not the narrower displayed one, and reaches its endpoint',async()=>{
 const f=await discoveredBase();
 const displayed={type:'object',properties:{mode:{type:'string'}},required:['mode'],additionalProperties:false};
 await f.publish({surface:await f.pin('surface.json',surfaceDoc([{name:'academy_source_status',description:'Coverage of saved evidence.',inputSchema:displayed}]))});
 const {client,close}=await connect(f.env);
 try{
  expect((await client.listTools()).tools).toEqual([{name:'academy_source_status',description:'Coverage of saved evidence.',inputSchema:displayed}]);
  const before=await readFile(f.events,'utf8');
  const result=await client.callTool({name:'academy_source_status',arguments:{}});
  expect(result.isError).not.toBe(true);
  expect(result.structuredContent).toMatchObject({sourceMode:'saved_context',receivedArguments:{}});
  expect((await readFile(f.events,'utf8')).slice(before.length)).toBe('start\nsource_status:{}\n');
 }finally{await close();await rm(f.base,{recursive:true,force:true});}
},30000);

/** The raw JSON-RPC line of a tools/list answer, as the bridge writes it on stdout. */
async function rawToolsList(env:Record<string,string>){
 const proc=Bun.spawn([process.execPath,'--preload',deny,entry],{env,stdin:'pipe',stdout:'pipe',stderr:'pipe'});
 const reader=proc.stdout.getReader(),decoder=new TextDecoder();let output='';
 const until=async(id:number)=>{while(!output.split('\n').some(line=>line.startsWith('{')&&JSON.parse(line).id===id)){const v=await reader.read();if(v.done)throw Error('raw_response_missing');output+=decoder.decode(v.value,{stream:true});}};
 proc.stdin.write(JSON.stringify({jsonrpc:'2.0',id:1,method:'initialize',params:{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'fictional-raw',version:'1'}}})+'\n');
 await until(1);
 proc.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 proc.stdin.write(JSON.stringify({jsonrpc:'2.0',id:2,method:'tools/list',params:{}})+'\n');
 await until(2);
 proc.stdin.end();await proc.exited;
 return output.split('\n').find(line=>line.startsWith('{')&&JSON.parse(line).id===2)!+'\n';
}

test('without a surface, tools/list is byte-identical to the origin/main answer for the same fixture release',async()=>{
 const f=await fixtureBase();
 try{
  await f.publish();
  const raw=await rawToolsList(f.env);
  if(process.env.ACADEMY_SURFACE_WRITE_GOLDEN==='1')await writeFile(golden,raw,{flag:'wx'});
  expect(raw).toBe(await readFile(golden,'utf8'));
 }finally{await rm(f.base,{recursive:true,force:true});}
},30000);
