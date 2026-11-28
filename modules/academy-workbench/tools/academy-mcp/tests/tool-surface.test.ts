import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {mkdtemp,readFile,rm,writeFile,rename} from 'node:fs/promises';
import {readFileSync,readdirSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {consolidatedTools,keptTools,surfaceOrder,baselineSurfaceOrder,newSurfaceTools,planConsolidatedCall,ConsolidatedInputInvalid,compositionEntries,compositionUpstreams} from '../src/consolidated-tools.ts';
import {surfaceText} from '../scripts/build-surface.ts';
import {lmsRequestRole,writeForbiddenTransport} from './network-deny.ts';
import {AcademyHttpClient} from '../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';
import {clock,context,learningPolicy,safeguard} from './dailyrecord-fixture.ts';

// specs/academy-mcp/tool-surface.md: release/surface.json, the four Academy skills, and the
// 15 tools driven through the bridge (src/neutral-server.ts) over recorded synthetic
// endpoints (tests/tool-surface-fixture.ts). Invented values only; no LMS, network or host path.
const root=resolve(import.meta.dir,'..'),repo=resolve(root,'../../../..');
const entry=resolve(root,'src/neutral-server.ts'),deny=resolve(import.meta.dir,'network-deny.ts'),fixture=resolve(import.meta.dir,'tool-surface-fixture.ts');
const surface=JSON.parse(readFileSync(resolve(root,'release/surface.json'),'utf8')) as {schemaVersion:string;tools:{name:string;title?:string;description:string;inputSchema?:any}[]};
const compactBytes=(tools:unknown)=>Buffer.byteLength(JSON.stringify(tools),'utf8');
const names=new Set(surface.tools.map(t=>t.name));
const hash=(bytes:string|Buffer)=>createHash('sha256').update(bytes).digest('hex');

/** The table "Old to new": removed, kept, or [new tool, its discriminating arguments]. */
const oldToNew:Record<string,'removed'|'kept'|[string,Record<string,unknown>]>={
 academy_item_label_scope:'removed',academy_find_item_labels:'removed',academy_review_type_questions:'removed',academy_cism_section:'removed',
 academy_student_metadata:'removed',academy_student_metadata_get:'removed',academy_textbook_section_results:'removed',class2_academy_paper_headers:'removed',
 academy_query:'removed',academy_source_status:'removed',
 academy_runtime_identity:['academy_runtime_inspect',{view:'identity'}],
 academy_capabilities:['academy_runtime_inspect',{view:'capabilities'}],
 academy_source_read_contracts:['academy_runtime_inspect',{view:'source_contracts'}],
 academy_paper_read_contracts:['academy_runtime_inspect',{view:'paper_contracts'}],
 academy_dailyrecord_get:['academy_dailyrecord_get',{view:'record'}],
 academy_dailyrecord_intake:['academy_dailyrecord_get',{view:'intake'}],
 academy_dailyrecord_execution_status:['academy_dailyrecord_get',{view:'status'}],
 academy_dailyrecord_reconcile:['academy_dailyrecord_get',{view:'reconcile'}],
 academy_dailyrecord_review:'kept',academy_dailyrecord_save:'kept',academy_student_resolve:'kept',
 academy_daily_test_inventory:['academy_dailytest_history',{includeHeaders:false,studentId:'77001'}],
 academy_dailytest_history_with_headers:['academy_dailytest_history',{includeHeaders:true,studentName:'Invented Student'}],
 academy_paper_headers:['academy_paper_inspect',{view:'headers'}],
 academy_paper_definition_lookup:['academy_paper_inspect',{view:'definition'}],
 academy_printed_paper_lookup:['academy_paper_inspect',{view:'attempt'}],
 academy_textbook_section_results_get:'kept',academy_clinic_paper_status_get:'kept',
 academy_item_section_trace:['academy_item_lookup',{include:['section']}],
 academy_item_answer_lookup:['academy_item_lookup',{include:['answer']}],
 academy_textbook_catalog:['academy_textbook_catalog_get',{view:'catalog'}],
 academy_textbook_toc_get:['academy_textbook_catalog_get',{view:'toc'}],
 academy_textbook_pdf:['academy_textbook_catalog_get',{view:'pdf'}],
 academy_prestudy_get:['academy_prestudy_get',{view:'videos'}],
 academy_prestudy_report_get:['academy_prestudy_get',{includeReports:true,reportId:'5002'}],
 academy_cism_weekly_get:['academy_cism_get',{scope:'week'}],
 academy_cism_month:['academy_cism_get',{scope:'month'}],
 academy_cism_weekly_review:'kept',academy_cism_weekly_save:'kept',
};
/** The endpoint tool of each mapped old tool (its upstream name); the bridge serves the rest. */
const oldUpstream:Record<string,string>={academy_runtime_identity:'academy_runtime_identity',academy_dailyrecord_get:'academy_dailyrecord_get',
 academy_dailytest_history_with_headers:'academy_student_metadata_get'};
/** Each Hangul term of the descriptions of the old tools that a tool replaces, at commit 76d04632 (R14). */
const hangul:Record<string,string[]>={
 academy_runtime_inspect:['세대','브리지','해시','이','로','할','수','있는','일','출력코드','변환기','시험','검색','폼','읽기','계약'],
 academy_dailyrecord_get:['수업일지','일지','진도','숙제','메모','출결','기록','조회','오늘','어제','월','일','저장','실행','상태','연결','끊김','들어갔는지','확인'],
 academy_dailytest_history:['결과','목록','이력','시험','기록','몇','개','쳤는지','오늘','어제','월','일','전부'],
 academy_paper_inspect:['시험지','제목','뭐였는지','이름','문항','수','출력코드'],
 academy_item_lookup:['가우스','문항','번호'],
 academy_textbook_catalog_get:['교재','카탈로그','답지','초등','중등','고등','대단원','총괄평가','가우스','초','권','학년','학기'],
 academy_prestudy_get:['예습영상','검사','대기','예습','점수','최근'],
 academy_cism_get:['오늘','어제','월','일','월별','빈','칸','몇','개'],
 academy_dailyrecord_review:['메모','코멘트','진도','숙제','까지'],
 academy_student_resolve:['오늘','어제','월','일'],
 academy_textbook_section_results_get:['교재','소단원','필수예제','유형다지기','가우스','다빈치','권','초'],
 academy_clinic_paper_status_get:['오답클리닉','유사클리닉','틀린','문항','오답'],
 academy_official_report_pdf_get:['공식','리포트','교재'],
};
const skillPaths=readdirSync(resolve(repo,'content/skills')).filter(d=>d.startsWith('academy-')).map(d=>resolve(repo,'content/skills',d,'SKILL.md')).sort();
const toolPattern=/\b(?:class2_)?academy_[a-z0-9_]+\b/g;

describe('the surface of release/surface.json',()=>{
 test('tool_surface_r9_built_surface',()=>{
  expect(readFileSync(resolve(root,'release/surface.json'),'utf8')).toBe(surfaceText());
 });
 test('tool_surface_r1_r8_fifteen_tools',()=>{
  // R1 bounds the 15 tools of the consolidation; the new tools added since are bounded by R2.
  expect(surface.tools.map(t=>t.name)).toEqual([...surfaceOrder]);
  expect(surfaceOrder).toEqual([...baselineSurfaceOrder,...newSurfaceTools]);
  const baseline=surface.tools.filter(t=>(baselineSurfaceOrder as readonly string[]).includes(t.name));
  expect(baseline).toHaveLength(15);
  expect(names.size).toBe(surface.tools.length);
  for(const name of names){expect(name).toMatch(/^academy_[a-z0-9_]+$/);expect(name.startsWith('class2_')).toBe(false);}
  expect(compactBytes(baseline)).toBeLessThanOrEqual(18432);
 });
 test('tool_surface_r2_running_cap',()=>{
  const planned=['academy_official_report_pdf_get','academy_paper_print_pdf_get','academy_lms_schedule_get','academy_clinic_paper_review',
   'academy_clinic_paper_save','academy_exam_paper_review','academy_exam_paper_save'];
  const remaining=planned.filter(n=>!names.has(n)).length;
  expect(compactBytes(surface.tools)).toBeLessThanOrEqual(24000-795*remaining);
  expect(surface.tools.length).toBeLessThanOrEqual(24-remaining);
 });
 test('tool_surface_r3_accounting',()=>{
  expect(Object.keys(oldToNew)).toHaveLength(39);
  for(const [old,row] of Object.entries(oldToNew)){
   if(row==='removed'){expect({old,listed:names.has(old)}).toEqual({old,listed:false});continue;}
   if(row==='kept'){expect({old,listed:names.has(old)}).toEqual({old,listed:true});continue;}
   const [tool,selector]=row;
   expect({old,tool,listed:names.has(tool),oldListed:old===tool||names.has(old)}).toEqual({old,tool,listed:true,oldListed:old===tool});
   const plan=planConsolidatedCall(tool,{...selector,...minimalInputs[old]});
   expect({old,upstream:plan.steps.at(-1)!.upstream}).toEqual({old,upstream:oldUpstream[old]??old});
  }
 });
 test('tool_surface_r5_selector_refused',()=>{
  const invalid:[string,Record<string,unknown>][]=[
   ['academy_runtime_inspect',{view:'everything'}],['academy_runtime_inspect',{}],['academy_dailyrecord_get',{view:'full',displayName:'X'}],
   ['academy_dailytest_history',{includeHeaders:'yes',studentName:'X'}],['academy_paper_inspect',{view:'header',studentId:'1'}],
   ['academy_item_lookup',{include:[],labels:['1']}],['academy_item_lookup',{include:['section','section'],labels:['1']}],['academy_item_lookup',{include:['trace'],labels:['1']}],
   ['academy_textbook_catalog_get',{view:'index'}],['academy_prestudy_get',{displayName:'X',view:'reports'}],['academy_cism_get',{scope:'year'}],
   ['academy_prestudy_get',{displayName:'X',includeReports:true}],['academy_prestudy_get',{displayName:'X',reportId:'5002'}],
   ['academy_prestudy_get',{displayName:'X',includeReports:false,reportId:'5002'}],
   // R5 checks the selector arguments of each tool's row only; `scope` of view 'intake'/'status' is the DailyRecord scope object.
   ['academy_dailyrecord_get',{view:'record',scope:{}}],['academy_paper_inspect',{view:'headers',studentId:'1',paperCode:'#23'}],
  ];
  for(const [tool,args] of invalid)expect(()=>planConsolidatedCall(tool,args)).toThrow(ConsolidatedInputInvalid);
  expect(planConsolidatedCall('academy_dailyrecord_get',{view:'status',scope:{studentId:'1'}}).steps[0]!.args).toEqual({scope:{studentId:'1'}});
 });
 test('tool_surface_r6_r7_r14_descriptions',()=>{
  for(const tool of surface.tools){
   expect({name:tool.name,stub:/not available|unavailable stub|same as|returns the same rows/i.test(tool.description)}).toEqual({name:tool.name,stub:false});
   const text=tool.description+' '+(tool.title??'');
   expect({name:tool.name,missing:(hangul[tool.name]??[]).filter(term=>!text.includes(term))}).toEqual({name:tool.name,missing:[]});
  }
 });
 test('tool_surface_r13_no_write_route',()=>{
  for(const tool of consolidatedTools)for(const route of Object.values(tool.routes))expect(route.upstream).not.toMatch(/_(review|save)$/);
  expect(keptTools.map(t=>t.name).filter(n=>/_save$/.test(n))).toEqual(['academy_dailyrecord_save','academy_cism_weekly_save']);
 });
 test('tool_surface_compositions_named_alike',()=>{
  const entries=compositionEntries();
  expect(entries.map(e=>e.tool.name).sort()).toEqual(consolidatedTools.map(t=>t.name).sort());
  for(const e of entries)expect(e.upstreamName).toBe(e.tool.name);
  expect([...compositionUpstreams].sort()).toEqual(consolidatedTools.map(t=>t.name).filter(n=>n!=='academy_prestudy_get').sort());
  const install=readFileSync(resolve(root,'install/registry_surface_install.py'),'utf8');
  const listed=[.../CONSOLIDATED = \{([^}]+)\}/.exec(install)![1]!.matchAll(/'([a-z0-9_]+)'/g)].map(m=>m[1]).sort();
  expect(listed).toEqual(consolidatedTools.map(t=>t.name).sort());
 });
});

