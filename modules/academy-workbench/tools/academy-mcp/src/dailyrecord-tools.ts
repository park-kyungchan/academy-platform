import {academyRoutingDescription} from './routing-context.ts';
import type {McpServer} from '@modelcontextprotocol/server';
import {readOnlyAnnotations} from './contracts.ts';
import {DailyRecordService,dailyRecordIntakeSchema,dailyRecordReviewSchema,dailyRecordSaveSchema,dailyRecordReconcileSchema,dailyRecordExecutionStatusSchema,prestudyGetSchema,prestudyReportGetSchema,textbookSectionResultsGetSchema,clinicPaperStatusGetSchema,itemSectionTraceSchema,studentMetadataGetSchema,type DailyRecordParentSafeguard} from './dailyrecord.ts';
import {dailyRecordSaveAdmissionSchema,type DailyRecordSaveAdmission} from './dailyrecord-save-admission.ts';
import {studentResolveInputSchema} from './student-resolver.ts';
import {readPinnedLocalBytes} from './item-label-store.ts';
import {dailyRecordGetSchema,dailyRecordGetDescription} from './dailyrecord-read.ts';
import {z} from 'zod';
import {itemAnswerLookupSchema} from './item-answer-lookup.ts';
import {OfficialReportPdfService,officialReportPdfGetSchema} from './official-report-pdf.ts';

