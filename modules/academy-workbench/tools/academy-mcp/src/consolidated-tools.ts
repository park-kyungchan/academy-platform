
/** The public tools of `academy_operations` (specs/academy-mcp/tool-surface.md): the 15 of the consolidation and the new tools built since.
 * Eight are bridge compositions: each routes its discriminating argument (`view`,
 * `scope`, `include`, `includeHeaders`) to the existing endpoint tools, which stay
 * in the registry as hidden routes. Seven are endpoint tools kept under their own
 * names. `release/surface.json` is built from this module by scripts/build-surface.ts
 * and never edited by hand. This module imports nothing, so the build and the install script
 * `install/composition_entries.ts` run without the module's dependencies. */

type Json={[key:string]:unknown};
const str:Json={type:'string'},int:Json={type:'integer'},bool:Json={type:'boolean'},obj:Json={type:'object'},arr:Json={type:'array'};
const strs:Json={type:'array',items:str},num:Json={type:'number'};
const oneOf=(...values:string[]):Json=>({type:'string',enum:values});
const sha:Json={type:'string',pattern:'^[a-f0-9]{64}$'};
const fieldNames=['progress','homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment'];
const fieldList:Json={type:'array',items:oneOf(...fieldNames)};
const periods:Json={type:'array',items:oneOf('11','12','13','14','15','16')};
/** Displayed schemas name the members and their allowed values compactly. What a call may hold
 * is decided by the registry schema: a composition's (registrySchema) refuses an unknown member,
 * planConsolidatedCall refuses a member that none of the chosen routes takes, and each route's
 * own registry schema validates its arguments; a kept tool's registry schema is its endpoint's. */
const schema=(properties:Json,required:string[]=[]):Json=>({type:'object',properties,...(required.length?{required}:{})});

/** A step of a consolidated call: one hidden endpoint tool and the arguments it takes. */
export interface Route{
  /** The endpoint tool (its upstream name), or a bridge-served read. */
  upstream:string;
  /** Endpoints to look for the upstream tool in, in order. */
  endpoints:readonly string[];
  /** Public argument names this route takes; `rename` maps a public name to the upstream name. */
  keys:readonly string[];
  rename?:Readonly<Record<string,string>>;
  /** A bridge-served read: the runtime identity, or one of the bridge's own two compositions. */
  bridge?:'identity'|'dailyrecord_record'|'dailytest_headers';
  /** Members the registry schema requires when this route is chosen (refused at input validation). */
  required?:readonly string[];
}
export interface ConsolidatedTool{
  name:string;title:string;description:string;
  /** The displayed schema of the surface; registrySchema() adds the members its routes take. */
  inputSchema:Json;
  /** The endpoint written in the composition's registry entry. */
  endpoint:string;
  /** The discriminating arguments (R5 validates only these). */
  selectors:readonly string[];
  /** The route chosen when the first selector is absent. */
  defaultRoute?:string;
  routes:Readonly<Record<string,Route>>;
}

const dr=['dailyrecord'],paper=['paper1','paper2'];
/** The audit members that the DailyRecord workflow reads take: a declared actor, and an attribution the bridge replaces. */
const audit=['actor','attribution'];
const intakeKeys=['context','scope','sourceNamespace','teacherId','studentId','lessonDate','schemaVersion','purpose','currentRequestRef','selectedFields','memoAnswer','facts','fields','suppliedValues',...audit];

