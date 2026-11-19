import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {existsSync,mkdtempSync,readdirSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {z} from 'zod';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {dailyRecordGetSchema} from '../src/dailyrecord-read.ts';
import {dailyTestHeaderRouteSchema} from '../src/dailytest-header-route.ts';

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
/** Every input-schema difference of v6 from v5, by JSON pointer; each is explained in docs/release-v6-differences.md. */
const knownDifferences:Record<string,string[]>={
 academy_dailyrecord_get:['/properties/lessonDate/description','/properties/view'],
};
/** Registered by a program and deliberately absent from the surface. */
const hiddenTools:Record<Program,string[]>={
 saved:['academy_endpoint_identity'],
 dailyrecord:['academy_endpoint_identity','academy_textbook_units_get'],
};
/** The paper endpoints' tools are not compared here: the change that adds the paper server compares them. */
const comparedWithPaperServer=fixture.tools.filter(t=>t.endpoint==='paper1'||t.endpoint==='paper2').map(t=>t.name);

/** JSON pointers at which two schemas differ, ignoring the top-level `$schema`. */
function differences(a:unknown,b:unknown,path=''):string[]{
 if(a!==null&&b!==null&&typeof a==='object'&&typeof b==='object'&&Array.isArray(a)===Array.isArray(b)){
  const ao=a as Record<string,unknown>,bo=b as Record<string,unknown>;
  const keys=[...new Set([...Object.keys(ao),...Object.keys(bo)])].filter(k=>!(path===''&&k==='$schema'));
  return keys.flatMap(k=>!(k in ao)||!(k in bo)?[path+'/'+k]:differences(ao[k],bo[k],path+'/'+k));
 }
 return Object.is(a,b)?[]:[path];
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
 test('the fixture holds the 38 routed tools of v5 and the bridge-served runtime identity',()=>{
  expect(fixture.tools.map(t=>t.name)).toEqual(surface.tools.map(t=>t.name));
  expect(fixture.tools.filter(t=>t.servedBy!=='bridge')).toHaveLength(38);
  expect(fixture.tools.filter(t=>t.servedBy==='bridge')).toEqual([{name:'academy_runtime_identity',servedBy:'bridge',compared:false}]);
  expect(comparedWithPaperServer.sort()).toEqual(['academy_daily_test_inventory','academy_paper_definition_lookup','academy_paper_headers',
   'academy_paper_read_contracts','academy_printed_paper_lookup','academy_source_read_contracts','class2_academy_paper_headers']);
 });

 test('saved and dailyrecord programs list the measured tool counts',()=>{
  expect(registered('saved').size).toBe(24);
  expect(registered('dailyrecord').size).toBe(26);
 });

 for(const tool of fixture.tools.filter(t=>t.servedBy!=='bridge'&&isProgram(t.endpoint))){
  test(`${tool.name} keeps its v5 input schema (${tool.servedBy==='bridge_composition'?'bridge composition':tool.endpoint})`,()=>{
   // A bridge composition is compared with the bridge's schema; an endpoint tool with what its program registers.
   const current=tool.servedBy==='bridge_composition'?bridgeSchemas[tool.bridgeSchema!]:registered(tool.endpoint as Program).get(tool.upstreamName!);
   expect(current).toBeDefined();
   expect(differences(tool.inputSchema,current).sort()).toEqual(knownDifferences[tool.name]??[]);
  });
 }

 test('every known difference is listed in docs/release-v6-differences.md',()=>{
  for(const [name,paths] of Object.entries(knownDifferences)){
   for(const path of paths)expect(differencesPage).toContain(`\`${name}\` \`${path}\``);
  }
 });

 test('every registered tool is on the surface, except the named hidden tools',()=>{
  const onSurface=new Set(surface.tools.map(t=>t.name));
  for(const program of Object.keys(programs) as Program[]){
   const missing=[...registered(program).keys()].filter(name=>!onSurface.has(name)).sort();
   expect(missing).toEqual([...hiddenTools[program]].sort());
  }
 });

 test("each displayed schema's required equals the registered required restricted to its displayed members",()=>{
  const route=new Map(fixture.tools.map(t=>[t.name,t]));
  const withoutMembers:string[]=[];
  for(const shown of surface.tools){
   const tool=route.get(shown.name);
   if(!shown.inputSchema||!tool||!isProgram(tool.endpoint))continue;
   const current=tool.servedBy==='bridge_composition'?bridgeSchemas[tool.bridgeSchema!]!:registered(tool.endpoint).get(tool.upstreamName!)!;
   const members=Object.keys((shown.inputSchema.properties??{}) as object);
   if(!members.length)withoutMembers.push(shown.name);
   const restricted=((current.required??[]) as string[]).filter(name=>members.includes(name));
   expect({name:shown.name,required:[...(shown.inputSchema.required??[]) as string[]].sort()}).toEqual({name:shown.name,required:restricted.sort()});
  }
  // Displayed without members, so without required: four compacted contracts and the
  // status tool, which has no members.
  expect(withoutMembers.sort()).toEqual(['academy_cism_section','academy_find_item_labels','academy_item_label_scope',
   'academy_review_type_questions','academy_source_status']);
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
