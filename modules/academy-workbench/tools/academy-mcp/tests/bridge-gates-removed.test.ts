import {expect,test} from 'bun:test';
import {mkdtemp,readFile,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';

// TASK-GATES-1 check 6. A fictional release base laid out as an installed one:
// a pinned owner admission in the routing file and a dailyrecord endpoint
// launched with the old admission arguments. No installed path, LMS or network.
const entry=resolve(import.meta.dir,'../src/neutral-server.ts'),deny=resolve(import.meta.dir,'network-deny.ts');
const upstreamFixture=resolve(import.meta.dir,'gates-upstream-fixture.ts');
const hash=(bytes:string|Buffer)=>createHash('sha256').update(bytes).digest('hex');
const request={purpose:'Synthetic bridge read',currentRequestRef:'synthetic-bridge-request'};

async function installedBase(){
  const base=await mkdtemp(resolve(tmpdir(),'gates-1-bridge-'));
  const events=resolve(base,'events.txt');
  await writeFile(events,'');
  let n=0;
  const pin=async(name:string,value:unknown)=>{
    const bytes=typeof value==='string'?value:JSON.stringify(value),path=resolve(base,`${++n}-${name}`);
    await writeFile(path,bytes,{mode:0o600,flag:'wx'});return {path,sha256:hash(bytes)};
  };
  const admission=await pin('dailyrecord-admission.json',{schemaVersion:'academy.owner-admission/2',decider:'user',decidedAt:'2040-01-16',sourceNamespace:'academy.lms.dayrecord',
    teacherId:'99001',allowedFrom:'2040-01-15',allowedThrough:'2040-01-17',failClosedDefault:'2026-10-23',studentScope:'source_verified'});
  const metadata=await pin('metadata-admission.json',{schemaVersion:'academy.metadata-admission/1',sourceNamespace:'academy.lms.metadata',teacherId:'99001',
    allowedFrom:'2040-01-15',allowedThrough:'2040-01-17',studentScope:'source_verified',relativeSelectors:['1'],operations:['daily_inventory']});
  const routingValue={admissionPath:admission.path,admissionSha256:admission.sha256};
  const endpointsValue={dailyrecord:{command:process.execPath,args:['--preload',deny,upstreamFixture,events,
    '--dailyrecord-admission',admission.path,admission.sha256,'--metadata-admission',metadata.path,metadata.sha256]}};
  // The bridge's own discovery of the endpoint writes registry.json.
  await writeFile(resolve(base,'routing.json'),JSON.stringify(routingValue));
  await writeFile(resolve(base,'endpoints.json'),JSON.stringify(endpointsValue));
  const env={PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base};
  const discovery=Bun.spawn([process.execPath,'--preload',deny,entry,'--discover'],{env,stdout:'pipe',stderr:'pipe'});
  expect(await discovery.exited).toBe(0);
  const registry={path:resolve(base,'registry.json'),sha256:hash(await readFile(resolve(base,'registry.json')))};
  const endpoints=await pin('endpoints.json',endpointsValue),routing=await pin('routing.json',routingValue);
  let generation=0;
  const publish=async(members:Record<string,unknown>={})=>{
    const next=resolve(base,`release-${++generation}.tmp`);
    await writeFile(next,JSON.stringify({schemaVersion:'academy.mcp-release/1',generation:`gates-${generation}`,routing,endpoints,registry,...members}),{mode:0o600});
    await rename(next,resolve(base,'release.json'));
  };
  return {base,events,env,pin,publish};
}
async function connect(env:Record<string,string>){
  const client=new Client({name:'fictional-gates-client',version:'1.0.0'});
  const transport=new StdioClientTransport({command:process.execPath,args:['--preload',deny,entry],env,stderr:'pipe'});
  await client.connect(transport);
  return {client,close:async()=>{await client.close();await transport.close();}};
}
const outsideRead={...request,displayName:'Unlisted Student',lessonDate:'2041-06-30'};

test('check 6: an installed release with an admission member and old endpoint arguments lists and calls tools, with no admission handshake',async()=>{
  const f=await installedBase();
  try{
    await f.publish();
    const bridge=await connect(f.env);
    try{
      const names=(await bridge.client.listTools()).tools.map(tool=>tool.name);
      expect(names).toEqual(expect.arrayContaining(['academy_dailyrecord_get','academy_student_resolve','academy_student_metadata_get']));
      expect(names).not.toContain('academy_runtime_read_admission');
      const got:any=await bridge.client.callTool({name:'academy_dailyrecord_get',arguments:outsideRead});
      expect(got.isError).toBeFalsy();
      expect(got.structuredContent).toMatchObject({status:'observed',scope:{teacherId:'99001',studentId:'77009',lessonDate:'2041-06-30'}});
      const resolved:any=await bridge.client.callTool({name:'academy_student_resolve',arguments:{...request,displayName:'Unlisted Student'}});
      expect(resolved.structuredContent).toMatchObject({status:'resolved',studentKey:'77009'});
    }finally{await bridge.close();}
    const calls=(await readFile(f.events,'utf8')).split('\n').filter(line=>line.startsWith('call:'));
    expect(calls.length).toBeGreaterThan(0);
    // The upstream records every tool call: none is the old per-call admission handshake.
    expect(calls).not.toContain('call:academy_runtime_read_admission');
  }finally{await rm(f.base,{recursive:true,force:true});}
},120000);

test('check 6: a release without an admission member lists and calls tools',async()=>{
  const f=await installedBase();
  try{
    await f.publish({routing:await f.pin('routing-without-admission.json',{})});
    const bridge=await connect(f.env);
    try{
      expect((await bridge.client.listTools()).tools.map(tool=>tool.name)).toContain('academy_dailyrecord_get');
      const got:any=await bridge.client.callTool({name:'academy_dailyrecord_get',arguments:outsideRead});
      expect(got.structuredContent).toMatchObject({status:'observed'});
    }finally{await bridge.close();}
  }finally{await rm(f.base,{recursive:true,force:true});}
},120000);

test('check 6: a surface with {{admittedDates}} renders "any date" in tools/list and in the instructions',async()=>{
  const f=await installedBase();
  try{
    const surface={schemaVersion:'academy.mcp-surface/1',instructions:'Dates: {{admittedDates}}.',
      tools:[{name:'academy_dailyrecord_get',description:'Read a DailyRecord. Dates: {{admittedDates}}; again {{admittedDates}}.'}]};
    await f.publish({surface:await f.pin('surface.json',surface)});
    const bridge=await connect(f.env);
    try{
      expect(bridge.client.getInstructions()).toBe('Dates: any date.');
      const listed=(await bridge.client.listTools()).tools;
      expect(listed.map(tool=>tool.description)).toEqual(['Read a DailyRecord. Dates: any date; again any date.']);
    }finally{await bridge.close();}
  }finally{await rm(f.base,{recursive:true,force:true});}
},120000);
