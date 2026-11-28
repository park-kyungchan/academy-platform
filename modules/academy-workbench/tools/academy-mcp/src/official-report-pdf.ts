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

/** `academy_official_report_pdf_get`: one official StudyQ report PDF, rendered on this host by
 * modules/studyq-linux from read-only LMS data. `reportType` names the report; `params` holds the
 * options of that type, so a later report type extends `reportType` and its own `params` without new
 * top-level members. `course_book` (one student and book) takes `params.bookLabel`; `cism` (the
 * owner's weekly or monthly CISM, no student) takes `params.scope` `week` (with `date`, default today)
 * or `month` (with `month`, `YYYY-MM`). */
export const officialReportTypes=['course_book','cism'] as const;
const name=z.string().trim().min(1).max(200);
const reportParams=z.object({bookLabel:z.string().trim().min(1).max(200).optional(),
  scope:z.enum(['week','month']).optional(),date:academyDateInputSchema.optional(),month:z.string().regex(/^20\d{2}-(0[1-9]|1[0-2])$/).optional()}).strict();
export const officialReportPdfGetSchema=z.object({
  reportType:z.enum(officialReportTypes),
  displayName:name.optional(),studentName:name.optional(),
  courseKey:directKeySchema.optional(),bookNo:directKeySchema.optional(),studentKey:directKeySchema.optional(),
  params:reportParams.optional(),
  outputPath:z.string().min(5).max(512).optional(),
}).strict().superRefine((v,c)=>{
  const params=v.params??{};
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
  for(const k of ['scope','date','month'] as const)if(params[k]!==undefined)c.addIssue({code:'custom',path:['params',k],message:'course_book_params_bookLabel_only'});
  if(v.displayName===undefined&&v.studentName===undefined)c.addIssue({code:'custom',path:['displayName'],message:'student_name_required'});
  if(v.displayName!==undefined&&v.studentName!==undefined&&v.displayName!==v.studentName)c.addIssue({code:'custom',path:['studentName'],message:'name_conflict'});
  const given=directKeyNames.filter(k=>v[k]!==undefined);
  if(given.length>0&&given.length<3)c.addIssue({code:'custom',path:[directKeyNames.find(k=>v[k]===undefined)!],message:'direct_keys_together_required'});
});
export type OfficialReportPdfGet=z.infer<typeof officialReportPdfGetSchema>;

export interface CourseBookReportBackend {
  /** The read-only CISM week of `date` (`CismChartMain`), for `reportType` `cism`. */
  readCismReport?(date:string):Promise<CismReport>;
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

export class OfficialReportPdfService {
  constructor(readonly backend:CourseBookReportBackend|null,readonly runner:StudyqRunner|null,readonly outputRoot:string,readonly now:()=>Date=()=>new Date()){}
  async get(value:unknown){
    const input=officialReportPdfGetSchema.parse(value);
    if(input.reportType==='cism')return this.getCism(input);
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
