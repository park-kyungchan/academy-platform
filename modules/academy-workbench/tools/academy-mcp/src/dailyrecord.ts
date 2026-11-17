import {randomUUID,createHash} from 'node:crypto';
import {z} from 'zod';
import {ASSESSMENT_CODE_TABLES,dailyRecordContextSchema,dailyRecordDateSchema,dailyRecordFieldSchema,dailyRecordFields,dailyRecordIdSchema,dailyRecordScopeSchema,dailyRecordWorkflowContext,type DailyRecordContext,type DailyRecordField,type DailyRecordScope} from './dailyrecord-context.ts';
import {freezeLabelData} from './item-label-context.ts';
import {itemAnswerLookupSchema,projectItemAnswerRead} from './item-answer-lookup.ts';
import {resolveStudentByName,studentResolveInputSchema} from './student-resolver.ts';
import {readDailyRecordByName,type DailyRecordTeacherScope} from './dailyrecord-read.ts';
import {readFailureResult} from './read-failure.ts';
import {toolResultBytes} from './tool-result.ts';
import {canonicalJson} from './dailyrecord-save-admission.ts';
import {flexibleContextSchema,factWarnings,requestFromInstruction,resolveProposal,resolveWriteContext,saveInstructionSchema,type ResolvedProposal} from './dailyrecord-write-input.ts';
import {attributionSchema,attributionSummary,consumedWithoutEffect,declaredActorSchema,executionKey,fieldExecutionState,needsReconciliation,studentExecutions,observeAttribution,requestBoundApproval,settledEffect,textDigest,unresolvedOnField,userRequestSchema,workflowLockKey,workflowRequestKey,canonicalSame,type DailyRecordActionApproval,type DailyRecordAttribution,type DailyRecordWorkflow} from './dailyrecord-workflow.ts';

const hash=z.string().regex(/^[a-f0-9]{64}$/),instant=z.iso.datetime({offset:true});
const knownValueSchema=z.discriminatedUnion('state',[
  z.object({state:z.literal('known'),value:z.string().max(8192),evidenceRefs:z.array(dailyRecordIdSchema).min(1).max(32)}).strict(),
  z.object({state:z.literal('unknown'),reasonCode:dailyRecordIdSchema}).strict(),
]);
const currentTargetSchema=z.object({sourceNamespace:dailyRecordIdSchema,teacherId:dailyRecordIdSchema,studentId:dailyRecordIdSchema,
  lessonDate:dailyRecordDateSchema,courseId:dailyRecordIdSchema,membershipId:dailyRecordIdSchema,recordId:dailyRecordIdSchema}).strict();