export const consolidatedTools:readonly ConsolidatedTool[]=[
 {name:'academy_runtime_inspect',title:'MCP release·업무·원천 계약',endpoint:'dailyrecord',selectors:['view'],
  description:"This server; no student rows. view: 'identity' = release 세대, pointer, 브리지 해시, surfaceSha256; no other argument. 'capabilities' = work it knows (이 MCP로 할 수 있는 일), no LMS request: capabilityView 'hierarchy' (default), 'operations' (offset, limit), 'source_contract' with operationId, or 'workflow_rules'; workflow, domain filter. 'source_contracts' = one LMS page's static program, not run: kind 'printed_code' (출력코드 변환기), 'daily_test_catalog', 'middle1_definitions' or 'elementary5_fraction_definitions' (unitCodes, unitNames optional). 'paper_contracts' = form fields and functions of the paper search and TestPool pages (시험 검색 폼, 읽기 계약). Contract views need studentId (studentKey). Read only.",
  inputSchema:schema({view:oneOf('identity','capabilities','source_contracts','paper_contracts'),capabilityView:oneOf('hierarchy','operations','source_contract','workflow_rules'),
   workflow:str,domain:str,operationId:str,offset:int,limit:int,studentId:str,kind:oneOf('printed_code','daily_test_catalog','middle1_definitions','elementary5_fraction_definitions'),
   unitCodes:strs,unitNames:obj},['view']),
  routes:{
   identity:{upstream:'academy_runtime_identity',endpoints:[],keys:[],bridge:'identity'},
   capabilities:{upstream:'academy_capabilities',endpoints:['saved','dailyrecord'],keys:['capabilityView','workflow','domain','operationId','offset','limit'],rename:{capabilityView:'view'}},
   source_contracts:{upstream:'academy_source_read_contracts',endpoints:paper,keys:['studentId','kind','unitCodes','unitNames']},
   paper_contracts:{upstream:'academy_paper_read_contracts',endpoints:paper,keys:['studentId']},
  }},
 {name:'academy_dailyrecord_get',title:'DailyRecord(수업일지) 조회',endpoint:'dailyrecord',selectors:['view'],defaultRoute:'record',
  description:"DailyRecord (수업일지, 일지) reads; no LMS write. view 'record' (default): one student's live record of one date: fields[] (field, before.value, code label before.label; progress 진도, homework 숙제, lessonMemo 메모, attendance 출결, DT and homework assessments), assessmentCodeLabels, scope, authoringHandle. For every 일지 or 기록 조회 and a save's readback. displayName: the name as written. lessonDate: omit for today, else YYYY-MM-DD, 오늘, 어제, 11월 16일 or 11/16 (Asia/Seoul). fields: names to return (default all six). detail 'compact' drops authoringHandle. student_name_not_found: no record that date. 'intake': before-state of a scope with observationHandle and interviews; only when record gave authoringHandle null, or after stale_source. scope (from record) in context.scope or top-level (sourceNamespace, teacherId, studentId, lessonDate); context members (schemaVersion, purpose, currentRequestRef, selectedFields, memoAnswer, facts) may be top-level, given once (context_member_conflict); fields, suppliedValues. 'status': save attempts of scope (저장 실행 상태): per-field executions, requestRef, state, who; no LMS request; requestRef adds its audit. 'reconcile': reads the LMS once and classes each unresolved save of the record persisted, not_persisted or conflict (연결 끊김, 들어갔는지 확인); context as for review. To close one as not landed: resolution {request: {requestRef, instruction: {text: the user's words}}, executions: [{requestRef, field}] from 'status'}; not_persisted needs a new review and save. Writes: academy_dailyrecord_review.",
  inputSchema:schema({view:oneOf('record','intake','status','reconcile'),detail:oneOf('full','compact'),displayName:str,purpose:str,currentRequestRef:str,lessonDate:str,
   fields:fieldList,context:obj,scope:obj,requestRef:str,resolution:obj}),
  routes:{
   record:{upstream:'academy_dailyrecord_get',endpoints:dr,keys:['displayName','purpose','currentRequestRef','lessonDate','fields','detail'],rename:{detail:'view'},bridge:'dailyrecord_record',required:['displayName']},
   intake:{upstream:'academy_dailyrecord_intake',endpoints:dr,keys:intakeKeys},
   status:{upstream:'academy_dailyrecord_execution_status',endpoints:dr,keys:['scope','requestRef',...audit]},
   reconcile:{upstream:'academy_dailyrecord_reconcile',endpoints:dr,keys:['context','resolution',...audit]},
  }},
 {name:'academy_dailytest_history',title:'DailyTest(DT) 이력',endpoint:'dailyrecord',selectors:['includeHeaders'],
  description:"A student's DailyTest (DT) history: DT 이력, 시험 기록, 몇 개 쳤는지. periods: '11' = last month, '12' = 1-2 months ago … '16' = 5-6 months ago; all six for 전부. includeHeaders true (default): with paper titles, question counts (periods[].rows[].header.title, sourcePaperCount); needs studentName, periods; lessonDate: a date the student had a lesson (YYYY-MM-DD, 오늘, 어제, 11월 16일 or 11/16; default today); 32 papers per call; next: headerOffset = nextHeaderOffset, expectedSelectionSha256 = selectionSha256. native_dailyrecord_occurrence_unverified: no lesson that day; use includeHeaders false. includeHeaders false: DT 결과 목록, rows (paper key, attempt, score) per period; takes studentId (studentKey) or resolves studentName (studentResolution); without periods only the current filter (often empty). Read only.",
  inputSchema:schema({includeHeaders:bool,studentName:str,studentId:str,lessonDate:str,periods,maxHeaders:int,headerOffset:int,expectedSelectionSha256:str,purpose:str,currentRequestRef:str}),
  routes:{
   headers:{upstream:'academy_student_metadata_get',endpoints:dr,keys:['displayName','lessonDate','purpose','currentRequestRef','periods','maxHeaders','headerOffset','expectedSelectionSha256'],bridge:'dailytest_headers'},
   inventory:{upstream:'academy_daily_test_inventory',endpoints:paper,keys:['studentId','periods']},
   resolve:{upstream:'academy_student_resolve',endpoints:dr,keys:['displayName','lessonDate','purpose','currentRequestRef']},
  }},
 {name:'academy_paper_inspect',title:'출력코드 시험지',endpoint:'paper1',selectors:['view'],
  description:"Printed papers (출력코드 #…). studentId: studentKey of academy_student_resolve. view: 'headers' = titles of up to 32 paperCodes, each with '#' (시험지 제목, 뭐였는지): observation.rows[].contentHeader.metadataRows, label 시험지 제목 = the title. 'definition' = one paperCode's 시험지 이름 and 문항 수: observation.information (title, questionCount, category, level); definitionView 'definition' (default) or 'header'; not whether the student took it. 'attempt' = whether and how the student took one paperCode in period {from, through} (YYYY-MM-DD): observation.status, observation.rows[] (sourceScore); not-found covers the searched page only. Read only.",
  inputSchema:schema({view:oneOf('headers','definition','attempt'),studentId:str,paperCode:str,paperCodes:strs,definitionView:oneOf('definition','header'),
   period:{type:'object',properties:{from:str,through:str}}},['view','studentId']),
  routes:{
   headers:{upstream:'academy_paper_headers',endpoints:paper,keys:['studentId','paperCodes']},
   definition:{upstream:'academy_paper_definition_lookup',endpoints:paper,keys:['studentId','paperCode','definitionView'],rename:{definitionView:'view'}},
   attempt:{upstream:'academy_printed_paper_lookup',endpoints:paper,keys:['studentId','paperCode','period']},
  }},
 {name:'academy_item_lookup',title:'문항 번호 → 교재 섹션·정답',endpoint:'dailyrecord',selectors:['include'],
  description:"Numeric item labels (문항 번호, lectureKey): the digits given or lectureKey of an earlier result (wrongItems, items, matches), max 32. include: 'section' = the original textbook section: items[].matches[] (sourceTitle, ordinal, O/X), ambiguousLabels; needs displayName; with several books pass bookLabel (a bindings[].bookLabel or the user's book words such as 가우스 5-1-2; bookSelection names it): scanning all books can time out. 'answer' = the official answer text, no student: answers[].answerLiteral '<prefix>|_|<answer>' with source unit and pattern names; quote the answer part, escape '|' in Markdown tables; view 'compact'. Both: {section, answer}. Read only.",
  inputSchema:schema({include:{type:'array',items:oneOf('section','answer')},labels:strs,displayName:str,bookLabel:str,view:oneOf('full','compact'),purpose:str,currentRequestRef:str},['include','labels']),
  routes:{
   section:{upstream:'academy_item_section_trace',endpoints:dr,keys:['displayName','purpose','currentRequestRef','labels','bookLabel']},
   answer:{upstream:'academy_item_answer_lookup',endpoints:dr,keys:['labels','purpose','currentRequestRef','view']},
  }},
 {name:'academy_textbook_catalog_get',title:'교재 목차·카탈로그·PDF',endpoint:'dailyrecord',selectors:['view'],
  description:"Textbooks. view: 'toc' first for a book's units, pages and chapter assessments (대단원 총괄평가), from kept data: no LMS request, no PDF; book: 가우스 5-1-2, 초5-1 가우스 2권, 가우스 5학년 1학기 2권 or the LMS bound-book label (no volume: every volume); unit: '6' or '6-5'; not_found lists names. 'catalog' = the LMS textbook and workbook PDF catalog (교재 카탈로그, 답지): entries[] (sourceLabels, kind sample/answer …, filename, documentRef); schoolLevel elementary 초등, middle 중등 (default), high 고등; query: book name words; documentKind; offset, limit. 'pdf' = one catalog PDF saved locally: sha256 and path, not text; documentRef, schoolLevel from catalog entries[]. Read only.",
  inputSchema:schema({view:oneOf('toc','catalog','pdf'),book:str,unit:str,schoolLevel:oneOf('elementary','middle','high'),
   documentKind:oneOf('sample','answer','repair','daily','other','all'),query:str,offset:int,limit:int,documentRef:str},['view']),
  routes:{
   toc:{upstream:'academy_textbook_toc_get',endpoints:dr,keys:['book','unit']},
   catalog:{upstream:'academy_textbook_catalog',endpoints:dr,keys:['schoolLevel','documentKind','query','offset','limit']},
   pdf:{upstream:'academy_textbook_pdf',endpoints:dr,keys:['schoolLevel','documentRef']},
  }},
 {name:'academy_prestudy_get',title:'예습영상·AI 리포트',endpoint:'dailyrecord',selectors:['view','includeReports','reportId'],
  description:"A student's prestudy videos (예습영상); displayName: the name as written. view (default 'videos'): 'videos' = waiting[] (uploaded, 검사 대기), completed[] (unitLabel, dateLiteral, prestudyKey) and summary[] (AI score literals: 예습 점수); 'incomplete' = waiting[] only; 'teacher_summary' = summary[] only. Empty summary: no AI score shown; say so, never derive one. includeReports true with reportId = a prestudyKey from completed[] or waiting[] (최근: the latest dateLiteral) gives that video's AI report: sections[] (title, text blocks), view.returnedSectionTitles; sectionTitles narrows it; maxOutputBytes. Without view only the report is read. One of includeReports, reportId without the other is refused. Read only.",
  inputSchema:schema({displayName:str,view:oneOf('videos','incomplete','teacher_summary'),includeReports:bool,reportId:str,sectionTitles:strs,maxOutputBytes:int,
   purpose:str,currentRequestRef:str},['displayName']),
  routes:{
   videos:{upstream:'academy_prestudy_get',endpoints:dr,keys:['displayName','view','purpose','currentRequestRef']},
   report:{upstream:'academy_prestudy_report_get',endpoints:dr,keys:['displayName','reportId','sectionTitles','maxOutputBytes','purpose','currentRequestRef'],rename:{reportId:'prestudyKey'}},
  }},
 {name:'academy_cism_get',title:'주간·월별 CISM',endpoint:'dailyrecord',selectors:['scope'],
  description:"CISM reads. scope: 'week' = the week (Sunday to Saturday) containing date: report.reportSha256, report.fields[] (fieldRef, label, kind, value, studentName); step 1 of a CISM write (get, review, save, get) and the readback after a save; date: YYYY-MM-DD, 오늘, 어제, 11월 16일 or 11/16 (Asia/Seoul), the answer states it. 'month' = every week touching month (YYYY-MM): reports[] (weekLabel, fields with values) and emptyFields, the empty field count (월별 CISM, 빈 칸 몇 개). Read only.",
  inputSchema:schema({scope:oneOf('week','month'),date:str,month:str},['scope']),
  routes:{
   week:{upstream:'academy_cism_weekly_get',endpoints:dr,keys:['date']},
   month:{upstream:'academy_cism_month',endpoints:dr,keys:['month']},
  }},
];

