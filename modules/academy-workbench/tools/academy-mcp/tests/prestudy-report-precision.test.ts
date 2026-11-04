import {expect,test} from 'bun:test';
import {DailyRecordService,prestudyReportGetSchema,type DailyRecordReadBackend,type PrestudyReportObservation} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';
import {dailyRecordMemory} from './dailyrecord-memory.ts';

const input={displayName:'Invented Student',prestudyKey:'12345',purpose:'Review invented report',currentRequestRef:'synthetic-report-precision'};
const observation=():PrestudyReportObservation=>({student:{studentId:'77001',displayName:input.displayName},prestudyKey:'12345',
  sourceRecord:{prestudyKey:'12345',unitLabel:'9-1 Invented unit',dateLiteral:'40.01.02 03:04'},
  sections:[{title:'개념 이해',subtitle:null,blocks:[{heading:'reason',text:'πr² = 50%\n−2 < 0'}]},
    {title:'논리적 설명',subtitle:null,blocks:[{heading:null,text:'An invented explanation'}]},
    {title:'총평',subtitle:null,blocks:[{heading:null,text:'An invented conclusion'}]}],
  scores:[77,80],encodingResult:'success',observedAt:'2040-01-02T03:04:00.000Z',coverage:'invented_source_page'});
function service(value=observation()){
  let reads=0;
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readPrestudyReportByName:async()=>{reads++;return structuredClone(value);}};
  return {reader:new DailyRecordService(backend),reads:()=>reads};
}

test('legacy full report retains sections, envelope data and explicit source identity',async()=>{
  const {reader}=service();const result=await reader.prestudyReportGet(input);
  expect(result).toMatchObject({status:'observed',sections:observation().sections,scores:[77,80],
    sourceRecord:observation().sourceRecord,academyWrites:0,parentSends:0,automaticRetryAllowed:false,
    studentContent:'teacher_only_do_not_store_or_relay',view:{mode:'full',completeRequestedView:true,completeSourceReport:true}});
});

test('section selection preserves source order and exact notation without returning unrelated narrative',async()=>{
  const {reader,reads}=service();const result=await reader.prestudyReportGet({...input,sectionTitles:['총평','개념 이해']});
  expect(result).toMatchObject({status:'observed',sections:[observation().sections[0],observation().sections[2]],
    view:{mode:'selected',requestedSectionTitles:['총평','개념 이해'],returnedSectionTitles:['개념 이해','총평'],
      availableSectionTitles:['개념 이해','논리적 설명','총평'],missingSectionTitles:[],completeRequestedView:true,completeSourceReport:false}});
  expect(JSON.stringify(result)).not.toContain('An invented explanation');expect(reads()).toBe(1);
});

test('missing selected sections remain explicit partial coverage, including when none matched',async()=>{
  for(const titles of [['概念理解','개념 이해'],['概念理解']]){
    const {reader,reads}=service();const result=await reader.prestudyReportGet({...input,sectionTitles:titles});
    expect(result).toMatchObject({status:'partial',view:{missingSectionTitles:['概念理解'],completeRequestedView:false,completeSourceReport:false}});
    expect('sections' in result&&result.sections).toHaveLength(titles.length-1);expect(reads()).toBe(1);
  }
});

test('ambiguous selected section titles fail without returning narratives or retrying',async()=>{
  const value=observation();value.sections.push({...value.sections[0],blocks:[{heading:null,text:'Other ambiguous narrative'}]});
  const {reader,reads}=service(value);const result=await reader.prestudyReportGet({...input,sectionTitles:['개념 이해']});
  expect(result).toMatchObject({status:'unavailable',reasonCode:'prestudy_report_section_ambiguous',failure:{category:'identity_ambiguity',automaticRetryAllowed:false}});
  expect(JSON.stringify(result)).not.toContain('Other ambiguous narrative');expect(reads()).toBe(1);
});

test('source identity mismatch fails even if the echoed requested key and student match',async()=>{
  const value=observation();value.sourceRecord.prestudyKey='12346';
  const {reader,reads}=service(value);const result=await reader.prestudyReportGet(input);
  expect(result).toMatchObject({status:'unavailable',reasonCode:'prestudy_report_scope_conflict'});
  expect(JSON.stringify(result)).not.toContain('An invented conclusion');expect(reads()).toBe(1);
});

test('UTF-8 output budget rejects the whole report instead of clipping or leaking it',async()=>{
  const value=observation();value.sections[0].blocks[0].text='가'.repeat(2000);
  const {reader,reads}=service(value);const result=await reader.prestudyReportGet({...input,maxOutputBytes:1024});
  expect(result).toMatchObject({status:'unavailable',reasonCode:'prestudy_report_result_too_large',failure:{category:'partial_coverage'}});
  expect(JSON.stringify(result)).not.toContain('가');expect(reads()).toBe(1);
});

test('a selected view can fit a budget when the complete source report cannot',async()=>{
  const value=observation();value.sections[1].blocks[0].text='large invented narrative '.repeat(3000);
  const {reader}=service(value);const result=await reader.prestudyReportGet({...input,sectionTitles:['개념 이해'],maxOutputBytes:4096});
  expect(result).toMatchObject({status:'observed',sections:[observation().sections[0]],view:{completeRequestedView:true,completeSourceReport:false}});
});

