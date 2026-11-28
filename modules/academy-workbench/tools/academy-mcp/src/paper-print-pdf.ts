import {z} from 'zod';
import {readFailureResult} from './read-failure.ts';
import {absentOutputPath,officialReportTarget,qualifiedOutputRoot} from './official-report-pdf.ts';
import {PcExecutorError,encodeCp949,type PcCleanup} from './pc-executor.ts';
import {pdfInfoText,printItemForPc,type PcPaperExecutor,type PrintPrincipal} from './pc-paper-executor.ts';
import {MAX_EXAM_KEY,PAPER_PRINT_CASES,paperPrintItem,type PaperPrintCase} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead';

/** `academy_paper_print_pdf_get` (specs/academy-mcp/pc-paper-executor.md): the native PDFs of an existing paper, made only on the
 * user's PC by `StudyQPdfManager.exe` and `ETestMaker.exe` (src/pc-paper-executor.ts) from a print list the Academy server
 * answers directly. `source` `paper`: the paper `paperNo` in the print case `printCase` (`teacher_paper` by default,
 * `student_paper` or `answer_paper`), one item built from `getTestExamList` as the page builds it; `source` `result`: the score
 * paper of one attempt (`testingNo`, `studentKey`), the `print_list` of its score view as the server answered it. Clinic papers
 * are printed by `academy_clinic_paper_save` with `printPdf` (`printClinicList`). Reads only; no Academy write. */
export const paperPrintSources=['paper','result'] as const;
const key=z.string().regex(/^[1-9][0-9]{0,15}$/);
export const paperPrintPdfGetSchema=z.object({
  source:z.enum(paperPrintSources),
  paperNo:key.optional(),printCase:z.enum(PAPER_PRINT_CASES).optional(),
  testingNo:key.optional(),studentKey:key.optional(),
  outputPath:z.string().min(5).max(512).optional(),
}).strict().superRefine((v,c)=>{
  if(v.source==='paper'){
    if(v.paperNo===undefined)c.addIssue({code:'custom',path:['paperNo'],message:'paper_no_required'});
    else if(Number(v.paperNo)>MAX_EXAM_KEY)c.addIssue({code:'custom',path:['paperNo'],message:'paper_no_out_of_range'});
    for(const k of ['testingNo','studentKey'] as const)if(v[k]!==undefined)c.addIssue({code:'custom',path:[k],message:'paper_takes_paper_no_and_print_case'});
    return;
  }
  for(const k of ['testingNo','studentKey'] as const)if(v[k]===undefined)c.addIssue({code:'custom',path:[k],message:k+'_required'});
  for(const k of ['paperNo','printCase'] as const)if(v[k]!==undefined)c.addIssue({code:'custom',path:[k],message:'result_takes_testing_no_and_student_key'});
});
export type PaperPrintPdfGet=z.infer<typeof paperPrintPdfGetSchema>;

export interface PaperPrintBackend {
  /** The school key (`sk`) and teacher key (`uk`) of the signed-in principal. */
  paperPrintPrincipal():Promise<PrintPrincipal>;
  /** The answer of `getTestExamList` for one paper and print case (a read of the LMS). */
  readPaperPrintSource(paperNo:string,printCase:PaperPrintCase):Promise<Record<string,unknown>>;
  /** The `print_list` of the score view of one attempt (a read of the LMS). */
  readResultPrintList(testingNo:string,studentKey:string):Promise<Record<string,unknown>[]>;
}
export interface PrintedPdf {paperType:string;pageCount:number;outputPath:string;sha256:string}
export type PrintListAnswer=
  |{status:'rendered';pdfs:PrintedPdf[];pcCleanup:PcCleanup}
  |{status:'unavailable';reasonCode:string;failure:ReturnType<typeof readFailureResult>['failure'];pdfs:PrintedPdf[];pcCleanup?:PcCleanup};

/** The launch option `--studyq-runner` of the retired Linux print executor and its five words are dropped unread, so that a launch
 * written for an earlier generation still starts; nothing of them is checked or used. */
export function withoutRetiredStudyqRunnerArguments(args:readonly string[]){
  const remainingArgs:string[]=[];
  for(let i=0;i<args.length;i++){
    if(args[i]!=='--studyq-runner'){remainingArgs.push(args[i]!);continue;}
    if(i+5>=args.length)throw Error('studyq_runner_arguments_invalid');
    i+=5;
  }
  return {remainingArgs};
}
const failureOf=(error:unknown)=>({...readFailureResult(error,'paper_print_unqualified_no_automatic_retry'),
  ...(error instanceof PcExecutorError&&error.pcCleanup?{pcCleanup:error.pcCleanup}:{})});
/** The n-th of `count` paths: the path itself for one item, else `-<n>` before `.pdf`. */
const nthPath=(path:string,n:number,count:number)=>count===1?path:path.replace(/\.pdf$/,'-'+n+'.pdf');