const book=schema({displayName:str,bookLabel:str,sourceTitles:strs,view:oneOf('full','compact'),purpose:str,currentRequestRef:str},['displayName']);
/** The section read also takes the three keys of an archived book (specs/academy-mcp/direct-key-lookup.md R2, R6); no pattern is shown, the registry schema checks them. */
const textbookBook=schema({displayName:str,bookLabel:str,sourceTitles:strs,view:oneOf('full','compact'),purpose:str,currentRequestRef:str,courseKey:str,bookNo:str,studentKey:str},['displayName']);

/** The endpoint tools kept under their own names, as the surface shows them. */
export const keptTools:readonly {name:string;title:string;description:string;inputSchema:Json}[]=[
 {name:'academy_dailyrecord_review',title:'DailyRecord 문안 검토',
  description:"Step 2 of a DailyRecord write (get, review, save, get): prepared with reviewDigest, or held with blockers; no write. Minimal call: {observationHandle: the string authoringHandle.observationHandle of academy_dailyrecord_get (60 minutes), proposals: [{field, value}]}. 메모/코멘트 = lessonMemo, 진도 = progress, 숙제 = homework. set or replace follows the current value. Code fields (attendance, dailyTestAssessment, previousHomeworkAssessment) only when the user asks for that field. Deadline line '- YYYY-MM-DD까지'. context.memoAnswer {kind: not_supplied, no_addition or content, text}: no_addition needs no requestRef; with a lessonMemo proposal it holds the row (no_addition_preserves_existing_memo). Show rows[].warnings to the user; if held, fix the blocker or ask.",
  inputSchema:schema({observationHandle:str,proposals:{type:'array',items:{type:'object',properties:{field:oneOf(...fieldNames),value:str,
   decision:{type:'object',properties:{kind:oneOf('set','replace','clear'),requestRef:str}}},required:['field','value']}},context:obj,cismObservationHandle:str},['proposals'])},
 {name:'academy_dailyrecord_save',title:'DailyRecord 저장(LMS 쓰기)',
  description:"Step 3: writes the reviewed fields to the LMS DailyRecord and reads them back: status saved, or held with reasonCode. Only after academy_dailyrecord_review returned prepared and the user asked to save. Minimal call: {reviewDigest, instruction: the user's message verbatim}. context.memoAnswer {kind: no_addition} needs no requestRef. Then read back with academy_dailyrecord_get. stale_source or drift: get, review, save again. A held uncertain dispatch: no retry; see academy_dailyrecord_get view 'status'.",
  inputSchema:schema({reviewDigest:sha,instruction:{anyOf:[str,obj]},request:obj,context:obj,observationHandle:str,cismObservationHandle:str,proposals:arr},['reviewDigest'])},
 {name:'academy_student_resolve',title:'학생 키(studentKey) 확인',
  description:"Returns studentKey, the studentId of academy_paper_inspect, academy_runtime_inspect contract views and academy_dailytest_history (includeHeaders false); other tools take the name. Without lessonDate it searches recent records only; on student_name_not_found call once more with lessonDate = a date the student had a lesson (YYYY-MM-DD, 오늘, 어제, 11월 16일 or 11/16, Asia/Seoul), else ask the user for one. scope.studentId of academy_dailyrecord_get is the same key. Read only.",
  inputSchema:schema({displayName:str,lessonDate:str,purpose:str,currentRequestRef:str},['displayName'])},
 {name:'academy_textbook_section_results_get',title:'교재 소단원 채점 결과(O/X)',
  description:"A student's textbook section results (교재 소단원, 필수예제, 유형다지기: score and O/X). Up to three calls, same displayName: 1) no bookLabel: with several books, interview_required, bindings[].bookLabel; 2) bookLabel = one of those labels or the user's book words as written (가우스 5-1-2, 다빈치 2권): index[].sourceTitle, courseKey, bookNo, bookSelection.boundLabel; words fitting several books or none give interview_required with candidate labels; 3) add sourceTitles = exact titles of that index: records[] with score and items[] (O/X, lectureKey). With book words start at 2; stop when answered. '가우스 5-1-2' is one book (grade 5, semester 1, volume 2: 초5-1 가우스 2권), not sections 5-1 or 5-2. Archived book: all of courseKey, bookNo, studentKey. Read only.",
  inputSchema:textbookBook},
 {name:'academy_clinic_paper_status_get',title:'오답·유사클리닉 대상',
  description:"Clinic candidates (오답클리닉, 유사클리닉): sections whose original attempt has wrong items, sections[].wrongItems[] (ordinal = item number, lectureKey). For 틀린 문항 and 오답 of a book or section. Steps as academy_textbook_section_results_get: no bookLabel gives bindings[].bookLabel; bookLabel (a label or the user's book words) gives sections[].sourceTitle and bookSelection; sourceTitles gives wrongItems. Clinic paper solving is not in the LMS: the teacher confirms it. Read only.",
  inputSchema:book},
 {name:'academy_cism_weekly_review',title:'주간 CISM 문안 검토',
  description:"Step 2 of a CISM write: checks proposed text against the current week and records the review: status reviewed, reviewDigest; no LMS write. date: as in the get. expectedReportSha256: report.reportSha256 of academy_cism_get scope 'week'. request: {requestRef: your id, instruction: the user's message verbatim}. changes[]: {fieldRef from report.fields[], before = its current value ('' if empty), after = new text from same-week DailyRecords or the user, evidenceRefs = the dated sources}. Only empty fields are filled unless edit is complete_existing_narrative. Never invent numbers.",
  inputSchema:schema({date:str,expectedReportSha256:sha,request:schema({requestRef:str,instruction:str},['requestRef','instruction']),
   changes:{type:'array',items:schema({fieldRef:str,before:str,after:str,edit:oneOf('fill_empty','complete_existing_narrative'),evidenceRefs:strs},['fieldRef','before','after','evidenceRefs'])}},
   ['date','expectedReportSha256','request','changes'])},
 {name:'academy_cism_weekly_save',title:'주간 CISM 저장(LMS 쓰기)',
  description:"Step 3: writes the reviewed CISM fields to the LMS, one request per change, read back: status persisted. Only after academy_cism_weekly_review returned reviewed. date, expectedReportSha256, request, changes: exactly as in that review; reviewDigest: from it. Then confirm with academy_cism_get scope 'week'. On a failure stop: never repeat the save for a field.",
  inputSchema:schema({date:str,expectedReportSha256:sha,request:obj,changes:arr,reviewDigest:sha},['date','expectedReportSha256','request','changes','reviewDigest'])},
 // The first new tool of specs/academy-mcp/tool-surface.md "New tools". `params` holds the options of a report type,
 // so a later report type adds a reportType value and its own params, not new members.
 {name:'academy_official_report_pdf_get',title:'공식 리포트 PDF',
  description:"Official StudyQ PDF (공식·교재 리포트), read-only LMS. reportType course_book: displayName; courseKey+bookNo+studentKey or params.bookLabel. cism: params.scope week+date|month. diag/formative/ganga/na_score: displayName+params.testingNo+classNo. smart_book: displayName+params.bookCode. new_diag: params.paperNo+applyCode. Saved under owner root (no overwrite); answers outputPath, sha256, pages, parityOk.",
  inputSchema:schema({reportType:oneOf('course_book','cism','diag','new_diag','formative','ganga','na_score','smart_book'),displayName:str,studentName:str,courseKey:str,bookNo:str,studentKey:str,params:obj,outputPath:str},['reportType'])},
 // The second new tool: the printable PDF of an existing paper (CreateTotalPrint, CreateClinic), read-only.
 {name:'academy_paper_print_pdf_get',title:'시험지·클리닉지 인쇄 PDF',
  description:"Printable PDF (시험지·클리닉지) of an existing paper. mode 'exam': paperNo (its exam list) or paperKeys (selType default obj); 'examtext': passage paperKeys; 'clinic': clinic print of either. includeAnswers, includeExplain add pages. Saved under owner root (outputPath optional); answers outputPath, sha256. Read only.",
  inputSchema:schema({mode:oneOf('exam','examtext','clinic'),paperNo:str,paperKeys:strs,selType:oneOf('obj','sbj','des'),includeAnswers:bool,includeExplain:bool,outputPath:str},['mode'])},
 // The third new tool: the read-only attendance, study schedule and course study manager pages (specs/academy-mcp/lms-reads.md).
 {name:'academy_lms_schedule_get',title:'출결·학습일정·과정관리',
  description:"Read-only LMS page. view: attendance (date or period, YYYY-MM-DD, ≤31 days, same month), study_schedule (gradeNo, groupKey, studentKey, courseKey), course_study_manager (gradeNo, semesterNo, levelNo, status).",
  inputSchema:schema({view:oneOf('attendance','study_schedule','course_study_manager'),date:str,period:obj,groupKey:str,studentKey:str,gradeNo:str,semesterNo:str,levelNo:str,courseKey:str,status:str},['view'])},
 // The fourth and fifth new tools: the two phases of the gated wrong-note and clinic paper creation (specs/academy-mcp/gated-writes.md).
 {name:'academy_clinic_paper_review',title:'오답·클리닉지 검토',
  description:"Read-only review of a wrong-note (book_incorrect) or clinic paper; clinic modes multiply/divide/custom. Answers change, reviewToken.",
  inputSchema:schema({kind:oneOf('book_incorrect','clinic'),studentKey:str,courseKey:str,bookNo:str,testingNo:str,testingNos:strs,clinicMode:oneOf('multiply','divide','custom'),createSimilar:bool,similarCount:num,createAdvance:bool,advanceCount:num,patList:arr},['kind','studentKey'])},
 {name:'academy_clinic_paper_save',title:'오답·클리닉지 저장(LMS 쓰기)',
  description:"Creates the reviewed paper once, only after the teacher approves: reviewToken, userApproved true, the reviewed change. saved after readback, else unverified: no retry.",
  inputSchema:schema({reviewToken:sha,userApproved:bool,change:obj},['reviewToken','userApproved','change'])},
 // The sixth and seventh new tools: the two phases of the gated FA and unit-exam creation and assignment (specs/academy-mcp/gated-writes.md).
 {name:'academy_exam_paper_review',title:'시험지 출제·배정 검토',
  description:"Read-only review of an FA (fa: sourcePaperNo or items {key,type obj|sbj|des}) or unit-exam (da: courseNo) paper for named students (targets {studentName,studentKey}, start/end YYYY-MM-DDTHH:MM). Answers change, preview, reviewToken.",
  inputSchema:schema({family:oneOf('fa','da'),testName:str,testgubunNo:int,sourcePaperNo:str,items:arr,courseNo:int,targets:arr,start:str,end:str,testingMinutes:int},['family','testName','testgubunNo','targets','start','end','testingMinutes'])},
 {name:'academy_exam_paper_save',title:'시험지 출제·배정 저장(LMS 쓰기)',
  description:"Creates and assigns the reviewed exam once, only after the teacher approves: reviewToken, userApproved true, the reviewed change. saved after readback, else unverified: no retry.",
  inputSchema:schema({reviewToken:sha,userApproved:bool,change:obj},['reviewToken','userApproved','change'])},
];

