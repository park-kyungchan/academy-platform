import type {CismReport} from '../../../../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol';
import {createHash,randomUUID} from 'node:crypto';
import {readFileSync,lstatSync,realpathSync,closeSync,fsyncSync,mkdirSync,openSync,writeSync} from 'node:fs';
import {dirname,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {z} from 'zod';
import {readPinnedLocalBytes} from './item-label-store.ts';
import {dailyRecordObservationSchema,studentMetadataBatchSchema,type DailyRecordReadBackend,type DailyRecordWriteBackend,type DailyRecordObservation,type StudentMetadataBatch,type StudentMetadataObservation,studentMetadataGetSchema,type StudentMetadataGet,type DailyRecordSaveFieldStatus,type TextbookSectionResults,type TextbookSectionObservation,type PrestudyGet,type PrestudyObservation,type PrestudyReportGet,type PrestudyReportObservation,type TextbookSectionResultsGet,type TextbookSectionResultsByNameInput,type HistoricalReportSummary,type TextbookSectionIndexObservation,type TextbookSectionResultsByNameObservation,type ItemSectionTrace,type ItemSectionTraceObservation} from './dailyrecord.ts';
import {createDailyRecordSaveAdmission,liveSaveRequestEvidence,type DailyRecordSaveAdmission,type DailyRecordSaveApproval} from './dailyrecord-save-admission.ts';
import type {DailyRecordActionApproval} from './dailyrecord-workflow.ts';
import type {DailyRecordField} from './dailyrecord-context.ts';
import {dailyRecordDateSchema,type DailyRecordScope} from './dailyrecord-context.ts';
import {AcademyReadFailure} from './read-failure.ts';
import {resolveStudentByName} from './student-resolver.ts';
import type {TextbookCatalogObservation,TextbookPdfObservation,TextbookSchoolLevel} from '../../../../../apps/academy/dc-gang-a/src/lms/textbookCatalogRead';
import {isTextbookBookLabel} from '../../../../../apps/academy/dc-gang-a/src/lms/prestudyRead';
import {itemAnswerLookupSchema} from './item-answer-lookup.ts';
import {bookSelectionOf,chooseBookBinding,type BookSelection} from './read-input.ts';
import type {HistoricalBindingReader} from './historical-binding-reader.ts';
import type {CourseBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/courseBookReport';
import {TEXTBOOK_SECTION_NOT_IN_EITHER} from '../../../../../apps/academy/dc-gang-a/src/lms/textbookSectionIndex';
import type {DailyRecordTeacherScope} from './dailyrecord-read.ts';
import type {PaperExamRow,PrintArchiveKind} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead';

const home='/opt/data/profiles/dc-gang-a-dev';
/** Owner-private artifacts root; owner workflow stores and admissions must live here. */
export const nativeOwnerArtifactsRoot=home+'/artifacts/';
const key=z.string().regex(/^[1-9][0-9]{0,15}$/),hash=z.string().regex(/^[a-f0-9]{64}$/);
export const nativeDailyRecordConfigSchema=z.object({schemaVersion:z.literal('academy.native-dailyrecord-read/1'),
  sourceBundlePath:z.string().startsWith('/'),sourceBundleSha256:hash,
  // `date` and `students` of earlier launches are accepted. The date gates nothing; a
  // listed student is only a known name/key pair, and every other student is found by name.
  date:z.iso.date().optional(),students:z.array(z.object({studentId:key,displayName:z.string().min(1).max(200)}).strict()).max(8).default([]),
}).strict().superRefine((v,c)=>{
  if(new Set(v.students.map(s=>s.studentId)).size!==v.students.length||new Set(v.students.map(s=>s.displayName)).size!==v.students.length)
    c.addIssue({code:'custom',message:'unique_admitted_students_required'});
});
type Target={studentName:string;date:string;groupId:string;teacherKey:string;studentKey:string;courseKey:string;recordKey:string;membershipKey:string};
type Row=Target&{fields:Record<string,string>};
interface SourcePort {
  readCismReport?(input:{teacherLabel:string;date:string}):Promise<CismReport>;
  applyCismField?(input:{teacherLabel:string;date:string;fieldRef:string;before:string;after:string;wireSha256:string;completeExistingNarrative?:boolean},authorize:()=>Promise<boolean>,recordAcknowledgement?:(value:{contentType:string;raw:Uint8Array;sha256:string})=>Promise<void>):Promise<{status:'persisted';fieldRef:string;report:CismReport}>;
  readTextbookCatalog?(input:{teacherLabel:string;schoolLevel:TextbookSchoolLevel}):Promise<TextbookCatalogObservation>;
  readTextbookPdf?(input:{teacherLabel:string;schoolLevel:TextbookSchoolLevel;documentRef:string}):Promise<TextbookPdfObservation>;
  readPaperExamList?(paperNo:string):Promise<PaperExamRow[]>;
  downloadPrintArchive?(kind:PrintArchiveKind,key:number):Promise<Uint8Array>;
  authenticate(credential:{username:string;password:string}):Promise<unknown>;
  discoverDayRecordGroups(input:{teacherLabel:string;date:string}):Promise<{groups:{groupId:string}[]}>;
  execute(input:unknown):Promise<{dayRecords:Row[]}>;
  /** `previousRecordKey` and `previousDate` are null on a first lesson, whose fields hold no `previousHomeworkAssessment`. */
  journalState(target:Target):Promise<{target:Target;fields:Record<string,string>;previousRecordKey:string|null;previousDate:string|null;wireDigest:string}>;
  /** Owner client write port (Education AcademyHttpClient.applyJournalAction).
   * Optional: read-only bundles omit it and saves stay fail-closed. */
  applyJournalAction?(action:{attemptId:string;target:Target;field:string;effectRecordKey:string;before:string;after:string;wireDigest:string},
    admission:{consumeApproval:(action:Readonly<{attemptId:string;target:Target;field:string;effectRecordKey:string;before:string;after:string;wireDigest:string}>)=>Promise<{status:'verified';actionDigest:string}|{status:'rejected'}>})
    :Promise<{status:DailyRecordSaveFieldStatus;transport:'returned'|'uncertain'|'not_invoked'}>;
  close():void;
  searchStudentPaperMetadata?(input:{teacherLabel:string;studentName:string;studentKey:string;period:{from:string;through:string}}):Promise<unknown>;
  searchDailyTestMetadata?(input:{teacherLabel:string;studentName:string;studentKey:string}):Promise<unknown>;
  inspectDailyTestInventory?(input:{teacherLabel:string;studentName:string;studentKey:string}):Promise<unknown>;
  readDailyTestHistory?(input:{teacherLabel:string;studentName:string;studentKey:string;periods:string[]}):Promise<unknown>;
  lookupPrintedPaperMetadata?(input:{teacherLabel:string;studentName:string;studentKey:string;paperCode:string;period:{from:string;through:string}}):Promise<unknown>;
  /** Name-only student search (REA contract 2026-10-26). The DT result servlet is keyed
   * on the wire by `{sort_date, stu_name}` alone; implementations must neither require nor
   * send a student key. Key/name equality is verified by the projection layer, never as
   * request input. No sequential key probing, before or after. */
  searchStudentsByName?(input:{teacherLabel:string;studentName:string}):Promise<unknown>;
  readPrestudy?(input:{teacherLabel:string;studentName:string}):Promise<{waiting:unknown[];completed:unknown[];summary:unknown[];observedAt:string;coverage:string}>;
  readPrestudyReport?(input:{teacherLabel:string;studentName:string;studentKey:string;prestudyKey:string}):Promise<{sections:unknown[];scores:number[];prestudyKey:string;sourceRecord:{prestudyKey:string;unitLabel:string|null;dateLiteral:string|null};encodingResult:string|null;observedAt:string;coverage:string}>;
  readTextbookBindings?(input:{teacherLabel:string;studentName:string;studentKey:string}):Promise<{courseKey:string;bookNo:string;studentKey:string;bookLabel:string}[]>;
  inspectTextbookIndex?(input:unknown):Promise<{rows:{sourceTitle:string;sourceDate:{literal:string|null;meaning:string};sources?:string[]}[];indexBasis?:string;coverage?:string}>;
  /** Direct-key historical reads (specs/academy-mcp/direct-key-lookup.md R3): the wrong-answer
   * note, the course-book report and, for sourceTitles, the single-user result of a verified attempt. */
  readHistoricalTextbookIndex?(input:HistoricalKeys):Promise<{rows:{sourceTitle:string;paperKey:string;attemptKey:string;sourceDate:{literal:string|null;meaning:string}}[];observedAt:string}>;
  readCourseBookReport?(input:HistoricalKeys):Promise<CourseBookReport>;
  /** One read-only report view of ReportViewServlet (specs/academy-mcp/direct-key-lookup.md R8): its POST form is
   * made and the view name validated by the client (reportViewClient.ts); the answer is the view's JSON. */
  readReportView?(view:string,params:Record<string,unknown>):Promise<Record<string,unknown>>;
  readHistoricalTextbookSections?(input:HistoricalKeys&{attempts:{sourceTitle:string;paperKey:string;attemptKey:string;sourceDateLiteral:string|null}[]}):Promise<{family:string;status:string;records:SectionRecord[];observedAt:string;coverage:string}>;
}
type HistoricalKeys={teacherLabel:string;studentKey:string;studentName:string;courseKey:string;bookNo:string};
type SectionRecord={sourceTitle:string;studentKey:string;studentName:string;courseKey:string;bookNo:string;
  score:number;items:{originalOrdinal:string;lectureKey?:string|number|null;outerLectureKey?:string|number|null;scoring:string}[];sourceDate:{literal:string|null;precision:string;meaning:string}};
type LiveBinding={courseKey:string;bookNo:string;studentKey:string;bookLabel:string};
/** The positive-integer key rule of R6, as readLiteralKey of the LMS client takes it. */
const directKey=(value:string)=>/^[1-9][0-9]{0,15}$/.test(value)&&Number(value)<=Number.MAX_SAFE_INTEGER;
/** The book a call reads: a live binding (R1), historical keys (R2), or a question to the teacher. */
type BookChoiceResult=
  |{kind:'live';binding:LiveBinding;bookSelection:{bookSelection?:BookSelection}}
  |{kind:'historical';courseKey:string;bookNo:string;label:string|null;bookSelection:{bookSelection?:BookSelection}}
  |{kind:'interview';candidates:{bookLabel:string;bookNo:string;courseKey?:string}[];hints:boolean;bookSelection:{bookSelection?:BookSelection}};
const normalizedTitle=(text:string)=>text.normalize('NFKC').toLowerCase().replace(/[\s\[\](){}<>「」『』【】·,._~-]+/g,'');
/** The report's book title agrees with a known label: the same text when normalized, one inside
 * the other, or the label's book words (brand, grade-term, volume) agreeing with the title alone. */
export function bookTitleAgrees(title:string,label:string):boolean{
  const a=normalizedTitle(title),b=normalizedTitle(label);
  if(!a||!b)return false;
  if(a===b||a.includes(b)||b.includes(a))return true;
  return chooseBookBinding(label,[{bookLabel:title}]).kind==='selected';
}
type Admission={teacherId:string;teacherLabel:string;date?:string;students:{studentId:string;displayName:string}[];sourceBundleSha256:string};
const fields=['progress','homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment'] as const;
const targetOf=(row:Target):Target=>Object.fromEntries(['studentName','date','groupId','teacherKey','studentKey','courseKey','recordKey','membershipKey'].map(k=>[k,row[k as keyof Target]])) as Target;
const observationTarget=(t:Target):DailyRecordObservation['target']=>({sourceNamespace:'academy.lms.dayrecord',teacherId:t.teacherKey,studentId:t.studentKey,lessonDate:t.date,courseId:t.courseKey,membershipId:t.membershipKey,recordId:t.recordKey});

const sourceInstant=z.iso.datetime({offset:true}),sourcePeriod=z.object({from:z.iso.date(),to:z.iso.date()});
const sourceTitle=z.string().max(2000).optional();
// These schemas deliberately strip every unlisted key. Page diagnostics, field inventories,
// scripts/handlers, controls, navigation and whole TR cells never cross the MCP boundary.
const metadataRow=z.object({studentKey:key,studentName:z.string().min(1).max(200),paperKey:key,attemptKey:key,
  sourceRowId:z.string().regex(/^[A-Za-z0-9_-]{1,64}$/).nullable(),sourceScore:z.number().finite().nullable(),
  titles:z.object({testpageName:sourceTitle,testPageName:sourceTitle,pName:sourceTitle,examName:sourceTitle,title:sourceTitle,
    testName:sourceTitle,testingName:sourceTitle,printTestingName:sourceTitle,testTypeName:sourceTitle,applyDate:sourceTitle}),
});
const metadataRows=z.array(metadataRow).max(2000);
// Name-search result pairs. The wire (DtResultSearch rows) and the owner admission table
// both project to exactly this shape; every unlisted key is stripped before the
// exact-name rules run. Extra keys, missing keys and non-string pairs are unqualified.
// An undated wire search that found the name after the sort_date '1' page names that page.
const historySelector=z.object({sort_date:z.enum(['11','12','13','14','15','16'])}).strict();
const nameSearchPair=z.object({studentKey:key,studentName:z.string().min(1).max(200),fixedSourceFilter:historySelector.optional()}).strict();
const nameSearchPairs=z.array(nameSearchPair).max(200);
const metadataPage=z.object({rows:metadataRows,operation:z.enum(['UserBySearchTestResult','DtResultSearch']),page:z.literal(1),
  sourcePeriod:sourcePeriod.nullable(),observedAt:sourceInstant,coverage:z.literal('one_observed_student_search_page_not_complete_history'),
  generatedMembership:z.literal('unqualified'),fixedSourceFilter:z.object({sort_date:z.enum(['1','11','12','13','14','15','16'])}).optional(),
  expectedPaperDefinitions:z.literal('not_observed').optional(),
});
function projectStudentMetadata(query:StudentMetadataBatch['queries'][number],value:unknown,student:Admission['students'][number]):unknown{
  const checkRows=(rows:z.infer<typeof metadataRows>)=>{
    if(rows.some(row=>row.studentKey!==student.studentId||row.studentName!==student.displayName)
      ||new Set(rows.map(row=>JSON.stringify([row.paperKey,row.attemptKey]))).size!==rows.length)throw Error('native_metadata_row_identity_conflict');
  };
  const checkPeriod=(period:z.infer<typeof sourcePeriod>|null,requested:{from:string;through:string})=>{
    if(period?.from!==requested.from||period.to!==requested.through)throw Error('native_metadata_period_conflict');
  };
  const checkDaily=(page:z.infer<typeof metadataPage>,selector:string)=>{
    checkRows(page.rows);
    if(page.operation!=='DtResultSearch'||page.sourcePeriod!==null||page.fixedSourceFilter?.sort_date!==selector)throw Error('native_metadata_selector_conflict');
  };
  if(query.kind==='daily_history'){
    const history=z.object({status:z.literal('observed'),studentKey:key,studentName:z.string(),
      currentFilterQualification:z.object({observedAt:sourceInstant,selector:z.literal('1'),rows:metadataRows}),
      observations:z.array(metadataPage.extend({selector:z.enum(['11','12','13','14','15','16']),expectedPaperDefinitions:z.literal('not_observed')})).max(6),
      calendarBounds:z.literal('source_relative_labels_only_not_exact_calendar_bounds'),expectedPaperDefinitions:z.literal('not_observed'),
      coverage:z.literal('all_parsed_embedded_rows_on_each_observed_response_not_complete_history'),
    }).parse(value);
    if(history.studentKey!==student.studentId||history.studentName!==student.displayName||history.observations.length!==query.periods.length)throw Error('native_metadata_history_scope_conflict');
    checkRows(history.currentFilterQualification.rows);
    history.observations.forEach((page,i)=>{if(page.selector!==query.periods[i])throw Error('native_metadata_selector_conflict');checkDaily(page,page.selector);});
    return history;
  }
  if(query.kind==='printed_paper_metadata'){
    const lookup=z.object({paperCode:z.string(),paperKey:key,status:z.enum(['observed','not_found_in_scope']),rows:metadataRows,
      sourcePeriod:sourcePeriod.nullable(),page:z.literal(1),observedAt:sourceInstant,coverage:z.literal('observed_source_pages_not_complete_history'),
      matchedSourceFamily:z.enum(['student_assessment','daily_zero']).nullable(),generatedMembership:z.literal('unqualified'),
      searchedSources:z.array(z.object({family:z.enum(['student_assessment','daily_zero']),operation:z.enum(['UserBySearchTestResult','DtResultSearch']),
        page:z.literal(1),sourcePeriod:sourcePeriod.nullable(),requestedPeriodApplied:z.boolean(),coverage:z.literal('one_observed_student_search_page_not_complete_history'),observedAt:sourceInstant})).min(1).max(2),
    }).parse(value);
    checkRows(lookup.rows);
    if(lookup.paperCode!==query.paperCode||lookup.rows.some(row=>row.paperKey!==lookup.paperKey)
      ||(lookup.status==='observed')!==(lookup.rows.length>0)||(lookup.matchedSourceFamily!==null)!==(lookup.rows.length>0))throw Error('native_metadata_paper_conflict');
    lookup.searchedSources.forEach((source,i)=>{
      if(i===0){if(source.family!=='student_assessment'||source.operation!=='UserBySearchTestResult'||!source.requestedPeriodApplied)throw Error('native_metadata_lookup_scope_conflict');checkPeriod(source.sourcePeriod,query.period);}
      else if(source.family!=='daily_zero'||source.operation!=='DtResultSearch'||source.requestedPeriodApplied||source.sourcePeriod!==null)throw Error('native_metadata_lookup_scope_conflict');
    });
    if(lookup.matchedSourceFamily==='daily_zero'){if(lookup.sourcePeriod!==null||lookup.searchedSources.length!==2)throw Error('native_metadata_lookup_scope_conflict');}
    else checkPeriod(lookup.sourcePeriod,query.period);
    return lookup;
  }
  const page=metadataPage.parse(value);
  if(query.kind==='student_papers'){
    checkRows(page.rows);checkPeriod(page.sourcePeriod,query.period);
    if(page.operation!=='UserBySearchTestResult'||page.fixedSourceFilter!==undefined||page.expectedPaperDefinitions!==undefined)throw Error('native_metadata_operation_conflict');
  }else{
    checkDaily(page,'1');
    if(query.kind==='daily_inventory'&&page.expectedPaperDefinitions!=='not_observed')throw Error('native_metadata_inventory_conflict');
  }
  return page;
}

/** Consumer transport glue. The existing Education client owns authentication, parsing,
 * source/wire qualification and independent occurrence binding. This port never writes. */
export class NativeDailyRecordBackend implements DailyRecordReadBackend,DailyRecordWriteBackend {
  #ready:Promise<unknown>|undefined;
  #sourceResolved=new Map<string,{studentId:string;displayName:string;lessonDate?:string;at:number}>();
  /** The fourth to sixth arguments held the owner admission, the student-name list and the
   * metadata admission of earlier launches. They are accepted and gate nothing: the LMS
   * itself limits every read to the configured teacher's students. */
  constructor(readonly client:SourcePort,readonly admission:Admission,readonly credential:()=>{username:string;password:string},
    _legacyOwnerAdmission:unknown=null,_legacyStudentNames:unknown=null,_legacyMetadataAdmission:unknown=null,
    readonly saveApproval:DailyRecordSaveApproval|null=null,
    /** The read-only edu-academy-memory bindings of the direct-key fallback; null: explicit keys only. */
    readonly historicalBindings:HistoricalBindingReader|null=null){}
  /** The student of a name-keyed read: the launch's own pair for that name, else the one
   * exact match of the source's name search, under the exact-name rules of the resolver. */
  async #studentByName(displayName:string):Promise<{studentId:string;displayName:string}>{
    const listed=this.admission.students.find(s=>s.displayName===displayName);
    if(listed)return listed;
    const rows=await this.#wireSearch(displayName);
    const result=await resolveStudentByName(displayName,{searchByName:async()=>rows});
    if(result.status!=='resolved')throw Error(result.reasonCode);
    return {studentId:result.studentKey,displayName:result.displayName};
  }
  /** `DiagReportView` or `FaView` of one student for academy_official_report_pdf_get: the student is found by
   * name, and an explicit studentKey must be that student's (official_report_scope_conflict) before the view
   * is requested with the resolved key. */
  async readStudentReportView(input:{view:'DiagReportView'|'FaView';displayName:string;studentKey?:string;testingNo:string;classNo:string;limitDate?:string}):Promise<{student:{studentId:string;displayName:string};raw:Record<string,unknown>}>{
    if(!this.client.readReportView)throw Error('report_view_source_port_unavailable');
    const student=await this.#studentByName(input.displayName);
    if(input.studentKey!==undefined&&input.studentKey!==student.studentId)throw Error('official_report_scope_conflict');
    await this.#authenticate();
    const raw=await this.client.readReportView(input.view,{testingNo:input.testingNo,studentKey:student.studentId,classNo:input.classNo,...(input.limitDate!==undefined?{limitDate:input.limitDate}:{})});
    return {student,raw};
  }
  /** `NewStudentReportView` of one application (paper number and apply code); with no college key the client
   * sends the owner's. No student is looked up: a new student has no key yet. */
  async readNewStudentReportView(input:{paperNo:string;applyCode:string;collegeKey?:string}):Promise<{raw:Record<string,unknown>}>{
    if(!this.client.readReportView)throw Error('report_view_source_port_unavailable');
    await this.#authenticate();
    return {raw:await this.client.readReportView('NewStudentReportView',{paperNo:input.paperNo,applyCode:input.applyCode,...(input.collegeKey!==undefined?{collegeKey:input.collegeKey}:{})})};
  }
  async readCismReport(date:string):Promise<CismReport>{
    if(!this.client.readCismReport)throw Error('cism_source_port_unavailable');await this.#authenticate();
    return this.client.readCismReport({teacherLabel:this.admission.teacherLabel,date});
  }
  async applyCismField(input:{date:string;fieldRef:string;before:string;after:string;wireSha256:string;completeExistingNarrative?:boolean},authorize:()=>Promise<boolean>){
    if(!this.client.applyCismField)throw Error('cism_source_port_unavailable');await this.#authenticate();
    return this.client.applyCismField({...input,teacherLabel:this.admission.teacherLabel},authorize,async value=>{
      const root=nativeOwnerArtifactsRoot+'cism-weekly/http-ack';mkdirSync(root,{recursive:true,mode:0o700});
      const st=lstatSync(root);if(!st.isDirectory()||st.isSymbolicLink()||(st.mode&0o077)!==0||st.uid!==process.getuid?.()||realpathSync(root)!==root)throw Error('cism_store_unqualified');
      const fd=openSync(resolve(root,input.date+'-'+input.fieldRef+'-'+randomUUID()+'.json'),'wx',0o600);
      try{writeSync(fd,JSON.stringify({date:input.date,fieldRef:input.fieldRef,contentType:value.contentType,sha256:value.sha256,rawBodyBase64:Buffer.from(value.raw).toString('base64')})+'\n');fsyncSync(fd);}finally{closeSync(fd);}
      const directory=openSync(root,'r');try{fsyncSync(directory);}finally{closeSync(directory);}
    });
  }
  async readTextbookCatalog(schoolLevel:TextbookSchoolLevel='middle'):Promise<TextbookCatalogObservation>{
    if(!this.client.readTextbookCatalog)throw Error('textbook_catalog_source_port_unavailable');
    await this.#authenticate();
    return this.client.readTextbookCatalog({teacherLabel:this.admission.teacherLabel,schoolLevel});
  }
  /** `getTestExamList` of one paper (a read) for academy_paper_print_pdf_get. */
  async readPaperExamList(paperNo:string):Promise<PaperExamRow[]>{
    if(!this.client.readPaperExamList)throw Error('paper_print_source_port_unavailable');
    await this.#authenticate();
    return this.client.readPaperExamList(paperNo);
  }
  /** One print archive of the static storage: a GET with no Academy credential, so no session is opened for it. */
  async downloadPrintArchive(kind:PrintArchiveKind,key:number):Promise<Uint8Array>{
    if(!this.client.downloadPrintArchive)throw Error('paper_print_source_port_unavailable');
    return this.client.downloadPrintArchive(kind,key);
  }
  async readTextbookPdf(schoolLevel:TextbookSchoolLevel,documentRef:string):Promise<TextbookPdfObservation>{
    if(!/^[a-f0-9]{64}$/.test(documentRef))throw Error('textbook_pdf_input_rejected');
    if(!this.client.readTextbookPdf)throw Error('textbook_pdf_source_port_unavailable');
    await this.#authenticate();
    return this.client.readTextbookPdf({teacherLabel:this.admission.teacherLabel,schoolLevel,documentRef});
  }
  /** Exact textbook section grading through the owner client's textbook
   * family. Titles are the source's own names and every returned record is
   * re-bound to the request. */
  async readTextbookSections(input:Readonly<TextbookSectionResults>):Promise<TextbookSectionObservation>{
    return this.#textbookSections(await this.#studentByName(input.displayName),input);
  }
  async #textbookSections(student:{studentId:string;displayName:string},input:Readonly<TextbookSectionResults>):Promise<TextbookSectionObservation>{
    await this.#authenticate();
    const raw=await this.client.execute({family:'textbook',teacherLabel:this.admission.teacherLabel,studentKey:student.studentId,
      studentName:student.displayName,courseKey:input.courseKey,bookNo:input.bookNo,sourceTitles:[...input.sourceTitles],gradeViewAdmission:true}) as unknown as {
        family:string;status:string;records:{sourceTitle:string;studentKey:string;studentName:string;courseKey:string;bookNo:string;
          score:number;items:{originalOrdinal:string;lectureKey?:string|number|null;outerLectureKey?:string|number|null;scoring:string}[];sourceDate:{literal:string|null;precision:string;meaning:string}}[];observedAt:string;coverage:string};
    if(raw.family!=='textbook'||raw.status!=='observed'||!Array.isArray(raw.records)||raw.records.length!==input.sourceTitles.length)
      throw Error('textbook_section_result_unqualified');
    const records=raw.records.map(r=>{
      if(r.studentKey!==student.studentId||r.studentName!==student.displayName||r.courseKey!==input.courseKey||r.bookNo!==input.bookNo
        ||!input.sourceTitles.includes(r.sourceTitle)||typeof r.score!=='number'||!Array.isArray(r.items)
        ||r.items.some(i=>i.scoring!=='O'&&i.scoring!=='X'))throw Error('textbook_section_result_unqualified');
      const items=r.items.map(i=>({ordinal:String(i.originalOrdinal),lectureKey:i.lectureKey==null?null:String(i.lectureKey),outerLectureKey:i.outerLectureKey==null?null:String(i.outerLectureKey),scoring:i.scoring as 'O'|'X'}));
      return {sourceTitle:r.sourceTitle,score:r.score,correctCount:items.filter(i=>i.scoring==='O').length,itemCount:items.length,items,
        sourceDate:{literal:r.sourceDate?.literal??null,precision:String(r.sourceDate?.precision??'unknown'),meaning:String(r.sourceDate?.meaning??'source_apply_date_not_exact_grade_time')}};
    });
    return {student:{studentId:student.studentId,displayName:student.displayName},courseKey:input.courseKey,bookNo:input.bookNo,
      records,observedAt:raw.observedAt,coverage:'exact_selected_existing_records_not_whole_book'};
  }
  /** Prestudy video read for one student. Source waiting/completed
   * list membership and summary AI literals are projected without video
   * URLs, and teacher judgment is always reported separately as null. */
  async readPrestudyByName(input:Readonly<PrestudyGet>):Promise<PrestudyObservation>{
    if(!this.client.readPrestudy)throw Error('prestudy_source_port_unavailable');
    const student=await this.#studentByName(input.displayName);
    await this.#authenticate();
    const raw=await this.client.readPrestudy({teacherLabel:this.admission.teacherLabel,studentName:student.displayName});
    const row=z.object({unitLabel:z.string().max(2000),dateLiteral:z.string().max(200),statusLiteral:z.string().max(2000),prestudyKey:key});
    const summaryRow=z.object({unitLabel:z.string().max(2000),dateLiteral:z.string().max(200),aiScoreLiteral:z.string().max(200),ratingLiteral:z.string().max(200),
      reportAvailable:z.boolean(),videoAvailable:z.boolean()});
    const waiting=z.array(row).max(1000).parse(raw.waiting),completed=z.array(row).max(1000).parse(raw.completed),
      summary=z.array(summaryRow).max(1000).parse(raw.summary);
    return {student:{studentId:student.studentId,displayName:student.displayName},waiting,completed,summary,
      teacherJudgment:null,observedAt:raw.observedAt,coverage:raw.coverage};
  }
  /** AI prestudy report for one exact record of one student. The
   * client binds the detail record to the student key before following
   * its analysis key. Report text stays in this return value only. */
  async readPrestudyReportByName(input:Readonly<PrestudyReportGet>):Promise<PrestudyReportObservation>{
    if(!this.client.readPrestudyReport)throw Error('prestudy_report_source_port_unavailable');
    const student=await this.#studentByName(input.displayName);
    await this.#authenticate();
    const raw=await this.client.readPrestudyReport({teacherLabel:this.admission.teacherLabel,studentName:student.displayName,studentKey:student.studentId,prestudyKey:input.prestudyKey});
    if(raw.prestudyKey!==input.prestudyKey)throw Error('prestudy_report_result_unqualified');
    const sourceRecord=z.object({prestudyKey:z.string().regex(/^[1-9][0-9]{0,15}$/),unitLabel:z.string().min(1).max(500).nullable(),dateLiteral:z.string().min(1).max(200).nullable()}).strict().parse(raw.sourceRecord);
    if(sourceRecord.prestudyKey!==input.prestudyKey)throw Error('prestudy_report_scope_conflict');
    const section=z.object({title:z.string().min(1).max(500),subtitle:z.string().max(1000).nullable(),
      blocks:z.array(z.object({heading:z.string().max(1000).nullable(),text:z.string().min(1).max(20000)})).min(1).max(60)});
    const sections=z.array(section).min(1).max(16).parse(raw.sections);
    const scores=z.array(z.number()).max(20).parse(raw.scores);
    const encodingResult=z.string().max(200).nullable().parse(raw.encodingResult);
    return {student:{studentId:student.studentId,displayName:student.displayName},prestudyKey:raw.prestudyKey,
      sourceRecord,sections,scores,encodingResult,observedAt:raw.observedAt,coverage:raw.coverage};
  }
  /** Backend half of the textbook-binding contract: a binding whose label
   * is not a qualified book title (percent/date/number/button cell) fails
   * closed here, so a parser drift never reaches the candidate list. */
  #assertTextbookBindings(bindings:{courseKey:string;bookNo:string;studentKey:string;bookLabel:string}[],studentId:string,allowEmpty=false):void{
    if(!bindings.length&&!allowEmpty)throw Error('textbook_binding_source_unqualified');
    const identities=new Set<string>();
    for(const b of bindings){
      if(b.studentKey!==studentId)throw Error('textbook_binding_scope_conflict');
      if(!isTextbookBookLabel(b.bookLabel))throw Error('textbook_binding_source_unqualified');
      const identity=JSON.stringify([b.courseKey,b.bookNo]);
      if(identities.has(identity))throw Error('textbook_binding_source_unqualified');
      identities.add(identity);
    }
  }
  /** Section grading/index with the binding discovered owner-side from the
   * live progress page. With several registered bindings and no exact
   * bookLabel the candidate labels are returned for caller selection; with
   * no titles the observed title index is returned for caller selection. */
  async readTextbookSectionsByName(input:Readonly<TextbookSectionResultsByNameInput>):Promise<TextbookSectionResultsByNameObservation>{
    if(!this.client.readTextbookBindings)throw Error('textbook_binding_source_port_unavailable');
    const student=await this.#studentByName(input.displayName);
    const fallback=input.allowHistoricalFallback===true;
    const explicit=fallback&&input.courseKey!==undefined&&input.bookNo!==undefined&&input.studentKey!==undefined
      ?{courseKey:input.courseKey,bookNo:input.bookNo,studentKey:input.studentKey}:null;
    // R4: an explicit student key is the resolved student's, before any fallback request.
    if(explicit&&explicit.studentKey!==student.studentId)throw Error('historical_key_unverified');
    await this.#authenticate();
    // Without a book label or explicit keys the live behaviour is unchanged, the fallback included.
    const historical=fallback&&(explicit!==null||input.bookLabel!==undefined);
    const bindings=await this.#liveBindings(student,historical);
    const choice=historical
      ?await this.#chooseBook(student,bindings,{bookLabel:input.bookLabel,explicit})
      :this.#chooseLiveBook(student,bindings,input.bookLabel);
    if(choice.kind==='interview')return this.#bookInterview(student,choice);
    if(choice.kind==='historical')return this.#historicalTextbook(student,choice,input.sourceTitles);
    const binding=choice.binding,bookSelection=choice.bookSelection;
    if(binding.studentKey!==student.studentId)throw Error('textbook_binding_scope_conflict');
    if(input.sourceTitles){
      return {...await this.#textbookSections(student,{displayName:input.displayName,purpose:input.purpose,currentRequestRef:input.currentRequestRef,
        courseKey:binding.courseKey,bookNo:binding.bookNo,sourceTitles:[...input.sourceTitles]}),...bookSelection};
    }
    if(!this.client.inspectTextbookIndex)throw Error('textbook_index_source_port_unavailable');
    const raw=await this.client.inspectTextbookIndex({family:'textbook',teacherLabel:this.admission.teacherLabel,studentKey:student.studentId,
      studentName:student.displayName,courseKey:binding.courseKey,bookNo:binding.bookNo,sourceTitles:['index'],gradeViewAdmission:true});
    const index=z.array(z.object({sourceTitle:z.string().min(1).max(2000),
      sourceDate:z.object({literal:z.string().nullable(),meaning:z.string().max(200)}).passthrough(),
      sources:z.array(z.string().max(64)).max(4).optional()}).passthrough()).max(1000).parse(raw.rows)
      .map(r=>({sourceTitle:r.sourceTitle,sourceDate:{literal:r.sourceDate.literal??null,meaning:String(r.sourceDate.meaning??'source_apply_date_not_exact_grade_time')},
        ...(r.sources?{sources:r.sources}:{})}));
    const indexBasis=typeof raw.indexBasis==='string'?raw.indexBasis:'wrong_answer_note_only';
    return {student:{studentId:student.studentId,displayName:student.displayName},courseKey:binding.courseKey,bookNo:binding.bookNo,
      bookLabel:binding.bookLabel,index,observedAt:new Date().toISOString(),indexBasis,...bookSelection,
      coverage:indexBasis==='wrong_answer_note_plus_student_paper_search'
        ?'wrong_answer_note_index_plus_one_observed_student_paper_search_page_for_exact_student_course_book_not_section_grading_not_complete_history'
        :'wrong_answer_note_title_index_only_for_exact_student_course_book_not_section_grading_not_complete_history'};
  }
  /** The live getTextBookProgress list. The LMS client refuses a page with no book row as
   * textbook_binding_source_unqualified, since such a page cannot prove that the student has no
   * book; for the direct-key fallback that page is a live list without the requested book
   * (R1, R5), and the historical keys are then verified by their own reads (R4). */
  async #liveBindings(student:{studentId:string;displayName:string},emptyAllowed:boolean):Promise<LiveBinding[]>{
    try{return await this.client.readTextbookBindings!({teacherLabel:this.admission.teacherLabel,studentName:student.displayName,studentKey:student.studentId});}
    catch(error){if(emptyAllowed&&error instanceof Error&&error.message==='textbook_binding_source_unqualified')return [];throw error;}
  }
  /** The live choice as before: the exact label wins, then a loose name (brand, grade-term,
   * volume) that agrees with one binding; several or none ask with every candidate. */
  #chooseLiveBook(student:{studentId:string},bindings:LiveBinding[],bookLabel:string|undefined):BookChoiceResult{
    this.#assertTextbookBindings(bindings,student.studentId);
    const choice=bookLabel?chooseBookBinding(bookLabel,bindings):null;
    const bookSelection=choice&&bookLabel?{bookSelection:bookSelectionOf(bookLabel,choice)}:{};
    if(choice?.kind==='interview'||(!choice&&bindings.length>1))
      return {kind:'interview',candidates:choice?.kind==='interview'?choice.candidates:bindings,hints:true,bookSelection};
    return {kind:'live',binding:choice?.kind==='selected'?choice.binding:bindings[0]!,bookSelection};
  }
  /** The direct-key fallback's choice (specs/academy-mcp/direct-key-lookup.md R1, R2, R5): the
   * live list first, an empty list counting as no match; then explicit keys, or the active
   * memory binding that the label selects. A label that matches neither keeps the live
   * candidates' interview when the live list has books, else refuses historical_binding_missing. */
  async #chooseBook(student:{studentId:string;displayName:string},bindings:LiveBinding[],request:{bookLabel?:string|undefined;explicit:{courseKey:string;bookNo:string}|null}):Promise<BookChoiceResult>{
    this.#assertTextbookBindings(bindings,student.studentId,true);
    const {bookLabel,explicit}=request;
    if(explicit){
      const live=bindings.find(b=>b.courseKey===explicit.courseKey&&b.bookNo===explicit.bookNo);
      if(live)return {kind:'live',binding:live,bookSelection:{}};
      return {kind:'historical',courseKey:explicit.courseKey,bookNo:explicit.bookNo,label:bookLabel??null,bookSelection:{}};
    }
    const label=bookLabel!;
    const choice=bindings.length?chooseBookBinding(label,bindings):null;
    if(choice?.kind==='selected')return {kind:'live',binding:choice.binding,bookSelection:{bookSelection:bookSelectionOf(label,choice)}};
    if(choice?.kind==='interview'&&choice.match!=='none')return {kind:'interview',candidates:choice.candidates,hints:true,bookSelection:{bookSelection:bookSelectionOf(label,choice)}};
    if(this.historicalBindings){
      const memory=await this.historicalBindings.activeBindings(student.displayName,student.studentId);
      const courseOf=new Map<string,string>();
      for(const b of memory){
        const seen=courseOf.get(b.bookNo);
        if(seen!==undefined&&seen!==b.courseKey)throw Error('historical_binding_ambiguous');
        courseOf.set(b.bookNo,b.courseKey);
      }
      const usable=memory.filter(b=>directKey(b.bookNo)&&directKey(b.courseKey)&&b.bookLabel);
      const matched=chooseBookBinding(label,usable);
      const found=matched.kind==='selected'?[matched.binding]:matched.match==='several'?matched.candidates:[];
      const distinct=[...new Map(found.map(b=>[b.bookNo,b] as const)).values()];
      if(distinct.length===1){
        const b=distinct[0]!;
        return {kind:'historical',courseKey:b.courseKey,bookNo:b.bookNo,label:b.bookLabel,
          bookSelection:{bookSelection:{requested:label,boundLabel:b.bookLabel,match:matched.kind==='selected'?matched.match:'loose'}}};
      }
      if(distinct.length>1)return {kind:'interview',candidates:distinct.map(b=>({bookLabel:b.bookLabel,bookNo:b.bookNo})),hints:false,
        bookSelection:{bookSelection:{requested:label,match:'several'}}};
    }
    if(bindings.length)return {kind:'interview',candidates:bindings,hints:true,bookSelection:{bookSelection:bookSelectionOf(label,choice!)}};
    throw Error('historical_binding_missing');
  }
  async #bookInterview(student:{studentId:string;displayName:string},choice:Extract<BookChoiceResult,{kind:'interview'}>):Promise<TextbookSectionResultsByNameObservation>{
    // Source bookLabel cells are not always titles (a progress row can
    // yield a percent literal), so attach each live binding's first observed
    // section title as a selection hint. The hint never selects; the
    // caller selects with a bookLabel on the second call.
    const candidates:{bookLabel:string;bookNo:string;hint?:string}[]=[];
    for(const b of choice.candidates){
      let hint:string|undefined;
      if(choice.hints&&b.courseKey!==undefined&&this.client.inspectTextbookIndex){
        try{
          const raw=await this.client.inspectTextbookIndex({family:'textbook',teacherLabel:this.admission.teacherLabel,studentKey:student.studentId,
            studentName:student.displayName,courseKey:b.courseKey,bookNo:b.bookNo,sourceTitles:['index'],gradeViewAdmission:true});
          const rows=z.array(z.object({sourceTitle:z.string().min(1).max(2000)}).passthrough()).max(1000).parse(raw.rows);
          hint=rows[0]?.sourceTitle;
        }catch{hint=undefined;}
      }
      candidates.push(hint?{bookLabel:b.bookLabel,bookNo:b.bookNo,hint}:{bookLabel:b.bookLabel,bookNo:b.bookNo});
    }
    return {kind:'interview_required',student:{studentId:student.studentId,displayName:student.displayName},
      bindings:candidates,reason:'multiple_book_bindings_select_one',observedAt:new Date().toISOString(),...choice.bookSelection};
  }
  /** The keys of a historical book, verified by the course-book report (R4): SERVER_RET = OK,
   * the student's name and the echoed keys (checked by the client), and a book title that
   * agrees with the known label. */
  async #verifiedCourseBookReport(student:{studentId:string;displayName:string},keys:{courseKey:string;bookNo:string},label:string|null):Promise<CourseBookReport>{
    if(!this.client.readCourseBookReport)throw Error('historical_source_port_unavailable');
    let report:CourseBookReport;
    // A report that fails, of any kind, verifies nothing; a login or session page stays authentication_required (R11).
    try{report=await this.client.readCourseBookReport({teacherLabel:this.admission.teacherLabel,studentKey:student.studentId,studentName:student.displayName,
      courseKey:keys.courseKey,bookNo:keys.bookNo});}
    catch(error){if(error instanceof Error&&error.message==='authentication_required')throw error;throw Error('historical_key_unverified');}
    if(!report?.metadata?.bookTitle||report.studyqInput?.student!==student.displayName||(label!==null&&!bookTitleAgrees(report.metadata.bookTitle,label)))
      throw Error('historical_key_unverified');
    return report;
  }
  /** R2-R4: the wrong-answer note and the course-book report of the historical keys, then, for
   * sourceTitles, the single-user result of each verified wrong-note attempt. */
  async #historicalTextbook(student:{studentId:string;displayName:string},choice:Extract<BookChoiceResult,{kind:'historical'}>,sourceTitles:readonly string[]|undefined):Promise<TextbookSectionResultsByNameObservation>{
    if(!this.client.readHistoricalTextbookIndex||!this.client.readCourseBookReport)throw Error('historical_source_port_unavailable');
    const keys={teacherLabel:this.admission.teacherLabel,studentKey:student.studentId,studentName:student.displayName,courseKey:choice.courseKey,bookNo:choice.bookNo};
    const note=await this.client.readHistoricalTextbookIndex(keys);
    const report=await this.#verifiedCourseBookReport(student,choice,choice.label);
    const rows=z.array(z.object({sourceTitle:z.string().min(1).max(2000),paperKey:key,attemptKey:key,
      sourceDate:z.object({literal:z.string().nullable(),meaning:z.string().max(200)})})).max(1000).parse(note.rows);
    const studentOut={studentId:student.studentId,displayName:student.displayName};
    if(sourceTitles){
      if(!this.client.readHistoricalTextbookSections)throw Error('historical_source_port_unavailable');
      const attempts=sourceTitles.map(title=>{
        const matches=rows.filter(r=>r.sourceTitle===title);
        // A title absent from the wrong-note index is not observed; no score is invented.
        if(matches.length!==1)throw Error(matches.length===0?TEXTBOOK_SECTION_NOT_IN_EITHER:'source_selection_missing_or_ambiguous');
        const r=matches[0]!;
        return {sourceTitle:r.sourceTitle,paperKey:r.paperKey,attemptKey:r.attemptKey,sourceDateLiteral:r.sourceDate.literal};
      });
      const raw=await this.client.readHistoricalTextbookSections({...keys,attempts});
      if(raw.family!=='textbook'||raw.status!=='observed'||!Array.isArray(raw.records)||raw.records.length!==sourceTitles.length)throw Error('textbook_section_result_unqualified');
      const records=raw.records.map(r=>{
        if(r.studentKey!==student.studentId||r.studentName!==student.displayName||r.courseKey!==choice.courseKey||r.bookNo!==choice.bookNo
          ||!sourceTitles.includes(r.sourceTitle)||typeof r.score!=='number'||!Array.isArray(r.items)
          ||r.items.some(i=>i.scoring!=='O'&&i.scoring!=='X'))throw Error('textbook_section_result_unqualified');
        const items=r.items.map(i=>({ordinal:String(i.originalOrdinal),lectureKey:i.lectureKey==null?null:String(i.lectureKey),outerLectureKey:i.outerLectureKey==null?null:String(i.outerLectureKey),scoring:i.scoring as 'O'|'X'}));
        return {sourceTitle:r.sourceTitle,score:r.score,correctCount:items.filter(i=>i.scoring==='O').length,itemCount:items.length,items,
          sourceDate:{literal:r.sourceDate?.literal??null,precision:String(r.sourceDate?.precision??'unknown'),meaning:String(r.sourceDate?.meaning??'source_apply_date_not_exact_grade_time')}};
      });
      return {student:studentOut,courseKey:choice.courseKey,bookNo:choice.bookNo,records,observedAt:raw.observedAt,
        coverage:'exact_selected_existing_records_not_whole_book',bindingSource:'historical_key_registry',...choice.bookSelection};
    }
    const reportSummary:HistoricalReportSummary={bookTitle:report.metadata.bookTitle,period:report.studyqInput.period,
      myAverage:report.studyqInput.my_average,allAverage:report.studyqInput.all_average,chapterCount:report.sourceChapterCount};
    return {student:studentOut,courseKey:choice.courseKey,bookNo:choice.bookNo,bookLabel:choice.label??report.metadata.bookTitle,
      index:rows.map(r=>({sourceTitle:r.sourceTitle,sourceDate:{literal:r.sourceDate.literal,meaning:r.sourceDate.meaning}})),
      observedAt:note.observedAt,indexBasis:'wrong_answer_note_only',bindingSource:'historical_key_registry',reportSummary,...choice.bookSelection,
      coverage:'historical_wrong_answer_note_and_course_book_report_only_all_correct_sections_omitted_from_wrong_note_index'};
  }
  /** The official course-book report of one student's book for academy_official_report_pdf_get:
   * explicit keys (all three, the student key the resolved student's), else the book the label
   * selects, live list first and active memory bindings after it, as the section read chooses. */
  async readCourseBookReportByName(input:{displayName:string;bookLabel?:string;courseKey?:string;bookNo?:string;studentKey?:string}):Promise<
    {kind:'report';student:{studentId:string;displayName:string};courseKey:string;bookNo:string;bindingSource?:'historical_key_registry';report:CourseBookReport;bookSelection?:BookSelection}
    |Extract<TextbookSectionResultsByNameObservation,{kind:'interview_required'}>>{
    if(!this.client.readTextbookBindings)throw Error('textbook_binding_source_port_unavailable');
    const student=await this.#studentByName(input.displayName);
    const explicit=input.courseKey!==undefined&&input.bookNo!==undefined&&input.studentKey!==undefined
      ?{courseKey:input.courseKey,bookNo:input.bookNo,studentKey:input.studentKey}:null;
    if(explicit&&explicit.studentKey!==student.studentId)throw Error('historical_key_unverified');
    await this.#authenticate();
    const bindings=await this.#liveBindings(student,explicit!==null||input.bookLabel!==undefined);
    const choice=explicit||input.bookLabel?await this.#chooseBook(student,bindings,{bookLabel:input.bookLabel,explicit}):this.#chooseLiveBook(student,bindings,undefined);
    if(choice.kind==='interview')return await this.#bookInterview(student,{...choice,hints:false}) as Extract<TextbookSectionResultsByNameObservation,{kind:'interview_required'}>;
    const keys=choice.kind==='live'?{courseKey:choice.binding.courseKey,bookNo:choice.binding.bookNo}:{courseKey:choice.courseKey,bookNo:choice.bookNo};
    const label=choice.kind==='live'?choice.binding.bookLabel:choice.label;
    const report=await this.#verifiedCourseBookReport(student,keys,label);
    return {kind:'report',student:{studentId:student.studentId,displayName:student.displayName},...keys,
      ...(choice.kind==='historical'?{bindingSource:'historical_key_registry' as const}:{}),report,...choice.bookSelection};
  }
  /** Reverse trace: scan one registered textbook's section index and match
   * item labels to original section items. With several bindings and no exact
   * bookLabel the candidate labels are returned for caller selection before
   * any per-book read, as the section results read does (without its per-book
   * hints): one call never scans every book. A scan cap above 64 titles fails
   * instead of a partial guess. Generated clinic membership is never asserted. */
  async readItemSectionTrace(input:Readonly<ItemSectionTrace>):Promise<ItemSectionTraceObservation>{
    if(!this.client.readTextbookBindings||!this.client.inspectTextbookIndex)throw Error('textbook_binding_source_port_unavailable');
    const student=await this.#studentByName(input.displayName);
    await this.#authenticate();
    const bindings=await this.client.readTextbookBindings({teacherLabel:this.admission.teacherLabel,studentName:student.displayName,studentKey:student.studentId});
    this.#assertTextbookBindings(bindings,student.studentId);
    const choice=input.bookLabel?chooseBookBinding(input.bookLabel,bindings):null;
    const bookSelection=choice&&input.bookLabel?{bookSelection:bookSelectionOf(input.bookLabel,choice)}:{};
    const selected=choice?.kind==='selected'?[choice.binding]:choice?choice.candidates:bindings;
    if(choice?.kind==='interview'||selected.length>1)return {kind:'interview_required',student:{studentId:student.studentId,displayName:student.displayName},
      bindings:selected.map(b=>({bookLabel:b.bookLabel,bookNo:b.bookNo})),reason:'multiple_book_bindings_select_one',observedAt:new Date().toISOString(),...bookSelection};
    const indexes: {binding:(typeof bindings)[number];titles:string[]}[]=[];
    let totalTitles=0;
    for(const binding of selected){
      const raw=await this.client.inspectTextbookIndex({family:'textbook',teacherLabel:this.admission.teacherLabel,studentKey:student.studentId,
        studentName:student.displayName,courseKey:binding.courseKey,bookNo:binding.bookNo,sourceTitles:['index'],gradeViewAdmission:true});
      const titles=z.array(z.object({sourceTitle:z.string().min(1).max(2000)}).passthrough()).max(1000).parse(raw.rows).map(r=>r.sourceTitle);
      indexes.push({binding,titles});totalTitles+=titles.length;
    }
    if(totalTitles>64)throw Error('textbook_section_scan_limit');
    const wanted=new Set(input.labels);
    const matches=new Map<string,ItemSectionTraceObservation extends never?never:any[]>();
    const matchedSections=new Map<string,Set<string>>();
    const books:{bookLabel:string;sectionsScanned:number;sectionsFailed:number}[]=[];
    let sectionsScanned=0,sectionsFailed=0;
    for(const {binding,titles} of indexes){
      let scanned=0,failed=0;
      for(const title of titles){
        scanned++;
        try{
          const raw=await this.client.execute({family:'textbook',teacherLabel:this.admission.teacherLabel,studentKey:student.studentId,
            studentName:student.displayName,courseKey:binding.courseKey,bookNo:binding.bookNo,sourceTitles:[title],gradeViewAdmission:true}) as unknown as {
              family:string;status:string;records:{sourceTitle:string;studentKey:string;studentName:string;courseKey:string;bookNo:string;
                score:number;items:{originalOrdinal:string;lectureKey?:string|number|null;outerLectureKey?:string|number|null;scoring:string}[];sourceDate:{literal:string|null;precision:string;meaning:string}}[];observedAt:string;coverage:string};
          if(raw.family!=='textbook'||raw.status!=='observed'||!Array.isArray(raw.records)||raw.records.length!==1)throw Error('textbook_section_result_unqualified');
          const r=raw.records[0]!;
          if(r.studentKey!==student.studentId||r.studentName!==student.displayName||r.courseKey!==binding.courseKey||r.bookNo!==binding.bookNo||r.sourceTitle!==title
            ||!Array.isArray(r.items)||r.items.some(i=>i.scoring!=='O'&&i.scoring!=='X'))throw Error('textbook_section_result_unqualified');
          for(const item of r.items){
            const lk=item.lectureKey==null?null:String(item.lectureKey),ok=item.outerLectureKey==null?null:String(item.outerLectureKey);
            for(const [label,by] of [[lk,'lectureKey'],[ok,'outerLectureKey']] as const){
              if(label&&wanted.has(label)){
                const list=matches.get(label)??[];list.push({bookLabel:binding.bookLabel,sourceTitle:title,ordinal:String(item.originalOrdinal),matchedBy:by,originalScoring:item.scoring});
                matches.set(label,list);
                const identities=matchedSections.get(label)??new Set<string>();
                identities.add(JSON.stringify([binding.courseKey,binding.bookNo,title]));
                matchedSections.set(label,identities);
              }
            }
          }
        }catch(error){
          // A failed source read stops dependent scans; never conceal authentication/contract failure as an unmatched label.
          throw error;
        }
      }
      books.push({bookLabel:binding.bookLabel,sectionsScanned:scanned,sectionsFailed:failed});sectionsScanned+=scanned;sectionsFailed+=failed;
    }
    const items=input.labels.map(label=>({label,matches:matches.get(label)??[]}));
    return {kind:'traced',student:{studentId:student.studentId,displayName:student.displayName},books,items,
      unmatchedLabels:items.filter(i=>!i.matches.length).map(i=>i.label),
      ambiguousLabels:items.filter(i=>(matchedSections.get(i.label)?.size??0)>1).map(i=>i.label),
      sectionsScanned,sectionsFailed,observedAt:new Date().toISOString(),
      coverage:'live_section_scan_original_attempt_lineage_not_generated_clinic_membership',...bookSelection};
  }
  saveConfigured():boolean{return this.saveApproval!==null&&typeof this.client.applyJournalAction==='function';}
  /** The owner client can write; authority comes from the per-request approval the owner workflow supplies. */
  requestBoundSaveConfigured():boolean{return typeof this.client.applyJournalAction==='function';}
  /** One serialized write batch on the already-authenticated owner client.
   * The client re-reads the exact before-state, consumes the launch-pinned
   * admission once per field, writes once and reads back; the first
   * non-settled field stops the batch with the rest not attempted. */
  async saveDailyRecord(scope:Readonly<DailyRecordScope>,patches:readonly {field:DailyRecordField;value:string;before:string;wireDigest:string}[],requestApproval?:DailyRecordActionApproval,subject?:{displayName:string}){
    const approval=requestApproval??this.saveApproval;
    if(!approval||typeof this.client.applyJournalAction!=='function')throw Error('dailyrecord_save_backend_not_configured');
    // A student outside the launch list is saved by the name recorded at intake;
    // the row identity check below still binds name to key.
    const student=this.admission.students.find(s=>s.studentId===scope.studentId)
      ??(requestApproval&&subject?{studentId:scope.studentId,displayName:subject.displayName}:undefined);
    if(scope.sourceNamespace!=='academy.lms.dayrecord'||scope.teacherId!==this.admission.teacherId
      ||!dailyRecordDateSchema.safeParse(scope.lessonDate).success||!student)throw Error('native_dailyrecord_scope_refused');
    await this.#authenticate();
    const selector=await this.client.discoverDayRecordGroups({teacherLabel:this.admission.teacherLabel,date:scope.lessonDate});
    const result=await this.client.execute({family:'day_record',teacherLabel:this.admission.teacherLabel,date:scope.lessonDate,
      groupIds:selector.groups.map(g=>g.groupId),studentNames:[student.displayName]});
    if(result.dayRecords.length!==1)throw Error('native_dailyrecord_occurrence_unverified');
    const row=result.dayRecords[0]!;
    if(row.studentKey!==scope.studentId||row.studentName!==student.displayName||row.teacherKey!==scope.teacherId||row.date!==scope.lessonDate)
      throw Error('native_dailyrecord_identity_conflict');
    const state=await this.client.journalState(targetOf(row));
    if(JSON.stringify(targetOf(state.target))!==JSON.stringify(targetOf(row)))throw Error('native_dailyrecord_state_conflict');
    // A first lesson has no previous record: a batch that assesses its homework is refused whole.
    if(state.previousRecordKey===null&&patches.some(p=>p.field==='previousHomeworkAssessment'))throw Error('dailyrecord_previous_record_absent');
    // Bind dispatch to the reviewed before-state: any drift refuses the
    // whole batch before a single write or admission consumption.
    if(patches.some(p=>p.wireDigest!==state.wireDigest||state.fields[p.field]!==p.before))
      return patches.map(p=>({field:p.field,status:'before_drift' as const,dispatched:false,transport:'not_invoked' as const}));
    const fields:{field:DailyRecordField;status:DailyRecordSaveFieldStatus;dispatched:boolean;transport:'returned'|'uncertain'|'not_invoked'}[]=[];
    for(let i=0;i<patches.length;i++){
      const patch=patches[i]!;
      const action={attemptId:randomUUID(),target:targetOf(row),field:patch.field,
        effectRecordKey:patch.field==='previousHomeworkAssessment'?state.previousRecordKey!:row.recordKey,
        before:patch.before,after:patch.value,wireDigest:state.wireDigest};
      let settled:{status:DailyRecordSaveFieldStatus;dispatched:boolean;transport:'returned'|'uncertain'|'not_invoked'};
      try{
        const result=await this.client.applyJournalAction!(action,approval);
        settled={status:result.status,dispatched:result.transport!=='not_invoked',transport:result.transport};
      }catch{
        // The client may have dispatched before throwing; report the field
        // as unresolved instead of denying a possible write upstream.
        settled={status:'unresolved',dispatched:true,transport:'uncertain'};
      }
      fields.push({field:patch.field,...settled});
      if(settled.status!=='persisted'&&settled.status!=='noop'){
        for(let j=i+1;j<patches.length;j++)fields.push({field:patches[j]!.field,status:'not_attempted',dispatched:false,transport:'not_invoked'});
        break;
      }
    }
    return fields;
  }
  /** The configured teacher that every DailyRecord row must carry. */
  teacherScope():DailyRecordTeacherScope{
    return {sourceNamespace:'academy.lms.dayrecord',teacherId:this.admission.teacherId};
  }
  #authenticate(){
    // One retained promise for every read family, including synchronous credential/auth failures.
    return this.#ready??=Promise.resolve().then(()=>this.client.authenticate(this.credential())).catch(error=>{throw new AcademyReadFailure('authentication','source_authentication_failed','src/native-dailyrecord-backend.ts:#authenticate',error);});
  }
  async readItemAnswers(labels:readonly string[]):Promise<unknown>{
    const input=itemAnswerLookupSchema.parse({labels:[...labels],purpose:'Owner-scoped item answer read',currentRequestRef:'native-item-answer'});
    await this.#authenticate();
    return this.client.execute({family:'item_answer',teacherLabel:this.admission.teacherLabel,teacherKey:this.admission.teacherId,
      lectureKeys:input.labels,answerReadAdmission:true});
  }
  async readStudentMetadata(value:Readonly<StudentMetadataBatch>):Promise<StudentMetadataObservation>{
    // The service fills request identity and resolves dates before this read.
    const input={...studentMetadataBatchSchema.parse(value),purpose:value.purpose,currentRequestRef:value.currentRequestRef},scope=input.scope;
    if(scope.teacherId!==this.admission.teacherId||!dailyRecordDateSchema.safeParse(scope.lessonDate).success)throw Error('native_metadata_scope_refused');
    // A key's name: the launch's pair, else this request's earlier source resolve.
    const resolved=this.#sourceResolved.get(input.currentRequestRef);
    const student=this.admission.students.find(s=>s.studentId===scope.studentId)
      ??(resolved&&resolved.studentId===scope.studentId&&Date.now()-resolved.at>=0&&Date.now()-resolved.at<=300000?{studentId:resolved.studentId,displayName:resolved.displayName}:undefined);
    if(!student)throw Error('native_student_name_unresolved');
    return this.#readMetadata(input,student);
  }
  async readStudentMetadataByName(value:Readonly<StudentMetadataGet>):Promise<StudentMetadataObservation>{
    const input={...studentMetadataGetSchema.parse(value),purpose:value.purpose,currentRequestRef:value.currentRequestRef};
    if(!dailyRecordDateSchema.safeParse(input.lessonDate).success)throw Error('native_dailyrecord_scope_refused');
    const configured=this.admission.students.find(s=>s.displayName===input.displayName);
    this.#requireMetadataMethods(input.queries);
    await this.#authenticate();
    const selector=await this.client.discoverDayRecordGroups({teacherLabel:this.admission.teacherLabel,date:input.lessonDate});
    const found=await this.client.execute({family:'day_record',teacherLabel:this.admission.teacherLabel,date:input.lessonDate,groupIds:selector.groups.map(g=>g.groupId),studentNames:[input.displayName]});
    if(!Array.isArray(found.dayRecords)||found.dayRecords.some(r=>r.teacherKey!==this.admission.teacherId||r.studentName!==input.displayName||r.date!==input.lessonDate))throw Error('native_dated_name_scope_conflict');
    if(found.dayRecords.length>1)throw Error('native_dated_name_ambiguous');
    if(found.dayRecords.length!==1)throw Error('native_dailyrecord_occurrence_unverified');
    const row=found.dayRecords[0]!,student={studentId:key.parse(row.studentKey),displayName:input.displayName};
    if(configured&&configured.studentId!==student.studentId)throw Error('native_dated_name_scope_conflict');
    // No caller proof or cross-process cache. This student is local to this batch.
    return this.#readMetadata({purpose:input.purpose,currentRequestRef:input.currentRequestRef,
      scope:{sourceNamespace:'academy.lms.metadata',teacherId:this.admission.teacherId,studentId:student.studentId,lessonDate:input.lessonDate},queries:input.queries},student);
  }
  #requireMetadataMethods(queries:StudentMetadataBatch['queries']){
    const methods={student_papers:'searchStudentPaperMetadata',daily_metadata:'searchDailyTestMetadata',daily_inventory:'inspectDailyTestInventory',daily_history:'readDailyTestHistory',printed_paper_metadata:'lookupPrintedPaperMetadata'} as const;
    for(const query of queries)if(typeof this.client[methods[query.kind]]!=='function')throw Error('native_metadata_method_unavailable');
  }
  async #readMetadata(input:StudentMetadataBatch,student:Admission['students'][number]):Promise<StudentMetadataObservation>{
    const scope=input.scope;
    this.#requireMetadataMethods(input.queries);
    await this.#authenticate();
    const target={teacherLabel:this.admission.teacherLabel,studentName:student.displayName,studentKey:student.studentId};
    const results:StudentMetadataObservation['results']=[];
    for(const query of input.queries){
      let observation:unknown;
      switch(query.kind){
        case 'student_papers':observation=await this.client.searchStudentPaperMetadata!({...target,period:query.period});break;
        case 'daily_metadata':observation=await this.client.searchDailyTestMetadata!(target);break;
        case 'daily_inventory':observation=await this.client.inspectDailyTestInventory!(target);break;
        case 'daily_history':observation=await this.client.readDailyTestHistory!({...target,periods:query.periods});break;
        case 'printed_paper_metadata':observation=await this.client.lookupPrintedPaperMetadata!({...target,paperCode:query.paperCode,period:query.period});break;
      }
      results.push({query,observation:projectStudentMetadata(query,observation,student)});
    }
    return {scope,results};
  }
  /** Name-only search feeding the service-layer resolver. Returns projected
   * `{studentKey, studentName}` pairs; exact-name rules (zero/multi/mismatch
   * unavailable, never a guess, no sequential probing) run in the service.
   * With a lesson date, the dated DailyRecord rows of the configured teacher;
   * otherwise the owner client's name search (name-only input, no key upfront).
   * The later intake still verifies key+name against the live wire. */
  async searchStudentsByName(exactName: string,requestRef?:string,lessonDate?:string): Promise<unknown>{
    if(requestRef)this.#sourceResolved.delete(requestRef);
    if(lessonDate){
      if(!dailyRecordDateSchema.safeParse(lessonDate).success)throw Error('native_dailyrecord_scope_refused');
      await this.#authenticate();
      const selector=await this.client.discoverDayRecordGroups({teacherLabel:this.admission.teacherLabel,date:lessonDate});
      const found=await this.client.execute({family:'day_record',teacherLabel:this.admission.teacherLabel,date:lessonDate,
        groupIds:selector.groups.map(g=>g.groupId),studentNames:[exactName]});
      if(Array.isArray(found.dayRecords)&&found.dayRecords.length>1)throw Error('native_dated_name_ambiguous');
      if(!Array.isArray(found.dayRecords)||found.dayRecords.some(row=>row.studentName!==exactName||row.date!==lessonDate||row.teacherKey!==this.admission.teacherId))
        throw Error('native_dated_name_scope_conflict');
      return this.#rememberResolved(found.dayRecords.map(row=>({studentKey:row.studentKey,studentName:row.studentName})),exactName,requestRef,lessonDate);
    }
    return this.#rememberResolved(await this.#wireSearch(exactName),exactName,requestRef);
  }
  /** The owner client's name-only search, projected to pairs. */
  async #wireSearch(exactName:string):Promise<unknown[]>{
    if(typeof this.client.searchStudentsByName!=='function')throw Error('student_resolve_backend_not_configured');
    // Name lookup uses the same owner session as subsequent source reads.
    // Retain an authentication failure rather than replaying a login.
    await this.#authenticate();
    const raw=await this.client.searchStudentsByName({teacherLabel:this.admission.teacherLabel,studentName:exactName});
    if(!Array.isArray(raw))throw Error('native_name_search_unqualified_no_automatic_retry');
    return raw.map((row)=>{
      if(typeof row!=='object'||row===null)throw Error('native_name_search_unqualified_no_automatic_retry');
      const pair=row as Record<string,unknown>;
      return {studentKey:pair.studentKey,studentName:pair.studentName,...(pair.fixedSourceFilter!==undefined?{fixedSourceFilter:pair.fixedSourceFilter}:{})};
    });
  }
  #rememberResolved(raw:unknown,exactName:string,requestRef?:string,lessonDate?:string){
    const pairs=nameSearchPairs.parse(raw);
    const unique=new Map(pairs.filter(row=>row.studentName===exactName).map(row=>[row.studentKey,row]));
    if(requestRef && pairs.every(row=>row.studentName===exactName) && unique.size===1){
      const row=[...unique.values()][0]!;
      this.#sourceResolved.set(requestRef,{studentId:row.studentKey,displayName:row.studentName,lessonDate,at:Date.now()});
      if(this.#sourceResolved.size>32)this.#sourceResolved.delete(this.#sourceResolved.keys().next().value!);
    }
    return pairs;
  }
  async readDailyRecord(scope:Readonly<DailyRecordScope>,requestRef?:string,subject?:{displayName:string}){
    const resolved=requestRef?this.#sourceResolved.get(requestRef):undefined;
    const sourceStudent=resolved && resolved.studentId===scope.studentId && (!resolved.lessonDate||resolved.lessonDate===scope.lessonDate) && Date.now()-resolved.at>=0 && Date.now()-resolved.at<=300000 ? resolved : undefined;
    // A later process re-selects a source-resolved student by the name recorded at
    // intake; the row identity check below still binds that name to the exact key.
    const recordedSubject=subject?{studentId:scope.studentId,displayName:subject.displayName}:undefined;
    const student=this.admission.students.find(s=>s.studentId===scope.studentId)??sourceStudent??recordedSubject;
    if(scope.sourceNamespace!=='academy.lms.dayrecord'||scope.teacherId!==this.admission.teacherId
      ||!dailyRecordDateSchema.safeParse(scope.lessonDate).success||!student)throw Error('native_dailyrecord_scope_refused');
    // Keep a failed authentication promise: subsequent calls must not retry login.
    await this.#authenticate();
    const selector=await this.client.discoverDayRecordGroups({teacherLabel:this.admission.teacherLabel,date:scope.lessonDate});
    const request={family:'day_record',teacherLabel:this.admission.teacherLabel,date:scope.lessonDate,
      groupIds:selector.groups.map(g=>g.groupId),studentNames:[student.displayName]};
    const result=await this.client.execute(request);
    if(result.dayRecords.length!==1)throw Error('native_dailyrecord_occurrence_unverified');
    const row=result.dayRecords[0]!;
    if(row.studentKey!==scope.studentId||row.studentName!==student.displayName||row.teacherKey!==scope.teacherId||row.date!==scope.lessonDate)
      throw Error('native_dailyrecord_identity_conflict');
    const state=await this.client.journalState(targetOf(row));
    if(JSON.stringify(targetOf(state.target))!==JSON.stringify(targetOf(row)))throw Error('native_dailyrecord_state_conflict');
    // A first lesson (no previous record key and date) reads no previous day record.
    const firstLesson=state.previousRecordKey===null&&state.previousDate===null;
    if(firstLesson?typeof state.fields.previousHomeworkAssessment==='string':state.previousRecordKey===null||state.previousDate===null)
      throw Error('native_dailyrecord_previous_conflict');
    let prior:Row|null=null;
    if(!firstLesson){
      const previous=await this.client.execute({...request,date:state.previousDate,groupIds:[row.groupId]});
      if(previous.dayRecords.length!==1)throw Error('native_dailyrecord_previous_unverified');
      prior=previous.dayRecords[0]!;
      if(prior.studentKey!==row.studentKey||prior.teacherKey!==row.teacherKey||prior.courseKey!==row.courseKey
        ||prior.membershipKey!==row.membershipKey||prior.recordKey!==state.previousRecordKey||prior.date!==state.previousDate)
        throw Error('native_dailyrecord_previous_conflict');
    }
    const observedAt=new Date().toISOString(),ref='native-dayrecord-read:'+randomUUID();
    const tone=prior?[prior.fields.progress,prior.fields.homework,prior.fields.lessonMemo].filter(Boolean).join('\n'):'';
    const observation=dailyRecordObservationSchema.parse({scope,target:observationTarget(row),previousTarget:prior?observationTarget(prior):null,observedAt,
      sourceRevision:createHash('sha256').update(this.admission.sourceBundleSha256+':'+state.wireDigest).digest('hex'),wireDigest:state.wireDigest,
      fields:Object.fromEntries(fields.map(field=>[field,typeof state.fields[field]==='string'
        ?{state:'known',value:state.fields[field],evidenceRefs:[ref+(field==='previousHomeworkAssessment'?':previous':':current')]}
        :{state:'unknown',reasonCode:field==='previousHomeworkAssessment'&&firstLesson?'first_lesson_no_previous_record':'native_field_not_observed'}])),
      toneSamples:tone&&prior?[{target:observationTarget(prior),observedAt,text:tone,evidenceRefs:[ref+':previous']}]:[],transmissionState:'unknown'});
    return {status:'observed' as const,observation,subject:{displayName:student.displayName}};
  }
}

