import {expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {mkdtemp,readFile,writeFile,symlink,rm,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {preserveTextbookPdf,registerTextbookSourceTools} from '../src/textbook-source-tools.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';
const bytes=new Uint8Array(Buffer.from('%PDF-1.7\nsynthetic transport fixture\n%%EOF\n'));
const documentRef='a'.repeat(64),sha256=createHash('sha256').update(bytes).digest('hex');
const observation={schoolLevel:'middle' as const,kind:'sample' as const,documentRef,sha256,bytes,byteLength:bytes.length,filename:'g7_fictional_sample_1_1.pdf',sourceUrl:'https://storage.studyq.net/data/answer/mi/book/g7_fictional_sample_1_1.pdf',sourceLabels:['Invented Plus'],catalogSha256:'b'.repeat(64),observedAt:'2040-01-01T00:00:00Z',sourceRequests:2 as const};
test('content-addressed PDF custody reads back exact bytes, rejects tampering and never follows a target symlink',async()=>{
 const root=await mkdtemp(join(tmpdir(),'textbook-custody-'));
 try{
  const a=await preserveTextbookPdf(root,observation),b=await preserveTextbookPdf(root,observation);expect(a).toEqual(b);expect(await readFile(a.path)).toEqual(Buffer.from(bytes));
  await writeFile(a.path,'tampered');await expect(preserveTextbookPdf(root,observation)).rejects.toThrow('textbook_artifact_readback_rejected');
  await rm(a.path);const unrelated=join(root,'unrelated');await writeFile(unrelated,bytes);await symlink(unrelated,a.path);
  await expect(preserveTextbookPdf(root,observation)).rejects.toThrow('textbook_artifact_readback_rejected');expect(await readFile(unrelated)).toEqual(Buffer.from(bytes));
  await expect(preserveTextbookPdf(root,{...observation,sha256:'0'.repeat(64)})).rejects.toThrow('textbook_pdf_digest_rejected');
 }finally{await rm(root,{recursive:true,force:true});}
});
test('native owner backend authenticates once and binds both source reads to its principal',async()=>{
 let auth=0;const calls:unknown[]=[];
 const port={authenticate:async()=>{auth++;},discoverDayRecordGroups:async()=>({groups:[]}),execute:async()=>({dayRecords:[]}),journalState:async()=>{throw Error('unused');},close:()=>{},
  readTextbookCatalog:async(input:unknown)=>{calls.push(input);return {schemaVersion:1 as const,status:'observed' as const,schoolLevel:'middle' as const,observedAt:observation.observedAt,catalogSha256:observation.catalogSha256,entries:[],coverage:'all_observed_pdf_links_in_one_catalog_document' as const,sourceRequests:1 as const};},
  readTextbookPdf:async(input:unknown)=>{calls.push(input);return observation;}};
 const backend=new NativeDailyRecordBackend(port,{teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-01',students:[],sourceBundleSha256:'b'.repeat(64)},()=>({username:'fictional',password:'fictional'}));
 await backend.readTextbookCatalog();await backend.readTextbookPdf('middle',documentRef);expect(auth).toBe(1);
 expect(calls).toEqual([{teacherLabel:'Invented Teacher',schoolLevel:'middle'},{teacherLabel:'Invented Teacher',schoolLevel:'middle',documentRef}]);
 await expect(backend.readTextbookPdf('middle','bad')).rejects.toThrow('textbook_pdf_input_rejected');expect(calls).toHaveLength(2);
});
test('bounded tool output filters catalog labels and returns artifact metadata without PDF bytes or credentials',async()=>{
 const root=await mkdtemp(join(tmpdir(),'textbook-tools-'));const tools=new Map<string,any>();
 const server={registerTool:(name:string,config:unknown,handler:unknown)=>tools.set(name,{config,handler})};
 const catalog={schemaVersion:1 as const,status:'observed' as const,schoolLevel:'middle' as const,observedAt:observation.observedAt,catalogSha256:observation.catalogSha256,entries:[{kind:'sample' as const,documentRef,url:observation.sourceUrl,filename:observation.filename,sourceLabels:['Invented Plus']}],coverage:'all_observed_pdf_links_in_one_catalog_document' as const,sourceRequests:1 as const};
 try{
  registerTextbookSourceTools(server as any,{readTextbookCatalog:async()=>catalog,readTextbookPdf:async()=>observation},root);
  const catalogTool=tools.get('academy_textbook_catalog'),pdfTool=tools.get('academy_textbook_pdf');
  expect(catalogTool.config.annotations.readOnlyHint).toBe(true);expect(pdfTool.config.annotations.destructiveHint).toBe(false);
  const result=await catalogTool.handler(catalogTool.config.inputSchema.parse({query:'Plus',limit:100}));expect(result.structuredContent.page).toMatchObject({totalMatched:1,returned:1,nextOffset:null});
  expect(()=>catalogTool.config.inputSchema.parse({limit:101})).toThrow();expect(()=>pdfTool.config.inputSchema.parse({documentRef,url:'https://evil.example'})).toThrow();
  const downloaded=await pdfTool.handler({schoolLevel:'middle',documentRef});expect(downloaded.structuredContent.status).toBe('observed');expect(downloaded.structuredContent.artifact.sha256).toBe(sha256);
  expect(downloaded.structuredContent.bytes).toBeUndefined();expect(JSON.stringify(downloaded)).not.toContain('%PDF-');expect(JSON.stringify(downloaded)).not.toContain('password');expect(JSON.stringify(downloaded)).not.toContain('username');
  expect(downloaded.structuredContent.completion).toBe('not_established');
 }finally{await rm(root,{recursive:true,force:true});}
});
test('source failure codes are finite and never copy unexpected upstream text into the public MCP result',async()=>{
 const tools=new Map<string,any>(),server={registerTool:(name:string,config:unknown,handler:unknown)=>tools.set(name,{config,handler})};
 registerTextbookSourceTools(server as any,{readTextbookCatalog:async()=>{throw Error('textbook_unexpected_private_payload');},readTextbookPdf:async()=>{throw Error('unused');}},'/unused');
 const tool=tools.get('academy_textbook_catalog'),value=await tool.handler(tool.config.inputSchema.parse({}));
 expect(value.structuredContent).toMatchObject({status:'failed',code:'textbook_source_read_failed',automaticRetryAllowed:false});
 expect(JSON.stringify(value)).not.toContain('private_payload');
});
