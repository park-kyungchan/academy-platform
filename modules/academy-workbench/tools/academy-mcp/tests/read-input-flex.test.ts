import {afterEach,expect,setSystemTime,test} from 'bun:test';
import {mkdtemp,readFile,rename,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {McpServer,InMemoryTransport} from '@modelcontextprotocol/server';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {registerDailyRecordTools} from '../src/dailyrecord-tools.ts';
import {installToolAudit} from '../src/tool-audit.ts';
import {readDailyTestHeaders} from '../src/dailytest-header-route.ts';

// TASK-FLEX-2. Fictional data only; no request leaves the process.
// Fixture clock: 2026-11-17 12:00 in Asia/Seoul.
const fixtureNow=new Date('2026-11-17T03:00:00Z');
const refusal='date_form_unsupported_use_yyyy_mm_dd_today_yesterday_or_month_day';
afterEach(()=>{setSystemTime();});

const teacherId='99001';
const student={studentId:'77009',displayName:'Unlisted Student'};
const launch={teacherId,teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
const bookTwo='(22개정) 초5-1 가우스 2권 초5-1 발전 2권(2022개정)',bookOne='(22개정) 초5-1 가우스 1권 초5-1 발전 1권(2022개정)';
const twoBooks=[{courseKey:'5',bookNo:'72',studentKey:student.studentId,bookLabel:bookTwo},{courseKey:'5',bookNo:'71',studentKey:student.studentId,bookLabel:bookOne}];
const previousDate=(date:string)=>new Date(Date.parse(date+'T00:00:00Z')-86400000).toISOString().slice(0,10);
const dayRow=(name:string,key:string,date:string)=>({studentName:name,date,groupId:'3',teacherKey:teacherId,studentKey:key,courseKey:'5',recordKey:date.replaceAll('-',''),membershipKey:'8',
  fields:{progress:'- Fictional progress',homework:'- Fictional homework',lessonMemo:'',attendance:'Y'}});

function sourcePort(bindings=[twoBooks[0]!]){
  const events:string[]=[];
  const keyOf=(name:string)=>name===student.displayName?student.studentId:null;
  const port={
    authenticate:async()=>{},
    discoverDayRecordGroups:async(input:any)=>{events.push('groups:'+input.date);return {groups:[{groupId:'3'}]};},
    searchStudentsByName:async(input:any)=>{const key=keyOf(input.studentName);return key?[{studentKey:key,studentName:input.studentName}]:[];},
    execute:async(input:any)=>{
      if(input.family==='day_record'){
        events.push('day:'+input.date);const name=input.studentNames[0],key=keyOf(name);
        return {dayRecords:key?[dayRow(name,key,input.date)]:[]};
      }
      if(input.family==='textbook'){
        events.push('textbook:'+input.bookNo);
        return {family:'textbook',status:'observed',observedAt:new Date().toISOString(),coverage:'fictional',
          records:input.sourceTitles.map((sourceTitle:string)=>({sourceTitle,studentKey:input.studentKey,studentName:input.studentName,courseKey:input.courseKey,bookNo:input.bookNo,score:80,
            items:[{originalOrdinal:'1',lectureKey:'8101',outerLectureKey:null,scoring:'X'}],sourceDate:{literal:null,precision:'unknown',meaning:'source_apply_date_not_exact_grade_time'}}))};
      }
      throw Error('unexpected_family');
    },
    journalState:async(target:any)=>({target,fields:{progress:'- Fictional progress',homework:'- Fictional homework',lessonMemo:'',attendance:'Y',dailyTestAssessment:'0',previousHomeworkAssessment:'3'},
      previousRecordKey:previousDate(target.date).replaceAll('-',''),previousDate:previousDate(target.date),wireDigest:'a'.repeat(64)}),
    readPrestudy:async()=>({waiting:[],completed:[{unitLabel:'Fictional unit',dateLiteral:'06-30',statusLiteral:'done',prestudyKey:'4401'}],summary:[],observedAt:new Date().toISOString(),coverage:'fictional'}),
    readTextbookBindings:async()=>bindings,
    readDailyTestHistory:async(input:any)=>{
      const page=(selector:string)=>({rows:[{studentKey:input.studentKey,studentName:input.studentName,paperKey:'81001',attemptKey:'91001',sourceRowId:null,sourceScore:80,titles:{title:'Fictional DailyTest'}}],
        operation:'DtResultSearch',page:1,sourcePeriod:null,observedAt:new Date().toISOString(),coverage:'one_observed_student_search_page_not_complete_history',
        generatedMembership:'unqualified',fixedSourceFilter:{sort_date:selector},expectedPaperDefinitions:'not_observed',selector});
      return {status:'observed',studentKey:input.studentKey,studentName:input.studentName,currentFilterQualification:{observedAt:new Date().toISOString(),selector:'1',rows:page('1').rows},
        observations:input.periods.map(page),calendarBounds:'source_relative_labels_only_not_exact_calendar_bounds',expectedPaperDefinitions:'not_observed',
        coverage:'all_parsed_embedded_rows_on_each_observed_response_not_complete_history'};
    },
    inspectTextbookIndex:async(input:any)=>({rows:[{sourceTitle:'Section of book '+input.bookNo,sourceDate:{literal:'11-10',meaning:'source_apply_date_not_exact_grade_time'}}]}),
    close:()=>{},
  };
  return {port,events};
}
/** The dailyrecord endpoint's composition (tool audit, then the tools) on the SDK wire. */
async function endpoint(bindings?:typeof twoBooks){
  const f=sourcePort(bindings);
  const backend=new NativeDailyRecordBackend(f.port as never,launch,()=>({username:'fictional',password:'fictional'}));
  const server=new McpServer({name:'fictional-flex-endpoint',version:'1.0.0'});
  installToolAudit(server,{endpoint:'dailyrecord'});
  registerDailyRecordTools(server,new DailyRecordService(backend));
  const client=new Client({name:'fictional-flex-client',version:'1.0.0'});
  const [clientTransport,serverTransport]=InMemoryTransport.createLinkedPair();
  await server.connect(serverTransport);await client.connect(clientTransport);
  const call=async(name:string,args:Record<string,unknown>)=>{
    const result:any=await client.callTool({name,arguments:args});
    expect(result.isError).toBeFalsy();
    return result.structuredContent as Record<string,any>;
  };
  return {call,events:f.events,close:async()=>{await client.close();await server.close();}};
}
/** Values that differ between two equal reads: request identity, clocks, handles and digests over them. */
const volatile=new Set(['purpose','currentRequestRef','observedAt','observationHandle','boundTo','sourceRevision','wireDigest','authoringHandle','evidenceRefs']);
function stable(value:unknown):unknown{
  if(Array.isArray(value))return value.map(stable);
  if(!value||typeof value!=='object')return value;
  return Object.fromEntries(Object.entries(value).filter(([k])=>!volatile.has(k)).map(([k,v])=>[k,stable(v)]));
}
const generatedRef=/^auto-[0-9a-f-]{36}$/;

test('check 1: four read tools answer without purpose and currentRequestRef as with them, and return the ref they made',async()=>{
  const given={purpose:'Fictional teacher read',currentRequestRef:'fictional-request-1'};
  const calls:[string,Record<string,unknown>][]=[
    ['academy_dailyrecord_get',{displayName:student.displayName,lessonDate:'2026-11-16'}],
    ['academy_student_resolve',{displayName:student.displayName}],
    ['academy_prestudy_get',{displayName:student.displayName}],
    ['academy_textbook_section_results_get',{displayName:student.displayName}],
  ];
  const e=await endpoint();
  try{
    for(const [name,args] of calls){
      const withIdentity=await e.call(name,{...args,...given});
      const without=await e.call(name,args);
      expect(withIdentity).toMatchObject({...given,academyWrites:0});
      expect(['observed','resolved']).toContain(withIdentity.status);
      expect(stable(without)).toEqual(stable(withIdentity));
      expect(without.purpose).toBe(name);
      expect(without.currentRequestRef).toMatch(generatedRef);
      const again=await e.call(name,args);
      expect(again.currentRequestRef).toMatch(generatedRef);
      expect(again.currentRequestRef).not.toBe(without.currentRequestRef);
    }
  }finally{await e.close();}
},30000);

test('check 1: through the bridge, academy_dailyrecord_get and academy_student_resolve need no request identity',async()=>{
  const entry=resolve(import.meta.dir,'../src/neutral-server.ts'),deny=resolve(import.meta.dir,'network-deny.ts');
  const hash=(bytes:string|Buffer)=>createHash('sha256').update(bytes).digest('hex');
  const base=await mkdtemp(resolve(tmpdir(),'flex-2-bridge-'));
  try{
    const events=resolve(base,'events.txt');await writeFile(events,'');
    const endpointsValue={dailyrecord:{command:process.execPath,args:['--preload',deny,resolve(import.meta.dir,'gates-upstream-fixture.ts'),events]}};
    await writeFile(resolve(base,'routing.json'),'{}');
    await writeFile(resolve(base,'endpoints.json'),JSON.stringify(endpointsValue));
    const env={PATH:process.env.PATH??'/usr/bin:/bin',ACADEMY_MCP_INSTALL_DIR:base};
    const discovery=Bun.spawn([process.execPath,'--preload',deny,entry,'--discover'],{env,stdout:'pipe',stderr:'pipe'});
    expect(await discovery.exited).toBe(0);
    const pin=async(name:string,bytes:string)=>{const path=resolve(base,'pinned-'+name);await writeFile(path,bytes,{mode:0o600,flag:'wx'});return {path,sha256:hash(bytes)};};
    const registry=await pin('registry.json',await readFile(resolve(base,'registry.json'),'utf8'));
    const release={schemaVersion:'academy.mcp-release/1',generation:'flex-2',routing:await pin('routing.json','{}'),endpoints:await pin('endpoints.json',JSON.stringify(endpointsValue)),registry};
    await writeFile(resolve(base,'release.tmp'),JSON.stringify(release),{mode:0o600});await rename(resolve(base,'release.tmp'),resolve(base,'release.json'));
    const client=new Client({name:'fictional-flex-bridge-client',version:'1.0.0'});
    const transport=new StdioClientTransport({command:process.execPath,args:['--preload',deny,entry],env,stderr:'pipe'});
    await client.connect(transport);
    try{
      const tools=(await client.listTools()).tools;
      for(const name of ['academy_dailyrecord_get','academy_student_resolve','academy_prestudy_get','academy_textbook_section_results_get']){
        const required=(tools.find(t=>t.name===name)!.inputSchema.required??[]) as string[];
        expect(required).not.toContain('purpose');expect(required).not.toContain('currentRequestRef');
      }
      const got:any=await client.callTool({name:'academy_dailyrecord_get',arguments:{displayName:'Unlisted Student',lessonDate:'2041-06-30'}});
      expect(got.isError).toBeFalsy();
      expect(got.structuredContent).toMatchObject({status:'observed',purpose:'academy_dailyrecord_get',lessonDate:'2041-06-30',scope:{studentId:'77009',lessonDate:'2041-06-30'}});
      expect(got.structuredContent.currentRequestRef).toMatch(generatedRef);
      const resolved:any=await client.callTool({name:'academy_student_resolve',arguments:{displayName:'Unlisted Student'}});
      expect(resolved.structuredContent).toMatchObject({status:'resolved',studentKey:'77009',purpose:'academy_student_resolve'});
      expect(resolved.structuredContent.currentRequestRef).toMatch(generatedRef);
    }finally{await client.close();await transport.close();}
  }finally{await rm(base,{recursive:true,force:true});}
},120000);

test('check 2: 어제, 11월 16일(일), 11/16 and 2026-11-16 give the same answer on 2026-11-17 in Asia/Seoul',async()=>{
  setSystemTime(fixtureNow);
  const forms=['어제','11월 16일(일)','11/16','2026-11-16'];
  const e=await endpoint();
  try{
    for(const name of ['academy_dailyrecord_get','academy_student_resolve','academy_student_metadata_get']){
      const extra=name==='academy_student_metadata_get'?{queries:[{kind:'daily_history',periods:['11']}]}:{};
      const answers:Record<string,any>[]=[];
      for(const lessonDate of forms){
        const answer=await e.call(name,{displayName:student.displayName,lessonDate,...extra});
        expect(answer.lessonDate).toBe('2026-11-16');
        answers.push(answer);
      }
      expect(answers.every(a=>a.status===answers[0]!.status)).toBe(true);
      for(const answer of answers.slice(1))expect(stable(answer)).toEqual(stable(answers[0]));
    }
    const days=new Set(e.events.filter(x=>x.startsWith('day:')));
    expect(days.has('day:2026-11-16')).toBe(true);expect([...days].filter(x=>x>'day:2026-11-16')).toEqual([]);
  }finally{await e.close();}
  // The bridge composition resolves the date before its metadata read.
  const seen:string[]=[];
  const port={metadata:async(input:any)=>{seen.push(input.lessonDate);throw Error('fixture_stop');},headers:async()=>{throw Error('not_called');}};
  for(const lessonDate of forms)await expect(readDailyTestHeaders({displayName:student.displayName,lessonDate,periods:['11']},port)).rejects.toThrow('fixture_stop');
  expect(seen).toEqual(['2026-11-16','2026-11-16','2026-11-16','2026-11-16']);
},30000);

test('check 2: 11월 31일 and next week are refused with the one code that names the accepted forms',async()=>{
  setSystemTime(fixtureNow);
  const e=await endpoint();
  try{
    for(const lessonDate of ['11월 31일','next week']){
      for(const name of ['academy_dailyrecord_get','academy_student_resolve','academy_student_metadata_get']){
        const extra=name==='academy_student_metadata_get'?{queries:[{kind:'daily_history',periods:['11']}]}:{};
        const answer=await e.call(name,{displayName:student.displayName,lessonDate,...extra});
        expect(answer).toMatchObject({status:'blocked',code:refusal,failure:{category:'input_validation',code:refusal},academyWrites:0});
      }
      const port={metadata:async()=>{throw Error('not_called');},headers:async()=>{throw Error('not_called');}};
      await expect(readDailyTestHeaders({displayName:student.displayName,lessonDate,periods:['11']},port)).rejects.toThrow(refusal);
    }
    expect(e.events.filter(x=>x.startsWith('day:'))).toEqual([]);
  }finally{await e.close();}
},30000);

test('check 3: a loose book name selects the one agreeing bound book; one that agrees with several or none asks',async()=>{
  const e=await endpoint(twoBooks);
  try{
    const tools:[string,Record<string,unknown>][]=[
      ['academy_textbook_section_results_get',{}],
      ['academy_textbook_section_results_get',{sourceTitles:['Section of book 72']}],
      ['academy_clinic_paper_status_get',{}],
      ['academy_clinic_paper_status_get',{sourceTitles:['Section of book 72']}],
      ['academy_item_section_trace',{labels:['8101']}],
    ];
    for(const [name,extra] of tools){
      for(const bookLabel of ['가우스 5-1-2','초5-1 가우스 2권','가우스 5학년 1학기 2권']){
        const answer=await e.call(name,{displayName:student.displayName,bookLabel,...extra});
        expect(answer.status).toBe('observed');
        expect(answer.bookSelection).toEqual({requested:bookLabel,boundLabel:bookTwo,match:'loose'});
        if(name!=='academy_item_section_trace')expect(answer.bookNo).toBe('72');
        else expect(answer.books).toEqual([{bookLabel:bookTwo,sectionsScanned:1,sectionsFailed:0}]);
      }
      const exact=await e.call(name,{displayName:student.displayName,bookLabel:bookOne,...extra});
      expect(exact.status).toBe('observed');
      expect(exact.bookSelection).toEqual({requested:bookOne,boundLabel:bookOne,match:'exact'});
      const several=await e.call(name,{displayName:student.displayName,bookLabel:'가우스 5-1',...extra});
      expect(several).toMatchObject({status:'interview_required',kind:'interview_required',bookSelection:{requested:'가우스 5-1',match:'several'}});
      expect(several.bindings.map((b:any)=>b.bookLabel)).toEqual([bookTwo,bookOne]);
      const none=await e.call(name,{displayName:student.displayName,bookLabel:'다빈치 5-1-2',...extra});
      expect(none).toMatchObject({status:'interview_required',kind:'interview_required',bookSelection:{requested:'다빈치 5-1-2',match:'none'}});
      expect(none.bindings.map((b:any)=>b.bookLabel)).toEqual([bookTwo,bookOne]);
    }
  }finally{await e.close();}
},30000);
