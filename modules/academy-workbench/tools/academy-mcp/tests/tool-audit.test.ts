import {test,expect} from 'bun:test';
import {mkdtemp,readFile,readdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {publicReadFailure,publicFailureFromData,AcademyReadFailure} from '../src/read-failure.ts';
import {UpstreamToolFailure} from '../src/upstream-failure.ts';
test('safe failure causes preserve HTTP stage without arbitrary upstream values',()=>{
 const cause=Object.assign(Error('source_http_failed'),{httpStatus:503,sourceOperation:'login_document',body:'PRIVATE_BODY'});
 expect(publicReadFailure(new AcademyReadFailure('authentication','source_authentication_failed','ignored',cause))).toMatchObject({category:'authentication',cause:{code:'source_http_failed',sourceHttpStatus:503,sourceOperation:'login_document'}});
 expect(new UpstreamToolFailure({isError:true,content:[{type:'text',text:'Input validation error: PRIVATE'}]}).failure.category).toBe('input_validation');
 expect(JSON.stringify(new UpstreamToolFailure({isError:true,structuredContent:{code:'PRIVATE_SECRET'},content:[]}).failure)).not.toContain('PRIVATE');
});
test('all finite name resolver reasons survive the audit projection',()=>{
 for(const [code,category] of [
  ['student_name_invalid','input_validation'],['student_resolve_backend_not_configured','endpoint_absence'],
  ['student_name_scope_conflict','contract_drift'],
  ['student_name_not_found','partial_coverage'],['student_name_ambiguous','identity_ambiguity'],
  ['student_resolve_search_unqualified_no_automatic_retry','upstream_unavailable'],
  ['student_resolve_unqualified_no_automatic_retry','upstream_unavailable'],
 ] as const)expect(publicFailureFromData({code})).toMatchObject({code,category});
});
test('audited SDK validates before execution, preserves safe failures and uncertain writes, and stores no private payload',async()=>{
 const root=await mkdtemp(resolve(tmpdir(),'academy-audit-')),events=resolve(root,'events');
 const client=new Client({name:'fictional-audit-test',version:'1'}),transport=new StdioClientTransport({command:process.execPath,args:[resolve(import.meta.dir,'tool-audit-fixture.ts'),resolve(root,'audit'),events],env:{PATH:process.env.PATH??'/usr/bin:/bin'},stderr:'pipe'});
 try{
  await client.connect(transport);
  const tools=(await client.listTools()).tools;expect(tools[0].inputSchema.required).toContain('value');
  const invalid=await client.callTool({name:'fictional_read',arguments:{value:'PRIVATE_VALUE',PRIVATE_KEY:'PRIVATE_VALUE'}});
  expect(invalid.structuredContent).toMatchObject({status:'blocked',code:'tool_input_invalid',academyWrites:0});
  expect((invalid.structuredContent as any).issues.some((i:{path:unknown[]})=>i.path.join('.')==='value')).toBe(true);
  expect(await Bun.file(events).exists()).toBe(false);expect(JSON.stringify(invalid)).not.toContain('PRIVATE');
  for(const [mode,code] of [['known','prestudy_search_form_drift'],['private','tool_handler_failed'],['auth','source_authentication_failed'],['valid',undefined]]){
   const out=await client.callTool({name:'fictional_read',arguments:{value:1,mode}});
   if(mode==='auth')expect(out.structuredContent).toMatchObject({failure:{cause:{sourceHttpStatus:503,sourceOperation:'login_document'}}});
   if(code)expect(out.structuredContent).toMatchObject({status:'blocked',code,academyWrites:0});else expect(out.structuredContent).toMatchObject({status:'observed'});
   expect(out._meta?.['academy/audit']).toMatchObject({tool:'fictional_read',endpoint:'synthetic',phase:'handler'});expect(JSON.stringify(out)).not.toContain('PRIVATE');
  }
  const write=await client.callTool({name:'fictional_write',arguments:{}});expect(write.structuredContent).toMatchObject({status:'blocked',academyWrites:null,effectStatus:'unknown'});
  const receipts=await Promise.all((await readdir(resolve(root,'audit'))).map(n=>readFile(resolve(root,'audit',n),'utf8')));
  expect(receipts).toHaveLength(12);expect(receipts.join('')).not.toContain('PRIVATE');expect(receipts.join('')).not.toContain('body');
  expect((await readFile(events,'utf8')).match(/effect/g)).toHaveLength(1);
 }finally{await client.close();await transport.close();await rm(root,{recursive:true,force:true});}
},20000);

test('release inventory cannot advertise CISM or any other absent upstream tool',async()=>{
 const {verifyEndpointInventory,EndpointInventoryMismatch}=await import('../src/endpoint-inventory.ts');
 const registry=[{endpoint:'dailyrecord',upstreamName:'academy_dailyrecord_review'},{endpoint:'dailyrecord',upstreamName:'academy_cism_weekly_get'}];
 expect(()=>verifyEndpointInventory('dailyrecord',registry,[{name:'academy_dailyrecord_review'}])).toThrow(EndpointInventoryMismatch);
 expect(verifyEndpointInventory('dailyrecord',registry,[{name:'academy_dailyrecord_review'},{name:'academy_cism_weekly_get'}])).toMatchObject({expectedTools:2,listedTools:2});
});