describe('the Academy skills and the install doc',()=>{
 test('tool_surface_r10_skill_sync',()=>{
  expect(skillPaths.map(p=>p.split('/').at(-2))).toEqual(['academy-counseling','academy-dailyrecord-authoring','academy-lesson-day','academy-prestudy-review']);
  for(const path of [...skillPaths,resolve(repo,'docs/academy-mcp-install.md')]){
   const unknown=[...readFileSync(path,'utf8').matchAll(toolPattern)].map(m=>m[0]).filter(n=>n!=='academy_operations'&&!names.has(n));
   expect({path:path.slice(repo.length+1),unknown}).toEqual({path:path.slice(repo.length+1),unknown:[]});
  }
 });
 test('tool_surface_r11_skill_selectors',()=>{
  const required=consolidatedTools.filter(t=>['view','scope','include'].includes(t.selectors[0]!)&&!['academy_dailyrecord_get','academy_prestudy_get'].includes(t.name));
  for(const path of skillPaths)for(const line of readFileSync(path,'utf8').split('\n'))for(const tool of required){
   if(!line.includes(tool.name))continue;
   const values=Object.keys(tool.routes).filter(v=>line.includes(`'${v}'`));
   expect({line,named:values.length>0}).toEqual({line,named:true});
  }
 });
});