/** Surface order: runtime, DailyRecord, student, DailyTest and paper, textbook, items and clinic, prestudy, CISM; then the new tools. */
export const baselineSurfaceOrder=['academy_runtime_inspect','academy_dailyrecord_get','academy_dailyrecord_review','academy_dailyrecord_save','academy_student_resolve',
 'academy_dailytest_history','academy_paper_inspect','academy_textbook_section_results_get','academy_clinic_paper_status_get','academy_item_lookup',
 'academy_textbook_catalog_get','academy_prestudy_get','academy_cism_get','academy_cism_weekly_review','academy_cism_weekly_save'] as const;
/** The new tools of Waves 3 to 5 already built (specs/academy-mcp/tool-surface.md R2). */
export const newSurfaceTools=['academy_official_report_pdf_get','academy_paper_print_pdf_get','academy_lms_schedule_get','academy_clinic_paper_review','academy_clinic_paper_save','academy_exam_paper_review','academy_exam_paper_save'] as const;
export const surfaceOrder=[...baselineSurfaceOrder,...newSurfaceTools] as const;

/** `release/surface.json` as built from this module. */
export function surfaceDocument(){
  const all=new Map<string,{name:string;title:string;description:string;inputSchema:Json}>([
    ...consolidatedTools.map(t=>[t.name,{name:t.name,title:t.title,description:t.description,inputSchema:t.inputSchema}] as const),
    ...keptTools.map(t=>[t.name,t] as const)]);
  if(all.size!==surfaceOrder.length||surfaceOrder.some(name=>!all.has(name)))throw Error('surface_definition_incomplete');
  return {schemaVersion:'academy.mcp-surface/1' as const,tools:surfaceOrder.map(name=>{const t=all.get(name)!;return {name:t.name,title:t.title,description:t.description,inputSchema:t.inputSchema};})};
}

