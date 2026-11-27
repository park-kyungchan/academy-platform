import {lstat,mkdir,realpath} from 'node:fs/promises';
import {basename,dirname,join,resolve,sep} from 'node:path';
import {z} from 'zod';
import {readFailureResult} from './read-failure.ts';
import {directKeySchema,directKeyNames} from './dailyrecord.ts';
import type {StudyqRunner} from './studyq-runner.ts';
import type {BookSelection} from './read-input.ts';
import {courseBookTruncated,type CourseBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/courseBookReport';

/** `academy_official_report_pdf_get`: one official StudyQ report PDF of one student, rendered on
 * this host by modules/studyq-linux from read-only LMS data. `reportType` names the report;
 * `params` holds the options of that type, so a later report type extends `reportType` and its
 * own `params` without new top-level members. `course_book` takes `params.bookLabel`. */
export const officialReportTypes=['course_book'] as const;
const name=z.string().trim().min(1).max(200);
const courseBookParams=z.object({bookLabel:z.string().trim().min(1).max(200).optional()}).strict();
export const officialReportPdfGetSchema=z.object({
  reportType:z.enum(officialReportTypes),
  displayName:name.optional(),studentName:name.optional(),
  courseKey:directKeySchema.optional(),bookNo:directKeySchema.optional(),studentKey:directKeySchema.optional(),
  params:courseBookParams.optional(),
  outputPath:z.string().min(5).max(512).optional(),
}).strict().superRefine((v,c)=>{
  if(v.displayName===undefined&&v.studentName===undefined)c.addIssue({code:'custom',path:['displayName'],message:'student_name_required'});
  if(v.displayName!==undefined&&v.studentName!==undefined&&v.displayName!==v.studentName)c.addIssue({code:'custom',path:['studentName'],message:'name_conflict'});
  const given=directKeyNames.filter(k=>v[k]!==undefined);
  if(given.length>0&&given.length<3)c.addIssue({code:'custom',path:[directKeyNames.find(k=>v[k]===undefined)!],message:'direct_keys_together_required'});
});
export type OfficialReportPdfGet=z.infer<typeof officialReportPdfGetSchema>;

export interface CourseBookReportBackend {
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
}
