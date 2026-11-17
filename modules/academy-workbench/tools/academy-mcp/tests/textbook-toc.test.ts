import {expect,test} from 'bun:test';
import {mkdtemp,readFile,rm,writeFile} from 'node:fs/promises';
import {readFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';

// TASK-TOC-1. The TOC of every catalog sample book as pinned data, served by academy_textbook_toc_get
// with no LMS request, no network, no PDF download and no image read.
const deny=resolve(import.meta.dir,'network-deny.ts'),endpoint=resolve(import.meta.dir,'../src/dailyrecord-server.ts');
const dataPath=resolve(import.meta.dir,'../data/textbook-toc.json'),script=resolve(import.meta.dir,'../scripts/build-textbook-toc.ts');
// The 147 sample PDF file names of the catalog (elementary, middle and high school), 2026-11-17.
const catalogSamples='g10_high_davinci_sample_1 g10_high_davinci_sample_2 g10_high_davinci_sample_3 g10_high_gauss_sample_1 g10_high_gauss_sample_2 g10_high_gauss_sample_3 g10_high_sigma_sample_1 g10_low_davinci_sample_1 g10_low_davinci_sample_2 g10_low_davinci_sample_3 g10_low_gauss_sample_1 g10_low_gauss_sample_2 g10_low_sigma_sample_1 g10_one_gauss_sample_1 g10_one_gauss_sample_2 g10_one_gauss_sample_3 g10_one_sigma_sample_1 g10_two_gauss_sample_1 g10_two_gauss_sample_2 g10_two_gauss_sample_3 g10_two_sigma_sample_1 g11_algebra_gauss_sample_1 g11_algebra_gauss_sample_2 g11_algebra_gauss_sample_3 g11_algebra_sigma_sample_1 g11_calculus1_gauss_sample_1 g11_calculus1_gauss_sample_2 g11_calculus1_gauss_sample_3 g11_calculus1_sigma_sample_1 g11_calculus2_gauss_sample_1 g11_calculus2_gauss_sample_2 g11_calculus2_gauss_sample_3 g11_calculus2_gauss_sample_4 g11_calculus_gauss_sample_1 g11_calculus_gauss_sample_2 g11_calculus_gauss_sample_3 g11_calculus_gauss_sample_4 g11_geometry_gauss_sample_1 g11_geometry_gauss_sample_2 g11_geometry_gauss_sample_3 g11_one_davinci_sample_1 g11_one_davinci_sample_2 g11_one_davinci_sample_3 g11_one_gauss_sample_1 g11_one_gauss_sample_2 g11_one_gauss_sample_3 g11_one_sigma_sample_1 g11_statistics_davinci_sample_1 g11_statistics_davinci_sample_2 g11_statistics_davinci_sample_3 g11_statistics_gauss_sample_1 g11_statistics_gauss_sample_2 g11_statistics_gauss_sample_3 g11_stats_gauss_sample_1 g11_stats_gauss_sample_2 g11_stats_gauss_sample_3 g11_two_davinci_sample_1 g11_two_davinci_sample_2 g11_two_davinci_sample_3 g11_two_gauss_sample_1 g11_two_gauss_sample_2 g11_two_gauss_sample_3 g11_two_sigma_sample_1 g2_basic_sample_1_1 g2_basic_sample_2_1 g3_basic_sample_1_1 g3_basic_sample_1_2 g3_basic_sample_2_1 g3_basic_sample_2_2 g4_ability_sample_1_1 g4_ability_sample_1_2 g4_ability_sample_2_1 g4_ability_sample_2_2 g4_develop_sample_1_1 g4_develop_sample_1_2 g4_develop_sample_2_1 g4_develop_sample_2_2 g4_highest_sample_1_1 g4_highest_sample_2_1 g4_sigma_sample_1_1 g4_sigma_sample_1_2 g4_sigma_sample_2_1 g4_sigma_sample_2_2 g5_ability_sample_1_1 g5_ability_sample_1_2 g5_ability_sample_2_1 g5_ability_sample_2_2 g5_develop_sample_1_1 g5_develop_sample_1_2 g5_develop_sample_2_1 g5_develop_sample_2_2 g5_highest_sample_1_1 g5_highest_sample_2_1 g5_sigma_sample_1_1 g5_sigma_sample_1_2 g5_sigma_sample_2_1 g5_sigma_sample_2_2 g6_ability_sample_1_1 g6_ability_sample_1_2 g6_ability_sample_2_1 g6_ability_sample_2_2 g6_develop_sample_1_1 g6_develop_sample_1_2 g6_develop_sample_2_1 g6_develop_sample_2_2 g6_highest_sample_1_1 g6_highest_sample_2_1 g6_sigma_sample_1_1 g6_sigma_sample_1_2 g6_sigma_sample_2_1 g6_sigma_sample_2_2 g7_davinci_sample_1_1 g7_davinci_sample_2_3 g7_euler_sample_1_1 g7_euler_sample_2_3 g7_gauss_sample_1_1 g7_gauss_sample_1_2 g7_gauss_sample_2_3 g7_gauss_sample_2_4 g7_pascal_sample_1_1 g7_pascal_sample_2_3 g7_sigma_sample_1_1 g7_sigma_sample_2_3 g8_davinci_sample_1_1 g8_davinci_sample_2_3 g8_euler_sample_1_1 g8_euler_sample_2_3 g8_gauss_sample_1_1 g8_gauss_sample_1_2 g8_gauss_sample_2_3 g8_gauss_sample_2_4 g8_pascal_sample_1_1 g8_pascal_sample_2_3 g8_sigma_sample_1_1 g8_sigma_sample_2_3 g9_davinci_sample_1_1 g9_davinci_sample_2_3 g9_euler_sample_1_1 g9_euler_sample_2_3 g9_gauss_sample_1_1 g9_gauss_sample_1_2 g9_gauss_sample_2_3 g9_gauss_sample_2_4 g9_pascal_sample_1_1 g9_pascal_sample_2_3 g9_sigma_sample_1_1 g9_sigma_sample_2_3'.split(' ').map(stem=>stem+'.pdf');
const lmsLabel='(22개정) 초5-1 가우스 2권 초5-1 발전 2권(2022개정)';

async function connect(command:string[],env:Record<string,string>={PATH:process.env.PATH??'/usr/bin:/bin'}){
  const client=new Client({name:'fictional-toc-client',version:'1.0.0'});
  const transport=new StdioClientTransport({command:command[0]!,args:command.slice(1),env,stderr:'pipe'});
  await client.connect(transport);
  return {client,close:async()=>{await client.close();await transport.close();}};
}
const call=async(client:Client,args:Record<string,unknown>)=>(await client.callTool({name:'academy_textbook_toc_get',arguments:args})).structuredContent as any;

test('check 1: a canonical name, an LMS bound-book label, a name without volume and a unit filter, over the committed data',async()=>{
  const {client,close}=await connect([process.execPath,'--preload',deny,endpoint]);
  try{
    const byName=await call(client,{book:'가우스 5-1-2'});
    expect(byName).toMatchObject({status:'observed',sourceMode:'pinned_toc_data',sourceRequests:0,pdfDownloads:0,imageReads:0});
    expect(byName.records).toHaveLength(1);
    const [book]=byName.records;
    expect(book).toMatchObject({book:'가우스 5-1-2',filename:'g5_develop_sample_1_2.pdf',schoolLevel:'elementary'});
    expect(book.units).toContainEqual({chapter:'6',chapterTitle:'다각형의 둘레와 넓이',code:'6-5',title:'마름모와 사다리꼴의 넓이',pages:'158'});
    expect(book.units).toContainEqual({chapter:'6',chapterTitle:'다각형의 둘레와 넓이',code:null,title:'대단원 총괄평가',pages:'178'});
    expect(book.toc).toMatch(/5\. 마름모와 사다리꼴의 넓이\s+158/);
    expect(book.toc).toMatch(/§ 대단원 총괄평가\s+178/);
    const byLabel=await call(client,{book:lmsLabel});
    expect(byLabel.status).toBe('observed');
    expect(byLabel.records).toEqual(byName.records);
    const bothVolumes=await call(client,{book:'가우스 5-1'});
    expect(bothVolumes.records.map((r:any)=>r.book)).toEqual(['가우스 5-1-1','가우스 5-1-2']);
    const chapter=await call(client,{book:'가우스 5-1-2',unit:'6'});
    const units=chapter.records[0].units;
    expect(units.map((u:any)=>u.code)).toEqual(['6-1','6-2','6-3','6-4','6-5',null]);
    expect(chapter.records[0].toc).toContain('158');
    expect(chapter.records[0].toc).not.toContain('진분수의 덧셈');
    expect((await call(client,{book:'가우스 5-1-2',unit:'6-5'})).records[0].units).toEqual(units);
    expect((await call(client,{book:'g5_develop_sample_1_2.pdf'})).records).toEqual(byName.records);
  }finally{await close();}
},30000);

test('check 2: a name that no record has answers not_found with canonical suggestions of the same brand or grade',async()=>{
  const {client,close}=await connect([process.execPath,'--preload',deny,endpoint]);
  try{
    const missing=await call(client,{book:'가우스 5-3-9'});
    expect(missing).toMatchObject({status:'not_found',sourceRequests:0,pdfDownloads:0,imageReads:0});
    expect(missing.suggestions.length).toBeGreaterThan(0);
    expect(missing.suggestions.length).toBeLessThanOrEqual(20);
    expect(missing.suggestions).toContain('가우스 5-1-2');
    const unknown=await call(client,{book:'없는 교재'});
    expect(unknown.status).toBe('not_found');
    expect(Array.isArray(unknown.suggestions)).toBe(true);
  }finally{await close();}
},30000);

test('check 3: every catalog sample has a record with TOC text or outline units, no body page and a unique book name',()=>{
  const data=JSON.parse(readFileSync(dataPath,'utf8'));
  expect(data.schemaVersion).toBe('academy.textbook-toc/1');
  expect(catalogSamples).toHaveLength(147);
  const byFile=new Map<string,any>(data.records.map((r:any)=>[r.filename,r]));
  for(const filename of catalogSamples)expect(byFile.has(filename)).toBe(true);
  expect(data.records).toHaveLength(147);
  for(const record of data.records){
    if(record.noTextToc===true)expect(record.toc).toBeNull();
    else expect(record.toc!==null||record.outline.length>0).toBe(true);
    if(record.toc!==null){
      // Exercise wording appears on body pages, never on a TOC page.
      expect(record.toc).not.toMatch(/써넣으시오|구하시오|구하여라|고르시오/);
      expect(record.toc).not.toMatch(/ +$/m);
      expect(record.toc).not.toMatch(/월\s+일\s*$/m);
      expect(record.outline).toEqual([]);
    }
    for(const item of record.outline)expect(item).not.toMatch(/^(책갈피|EXAM_AREA|OBJ_AREA|정답)|_시작$|_끝$/);
    expect(record.sourceSha256).toMatch(/^[a-f0-9]{64}$/);
  }
  const books=data.records.map((r:any)=>r.book);
  expect(new Set(books).size).toBe(books.length);
  expect(readFileSync(dataPath,'utf8')).not.toMatch(/https?:|storage\.studyq/);
});

test('check 4: the build script gives the same bytes on two runs over the same raw extraction',async()=>{
  const dir=await mkdtemp(resolve(tmpdir(),'toc-build-'));
  try{
    const raw=resolve(import.meta.dir,'textbook-toc-raw'),outs=[resolve(dir,'a.json'),resolve(dir,'b.json')];
    for(const out of outs){
      const run=Bun.spawn([process.execPath,script,'--raw',raw,'--out',out],{stdout:'pipe',stderr:'pipe'});
      expect(await run.exited).toBe(0);
    }
    const [a,b]=await Promise.all(outs.map(out=>readFile(out)));
    expect(createHash('sha256').update(a!).digest('hex')).toBe(createHash('sha256').update(b!).digest('hex'));
    const records=JSON.parse(a!.toString()).records;
    expect(records.map((r:any)=>r.book)).toEqual(['가우스 5-1-2','시그마 중1-1']);
    // The plan table: the body page that the loose extraction matched (page 18) is not in the TOC.
    expect(records[0].toc).not.toContain('써넣으시오');
    expect(records[0].units.find((u:any)=>u.code==='6-5')).toMatchObject({title:'마름모와 사다리꼴의 넓이',pages:'158'});
    // A coded list without the plan-table words is a TOC page as well.
    expect(records[1].units.slice(0,2)).toEqual([{chapter:'1',chapterTitle:null,code:'1-1',title:'소인수분해',pages:'6'},{chapter:'1',chapterTitle:null,code:'1-2',title:'최대공약수',pages:'15'}]);
  }finally{await rm(dir,{recursive:true,force:true});}
},30000);

test('check 5: tools/list of the endpoint and of the bridge includes the tool, and the bridge routes a call to it',async()=>{
  const direct=await connect([process.execPath,'--preload',deny,endpoint]);
  try{
    const tools=(await direct.client.listTools()).tools;
    const toc=tools.find(t=>t.name==='academy_textbook_toc_get')!;
    expect(toc.annotations?.readOnlyHint).toBe(true);
    expect(toc.annotations?.openWorldHint).toBe(false);
    expect(toc.description).toMatch(/no PDF download/i);
    for(const name of ['academy_textbook_units_get','academy_textbook_pdf'])
      expect(tools.find(t=>t.name===name)!.description).toContain('academy_textbook_toc_get first');
    expect(tools.find(t=>t.name==='academy_textbook_units_get')!.description).not.toMatch(/Hindsight/);
  }finally{await direct.close();}
  const base=await mkdtemp(resolve(tmpdir(),'toc-bridge-'));
  try{
    await writeFile(resolve(base,'endpoints.json'),JSON.stringify({dailyrecord:{command:process.execPath,args:['--preload',deny,endpoint]}}));
    await writeFile(resolve(base,'routing.json'),'{}');
    const env={PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base},bridgeEntry=resolve(import.meta.dir,'../src/neutral-server.ts');
    const discovery=Bun.spawn([process.execPath,'--preload',deny,bridgeEntry,'--discover'],{env,stdout:'pipe',stderr:'pipe'});
    expect(await discovery.exited).toBe(0);
    const registry=JSON.parse(await readFile(resolve(base,'registry.json'),'utf8'));
    expect(registry.find((r:any)=>r.tool.name==='academy_textbook_toc_get')).toMatchObject({endpoint:'dailyrecord',upstreamName:'academy_textbook_toc_get'});
    const bridge=await connect([process.execPath,'--preload',deny,bridgeEntry],env);
    try{
      const listed=(await bridge.client.listTools()).tools;
      expect(listed.map(t=>t.name)).toContain('academy_textbook_toc_get');
      expect(listed.find(t=>t.name==='academy_textbook_toc_get')!.description).not.toMatch(/Hindsight/);
      const routed=await call(bridge.client,{book:lmsLabel});
      expect(routed).toMatchObject({status:'observed',sourceRequests:0,pdfDownloads:0,imageReads:0});
      expect(routed.records[0].book).toBe('가우스 5-1-2');
    }finally{await bridge.close();}
  }finally{await rm(base,{recursive:true,force:true});}
},60000);