export const consolidatedNames=new Set(consolidatedTools.map(t=>t.name));
/** Composition upstream names that no endpoint serves (the endpoint inventory skips them).
 * `academy_prestudy_get` is also the dailyrecord endpoint's own tool, so it is checked. */
export const compositionUpstreams=new Set([...consolidatedNames].filter(name=>name!=='academy_prestudy_get'));
export function consolidatedTool(name:string){return consolidatedTools.find(t=>t.name===name);}
/** A registry entry is a consolidated composition when its name and upstream are one consolidated name. */
export function isCompositionEntry(entry:{upstreamName:string;tool:{name:string}}){
  return consolidatedNames.has(entry.tool.name)&&entry.upstreamName===entry.tool.name;
}

/** The registry schema of a composition, which the bridge validates every call with: the
 * displayed members, every member that one of its routes takes, no other member, and the
 * members a chosen route requires. */
export function registrySchema(tool:ConsolidatedTool):Json{
  const shown=tool.inputSchema as {properties:Json;required?:string[]};
  const taken=[...new Set(Object.values(tool.routes).flatMap(r=>r.keys))].filter(k=>!Object.hasOwn(shown.properties,k));
  const selector=tool.selectors[0]!;
  const chosen=(value:string):Json=>{
    const named={properties:{[selector]:{const:value}},required:[selector]};
    return value===tool.defaultRoute?{anyOf:[{not:{required:[selector]}},named]}:named;
  };
  const conditions=Object.entries(tool.routes).filter(([,r])=>r.required?.length).map(([value,r])=>({if:chosen(value),then:{required:[...r.required!]}}));
  return {...tool.inputSchema,properties:{...shown.properties,...Object.fromEntries(taken.map(k=>[k,{}]))},required:shown.required??[],
    additionalProperties:false,...(conditions.length?{allOf:conditions}:{})};
}

