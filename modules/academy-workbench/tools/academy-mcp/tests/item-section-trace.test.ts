import {expect,test} from 'bun:test';
import {DailyRecordService,type DailyRecordReadBackend,type ItemSectionTraceObservation} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';

const request={displayName:'Invented Student',purpose:'Trace fictional items',currentRequestRef:'synthetic-trace-request',labels:['8101','8201','8301']};
const traced:ItemSectionTraceObservation={kind:'traced',student:{studentId:'77001',displayName:'Invented Student'},
  books:[{bookLabel:'Invented Book',sectionsScanned:2,sectionsFailed:0}],
  items:[{label:'8101',matches:[{bookLabel:'Invented Book',sourceTitle:'[필수예제] Unit A - Invented Student',ordinal:'2',matchedBy:'lectureKey',originalScoring:'X'}]},
    {label:'8201',matches:[]},{label:'8301',matches:[{bookLabel:'Invented Book',sourceTitle:'[유형다지기] Unit A - Invented Student',ordinal:'1',matchedBy:'outerLectureKey',originalScoring:'O'}]}],
  unmatchedLabels:['8201'],ambiguousLabels:[],sectionsScanned:2,sectionsFailed:0,observedAt:'2040-01-16T09:00:00.000Z',
  coverage:'live_section_scan_original_attempt_lineage_not_generated_clinic_membership'};

test('item section trace projects matches, unmatched labels and lineage limits',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readItemSectionTrace:async input=>{expect(input.labels).toEqual(request.labels);return traced;}};
  const result=await new DailyRecordService(backend).itemSectionTrace(request);
  expect(result).toMatchObject({status:'observed',kind:'traced',academyWrites:0,unmatchedLabels:['8201']});
  expect((result as any).items[0]).toMatchObject({label:'8101',matches:[{sourceTitle:'[필수예제] Unit A - Invented Student',originalScoring:'X'}]});
});

test('item section trace passes interview-required bindings through without guessing',async()=>{
  const backend:DailyRecordReadBackend={readDailyRecord:async()=>({status:'unavailable',reasonCode:'not_used'}),
    readItemSectionTrace:async()=>({kind:'interview_required',student:{studentId:'77001',displayName:'Invented Student'},
      bindings:[{bookLabel:'Book One',bookNo:'7'},{bookLabel:'Book Two',bookNo:'8'}],reason:'multiple_book_bindings_select_one',observedAt:'2040-01-16T09:00:00.000Z'})};
  expect(await new DailyRecordService(backend).itemSectionTrace(request)).toMatchObject({status:'interview_required',bindings:[{bookLabel:'Book One',bookNo:'7'},{bookLabel:'Book Two',bookNo:'8'}]});
  expect(await new DailyRecordService().itemSectionTrace(request)).toMatchObject({status:'unavailable',reasonCode:'item_section_trace_backend_not_configured'});
});

test('native backend scans every binding, matches lecture and outer keys, and keeps failed sections explicit',async()=>{
  const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',
    students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
  const sections:Record<string,any>={'[필수예제] Unit A - Invented Student':{score:80,items:[{originalOrdinal:'2',lectureKey:'8101',outerLectureKey:'9101',scoring:'X'}]},
    '[유형다지기] Unit A - Invented Student':{score:90,items:[{originalOrdinal:'1',lectureKey:null,outerLectureKey:'8301',scoring:'O'}]}};
  const port={authenticate:async()=>{},discoverDayRecordGroups:async()=>({groups:[]}),
    readTextbookBindings:async()=>[{courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'Invented Book'}],
    inspectTextbookIndex:async()=>({rows:Object.keys(sections).map(sourceTitle=>({sourceTitle,sourceDate:{literal:null,meaning:'x'}}))}),
    execute:async(input:any)=>{const title=input.sourceTitles[0];const s=sections[title];
      return {family:'textbook',status:'observed',observedAt:'2040-01-16T09:00:00.000Z',coverage:'x',records:[{sourceTitle:title,studentKey:'77001',studentName:'Invented Student',courseKey:'5',bookNo:'7',score:s.score,items:s.items,sourceDate:{literal:null,precision:'unknown',meaning:'source_apply_date_not_exact_grade_time'}}]};},
    journalState:async()=>{throw Error('not_called');},close:()=>{}};
  const backend=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
  const result=await backend.readItemSectionTrace!(request);
  expect(result).toMatchObject({kind:'traced',unmatchedLabels:['8201'],sectionsScanned:2,sectionsFailed:0});
  expect((result as any).items[0]).toMatchObject({label:'8101',matches:[{sourceTitle:'[필수예제] Unit A - Invented Student',ordinal:'2',matchedBy:'lectureKey'}]});
  expect((result as any).items[2]).toMatchObject({label:'8301',matches:[{matchedBy:'outerLectureKey'}]});
  await expect(backend.readItemSectionTrace!({...request,displayName:'Foreign Student'})).rejects.toThrow('textbook_student_not_admitted');
});
test('native trace stops at the first source failure and reports its cause instead of unmatched labels',async()=>{
 const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
 let reads=0;
 const port={authenticate:async()=>{},readTextbookBindings:async()=>[{courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'Invented Book'}],inspectTextbookIndex:async()=>({rows:[{sourceTitle:'A'},{sourceTitle:'B'}]}),execute:async()=>{reads++;throw Error('textbook_section_result_unqualified');},close:()=>{}};
 const native=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
 const out=await new DailyRecordService(native).itemSectionTrace(request);
 expect(out).toMatchObject({status:'unavailable',reasonCode:'textbook_section_result_unqualified',failure:{category:'contract_drift'}});
 expect(reads).toBe(1);expect(out).not.toHaveProperty('unmatchedLabels');
});