test('the selected budget measures one JSON payload, not its duplicated MCP envelope',async()=>{
  const value=observation();value.sections[0].blocks[0].text='a'.repeat(17000);
  const result=await service(value).reader.prestudyReportGet({...input,sectionTitles:['개념 이해']});
  expect(result).toMatchObject({status:'observed',sections:[value.sections[0]]});
  const bytes=Buffer.byteLength(JSON.stringify(result),'utf8');
  expect(bytes).toBeLessThan(20*1024);
  expect(await service(value).reader.prestudyReportGet({...input,sectionTitles:['개념 이해'],maxOutputBytes:bytes}))
    .toMatchObject({status:'observed',sections:[value.sections[0]]});
  expect(await service(value).reader.prestudyReportGet({...input,sectionTitles:['개념 이해'],maxOutputBytes:bytes-1}))
    .toMatchObject({status:'unavailable',reasonCode:'prestudy_report_result_too_large'});
});

test('the existing independent MCP envelope safety cap still applies',async()=>{
  const value=observation();
  value.sections=['A','B'].map(title=>({title,subtitle:null,blocks:Array.from({length:50},()=>({heading:'가'.repeat(1000),text:'valid'}))}));
  expect(Buffer.byteLength(JSON.stringify(value),'utf8')).toBeLessThan(512*1024);
  expect(await service(value).reader.prestudyReportGet({...input,maxOutputBytes:512*1024}))
    .toMatchObject({status:'unavailable',reasonCode:'prestudy_report_result_too_large'});
});

test('all selected sections can comprise the full source; default selected budget remains bounded',async()=>{
  const {reader}=service();expect(await reader.prestudyReportGet({...input,sectionTitles:['総評','개념 이해','논리적 설명','총평']}))
    .toMatchObject({status:'partial',view:{completeRequestedView:false,completeSourceReport:true}});
  const value=observation();value.sections[0].blocks[0].text='large invented narrative '.repeat(2000);
  expect(await service(value).reader.prestudyReportGet({...input,sectionTitles:['개념 이해']}))
    .toMatchObject({status:'unavailable',reasonCode:'prestudy_report_result_too_large'});
});

test('invalid selectors and budget are refused before backend use',async()=>{
  for(const extra of [{sectionTitles:[]},{sectionTitles:['a','a']},{maxOutputBytes:0},{maxOutputBytes:524289}]){
    expect(prestudyReportGetSchema.safeParse({...input,...extra}).success).toBe(false);
  }
});

test('native backend preserves source metadata and rejects conflicting or missing source identity',async()=>{
  const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-02',
    students:[{studentId:'77001',displayName:input.displayName}],sourceBundleSha256:'b'.repeat(64)};
  for(const kind of ['valid','mismatch','missing'] as const){
    let reads=0;
    const port={authenticate:async()=>{},readPrestudyReport:async(args:unknown)=>{
      expect(args).toEqual({teacherLabel:'Invented Teacher',studentName:input.displayName,studentKey:'77001',prestudyKey:'12345'});
      reads++;const value=observation();
      return {...value,sourceRecord:kind==='missing'?undefined:{...value.sourceRecord,prestudyKey:kind==='mismatch'?'12346':'12345'}};
    },close:()=>{}};
    const backend=new NativeDailyRecordBackend(port as never,admission,()=>({username:'invented',password:'invented'}));
    const result=await new DailyRecordService(backend).prestudyReportGet(input);
    expect(result).toMatchObject(kind==='valid'?{status:'observed',sourceRecord:observation().sourceRecord}:
      {status:'unavailable',reasonCode:kind==='mismatch'?'prestudy_report_scope_conflict':'source_response_malformed'});
    expect(reads).toBe(1);
  }
});

test('source failures keep their finite cause and never return a fabricated view',async()=>{
  let calls=0;
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readPrestudyReportByName:async()=>{calls++;throw Error('prestudy_report_not_ready');}};
  const result=await new DailyRecordService(backend).prestudyReportGet({...input,sectionTitles:['개념 이해']});
  expect(result).toMatchObject({status:'unavailable',reasonCode:'prestudy_report_not_ready',failure:{category:'partial_coverage',automaticRetryAllowed:false}});
  expect('view' in result).toBe(false);expect(calls).toBe(1);
});

test('selected and partial views survive the actual MCP SDK wire in both compatible representations',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readPrestudyReportByName:async()=>observation()};
  const connection=await dailyRecordMemory(backend);
  try{
    for(const sectionTitles of [['개념 이해'],['개념 이해','Unavailable section']]){
      const result=await connection.client.callTool({name:'academy_prestudy_report_get',arguments:{...input,sectionTitles}});
      expect(result.isError).not.toBe(true);
      expect(result.structuredContent).toMatchObject({status:sectionTitles.length===1?'observed':'partial',sections:[observation().sections[0]],
        sourceRecord:observation().sourceRecord,view:{completeRequestedView:sectionTitles.length===1,completeSourceReport:false}});
      const text=(result.content as {type:string;text:string}[]).find(c=>c.type==='text')!;
      expect(JSON.parse(text.text)).toEqual(result.structuredContent);
    }
  }finally{await connection.close();}
});