export class PaperPrintPdfService {
  constructor(readonly backend:PaperPrintBackend|null,readonly executor:PcPaperExecutor|null,readonly outputRoot:string,readonly now:()=>Date=()=>new Date()){}
  async get(value:unknown){
    const input=paperPrintPdfGetSchema.parse(value);
    const scope=input.source==='paper'?{paperNo:input.paperNo!,printCase:input.printCase??'teacher_paper'}:{testingNo:input.testingNo!,studentKey:input.studentKey!};
    const base={source:input.source,...scope,academyWrites:0,parentSends:0,automaticRetryAllowed:false,
      execution:'read_only_lms_pc_executor' as const,sourceContentIsUntrustedData:true};
    if(!this.executor)return {...base,status:'unavailable' as const,reasonCode:'pc_executor_not_configured'};
    if(!this.backend)return {...base,status:'unavailable' as const,reasonCode:'paper_print_backend_not_configured'};
    try{
      // The output root and a given path are settled before any LMS request.
      const root=await qualifiedOutputRoot(this.outputRoot);
      if(input.outputPath!==undefined)await officialReportTarget(root,input.outputPath,'print',null,this.now());
      const principal=await this.backend.paperPrintPrincipal();
      let list:Record<string,unknown>[],name:string,keyNo:string;
      if(input.source==='paper'){
        const printCase=input.printCase??'teacher_paper';
        list=[paperPrintItem(await this.backend.readPaperPrintSource(input.paperNo!,printCase),printCase,principal.schoolKey)];
        name='print-'+printCase.replace('_','-');keyNo=input.paperNo!;
      }else{
        list=await this.backend.readResultPrintList(input.testingNo!,input.studentKey!);
        name='print-score';keyNo=input.testingNo!;
      }
      return {...base,...await this.printList(list,{fromExamList:input.source==='paper',principal,outputPath:input.outputPath,name,keyNo})};
    }catch(error){return {...base,status:'unavailable' as const,...failureOf(error),pdfs:[]};}
  }

  /** The clinic papers of the write answer of an approved `academy_clinic_paper_save`, in the same call (never stored). */
  async printClinicList(printList:readonly unknown[],keyNo:string):Promise<PrintListAnswer|{status:'unavailable';reasonCode:string;pdfs:PrintedPdf[]}>{
    if(!this.executor)return {status:'unavailable',reasonCode:'pc_executor_not_configured',pdfs:[]};
    if(!this.backend)return {status:'unavailable',reasonCode:'paper_print_backend_not_configured',pdfs:[]};
    try{return await this.printList([...printList] as Record<string,unknown>[],{fromExamList:false,principal:await this.backend.paperPrintPrincipal(),outputPath:undefined,name:'print-clinic',keyNo});}
    catch(error){return {status:'unavailable',...failureOf(error),pdfs:[]};}
  }

  /** One PDF per item, in order. Every item, its `pdf.info` (CP949) and every output path are checked before the first PC step;
   * when an item fails the PDFs before it stay and are answered with the error. */
  async printList(list:readonly Record<string,unknown>[],options:{fromExamList:boolean;principal:PrintPrincipal;outputPath:string|undefined;name:string;keyNo:string}):Promise<PrintListAnswer>{
    const executor=this.executor!;
    if(!Array.isArray(list)||list.length===0)throw Error('paper_print_list_empty');
    if(list.length>64)throw Error('paper_print_source_unverified');
    const items=list.map(item=>printItemForPc(item,{fromExamList:options.fromExamList}));
    for(const item of items)encodeCp949(pdfInfoText(item,options.principal,String(Date.now())));
    const root=await qualifiedOutputRoot(this.outputRoot);
    const target=await officialReportTarget(root,options.outputPath,options.name,options.keyNo,this.now());
    const paths:string[]=[];
    for(let n=1;n<=items.length;n++){
      const path=nthPath(target,n,items.length);
      if(paths.includes(path))throw Error('output_path_invalid');
      paths.push(await absentOutputPath(path));
    }
    const pdfs:PrintedPdf[]=[];let cleanup:PcCleanup='done';
    for(const [n,item] of items.entries()){
      try{
        const printed=await executor.print({item,fromExamList:options.fromExamList,principal:options.principal,outputPath:paths[n]!});
        pdfs.push({paperType:printed.paperType,pageCount:printed.pageCount,outputPath:printed.outputPath,sha256:printed.sha256});
        if(printed.pcCleanup!=='done')cleanup='incomplete';
      }catch(error){
        const failure=failureOf(error);
        return {status:'unavailable',...failure,pdfs,...(failure.pcCleanup||pdfs.length?{pcCleanup:failure.pcCleanup==='incomplete'||cleanup==='incomplete'?'incomplete' as const:'done' as const}:{})};
      }
    }
    return {status:'rendered',pdfs,pcCleanup:cleanup};
  }
}
