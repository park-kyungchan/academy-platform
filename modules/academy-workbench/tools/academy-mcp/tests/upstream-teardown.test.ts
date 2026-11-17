import {expect,test} from 'bun:test';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';

const alive=(pid:number)=>{try{process.kill(pid,0);return true;}catch{return false;}};
const startedPids=async(events:string)=>[...(await readFile(events,'utf8')).matchAll(/^start (\d+)$/gm)].map(m=>Number(m[1]));

test('check 5: after a call whose endpoint hangs past the bound, a lookup in the same bridge process completes normally',async()=>{
  const base=await mkdtemp(resolve(tmpdir(),'academy-teardown-synthetic-'));
  const entry=resolve(import.meta.dir,'../src/neutral-server.ts'),deny=resolve(import.meta.dir,'network-deny.ts'),events=resolve(base,'events.txt');
  const admission={schemaVersion:'academy.owner-admission/1',decider:'user',decidedAt:'2040-01-16',sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',
    allowedFrom:'2040-01-15',allowedThrough:'2040-01-17',failClosedDefault:'2026-10-23'};
  const bytes=JSON.stringify(admission),admissionPath=resolve(base,'admission.json');
  // A short bound for the hanging call; the lock wait is shorter than the endpoint's hang.
  const env={PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base,ACADEMY_MCP_UPSTREAM_TIMEOUT_MS:'2000'};
  let client:Client|undefined,transport:StdioClientTransport|undefined;
  try{
    await writeFile(admissionPath,bytes);
    await writeFile(resolve(base,'routing.json'),JSON.stringify({admissionPath,admissionSha256:createHash('sha256').update(bytes).digest('hex')}));
    await writeFile(resolve(base,'endpoints.json'),JSON.stringify({dailyrecord:{command:'flock',
      args:['--wait','5',resolve(base,'account.lock'),process.execPath,'--preload',deny,resolve(import.meta.dir,'hanging-upstream-fixture.ts'),events]}}));
    const discovery=Bun.spawn([process.execPath,'--preload',deny,entry,'--discover'],{env,stdout:'pipe',stderr:'pipe'});
    expect(await discovery.exited).toBe(0);
    client=new Client({name:'fictional-teardown-client',version:'1.0.0'});
    transport=new StdioClientTransport({command:process.execPath,args:['--preload',deny,entry],env,stderr:'pipe'});
    await client.connect(transport);

    const trace:any=await client.callTool({name:'academy_item_section_trace',arguments:{label:'8101'}},{timeout:120000});
    expect(trace.structuredContent).toMatchObject({status:'blocked',phase:'upstream',endpoint:'dailyrecord',automaticRetryAllowed:false,academyWrites:0});
    const [, hanging]=await startedPids(events);
    expect(hanging).toBeGreaterThan(0);

    const startedAt=Date.now();
    const lookup:any=await client.callTool({name:'academy_item_answer_lookup',arguments:{label:'8101'}},{timeout:120000});
    expect(lookup.isError).not.toBe(true);
    expect(lookup.structuredContent).toEqual({status:'observed',label:'8101'});
    expect(Date.now()-startedAt).toBeLessThan(5000);
    expect((await readFile(events,'utf8')).split('\n').filter(line=>!line.startsWith('start '))).toEqual(['trace-started','lookup','']);
    // Nothing of the failed call is left: its endpoint process, which held the lock, is gone.
    expect(alive(hanging!)).toBe(false);
  }finally{
    await client?.close();await transport?.close();
    for(const pid of await startedPids(events).catch(()=>[]))try{process.kill(pid,'SIGKILL');}catch{}
    await rm(base,{recursive:true,force:true});
  }
},150000);
