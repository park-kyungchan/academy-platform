import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {existsSync,mkdtempSync,readdirSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {z} from 'zod';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {dailyRecordGetSchema} from '../src/dailyrecord-read.ts';
import {dailyTestHeaderRouteSchema} from '../src/dailytest-header-route.ts';
import {academyDateInputSchema} from '../src/read-input.ts';
import {poolQuestionContentInputSchema} from '../src/pool-question-content.ts';
import {consolidatedTools,consolidatedNames} from '../src/consolidated-tools.ts';

// Compares the tools of release v5 (tests/fixtures/release-v5-tools.json, taken from its
// surface.json and registry.json) with what the saved and dailyrecord endpoint programs
// register today. The programs start over stdio with no owner arguments and outbound
// network denied, so no owner check, LMS or host path is touched.
type JsonSchema={[key:string]:unknown};
type FixtureTool={name:string;servedBy:'endpoint'|'bridge_composition'|'bridge';endpoint?:string;upstreamName?:string;
 bridgeSchema?:string;compared?:false;inputSchema?:JsonSchema};
type SurfaceTool={name:string;inputSchema?:JsonSchema};

const root=resolve(import.meta.dir,'..');
const fixture=JSON.parse(readFileSync(resolve(import.meta.dir,'fixtures/release-v5-tools.json'),'utf8')) as {tools:FixtureTool[]};
const surface=JSON.parse(readFileSync(resolve(root,'release/surface.json'),'utf8')) as {schemaVersion:string;tools:SurfaceTool[]};
const differencesPage=readFileSync(resolve(root,'docs/release-v6-differences.md'),'utf8');

const programs={saved:'server.ts',dailyrecord:'dailyrecord-server.ts'} as const;
type Program=keyof typeof programs;
const isProgram=(endpoint:string|undefined):endpoint is Program=>endpoint==='saved'||endpoint==='dailyrecord';

/** The bridge's own schemas for the tools it composes (src/neutral-server.ts). */
const bridgeSchemas:Record<string,JsonSchema>={
 dailyRecordGetSchema:z.toJSONSchema(dailyRecordGetSchema,{io:'input'}) as JsonSchema,
 dailyTestHeaderRouteSchema:z.toJSONSchema(dailyTestHeaderRouteSchema,{io:'input'}) as JsonSchema,
};
/** Every input-schema difference of v6 from v5, by the JSON pointer of the member that
 * differs: each difference lies at or under a listed pointer, and each listed pointer has
 * one. `required` is compared as a set, so a `required` list has no index pointer; an `oneOf` branch of a member is named by its index. Each is explained in docs/release-v6-differences.md. */
const identity=['/properties/currentRequestRef','/properties/purpose'];
const period=(kind:number)=>`/properties/queries/items/oneOf/${kind}/properties/period`;
const memoAnswerNoAddition='/properties/context/properties/memoAnswer/oneOf/1/required';
/** The memo bound stated in the write tools' input: the content text of `memoAnswer` and the proposed value. */
const memoAnswerContentBound='/properties/context/properties/memoAnswer/oneOf/2/properties/text/description';
const proposalValueBound=(member:string)=>`/properties/${member}/items/properties/value/description`;
/** Intake's flat form: the context members, and the scope written out, at the top level. */
const intakeFlat=['/properties/schemaVersion','/properties/purpose','/properties/currentRequestRef','/properties/scope','/properties/selectedFields',
 '/properties/memoAnswer','/properties/facts','/properties/sourceNamespace','/properties/teacherId','/properties/studentId','/properties/lessonDate'];
const knownDifferences:Record<string,string[]>={
 academy_dailyrecord_get:[...identity,'/properties/lessonDate','/properties/view','/properties/fields','/required'],
 academy_dailyrecord_intake:[...intakeFlat,'/properties/fields','/properties/context/required',memoAnswerNoAddition,memoAnswerContentBound,proposalValueBound('suppliedValues'),'/required'],
 academy_dailyrecord_review:[memoAnswerNoAddition,memoAnswerContentBound,proposalValueBound('proposals')],
 academy_dailyrecord_save:[memoAnswerNoAddition,memoAnswerContentBound,proposalValueBound('proposals')],
 academy_dailytest_history_with_headers:[...identity,'/properties/lessonDate','/required'],
 academy_student_resolve:['/properties/lessonDate','/required'],
 academy_student_metadata_get:[...identity,'/properties/lessonDate',period(0),period(4),'/required'],
 academy_textbook_section_results_get:[...identity,'/required','/properties/courseKey','/properties/bookNo','/properties/studentKey'],
 academy_clinic_paper_status_get:[...identity,'/required'],
 academy_item_section_trace:[...identity,'/required'],
 academy_item_answer_lookup:['/required'],
 academy_prestudy_get:[...identity,'/properties/view','/required'],
 academy_prestudy_report_get:[...identity,'/required'],
 academy_cism_weekly_get:['/properties/date'],
};
/** Input schemas changed for v6 of tools that neither v5 nor these two endpoints route
 * (the pool server's): no v5 record to compare with, so each states its v6 form. */
const changedOutsideTheEndpoints:Record<string,{pointers:string[];schema:JsonSchema;check:(schema:JsonSchema)=>unknown}>={
 academy_pool_question_content:{pointers:['/properties/purpose','/required'],schema:z.toJSONSchema(poolQuestionContentInputSchema,{io:'input'}) as JsonSchema,
  check:schema=>({purposeDefault:at(schema,'/properties/purpose/default'),purposeRequired:((schema.required??[]) as string[]).includes('purpose')})},
};
/** Registered by a program, absent from the surface and routed to by no consolidated tool. */
const hiddenTools:Record<Program,string[]>={
 saved:['academy_endpoint_identity'],
 dailyrecord:['academy_endpoint_identity','academy_textbook_units_get'],
};
/** The endpoint tools that the consolidated tools of the surface route to (src/consolidated-tools.ts). */
const routeUpstreams=new Set(consolidatedTools.flatMap(t=>Object.values(t.routes).map(r=>r.upstream)));
/** v5 tools that are neither on the surface nor a route: the retired bridge composition
 * `academy_dailytest_history_with_headers` (now `academy_dailytest_history`) and paper2's alias
 * `class2_academy_paper_headers`, which `selectPaperRoute` reaches from `academy_paper_headers`. */
const hiddenRouteTargets=['academy_dailytest_history_with_headers','class2_academy_paper_headers'];
/** The paper endpoints' tools are not compared here: the change that adds the paper server compares them. */
const comparedWithPaperServer=fixture.tools.filter(t=>t.endpoint==='paper1'||t.endpoint==='paper2').map(t=>t.name);

/** JSON pointers at which two schemas differ, ignoring the top-level `$schema`; a
 * `required` list is compared as a set, so its order and indices do not count. */
function differences(a:unknown,b:unknown,path=''):string[]{
 if(path.endsWith('/required')&&Array.isArray(a)&&Array.isArray(b)){
  const set=(list:unknown[])=>JSON.stringify([...new Set(list.map(String))].sort());
  return set(a)===set(b)?[]:[path];
 }
 if(a!==null&&b!==null&&typeof a==='object'&&typeof b==='object'&&Array.isArray(a)===Array.isArray(b)){
  const ao=a as Record<string,unknown>,bo=b as Record<string,unknown>;
  const keys=[...new Set([...Object.keys(ao),...Object.keys(bo)])].filter(k=>!(path===''&&k==='$schema'));
  return keys.flatMap(k=>!(k in ao)||!(k in bo)?[path+'/'+k]:differences(ao[k],bo[k],path+'/'+k));
 }
 return Object.is(a,b)?[]:[path];
}

const under=(path:string,member:string)=>path===member||path.startsWith(member+'/');

/** The date members of a registered schema: string members described by the date
 * input schema of src/read-input.ts, by JSON pointer. */
const dateDescription=academyDateInputSchema.description!;
function dateMembers(schema:unknown,path=''):string[]{
 if(!schema||typeof schema!=='object'||Array.isArray(schema))return [];
 const s=schema as JsonSchema;
 if(typeof s.description==='string'&&s.description.includes(dateDescription))return [path];
 const nested=(key:string)=>Object.entries((s[key]??{}) as Record<string,unknown>).flatMap(([k,v])=>dateMembers(v,`${path}/${key}/${k}`));
 return [...nested('properties'),...(s.items?dateMembers(s.items,path+'/items'):[]),...nested('anyOf'),...nested('oneOf')];
}
const at=(schema:unknown,pointer:string)=>pointer.split('/').slice(1).reduce<unknown>((node,key)=>
 node&&typeof node==='object'?(node as Record<string,unknown>)[key]:undefined,schema);
/** Whether a displayed or registered string schema lets a value through: a `format` or a
 * `pattern` that the value fails rules it out, as does a `const` or `enum` without it. */
function acceptsString(schema:unknown,value:string):boolean{
 const s=(schema??{}) as JsonSchema;
 if(Array.isArray(s.anyOf))return s.anyOf.some(branch=>acceptsString(branch,value));
 if(Array.isArray(s.oneOf))return s.oneOf.filter(branch=>acceptsString(branch,value)).length===1;
 if(s.type!==undefined&&![s.type].flat().includes('string'))return false;
 if('const' in s)return s.const===value;
 if(Array.isArray(s.enum))return s.enum.includes(value);
 if(s.format!==undefined)return false;
 if(typeof s.pattern==='string'&&!new RegExp(s.pattern,'u').test(value))return false;
 if(typeof s.minLength==='number'&&value.length<s.minLength)return false;
 if(typeof s.maxLength==='number'&&value.length>s.maxLength)return false;
 return true;
}

const listed:Partial<Record<Program,Map<string,JsonSchema>>>={};
const open:{client:Client;transport:StdioClientTransport}[]=[];
beforeAll(async()=>{
 for(const program of Object.keys(programs) as Program[]){
  const client=new Client({name:'synthetic-release-tools-verifier',version:'0.1.0'});
  const transport=new StdioClientTransport({command:process.execPath,
   args:['--preload',resolve(import.meta.dir,'network-deny.ts'),resolve(root,'src',programs[program])],
   env:{PATH:process.env.PATH??'/usr/bin:/bin'},stderr:'pipe'});
  await client.connect(transport);open.push({client,transport});
  listed[program]=new Map((await client.listTools()).tools.map(t=>[t.name,t.inputSchema as JsonSchema]));
 }
},30000);
afterAll(async()=>{for(const {client,transport} of open){await client.close();await transport.close();}});
const registered=(program:Program)=>listed[program]!;

describe('release v5 tools against the endpoint programs',()=>{
 test('each of the 30 routed v5 tools that remain is a surface tool or a route of a consolidated surface tool',()=>{
  const onSurface=new Set(surface.tools.map(t=>t.name));
  expect(fixture.tools.map(t=>t.name).filter(name=>!hiddenRouteTargets.includes(name)&&!onSurface.has(name)&&!routeUpstreams.has(name))).toEqual([]);
  expect(surface.tools).toHaveLength(18);
  expect(fixture.tools.map(t=>t.name)).toEqual(expect.arrayContaining(hiddenRouteTargets));
  expect(fixture.tools.filter(t=>t.servedBy!=='bridge')).toHaveLength(30);
  expect(fixture.tools.filter(t=>t.servedBy==='bridge')).toEqual([{name:'academy_runtime_identity',servedBy:'bridge',compared:false}]);
  expect(comparedWithPaperServer.sort()).toEqual(['academy_daily_test_inventory','academy_paper_definition_lookup','academy_paper_headers',
   'academy_paper_read_contracts','academy_printed_paper_lookup','academy_source_read_contracts','class2_academy_paper_headers']);
 });

 test('saved and dailyrecord programs list the measured tool counts',()=>{
  expect(registered('saved').size).toBe(16);
  expect(registered('dailyrecord').size).toBe(26);
 });

 for(const tool of fixture.tools.filter(t=>t.servedBy!=='bridge'&&isProgram(t.endpoint))){
  test(`${tool.name} keeps its v5 input schema but for the listed differences (${tool.servedBy==='bridge_composition'?'bridge composition':tool.endpoint})`,()=>{
   // A bridge composition is compared with the bridge's schema; an endpoint tool with what its program registers.
   const current=tool.servedBy==='bridge_composition'?bridgeSchemas[tool.bridgeSchema!]:registered(tool.endpoint as Program).get(tool.upstreamName!);
   expect(current).toBeDefined();
   const found=differences(tool.inputSchema,current).sort(),known=knownDifferences[tool.name]??[];
   expect({unlisted:found.filter(path=>!known.some(member=>under(path,member))),unused:known.filter(member=>!found.some(path=>under(path,member)))})
    .toEqual({unlisted:[],unused:[]});
  });
 }

 test('every known difference is listed in docs/release-v6-differences.md, one row per tool',()=>{
  const rows=new Map(differencesPage.split('\n').flatMap(line=>{const m=/^\| `(academy_\w+)` \|/.exec(line);return m?[[m[1]!,line] as const]:[];}));
  expect([...rows.keys()].sort()).toEqual([...Object.keys(knownDifferences),...Object.keys(changedOutsideTheEndpoints)].sort());
  const outside=Object.entries(changedOutsideTheEndpoints).map(([name,{pointers}])=>[name,pointers] as const);
  for(const [name,paths] of [...Object.entries(knownDifferences),...outside]){
   for(const path of paths)expect({name,row:rows.get(name)?.includes(`\`${path}\``)}).toEqual({name,row:true});
  }
 });

 test('academy_pool_question_content, outside the two endpoints, takes purpose by default',()=>{
  const {schema,check}=changedOutsideTheEndpoints.academy_pool_question_content!;
  expect(check(schema)).toEqual({purposeDefault:'teacher_requested_review',purposeRequired:false});
 });

 test('every registered tool is on the surface or a route of a consolidated surface tool, except the named hidden tools',()=>{
  const onSurface=new Set(surface.tools.map(t=>t.name));
  for(const program of Object.keys(programs) as Program[]){
   const missing=[...registered(program).keys()].filter(name=>!onSurface.has(name)&&!routeUpstreams.has(name)).sort();
   expect(missing).toEqual([...hiddenTools[program]].sort());
  }
 });

 test("each displayed schema's required equals the registered required restricted to its displayed members",()=>{
  const route=new Map(fixture.tools.map(t=>[t.name,t]));
  const withoutMembers:string[]=[];
  for(const shown of surface.tools){
   const tool=route.get(shown.name);
   // A consolidated tool is validated by its routes' schemas (tests/tool-surface.test.ts).
   if(!shown.inputSchema||!tool||!isProgram(tool.endpoint)||consolidatedNames.has(shown.name))continue;
   const current=tool.servedBy==='bridge_composition'?bridgeSchemas[tool.bridgeSchema!]!:registered(tool.endpoint).get(tool.upstreamName!)!;
   const members=Object.keys((shown.inputSchema.properties??{}) as object);
   if(!members.length)withoutMembers.push(shown.name);
   const restricted=((current.required??[]) as string[]).filter(name=>members.includes(name));
   expect({name:shown.name,required:[...(shown.inputSchema.required??[]) as string[]].sort()}).toEqual({name:shown.name,required:restricted.sort()});
  }
  // Every displayed tool of the two programs has members: the compacted contracts and the
  // status tool of earlier surfaces are retired.
  expect(withoutMembers.sort()).toEqual([]);
 });

 test('each displayed date member lets 어제 and 11월 16일 through, as its registered schema does',()=>{
  const route=new Map(fixture.tools.map(t=>[t.name,t]));
  const displayed=new Set<string>();
  /** The registered schemas behind one surface tool: its own, or each of its routes' (with the route's renamed members). */
  const behind=(name:string):{schema:JsonSchema;rename:Record<string,string>}[]=>{
   const consolidated=consolidatedTools.find(t=>t.name===name);
   if(!consolidated){
    const tool=route.get(name);
    if(!tool||!isProgram(tool.endpoint))return [];
    return [{schema:tool.servedBy==='bridge_composition'?bridgeSchemas[tool.bridgeSchema!]!:registered(tool.endpoint).get(tool.upstreamName!)!,rename:{}}];
   }
   return Object.values(consolidated.routes).flatMap(r=>{
    const schema=r.bridge==='dailyrecord_record'?bridgeSchemas.dailyRecordGetSchema:r.bridge==='dailytest_headers'?bridgeSchemas.dailyTestHeaderRouteSchema
     :(['dailyrecord','saved'] as const).map(p=>registered(p).get(r.upstream)).find(Boolean);
    return schema?[{schema,rename:Object.fromEntries(Object.entries(r.rename??{}).map(([k,v])=>[v,k]))}]:[];
   });
  };
  for(const shown of surface.tools){
   if(!shown.inputSchema)continue;
   for(const {schema:current,rename} of behind(shown.name))for(const pointer of dateMembers(current)){
    for(const value of ['어제','11월 16일']){
     expect({name:shown.name,pointer,value,registered:acceptsString(at(current,pointer),value)}).toEqual({name:shown.name,pointer,value,registered:true});
    }
    const member=/^\/properties\/([^/]+)$/.exec(pointer)?.[1];
    const shownPointer=member!==undefined?'/properties/'+(rename[member]??member):pointer;
    const node=at(shown.inputSchema,shownPointer);
    if(node===undefined)continue;
    displayed.add(`${shown.name} ${shownPointer}`);
    for(const value of ['어제','11월 16일']){
     expect({name:shown.name,pointer,value,displayed:acceptsString(node,value)}).toEqual({name:shown.name,pointer,value,displayed:true});
    }
   }
  }
  // The date members the surface displays; a date member inside a displayed
  // `queries: {type: 'array'}` is not displayed.
  expect([...displayed].sort()).toEqual(['academy_cism_get /properties/date','academy_dailyrecord_get /properties/lessonDate',
   'academy_dailytest_history /properties/lessonDate','academy_student_resolve /properties/lessonDate']);
 });

 test('build:endpoints refuses an out directory other than modules/academy-workbench/.local/<name>/src',()=>{
  const scratch=mkdtempSync(resolve(tmpdir(),'release-tools-'));
  try{
   for(const out of [resolve(scratch,'src'),resolve(root,'../../.local/release-tools-refusal/bundle'),resolve(root,'../../.local/src')]){
    const run=Bun.spawnSync([process.execPath,resolve(root,'scripts/build-endpoints.ts'),out],{cwd:root,stdout:'pipe',stderr:'pipe'});
    expect(run.exitCode).toBe(2);
    expect(run.stderr.toString()).toContain('build_endpoints_out_dir_refused');
    expect(existsSync(out)).toBe(false);
   }
   expect(readdirSync(scratch)).toEqual([]);
  }finally{rmSync(scratch,{recursive:true,force:true});}
 });
});
