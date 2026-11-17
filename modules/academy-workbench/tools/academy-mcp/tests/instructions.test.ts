import {expect,test} from 'bun:test';
import {mkdtemp,readFile,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {loadRuntimeRelease} from '../src/runtime-release.ts';
import {publicReadFailure} from '../src/read-failure.ts';

// Fictional release bases for the optional `instructions` member of academy.mcp-surface/1.
// Nothing here reads an installed path, an LMS or a network.
const entry=resolve(import.meta.dir,'../src/neutral-server.ts'),deny=resolve(import.meta.dir,'network-deny.ts');
const golden=resolve(import.meta.dir,'golden/surface-absent-initialize.json');
const hash=(bytes:string|Buffer)=>createHash('sha256').update(bytes).digest('hex');
type Pin={path:string;sha256:string};

const fictionalRegistry=[
 {endpoint:'dailyrecord',upstreamName:'academy_source_status',tool:{name:'academy_source_status',description:'Fictional status.',
  inputSchema:{type:'object',properties:{},additionalProperties:false}}},
];

async function fixtureBase(){
 const base=await mkdtemp(resolve(tmpdir(),'academy-instructions-'));
 let n=0;
 const pin=async(name:string,value:unknown):Promise<Pin>=>{
  const bytes=typeof value==='string'?value:JSON.stringify(value),path=resolve(base,`${++n}-${name}`);
  await writeFile(path,bytes,{mode:0o600,flag:'wx'});return {path,sha256:hash(bytes)};
 };
 const admission=await pin('admission.json',{schemaVersion:'academy.owner-admission/1',decider:'user',decidedAt:'2040-01-16',
  sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',allowedFrom:'2040-01-15',allowedThrough:'2040-01-17',failClosedDefault:'2026-10-23'});
 const routing=await pin('routing.json',{admissionPath:admission.path,admissionSha256:admission.sha256});
 const upstream={command:process.execPath,args:['--preload',deny,resolve(import.meta.dir,'neutral-upstream-fixture.ts'),resolve(base,'events.txt')]};
 const endpoints=await pin('endpoints.json',{dailyrecord:upstream});
 const registry=await pin('registry.json',fictionalRegistry);
 const env={PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base};
 let generation=0;
 const publish=async(members:Record<string,unknown>={})=>{
  const next=resolve(base,`release-${++generation}.tmp`);
  await writeFile(next,JSON.stringify({schemaVersion:'academy.mcp-release/1',generation:`instructions-${generation}`,routing,endpoints,registry,...members}),{mode:0o600});
  await rename(next,resolve(base,'release.json'));
 };
 return {base,pin,publish,env};
}

const surfaceDoc=(extra:Record<string,unknown>={})=>({schemaVersion:'academy.mcp-surface/1',
 tools:[{name:'academy_source_status',description:'Status.'}],...extra});

/** A bridge process driven by raw JSON-RPC lines, so an answer can be compared byte for byte. */
function rawBridge(env:Record<string,string>){
 const proc=Bun.spawn([process.execPath,'--preload',deny,entry],{env,stdin:'pipe',stdout:'pipe',stderr:'pipe'});
 const reader=proc.stdout.getReader(),decoder=new TextDecoder();let output='';
 const line=(id:number)=>output.split('\n').find(l=>l.startsWith('{')&&JSON.parse(l).id===id);
 const request=async(id:number,method:string,params:unknown)=>{
  proc.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');
  while(line(id)===undefined){const v=await reader.read();if(v.done)throw Error('raw_response_missing');output+=decoder.decode(v.value,{stream:true});}
  return line(id)!+'\n';
 };
 const initialize=(id:number)=>request(id,'initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'fictional-raw',version:'1'}});
 return {request,initialize,close:async()=>{proc.stdin.end();await proc.exited;}};
}

async function loadFailure(base:string){
 try{await loadRuntimeRelease(base);}catch(error){return {message:(error as Error).message,code:publicReadFailure(error).code};}
 throw Error('expected_release_load_failure');
}

test('initialize carries the surface instructions with {{admittedDates}} rendered as any date, fixed for the life of the process',async()=>{
 const f=await fixtureBase();
 const text='Call academy_student_resolve first. Dates: {{admittedDates}}; again {{admittedDates}}. 저장은 요청이 있을 때만.';
 const filled='Call academy_student_resolve first. Dates: any date; again any date. 저장은 요청이 있을 때만.';
 await f.publish({surface:await f.pin('surface.json',surfaceDoc({instructions:text}))});
 const bridge=rawBridge(f.env);
 try{
  const first=JSON.parse(await bridge.initialize(1)).result;
  expect(first.instructions).toBe(filled);
  // A pointer change under the running bridge changes tools/list, not the instructions it answers with.
  await f.publish({surface:await f.pin('surface.json',surfaceDoc({instructions:'Other text.',tools:[{name:'academy_source_status',description:'Changed.'}]}))});
  const listed=JSON.parse(await bridge.request(2,'tools/list',{})).result;
  expect(listed.tools.map((t:any)=>t.description)).toEqual(['Changed.']);
  const again=JSON.parse(await bridge.initialize(3)).result;
  expect(again.instructions).toBe(filled);
  await f.publish();
  expect(JSON.parse(await bridge.initialize(4)).result.instructions).toBe(filled);
 }finally{await bridge.close();await rm(f.base,{recursive:true,force:true});}
},30000);

test('instructions that are not a string or longer than 16,384 UTF-8 bytes fail the release with release_surface_unqualified',async()=>{
 const f=await fixtureBase();
 try{
  const cases:[string,unknown][]=[
   ['number',42],
   ['16,385 ASCII bytes','a'.repeat(16385)],
   ['16,385 bytes in 5,462 characters','가'.repeat(5461)+'ab'],
   ['null',null],
   ['array',['text']],
  ];
  for(const [label,instructions] of cases){
   await f.publish({surface:await f.pin('surface.json',surfaceDoc({instructions}))});
   expect({label,...await loadFailure(f.base)}).toEqual({label,message:'release_surface_unqualified',code:'release_surface_unqualified'});
  }
  // Another new member is still refused.
  await f.publish({surface:await f.pin('surface.json',surfaceDoc({instructions:'Text.',notes:'x'}))});
  expect(await loadFailure(f.base)).toEqual({message:'release_surface_unqualified',code:'release_surface_unqualified'});
  // A fresh bridge refuses to start on such a release.
  await f.publish({surface:await f.pin('surface.json',surfaceDoc({instructions:'a'.repeat(16385)}))});
  const fresh=Bun.spawn([process.execPath,'--preload',deny,entry],{env:f.env,stdin:'ignore',stdout:'pipe',stderr:'pipe'});
  expect(await fresh.exited).toBe(1);expect(await new Response(fresh.stderr).text()).toBe('release_surface_unqualified\n');
  // Exactly 16,384 bytes loads, whether the characters take one byte or three.
  for(const instructions of ['a'.repeat(16384),'가'.repeat(5461)+'a']){
   expect(Buffer.byteLength(instructions,'utf8')).toBe(16384);
   const surface=await f.pin('surface.json',surfaceDoc({instructions}));
   await f.publish({surface});
   expect((await loadRuntimeRelease(f.base))?.identity).toMatchObject({surfaceSha256:surface.sha256});
  }
 }finally{await rm(f.base,{recursive:true,force:true});}
},30000);

test('without a surface, and with a surface without instructions, initialize is byte-identical to the origin/main answer',async()=>{
 const f=await fixtureBase();
 try{
  await f.publish();
  const absent=rawBridge(f.env);
  let raw:string;
  try{raw=await absent.initialize(1);}finally{await absent.close();}
  if(process.env.ACADEMY_INSTRUCTIONS_WRITE_GOLDEN==='1')await writeFile(golden,raw,{flag:'wx'});
  const expected=await readFile(golden,'utf8');
  expect(raw).toBe(expected);
  await f.publish({surface:await f.pin('surface.json',surfaceDoc())});
  const plain=rawBridge(f.env);
  try{expect(await plain.initialize(1)).toBe(expected);}finally{await plain.close();}
 }finally{await rm(f.base,{recursive:true,force:true});}
},30000);