/** The registry entry of each composition, as `neutral-server.ts --discover` writes it. */
export function compositionEntries(){
  return consolidatedTools.map(t=>({endpoint:t.endpoint,upstreamName:t.name,tool:{name:t.name,title:t.title,description:t.description,inputSchema:registrySchema(t),
    annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:false,openWorldHint:true}}}));
}

export type InputIssue={kind:string;path:(string|number)[];missingKey?:string};
export class ConsolidatedInputInvalid extends Error{
  constructor(readonly issues:InputIssue[]){super('tool_input_invalid');}
}
export interface PlannedStep{route:string;upstream:string;endpoints:readonly string[];bridge?:Route['bridge'];args:Record<string,unknown>}
export interface Plan{
  tool:string;steps:PlannedStep[];
  /** single: the one step's answer. nested: {include, <route>: answer}. resolve_inventory: resolve then inventory.
   * prestudy: the video view's answer, with `report` when a report was also asked. */
  mode:'single'|'nested'|'resolve_inventory'|'prestudy';
  /** The prestudy video view that projects the answer of the videos read. */
  prestudyView?:'videos'|'incomplete'|'teacher_summary';
}

const has=(args:Record<string,unknown>,k:string)=>Object.hasOwn(args,k)&&args[k]!==undefined;
const invalid=(path:string,kind='invalid_value'):InputIssue=>({kind,path:[path]});
function selectorValue(args:Record<string,unknown>,name:string,allowed:readonly string[],fallback?:string):string|InputIssue{
  if(!has(args,name))return fallback??{kind:'required_property',path:[],missingKey:name};
  const value=args[name];
  return typeof value==='string'&&allowed.includes(value)?value:invalid(name);
}
function step(tool:ConsolidatedTool,route:string,args:Record<string,unknown>):PlannedStep{
  const r=tool.routes[route]!;
  const picked=Object.fromEntries(r.keys.filter(k=>has(args,k)).map(k=>[r.rename?.[k]??k,args[k]]));
  return {route,upstream:r.upstream,endpoints:r.endpoints,...(r.bridge?{bridge:r.bridge}:{}),args:picked};
}
/** Every argument other than the selectors must be taken by one of the chosen routes. */
function unused(tool:ConsolidatedTool,routes:string[],args:Record<string,unknown>,extraSelectors:string[]=[]):InputIssue[]{
  const taken=new Set(routes.flatMap(r=>tool.routes[r]!.keys));
  return Object.keys(args).filter(k=>has(args,k)&&!tool.selectors.includes(k)&&!extraSelectors.includes(k)&&!taken.has(k))
    .map(k=>({kind:'member_not_for_selector',path:[k]}));
}
const fail=(issues:InputIssue[])=>{throw new ConsolidatedInputInvalid(issues.slice(0,5));};