export function registerDailyRecordTools(server:McpServer,service=new DailyRecordService()){
  const annotations={...readOnlyAnnotations,openWorldHint:service.backend!==null,idempotentHint:service.backend===null};
  const result=(value:unknown)=>{const structuredContent=JSON.parse(JSON.stringify(value)) as Record<string,unknown>;
    return {content:[{type:'text' as const,text:JSON.stringify(structuredContent)}],structuredContent};};
  server.registerTool('academy_item_answer_lookup',{
    title:'Read official answer literals by exact item labels',
    description:academyRoutingDescription('academy_item_answer_lookup','Read source-provided official answer literals for the exact requested numeric item labels. Each answer may include delimiter-based answerPrefixCandidate and answerPayloadCandidate; their rendering meaning is not qualified. Escape pipes when presenting answerLiteral in a Markdown table. Each row also carries sourceUnitName and sourcePatternName from the same source item row: reverse-mapping candidates toward a textbook subunit, not a section title (for example a required-examples or type-drill section) or paper membership, so confirm the exact section by interview or a section title index before mapping. This does not read a student response or establish a printed paper version, and literal rendering may be unqualified or refer to an external solution. Bounded to 32 labels. No grading, saving, printing, sending or automatic retry.'),
    inputSchema:itemAnswerLookupSchema,annotations,
  },async input=>result(await service.itemAnswers(input)));
  server.registerTool('academy_dailyrecord_get',{
    title:'Read live DailyRecord by student name (today by default)',
    description:academyRoutingDescription('academy_dailyrecord_get',dailyRecordGetDescription),inputSchema:dailyRecordGetSchema,annotations,
  },async input=>result(await service.get(input)));
  server.registerTool('academy_student_metadata_get',{
    title:'Read metadata by exact dated identity under its own owner policy',
    description:academyRoutingDescription('academy_student_metadata_get','Resolve the exact student on the requested lesson date from the DailyRecord rows of the configured teacher, then read explicit queries in one backend lifetime. Existing page and stage limitations remain. No writes, printing, grading, generation, sending or automatic retry.'),
    inputSchema:studentMetadataGetSchema,annotations,
  },async input=>result(await service.metadataByName(input)));
  server.registerTool('academy_student_resolve',{
    title:'Resolve one exact student display name to its native student key',
    description:academyRoutingDescription('academy_student_resolve','Resolve an exact display name to its native key for live tools. For a DailyRecord lookup prefer academy_dailyrecord_get, which resolves and runs intake using owner-supplied scope. lessonDate selects the exact qualified DailyRecord occurrence rather than a recent-test page. Zero rows or multi-key collisions stop with a named blocker and never prove the student is absent. Uses the same native read lifecycle. No writes, guessing or automatic retry.'),
    inputSchema:studentResolveInputSchema,annotations,
  },async input=>result(await service.resolveStudent(input)));
  server.registerTool('academy_dailyrecord_intake',{
    title:'Prepare scoped DailyRecord intake and missing-only interview',
    description:academyRoutingDescription('academy_dailyrecord_intake','Read qualified live DailyRecord before-state and prepare missing-only authoring interview. For name/today lookups prefer academy_dailyrecord_get; for explicit scope first resolve the name with academy_student_resolve. Requires context.scope (exact native teacher/student/date/namespace); every other context member is optional with the same defaults as the review (purpose "DailyRecord write", a new request ref, selected fields from suppliedValues or all six, memo answer from a supplied lessonMemo or no_addition, no facts). Never invent a memo answer or source codes. Configured owner backends are called live; sourceAvailability=observed proves the read even if status=interview_required for later authoring. Saved academy_source_status does not describe this backend. The returned observationHandle is the same kind of string as authoringHandle.observationHandle of academy_dailyrecord_get; pass that string, not the authoringHandle object, to academy_dailyrecord_review. Observation handles are scoped to the exact student/date/request and remain usable for 60 minutes from observedAt; review and save read the record again, that fresh read must be within 5 minutes, and a changed current/previous target, field values, sourceRevision, wireDigest or transmissionState causes source_before_or_contract_drift. Use fresh intake then review immediately before save after stale_source or drift; a different student save alone does not invalidate an unchanged target. Caller context is not authority. No writes or sending.'),
    inputSchema:dailyRecordIntakeSchema,annotations,
  },async input=>result(await service.intake(input)));
  server.registerTool('academy_dailyrecord_review',{
    title:'Review exact DailyRecord field differences without writing',
    description:academyRoutingDescription('academy_dailyrecord_review','Minimal call: {observationHandle, proposals:[{field, value}]}; observationHandle is the string authoringHandle.observationHandle of academy_dailyrecord_get (not the authoringHandle object), or observationHandle of intake. Every other member is optional: context scope and request come from the handle, selectedFields from the proposals, factRefs are all facts of the field, facts default to none; decision is set when the current value is empty or blank, else replace (an explicit set on a filled field is read as replace; clear must be explicit); memoAnswer is the proposed lessonMemo value, or no_addition when lessonMemo is not proposed. Without a handle only text is reviewed and no patch is prepared. Kept blockers: unknown or stale before-state, drift, missing target (previousHomeworkAssessment on a first lesson), the parent safeguard, the field length, invalid code values, and a code value (attendance, dailyTestAssessment, previousHomeworkAssessment) that a supplied fact contradicts: explicit_user_source_option_contradicts_value when the explicitValue of a fact differs from the proposal, conflicting_explicit_user_source_options when facts differ. Other checks against supplied facts are row warnings, never blockers: a code value no fact gives (explicit_user_source_option_fact_required, also with no facts), and fragments, question counts, deadlines, completion and stage claims for fields with facts. status prepared means no blocker remains; then save with {reviewDigest, instruction}. learningEntries and teacherReported are accepted and ignored. A month-day fact deadline (11월 19일) supports YYYY-MM-DD of the lesson year, or of the next year when its month is earlier than the lesson month. Observation handles last 60 minutes and are invalid on scope/request or source/contract drift: run fresh intake and review, never reuse the old digest. questionCounts is for cardinal counts (8문항); 유형 10 is an ordinal and belongs in requiredFragments. Parent safeguard excludes internal language including 채점, 검사 미완료 and 교사 완료판정 with parent_internal_language_excluded; preserve the confirmed stage and use supported wording such as 오답까지 완료. Semantic equivalence needs teacher review. No rewrite, save, send, approval consumption or automatic retry.'),
    inputSchema:dailyRecordReviewSchema,annotations,
  },async input=>result(await service.review(input)));
  server.registerTool('academy_prestudy_get',{
    title:'Read prestudy video upload and review lists by student name',
    description:academyRoutingDescription('academy_prestudy_get','Read live prestudy video state for one exact student name: waiting (uploaded, awaiting teacher review) rows, completed rows and one observed summary page with AI score/star literals. Upload, list membership, AI score and teacher review are distinct; teacherJudgment is always null here and video URLs are never returned. Missing rows do not prove nonperformance. No writes, printing, sending or automatic retry.'),
    inputSchema:prestudyGetSchema,annotations,
  },async input=>result(await service.prestudyGet(input)));
  server.registerTool('academy_prestudy_report_get',{
    title:'Read the AI prestudy report for one exact uploaded-video record',
    description:academyRoutingDescription('academy_prestudy_report_get','Read server-rendered AI report sections for one exact student and prestudyKey returned by academy_prestudy_get. The source detail must match both student and record key before its analysis key is followed; sourceRecord includes original unit/date literals or null when absent. Optional sectionTitles selects exact titles in source order; view lists available, returned and missing titles. Missing requested titles return partial, ambiguous selected titles fail closed. Complete requested view is distinct from complete source report; neither proves video/concept coverage. Optional maxOutputBytes bounds successful UTF-8 JSON payload bytes, excluding the MCP envelope (default full 512 KiB, selected 32 KiB); oversized results fail without clipping. Consume structuredContent or text once, not both. Report text is teacher-only student content, never stored in memory, parent text or artifacts. No transcript is exposed or derived; AI scores are not teacher judgment and unlabeled score arrays must not be assigned by position. A report not ready stops as unavailable. No writes, generation, printing, sending or automatic retry.'),
    inputSchema:prestudyReportGetSchema,annotations,
  },async input=>result(await service.prestudyReportGet(input)));
  server.registerTool('academy_item_section_trace',{
    title:'Trace item labels to original textbook sections',
    description:academyRoutingDescription('academy_item_section_trace','Trace exact numeric item labels to the original textbook sections (activity titles such as required-examples, type-drill, skill-building or unit evaluation) whose observed items carry those labels, with item ordinal and original O/X per match. Scans the student registered book bindings section indexes; when the scan would exceed the bounded section limit it returns interview_required with book candidates for teacher selection (or pass a bookLabel). bookLabel takes the bound label or a loose name: brand (가우스, 다빈치 …), grade and term (5-1, 초5-1, 5학년 1학기, 중1-1) and volume (-2, 2권), e.g. 가우스 5-1-2; the answer names the bound label in bookSelection, and a name that agrees with several books or none returns interview_required with the candidate labels. A label matching several sections is returned with every match and listed in ambiguousLabels for teacher confirmation. This proves original-attempt lineage only: a generated wrong-answer clinic paper version, membership and printed order are not provided by the source and remain unqualified, and original scoring does not prove clinic solving or teacher completion. No writes, grading actions, printing, sending or automatic retry.'),
    inputSchema:itemSectionTraceSchema,annotations,
  },async input=>result(await service.itemSectionTrace(input)));
  server.registerTool('academy_textbook_section_results_get',{
    title:'Read textbook section grading by student name',
    description:academyRoutingDescription('academy_textbook_section_results_get','Read textbook section answer input and grading for one exact student name. The owner backend discovers the registered course/book bindings from the live progress page. An archived book that the live page no longer lists is read by its keys (specs/academy-mcp/direct-key-lookup.md): courseKey, bookNo and studentKey given together (the studentKey must be the named student\'s), or the active edu-academy-memory binding that bookLabel selects; its answer has bindingSource historical_key_registry, a wrong-answer-note index and the course-book reportSummary. Two-step book selection: a first call without bookLabel may return interview_required with book candidates when several books are registered; call again with a bookLabel. bookLabel takes the bound label or a loose name: brand (가우스, 다빈치 …), grade and term (5-1, 초5-1, 5학년 1학기, 중1-1) and volume (-2, 2권), e.g. 가우스 5-1-2; the answer names the bound label in bookSelection, and a name that agrees with several books or none returns interview_required with the candidate labels. With exact sourceTitles returns score and per-item O/X per title; without titles returns the observed source-title index for explicit selection. The index basis is the wrong-answer-note index merged with one observed student paper-search page for the exact student, so all-correct sections are listed too; paper-search coverage is one page under the source form period, not complete history, and indexBasis/coverage state the basis. A title absent from both observed sources returns not_observed_in_either_source, never a no-record claim. Missing or ambiguous bindings/titles stop without guessing. This establishes observed answer input and grading only, not clinic solving, teacher acceptance, printing or whole-book completion. No writes, grading actions, printing, sending or automatic retry.'),
    inputSchema:textbookSectionResultsGetSchema,annotations,
  },async input=>result(await service.textbookSectionsByName(input)));
  server.registerTool('academy_clinic_paper_status_get',{
    title:'Read clinic-eligible original sections by student name',
    description:academyRoutingDescription('academy_clinic_paper_status_get','Read the wrong-answer/similar clinic bundle for one exact student name: registered book discovery (two-step bookLabel selection like the textbook section read; bookLabel takes the bound label or a loose name: brand (가우스, 다빈치 …), grade and term (5-1, 초5-1, 5학년 1학기, 중1-1) and volume (-2, 2권), e.g. 가우스 5-1-2; the answer names the bound label in bookSelection, and a name that agrees with several books or none returns interview_required with the candidate labels.), then either the observed section title index or, with exact sourceTitles, each original section attempt with its wrong items. Wrong items mark clinic eligibility only. Clinic papers have no server score-input support: clinic solving and scoring are teacher-judged only, generatedMembership stays unqualified, and printed/handedOut/solved/scored/teacherReviewed are returned as explicit teacher-judgment slots. This does not prove clinic generation, printing, hand-out, solving, grading or teacher review, and original section O/X is not clinic evidence. Missing titles return not_observed_in_either_source, not proof of no record. No writes, generation, printing, sending or automatic retry.'),
    inputSchema:clinicPaperStatusGetSchema,annotations,
  },async input=>result(await service.clinicPaperStatus(input)));
  server.registerTool('academy_dailyrecord_save',{
    title:'Save reviewed DailyRecord fields through the owner write backend',
    description:academyRoutingDescription('academy_dailyrecord_save','Save only an exact reviewed patch. Minimal call: {reviewDigest, instruction} where instruction is the user\'s verbatim words (a string or {text}); the handle, context and proposals are loaded from the prepared review of that digest. The full form (context, observationHandle, proposals, request) stays accepted. With the owner workflow (default for native launches), any delegated session may save when it supplies the user\'s non-empty instruction and a prior academy_dailyrecord_review for this reviewDigest exists in the owner store. The bridge records the observed caller attribution and the caller may declare actor {runtime, sessionRef, paneRef, model}; both are audit evidence, not credentials. Writers serialize on a shared owner lock, the review is recomputed against a fresh before-state, each exact request/field transition is recorded once before dispatch, written once and read back. Duplicates report the recorded outcome; an unresolved dispatch on the same record field under any request is held for academy_dailyrecord_reconcile and never retried. A requestRef binds the first instruction text byte-for-byte: reuse that exact instruction on recovery; different text returns request_instruction_conflict. Do not change a requestRef to bypass an uncertain dispatch. The observation must be within 60 minutes and match the fresh current/previous targets, fields and source contract; stale_source/source_before_or_contract_drift require fresh intake and a new recorded review/digest before save. Parent safeguard still blocks 채점 and other internal language. Parent transmission is never part of a save.'),
    inputSchema:dailyRecordSaveSchema,annotations:{readOnlyHint:false,destructiveHint:true,idempotentHint:false,openWorldHint:service.backend!==null},
  },async input=>result(await service.save(input)));
  server.registerTool('academy_dailyrecord_reconcile',{
    title:'Reconcile uncertain DailyRecord dispatches by authoritative readback',
    description:academyRoutingDescription('academy_dailyrecord_reconcile','For one exact student/date record, read the current source once under the shared owner lock and classify every unresolved dispatch, under any request, as persisted, not_persisted or conflict. The new value closes a dispatch at once; the old or another value closes it only after a returned transport, because an uncertain request may still land at any later time. To close such an uncertain dispatch as not landed, pass resolution.request with the verbatim user instruction for the current requestRef and resolution.executions naming each {requestRef, field} to close on this exact lesson record (as listed by academy_dailyrecord_execution_status); it is recorded with who reconciled. Never writes to the academy, retries, rolls back or sends; a not_persisted or conflict field needs a new user request and review.'),
    inputSchema:dailyRecordReconcileSchema,annotations:{...annotations,idempotentHint:false},
  },async input=>result(await service.reconcile(input)));
  server.registerTool('academy_dailyrecord_execution_status',{
    title:'Show durable DailyRecord execution state and attribution for handoff',
    description:academyRoutingDescription('academy_dailyrecord_execution_status','Metadata-only handoff view of one exact student/date record: per-field executions with request, state and who performed them, plus the request record and append-only audit when requestRef is given. Field and instruction text are reported only as digests. No source read, write or send.'),
    inputSchema:dailyRecordExecutionStatusSchema,annotations:{...annotations,openWorldHint:false},
  },async input=>result(await service.executionStatus(input)));
}

