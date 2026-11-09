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

test('unit lookup reuses exact retained original with no source calls; refresh is explicit and bounded',async()=>{
 const root=await mkdtemp(join(tmpdir(),'textbook-units-')),tools=new Map<string,any>();let reads=0,readerCalls=0;
 const server={registerTool:(name:string,config:unknown,handler:unknown)=>tools.set(name,{config,handler})};
 const backend={readTextbookCatalog:async()=>{throw Error('unused');},readTextbookPdf:async()=>{reads++;return observation;}};
 const reader={read:async(path:string,sha:string,request:any)=>{readerCalls++;expect(path).toBe(join(root,sha256+'.pdf'));expect(sha).toBe(sha256);expect(request.pages).toEqual([10]);return {schemaVersion:1 as const,pdfSha256:sha,pageCount:12,scannedPages:[10],units:[{code:'4-3',title:'Fictional unit',majorTitle:null,middleTitle:'04. Fictional group',printedPage:12,pdfPage:10,qualification:'text_verified' as const}],missingUnitCodes:[],coverage:'selected_pages_only' as const,requiresVisualVerification:false,reasonCodes:['selected_pages_only'],textCandidates:[]};}};
 try{
  await preserveTextbookPdf(root,observation);registerTextbookSourceTools(server as any,backend,root,reader);
  const t=tools.get('academy_textbook_units_get');
  const input=t.config.inputSchema.parse({source:{kind:'retained_pdf',sha256},unitCodes:['4-3'],pages:[10]});
  const a=(await t.handler(input)).structuredContent;expect(a).toMatchObject({status:'observed',sourceMode:'preserved_pdf_artifact',sourceObservedAt:null,catalogMembership:'not_refreshed',sourceRequests:0,pdfDownloads:0,imageReads:0,studentBinding:'not_established'});expect(reads).toBe(0);expect(readerCalls).toBe(1);
  await t.handler(t.config.inputSchema.parse({source:{kind:'catalog_pdf',schoolLevel:'middle',documentRef},pages:[10]}));expect(reads).toBe(1);
  for(const source of [{kind:'retained_pdf',sha256,path:'/etc/passwd'},{kind:'retained_pdf',sha256:'../escape'},{kind:'catalog_pdf',documentRef,url:'https://evil.example'}])expect(()=>t.config.inputSchema.parse({source})).toThrow();
  expect(()=>t.config.inputSchema.parse({source:{kind:'retained_pdf',sha256},pages:[1,1]})).toThrow();
  expect(()=>t.config.inputSchema.parse({source:{kind:'retained_pdf',sha256},scanPages:25})).toThrow();
  await writeFile(join(root,sha256+'.pdf'),'corrupt');const b=(await t.handler(input)).structuredContent;expect(b.status).toBe('failed');expect(readerCalls).toBe(2);expect(reads).toBe(1);
 }finally{await rm(root,{recursive:true,force:true});}
});
test('missing reader does not acquire a PDF, and glyph ambiguity is not completion',async()=>{
 const root=await mkdtemp(join(tmpdir(),'textbook-reader-unavailable-')),tools=new Map<string,any>();let reads=0;
 const server={registerTool:(n:string,c:any,h:any)=>tools.set(n,{config:c,handler:h})};
 try{
  registerTextbookSourceTools(server as any,{readTextbookCatalog:async()=>{throw Error('unused');},readTextbookPdf:async()=>{reads++;return observation;}},root);
  const t=tools.get('academy_textbook_units_get');expect((await t.handler(t.config.inputSchema.parse({source:{kind:'catalog_pdf',documentRef}}))).structuredContent.code).toBe('textbook_pdf_reader_not_configured');expect(reads).toBe(0);
 }finally{await rm(root,{recursive:true,force:true});}
});