/** The inputs other than the selector that each old tool took in these fixtures. */
const scope={sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001',lessonDate:'2040-01-16'};
const minimalInputs:Record<string,Record<string,unknown>>={
 academy_capabilities:{capabilityView:'operations',offset:0,limit:5},
 academy_source_read_contracts:{studentId:'77001',kind:'printed_code'},
 academy_paper_read_contracts:{studentId:'77001'},
 academy_dailyrecord_get:{displayName:'Invented Student'},
 academy_dailyrecord_intake:{...scope},
 academy_dailyrecord_execution_status:{scope},
 academy_dailyrecord_reconcile:{context:{...context(),scope}},
 academy_daily_test_inventory:{periods:['11']},
 academy_dailytest_history_with_headers:{periods:['11'],lessonDate:'2040-01-16'},
 academy_paper_headers:{studentId:'77001',paperCodes:['#23']},
 academy_paper_definition_lookup:{studentId:'77001',paperCode:'#23',definitionView:'header'},
 academy_printed_paper_lookup:{studentId:'77001',paperCode:'#23',period:{from:'2040-01-01',through:'2040-01-16'}},
 academy_item_section_trace:{displayName:'Invented Student',labels:['101'],bookLabel:'가우스 5-1-2'},
 academy_item_answer_lookup:{labels:['101'],view:'compact'},
 academy_textbook_catalog:{query:'가우스',limit:5},
 academy_textbook_toc_get:{book:'가우스 5-1-2',unit:'6'},
 academy_textbook_pdf:{documentRef:'a'.repeat(64)},
 academy_prestudy_get:{displayName:'Invented Student'},
 academy_prestudy_report_get:{displayName:'Invented Student',sectionTitles:['Invented section']},
 academy_cism_weekly_get:{date:'2040-01-16'},
 academy_cism_month:{month:'2040-01'},
};
/** The old tool's own arguments: the new arguments with the renamed members given back their old names. */
const oldArgs=(old:string,input:Record<string,unknown>)=>{
 const renamed:Record<string,Record<string,string>>={academy_capabilities:{capabilityView:'view'},academy_paper_definition_lookup:{definitionView:'view'},
  academy_prestudy_report_get:{reportId:'prestudyKey'}};
 const extra:Record<string,Record<string,unknown>>={academy_prestudy_report_get:{prestudyKey:'5002'},academy_daily_test_inventory:{studentId:'77001'}};
 return {...Object.fromEntries(Object.entries(input).map(([k,v])=>[renamed[old]?.[k]??k,v])),...extra[old]};
};

describe('the 15 tools through the bridge over recorded synthetic endpoints',()=>{
 let base:string,events:string,client:Client,transport:StdioClientTransport;
 const env=()=>({PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base});
 beforeAll(async()=>{
  base=await mkdtemp(resolve(tmpdir(),'academy-tool-surface-'));events=resolve(base,'events.txt');
  await writeFile(events,'');
  const endpoint=(name:string)=>({command:process.execPath,args:['--preload',deny,fixture,name,events]});
  await writeFile(resolve(base,'endpoints.json'),JSON.stringify({dailyrecord:endpoint('dailyrecord'),paper1:endpoint('paper1'),saved:endpoint('saved')}),{mode:0o600});
  await writeFile(resolve(base,'routing.json'),JSON.stringify({}),{mode:0o600});
  const discovery=Bun.spawn([process.execPath,'--preload',deny,entry,'--discover'],{env:env(),stdout:'pipe',stderr:'pipe'});
  expect(await discovery.exited).toBe(0);
  client=new Client({name:'fictional-tool-surface-client',version:'1.0.0'});
  transport=new StdioClientTransport({command:process.execPath,args:['--preload',deny,entry],env:env(),stderr:'pipe'});
  await client.connect(transport);
 },30000);
 afterAll(async()=>{await client?.close();await transport?.close();await rm(base,{recursive:true,force:true});});

 /** One call: its structured answer and the endpoint requests it recorded. */
 const run=async(name:string,args:Record<string,unknown>)=>{
  const before=(await readFile(events,'utf8')).length;
  const result:any=await client.callTool({name,arguments:args});
  return {data:result.structuredContent,isError:result.isError===true,requests:(await readFile(events,'utf8')).slice(before).split('\n').filter(Boolean)};
 };

 test('tool_surface_discovery',async()=>{
  const registry=JSON.parse(await readFile(resolve(base,'registry.json'),'utf8')) as {endpoint:string;upstreamName:string;tool:{name:string}}[];
  const registered=new Set(registry.map(r=>r.tool.name));
  for(const name of surfaceOrder)expect({name,registered:registered.has(name)}).toEqual({name,registered:true});
  for(const hidden of ['academy_dailyrecord_intake','academy_paper_headers','academy_item_section_trace','academy_cism_month','academy_capabilities'])expect(registered.has(hidden)).toBe(true);
  expect(registered.has('academy_dailytest_history_with_headers')).toBe(false);
  expect(registry.find(r=>r.tool.name==='academy_prestudy_get')!.upstreamName).toBe('academy_prestudy_get');
 });

 test('tool_surface_r4_parity',async()=>{
  const direct=Object.entries(oldToNew).filter(([old,row])=>Array.isArray(row)&&old!==row[0]&&!['academy_runtime_identity','academy_dailytest_history_with_headers'].includes(old));
  expect(direct).toHaveLength(18);
  for(const [old,row] of direct){
   const [tool,selector]=row as [string,Record<string,unknown>];
   const input=minimalInputs[old]??{};
   const before=await run(old,oldArgs(old,input)),after=await run(tool,{...selector,...input});
   expect({old,isError:before.isError,requests:before.requests.length}).toEqual({old,isError:false,requests:1});
   expect({old,data:after.data,requests:after.requests}).toEqual({old,data:before.data,requests:before.requests});
  }
 },120000);

 test('tool_surface_r4_bridge_rows',async()=>{
  const identity=await run('academy_runtime_inspect',{view:'identity'}),legacy=await run('academy_runtime_identity',{});
  expect(identity).toEqual(legacy);expect(identity.requests).toEqual([]);
  const videos=await run('academy_prestudy_get',{view:'videos',displayName:'Invented Student'}),plain=await run('academy_prestudy_get',{displayName:'Invented Student'});
  expect(videos).toEqual(plain);
  expect(videos.requests).toEqual(['dailyrecord academy_prestudy_get {"displayName":"Invented Student"}']);
  expect(videos.data).toMatchObject({tool:'academy_prestudy_get',waiting:[{prestudyKey:'5001'}],completed:[{prestudyKey:'5002'}],summary:[{aiScoreLiteral:'80'}]});
  // The DT history with headers reads the dailyrecord metadata first, as academy_dailytest_history_with_headers did.
  const headers=await run('academy_dailytest_history',{studentName:'Invented Student',lessonDate:'2040-01-16',periods:['11']});
  expect(headers.requests[0]).toStartWith('dailyrecord academy_student_metadata_get {"displayName":"Invented Student","lessonDate":"2040-01-16"');
 },30000);

 test('tool_surface_one_call',async()=>{
  const inventory=await run('academy_dailytest_history',{includeHeaders:false,studentName:'Invented Student',periods:['11']});
  expect(inventory.requests).toEqual(['dailyrecord academy_student_resolve {"displayName":"Invented Student"}','paper1 academy_daily_test_inventory {"studentId":"77001","periods":["11"]}']);
  expect(inventory.data).toMatchObject({tool:'academy_daily_test_inventory',studentResolution:{studentId:'77001',displayName:'Invented Student'}});
  const unknown=await run('academy_dailytest_history',{includeHeaders:false,studentName:'Unknown Student'});
  expect(unknown.data).toMatchObject({status:'unavailable',reasonCode:'student_name_not_found'});expect(unknown.requests).toHaveLength(1);
  const both=await run('academy_item_lookup',{include:['section','answer'],displayName:'Invented Student',labels:['101'],view:'compact'});
  expect(both.requests).toEqual(['dailyrecord academy_item_section_trace {"displayName":"Invented Student","labels":["101"]}','dailyrecord academy_item_answer_lookup {"labels":["101"],"view":"compact"}']);
  expect(both.data).toMatchObject({include:['section','answer'],section:{tool:'academy_item_section_trace'},answer:{tool:'academy_item_answer_lookup'}});
  const incomplete=await run('academy_prestudy_get',{view:'incomplete',displayName:'Invented Student'});
  expect(incomplete.data).toMatchObject({view:'incomplete',waiting:[{prestudyKey:'5001'}]});
  expect(incomplete.data.completed).toBeUndefined();expect(incomplete.data.summary).toBeUndefined();
  const summary=await run('academy_prestudy_get',{view:'teacher_summary',displayName:'Invented Student',includeReports:true,reportId:'5002'});
  expect(summary.requests.map(r=>r.split(' ')[1])).toEqual(['academy_prestudy_get','academy_prestudy_report_get']);
  expect(summary.data).toMatchObject({view:'teacher_summary',summary:[{aiScoreLiteral:'80'}],report:{tool:'academy_prestudy_report_get',received:{prestudyKey:'5002'}}});
  expect(summary.data.waiting).toBeUndefined();
 },30000);

 test('tool_surface_r5_bridge_refuses',async()=>{
  for(const [name,args] of [['academy_paper_inspect',{view:'everything',studentId:'77001'}],['academy_cism_get',{scope:'week',month:'2040-01'}],
   ['academy_prestudy_get',{displayName:'Invented Student',reportId:'5002'}],['academy_textbook_catalog_get',{view:'pdf',documentRef:'not-a-digest'}],
   ['academy_dailytest_history',{includeHeaders:false,studentName:'Invented Student',periods:['99']}],['academy_item_lookup',{include:['answer'],labels:['101'],bookLabel:'x'}]] as const){
   const result=await run(name,args as Record<string,unknown>);
   expect({name,code:result.data.code,requests:result.requests}).toEqual({name,code:'tool_input_invalid',requests:[]});
  }
  const renamed=await run('academy_runtime_inspect',{view:'capabilities',capabilityView:'everything'});
  expect(renamed.data).toMatchObject({code:'tool_input_invalid'});expect(renamed.requests).toEqual([]);
 },30000);

 test('tool_surface_r9_pinned_list',async()=>{
  const pin=async(name:string,bytes:string|Buffer)=>{const path=resolve(base,name);await writeFile(path,bytes,{mode:0o600});return {path,sha256:hash(bytes)};};
  const existing=async(name:string)=>{const path=resolve(base,name);return {path,sha256:hash(await readFile(path))};};
  const release={schemaVersion:'academy.mcp-release/1',generation:'tool-surface-1',routing:await existing('routing.json'),endpoints:await existing('endpoints.json'),
   registry:await existing('registry.json'),surface:await pin('surface.json',readFileSync(resolve(root,'release/surface.json')))};
  await writeFile(resolve(base,'release.tmp'),JSON.stringify(release),{mode:0o600});await rename(resolve(base,'release.tmp'),resolve(base,'release.json'));
  const pinned=new Client({name:'fictional-tool-surface-client',version:'1.0.0'});
  const pinnedTransport=new StdioClientTransport({command:process.execPath,args:['--preload',deny,entry],env:env(),stderr:'pipe'});
  await pinned.connect(pinnedTransport);
  try{
   const listed=(await pinned.listTools()).tools;
   expect(listed.map(t=>t.name)).toEqual([...surfaceOrder]);
   expect(listed.map(t=>t.description)).toEqual(surface.tools.map(t=>t.description));
   const old:any=await pinned.callTool({name:'academy_paper_headers',arguments:{studentId:'77001',paperCodes:['#23']}});
   expect(old.structuredContent).toMatchObject({status:'blocked',code:'unknown_tool'});
   const headers:any=await pinned.callTool({name:'academy_paper_inspect',arguments:{view:'headers',studentId:'77001',paperCodes:['#23']}});
   expect(headers.structuredContent).toMatchObject({tool:'academy_paper_headers',received:{studentId:'77001',paperCodes:['#23']}});
   const identity:any=await pinned.callTool({name:'academy_runtime_inspect',arguments:{view:'identity'}});
   expect(identity.structuredContent).toMatchObject({generation:'tool-surface-1',surfaceSha256:release.surface.sha256});
  }finally{await pinned.close();await pinnedTransport.close();}
 },30000);
});

describe('I1 write_forbidden: request classes by named route and operation',()=>{
 test('tool_surface_i1_request_classes',()=>{
  const day='https://lms.invalid/servlet/controller.cct.tutor.DayRecordServlet';
  expect(lmsRequestRole(day+'?p_process=udtPrg&prg_txt=x')).toBe('write');
  expect(lmsRequestRole(day,'p_process=Main&std_ymd=20400116')).toBe('read');
  expect(lmsRequestRole('/servlet/controller.dailyzerotest.DailyZeroTestServlet?reqCmd=DtResultSearch','sort_date=1&stu_name=x')).toBe('read');
  expect(lmsRequestRole('/servlet/controller.cct.common.CourseCommonServlet','reqCmd=SetCismContentData&field_name=x')).toBe('write');
  // Operations that neither lmsRouteRegistry.ts nor endpointCandidates.ts classes read fail closed.
  for(const unclassed of ['/servlet/controller.coursemanage.CourseManageServlet?reqCmd=CourseStudyManager',
   '/servlet/controller.coursemanage.CourseManageServlet?reqCmd=WebPreStudy&prestudy_key=1','/app-ui/solution_common/prestudy/prestudy_ai_report.jsp?analysis_key=1',
   '/servlet/controller.cct.tutor.DayRecordServlet?p_process=DaysMain','/servlet/controller.cct.common.CourseCommonServlet?reqCmd=CismChartMain',
   '/servlet/controller.tutor.base.TestPageListServlet?p_process=getWrongAnswerNoteResult','/servlet/controller.common.TestPageShowExServlet?p_process=GetTypeTotal',
   '/servlet/controller.common.TestPageShowExServlet?p_process=GetExamList','/servlet/controller.tutor.etest.TestPoolServlet?reqCmd=LogViewList'])
   expect({unclassed,role:lmsRequestRole(unclassed)}).toEqual({unclassed,role:'write'});
  expect(lmsRequestRole('/servlet/controller.login.LoginServlet','p_process=c_login')).toBe('login');
  expect(lmsRequestRole('/servlet/controller.coursemanage.CourseManageServlet?reqCmd=Invented')).toBe('write');
  expect(lmsRequestRole('/servlet/controller.invented.Servlet?p_process=Main')).toBe('write');
 });

 test('tool_surface_i1_resolution_write_forbidden',async()=>{
  const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
  const principal='<a href="/logout">Invented Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
  const selectors=['1','11','12','13','14','15','16'];
  const page=(rows:unknown[])=>'<form name="frm"><select name="sort_date">'+selectors.map(v=>'<option value="'+v+'">Invented period '+v+'</option>').join('')
   +"</select></form><script>const x={testResultList:JSON.parse('"+JSON.stringify(rows)+"')};</script>";
  let calls=0;
  const guard=writeForbiddenTransport(url=>{calls++;
   if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
   if(!url.includes('DailyZeroTestServlet?reqCmd=DtResultSearch'))throw Error('unexpected_fixture_request');
   return new Response(page([{priNo:9101,userName:'Invented Student'}]));});
  const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'9101',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
  const client=new AcademyHttpClient({teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001'},guard.transport as never);
  const service=new DailyRecordService(new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'})),safeguard,()=>clock,learningPolicy);
  expect(await service.resolveStudent({displayName:'Invented Student'})).toMatchObject({status:'resolved',studentKey:'9101'});
  expect(guard.forbidden()).toEqual([]);
  expect(guard.requests.map(r=>r.role)).toEqual(['login','login','read','read']);
  expect(guard.requests.at(-1)!.route).toBe('/servlet/controller.dailyzerotest.DailyZeroTestServlet DtResultSearch');
 });
});