/** academy_official_report_pdf_get on the dailyrecord endpoint (src/dailyrecord-server.ts): one
 * official StudyQ report PDF from read-only LMS data, written under the owner output root. */
export function registerOfficialReportPdfTool(server:McpServer,service:OfficialReportPdfService){
  const result=(value:unknown)=>{const structuredContent=JSON.parse(JSON.stringify(value)) as Record<string,unknown>;
    return {content:[{type:'text' as const,text:JSON.stringify(structuredContent)}],structuredContent};};
  server.registerTool('academy_official_report_pdf_get',{
    title:'Render one official StudyQ report PDF from read-only LMS data',
    description:academyRoutingDescription('academy_official_report_pdf_get','reportType course_book: the official course-book report of one student and book, read by getCourseBookReport and rendered on this host by the pinned studyq-linux executor, then checked by its parity run (parityOk, parityMismatches, parityRawRn). The book: courseKey, bookNo and studentKey together, or params.bookLabel (live book list first, then the active edu-academy-memory binding, as academy_textbook_section_results_get chooses it). The PDF is written under the owner output root (outputPath optional, .pdf, inside that root; never overwritten). No Academy write, printing or sending.'),
    inputSchema:officialReportPdfGetSchema,annotations:{readOnlyHint:false,destructiveHint:false,idempotentHint:false,openWorldHint:service.backend!==null},
  },async input=>result(await service.get(input)));
}