/** Durable one-time consumption for save admissions: exclusive create + fsync
 * inside an owner-private directory; an existing record means already used. */
export function durableSaveConsumeOnce(storeDir:string){
  return async (key:string,record:Readonly<Record<string,unknown>>):Promise<boolean>=>{
    mkdirSync(storeDir,{recursive:true,mode:0o700});
    let fd:number;
    try{fd=openSync(resolve(storeDir,key+'.json'),'wx',0o600);}catch{return false;}
    try{writeSync(fd,JSON.stringify(record)+'\n');fsyncSync(fd);}finally{closeSync(fd);}
    return true;
  };
}
export async function nativeDailyRecordConfiguration(args:readonly string[],saveAdmission:DailyRecordSaveAdmission|null=null,saveStoreDir:string|null=null,saveRequestPath:string|null=null,historicalBindings:HistoricalBindingReader|null=null){
  if(!args.length)return null;
  if(args.length!==3||args[0]!=='--dailyrecord-native')throw Error('dailyrecord_native_arguments_invalid');
  if(process.getuid?.()!==10000||process.env.HERMES_HOME!==home)throw Error('native_dailyrecord_owner_required');
  const configPath=resolve(args[1]!),stat=lstatSync(configPath);
  if(!configPath.startsWith(home+'/artifacts/')||realpathSync(dirname(configPath))!==dirname(configPath)
    ||stat.uid!==10000||(stat.mode&0o077)!==0||!stat.isFile()||stat.isSymbolicLink())throw Error('native_dailyrecord_private_config_required');
  const pinned=await readPinnedLocalBytes(configPath,hash.parse(args[2]));
  const config=nativeDailyRecordConfigSchema.parse(JSON.parse(pinned.bytes.toString('utf8')));
  // Teacher, college and login of the owner file. No student list in it gates a read.
  const owner=JSON.parse(readFileSync(home+'/academy/owner.json','utf8'));
  const bundle=await readPinnedLocalBytes(config.sourceBundlePath,config.sourceBundleSha256);
  const jsdom='/opt/data/workspace/dc-gang-a/node_modules/jsdom/lib/api.js';
  const dependency=JSON.parse(readFileSync(resolve(jsdom,'../../package.json'),'utf8'));
  if(dependency.version!=='30.1.2')throw Error('native_dailyrecord_dependency_drift');
  const source=bundle.bytes.toString('utf8');
  if(source.split('from "jsdom"').length!==2)throw Error('native_dailyrecord_bundle_contract_rejected');
  // Import the exact verified bytes; never reopen a mutable source pathname for execution.
  const module=await import('data:text/javascript;base64,'+Buffer.from(source.replace('from "jsdom"','from '+JSON.stringify(pathToFileURL(jsdom).href))).toString('base64'));
  if(typeof module.AcademyHttpClient!=='function')throw Error('native_dailyrecord_client_required');
  const password=process.env.DC_ACADEMY_HTTP_PASSWORD;delete process.env.DC_ACADEMY_HTTP_PASSWORD;
  if(!password)throw Error('native_http_secret_required');
  const client=new module.AcademyHttpClient({teacherLabel:owner.teacherLabel,teacherKey:owner.teacherSourceKey,collegeKey:owner.collegeSourceKey}) as SourcePort;
  // Save stays fail-closed unless the launch pinned both an exact save
  // admission and its durable consumption store inside the owner home.
  let saveApproval:DailyRecordSaveApproval|null=null;
  if(saveAdmission||saveStoreDir||saveRequestPath){
    if(!saveAdmission||!saveStoreDir||!saveRequestPath||!saveStoreDir.startsWith(home+'/artifacts/')||!saveRequestPath.startsWith(home+'/artifacts/'))throw Error('dailyrecord_save_launch_incomplete');
    saveApproval=createDailyRecordSaveAdmission(saveAdmission,{
      readCurrentRequest:liveSaveRequestEvidence(saveRequestPath),
      consumeOnce:durableSaveConsumeOnce(saveStoreDir),
    });
  }
  const backend=new NativeDailyRecordBackend(client,{teacherId:owner.teacherSourceKey,teacherLabel:owner.teacherLabel,
    date:config.date,students:config.students,sourceBundleSha256:config.sourceBundleSha256},()=>({username:owner.sourceUsername,password}),null,null,null,saveApproval,historicalBindings);
  return {backend,close:()=>client.close()};
}