export const dailyRecordObservationSchema=z.object({scope:dailyRecordScopeSchema,target:currentTargetSchema,
  previousTarget:currentTargetSchema.nullable(),observedAt:instant,sourceRevision:hash,wireDigest:hash,
  fields:z.object({progress:knownValueSchema,homework:knownValueSchema,lessonMemo:knownValueSchema,attendance:knownValueSchema,dailyTestAssessment:knownValueSchema,previousHomeworkAssessment:knownValueSchema}).strict(),
  toneSamples:z.array(z.object({target:currentTargetSchema,observedAt:instant,text:z.string().max(8192),evidenceRefs:z.array(dailyRecordIdSchema).min(1).max(32)}).strict()).max(3),
  transmissionState:z.enum(['not_sent','already_sent','unknown']),
}).strict();
export type DailyRecordObservation=z.infer<typeof dailyRecordObservationSchema>;
/** Retired members: accepted when present, never read. */
const learningEntrySchema=z.object({bookRef:dailyRecordIdSchema,unitRef:dailyRecordIdSchema,scope:z.enum(['unit','sections']),sectionRefs:z.array(dailyRecordIdSchema).max(64),activity:z.string().min(1).max(400),deadline:dailyRecordDateSchema.nullable()}).strict();
/** Only field and value are required; factRefs, decision and its requestRef have defaults (dailyrecord-write-input.ts). */
const proposalSchema=z.object({field:dailyRecordFieldSchema,value:z.string().max(8192),factRefs:z.array(dailyRecordIdSchema).max(32).optional(),
  decision:z.object({kind:z.enum(['set','replace','clear']),requestRef:dailyRecordIdSchema.optional()}).strict().optional(),learningEntries:z.array(learningEntrySchema).max(64).optional(),
  teacherReported:z.object({preserveBefore:z.boolean(),mappedText:z.string().max(8192),lines:z.array(z.object({factRef:dailyRecordIdSchema,text:z.string().min(3).max(1000)}).strict()).min(1).max(32)}).strict().optional(),
}).strict();
export const dailyRecordIntakeSchema=z.object({context:flexibleContextSchema.required({scope:true}),suppliedValues:z.array(proposalSchema).max(6).default([]),actor:declaredActorSchema.optional(),attribution:attributionSchema.optional()}).strict();
export const dailyRecordReviewSchema=z.object({context:flexibleContextSchema.optional(),observationHandle:dailyRecordIdSchema.optional(),
  cismObservationHandle:dailyRecordIdSchema.optional(),
  proposals:z.array(proposalSchema).min(1).max(6),actor:declaredActorSchema.optional(),attribution:attributionSchema.optional(),
}).strict();
const cismSelectionSchema=z.object({reportId:dailyRecordIdSchema,periodStart:dailyRecordDateSchema,periodEnd:dailyRecordDateSchema,section:z.literal('learning_support')}).strict().refine(v=>v.periodStart<=v.periodEnd,'ordered_reporting_period_required');
export const cismSectionSchema=z.object({context:dailyRecordContextSchema,selection:cismSelectionSchema}).strict();
export const cismObservationSchema=z.object({scope:dailyRecordScopeSchema,selection:cismSelectionSchema,observedAt:instant,sourceRevision:hash,
  sourceReferenceDate:dailyRecordDateSchema,sectionLabel:z.literal('학습부진아 지도'),text:z.string().max(20000),evidenceRefs:z.array(dailyRecordIdSchema).min(1).max(32),
}).strict();
export type CismObservation=z.infer<typeof cismObservationSchema>;
type Proposal=ResolvedProposal<z.infer<typeof proposalSchema>>;
/** `subject` carries the source-verified display name so a later process can re-select the same student. */
type ReadResult<T>={status:'observed';observation:T;subject?:{displayName:string}}|{status:'unavailable';reasonCode:string};
const metadataPeriod=z.object({from:dailyRecordDateSchema,through:dailyRecordDateSchema}).strict().refine(v=>v.from<=v.through,'ordered_metadata_period_required');
export const studentMetadataQuerySchema=z.discriminatedUnion('kind',[
  z.object({kind:z.literal('student_papers'),period:metadataPeriod}).strict(),
  z.object({kind:z.literal('daily_metadata')}).strict(),
  z.object({kind:z.literal('daily_inventory')}).strict(),
  z.object({kind:z.literal('daily_history'),periods:z.array(z.enum(['11','12','13','14','15','16'])).min(1).max(6).refine(v=>new Set(v).size===v.length,'unique_metadata_selectors_required')}).strict(),
  z.object({kind:z.literal('printed_paper_metadata'),paperCode:z.string().regex(/^#[3456789ABCDEFGHJKLMNPQRSTUVWXYZ][23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{0,10}$/),period:metadataPeriod}).strict(),
]);
/** Separate read-only query transport: never a DailyRecord field, fact, approval or credential input. */
export const studentMetadataBatchSchema=z.object({purpose:dailyRecordIdSchema,currentRequestRef:dailyRecordIdSchema,
  scope:dailyRecordScopeSchema.extend({sourceNamespace:z.literal('academy.lms.metadata')}).strict(),
  queries:z.array(studentMetadataQuerySchema).min(1).max(8),
}).strict();
export type StudentMetadataBatch=z.infer<typeof studentMetadataBatchSchema>;
export const studentMetadataGetSchema=z.object({displayName:z.string().trim().min(1).max(200),lessonDate:dailyRecordDateSchema,
  purpose:dailyRecordIdSchema,currentRequestRef:dailyRecordIdSchema,queries:z.array(studentMetadataQuerySchema).min(1).max(8)}).strict();
export type StudentMetadataGet=z.infer<typeof studentMetadataGetSchema>;
export type StudentMetadataObservation={scope:StudentMetadataBatch['scope'];results:{query:StudentMetadataBatch['queries'][number];observation:unknown}[]};
const nativeKeySchema=z.string().regex(/^[1-9][0-9]{0,15}$/);
/** Exact textbook section grading read: source titles are the source's own
 * section/test titles for one student/course/book. Scores and per-item O/X
 * outcomes only; no clinic solving, teacher acceptance or print state. */
export const textbookSectionResultsSchema=z.object({displayName:z.string().trim().min(1).max(200),
  purpose:dailyRecordIdSchema,currentRequestRef:dailyRecordIdSchema,
  courseKey:nativeKeySchema,bookNo:nativeKeySchema,
  sourceTitles:z.array(z.string().trim().min(1).max(200)).min(1).max(12).refine(v=>new Set(v).size===v.length,'unique_source_titles_required')}).strict();
export type TextbookSectionResults=z.infer<typeof textbookSectionResultsSchema>;
export type TextbookSectionObservation={student:{studentId:string;displayName:string};courseKey:string;bookNo:string;
  records:{sourceTitle:string;score:number;correctCount:number;itemCount:number;items:{ordinal:string;lectureKey:string|null;outerLectureKey:string|null;scoring:'O'|'X'}[];sourceDate:{literal:string|null;precision:string;meaning:string}}[];observedAt:string;coverage:string};
/** Prestudy video read by exact student name. Upload (waiting list), completed
 * list membership, AI score/star literals and teacher review are four
 * distinct axes; teacherJudgment is never derived from a source OK/score. */
export const prestudyGetSchema=z.object({displayName:z.string().trim().min(1).max(200),
  purpose:dailyRecordIdSchema,currentRequestRef:dailyRecordIdSchema}).strict();
export type PrestudyGet=z.infer<typeof prestudyGetSchema>;
export type PrestudyObservation={student:{studentId:string;displayName:string};
  waiting:{unitLabel:string;dateLiteral:string;statusLiteral:string;prestudyKey:string}[];
  completed:{unitLabel:string;dateLiteral:string;statusLiteral:string;prestudyKey:string}[];
  summary:{unitLabel:string;dateLiteral:string;aiScoreLiteral:string;ratingLiteral:string;reportAvailable:boolean;videoAvailable:boolean}[];
  teacherJudgment:null;observedAt:string;coverage:string};
/** AI prestudy report read by exact student name and prestudy key. The
 * report text is student content for the teacher only: it is returned to
 * the caller and never stored. No transcript exists on the source pages;
 * the sections below are the LLM report itself. */
export const prestudyReportGetSchema=z.object({displayName:z.string().trim().min(1).max(200),
  prestudyKey:z.string().regex(/^[1-9][0-9]{0,15}$/),
  sectionTitles:z.array(z.string().min(1).max(500)).min(1).max(16).refine(v=>new Set(v).size===v.length,'unique_section_titles_required').optional(),
  maxOutputBytes:z.number().int().min(1024).max(512*1024).optional(),
  purpose:dailyRecordIdSchema,currentRequestRef:dailyRecordIdSchema}).strict();
export type PrestudyReportGet=z.infer<typeof prestudyReportGetSchema>;
export type PrestudyReportObservation={student:{studentId:string;displayName:string};prestudyKey:string;
  sourceRecord:{prestudyKey:string;unitLabel:string|null;dateLiteral:string|null};
  sections:{title:string;subtitle:string|null;blocks:{heading:string|null;text:string}[]}[];scores:number[];
  encodingResult:string|null;observedAt:string;coverage:string};
/** Textbook section grading resolved by exact student name: the owner
 * backend discovers the registered course/book binding from the live
 * progress page. Without sourceTitles it returns the observed title index. */
export const textbookSectionResultsGetSchema=z.object({displayName:z.string().trim().min(1).max(200),
  purpose:dailyRecordIdSchema,currentRequestRef:dailyRecordIdSchema,
  bookLabel:z.string().trim().min(1).max(200).optional(),
  sourceTitles:z.array(z.string().trim().min(1).max(200)).min(1).max(12).refine(v=>new Set(v).size===v.length,'unique_source_titles_required').optional(),
  view:z.enum(['full','compact']).optional()}).strict();
export type TextbookSectionResultsGet=z.infer<typeof textbookSectionResultsGetSchema>;
/** Clinic (wrong-answer/similar) bundle read by exact student name. Clinic
 * papers have no server score-input support, so this read can only bundle
 * the original section attempt (wrong items make a section clinic-eligible)
 * and the merged title index. Clinic generation, printing, hand-out, solving,
 * scoring and teacher review are never asserted from source rows: generation
 * stays 'unqualified' and the remaining axes are explicit teacher-judgment
 * slots. */
export const clinicPaperStatusGetSchema=textbookSectionResultsGetSchema;
export type ClinicPaperStatusGet=z.infer<typeof clinicPaperStatusGetSchema>;
export type ClinicPaperStatusObservation={student:{studentId:string;displayName:string};courseKey:string;bookNo:string;bookLabel:string;
  sections:{sourceTitle:string;originalAttempt:'observed_original_attempt'|'paper_row_observed'|'not_observed_in_scope';
    score:number|null;correctCount:number|null;itemCount:number|null;
    wrongItems:{ordinal:string;lectureKey:string|null;outerLectureKey:string|null}[]|null;
    clinicEligible:boolean|null;sourceDate:{literal:string|null;meaning:string}|{literal:string|null;precision:string;meaning:string}}[];
  generatedMembership:'unqualified';clinicGeneration:'unqualified';
  teacherJudgmentSlots:{printed:'teacher_report_required';handedOut:'teacher_report_required';solved:'teacher_judged_only_no_server_score_input';scored:'teacher_judged_only_no_server_score_input';teacherReviewed:'teacher_report_required'};
  observedAt:string;coverage:string;indexBasis?:string};
/** Reverse trace from numeric item labels to the original textbook sections
 * (activity titles) whose observed items carry those labels. This proves
 * original-attempt lineage only: a generated wrong-answer clinic paper's own
 * version, membership and printed order remain unqualified. */
export const itemSectionTraceSchema=z.object({displayName:z.string().trim().min(1).max(200),
  purpose:dailyRecordIdSchema,currentRequestRef:dailyRecordIdSchema,
  labels:z.array(z.string().regex(/^[1-9][0-9]{0,15}$/)).min(1).max(32).refine(v=>new Set(v).size===v.length,'unique_item_labels_required'),
  bookLabel:z.string().trim().min(1).max(200).optional()}).strict();
export type ItemSectionTrace=z.infer<typeof itemSectionTraceSchema>;
export type ItemSectionTraceObservation=
  |{kind:'interview_required';student:{studentId:string;displayName:string};bindings:{bookLabel:string;bookNo:string}[];reason:'multiple_book_bindings_select_one';observedAt:string}
  |{kind:'traced';student:{studentId:string;displayName:string};
    books:{bookLabel:string;sectionsScanned:number;sectionsFailed:number}[];
    items:{label:string;matches:{bookLabel:string;sourceTitle:string;ordinal:string;matchedBy:'lectureKey'|'outerLectureKey';originalScoring:'O'|'X'}[]}[];
    unmatchedLabels:string[];ambiguousLabels:string[];sectionsScanned:number;sectionsFailed:number;observedAt:string;coverage:string};
export type TextbookSectionIndexObservation={student:{studentId:string;displayName:string};courseKey:string;bookNo:string;bookLabel:string;
  index:{sourceTitle:string;sourceDate:{literal:string|null;meaning:string};sources?:string[]}[];observedAt:string;coverage:string;indexBasis?:string};
export type TextbookSectionInterviewObservation={kind:'interview_required';student:{studentId:string;displayName:string};bindings:{bookLabel:string;bookNo:string;hint?:string}[];reason:'multiple_book_bindings_select_one';observedAt:string};
export type TextbookSectionResultsByNameObservation=TextbookSectionObservation|TextbookSectionIndexObservation|TextbookSectionInterviewObservation;
/** Implemented by the native/source owner, never by MCP arguments. All methods are read-only. */
export interface DailyRecordReadBackend {
  teacherScope?():DailyRecordTeacherScope;
  /** `subject` is the source-verified name recorded at intake, letting a fresh process re-select a student outside the launch list. */
  readDailyRecord(scope:Readonly<DailyRecordScope>,requestRef:string,subject?:{displayName:string}):Promise<ReadResult<DailyRecordObservation>>;
  readStudentMetadataByName?(input:Readonly<StudentMetadataGet>):Promise<StudentMetadataObservation>;
  readStudentMetadata?(input:Readonly<StudentMetadataBatch>):Promise<StudentMetadataObservation>;
  readItemAnswers?(labels:readonly string[]):Promise<unknown>;
  readCismSection?(scope:Readonly<DailyRecordScope>,selection:Readonly<z.infer<typeof cismSelectionSchema>>):Promise<ReadResult<CismObservation>>;
  searchStudentsByName?(exactName:Readonly<string>,requestRef:string,lessonDate?:string):Promise<unknown>;
  readTextbookSections?(input:Readonly<TextbookSectionResults>):Promise<TextbookSectionObservation>;
  readPrestudyByName?(input:Readonly<PrestudyGet>):Promise<PrestudyObservation>;
  readPrestudyReportByName?(input:Readonly<PrestudyReportGet>):Promise<PrestudyReportObservation>;
  readTextbookSectionsByName?(input:Readonly<TextbookSectionResultsGet>):Promise<TextbookSectionResultsByNameObservation>;
  readItemSectionTrace?(input:Readonly<ItemSectionTrace>):Promise<ItemSectionTraceObservation>;
}
/** Minimal form {reviewDigest, instruction}: context, handle and proposals come from the recorded prepared review. */
export const dailyRecordSaveSchema=z.object({context:flexibleContextSchema.optional(),observationHandle:dailyRecordIdSchema.optional(),
  cismObservationHandle:dailyRecordIdSchema.optional(),
  reviewDigest:hash,proposals:z.array(proposalSchema).min(1).max(6).optional(),
  /** The user's verbatim instruction behind this save: provenance for audit, required by the owner workflow. */
  request:userRequestSchema.optional(),instruction:saveInstructionSchema.optional(),actor:declaredActorSchema.optional(),attribution:attributionSchema.optional()}).strict();
export const dailyRecordReconcileSchema=z.object({context:dailyRecordContextSchema,resolution:z.object({request:userRequestSchema,
  executions:z.array(z.object({requestRef:dailyRecordIdSchema,field:dailyRecordFieldSchema}).strict()).min(1).max(6)}).strict().optional(),
  actor:declaredActorSchema.optional(),attribution:attributionSchema.optional()}).strict();
export const dailyRecordExecutionStatusSchema=z.object({scope:dailyRecordScopeSchema,requestRef:dailyRecordIdSchema.optional(),actor:declaredActorSchema.optional(),attribution:attributionSchema.optional()}).strict();
/** Upstream DailyRecord workflow tools that receive bridge-observed attribution. */
export const dailyRecordWorkflowToolSchemas={academy_dailyrecord_intake:dailyRecordIntakeSchema,academy_dailyrecord_review:dailyRecordReviewSchema,
  academy_dailyrecord_save:dailyRecordSaveSchema,academy_dailyrecord_reconcile:dailyRecordReconcileSchema,academy_dailyrecord_execution_status:dailyRecordExecutionStatusSchema} as const;
export type DailyRecordWorkflowToolName=keyof typeof dailyRecordWorkflowToolSchemas;
export type DailyRecordSave=z.infer<typeof dailyRecordSaveSchema>;
export type DailyRecordSaveFieldStatus='persisted'|'noop'|'not_persisted'|'conflict'|'unresolved'|'not_attempted'|'before_drift';
/** Implemented by the native/source owner only, never from MCP arguments.
 * Each patch carries the reviewed before-value and wire digest; the backend
 * must refuse the whole batch before any write when its fresh before-state
 * differs. It then runs the launch-pinned save admission's one-time
 * consumption, a single write and an authoritative readback inside the
 * owner client. Uncertain dispatches resolve by later readback, never by
 * retrying inside this call, and are reported, never thrown past the point
 * of possible dispatch. */
export interface DailyRecordWriteBackend {
  /** Legacy launch-pinned admission path. */
  saveConfigured():boolean;
  /** Owner client write port is present; per-request approval is supplied by the owner workflow. */
  requestBoundSaveConfigured?():boolean;
  saveDailyRecord(scope:Readonly<DailyRecordScope>,patches:readonly {field:DailyRecordField;value:string;before:string;wireDigest:string}[],approval?:DailyRecordActionApproval,subject?:{displayName:string}):Promise<{field:DailyRecordField;status:DailyRecordSaveFieldStatus;dispatched:boolean;transport?:'returned'|'uncertain'|'not_invoked'}[]>;
}
/** Inject the canonical Education safeguard, together with its verified policy/source identity. */
export interface DailyRecordParentSafeguard {readonly policySha256:string;readonly safeguardSourceSha256:string;readonly sourceRef:string;check(text:string):boolean;checkLines(text:string):boolean}
const same=(a:unknown,b:unknown)=>JSON.stringify(a)===JSON.stringify(b);
const digest=(value:unknown)=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
const fieldValues=(observation:DailyRecordObservation)=>Object.fromEntries(dailyRecordFields.map(field=>{const value=observation.fields[field];return [field,value.state==='known'?{state:value.state,value:value.value}:{state:value.state,reasonCode:value.reasonCode}];}));
const scopeOf=(target:z.infer<typeof currentTargetSchema>):DailyRecordScope=>({sourceNamespace:target.sourceNamespace,teacherId:target.teacherId,studentId:target.studentId,lessonDate:target.lessonDate});
const safeReason=(value:string)=>readFailureResult(Error(value),'source_read_unavailable').reasonCode;
/** A requested title absent from BOTH observed sources (wrong-note index and
 * one student paper-search page) is a coverage-qualified non-observation,
 * never a claim that the student has no such record. */
export const TEXTBOOK_SECTION_NOT_IN_EITHER='textbook_section_not_in_either_observed_source';
export function textbookSectionNotInEither(base:Record<string,unknown>,displayName:string){
  return {...base,status:'not_observed_in_either_source' as const,reasonCode:TEXTBOOK_SECTION_NOT_IN_EITHER,displayName,
    coverage:'wrong_answer_note_index_plus_one_observed_student_paper_search_page_not_complete_history' as const,
    meaning:'requested_title_absent_from_both_observed_sources_only_not_proof_of_no_record' as const};
}
export function isTextbookSectionNotInEither(error:unknown){return error instanceof Error&&error.message===TEXTBOOK_SECTION_NOT_IN_EITHER;}
function validValue(field:DailyRecordField,value:string){
  if(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(value))return false;
  if(field==='attendance')return (ASSESSMENT_CODE_TABLES.attendance as readonly string[]).includes(value);
  if(field==='dailyTestAssessment')return (ASSESSMENT_CODE_TABLES.dailyTestAssessment as readonly string[]).includes(value);
  if(field==='previousHomeworkAssessment')return (ASSESSMENT_CODE_TABLES.previousHomeworkAssessment as readonly string[]).includes(value);
  return value.split('').reduce((n,c)=>n+(c.charCodeAt(0)>127?2:1),0)<=400;
}
function unknownAssessment(field:DailyRecordField,value:string){return (field==='dailyTestAssessment'||field==='previousHomeworkAssessment')&&value==='-1';}
function qualifiedObservation(value:unknown,scope:DailyRecordScope){
  const observation=dailyRecordObservationSchema.parse(value);
  if(!same(observation.scope,scope)||!same(scopeOf(observation.target),scope))throw Error('source_scope_conflict');
  const p=observation.previousTarget,t=observation.target;
  if(p&&(p.sourceNamespace!==t.sourceNamespace||p.teacherId!==t.teacherId||p.studentId!==t.studentId||p.courseId!==t.courseId||p.membershipId!==t.membershipId||p.recordId===t.recordId||p.lessonDate>=t.lessonDate))throw Error('previous_record_scope_conflict');
  if(!p&&observation.fields.previousHomeworkAssessment.state==='known')throw Error('previous_record_required');
  for(const field of dailyRecordFields){const v=observation.fields[field];if(v.state==='known'&&!validValue(field,v.value))throw Error('source_field_value_unqualified');}
  for(const sample of observation.toneSamples){if(sample.target.sourceNamespace!==t.sourceNamespace||sample.target.teacherId!==t.teacherId||sample.target.studentId!==t.studentId||sample.target.lessonDate>=t.lessonDate)throw Error('tone_sample_scope_conflict');}
  return freezeLabelData(observation);
}
const deadlinePattern=/(?:(\d{4})[-./](\d{1,2})[-./](\d{1,2})|(\d{1,2})\s*월\s*(\d{1,2})\s*일)/g;
type Deadline={year:number|null;month:number;day:number};
const deadlinesIn=(text:string):Deadline[]=>[...text.matchAll(deadlinePattern)].map(m=>m[1]?{year:Number(m[1]),month:Number(m[2]),day:Number(m[3])}:{year:null,month:Number(m[4]),day:Number(m[5])});
/** Every deadline the text states must be supported by the field's facts. A
 * month-day fact supports itself and the full date of the lesson year, or of
 * the next year when its month is earlier than the lesson month. A full-date
 * fact supports itself and its month-day form. */
function deadlineSupported(value:string,facts:DailyRecordContext['facts'],lessonDate:string){
  const full=new Set<string>(),monthDay=new Set<string>(),monthDayOfFull=new Set<string>();
  const add=(d:Deadline,boundToFullDate=false)=>{
    const md=d.month+'-'+d.day;
    if(d.year!==null){full.add(d.year+'-'+md);monthDayOfFull.add(md);}
    else (boundToFullDate?monthDayOfFull:monthDay).add(md);
  };
  for(const f of facts){
    for(const d of deadlinesIn(f.text))add(d);
    for(const d of f.deadlines){add({year:Number(d.date.slice(0,4)),month:Number(d.date.slice(5,7)),day:Number(d.date.slice(8,10))});for(const r of deadlinesIn(d.rendering))add(r,true);}
  }
  const lessonYear=Number(lessonDate.slice(0,4)),lessonMonth=Number(lessonDate.slice(5,7));
  return deadlinesIn(value).every(d=>{
    const md=d.month+'-'+d.day;
    if(d.year===null)return monthDay.has(md)||monthDayOfFull.has(md);
    return full.has(d.year+'-'+md)||(monthDay.has(md)&&(d.year===lessonYear||(d.year===lessonYear+1&&d.month<lessonMonth)));
  });
}
function proposalBlockers(p:Proposal,context:DailyRecordContext){
  const blockers:string[]=[];
  if(!context.selectedFields.includes(p.field))blockers.push('field_not_selected');
  if(p.decision.requestRef!==context.currentRequestRef)blockers.push('current_request_ref_required');
  if(!validValue(p.field,p.value))blockers.push('invalid_explicit_field_value');
  if(p.factRefs.some(ref=>!context.facts.some(f=>f.ref===ref&&f.field===p.field)))blockers.push('field_fact_reference_required');
  if(new Set(p.factRefs).size!==p.factRefs.length)blockers.push('duplicate_fact_ref');
  blockers.push(...codeFactBlockers(p,context));
  if(p.decision.kind==='clear'&&p.value!=='')blockers.push('clear_requires_empty_value');
  if(p.value===''&&p.decision.kind!=='clear'&&['progress','homework','lessonMemo'].includes(p.field))blockers.push('explicit_clear_decision_required');
  if(p.field==='lessonMemo'&&context.memoAnswer.kind==='no_addition')blockers.push('no_addition_preserves_existing_memo');
  if(p.field==='lessonMemo'&&context.memoAnswer.kind==='not_supplied')blockers.push('today_memo_interview_required');
  return blockers;
}
const codeFields:readonly DailyRecordField[]=['attendance','dailyTestAssessment','previousHomeworkAssessment'];
/** A code value the teacher's own facts contradict holds the row (TASK-FLEX-1R):
 * user-supplied facts of the field that give different explicit values, or one
 * whose explicit value differs from the proposed value. */
function codeFactBlockers(p:Proposal,context:DailyRecordContext){
  if(!codeFields.includes(p.field))return [];
  const values=new Set(context.facts.filter(f=>f.field===p.field&&f.origin==='user_supplied'&&f.explicitValue!==undefined).map(f=>f.explicitValue));
  return [...(values.size>1?['conflicting_explicit_user_source_options']:[]),...([...values].some(v=>v!==p.value)?['explicit_user_source_option_contradicts_value']:[])];
}
/** Checks of the text against the field's supplied facts. They are warnings,
 * never blockers. A code value no referenced fact gives is a warning even
 * without facts; the other checks run only when the field has facts. */
function factCheckCodes(p:Proposal,context:DailyRecordContext){
  const facts=context.facts.filter(f=>f.field===p.field),codes:string[]=[];
  if(codeFields.includes(p.field)&&!facts.some(f=>p.factRefs.includes(f.ref)&&f.origin==='user_supplied'&&f.explicitValue===p.value))
    codes.push('explicit_user_source_option_fact_required');
  if(!facts.length)return {codes,missingFragments:[] as string[]};
  const constraints=facts.flatMap(f=>[...f.requiredFragments,...f.questionCounts.map(c=>c.rendering),...f.deadlines.map(d=>d.rendering),...f.stageClaims.map(s=>s.rendering)]);
  if(facts.some(f=>f.questionCounts.some(c=>!new RegExp('(?<!\\d)'+c.count+'\\s*문항(?!\\d)').test(c.rendering))))codes.push('count_rendering_must_preserve_count');
  const missingFragments=constraints.filter(fragment=>!p.value.includes(fragment));
  if(missingFragments.length)codes.push('supplied_fact_fragment_missing');
  if(['progress','homework','lessonMemo'].includes(p.field)){
    const factText=facts.map(f=>f.text).join('\n');
    const counts=(text:string)=>[...text.matchAll(/(?<!\d)(\d{1,6})\s*문항/g)].map(m=>Number(m[1]));
    const supportedCounts=new Set([...counts(factText),...facts.flatMap(f=>f.questionCounts.filter(c=>new RegExp('(?<!\\d)'+c.count+'\\s*문항(?!\\d)').test(c.rendering)).map(c=>c.count))]);
    if(counts(p.value).some(count=>!supportedCounts.has(count)))codes.push('unsupported_question_count');
    if(!deadlineSupported(p.value,facts,context.scope.lessonDate))codes.push('unsupported_deadline');
    if(/완료/.test(p.value)&&!facts.some(f=>/완료/.test(f.text)||f.stageClaims.some(s=>s.status==='complete')))codes.push('unsupported_completion_claim');
    const completionPatterns={solving:/(?:문제\s*)?풀이\s*(?:까지\s*)?완료/,grading:/채점(?:·검사)?\s*(?:까지\s*)?완료/,
      correction:/(?:오답\s*(?:수정|정리|처리)|재풀이)\s*(?:까지\s*)?완료/,
      teacher_review:/(?:교사\s*확인|검토)\s*(?:까지\s*)?완료/};
    for(const [stage,pattern] of Object.entries(completionPatterns))if(pattern.test(p.value)){
      const claims=facts.flatMap(f=>f.stageClaims).filter(s=>s.stage===stage);
      if(claims.some(s=>s.status!=='complete'))codes.push('stage_completion_mismatch:'+stage);
      if(!claims.some(s=>s.status==='complete')&&!facts.some(f=>pattern.test(f.text)))codes.push('unsupported_stage_completion_claim:'+stage);
    }
  }
  return {codes,missingFragments};
}

/** Field-local next steps for held rows. Guidance names who can resolve a
 * hold; it never supplies a mapping, option or teacher decision. */
const holdActionTable:[RegExp,string,string][]=[
  [/^canonical_parent_safeguard_not_connected$/,'owner_configuration','The runtime owner must connect the pinned policy; a caller cannot resolve this.'],
  [/^(field_fact_reference_required|duplicate_fact_ref)$/,'supply_or_correct_fact','A factRef names no supplied fact of this field, or names one twice; correct factRefs or omit them.'],
  [/^(explicit_user_source_option_contradicts_value|conflicting_explicit_user_source_options)$/,'supply_or_correct_fact','The teacher\'s supplied facts give another code value for this field, or different ones; confirm the value with the teacher and correct the fact or the proposal.'],
  [/^(source_before_or_contract_drift|stale_source|qualified_before_source_unavailable|unknown_before|qualified_current_record_required|independent_previous_record_required|exact_fresh_cism_observation_handle_required|cism_fact_not_bound_to_exact_observed_section)$/,'fresh_intake','Run a fresh intake and review; the source changed or is not qualified.'],
  [/^(parent_hyphen_lines_required|parent_internal_language_excluded|invalid_explicit_field_value)$/,'rewrite_text','Rewrite the parent-facing field text and review again.'],
  [/^(explicit_clear_decision_required|clear_requires_empty_value|no_addition_preserves_existing_memo|today_memo_interview_required|field_not_selected|current_request_ref_required)$/,'explicit_teacher_decision','Obtain the explicit teacher decision for this exact field and request.'],
];
function holdActions(field:DailyRecordField,blockers:readonly string[]){
  return blockers.map(code=>{
    const match=holdActionTable.find(([pattern])=>pattern.test(code.split(':')[0]!));
    return {field,code,action:match?.[1]??'resolve_blocker',guidance:match?.[2]??null};
  });
}
type ReviewPolicyPins={parentSafeguard:{policySha256:string;safeguardSourceSha256:string}|null};
type ReviewedPatch={field:DailyRecordField;target:z.infer<typeof currentTargetSchema>|null;before:string|null;after:string};
type StoredReview={reviewDigest:string;status:string;scope:DailyRecordScope;requestRef:string;observationHandle:string|null;
  patch:{field:DailyRecordField;before:string;after:string;effectRecordKey:string}[];reviewPolicy:ReviewPolicyPins;reviewedBy:DailyRecordAttribution;reviewedAt:string;
  /** Digest of the exact context and proposals this review judged; a save must submit the same. */
  inputDigest:string;inputLength?:number;
  /** The judged input with its defaults filled in, so a save may give only the digest and the instruction. */
  input?:ReviewedInput};
type ReviewedInput={context:DailyRecordContext;observationHandle:string;cismObservationHandle:string|null;proposals:Proposal[]};
type ResolvedSave=Omit<DailyRecordSave,'context'|'observationHandle'|'proposals'>&{context:DailyRecordContext;observationHandle:string;proposals:Proposal[]};

/** A fresh source read must be at most 5 minutes old; a held observation handle is usable for 60 minutes. */
const FRESH_READ_MS=5*60_000,HELD_OBSERVATION_MS=60*60_000;

export class DailyRecordService {
  readonly #handles=new Map<string,{observation:DailyRecordObservation;scope:DailyRecordScope;requestRef:string;subject?:{displayName:string}}>();
  readonly #cismHandles=new Map<string,CismObservation>();
  /** Prepared reviews of this process, for a save without the owner workflow. */
  readonly #reviewInputs=new Map<string,ReviewedInput>();
  #tail:Promise<unknown>=Promise.resolve();
  /** The fourth parameter is a retired learning-policy slot: kept so positional callers keep working, never read. */
  constructor(readonly backend:DailyRecordReadBackend|null=null,readonly parentSafeguard:DailyRecordParentSafeguard|null=null,readonly now:()=>number=Date.now,_retiredLearningPolicy:unknown=null,readonly writeBackend:DailyRecordWriteBackend|null=null,
    /** Owner-durable workflow; when present it replaces process-local handles and the launch-pinned save path. */
    readonly workflow:DailyRecordWorkflow|null=null){
    if(parentSafeguard){hash.parse(parentSafeguard.policySha256);hash.parse(parentSafeguard.safeguardSourceSha256);dailyRecordIdSchema.parse(parentSafeguard.sourceRef);}
  }
  #base(context:DailyRecordContext){return {workflowContext:dailyRecordWorkflowContext,userContext:context,academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true};}
  #age(observedAt:string,limit=FRESH_READ_MS){const age=this.now()-Date.parse(observedAt);return age<0||age>limit;}
  #policyPins():ReviewPolicyPins{
    return {parentSafeguard:this.parentSafeguard?{policySha256:this.parentSafeguard.policySha256,safeguardSourceSha256:this.parentSafeguard.safeguardSourceSha256}:null};
  }
  /** Bridge-observed attribution when present, otherwise what this upstream process can observe itself. */
  #attribution(input:{actor?:unknown;attribution?:DailyRecordAttribution}):DailyRecordAttribution{
    return input.attribution??observeAttribution({observedBy:'upstream',client:null,bridgeSha256:null,declared:input.actor});
  }
  #observation(handle:string):{observation:DailyRecordObservation;scope:DailyRecordScope;requestRef:string;subject?:{displayName:string}}|undefined{
    const local=this.#handles.get(handle);if(local||!this.workflow)return local;
    const stored=this.workflow.store.get('observation',handle) as {observation:unknown;scope:unknown;requestRef:unknown;subject?:{displayName:string}|null}|null;
    if(!stored)return undefined;
    return {observation:freezeLabelData(dailyRecordObservationSchema.parse(stored.observation)),scope:dailyRecordScopeSchema.parse(stored.scope),requestRef:dailyRecordIdSchema.parse(stored.requestRef),
      ...(stored.subject?{subject:{displayName:z.string().min(1).max(200).parse(stored.subject.displayName)}}:{})};
  }
  // P4: serialization keeps the account-lock order, but failures close per
  // request. No session-wide latch: the next request gets a fresh judgement,
  // still with no automatic retry inside a request. Uncertain source state
  // is resolved only by a later readback, never by replaying the failure.
  async #serial<T>(run:()=>Promise<T>):Promise<T>{
    const result=this.#tail.then(run);
    this.#tail=result.catch(()=>{});return result;
  }
  async #read(scope:DailyRecordScope,requestRef:string,subject?:{displayName:string}){
    if(!this.backend)return {status:'unavailable' as const,reasonCode:'dailyrecord_read_backend_not_configured'};
    try{return await this.#serial(async()=>{
      const read=await this.backend!.readDailyRecord(freezeLabelData(structuredClone(scope)),requestRef,subject);
      if(read.status==='unavailable')return {status:'unavailable' as const,reasonCode:safeReason(read.reasonCode)};
      return {status:'observed' as const,observation:qualifiedObservation(read.observation,scope),subject:read.subject};
    });
    }catch(error){return {status:'unavailable' as const,...readFailureResult(error,'dailyrecord_source_unqualified_no_automatic_retry')};}
  }
  async metadata(value:unknown){
    const input=studentMetadataBatchSchema.parse(value);
    const base={scope:input.scope,purpose:input.purpose,currentRequestRef:input.currentRequestRef,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    // Failure responses retain correlation and effects without duplicating the
    // caller's scope/purpose in both MCP representations or returning source rows.
    const failureBase={currentRequestRef:input.currentRequestRef,academyWrites:0,parentSends:0,automaticRetryAllowed:false,execution:'read_only' as const};
    if(!this.backend?.readStudentMetadata)return {...failureBase,status:'unavailable' as const,reasonCode:'student_metadata_backend_not_configured'};
    try{
      // The same queue/latch protects metadata, DailyRecord and CISM reads on this service/client.
      return await this.#serial(async()=>{
        const observation=await this.backend!.readStudentMetadata!(freezeLabelData(structuredClone(input)));
        if(!same(observation.scope,input.scope)||observation.results.length!==input.queries.length
          ||observation.results.some((row,i)=>!same(row.query,input.queries[i])))throw Error('metadata_scope_or_query_conflict');
        const result={...base,status:'observed' as const,results:observation.results};
        // Measure the actual dual MCP representation, not just its structuredContent half.
        if(toolResultBytes(result)>512*1024)throw Error('metadata_result_too_large');
        return freezeLabelData(result);
      });
    }catch(error){
      const result=readFailureResult(error,'student_metadata_unqualified_no_automatic_retry');
      if(error instanceof Error && error.message==='metadata_result_too_large'){
        const {origin:_origin,errorKind:_kind,errorSha256:_hash,...failure}=result.failure;
        return {...failureBase,status:'unavailable' as const,reasonCode:result.reasonCode,failure};
      }
      return {...failureBase,status:'unavailable' as const,...result};
    }
  }
  async metadataByName(value:unknown){
    const input=studentMetadataGetSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,academyWrites:0,parentSends:0,automaticRetryAllowed:false,execution:'read_only' as const};
    if(!this.backend?.readStudentMetadataByName)return {...base,status:'unavailable' as const,reasonCode:'student_metadata_backend_not_configured'};
    try{return await this.#serial(async()=>{
      const observation=await this.backend!.readStudentMetadataByName!(freezeLabelData(structuredClone(input)));
      const scope=studentMetadataBatchSchema.shape.scope.parse(observation.scope);
      if(scope.lessonDate!==input.lessonDate||observation.results.length!==input.queries.length||observation.results.some((r,i)=>!same(r.query,input.queries[i])))throw Error('native_metadata_row_identity_conflict');
      const result={...base,status:'observed' as const,scope,results:observation.results};
      if(toolResultBytes(result)>512*1024)throw Error('metadata_result_too_large');
      return freezeLabelData(result);
    });}catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'student_metadata_unqualified_no_automatic_retry')};}
  }
  async itemAnswers(value:unknown){
    const input=itemAnswerLookupSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,labels:input.labels,
      academyWrites:0,parentSends:0,nativePrintInvoked:false,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    if(!this.backend?.readItemAnswers)return {...base,status:'unavailable' as const,reasonCode:'item_answer_backend_not_configured'};
    try{
      const result={...base,...projectItemAnswerRead(await this.#serial(()=>this.backend!.readItemAnswers!(input.labels)),input.labels,input.view)};
      if(toolResultBytes(result)>512*1024)throw Error('item_answer_result_too_large');
      return freezeLabelData(result);
    }
    catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'item_answer_source_unqualified_no_automatic_retry')};}
  }
  async textbookSections(value:unknown){
    const input=textbookSectionResultsSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,displayName:input.displayName,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    if(!this.backend?.readTextbookSections)return {...base,status:'unavailable' as const,reasonCode:'textbook_section_backend_not_configured'};
    try{
      const result=await this.#serial(async()=>{
        const observation=await this.backend!.readTextbookSections!(freezeLabelData(structuredClone(input)));
        if(observation.student.displayName!==input.displayName||observation.courseKey!==input.courseKey||observation.bookNo!==input.bookNo
          ||observation.records.length!==input.sourceTitles.length
          ||new Set(observation.records.map(r=>r.sourceTitle)).size!==input.sourceTitles.length
          ||observation.records.some(r=>!input.sourceTitles.includes(r.sourceTitle)))throw Error('textbook_section_scope_conflict');
        return {...base,status:'observed' as const,...observation};
      });
      if(toolResultBytes(result)>512*1024)throw Error('textbook_section_result_too_large');
      return freezeLabelData(result);
    }catch(error){if(isTextbookSectionNotInEither(error))return textbookSectionNotInEither(base,input.displayName);return {...base,status:'unavailable' as const,...readFailureResult(error,'textbook_section_unqualified_no_automatic_retry')};}
  }
  async prestudyGet(value:unknown){
    const input=prestudyGetSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,displayName:input.displayName,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    if(!this.backend?.readPrestudyByName)return {...base,status:'unavailable' as const,reasonCode:'prestudy_backend_not_configured'};
    try{
      const result=await this.#serial(async()=>{
        const observation=await this.backend!.readPrestudyByName!(freezeLabelData(structuredClone(input)));
        if(observation.student.displayName!==input.displayName||observation.teacherJudgment!==null)throw Error('prestudy_scope_conflict');
        return {...base,status:'observed' as const,...observation};
      });
      if(toolResultBytes(result)>512*1024)throw Error('prestudy_result_too_large');
      return freezeLabelData(result);
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'prestudy_unqualified_no_automatic_retry')};}
  }
  async prestudyReportGet(value:unknown){
    const input=prestudyReportGetSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,displayName:input.displayName,prestudyKey:input.prestudyKey,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const,
      studentContent:'teacher_only_do_not_store_or_relay' as const};
    if(!this.backend?.readPrestudyReportByName)return {...base,status:'unavailable' as const,reasonCode:'prestudy_report_backend_not_configured'};
    try{
      const result=await this.#serial(async()=>{
        const observation=await this.backend!.readPrestudyReportByName!(freezeLabelData(structuredClone(input)));
        if(observation.student.displayName!==input.displayName||observation.prestudyKey!==input.prestudyKey||observation.sourceRecord?.prestudyKey!==input.prestudyKey)throw Error('prestudy_report_scope_conflict');
        const availableSectionTitles=observation.sections.map(section=>section.title);
        const requested=input.sectionTitles??availableSectionTitles;
        if(input.sectionTitles&&requested.some(title=>availableSectionTitles.filter(t=>t===title).length>1))throw Error('prestudy_report_section_ambiguous');
        const missingSectionTitles=requested.filter(title=>!availableSectionTitles.includes(title));
        const sections=input.sectionTitles?observation.sections.filter(section=>requested.includes(section.title)):observation.sections;
        const view={mode:input.sectionTitles?'selected' as const:'full' as const,
          requestedSectionTitles:input.sectionTitles??null,availableSectionTitles,returnedSectionTitles:sections.map(section=>section.title),missingSectionTitles,
          completeRequestedView:missingSectionTitles.length===0,completeSourceReport:sections.length===observation.sections.length};
        return {...base,...observation,status:missingSectionTitles.length?'partial' as const:'observed' as const,sections,view};
      });
      // Budget is UTF-8 JSON payload bytes, excluding the MCP envelope. Never clip source blocks.
      const budget=input.maxOutputBytes??(input.sectionTitles?32*1024:512*1024);
      if(Buffer.byteLength(JSON.stringify(result),'utf8')>budget||toolResultBytes(result)>512*1024)throw Error('prestudy_report_result_too_large');
      return freezeLabelData(result);
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'prestudy_report_unqualified_no_automatic_retry')};}
  }
  async textbookSectionsByName(value:unknown){
    const input=textbookSectionResultsGetSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,displayName:input.displayName,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    if(!this.backend?.readTextbookSectionsByName)return {...base,status:'unavailable' as const,reasonCode:'textbook_section_backend_not_configured'};
    try{
      const result=await this.#serial(async()=>{
        const observation=await this.backend!.readTextbookSectionsByName!(freezeLabelData(structuredClone(input)));
        if(observation.student.displayName!==input.displayName)throw Error('textbook_section_scope_conflict');
        if('kind' in observation){
          if(observation.kind!=='interview_required'||!Array.isArray(observation.bindings)||observation.bindings.length<2)throw Error('textbook_section_scope_conflict');
          return {...base,status:'interview_required' as const,...observation};
        }
        if(input.sourceTitles){
          const records=(observation as TextbookSectionObservation).records;
          if(!Array.isArray(records)||records.length!==input.sourceTitles.length
            ||new Set(records.map(r=>r.sourceTitle)).size!==input.sourceTitles.length
            ||records.some(r=>!input.sourceTitles!.includes(r.sourceTitle)))throw Error('textbook_section_scope_conflict');
          if(input.view==='compact')return {...base,status:'observed' as const,...observation,records:records.map(({items:_items,...rest})=>rest)};
        }else if(!Array.isArray((observation as TextbookSectionIndexObservation).index))throw Error('textbook_section_scope_conflict');
        return {...base,status:'observed' as const,...observation};
      });
      if(toolResultBytes(result)>512*1024)throw Error('textbook_section_result_too_large');
      return freezeLabelData(result);
    }catch(error){if(isTextbookSectionNotInEither(error))return textbookSectionNotInEither(base,input.displayName);return {...base,status:'unavailable' as const,...readFailureResult(error,'textbook_section_unqualified_no_automatic_retry')};}
  }
  async clinicPaperStatus(value:unknown){
    const input=clinicPaperStatusGetSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,displayName:input.displayName,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    if(!this.backend?.readTextbookSectionsByName)return {...base,status:'unavailable' as const,reasonCode:'clinic_paper_backend_not_configured'};
    const slots={printed:'teacher_report_required' as const,handedOut:'teacher_report_required' as const,
      solved:'teacher_judged_only_no_server_score_input' as const,scored:'teacher_judged_only_no_server_score_input' as const,
      teacherReviewed:'teacher_report_required' as const};
    try{
      const result=await this.#serial(async()=>{
        const backend=this.backend!;
        const first=await backend.readTextbookSectionsByName!(freezeLabelData(structuredClone(
          input.sourceTitles?{displayName:input.displayName,purpose:input.purpose,currentRequestRef:input.currentRequestRef,bookLabel:input.bookLabel,sourceTitles:input.sourceTitles}
            :{displayName:input.displayName,purpose:input.purpose,currentRequestRef:input.currentRequestRef,bookLabel:input.bookLabel})));
        if(first.student.displayName!==input.displayName)throw Error('clinic_paper_scope_conflict');
        if('kind' in first){
          if(first.kind!=='interview_required'||!Array.isArray(first.bindings)||first.bindings.length<2)throw Error('clinic_paper_scope_conflict');
          return {...base,status:'interview_required' as const,...first};
        }
        let sections:ClinicPaperStatusObservation['sections'];let observedAt:string;let coverage:string;let indexBasis:string|undefined;
        if(input.sourceTitles){
          const grading=first as TextbookSectionObservation;
          if(!Array.isArray(grading.records)||grading.records.length!==input.sourceTitles.length
            ||grading.records.some(r=>!input.sourceTitles!.includes(r.sourceTitle)))throw Error('clinic_paper_scope_conflict');
          sections=grading.records.map(r=>{
            const wrongItems=r.items.filter(i=>i.scoring==='X').map(i=>({ordinal:i.ordinal,lectureKey:i.lectureKey,outerLectureKey:i.outerLectureKey}));
            return {sourceTitle:r.sourceTitle,originalAttempt:'observed_original_attempt' as const,score:r.score,
              correctCount:r.correctCount,itemCount:r.itemCount,wrongItems,clinicEligible:wrongItems.length>0,sourceDate:r.sourceDate};
          });
          observedAt=grading.observedAt;coverage=grading.coverage;
        }else{
          const indexObs=first as TextbookSectionIndexObservation;
          if(!Array.isArray(indexObs.index))throw Error('clinic_paper_scope_conflict');
          sections=indexObs.index.map(r=>({sourceTitle:r.sourceTitle,originalAttempt:'paper_row_observed' as const,score:null,
            correctCount:null,itemCount:null,wrongItems:null,clinicEligible:null,sourceDate:r.sourceDate}));
          observedAt=indexObs.observedAt;coverage=indexObs.coverage;indexBasis=indexObs.indexBasis;
        }
        const observation:ClinicPaperStatusObservation={student:first.student,courseKey:first.courseKey,bookNo:first.bookNo,
          bookLabel:(first as TextbookSectionIndexObservation).bookLabel??'',sections,
          generatedMembership:'unqualified',clinicGeneration:'unqualified',teacherJudgmentSlots:slots,
          observedAt,coverage,...(indexBasis?{indexBasis}:{})};
        return {...base,status:'observed' as const,...observation};
      });
      if(toolResultBytes(result)>512*1024)throw Error('clinic_paper_result_too_large');
      return freezeLabelData(result);
    }catch(error){if(isTextbookSectionNotInEither(error))return textbookSectionNotInEither(base,input.displayName);return {...base,status:'unavailable' as const,...readFailureResult(error,'clinic_paper_unqualified_no_automatic_retry')};}
  }
  async itemSectionTrace(value:unknown){
    const input=itemSectionTraceSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,displayName:input.displayName,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    if(!this.backend?.readItemSectionTrace)return {...base,status:'unavailable' as const,reasonCode:'item_section_trace_backend_not_configured'};
    try{
      const result=await this.#serial(async()=>{
        const observation=await this.backend!.readItemSectionTrace!(freezeLabelData(structuredClone(input)));
        if(observation.student.displayName!==input.displayName)throw Error('item_section_trace_scope_conflict');
        if(observation.kind==='traced'){
          if(observation.items.length!==input.labels.length||observation.items.some((row,i)=>row.label!==input.labels[i])
            ||observation.items.some(row=>row.matches.some(m=>!m.sourceTitle||(m.matchedBy!=='lectureKey'&&m.matchedBy!=='outerLectureKey'))))throw Error('item_section_trace_scope_conflict');
        }else if(observation.kind!=='interview_required'||!Array.isArray(observation.bindings)||observation.bindings.length<2)throw Error('item_section_trace_scope_conflict');
        return {...base,status:observation.kind==='traced'?'observed' as const:'interview_required' as const,...observation};
      });
      if(toolResultBytes(result)>512*1024)throw Error('item_section_trace_result_too_large');
      return freezeLabelData(result);
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'item_section_trace_unqualified_no_automatic_retry')};}
  }
  async get(value:unknown){
    return readDailyRecordByName(value,this,this.now);
  }
  async resolveStudent(value:unknown){
    const input=studentResolveInputSchema.parse(value);
    const base={purpose:input.purpose,currentRequestRef:input.currentRequestRef,displayName:input.displayName,
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only' as const};
    if(!this.backend?.searchStudentsByName)return {...base,status:'unavailable' as const,reasonCode:'student_resolve_backend_not_configured'};
    try{
      return await this.#serial(async()=>{
        const result=await resolveStudentByName(input.displayName,{searchByName:name=>this.backend!.searchStudentsByName!(name,input.currentRequestRef,input.lessonDate)});
        if(result.status==='unavailable')return {...base,...result};
        // The configured teacher binds the resolved key for the intake that follows.
        const teacher=this.backend!.teacherScope?.();
        return freezeLabelData({...base,...result,...(teacher?{teacherId:teacher.teacherId}:{})});
      });
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'student_resolve_unqualified_no_automatic_retry')};}
  }
  async intake(value:unknown){
    const input=dailyRecordIntakeSchema.parse(value),context=resolveWriteContext(input.context,input.suppliedValues);
    if(new Set(input.suppliedValues.map(p=>p.field)).size!==input.suppliedValues.length)throw Error('duplicate_proposal_field');
    const read=await this.#read(context.scope,context.currentRequestRef),source=read.status==='observed'?read.observation:null;
    const suppliedValues=input.suppliedValues.map(p=>resolveProposal(p,context,source?.fields[p.field]));
    const blockers:string[]=read.status==='unavailable'?[read.reasonCode]:this.#age(source!.observedAt)?['stale_source']:[];
    const interviews:{field:DailyRecordField;reasonCode:string}[]=[];
    const fields=dailyRecordFields.map(field=>{
      const current=source?.fields[field]??{state:'unknown' as const,reasonCode:'source_not_read'},explicit=suppliedValues.find(p=>p.field===field);
      const supplied=explicit&&!proposalBlockers(explicit,context).length;
      const existing=current.state==='known'&&!unknownAssessment(field,current.value)
        &&(!['attendance','progress','homework'].includes(field)||current.value.trim().length>0);
      const facts=context.facts.some(f=>f.field===field);
      // A first lesson has no previous record whose homework could be assessed.
      const noPrevious=field==='previousHomeworkAssessment'&&!!source&&source.previousTarget===null;
      if(field==='lessonMemo'){if(context.memoAnswer.kind==='not_supplied')interviews.push({field,reasonCode:'today_student_specific_memo_required'});}
      else if(!noPrevious&&!existing&&!supplied&&(!['progress','homework'].includes(field)||!facts))interviews.push({field,reasonCode:'explicit_value_or_qualified_existing_value_required'});
      return {field,before:current,selected:context.selectedFields.includes(field),disposition:noPrevious?'no_previous_record':existing?'preserve_existing':supplied?'explicit_user_value':facts&&['progress','homework'].includes(field)?'user_facts_available':'missing',
        target:field==='previousHomeworkAssessment'?source?.previousTarget??null:source?.target??null};
    });
    for(const p of suppliedValues)blockers.push(...proposalBlockers(p,context).map(b=>p.field+':'+b));
    const warnings=suppliedValues.flatMap(p=>factWarnings(factCheckCodes(p,context).codes).map(w=>({field:p.field,...w})));
    let observationHandle:string|null=null;
    const attribution=this.workflow?this.#attribution(input):null;
    if(source&&!blockers.length){
      if(this.#handles.size>=128)this.#handles.delete(this.#handles.keys().next().value!);observationHandle='dailyrecord-observation:'+randomUUID();
      const subject=read.status==='observed'&&read.subject?{displayName:read.subject.displayName}:undefined;
      this.#handles.set(observationHandle,{observation:source,scope:freezeLabelData(structuredClone(context.scope)),requestRef:context.currentRequestRef,...(subject?{subject}:{})});
      if(this.workflow&&attribution){
        this.workflow.store.publishOnce('observation',observationHandle,{observation:source,scope:context.scope,requestRef:context.currentRequestRef,subject:subject??null,createdBy:attribution,createdAt:new Date(this.now()).toISOString()});
        this.workflow.store.appendAudit(workflowRequestKey(context.scope,context.currentRequestRef),{event:'observation_recorded',observationHandle,by:attributionSummary(attribution),sourceRevision:source.sourceRevision});
      }
    }
    return {...this.#base(context),status:blockers.length?'held':interviews.length?'interview_required':'ready_for_review',observationHandle,
      observationPersistence:observationHandle&&this.workflow?'owner_durable' as const:'process_local' as const,
      attribution:attribution?attributionSummary(attribution):null,fields,interviews,assessmentCodeTables:ASSESSMENT_CODE_TABLES,blockers,warnings,
      source:source?{observedAt:source.observedAt,sourceRevision:source.sourceRevision,wireDigest:source.wireDigest,transmissionState:source.transmissionState}:null,
      toneSamples:source?.toneSamples.filter(s=>this.parentSafeguard?.checkLines(s.text))??[],toneSelection:'hyphen_lines_only_other_formats_ignored',sourceAvailability:read.status,failure:'failure' in read?read.failure:null,execution:'not_supported'};
  }
  async review(value:unknown){return this.#review(value,true);}
  async #review(value:unknown,persist:boolean){
    const parsed=dailyRecordReviewSchema.parse(value),held=parsed.observationHandle?this.#observation(parsed.observationHandle):undefined;
    if(parsed.observationHandle&&!held)throw Error('exact_backend_observation_handle_required');
    // Members the caller left out come from the handle (scope, request) and the defaults.
    const context=resolveWriteContext(parsed.context,parsed.proposals,held);
    if(held&&(!same(held.scope,context.scope)||held.requestRef!==context.currentRequestRef))throw Error('exact_backend_observation_handle_required');
    const input={...parsed,context,proposals:parsed.proposals.map(p=>resolveProposal(p,context,held?.observation.fields[p.field]))};
    if(new Set(input.proposals.map(p=>p.field)).size!==input.proposals.length)throw Error('duplicate_proposal_field');
    const global:string[]=held?[]:['qualified_before_source_unavailable'];
    const cism=input.cismObservationHandle?this.#cismHandles.get(input.cismObservationHandle):undefined;
    if(input.cismObservationHandle&&(!cism||!same(cism.scope,context.scope)||this.#age(cism.observedAt)))global.push('exact_fresh_cism_observation_handle_required');
    for(const fact of context.facts.filter(f=>f.origin==='cism_section')){
      if(!cism||!same(cism.scope,context.scope)||this.#age(cism.observedAt)||!cism.evidenceRefs.includes(fact.ref)||fact.text!==cism.text)global.push('cism_fact_not_bound_to_exact_observed_section');
    }
    if(held&&this.#age(held.observation.observedAt,HELD_OBSERVATION_MS))global.push('stale_source');
    // Fresh authoritative before-state; this pure review neither consumes approval nor writes.
    const read=!held||global.length?null:await this.#read(context.scope,context.currentRequestRef,held.subject);
    if(read?.status==='unavailable')global.push(read.reasonCode);
    if(held&&read?.status==='observed'){
      if(this.#age(read.observation.observedAt))global.push('stale_source');
      if(!same(read.observation.target,held.observation.target)||!same(read.observation.previousTarget,held.observation.previousTarget)||!same(fieldValues(read.observation),fieldValues(held.observation))||read.observation.wireDigest!==held.observation.wireDigest||read.observation.sourceRevision!==held.observation.sourceRevision||read.observation.transmissionState!==held.observation.transmissionState)global.push('source_before_or_contract_drift');
    }
    const rows=input.proposals.map(p=>{
      const before=held?.observation.fields[p.field]??{state:'unknown' as const,reasonCode:'qualified_before_source_unavailable'},target=p.field==='previousHomeworkAssessment'?held?.observation.previousTarget??null:held?.observation.target??null;
      const blockers=[...global,...proposalBlockers(p,context)];
      if(before.state!=='known')blockers.push('unknown_before');
      if(!target)blockers.push(p.field==='previousHomeworkAssessment'?'independent_previous_record_required':'qualified_current_record_required');
      const changed=before.state!=='known'||before.value!==p.value,checked=factCheckCodes(p,context);
      if(changed&&p.value!==''&&['progress','homework','lessonMemo'].includes(p.field)){
        if(this.parentSafeguard&&!this.parentSafeguard.checkLines(p.value))blockers.push('parent_hyphen_lines_required');
        if(!this.parentSafeguard)blockers.push('canonical_parent_safeguard_not_connected');
        else if(this.parentSafeguard.check(p.value))blockers.push('parent_internal_language_excluded');
      }
      const unique=[...new Set(blockers)];
      return {field:p.field,target,before,after:p.value,factRefs:p.factRefs,decision:p.decision,
        disposition:unique.length?'blocked':changed?'proposed':'noop',blockers:unique,warnings:factWarnings(checked.codes),missingFactFragments:checked.missingFragments,
        holdActions:holdActions(p.field,unique),
        semanticFactReview:'teacher_review_required_not_proven_by_fragment_checks'};
    });
    // The memo is asked for only when it is proposed without content.
    const interviews=context.memoAnswer.kind==='not_supplied'&&input.proposals.some(p=>p.field==='lessonMemo')?[{field:'lessonMemo',reasonCode:'today_student_specific_memo_required'}]:[];
    const patch=rows.filter(r=>r.disposition==='proposed').map(r=>({field:r.field,target:r.target,before:r.before.state==='known'?r.before.value:null,after:r.after,factRefs:r.factRefs}));
    const parentSafeguard=this.parentSafeguard?{sourceRef:this.parentSafeguard.sourceRef,policySha256:this.parentSafeguard.policySha256,safeguardSourceSha256:this.parentSafeguard.safeguardSourceSha256}:null;
    const cismSource=cism?{observationHandle:input.cismObservationHandle,scope:cism.scope,selection:cism.selection,observedAt:cism.observedAt,sourceRevision:cism.sourceRevision,evidenceRefs:cism.evidenceRefs}:null;
    // The exact submitted input is part of the digest, so distinct reviewed inputs never share one immutable review record.
    const inputJson=canonicalJson({context,proposals:input.proposals,cismObservationHandle:input.cismObservationHandle??null});
    const inputDigest=textDigest(inputJson);
    const body={observationHandle:input.observationHandle??null,inputDigest,scope:context.scope,requestRef:context.currentRequestRef,userContextDigest:digest(context),parentSafeguard,cismSource,
      sourceRevision:held?.observation.sourceRevision??null,wireDigest:held?.observation.wireDigest??null,transmissionState:read?.status==='observed'?read.observation.transmissionState:held?.observation.transmissionState??'unknown',
      // Decisions and warnings are reported, not digested: inputDigest binds the decision, and a review recorded before them keeps its digest.
      rows:rows.map(({decision:_decision,warnings:_warnings,...row})=>row),patch};
    const status=rows.some(r=>r.blockers.length)||interviews.length?'held' as const:'prepared' as const,reviewDigest=digest(body);
    let reviewRecord:{persisted:boolean;reviewedBy:ReturnType<typeof attributionSummary>}|null=null;
    const reviewedInput:ReviewedInput|null=input.observationHandle?{context,observationHandle:input.observationHandle,cismObservationHandle:input.cismObservationHandle??null,proposals:input.proposals}:null;
    if(persist&&status==='prepared'&&reviewedInput){
      if(this.#reviewInputs.size>=128)this.#reviewInputs.delete(this.#reviewInputs.keys().next().value!);
      this.#reviewInputs.set(reviewDigest,reviewedInput);
    }
    if(persist&&this.workflow&&status==='prepared'&&input.observationHandle){
      const reviewedBy=this.#attribution(input);
      const record:StoredReview={reviewDigest,status,scope:context.scope,requestRef:context.currentRequestRef,observationHandle:input.observationHandle,
        patch:(patch as ReviewedPatch[]).map(p=>({field:p.field,before:p.before??'',after:p.after,effectRecordKey:p.target?.recordId??''})),
        reviewPolicy:this.#policyPins(),reviewedBy,reviewedAt:new Date(this.now()).toISOString(),
        inputDigest,inputLength:inputJson.length,input:reviewedInput!};
      this.workflow.store.publishOnce('review',reviewDigest,record);
      this.workflow.store.appendAudit(workflowRequestKey(context.scope,context.currentRequestRef),{event:'review_recorded',reviewDigest,by:attributionSummary(reviewedBy),fields:record.patch.map(p=>p.field)});
      reviewRecord={persisted:true,reviewedBy:attributionSummary(reviewedBy)};
    }
    return {...this.#base(context),...body,rows,status,interviews,reviewDigest,reviewRecord,
      execution:'not_supported',saveAdmission:this.workflow?'recorded_review_and_user_instruction_required':'separate_trusted_native_owner_required'};
  }
  #reviewedInput(reviewDigest:string):ReviewedInput|null{
    return this.#reviewInputs.get(reviewDigest)??(this.workflow?.store.get('review',reviewDigest) as StoredReview|null|undefined)?.input??null;
  }
  async save(value:unknown){
    const parsed=dailyRecordSaveSchema.parse(value);
    // Members the caller left out come from the recorded prepared review of this digest.
    const stored=parsed.context&&parsed.proposals&&parsed.observationHandle?null:this.#reviewedInput(parsed.reviewDigest);
    const observationHandle=parsed.observationHandle??stored?.observationHandle,offered=parsed.proposals??stored?.proposals;
    if(!observationHandle||!offered)return {workflowContext:dailyRecordWorkflowContext,academyWrites:0,parentSends:0,automaticRetryAllowed:false,reviewDigest:parsed.reviewDigest,
      status:'held' as const,reasonCode:'prior_review_record_required',execution:'not_supported' as const,fields:[]};
    const held=this.#observation(observationHandle),context=resolveWriteContext(parsed.context??stored?.context,offered,held);
    const instructed=parsed.instruction===undefined?undefined:requestFromInstruction(parsed.instruction,context.currentRequestRef);
    const input:ResolvedSave={...parsed,context,observationHandle,cismObservationHandle:parsed.cismObservationHandle??stored?.cismObservationHandle??undefined,
      proposals:offered.map(p=>resolveProposal(p,context,held?.observation.fields[p.field])),request:parsed.request??instructed??undefined};
    if(this.workflow)return this.#saveRequestBound(input);
    const base={...this.#base(context),observationHandle:input.observationHandle,reviewDigest:input.reviewDigest};
    if(!input.request&&instructed===null)return {...base,status:'held' as const,reasonCode:'user_instruction_provenance_required',academyWrites:0,execution:'not_supported' as const,fields:[]};
    if(!this.writeBackend?.saveConfigured())return {...base,status:'unavailable' as const,reasonCode:'dailyrecord_save_backend_not_configured',
      academyWrites:0,execution:'not_supported' as const,saveAdmission:'separate_trusted_native_owner_required' as const,fields:[]};
    // Recompute the exact review against a fresh before-state. The caller's
    // reviewDigest must match it, so unreviewed or drifted text never writes.
    const reviewed=await this.#review({context,observationHandle:input.observationHandle,cismObservationHandle:input.cismObservationHandle,proposals:input.proposals},false);
    if(reviewed.status!=='prepared'||reviewed.reviewDigest!==input.reviewDigest||reviewed.execution!=='not_supported')
      return {...base,status:'held' as const,reasonCode:'reviewed_patch_required',academyWrites:0,execution:'not_supported' as const,
        fields:reviewed.rows.map(r=>({field:r.field,status:'not_attempted' as const,dispatched:false}))};
    const patch=(reviewed.patch as {field:DailyRecordField;before:string|null;after:string}[]).map(p=>({field:p.field,value:p.after,before:p.before??'',wireDigest:reviewed.wireDigest??''}));
    if(!patch.length)return {...base,status:'saved' as const,academyWrites:0,execution:'owner_write_backend' as const,
      fields:input.proposals.map(p=>({field:p.field,status:'noop' as const,dispatched:false}))};
    try{
      const fields=await this.#serial(()=>this.writeBackend!.saveDailyRecord(freezeLabelData(structuredClone(context.scope)),patch));
      const academyWrites=fields.filter(f=>f.dispatched).length;
      const settled=fields.every(f=>f.status==='persisted'||f.status==='noop');
      return {...base,status:settled?'saved' as const:'held' as const,reasonCode:settled?null:'save_reconciliation_required',
        academyWrites,execution:'owner_write_backend' as const,fields};
    }catch(error){return {...base,status:'held' as const,...readFailureResult(error,'dailyrecord_save_unqualified_no_automatic_retry'),reasonCode:'save_reconciliation_required',academyWrites:null,execution:'not_supported' as const,fields:[]};}
    // A backend throw is reconciled by readback, never denied as zero
    // writes: conforming backends report per-field dispatch themselves,
    // so this path only covers pre-dispatch or non-conforming failures.
  }
  /** Owner-workflow save. Any delegated session may save; the request
   * records the user's verbatim instruction and every step records who did
   * it. A prior recorded review, the shared writer lock, a fresh recomputed
   * review, one dispatch per exact request/field transition and recorded
   * outcomes still gate every write. An unresolved dispatch on the same
   * record field, under any request, blocks until academy_dailyrecord_reconcile. */
  async #saveRequestBound(input:ResolvedSave){
    const store=this.workflow!.store,context=input.context,scope=context.scope,requestRef=context.currentRequestRef;
    const requestKey=workflowRequestKey(scope,requestRef),attribution=this.#attribution(input),by=attributionSummary(attribution);
    const base={...this.#base(context),observationHandle:input.observationHandle,reviewDigest:input.reviewDigest,attribution:by,
      parentTransmission:'separate_explicit_instruction_required' as const,workflowStoreScope:store.scope};
    const hold=(reasonCode:string,extra:Record<string,unknown>={})=>{
      store.appendAudit(requestKey,{event:'save_held',reasonCode,reviewDigest:input.reviewDigest,by});
      return {...base,status:'held' as const,reasonCode,academyWrites:0,execution:'not_supported' as const,fields:[] as unknown[],...extra};
    };
    if(!this.writeBackend?.requestBoundSaveConfigured?.())
      return {...base,status:'unavailable' as const,reasonCode:'dailyrecord_request_bound_save_backend_not_configured',academyWrites:0,execution:'not_supported' as const,fields:[]};
    if(!input.request)return hold('user_instruction_provenance_required');
    if(input.request.requestRef!==requestRef)return hold('user_request_ref_mismatch');
    const instructionSha256=textDigest(input.request.instruction.text);
    const recorded=store.publishOnce('request',requestKey,{request:input.request,instructionSha256,recordedBy:attribution,recordedAt:new Date(this.now()).toISOString()});
    if(!recorded.created&&(recorded.record as {instructionSha256:string}).instructionSha256!==instructionSha256)return hold('request_instruction_conflict');
    if(recorded.created)store.appendAudit(requestKey,{event:'request_recorded',instructionSha256,sessionRef:input.request.instruction.sessionRef??null,by});
    const prior=store.get('review',input.reviewDigest) as StoredReview|null;
    if(!prior)return hold('prior_review_record_required');
    const inputJson=canonicalJson({context,proposals:input.proposals,cismObservationHandle:input.cismObservationHandle??null});
    const mismatchPaths:string[]=[];
    if(prior.status!=='prepared')mismatchPaths.push('status');
    if(!canonicalSame(prior.scope,scope))mismatchPaths.push('scope');
    if(prior.requestRef!==requestRef)mismatchPaths.push('requestRef');
    if(prior.observationHandle!==input.observationHandle)mismatchPaths.push('observationHandle');
    if(!canonicalSame(prior.reviewPolicy,this.#policyPins()))mismatchPaths.push('reviewPolicy');
    if(prior.inputDigest!==textDigest(inputJson))mismatchPaths.push('inputDigest');
    if(mismatchPaths.length)return hold('prior_review_record_mismatch',{mismatchPaths,...(mismatchPaths.includes('inputDigest')?{expectedLen:prior.inputLength??null,actualLen:inputJson.length}:{})});
    const key=(p:{field:DailyRecordField;effectRecordKey:string;before:string;after:string})=>executionKey(requestRef,scope,p.field,p.effectRecordKey,p.before,p.after);
    const report=(p:StoredReview['patch'][number],extra:Record<string,unknown>={})=>({field:p.field,executionState:fieldExecutionState(store,key(p)),...extra});
    try{
      return await store.withLock(workflowLockKey(scope),async()=>{
        // Under the lock: another process may have dispatched meanwhile, under this or another request.
        const unresolved=prior.patch.map(p=>({field:p.field,count:unresolvedOnField(store,scope,p.field,p.effectRecordKey).length})).filter(u=>u.count);
        if(unresolved.length)return hold('uncertain_dispatch_reconcile_required',{fields:unresolved.map(u=>({field:u.field,unresolvedExecutions:u.count}))});
        const states=prior.patch.map(p=>fieldExecutionState(store,key(p)));
        if(states.some(consumedWithoutEffect))return hold('transition_already_attempted_new_request_required',{fields:prior.patch.map(p=>report(p))});
        if(prior.patch.length&&states.every(settledEffect))return {...base,status:'saved' as const,reasonCode:'duplicate_execution_already_settled',academyWrites:0,
          execution:'owner_request_bound' as const,fields:prior.patch.map(p=>report(p,{status:'noop',dispatched:false}))};
        // Even an empty reviewed patch is re-checked against the fresh source before reporting success.
        const fresh=await this.#review({context,observationHandle:input.observationHandle,cismObservationHandle:input.cismObservationHandle,proposals:input.proposals},false);
        if(fresh.status!=='prepared'||fresh.reviewDigest!==input.reviewDigest)
          return hold('fresh_review_drift_new_review_required',{fields:fresh.rows.map(r=>({field:r.field,status:'not_attempted',dispatched:false,blockers:r.blockers,holdActions:r.holdActions}))});
        if(!prior.patch.length)return {...base,status:'saved' as const,reasonCode:'reviewed_patch_empty',academyWrites:0,execution:'owner_request_bound' as const,
          fields:input.proposals.map(p=>({field:p.field,status:'noop' as const,dispatched:false}))};
        const patches=(fresh.patch as ReviewedPatch[]).map(p=>({field:p.field,value:p.after,before:p.before??'',wireDigest:fresh.wireDigest??''}));
        const approval=requestBoundApproval(store,{requestRef,attribution,reviewDigest:input.reviewDigest,scope,reviewed:prior.patch});
        const subject=this.#observation(input.observationHandle)?.subject;
        store.appendAudit(requestKey,{event:'dispatch_started',by,fields:patches.map(p=>p.field)});
        const outcome=(p:StoredReview['patch'][number],record:Record<string,unknown>)=>{
          if(store.get('execution',key(p)))store.publishOnce('outcome',key(p),{...record,recordedBy:by,recordedAt:new Date(this.now()).toISOString()});
        };
        let results:Awaited<ReturnType<DailyRecordWriteBackend['saveDailyRecord']>>;
        try{results=await this.#serial(()=>this.writeBackend!.saveDailyRecord(freezeLabelData(structuredClone(scope)),patches,approval,subject));}
        catch(error){
          // An execution record marks a possible dispatch; it stays unresolved until a readback reconciliation.
          for(const p of prior.patch)outcome(p,{status:'unresolved',dispatched:null});
          store.appendAudit(requestKey,{event:'dispatch_failed',by,reasonCode:readFailureResult(error,'dailyrecord_save_unqualified_no_automatic_retry').reasonCode});
          return {...base,status:'held' as const,reasonCode:'save_reconciliation_required',academyWrites:null,execution:'owner_request_bound' as const,
            fields:prior.patch.map(p=>report(p,{status:'unresolved',dispatched:null}))};
        }
        // Fields refused before their execution record (drift, not attempted) stay available.
        // A readback that shows the old value after an uncertain transport does not prove
        // the request will never land; it stays unresolved until reconciliation.
        const recorded=(r:typeof results[number])=>r.status==='not_persisted'&&r.transport!=='returned'?'unresolved':r.status;
        for(const r of results){const p=prior.patch.find(x=>x.field===r.field);if(p)outcome(p,{status:recorded(r),readbackStatus:r.status,dispatched:r.dispatched,transport:r.transport??null});}
        const academyWrites=results.filter(r=>r.dispatched).length;
        const settled=results.every(r=>r.status==='persisted'||r.status==='noop');
        const reasonCode=settled?null:results.some(r=>r.status==='unresolved'||r.status==='conflict'||recorded(r)==='unresolved')?'save_reconciliation_required'
          :results.some(r=>r.status==='before_drift')?'fresh_before_drift_new_review_required'
          :results.some(r=>r.status==='not_persisted')?'readback_not_persisted_new_request_required':'partial_save_review_remaining_fields';
        store.appendAudit(requestKey,{event:'dispatch_completed',by,academyWrites,results:results.map(r=>({field:r.field,status:r.status,dispatched:r.dispatched}))});
        return {...base,status:settled?'saved' as const:'held' as const,reasonCode,academyWrites,execution:'owner_request_bound' as const,
          fields:results.map(r=>{const p=prior.patch.find(x=>x.field===r.field);return {field:r.field,status:recorded(r),dispatched:r.dispatched,transport:r.transport??null,executionState:p?fieldExecutionState(store,key(p)):'available'};})};
      });
    }catch(error){
      if(error instanceof Error&&error.message==='shared_writer_lock_busy')return hold('shared_writer_lock_busy');
      throw error;
    }
  }
  /** Read-only classification of every unresolved dispatch of this student, by
   * authoritative readback of each execution's own lesson record. Never
   * writes to the academy or retries. Only the new value, or the old value
   * after a returned transport, closes an execution by readback: elapsed time
   * never proves an uncertain request will not land. Closing an uncertain
   * dispatch as not landed needs the user's verbatim instruction. */
  async reconcile(value:unknown){
    const input=dailyRecordReconcileSchema.parse(value),context=input.context,scope=context.scope;
    const base={...this.#base(context),execution:'read_only' as const,parentTransmission:'separate_explicit_instruction_required' as const};
    if(!this.workflow)return {...base,status:'unavailable' as const,reasonCode:'dailyrecord_workflow_not_configured',fields:[]};
    const store=this.workflow.store,by=attributionSummary(this.#attribution(input)),resolution=input.resolution?.request;
    if(resolution&&resolution.requestRef!==context.currentRequestRef)return {...base,status:'held' as const,reasonCode:'user_request_ref_mismatch',by,fields:[]};
    const resolvedBy=resolution?{requestRef:resolution.requestRef,instruction:resolution.instruction,instructionSha256:textDigest(resolution.instruction.text)}:null;
    // A resolution closes only the executions it names, on this exact lesson record.
    const resolves=(record:{scope:DailyRecordScope;requestRef:string;field:DailyRecordField})=>!!resolvedBy&&canonicalSame(record.scope,scope)
      &&input.resolution!.executions.some(e=>e.requestRef===record.requestRef&&e.field===record.field);
    return store.withLock(workflowLockKey(scope),async()=>{
      const fields=()=>studentExecutions(store,scope).map(e=>({field:e.record.field,lessonDate:e.record.scope.lessonDate,requestRef:e.record.requestRef,executionState:e.state}));
      const pending=studentExecutions(store,scope).filter(e=>needsReconciliation(e.state));
      if(!pending.length)return {...base,status:'settled' as const,reasonCode:null,by,fields:fields()};
      type SourceRead={status:'observed';observation:DailyRecordObservation}|{status:'unavailable';reasonCode:string};
      const reads=new Map<string,SourceRead>();let notObserved=false,uncertain=false;
      for(const {key,record} of pending){
        const scopeKey=JSON.stringify(record.scope);
        if(!reads.has(scopeKey)){
          const observation=store.get('observation',(store.get('review',record.reviewDigest) as StoredReview|null)?.observationHandle??'') as {subject?:{displayName:string}|null}|null;
          reads.set(scopeKey,await this.#read(record.scope,context.currentRequestRef,observation?.subject??undefined));
        }
        const read=reads.get(scopeKey)!;
        if(read.status!=='observed'){
          store.appendAudit(workflowRequestKey(record.scope,record.requestRef),{event:'reconcile_readback_unavailable',by,reasonCode:read.reasonCode});
          continue;
        }
        const target=record.field==='previousHomeworkAssessment'?read.observation.previousTarget?.recordId:read.observation.target.recordId;
        // The lesson now binds a different record (for example a backfilled
        // intermediate lesson): the affected record was not observed, so the
        // effect stays unresolved and blocks its own record field.
        if(record.effectRecordKey!==target){
          notObserved=true;
          store.appendAudit(workflowRequestKey(record.scope,record.requestRef),{event:'reconcile_effect_record_not_observed',field:record.field,by});
          continue;
        }
        const current=read.observation.fields[record.field];
        if(current.state!=='known')continue;
        const classification=textDigest(current.value)===record.afterSha256?'persisted':textDigest(current.value)===record.beforeSha256?'not_persisted':'conflict';
        // Without a returned transport the request may still be pending remotely after its
        // sender died, so only the new value, or the user's instruction, closes it.
        const returned=(store.get('outcome',key) as {transport?:string|null}|null)?.transport==='returned';
        if(classification!=='persisted'&&!returned&&!resolves(record)){uncertain=true;continue;}
        const basis=classification==='persisted'?'readback_new_value':returned?'readback_after_returned_transport':'user_instruction';
        store.publishOnce('reconciliation',key,{classification,basis,resolvedBy:basis==='user_instruction'?resolvedBy:null,
          sourceRevision:read.observation.sourceRevision,observedAt:read.observation.observedAt,by,reconciledAt:new Date(this.now()).toISOString()});
        store.appendAudit(workflowRequestKey(record.scope,record.requestRef),{event:'reconciled',field:record.field,classification,basis,
          instructionSha256:basis==='user_instruction'?resolvedBy!.instructionSha256:null,by});
      }
      const after=fields(),open=after.some(f=>needsReconciliation(f.executionState));
      const unavailable=[...reads.values()].some(r=>r.status!=='observed');
      return {...base,status:open?'held' as const:'settled' as const,
        reasonCode:open?(unavailable?'readback_unavailable_reconcile_later':notObserved?'affected_record_not_observed_manual_reconcile_required':uncertain?'uncertain_dispatch_not_landed_user_resolution_required':'readback_field_unknown_reconcile_later'):null,by,fields:after};
    });
  }
  /** Metadata-only handoff view: per-field executions on this record with who did them, plus the request audit. No field or instruction text. */
  async executionStatus(value:unknown){
    const input=dailyRecordExecutionStatusSchema.parse(value),scope=input.scope;
    const base={scope,requestRef:input.requestRef??null,academyWrites:0,parentSends:0,automaticRetryAllowed:false,execution:'read_only' as const};
    if(!this.workflow)return {...base,status:'unavailable' as const,reasonCode:'dailyrecord_workflow_not_configured'};
    const store=this.workflow.store;
    const executions=studentExecutions(store,scope).filter(e=>canonicalSame(e.record.scope,scope));
    const fields=dailyRecordFields.map(field=>({field,executions:executions.filter(e=>e.record.field===field).map(({record:e,state})=>
      ({requestRef:e.requestRef,effectRecordKey:e.effectRecordKey,beforeSha256:e.beforeSha256,afterSha256:e.afterSha256,executionState:state,by:attributionSummary(e.attribution),recordedAt:e.recordedAt}))}))
      .filter(f=>f.executions.length);
    const request=input.requestRef?store.get('request',workflowRequestKey(scope,input.requestRef)) as {instructionSha256:string;request:{instruction:{sessionRef?:string;receivedAt?:string}};recordedBy:DailyRecordAttribution;recordedAt:string}|null:null;
    return {...base,status:'recorded' as const,storeScope:store.scope,fields,
      request:request?{instructionSha256:request.instructionSha256,sessionRef:request.request.instruction.sessionRef??null,receivedAt:request.request.instruction.receivedAt??null,
        recordedBy:attributionSummary(request.recordedBy),recordedAt:request.recordedAt}:null,
      audit:input.requestRef?store.audit(workflowRequestKey(scope,input.requestRef)):[]};
  }
  async cism(value:unknown){
    const input=cismSectionSchema.parse(value),context=input.context;
    if(!this.backend?.readCismSection)return {...this.#base(context),status:'unavailable',reasonCode:'exact_cism_read_backend_not_configured',observationHandle:null,execution:'not_supported'};
    try{return await this.#serial(async()=>{
      const read=await this.backend!.readCismSection!(freezeLabelData(structuredClone(context.scope)),freezeLabelData(structuredClone(input.selection)));
      if(read.status==='unavailable')return {...this.#base(context),status:'unavailable',reasonCode:safeReason(read.reasonCode),observationHandle:null,execution:'not_supported'};
      const observation=cismObservationSchema.parse(read.observation);
      if(!same(observation.scope,context.scope)||!same(observation.selection,input.selection)||this.#age(observation.observedAt)
        ||observation.sourceReferenceDate<input.selection.periodStart||observation.sourceReferenceDate>input.selection.periodEnd)throw Error('cism_scope_or_freshness_conflict');
      if(this.#cismHandles.size>=128)this.#cismHandles.delete(this.#cismHandles.keys().next().value!);
      const observationHandle='cism-observation:'+randomUUID();this.#cismHandles.set(observationHandle,freezeLabelData(observation));
      return {...this.#base(context),status:'observed',observationHandle,observation:freezeLabelData(observation),parentRewording:'not_performed',execution:'not_supported'};
    });
    }catch(error){return {...this.#base(context),status:'unavailable',...readFailureResult(error,'exact_cism_source_unqualified_no_automatic_retry'),observationHandle:null,execution:'not_supported'};}
  }
}