/** Launch-time owner-selected definition bytes; never accept policy paths or code in a tool call. */
export async function dailyRecordToolConfiguration(args:readonly string[]):Promise<{remainingArgs:string[];parentSafeguard:DailyRecordParentSafeguard|null;saveAdmission:DailyRecordSaveAdmission|null;saveStoreDir:string|null;saveRequestPath:string|null}>{
  const remainingArgs:string[]=[];let parentSafeguard:DailyRecordParentSafeguard|null=null,saveAdmission:DailyRecordSaveAdmission|null=null,saveStoreDir:string|null=null,saveRequestPath:string|null=null;
  for(let i=0;i<args.length;i++){
    if(args[i]==='--dailyrecord-save-request'){
      if(saveRequestPath||i+1>=args.length||!args[i+1]!.startsWith('/'))throw Error('dailyrecord_save_request_arguments_invalid');
      saveRequestPath=args[i+1]!;i+=1;continue;
    }
    if(args[i]==='--dailyrecord-save-store'){
      if(saveStoreDir||i+1>=args.length||!args[i+1]!.startsWith('/'))throw Error('dailyrecord_save_store_arguments_invalid');
      saveStoreDir=args[i+1]!;i+=1;continue;
    }
    if(args[i]==='--dailyrecord-save-admission'){
      if(saveAdmission||i+2>=args.length)throw Error('dailyrecord_save_admission_arguments_invalid');
      const pinned=await readPinnedLocalBytes(args[i+1],args[i+2]);
      saveAdmission=dailyRecordSaveAdmissionSchema.parse(JSON.parse(pinned.bytes.toString('utf8')));i+=2;continue;
    }
    // An installed launch may still pass its old read admissions (path and sha256).
    // They are accepted and gate nothing.
    if(args[i]==='--metadata-admission'||args[i]==='--dailyrecord-admission'){
      if(i+2>=args.length)throw Error(args[i]==='--metadata-admission'?'metadata_admission_arguments_invalid':'dailyrecord_admission_arguments_invalid');
      i+=2;continue;
    }
    if(args[i]!=='--dailyrecord-policy'){remainingArgs.push(args[i]);continue;}
    if(parentSafeguard||i+4>=args.length)throw Error('dailyrecord_policy_arguments_invalid');
    const [policyPath,policySha256,safeguardPath,safeguardSourceSha256]=args.slice(i+1,i+5);
    const policy=await readPinnedLocalBytes(policyPath,policySha256),source=await readPinnedLocalBytes(safeguardPath,safeguardSourceSha256);
    if(!policy.bytes.toString('utf8').includes('# DailyRecord authoring policy'))throw Error('dailyrecord_policy_owner_required');
    // Read only the canonical literal. Never import/evaluate source code or execute instructions in text.
    const matches=[...source.bytes.toString('utf8').matchAll(/^export const parentInternalLanguage=\/(.+)\/([imsu]*);$/gm)];
    if(matches.length!==1)throw Error('canonical_parent_safeguard_definition_unqualified');
    const lineMatches=[...source.bytes.toString('utf8').matchAll(/^export const parentDailyRecordLines=\/(.+)\/([imsu]*);$/gm)];
    if(lineMatches.length!==1)throw Error('canonical_parent_line_definition_unqualified');
    const regex=new RegExp(matches[0][1],matches[0][2]),lines=new RegExp(lineMatches[0][1],lineMatches[0][2]);
    parentSafeguard={policySha256,safeguardSourceSha256,sourceRef:'Education:DailyRecord:parentInternalLanguage',check:text=>regex.test(text),checkLines:text=>lines.test(text)};
    i+=4;
  }
  return {remainingArgs,parentSafeguard,saveAdmission,saveStoreDir,saveRequestPath};
}