test('native trace refuses unqualified empty bindings before claiming unmatched labels',async()=>{
 const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
 let reads=0;const port={authenticate:async()=>{},readTextbookBindings:async()=>[],inspectTextbookIndex:async()=>{reads++;return {rows:[]};},close:()=>{}};
 const native=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
 const out=await new DailyRecordService(native).itemSectionTrace(request);
 expect(out).toMatchObject({status:'unavailable',reasonCode:'textbook_binding_source_unqualified',failure:{category:'contract_drift'}});
 expect(out).not.toHaveProperty('unmatchedLabels');expect(reads).toBe(0);
});
test('section ambiguity uses registered course and book identity even when book labels and titles coincide',async()=>{
 const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
 for(const multiple of [false,true]){
  const binding={courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'Same displayed book label'};
  const port={authenticate:async()=>{},readTextbookBindings:async()=>multiple?[binding,{...binding,courseKey:'6',bookNo:'8'}]:[binding],inspectTextbookIndex:async()=>({rows:[{sourceTitle:'Same unit title'}]}),
   execute:async(i:any)=>({family:'textbook',status:'observed',records:[{studentKey:'77001',studentName:'Invented Student',courseKey:i.courseKey,bookNo:i.bookNo,sourceTitle:'Same unit title',score:80,items:[{originalOrdinal:'1',lectureKey:'8101',outerLectureKey:'8101',scoring:'O'}]}]}),close:()=>{}};
  const native=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
  const out=await native.readItemSectionTrace!({...request,labels:['8101']});
  // Two registered books without a bookLabel: the caller selects one; a trace never spans books.
  expect(out).toMatchObject(multiple?{kind:'interview_required',bindings:[{bookLabel:'Same displayed book label',bookNo:'7'},{bookLabel:'Same displayed book label',bookNo:'8'}]}
   :{kind:'traced',ambiguousLabels:[]});
 }
});

test('a single book above the scan cap returns an explicit coverage limit without an unresolvable interview',async()=>{
 const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
 let details=0;const port={authenticate:async()=>{},readTextbookBindings:async()=>[{courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'Invented Book'}],inspectTextbookIndex:async()=>({rows:Array.from({length:65},(_,i)=>({sourceTitle:'Fictional Unit '+i}))}),execute:async()=>{details++;throw Error('not_called');},close:()=>{}};
 const native=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
 for(const bookLabel of [undefined,'Invented Book']){
  const out=await new DailyRecordService(native).itemSectionTrace({...request,...(bookLabel?{bookLabel}:{})});
  expect(out).toMatchObject({status:'unavailable',reasonCode:'textbook_section_scan_limit',failure:{category:'partial_coverage'}});
  expect(out).not.toHaveProperty('unmatchedLabels');expect(out).not.toHaveProperty('bindings');
 }
 expect(details).toBe(0);
});

test('check 4: a trace without bookLabel for two bound books asks for the book before any per-book search',async()=>{
 const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
 const reads:string[]=[];
 const port={authenticate:async()=>{},
  readTextbookBindings:async()=>{reads.push('bindings');return [{courseKey:'5',bookNo:'7',studentKey:'77001',bookLabel:'Invented Book One'},{courseKey:'6',bookNo:'8',studentKey:'77001',bookLabel:'Invented Book Two'}];},
  inspectTextbookIndex:async()=>{reads.push('index');return {rows:[{sourceTitle:'Fictional Unit'}]};},
  execute:async()=>{reads.push('section');throw Error('not_called');},close:()=>{}};
 const native=new NativeDailyRecordBackend(port as never,admission,()=>({username:'fictional',password:'fictional'}));
 const out=await new DailyRecordService(native).itemSectionTrace(request);
 expect(out).toMatchObject({status:'interview_required',kind:'interview_required',reason:'multiple_book_bindings_select_one',
  bindings:[{bookLabel:'Invented Book One',bookNo:'7'},{bookLabel:'Invented Book Two',bookNo:'8'}]});
 expect(out).not.toHaveProperty('unmatchedLabels');
 expect(reads).toEqual(['bindings']);
 // With the book named, the trace proceeds with that one book only.
 reads.length=0;
 const named=await native.readItemSectionTrace!({...request,bookLabel:'Invented Book Two'}).catch(error=>error);
 expect(named).toBeInstanceOf(Error);
 expect(reads).toEqual(['bindings','index','section']);
});
