import {lstat,mkdir,realpath} from 'node:fs/promises';
import {basename,dirname,join,resolve,sep} from 'node:path';
import {z} from 'zod';
import {readFailureResult} from './read-failure.ts';
import {directKeySchema,directKeyNames} from './dailyrecord.ts';
import type {StudyqRunner} from './studyq-runner.ts';
import {academyDateInputSchema,resolveAcademyDate,resolvedDateOf,withResolvedDate,type BookSelection} from './read-input.ts';
import {courseBookTruncated,type CourseBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/courseBookReport';
import {cismMonthDates,type CismReport} from '../../../../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol';
import {cismMonthToStudyqInput,cismWeekToStudyqInput} from '../../../../../apps/academy/dc-gang-a/src/lms/cismReportAdapter';
import {SMART_BOOK_CODE,projectDiagReport,projectFormativeReport,projectGangaReport,projectNaScoreReport,projectNewDiagReport,projectSmartBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/reportViewClient';

/** `academy_official_report_pdf_get`: one official StudyQ report PDF, rendered on this host by
 * modules/studyq-linux from read-only LMS data. `reportType` names the report; `params` holds the
 * options of that type, so a later report type extends `reportType` and its own `params` without new
 * top-level members. `course_book` (one student and book) takes `params.bookLabel`; `cism` (the
 * owner's weekly or monthly CISM, no student) takes `params.scope` `week` (with `date`, default today)
 * or `month` (with `month`, `YYYY-MM`); `diag` and `formative` (one student's diagnostic or formative
 * test, `DiagReportView` or `FaView`) take `params.testingNo`, `params.classNo` and optionally
 * `params.limitDate`; `new_diag` (one new-student application, `NewStudentReportView`) takes
 * `params.paperNo`, `params.applyCode` and optionally `params.collegeKey` (default the owner's); `ganga` and
 * `na_score` (one student's Gang-A or NA score test, `GaReportView` or `NaScoreResultView`) take what `diag`
 * takes; `smart_book` (one student's SmartBook, `SmartBookReportView`) takes `params.bookCode`. */
export const officialReportTypes=['course_book','cism','diag','new_diag','formative','ganga','na_score','smart_book'] as const;
/** The report types read from one student's `ReportViewServlet` view by test number and class. */
const testingViews={diag:'DiagReportView',formative:'FaView',ganga:'GaReportView',na_score:'NaScoreResultView'} as const;
type TestingReportType=keyof typeof testingViews;
const isTestingType=(type:string):type is TestingReportType=>Object.hasOwn(testingViews,type);
const name=z.string().trim().min(1).max(200);
const reportParams=z.object({bookLabel:z.string().trim().min(1).max(200).optional(),
  scope:z.enum(['week','month']).optional(),date:academyDateInputSchema.optional(),month:z.string().regex(/^20\d{2}-(0[1-9]|1[0-2])$/).optional(),
  testingNo:directKeySchema.optional(),classNo:directKeySchema.optional(),limitDate:z.string().regex(/^20\d{2}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/).optional(),
  paperNo:directKeySchema.optional(),applyCode:z.string().min(1).max(64).regex(/^[^\u0000-\u001f\u007f]+$/).refine(v=>v.trim()===v).optional(),
  collegeKey:directKeySchema.optional(),bookCode:z.string().regex(SMART_BOOK_CODE).optional()}).strict();
/** The `params` members each report type takes; any other member refuses. */
const paramsOf:Record<(typeof officialReportTypes)[number],readonly string[]>={course_book:['bookLabel'],cism:['scope','date','month'],
  diag:['testingNo','classNo','limitDate'],formative:['testingNo','classNo','limitDate'],new_diag:['paperNo','applyCode','collegeKey'],
  ganga:['testingNo','classNo','limitDate'],na_score:['testingNo','classNo','limitDate'],smart_book:['bookCode']};
export const officialReportPdfGetSchema=z.object({
  reportType:z.enum(officialReportTypes),
  displayName:name.optional(),studentName:name.optional(),
  courseKey:directKeySchema.optional(),bookNo:directKeySchema.optional(),studentKey:directKeySchema.optional(),
  params:reportParams.optional(),
  outputPath:z.string().min(5).max(512).optional(),
}).strict().superRefine((v,c)=>{
  const params=v.params??{};
  for(const k of Object.keys(params) as (keyof typeof params)[])
    if(params[k]!==undefined&&!paramsOf[v.reportType].includes(k))c.addIssue({code:'custom',path:['params',k],message:v.reportType+'_params_'+paramsOf[v.reportType].join('_')+'_only'});
  if(v.displayName!==undefined&&v.studentName!==undefined&&v.displayName!==v.studentName)c.addIssue({code:'custom',path:['studentName'],message:'name_conflict'});
  if(v.reportType==='cism'){
    for(const k of ['displayName','studentName','courseKey','bookNo','studentKey'] as const)if(v[k]!==undefined)c.addIssue({code:'custom',path:[k],message:'cism_takes_no_student_or_book'});
    if(params.bookLabel!==undefined)c.addIssue({code:'custom',path:['params','bookLabel'],message:'cism_takes_no_student_or_book'});
    if(params.scope===undefined)c.addIssue({code:'custom',path:['params','scope'],message:'cism_scope_required'});
    if(params.scope==='week'&&params.month!==undefined)c.addIssue({code:'custom',path:['params','month'],message:'cism_week_takes_date'});
    if(params.scope==='month'){
      if(params.month===undefined)c.addIssue({code:'custom',path:['params','month'],message:'cism_month_required'});
      if(params.date!==undefined)c.addIssue({code:'custom',path:['params','date'],message:'cism_month_takes_month'});
    }
    return;
  }
  if(isTestingType(v.reportType)){
    if(v.displayName===undefined&&v.studentName===undefined)c.addIssue({code:'custom',path:['displayName'],message:'student_name_required'});
    for(const k of ['courseKey','bookNo'] as const)if(v[k]!==undefined)c.addIssue({code:'custom',path:[k],message:v.reportType+'_takes_no_course_or_book'});
    for(const k of ['testingNo','classNo'] as const)if(params[k]===undefined)c.addIssue({code:'custom',path:['params',k],message:k+'_required'});
    return;
  }
  if(v.reportType==='smart_book'){
    if(v.displayName===undefined&&v.studentName===undefined)c.addIssue({code:'custom',path:['displayName'],message:'student_name_required'});
    for(const k of ['courseKey','bookNo'] as const)if(v[k]!==undefined)c.addIssue({code:'custom',path:[k],message:'smart_book_takes_no_course_or_book'});
    if(params.bookCode===undefined)c.addIssue({code:'custom',path:['params','bookCode'],message:'bookCode_required'});
    return;
  }
  if(v.reportType==='new_diag'){
    for(const k of ['courseKey','bookNo','studentKey'] as const)if(v[k]!==undefined)c.addIssue({code:'custom',path:[k],message:'new_diag_takes_no_course_book_or_student_key'});
    for(const k of ['paperNo','applyCode'] as const)if(params[k]===undefined)c.addIssue({code:'custom',path:['params',k],message:k+'_required'});
    return;
  }
  if(v.displayName===undefined&&v.studentName===undefined)c.addIssue({code:'custom',path:['displayName'],message:'student_name_required'});
  const given=directKeyNames.filter(k=>v[k]!==undefined);
  if(given.length>0&&given.length<3)c.addIssue({code:'custom',path:[directKeyNames.find(k=>v[k]===undefined)!],message:'direct_keys_together_required'});
});
export type OfficialReportPdfGet=z.infer<typeof officialReportPdfGetSchema>;

export interface CourseBookReportBackend {
  /** The read-only CISM week of `date` (`CismChartMain`), for `reportType` `cism`. */
  readCismReport?(date:string):Promise<CismReport>;
  /** `DiagReportView`, `FaView`, `GaReportView` or `NaScoreResultView` of one student (direct-key-lookup.md R8), for
   * `reportType` `diag`, `formative`, `ganga` and `na_score`. */
  readStudentReportView?(input:{view:'DiagReportView'|'FaView'|'GaReportView'|'NaScoreResultView';displayName:string;studentKey?:string;testingNo:string;classNo:string;limitDate?:string}):Promise<{student:{studentId:string;displayName:string};raw:Record<string,unknown>}>;
  /** `SmartBookReportView` of one student and book (direct-key-lookup.md R8), for `reportType` `smart_book`. */
  readSmartBookReportView?(input:{displayName:string;studentKey?:string;bookCode:string}):Promise<{student:{studentId:string;displayName:string};raw:Record<string,unknown>}>;
  /** `NewStudentReportView` of one application (direct-key-lookup.md R8), for `reportType` `new_diag`. */
  readNewStudentReportView?(input:{paperNo:string;applyCode:string;collegeKey?:string}):Promise<{raw:Record<string,unknown>}>;
  readCourseBookReportByName(input:{displayName:string;bookLabel?:string;courseKey?:string;bookNo?:string;studentKey?:string}):Promise<
    {kind:'report';student:{studentId:string;displayName:string};courseKey:string;bookNo:string;bindingSource?:'historical_key_registry';report:CourseBookReport;bookSelection?:BookSelection}
    |{kind:'interview_required';student:{studentId:string;displayName:string};bindings:{bookLabel:string;bookNo:string;hint?:string}[];reason:string;observedAt:string;bookSelection?:BookSelection}>;
}

/** The owner output root: a real directory (made 0700 when absent), not a symbolic link. */
export async function qualifiedOutputRoot(root:string):Promise<string>{
  await mkdir(root,{recursive:true,mode:0o700});
  const stat=await lstat(root);
  if(stat.isSymbolicLink()||!stat.isDirectory())throw Error('output_root_unqualified');
  const real=await realpath(root);
  if(real!==resolve(root))throw Error('output_root_unqualified');
  return real;
}
/** The PDF path of one call: `outputPath` (a `.pdf` name with no `..`, resolved against the root,
 * whose real parent directory lies inside the real root), else
 * `<root>/<reportType>-<bookNo>-<YYYYMMDDTHHMMSSZ>.pdf`, which holds no personal value. An
 * existing file refuses with output_exists. */
export async function officialReportPath(rootReal:string,outputPath:string|undefined,reportType:string,bookNo:string|null,now:Date):Promise<string>{
  let target:string;
  if(outputPath!==undefined){
    if(!outputPath.endsWith('.pdf')||basename(outputPath)==='.pdf'||outputPath.split(/[\\/]/).includes('..')||outputPath.includes('\0'))throw Error('output_path_invalid');
    const wanted=resolve(rootReal,outputPath);
    let parent:string;
    try{parent=await realpath(dirname(wanted));}catch{throw Error('output_path_invalid');}
    if(parent!==rootReal&&!parent.startsWith(rootReal+sep))throw Error('output_path_invalid');
    target=join(parent,basename(wanted));
  }else{
    if(bookNo===null)throw Error('output_path_invalid');
    target=join(rootReal,`${reportType}-${bookNo}-${now.toISOString().replace(/\.\d{3}Z$/,'Z').replace(/[-:]/g,'')}.pdf`);
  }
  try{await lstat(target);}catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return target;throw Error('output_path_invalid');}
  throw Error('output_exists');
}

const testSummary=(input:{title:string;test_date:string;questions:unknown[]})=>({title:input.title,testDate:input.test_date,questions:input.questions.length});

export class OfficialReportPdfService {
  constructor(readonly backend:CourseBookReportBackend|null,readonly runner:StudyqRunner|null,readonly outputRoot:string,readonly now:()=>Date=()=>new Date()){}
  async get(value:unknown){
    const input=officialReportPdfGetSchema.parse(value);
    if(input.reportType==='cism')return this.getCism(input);
    if(input.reportType!=='course_book')return this.getReportView(input);
    const displayName=(input.displayName??input.studentName)!;
    const base={reportType:input.reportType,displayName,academyWrites:0,parentSends:0,automaticRetryAllowed:false,
      execution:'read_only_lms_local_pdf' as const,sourceContentIsUntrustedData:true};
    if(!this.backend)return {...base,status:'unavailable' as const,reasonCode:'official_report_backend_not_configured'};
    if(!this.runner)return {...base,status:'unavailable' as const,reasonCode:'studyq_runner_not_configured'};
    try{
      // The output root and a given path are settled before any LMS request.
      const root=await qualifiedOutputRoot(this.outputRoot);
      if(input.outputPath!==undefined)await officialReportPath(root,input.outputPath,input.reportType,null,this.now());
      const read=await this.backend.readCourseBookReportByName({displayName,...(input.params?.bookLabel?{bookLabel:input.params.bookLabel}:{}),
        ...(input.courseKey!==undefined?{courseKey:input.courseKey,bookNo:input.bookNo!,studentKey:input.studentKey!}:{})});
      if(read.student.displayName!==displayName)throw Error('official_report_scope_conflict');
      if(read.kind==='interview_required')return {...base,status:'interview_required' as const,...read};
      if(input.courseKey!==undefined&&(read.courseKey!==input.courseKey||read.bookNo!==input.bookNo))throw Error('official_report_scope_conflict');
      const path=await officialReportPath(root,input.outputPath,input.reportType,read.bookNo,this.now());
      const report=read.report,input11=report.studyqInput;
      const summary={student:read.student,courseKey:read.courseKey,bookNo:read.bookNo,...(read.bindingSource?{bindingSource:read.bindingSource}:{}),
        ...(read.bookSelection?{bookSelection:read.bookSelection}:{}),
        reportSummary:{bookTitle:report.metadata.bookTitle,period:input11.period,myAverage:input11.my_average,allAverage:input11.all_average,chapterCount:report.sourceChapterCount},
        ...(courseBookTruncated(report.truncation)?{truncation:report.truncation}:{})};
      const rendered=await this.runner.renderCourseBook(input11,path);
      if(rendered.status==='refused')return {...base,...summary,status:'refused' as const,reasonCode:'studyq_'+rendered.code,studyqDetail:rendered.detail};
      return {...base,...summary,status:'rendered' as const,outputPath:rendered.outputPath,sha256:rendered.sha256,pages:rendered.pages,
        parityOk:rendered.parityOk,parityMismatches:rendered.parityMismatches,parityRawRn:rendered.parityRawRn};
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'official_report_unqualified_no_automatic_retry')};}
  }
  /** `reportType` `diag`, `formative`, `ganga`, `na_score` (one student, `DiagReportView`, `FaView`, `GaReportView` or
   * `NaScoreResultView`), `smart_book` (one student and book, `SmartBookReportView`) and `new_diag` (one application,
   * `NewStudentReportView`): one read-only POST of ReportViewServlet, projected to the strict input of the type,
   * rendered and parity-checked. The output path is settled before any LMS request. */
  private async getReportView(input:OfficialReportPdfGet){
    const reportType=input.reportType as Exclude<OfficialReportPdfGet['reportType'],'course_book'|'cism'>,params=input.params!;
    const displayName=input.displayName??input.studentName;
    const base={reportType,...(displayName!==undefined?{displayName}:{}),academyWrites:0,parentSends:0,automaticRetryAllowed:false,
      execution:'read_only_lms_local_pdf' as const,sourceContentIsUntrustedData:true};
    if(!this.backend)return {...base,status:'unavailable' as const,reasonCode:'official_report_backend_not_configured'};
    if(!this.runner)return {...base,status:'unavailable' as const,reasonCode:'studyq_runner_not_configured'};
    try{
      const root=await qualifiedOutputRoot(this.outputRoot);
      if(input.outputPath!==undefined)await officialReportPath(root,input.outputPath,reportType,null,this.now());
      let studyqInput:{student:string},student:{studentId:string;displayName:string}|undefined,keyNo:string,scope:Record<string,string>,reportSummary:Record<string,unknown>;
      if(reportType==='new_diag'){
        const read=this.backend.readNewStudentReportView?.bind(this.backend);
        if(!read)throw Error('report_view_source_port_unavailable');
        const answer=await read({paperNo:params.paperNo!,applyCode:params.applyCode!,...(params.collegeKey!==undefined?{collegeKey:params.collegeKey}:{})});
        const application=projectNewDiagReport(answer.raw);
        studyqInput=application;
        // A name the caller gives must be the application's student.
        if(displayName!==undefined&&application.student!==displayName)throw Error('official_report_scope_conflict');
        keyNo=params.paperNo!;scope={paperNo:params.paperNo!};
        reportSummary=testSummary(application);
      }else if(reportType==='smart_book'){
        const read=this.backend.readSmartBookReportView?.bind(this.backend);
        if(!read)throw Error('report_view_source_port_unavailable');
        const answer=await read({displayName:displayName!,...(input.studentKey!==undefined?{studentKey:input.studentKey}:{}),bookCode:params.bookCode!});
        if(answer.student.displayName!==displayName||(input.studentKey!==undefined&&answer.student.studentId!==input.studentKey))throw Error('official_report_scope_conflict');
        student=answer.student;
        const book=projectSmartBookReport(answer.raw,params.bookCode!);
        studyqInput=book;
        // The report must be the resolved student's.
        if(book.student!==answer.student.displayName)throw Error('official_report_scope_conflict');
        keyNo=params.bookCode!;scope={bookCode:params.bookCode!};
        reportSummary={period:book.period,myAverage:book.my_average,allAverage:book.all_average,chapterCount:book.bunit_list.length,
          questions:book.bunit_list.reduce((sum,c)=>sum+c.questions.length,0)};
      }else{
        const read=this.backend.readStudentReportView?.bind(this.backend);
        if(!read)throw Error('report_view_source_port_unavailable');
        const answer=await read({view:testingViews[reportType],displayName:displayName!,...(input.studentKey!==undefined?{studentKey:input.studentKey}:{}),
          testingNo:params.testingNo!,classNo:params.classNo!,...(params.limitDate!==undefined?{limitDate:params.limitDate}:{})});
        if(answer.student.displayName!==displayName||(input.studentKey!==undefined&&answer.student.studentId!==input.studentKey))throw Error('official_report_scope_conflict');
        student=answer.student;
        const project={diag:projectDiagReport,formative:projectFormativeReport,ganga:projectGangaReport,na_score:projectNaScoreReport}[reportType];
        const test=project(answer.raw);
        studyqInput=test;
        // The report must be the resolved student's.
        if(test.student!==answer.student.displayName)throw Error('official_report_scope_conflict');
        keyNo=params.testingNo!;scope={testingNo:params.testingNo!,classNo:params.classNo!};
        reportSummary=testSummary(test);
      }
      const path=await officialReportPath(root,input.outputPath,reportType,keyNo,this.now());
      const summary={...(student?{student}:{}),...scope,reportSummary};
      const rendered=await this.runner.renderReport(reportType,studyqInput,path);
      if(rendered.status==='refused')return {...base,...summary,status:'refused' as const,reasonCode:'studyq_'+rendered.code,studyqDetail:rendered.detail};
      return {...base,...summary,status:'rendered' as const,outputPath:rendered.outputPath,sha256:rendered.sha256,pages:rendered.pages,
        parityOk:rendered.parityOk,parityMismatches:rendered.parityMismatches,parityRawRn:rendered.parityRawRn};
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'official_report_unqualified_no_automatic_retry')};}
  }
  /** `reportType` `cism`: the owner's CISM week or month, read-only, rendered and parity-checked. */
  private async getCism(input:OfficialReportPdfGet){
    const scope=input.params!.scope!;
    const base={reportType:'cism' as const,scope,academyWrites:0,parentSends:0,automaticRetryAllowed:false,
      execution:'read_only_lms_local_pdf' as const,sourceContentIsUntrustedData:true};
    if(!this.backend)return {...base,status:'unavailable' as const,reasonCode:'official_report_backend_not_configured'};
    if(!this.runner)return {...base,status:'unavailable' as const,reasonCode:'studyq_runner_not_configured'};
    try{
      const root=await qualifiedOutputRoot(this.outputRoot);
      if(input.outputPath!==undefined)await officialReportPath(root,input.outputPath,'cism',null,this.now());
      // The date or month is settled before any LMS request, and a failure after it still states it.
      const target=scope==='week'?resolveAcademyDate(input.params!.date??'today',this.now().getTime()):input.params!.month!;
      return await withResolvedDate(target,async()=>{
        const read=this.backend!.readCismReport?.bind(this.backend);
        if(!read)throw Error('cism_source_port_unavailable');
        let studyqInput,reports:CismReport[];
        if(scope==='week'){reports=[await read(target)];studyqInput=cismWeekToStudyqInput(reports[0]!);}
        else{reports=[];for(const date of cismMonthDates(target))reports.push(await read(date));studyqInput=cismMonthToStudyqInput(target,reports);}
        const path=await officialReportPath(root,input.outputPath,'cism-'+scope,target,this.now());
        const summary={...(scope==='week'?{date:target}:{month:target}),weeks:reports.length,
          reportSummary:{weekLabel:studyqInput.week_label,period:studyqInput.period,studentRows:studyqInput.chart_data.length,
            filledSections:studyqInput.report_list.filter(s=>s.text.trim()).length}};
        const rendered=await this.runner!.renderCism(studyqInput,path);
        if(rendered.status==='refused')return {...base,...summary,status:'refused' as const,reasonCode:'studyq_'+rendered.code,studyqDetail:rendered.detail};
        return {...base,...summary,status:'rendered' as const,outputPath:rendered.outputPath,sha256:rendered.sha256,pages:rendered.pages,
          parityOk:rendered.parityOk,parityMismatches:rendered.parityMismatches,parityRawRn:rendered.parityRawRn};
      });
    }catch(error){const resolved=resolvedDateOf(error);return {...base,...(resolved?(scope==='week'?{date:resolved}:{month:resolved}):{}),status:'unavailable' as const,...readFailureResult(error,'official_report_unqualified_no_automatic_retry')};}
  }
}