/** Plan a consolidated call: validate its discriminating arguments (R5) and give each
 * chosen route exactly the arguments it takes. Throws ConsolidatedInputInvalid before any request. */
export function planConsolidatedCall(name:string,raw:Record<string,unknown>):Plan{
  const tool=consolidatedTool(name);if(!tool)throw Error('unknown_tool');
  const args={...raw};
  const single=(route:string):Plan=>{const issues=unused(tool,[route],args);if(issues.length)fail(issues);return {tool:name,mode:'single',steps:[step(tool,route,args)]};};
  const pickView=(allowed:readonly string[],fallback?:string)=>{const v=selectorValue(args,tool.selectors[0]!,allowed,fallback);if(typeof v!=='string')fail([v]);return v as string;};
  switch(name){
   case 'academy_runtime_inspect':case 'academy_paper_inspect':case 'academy_textbook_catalog_get':case 'academy_cism_get':
    return single(pickView(Object.keys(tool.routes)));
   case 'academy_dailyrecord_get':
    return single(pickView(Object.keys(tool.routes),'record'));
   case 'academy_item_lookup':{
    const include=args.include;
    if(!has(args,'include'))fail([{kind:'required_property',path:[],missingKey:'include'}]);
    if(!Array.isArray(include)||include.length===0||include.some(v=>v!=='section'&&v!=='answer')||new Set(include).size!==include.length)fail([invalid('include')]);
    const routes=(['section','answer'] as const).filter(r=>(include as string[]).includes(r));
    const issues=unused(tool,[...routes],args);if(issues.length)fail(issues);
    return {tool:name,mode:routes.length===1?'single':'nested',steps:routes.map(r=>step(tool,r,args))};
   }
   case 'academy_dailytest_history':{
    if(has(args,'includeHeaders')&&typeof args.includeHeaders!=='boolean')fail([invalid('includeHeaders')]);
    const headers=args.includeHeaders!==false;
    if(has(args,'studentName')){
     if(has(args,'displayName')&&args.displayName!==args.studentName)fail([{kind:'name_conflict',path:['studentName']}]);
     args.displayName=args.studentName;delete args.studentName;
    }
    if(headers){
     if(!has(args,'displayName'))fail([{kind:'required_property',path:[],missingKey:'studentName'}]);
     return single('headers');
    }
    if(has(args,'studentId'))return single('inventory');
    if(!has(args,'displayName'))fail([{kind:'required_property',path:[],missingKey:'studentId'}]);
    const issues=unused(tool,['resolve','inventory'],args);if(issues.length)fail(issues);
    return {tool:name,mode:'resolve_inventory',steps:[step(tool,'resolve',args),step(tool,'inventory',args)]};
   }
   case 'academy_prestudy_get':{
    const view=has(args,'view')?selectorValue(args,'view',['videos','incomplete','teacher_summary']):undefined;
    if(view!==undefined&&typeof view!=='string')fail([view]);
    if(has(args,'includeReports')&&typeof args.includeReports!=='boolean')fail([invalid('includeReports')]);
    const report=args.includeReports===true;
    // specs/academy-mcp/lms-reads.md: includeReports:true and reportId come together.
    if(report!==has(args,'reportId'))fail([invalid(report?'reportId':'includeReports','reports_pairing')]);
    const routes=[...(view!==undefined||!report?['videos']:[]),...(report?['report']:[])];
    const issues=unused(tool,routes,args);if(issues.length)fail(issues);
    // `videos` is the endpoint's own default, so it is forwarded only when it differs: the old tool's request stays byte for byte.
    const steps=routes.map(r=>{const planned=step(tool,r,args);if(r==='videos'&&planned.args.view==='videos'){const {view:_view,...rest}=planned.args;return {...planned,args:rest};}return planned;});
    return routes.length===1&&routes[0]==='report'?{tool:name,mode:'single',steps}
      :{tool:name,mode:'prestudy',steps,prestudyView:(view as Plan['prestudyView'])??'videos'};
   }
  }
  throw Error('unknown_tool');
}

/** The video view's answer: 'incomplete' keeps waiting[], 'teacher_summary' keeps summary[]. */
export function projectPrestudy(data:Record<string,unknown>,view:NonNullable<Plan['prestudyView']>){
  if(view==='videos')return data;
  const {waiting,completed,summary,...rest}=data;
  return view==='incomplete'?{...rest,view,waiting}:{...rest,view,summary};
}
